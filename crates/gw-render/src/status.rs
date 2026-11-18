//! `gw status` view.

use crate::style::{BRANCH, BRANCH_BOLD, DIM, GREEN, RED, UPSTREAM, YELLOW, paint, sanitize};
use gw_parse::{Entry, Numstat, Status, SubmoduleState};
use unicode_width::UnicodeWidthStr;

/// Rewrites a repo-root relative path to be relative to `prefix` (from `git rev-parse --show-prefix`).
pub fn relative(path: &str, prefix: &str) -> String {
    // Untracked directories come as `dir/`: resolve the dir itself, then restore the slash.
    if let Some(dir) = path.strip_suffix('/') {
        return relative(dir, prefix) + "/";
    }
    let pre: Vec<&str> = prefix.split('/').filter(|s| !s.is_empty()).collect();
    let parts: Vec<&str> = path.split('/').collect();
    let dirs = &parts[..parts.len() - 1];
    let common = pre.iter().zip(dirs).take_while(|(a, b)| a == b).count();
    let mut out = "../".repeat(pre.len() - common);
    out += &parts[common..].join("/");
    out
}

/// `a/b/c.rs` -> (`a/b`, `c.rs`); an untracked dir `a/b/` -> (`a`, `b/`); top-level -> (``, name).
fn split_dir(path: &str) -> (String, String) {
    let (p, slash) = match path.strip_suffix('/') {
        Some(p) => (p, "/"),
        None => (path, ""),
    };
    match p.rsplit_once('/') {
        Some((dir, base)) => (dir.to_string(), format!("{base}{slash}")),
        None => (String::new(), path.to_string()),
    }
}

fn count(n: u32, sgr: &str, arrow: &str, color: bool) -> String {
    paint(&format!("{arrow}{n}"), if n > 0 { sgr } else { DIM }, color)
}

/// Line counts per group, keyed by repo-relative path. Missing entries show no stat.
#[derive(Default)]
pub struct LineStats {
    pub staged: Numstat,
    pub unstaged: Numstat,
    pub untracked: Numstat,
    /// Checked-out commit per submodule path (from `git submodule status`), when known.
    pub checkouts: std::collections::HashMap<String, String>,
}

fn short(sha: &str) -> &str {
    &sha[..sha.len().min(7)]
}

/// `<recorded> → <current>  <labels>` for a submodule line.
fn submodule_detail(e: &Entry, sub: &SubmoduleState, checkouts: &std::collections::HashMap<String, String>, color: bool) -> String {
    // A newly added submodule has no commit in HEAD (all zeros): its recorded commit is the index one.
    let recorded = if sub.head.bytes().all(|b| b == b'0') { &sub.index } else { &sub.head };
    let current = if sub.commit_changed { checkouts.get(&e.path).unwrap_or(recorded) } else { &sub.index };
    let mut s = if current == recorded {
        paint(short(recorded), DIM, color)
    } else {
        format!("{} → {}", paint(short(recorded), DIM, color), paint(short(current), YELLOW, color))
    };
    let labels: Vec<String> = [
        (sub.head != sub.index && recorded == &sub.head, "staged", GREEN),
        (sub.commit_changed, "new commits", YELLOW),
        (sub.modified, "modified content", YELLOW),
        (sub.untracked, "untracked content", DIM),
    ]
    .into_iter()
    .filter(|l| l.0)
    .map(|(_, t, sgr)| paint(t, sgr, color))
    .collect();
    if !labels.is_empty() {
        s += &format!("  {}", labels.join(", "));
    }
    s
}

/// `+a -d` (GitHub/GitLab style), `bin` for binary files.
fn stat(n: Option<Option<(u64, u64)>>, color: bool) -> String {
    match n {
        Some(Some((a, d))) => format!(
            "{} {}",
            paint(&format!("+{a}"), if a > 0 { GREEN } else { DIM }, color),
            paint(&format!("-{d}"), if d > 0 { RED } else { DIM }, color)
        ),
        Some(None) => paint("bin", DIM, color),
        None => String::new(),
    }
}

/// A `gw status` group, in display order.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Group {
    Conflicts,
    Staged,
    Changed,
    Submodules,
    Untracked,
}

impl Group {
    pub fn title(self) -> &'static str {
        match self {
            Group::Conflicts => "Conflicts",
            Group::Staged => "Staged",
            Group::Changed => "Changed",
            Group::Submodules => "Submodules",
            Group::Untracked => "Untracked",
        }
    }

    pub fn sgr(self) -> &'static str {
        match self {
            Group::Conflicts => RED,
            Group::Staged => GREEN,
            Group::Changed => YELLOW,
            Group::Submodules => BRANCH,
            Group::Untracked => DIM,
        }
    }

    /// The one-letter change code shown for `e` in this group.
    pub fn code(self, e: &Entry) -> char {
        match self {
            Group::Conflicts => 'U',
            Group::Staged => e.x,
            Group::Changed => e.y,
            Group::Submodules if e.x == 'A' => 'A',
            Group::Submodules => 'M',
            Group::Untracked => '?',
        }
    }

    /// Renames show `old -> new` in these groups.
    pub fn shows_orig(self) -> bool {
        matches!(self, Group::Staged | Group::Submodules)
    }
}

