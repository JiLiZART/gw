use crate::Event;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RefKind {
    FastForward,
    Forced,
    New,
    Deleted,
    Rejected,
    UpToDate,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RefUpdate {
    pub kind: RefKind,
    pub old: Option<String>,
    pub new: Option<String>,
    /// Empty when git does not report a source (fetch porcelain).
    pub src: String,
    pub dst: String,
    pub reason: Option<String>,
}

fn short_ref(r: &str) -> String {
    ["refs/heads/", "refs/tags/", "refs/remotes/"]
        .iter()
        .find_map(|p| r.strip_prefix(p))
        .unwrap_or(r)
        .to_string()
}

fn kind_from_flag(flag: char) -> Option<RefKind> {
    Some(match flag {
        ' ' => RefKind::FastForward,
        '+' | 't' => RefKind::Forced,
        '*' => RefKind::New,
        '-' => RefKind::Deleted,
        '!' => RefKind::Rejected,
        '=' => RefKind::UpToDate,
        _ => return None,
    })
}

/// Splits `a..b` / `a...b` into SHAs.
fn shas(summary: &str) -> (Option<String>, Option<String>) {
    match summary.split_once("...").or_else(|| summary.split_once("..")) {
        Some((a, b)) => (Some(a.to_string()), Some(b.to_string())),
        None => (None, None),
    }
}

/// `git push --porcelain` stdout: `To <url>`, `<flag>\t<src>:<dst>\t<summary>`, `Done`.
pub fn parse_push_porcelain(text: &str) -> Vec<Event> {
    let mut out = Vec::new();
    for line in text.lines() {
        if let Some(url) = line.strip_prefix("To ") {
            out.push(Event::Target(url.to_string()));
            continue;
        }
        let mut f = line.splitn(3, '\t');
        let (Some(flag), Some(refs), Some(summary)) = (f.next(), f.next(), f.next()) else { continue };
        let Some(kind) = flag.chars().next().and_then(kind_from_flag) else { continue };
        let (src, dst) = refs.split_once(':').unwrap_or(("", refs));
        // summary: "a..b", "+a...b (forced update)", "[new branch]", "[rejected] (fetch first)"
        let (head, reason) = match summary.split_once(" (") {
            Some((h, r)) => (h, Some(r.trim_end_matches(')').to_string())),
            None => (summary, None),
        };
        let (old, new) = shas(head.trim_start_matches('+'));
        let reason = reason.filter(|r| kind == RefKind::Rejected || r != "forced update");
        out.push(Event::RefUpdate(RefUpdate { kind, old, new, src: short_ref(src), dst: short_ref(dst), reason }));
    }
    out
}

/// `git fetch --porcelain` stdout: `<flag> <old> <new> <ref>`.
pub fn parse_fetch_porcelain(text: &str) -> Vec<RefUpdate> {
    text.lines()
        .filter_map(|line| {
            let mut chars = line.chars();
            let kind = kind_from_flag(chars.next()?)?;
            let mut f = chars.as_str().split_whitespace();
            let (old, new, dst) = (f.next()?, f.next()?, f.next()?);
            let zero = |s: &str| s.bytes().all(|b| b == b'0');
            Some(RefUpdate {
                kind,
                old: (!zero(old)).then(|| old.to_string()),
                new: (!zero(new)).then(|| new.to_string()),
                src: String::new(),
                dst: short_ref(dst),
                reason: None,
            })
        })
        .collect()
}

/// Human ref line from pull/clone/fetch stderr (C locale):
/// `   old..new  src -> dst`, ` * [new branch]  src -> dst`, ` + a...b  src -> dst  (forced update)`.
pub fn parse_human_ref_line(line: &str) -> Option<RefUpdate> {
    let rest = line.strip_prefix(' ')?;
    let mut chars = rest.chars();
    let flag = chars.next()?;
    let mut kind = kind_from_flag(flag)?;
    let rest = chars.as_str().strip_prefix(' ')?.trim_start();
    let (summary, rest) = if rest.starts_with('[') {
        let end = rest.find(']')?;
        (&rest[..=end], &rest[end + 1..])
    } else {
        rest.split_once(' ').unwrap_or((rest, ""))
    };
    let (refs, reason) = match rest.rfind(" (") {
        Some(i) if rest.trim_end().ends_with(')') => (&rest[..i], Some(rest[i + 2..].trim_end().trim_end_matches(')'))),
        _ => (rest, None),
    };
    let (src, dst) = refs.split_once(" -> ")?;
    let (old, new) = shas(summary);
    // " * branch  main -> FETCH_HEAD" only records FETCH_HEAD: nothing changed.
    if summary == "branch" || summary == "tag" {
        kind = RefKind::UpToDate;
    }
    let reason = reason.filter(|r| *r != "forced update").map(str::to_string);
    Some(RefUpdate { kind, old, new, src: src.trim().to_string(), dst: dst.trim().to_string(), reason })
}
