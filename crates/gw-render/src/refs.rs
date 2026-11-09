//! Target header, ref update list, and pull summary.

use crate::style::{BOLD, BRANCH, DIM, GREEN, LINK, RED, YELLOW, link, paint, sanitize};
use crate::remote_box::titled_box;
use gw_parse::{PullLine, PullResult, RefKind, RefUpdate, classify_pull_line};
use unicode_width::UnicodeWidthStr;

fn short(sha: &str) -> &str {
    &sha[..sha.len().min(7)]
}

/// `-> <target>` line; URL forms become links.
pub fn render_target(target: &str, color: bool) -> String {
    format!("{} {}\n", paint("->", BOLD, color), target_text(target, color))
}

fn target_text(target: &str, color: bool) -> String {
    let target = sanitize(target);
    if target.contains("://") { link(&target, &paint(&target, LINK, color)) } else { target }
}

pub fn all_up_to_date(refs: &[RefUpdate]) -> bool {
    refs.iter().all(|r| r.kind == RefKind::UpToDate)
}

pub fn render_up_to_date(color: bool) -> String {
    format!("{}\n", paint("Everything up-to-date", DIM, color))
}

pub fn render_ref(r: &RefUpdate, color: bool) -> String {
    let (marker, sgr) = match r.kind {
        RefKind::FastForward => ("✓", GREEN),
        RefKind::Forced => ("± forced", YELLOW),
        RefKind::New => ("+ new", GREEN),
        RefKind::Deleted => ("- deleted", RED),
        RefKind::Rejected => ("✗ rejected", RED),
        RefKind::UpToDate => ("=", DIM),
    };
    let mut s = format!("  {} ", paint(marker, sgr, color));
    if let (Some(old), Some(new)) = (&r.old, &r.new) {
        let dots = if r.kind == RefKind::Forced { "..." } else { ".." };
        s += &format!("{}{dots}{}  ", paint(short(old), DIM, color), paint(short(new), BOLD, color));
    }
    if r.kind == RefKind::UpToDate {
        // Whole line dim.
        let refs = if r.src.is_empty() { r.dst.clone() } else { format!("{} -> {}", r.src, r.dst) };
        s += &paint(&refs, DIM, color);
    } else {
        if !r.src.is_empty() && r.src != "(none)" {
            s += &format!("{} -> ", paint(&r.src, BRANCH, color));
        }
        s += &paint(&r.dst, BRANCH, color);
    }
    if let Some(reason) = &r.reason {
        s += &format!("  {}", paint(&format!("({reason})"), sgr, color));
    }
    s.push('\n');
    s
}

/// Target (if any) then each ref.
pub fn render_refs(target: Option<&str>, refs: &[RefUpdate], color: bool) -> String {
    let mut out = target.map(|t| render_target(t, color)).unwrap_or_default();
    for r in refs {
        out += &render_ref(r, color);
    }
    out
}

/// One fetched repository's ref updates. `submodule` names the submodule for a recursive fetch.
#[derive(Debug, Clone, Default)]
pub struct RefGroup {
    pub submodule: Option<String>,
    pub target: Option<String>,
    pub refs: Vec<RefUpdate>,
}

/// Each group with updates under its own header; groups without updates print nothing.
pub fn render_ref_groups(groups: &[RefGroup], color: bool) -> String {
    let mut out = String::new();
    for g in groups.iter().filter(|g| !g.refs.is_empty()) {
        match &g.submodule {
            Some(sub) => {
                let shown = g.target.as_deref().map(|t| format!("  {}", target_text(t, color))).unwrap_or_default();
                out += &format!(
                    "{} {} {}{shown}\n",
                    paint("->", BOLD, color),
                    paint("submodule", DIM, color),
                    paint(&sanitize(sub), BRANCH, color)
                );
                out += &render_refs(None, &g.refs, color);
            }
            None => out += &render_refs(g.target.as_deref(), &g.refs, color),
        }
    }
    out
}

fn bars(plus: usize, minus: usize, color: bool) -> String {
    format!("{}{}", paint(&"+".repeat(plus), GREEN, color), paint(&"-".repeat(minus), RED, color))
}

