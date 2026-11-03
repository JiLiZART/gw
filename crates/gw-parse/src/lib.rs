//! Pure parsers for git's output. No IO, no terminal.

mod refs;
mod status;

pub use refs::{RefKind, RefUpdate, parse_fetch_porcelain, parse_human_ref_line, parse_push_porcelain};
pub use status::{Entry, Numstat, Status, parse_numstat_z, parse_status_v2};

/// One classified line of git output.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Event {
    Progress(Progress),
    /// A `remote:` line that is not progress, prefix (and one space) stripped.
    Remote(String),
    /// `To <url>` / `From <url>`.
    Target(String),
    RefUpdate(RefUpdate),
    UpToDate,
    Other(String),
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Progress {
    pub remote: bool,
    pub label: String,
    pub cur: Option<u64>,
    pub total: Option<u64>,
    pub pct: Option<u8>,
    /// Text after the counters, e.g. `, 840 bytes | 840.00 KiB/s, done.`
    pub tail: String,
}

/// Streaming splitter: yields a line on `\r` or `\n` (a `\r\n` pair yields one line),
/// even when a line spans several chunks. Bytes are decoded as lossy UTF-8.
#[derive(Default)]
pub struct Splitter {
    buf: Vec<u8>,
    last_cr: bool,
}

impl Splitter {
    pub fn push(&mut self, chunk: &[u8]) -> Vec<String> {
        let mut out = Vec::new();
        for &b in chunk {
            match b {
                b'\n' if self.last_cr => self.last_cr = false,
                b'\r' | b'\n' => {
                    out.push(String::from_utf8_lossy(&self.buf).into_owned());
                    self.buf.clear();
                    self.last_cr = b == b'\r';
                }
                _ => {
                    self.buf.push(b);
                    self.last_cr = false;
                }
            }
        }
        out
    }

    /// Remaining bytes without a line terminator, if any.
    pub fn finish(self) -> Option<String> {
        (!self.buf.is_empty()).then(|| String::from_utf8_lossy(&self.buf).into_owned())
    }
}

/// Classify one line from git's stderr.
pub fn classify(line: &str) -> Event {
    if is_pack_stats(line) {
        return Event::Other(line.to_string());
    }
    if line.starts_with("Delta compression using up to ") {
        return Event::Progress(Progress { label: "Delta compression".into(), ..Default::default() });
    }
    if let Some(body) = line.strip_prefix("remote:") {
        let body = body.strip_prefix(' ').unwrap_or(body).trim_end();
        return match parse_progress(body) {
            Some(mut p) => {
                p.remote = true;
                Event::Progress(p)
            }
            None => Event::Remote(body.to_string()),
        };
    }
    let t = line.trim_end();
    if let Some(p) = parse_progress(t) {
        return Event::Progress(p);
    }
    if let Some(url) = t.strip_prefix("To ").or_else(|| t.strip_prefix("From ")) {
        return Event::Target(url.to_string());
    }
    if t == "Everything up-to-date" {
        return Event::UpToDate;
    }
    if let Some(r) = parse_human_ref_line(t) {
        return Event::RefUpdate(r);
    }
    Event::Other(line.to_string())
}

/// Pack statistics line (`Total 11 (delta 10), reused ...`), local or `remote:`.
/// Not progress and not a server message: callers drop it.
pub fn is_pack_stats(line: &str) -> bool {
    let body = line.strip_prefix("remote:").map_or(line, str::trim_start);
    body.starts_with("Total ") && body.contains("(delta ")
}

/// `<Label>: <pct>% (<cur>/<total>)<tail>`, `<Label>: <n>, done.` or `<Label>: <n>`.
fn parse_progress(s: &str) -> Option<Progress> {
    let (label, rest) = s.split_once(": ")?;
    if label.is_empty() || !label.starts_with(|c: char| c.is_ascii_uppercase()) || label.contains(':') {
        return None;
    }
    let rest = rest.trim_start();
    let label = label.to_string();
    if let Some((pct, after)) = rest.split_once("% (") {
        let pct: u8 = pct.parse().ok()?;
        let (counts, tail) = after.split_once(')')?;
        let (cur, total) = counts.split_once('/')?;
        return Some(Progress {
            label,
            cur: Some(cur.parse().ok()?),
            total: Some(total.parse().ok()?),
            pct: Some(pct),
            tail: tail.to_string(),
            ..Default::default()
        });
    }
    // Stages without percentage: "Enumerating objects: 8, done." / "Delta compression ...: 3"
    let digits = rest.find(|c: char| !c.is_ascii_digit()).unwrap_or(rest.len());
    if digits == 0 {
        return None;
    }
    let tail = &rest[digits..];
    if !(tail.is_empty() || tail.ends_with(", done.")) {
        return None;
    }
    Some(Progress { label, cur: rest[..digits].parse().ok(), tail: tail.to_string(), ..Default::default() })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn split_across_chunks() {
        let mut s = Splitter::default();
        assert_eq!(s.push(b"Writing obj"), Vec::<String>::new());
        assert_eq!(s.push(b"ects: 1\rWri"), vec!["Writing objects: 1"]);
        assert_eq!(s.push(b"ting\n"), vec!["Writing"]);
        assert_eq!(s.finish(), None);
    }

    #[test]
    fn crlf_is_one_line() {
        let mut s = Splitter::default();
        assert_eq!(s.push(b"a\r"), vec!["a"]);
        assert_eq!(s.push(b"\nb\r\n\nc"), vec!["b", ""]);
        assert_eq!(s.finish().as_deref(), Some("c"));
    }

    #[test]
    fn lossy_utf8() {
        let mut s = Splitter::default();
        assert_eq!(s.push(b"\xffok\n"), vec!["\u{fffd}ok"]);
    }

    #[test]
    fn progress_kinds() {
        let Event::Progress(p) = classify("remote: Resolving deltas:  50% (3/6)        ") else { panic!() };
        assert!(p.remote);
        assert_eq!((p.label.as_str(), p.cur, p.total, p.pct), ("Resolving deltas", Some(3), Some(6), Some(50)));
        assert!(matches!(classify("Enumerating objects: 8, done."), Event::Progress(_)));
        assert!(matches!(classify("remote: Processing changes: refs: 1, done"), Event::Remote(_)));
        assert!(matches!(classify("error: failed to push some refs to 'x'"), Event::Other(_)));
        assert!(is_pack_stats("remote: Total 6 (delta 3), reused 0 (delta 0)"));
    }
}