/// Entries per group, all five groups in display order (some may be empty).
/// A file staged and changed again appears in both Staged and Changed.
pub fn groups(st: &Status) -> Vec<(Group, Vec<&Entry>)> {
    let file = |e: &&Entry| !e.unmerged && e.x != '?' && e.submodule.is_none();
    let pick = |f: &dyn Fn(&&Entry) -> bool| st.entries.iter().filter(|e| f(e)).collect::<Vec<_>>();
    vec![
        (Group::Conflicts, pick(&|e| e.unmerged)),
        (Group::Staged, pick(&|e| file(e) && e.x != '.')),
        (Group::Changed, pick(&|e| file(e) && e.y != '.')),
        (Group::Submodules, pick(&|e| e.submodule.is_some() && !e.unmerged)),
        (Group::Untracked, pick(&|e| e.x == '?')),
    ]
}

pub fn render_status(st: &Status, prefix: &str, stats: &LineStats, color: bool) -> String {
    let mut out = match (&st.head, &st.oid) {
        (Some(h), _) => paint(&sanitize(h), BRANCH_BOLD, color),
        (None, Some(oid)) => paint(&format!("detached at {}", &oid[..oid.len().min(7)]), YELLOW, color),
        (None, None) => paint("detached", YELLOW, color),
    };
    if st.head.is_some() {
        match &st.upstream {
            Some(up) => {
                out += &format!(" -> {}", paint(&sanitize(up), UPSTREAM, color));
                if let Some((a, b)) = st.ahead_behind {
                    out += &format!("  {} {}", count(a, GREEN, "↑", color), count(b, YELLOW, "↓", color));
                }
            }
            None => out += &format!("  {}", paint("no upstream", DIM, color)),
        }
    }
    out.push('\n');

    // -z output is unquoted: file names may carry escape sequences.
    let rel = |p: &str| split_dir(&sanitize(&relative(p, prefix)));
    // A rename within one directory shows under it; across directories it shows in full at the top.
    let place = |e: &Entry| match (&e.orig, rel(&e.path)) {
        (Some(o), (dir, base)) if rel(o).0 == dir => (dir, format!("{} -> {base}", rel(o).1)),
        (Some(o), _) => (String::new(), format!("{} -> {}", sanitize(&relative(o, prefix)), sanitize(&relative(&e.path, prefix)))),
        (None, p) => p,
    };
    // (code, dir, name, stats, submodule detail)
    type Line = (char, String, String, Option<Option<(u64, u64)>>, String);
    let groups: Vec<(&str, &str, Vec<Line>)> = groups(st)
        .into_iter()
        .map(|(g, entries)| {
            let mut lines: Vec<Line> = entries
                .into_iter()
                .map(|e| {
                    let (dir, path) = if g.shows_orig() { place(e) } else { rel(&e.path) };
                    let n = match g {
                        Group::Staged => stats.staged.get(&e.path).copied(),
                        Group::Changed => stats.unstaged.get(&e.path).copied(),
                        Group::Untracked => stats.untracked.get(&e.path).copied(),
                        _ => None,
                    };
                    let detail = match (g, &e.submodule) {
                        (Group::Submodules, Some(sub)) => submodule_detail(e, sub, &stats.checkouts, color),
                        _ => String::new(),
                    };
                    (g.code(e), dir, path, n, detail)
                })
                .collect();
            // Stable: files keep git's order within a directory; top-level files come first.
            lines.sort_by(|a, b| a.1.cmp(&b.1));
            (g.title(), g.sgr(), lines)
        })
        .collect();
    if groups.iter().all(|g| g.2.is_empty()) {
        out += &paint("clean", DIM, color);
        out.push('\n');
        return out;
    }
    for (title, sgr, files) in groups.iter().filter(|g| !g.2.is_empty()) {
        out += &format!("{}\n", paint(&format!("{title} ({})", files.len()), sgr, color));
        // Files under a directory header are indented two more columns; stats align across both.
        let indent = |dir: &str| if dir.is_empty() { 0 } else { 2 };
        let width = files.iter().filter(|f| f.3.is_some() || !f.4.is_empty()).map(|f| indent(&f.1) + f.2.width()).max().unwrap_or(0);
        let mut cur: Option<&str> = None;
        for (code, dir, path, n, detail) in files {
            if !dir.is_empty() && cur != Some(dir) {
                out += &format!("  {}\n", paint(&format!("{dir}/"), DIM, color));
            }
            cur = Some(dir);
            let pad = " ".repeat(indent(dir));
            let code = paint(&code.to_string(), sgr, color);
            let tail = if detail.is_empty() { stat(*n, color) } else { detail.clone() };
            if tail.is_empty() {
                out += &format!("  {pad}{code} {path}\n");
            } else {
                out += &format!("  {pad}{code} {path}{}  {tail}\n", " ".repeat(width.saturating_sub(indent(dir) + path.width())));
            }
        }
    }
    // Sum of every count shown; binary files and files without stats add nothing.
    let (a, d) = groups.iter().flat_map(|g| &g.2).filter_map(|f| f.3.flatten()).fold((0, 0), |t, (a, d)| (t.0 + a, t.1 + d));
    if groups.iter().any(|g| g.2.iter().any(|f| f.3.is_some())) {
        out += &format!("Total {}\n", stat(Some(Some((a, d))), color));
    }
    out
}

#[cfg(test)]
mod tests {
    use super::relative;

    #[test]
    fn relative_paths() {
        assert_eq!(relative("src/a.rs", "src/"), "a.rs");
        assert_eq!(relative("b.rs", "src/"), "../b.rs");
        assert_eq!(relative("src2/x", "src/"), "../src2/x");
        assert_eq!(relative("a/b/c", "a/d/"), "../b/c");
        assert_eq!(relative("x", ""), "x");
        assert_eq!(relative("src/", "src/"), "../src/");
        assert_eq!(relative("src/d/", "src/"), "d/");
    }
}