/// Pull's stdout after the fetch (also clone's), colored line by line. Unknown lines are printed
/// unchanged; `Submodule path` lines are gathered into a "Submodules" section at the end.
pub fn render_pull_summary(lines: &[String], color: bool) -> String {
    let mut out = String::new();
    let mut subs: Vec<(String, String, String)> = Vec::new();
    for line in lines {
        let p = |s: &str, sgr: &str| paint(&sanitize(s), sgr, color);
        let l = match classify_pull_line(line) {
            PullLine::Submodule { path, verb, sha } => {
                subs.push((sanitize(&path), verb, sanitize(&sha)));
                continue;
            }
            PullLine::SubmoduleNoise => continue,
            _ if !color => sanitize(line),
            PullLine::Updating { old, new } => format!("Updating {}..{}", p(&old, DIM), p(&new, BOLD)),
            PullLine::Result(r) => {
                let sgr = match r {
                    PullResult::FastForward | PullResult::Merge | PullResult::Rebased => GREEN,
                    PullResult::UpToDate => DIM,
                    PullResult::Conflict | PullResult::Failed => RED,
                };
                p(line, sgr)
            }
            PullLine::Diffstat { path, count, plus, minus } if plus + minus == 0 => format!("{} | {}", sanitize(&path), p(&count, DIM)),
            PullLine::Diffstat { path, count, plus, minus } => {
                format!("{} | {} {}", sanitize(&path), p(&count, DIM), bars(plus, minus, color))
            }
            PullLine::BinStat { path, rest } => format!("{} | {}", sanitize(&path), p(&format!("Bin {rest}"), DIM)),
            PullLine::Summary { files, ins, del } => {
                let mut parts = vec![sanitize(&files)];
                parts.extend(ins.map(|i| p(&i, GREEN)));
                parts.extend(del.map(|d| p(&d, RED)));
                parts.join(", ")
            }
            PullLine::Mode { verb, mode, rest } => {
                let sgr = match verb.as_str() {
                    "create mode" => GREEN,
                    "delete mode" => RED,
                    _ => YELLOW,
                };
                let mode = if mode.is_empty() { String::new() } else { format!("{} ", p(&mode, DIM)) };
                format!(" {} {mode}{}", p(&verb, sgr), sanitize(&rest))
            }
            PullLine::Action { verb, path } => {
                format!("{} {}", p(&verb, if verb == "Removing" { RED } else { DIM }), sanitize(&path))
            }
            PullLine::Other => sanitize(line),
        };
        out += &l;
        out.push('\n');
    }
    if !subs.is_empty() {
        out += &format!("{}\n", paint("Submodules", BRANCH, color));
        let w = subs.iter().map(|s| s.0.width()).max().unwrap_or(0);
        for (path, verb, sha) in subs {
            out += &format!(
                "  {}{}  {} {}\n",
                paint(&path, BRANCH, color),
                " ".repeat(w - path.width()),
                paint(&verb, GREEN, color),
                paint(short(&sha), YELLOW, color)
            );
        }
    }
    out
}

/// Like [`render_messages`], but each run of `hint:` lines becomes one `hint` box at its place.
pub fn render_messages_boxed(lines: &[String], width: usize, color: bool) -> String {
    let mut out = String::new();
    let mut i = 0;
    while i < lines.len() {
        let n = lines[i..].iter().take_while(|l| l.starts_with("hint:")).count();
        if n > 0 {
            let body: Vec<String> = lines[i..i + n].iter().map(|l| l["hint:".len()..].strip_prefix(' ').unwrap_or(&l["hint:".len()..]).to_string()).collect();
            out += &titled_box("hint", DIM, &body, width, color);
            i += n;
        } else {
            let m = lines[i..].iter().take_while(|l| !l.starts_with("hint:")).count();
            out += &render_messages(&lines[i..i + m], color);
            i += m;
        }
    }
    out
}

/// Operation stopped by a conflict, for the next-step lines.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ConflictOp {
    Merge,
    Rebase,
    Unknown,
}

