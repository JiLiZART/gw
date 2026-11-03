//! `gw status` view.

use crate::style::{BRANCH_BOLD, DIM, GREEN, RED, UPSTREAM, YELLOW, paint, sanitize};
use gw_parse::{Entry, Numstat, Status};
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
    let groups: [(&str, &str, Vec<(char, String, Option<Option<(u64, u64)>>)>); 4] = [
        ("Conflicts", RED, st.entries.iter().filter(|e| e.unmerged).map(|e| ('U', rel(&e.path), None)).collect()),
        (
            "Staged",
            GREEN,
            st.entries
                .iter()
                .filter(|e| !e.unmerged && !untracked(e) && e.x != '.')
                .map(|e| (e.x, name(e), stats.staged.get(&e.path).copied()))
                .collect(),
        ),
        (
            "Changed",
            YELLOW,
            st.entries
                .iter()
                .filter(|e| !e.unmerged && !untracked(e) && e.y != '.')
                .map(|e| (e.y, rel(&e.path), stats.unstaged.get(&e.path).copied()))
                .collect(),
        ),
        ("Untracked", DIM, st.entries.iter().filter(|e| untracked(e)).map(|e| ('?', rel(&e.path), stats.untracked.get(&e.path).copied())).collect()),
    ];
    if groups.iter().all(|g| g.2.is_empty()) {
        out += &paint("clean", DIM, color);
        out.push('\n');
        return out;
    }
    for (title, sgr, files) in groups.iter().filter(|g| !g.2.is_empty()) {
        out += &format!("{}\n", paint(&format!("{title} ({})", files.len()), sgr, color));
        let width = files.iter().filter(|f| f.2.is_some()).map(|f| f.1.width()).max().unwrap_or(0);
        for (code, path, n) in files {
            let code = paint(&code.to_string(), sgr, color);
            let stat = stat(*n, color);
            if stat.is_empty() {
                out += &format!("  {code} {path}\n");
            } else {
                out += &format!("  {code} {path}{}  {stat}\n", " ".repeat(width.saturating_sub(path.width())));
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
