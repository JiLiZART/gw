//! Target header, ref update list, and pull summary.

use crate::style::{BOLD, BRANCH, DIM, GREEN, LINK, RED, YELLOW, link, paint, sanitize};
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
