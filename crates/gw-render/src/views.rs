//! Static views: `gw branch`, `gw commit` summary, `gw stash list`.

use gw_parse::{Branch, Stash};
use unicode_width::UnicodeWidthStr;

use crate::style::{BRANCH, BRANCH_BOLD, DIM, GREEN, RED, UPSTREAM, YELLOW, paint, sanitize, truncate};

fn pad(s: &str, w: usize) -> String {
    format!("{s}{}", " ".repeat(w.saturating_sub(s.width())))
}

fn counts(a: u32, b: u32, color: bool) -> String {
    format!(
        "{} {}",
        paint(&format!("↑{a}"), if a > 0 { GREEN } else { DIM }, color),
        paint(&format!("↓{b}"), if b > 0 { YELLOW } else { DIM }, color)
    )
}

/// `+N -M` with zero counts dim.
pub fn lines_stat(add: u64, del: u64, color: bool) -> String {
    format!(
        "{} {}",
        paint(&format!("+{add}"), if add > 0 { GREEN } else { DIM }, color),
        paint(&format!("-{del}"), if del > 0 { RED } else { DIM }, color)
    )
}

fn files_word(n: usize) -> String {
    format!("{n} {}", if n == 1 { "file" } else { "files" })
}

/// Current branch first, then by recency (input is already sorted by committer date).
pub fn render_branches(branches: &[Branch], width: usize, color: bool) -> String {
    let mut order: Vec<&Branch> = branches.iter().filter(|b| b.current).collect();
    order.extend(branches.iter().filter(|b| !b.current));
    let name_w = order.iter().map(|b| sanitize(&b.name).width()).max().unwrap_or(0);
    let track = |b: &Branch| match &b.upstream {
        Some(u) => format!("-> {}", sanitize(u)),
        None => "no upstream".into(),
    };
    let track_w = order.iter().map(|b| track(b).width()).max().unwrap_or(0);
    let age_w = order.iter().map(|b| b.age.width()).max().unwrap_or(0);
    let mut out = String::new();
    for b in order {
        let name = sanitize(&b.name);
        let mark = if b.current { paint("*", GREEN, color) } else { " ".into() };
        let name_s = paint(&pad(&name, name_w), if b.current { BRANCH_BOLD } else { BRANCH }, color);
        let t = track(b);
        let track_s = match &b.upstream {
            Some(u) => format!("-> {}{}", paint(&sanitize(u), UPSTREAM, color), " ".repeat(track_w - t.width())),
            None => paint(&pad(&t, track_w), DIM, color),
        };
        let state = if b.gone {
            format!("{}   ", paint("gone", RED, color))
        } else if b.upstream.is_some() {
            counts(b.ahead, b.behind, color)
        } else {
            " ".repeat(7)
        };
        // Visible width so far: mark + space + name + 2 + track + 2 + state(7-ish) + 2 + age + 2
        let used = 2 + name_w + 2 + track_w + 2 + crate::style::visible_width(&state) + 2 + age_w + 2;
        let subject = truncate(&sanitize(&b.subject), width.saturating_sub(used).max(10));
        out += &format!(
            "{mark} {name_s}  {track_s}  {state}  {}  {subject}\n",
            paint(&pad(&b.age, age_w), DIM, color)
        );
    }
    out
}

/// After `git commit`: `✓ <sha> <branch>  <subject>` then `<K> files  +N -M`.
pub fn render_commit_summary(short: &str, branch: &str, subject: &str, files: usize, add: u64, del: u64, color: bool) -> String {
    format!(
        "{} {} {}  {}\n{}  {}\n",
        paint("✓", GREEN, color),
        paint(&sanitize(short), YELLOW, color),
        paint(&sanitize(branch), BRANCH_BOLD, color),
        sanitize(subject),
        files_word(files),
        lines_stat(add, del, color)
    )
}

/// One line per stash: index, age, branch, message, `<K> files +N -M`.
pub fn render_stash_list(stashes: &[(Stash, usize, u64, u64)], color: bool) -> String {
    if stashes.is_empty() {
        return format!("{}\n", paint("no stashes", DIM, color));
    }
    let age_w = stashes.iter().map(|s| s.0.age.width()).max().unwrap_or(0);
    let br_w = stashes.iter().map(|s| s.0.branch.as_deref().map_or(0, |b| sanitize(b).width())).max().unwrap_or(0);
    let mut out = String::new();
    for (s, files, add, del) in stashes {
        let br = s.branch.as_deref().map(sanitize).unwrap_or_default();
        out += &format!(
            "{}  {}  {}  {}  {}\n",
            paint(&s.index.to_string(), YELLOW, color),
            paint(&pad(&s.age, age_w), DIM, color),
            paint(&pad(&br, br_w), BRANCH, color),
            sanitize(&s.message),
            paint(&format!("{} {}", files_word(*files), lines_stat(*add, *del, color)), DIM, false)
        );
    }
    out
}
