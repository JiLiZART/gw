//! Fixed colors, OSC 8 links, and escape-aware width measuring for static output.

use unicode_width::UnicodeWidthChar;

pub const GREEN: &str = "32";
pub const RED: &str = "31";
pub const YELLOW: &str = "33";
pub const DIM: &str = "2";
pub const BOLD: &str = "1";
pub const BRANCH: &str = "36";
pub const BRANCH_BOLD: &str = "1;36";
pub const LINK: &str = "4;34";

/// True when `NO_COLOR` is set to a non-empty value.
pub fn no_color_env() -> bool {
    std::env::var_os("NO_COLOR").is_some_and(|v| !v.is_empty())
}

/// Wraps `s` in an SGR sequence, or returns it unchanged when `color` is false.
pub fn paint(s: &str, sgr: &str, color: bool) -> String {
    if color && !s.is_empty() { format!("\x1b[{sgr}m{s}\x1b[0m") } else { s.to_string() }
}

/// OSC 8 hyperlink. Emitted even without color: links are not styling.
pub fn link(url: &str, text: &str) -> String {
    format!("\x1b]8;;{url}\x1b\\{text}\x1b]8;;\x1b\\")
}

/// Removes CSI (`ESC [ ... final`) and OSC (`ESC ] ... ST|BEL`) sequences.
pub fn strip_escapes(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let mut it = s.chars().peekable();
    while let Some(c) = it.next() {
        if c != '\x1b' {
            out.push(c);
            continue;
        }
        match it.next() {
            Some('[') => {
                for c in it.by_ref() {
                    if ('\x40'..='\x7e').contains(&c) {
                        break;
                    }
                }
            }
            Some(']') => {
                while let Some(c) = it.next() {
                    if c == '\x07' || (c == '\x1b' && it.next_if_eq(&'\\').is_some()) {
                        break;
                    }
                }
            }
            _ => {}
        }
    }
    out
}

/// Display width of the visible text, ignoring escapes.
pub fn visible_width(s: &str) -> usize {
    unicode_width::UnicodeWidthStr::width(strip_escapes(s).as_str())
}

/// Cuts plain text to at most `max` columns, ending with `…` when cut.
pub fn truncate(s: &str, max: usize) -> String {
    if unicode_width::UnicodeWidthStr::width(s) <= max {
        return s.to_string();
    }
    let mut out = String::new();
    let mut w = 0;
    for c in s.chars() {
        let cw = c.width().unwrap_or(0);
        if w + cw + 1 > max {
            break;
        }
        out.push(c);
        w += cw;
    }
    if max > 0 {
        out.push('…');
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn no_color_has_no_sgr_but_keeps_links() {
        let s = format!("{} {}", paint("x", GREEN, false), link("https://a.b", "a.b"));
        assert!(!s.contains("\x1b["));
        assert!(s.contains("\x1b]8;;https://a.b\x1b\\"));
    }

    #[test]
    fn width_ignores_escapes() {
        let s = format!("{}─{}", paint("ab", BOLD, true), link("https://x.y/zz", "x…"));
        assert_eq!(visible_width(&s), 5);
        assert_eq!(truncate("abcdef", 4), "abc…");
        assert_eq!(truncate("abc", 4), "abc");
    }
}
