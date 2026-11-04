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
    let rel = |p: &str| sanitize(&relative(p, prefix));
    let name = |e: &Entry| match &e.orig {
        Some(o) => format!("{} -> {}", rel(o), rel(&e.path)),
        None => rel(&e.path),
    };
    let untracked = |e: &Entry| e.x == '?';
    let file = |e: &&Entry| !e.unmerged && !untracked(e) && e.submodule.is_none();
    // (code, path, stats, submodule detail)
    type Line = (char, String, Option<Option<(u64, u64)>>, String);
    let groups: [(&str, &str, Vec<Line>); 5] = [
        ("Conflicts", RED, st.entries.iter().filter(|e| e.unmerged).map(|e| ('U', rel(&e.path), None, String::new())).collect()),
        (
            "Staged",
            GREEN,
            st.entries
                .iter()
                .filter(|e| file(e) && e.x != '.')
                .map(|e| (e.x, name(e), stats.staged.get(&e.path).copied(), String::new()))
                .collect(),
        ),
        (
            "Changed",
            YELLOW,
            st.entries
                .iter()
                .filter(|e| file(e) && e.y != '.')
                .map(|e| (e.y, rel(&e.path), stats.unstaged.get(&e.path).copied(), String::new()))
                .collect(),
        ),
        (
            "Submodules",
            BRANCH,
            st.entries
                .iter()
                .filter_map(|e| {
                    let sub = e.submodule.as_ref().filter(|_| !e.unmerged)?;
                    let code = if e.x == 'A' { 'A' } else { 'M' };
                    Some((code, name(e), None, submodule_detail(e, sub, &stats.checkouts, color)))
                })
                .collect(),
        ),
        (
            "Untracked",
            DIM,
            st.entries.iter().filter(|e| untracked(e)).map(|e| ('?', rel(&e.path), stats.untracked.get(&e.path).copied(), String::new())).collect(),
        ),
    ];
    if groups.iter().all(|g| g.2.is_empty()) {
        out += &paint("clean", DIM, color);
        out.push('\n');
        return out;
    }
    for (title, sgr, files) in groups.iter().filter(|g| !g.2.is_empty()) {
        out += &format!("{}\n", paint(&format!("{title} ({})", files.len()), sgr, color));
        let width = files.iter().filter(|f| f.2.is_some() || !f.3.is_empty()).map(|f| f.1.width()).max().unwrap_or(0);
        for (code, path, n, detail) in files {
            let code = paint(&code.to_string(), sgr, color);
            let tail = if detail.is_empty() { stat(*n, color) } else { detail.clone() };
            if tail.is_empty() {
                out += &format!("  {code} {path}\n");
            } else {
                out += &format!("  {code} {path}{}  {tail}\n", " ".repeat(width.saturating_sub(path.width())));
            }
        }
    }
    // Sum of every count shown; binary files and files without stats add nothing.
    let (a, d) = groups.iter().flat_map(|g| &g.2).filter_map(|f| f.2.flatten()).fold((0, 0), |t, (a, d)| (t.0 + a, t.1 + d));
    if groups.iter().any(|g| g.2.iter().any(|f| f.2.is_some())) {
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
