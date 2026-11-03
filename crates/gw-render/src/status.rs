//! `gw status` view.

use crate::style::{BRANCH, BRANCH_BOLD, DIM, GREEN, RED, YELLOW, paint};
use gw_parse::{Entry, Status};

/// Rewrites a repo-root relative path to be relative to `prefix` (from `git rev-parse --show-prefix`).
pub fn relative(path: &str, prefix: &str) -> String {
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

pub fn render_status(st: &Status, prefix: &str, color: bool) -> String {
    let mut out = match (&st.head, &st.oid) {
        (Some(h), _) => paint(h, BRANCH_BOLD, color),
        (None, Some(oid)) => paint(&format!("detached at {}", &oid[..oid.len().min(7)]), YELLOW, color),
        (None, None) => paint("detached", YELLOW, color),
    };
    if st.head.is_some() {
        match &st.upstream {
            Some(up) => {
                out += &format!(" -> {}", paint(up, BRANCH, color));
                if let Some((a, b)) = st.ahead_behind {
                    out += &format!("  {} {}", count(a, GREEN, "↑", color), count(b, YELLOW, "↓", color));
                }
            }
            None => out += &format!("  {}", paint("no upstream", DIM, color)),
        }
    }
    out.push('\n');

    let rel = |p: &str| relative(p, prefix);
    let name = |e: &Entry| match &e.orig {
        Some(o) => format!("{} -> {}", rel(o), rel(&e.path)),
        None => rel(&e.path),
    };
    let untracked = |e: &Entry| e.x == '?';
    let groups: [(&str, &str, Vec<(char, String)>); 4] = [
        ("Conflicts", RED, st.entries.iter().filter(|e| e.unmerged).map(|e| ('U', rel(&e.path))).collect()),
        ("Staged", GREEN, st.entries.iter().filter(|e| !e.unmerged && !untracked(e) && e.x != '.').map(|e| (e.x, name(e))).collect()),
        ("Changed", YELLOW, st.entries.iter().filter(|e| !e.unmerged && !untracked(e) && e.y != '.').map(|e| (e.y, rel(&e.path))).collect()),
        ("Untracked", DIM, st.entries.iter().filter(|e| untracked(e)).map(|e| ('?', rel(&e.path))).collect()),
    ];
    if groups.iter().all(|g| g.2.is_empty()) {
        out += &paint("clean", DIM, color);
        out.push('\n');
        return out;
    }
    for (title, sgr, files) in groups.iter().filter(|g| !g.2.is_empty()) {
        out += &format!("{}\n", paint(&format!("{title} ({})", files.len()), sgr, color));
        for (code, path) in files {
            out += &format!("  {} {path}\n", paint(&code.to_string(), sgr, color));
        }
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
    }
}