/// `Merge conflict` box: unmerged paths, then how to continue or abort.
pub fn render_conflict_box(paths: &[String], op: ConflictOp, width: usize, color: bool) -> String {
    let mut lines: Vec<String> = paths.to_vec();
    let steps = match op {
        ConflictOp::Merge => Some(("gw add <file> && gw commit", "gw merge --abort")),
        ConflictOp::Rebase => Some(("gw add <file> && git rebase --continue", "git rebase --abort")),
        ConflictOp::Unknown => None,
    };
    if let Some((go, abort)) = steps {
        lines.push(String::new());
        lines.push(format!("resolve, then: {go}"));
        lines.push(format!("or abort:      {abort}"));
    }
    titled_box("Merge conflict", "1;31", &lines, width, color)
}

/// git's leftover stderr lines: `error:`/`fatal:` prefix red, `warning:` yellow, `hint:` lines dim.
pub fn render_messages(lines: &[String], color: bool) -> String {
    let mut out = String::new();
    for line in lines {
        let l = sanitize(line);
        let prefix = ["error:", "fatal:", "warning:", "hint:"].into_iter().find(|p| l.starts_with(p));
        out += &match prefix {
            _ if !color => l,
            Some("hint:") => paint(&l, DIM, color),
            Some(p) => format!("{}{}", paint(p, if p == "warning:" { YELLOW } else { RED }, color), &l[p.len()..]),
            None => l,
        };
        out.push('\n');
    }
    out
}

#[cfg(test)]
mod box_tests {
    use super::*;

    fn v(l: &[&str]) -> Vec<String> {
        l.iter().map(|s| s.to_string()).collect()
    }

    #[test]
    fn hints_boxed_in_place() {
        let l = v(&[
            "error: Merging is not possible because you have unmerged files.",
            "hint: Fix them up in the work tree, and then use 'git add/rm <file>'",
            "hint: as appropriate to mark resolution and make a commit.",
            "fatal: Exiting because of an unresolved conflict.",
        ]);
        let out = render_messages_boxed(&l, 100, false);
        let rows: Vec<&str> = out.lines().collect();
        assert!(rows[0].starts_with("error: Merging"), "{out}");
        assert!(rows[1].starts_with("┌ hint "), "{out}");
        assert!(rows[2].starts_with("│ Fix them up"), "{out}");
        assert!(rows[4].starts_with("└"), "{out}");
        assert_eq!(rows[5], "fatal: Exiting because of an unresolved conflict.");
        assert!(!out.contains("hint:"), "{out}");
    }

    #[test]
    fn diverging_hints_one_box() {
        let l = v(&[
            "hint: Diverging branches can't be fast-forwarded, you need to either:",
            "hint:",
            "hint: \tgit merge --no-ff",
            "hint:",
            "hint: or:",
            "hint:",
            "hint: \tgit rebase",
            "hint:",
            "hint: Disable this message with \"git config set advice.diverging false\"",
            "fatal: Not possible to fast-forward, aborting.",
        ]);
        let out = render_messages_boxed(&l, 50, true);
        assert_eq!(out.matches("┌").count(), 1, "{out}");
        assert!(crate::style::strip_escapes(&out).contains("│     git merge --no-ff"), "{out}");
        assert!(crate::style::strip_escapes(&out).contains("│ Diverging branches can't be fast-forwarded,    │\n│ you need to either:"), "{out}");
        assert!(out.lines().last().unwrap().contains("\x1b[31mfatal:"), "{out:?}");
        assert!(crate::style::strip_escapes(&out).lines().all(|l| l.width() <= 50), "{out}");
    }

    #[test]
    fn conflict_box_steps() {
        let p = v(&["src/a.rs", "b.txt"]);
        insta::assert_snapshot!("conflict_merge", render_conflict_box(&p, ConflictOp::Merge, 80, false));
        insta::assert_snapshot!("conflict_rebase", render_conflict_box(&p, ConflictOp::Rebase, 80, false));
        insta::assert_snapshot!("conflict_unknown", render_conflict_box(&p, ConflictOp::Unknown, 80, false));
    }
}
