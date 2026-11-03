//! Box around `remote:` messages, with a host-aware title, links, and branch highlighting.

use crate::style::{BRANCH, DIM, LINK, link, paint, sanitize, truncate};
use unicode_width::UnicodeWidthStr;

enum Seg<'a> {
    Plain(&'a str),
    Url(&'a str),
    Branch(&'a str),
}

pub fn title(lines: &[String]) -> &'static str {
    let all = lines.join("\n").to_lowercase();
    if all.contains("merge request") {
        "Merge request"
    } else if all.contains("pull request") {
        "Pull request"
    } else {
        "remote"
    }
}

fn segments<'a>(line: &'a str, branches: &[String]) -> Vec<Seg<'a>> {
    let mut out = Vec::new();
    let mut rest = line;
    while !rest.is_empty() {
        let ws = rest.find(|c: char| !c.is_whitespace()).unwrap_or(rest.len());
        if ws > 0 {
            out.push(Seg::Plain(&rest[..ws]));
            rest = &rest[ws..];
            continue;
        }
        let end = rest.find(char::is_whitespace).unwrap_or(rest.len());
        let tok = &rest[..end];
        rest = &rest[end..];
        if tok.starts_with("http://") || tok.starts_with("https://") {
            let url = tok.trim_end_matches(['.', ',', ')']);
            out.push(Seg::Url(url));
            out.push(Seg::Plain(&tok[url.len()..]));
            continue;
        }
        let core = tok.trim_matches(|c: char| matches!(c, ':' | ',' | '.' | '\'' | '"' | '(' | ')' | '`'));
        match tok.find(core) {
            Some(i) if !core.is_empty() && branches.iter().any(|b| b == core) => {
                out.push(Seg::Plain(&tok[..i]));
                out.push(Seg::Branch(core));
                out.push(Seg::Plain(&tok[i + core.len()..]));
            }
            _ => out.push(Seg::Plain(tok)),
        }
    }
    out
}

/// Renders the box, or an empty string when there is nothing to show.
/// `term_width` caps the box width; `branches` are highlighted when they appear as words.
pub fn render_box(lines: &[String], branches: &[String], term_width: usize, color: bool) -> String {
    let clean: Vec<String> = lines.iter().map(|l| sanitize(l)).collect();
    let lines: Vec<&str> = clean.iter().map(|l| l.trim_end()).collect();
    let Some(first) = lines.iter().position(|l| !l.is_empty()) else { return String::new() };
    let last = lines.iter().rposition(|l| !l.is_empty()).unwrap();
    let lines = &lines[first..=last];
    let owned: Vec<String> = lines.iter().map(|s| s.to_string()).collect();
    let title = title(&owned);

    let longest = lines.iter().map(|l| l.width()).max().unwrap_or(0);
    let total = (longest + 4).max(title.width() + 6).min(term_width.max(title.width() + 6));
    let inner = total - 4;

    let mut out = String::new();
    let top_fill = total - 2 - (title.width() + 2);
    out += &paint("┌", DIM, color);
    out += &format!(" {} ", paint(title, "1", color));
    out += &paint(&format!("{}┐", "─".repeat(top_fill)), DIM, color);
    out.push('\n');

    for line in lines {
        let mut room = inner;
        let mut body = String::new();
        for seg in segments(line, branches) {
            if room == 0 {
                break;
            }
            let text = match seg {
                Seg::Plain(s) | Seg::Url(s) | Seg::Branch(s) => s,
            };
            let shown = truncate(text, room);
            room -= shown.width();
            body += &match seg {
                Seg::Plain(_) => shown.clone(),
                Seg::Url(url) => link(url, &paint(&shown, LINK, color)),
                Seg::Branch(_) => paint(&shown, BRANCH, color),
            };
            if shown != text {
                break;
            }
        }
        out += &paint("│", DIM, color);
        out += &format!(" {body}{} ", " ".repeat(room));
        out += &paint("│", DIM, color);
        out.push('\n');
    }
    out += &paint(&format!("└{}┘", "─".repeat(total - 2)), DIM, color);
    out.push('\n');
    out
}
