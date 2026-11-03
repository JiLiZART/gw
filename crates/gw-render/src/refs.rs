//! Target header, ref update list, and pull summary.

use crate::style::{BOLD, BRANCH, DIM, GREEN, LINK, RED, YELLOW, link, paint};
use gw_parse::{RefKind, RefUpdate};

fn short(sha: &str) -> &str {
    &sha[..sha.len().min(7)]
}

/// `-> <target>` line; URL forms become links.
pub fn render_target(target: &str, color: bool) -> String {
    let shown = if target.contains("://") { link(target, &paint(target, LINK, color)) } else { target.to_string() };
    format!("{} {shown}\n", paint("->", BOLD, color))
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

/// Pull's post-fetch output: diffstat `+` green, `-` red; other lines unchanged.
pub fn render_pull_summary(lines: &[String], color: bool) -> String {
    let mut out = String::new();
    for line in lines {
        match line.rsplit_once(" | ") {
            Some((path, stat)) if line.starts_with(' ') => {
                let bars = stat.trim_start_matches(|c: char| c.is_ascii_digit() || c == ' ');
                let head = &stat[..stat.len() - bars.len()];
                let plus = bars.chars().filter(|&c| c == '+').count();
                let minus = bars.chars().filter(|&c| c == '-').count();
                if plus + minus == bars.chars().count() {
                    out += &format!("{path} | {head}{}{}\n", paint(&"+".repeat(plus), GREEN, color), paint(&"-".repeat(minus), RED, color));
                    continue;
                }
                out += &format!("{line}\n");
            }
            _ => out += &format!("{line}\n"),
        }
    }
    out
}
