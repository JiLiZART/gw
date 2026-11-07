//! Parsers for `git log --graph`, `for-each-ref`, `stash list`, `shortlog`, and `--name-status -z`.

/// Field separator used in the formats below (`%x1f` / `%1f`).
pub const US: char = '\x1f';

/// Format string for `git log --graph`: graph prefix, then US-separated fields.
pub const LOG_FORMAT: &str = "--format=%x1f%H%x1f%h%x1f%at%x1f%an%x1f%D%x1f%s";

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Commit {
    pub hash: String,
    pub short: String,
    /// Author time, unix seconds.
    pub time: i64,
    pub author: String,
    /// Decorations from `%D`, e.g. `HEAD -> main`, `origin/main`, `tag: v1`.
    pub refs: Vec<String>,
    pub subject: String,
}

/// One output line of `git log --graph`: a commit, or a graph-only connector line.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LogRow {
    pub graph: String,
    pub commit: Option<Commit>,
}

pub fn parse_graph_line(line: &str) -> LogRow {
    let Some((graph, rest)) = line.split_once(US) else {
        return LogRow { graph: line.trim_end().to_string(), commit: None };
    };
    let f: Vec<&str> = rest.split(US).collect();
    let get = |i: usize| f.get(i).copied().unwrap_or("");
    let refs = get(4).split(", ").filter(|r| !r.is_empty()).map(str::to_string).collect();
    LogRow {
        graph: graph.trim_end().to_string(),
        commit: Some(Commit {
            hash: get(0).into(),
            short: get(1).into(),
            time: get(2).parse().unwrap_or(0),
            author: get(3).into(),
            refs,
            // Subject is last; rejoin in case it contained the separator.
            subject: f.get(5..).map(|s| s.join("\u{1f}")).unwrap_or_default(),
        }),
    }
}

/// Format for `git for-each-ref refs/heads` parsed by [`parse_branches`].
pub const BRANCH_FORMAT: &str = "--format=%(HEAD)%1f%(refname:short)%1f%(upstream:short)%1f%(upstream:track,nobracket)%1f%(committerdate:relative)%1f%(subject)";

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Branch {
    pub name: String,
    pub current: bool,
    pub upstream: Option<String>,
    pub ahead: u32,
    pub behind: u32,
    /// Upstream configured but deleted.
    pub gone: bool,
    pub age: String,
    pub subject: String,
}

pub fn parse_branches(text: &str) -> Vec<Branch> {
    text.lines()
        .filter_map(|l| {
            let f: Vec<&str> = l.split(US).collect();
            if f.len() < 6 {
                return None;
            }
            let track = f[3];
            let num = |key: &str| {
                track.split(", ").find_map(|p| p.strip_prefix(key)).and_then(|n| n.trim().parse().ok()).unwrap_or(0)
            };
            Some(Branch {
                current: f[0] == "*",
                name: f[1].into(),
                upstream: (!f[2].is_empty()).then(|| f[2].to_string()),
                ahead: num("ahead "),
                behind: num("behind "),
                gone: track == "gone",
                age: f[4].into(),
                subject: f[5..].join("\u{1f}"),
            })
        })
        .collect()
}

/// Format for `git stash list` parsed by [`parse_stash_list`].
pub const STASH_FORMAT: &str = "--format=%gd%x1f%H%x1f%cr%x1f%gs";

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Stash {
    /// `stash@{N}`
    pub reference: String,
    pub index: usize,
    /// Commit id, to detect a shifted list before acting on `reference`.
    pub oid: String,
    pub age: String,
    pub branch: Option<String>,
    pub message: String,
}

pub fn parse_stash_list(text: &str) -> Vec<Stash> {
    text.lines()
        .filter_map(|l| {
            let mut f = l.splitn(4, US);
            let (reference, oid, age, gs) = (f.next()?, f.next()?, f.next()?, f.next()?);
            let index = reference.strip_prefix("stash@{")?.strip_suffix('}')?.parse().ok()?;
            // "%gs" is "WIP on <branch>: <sha> <subject>" or "On <branch>: <message>".
            let body = gs.strip_prefix("WIP on ").or_else(|| gs.strip_prefix("On "));
            let (branch, message) = match body.and_then(|b| b.split_once(": ")) {
                Some((b, m)) => (Some(b.to_string()), m.to_string()),
                None => (None, gs.to_string()),
            };
            Some(Stash { reference: reference.into(), index, oid: oid.into(), age: age.into(), branch, message })
        })
        .collect()
}

/// `git shortlog -sne`: `(count, "Name <email>")`, most commits first (git's order).
pub fn parse_shortlog(text: &str) -> Vec<(u32, String)> {
    text.lines()
        .filter_map(|l| {
            let (n, who) = l.trim_start().split_once('\t')?;
            Some((n.parse().ok()?, who.to_string()))
        })
        .collect()
}

/// `git diff/show --name-status -z`: `(code, path, orig)`; renames/copies carry the source path.
pub fn parse_name_status_z(bytes: &[u8]) -> Vec<(char, String, Option<String>)> {
    let text = String::from_utf8_lossy(bytes);
    let mut it = text.split('\0').filter(|s| !s.is_empty());
    let mut out = Vec::new();
    while let Some(status) = it.next() {
        let code = status.chars().next().unwrap_or('M');
        if matches!(code, 'R' | 'C') {
            let (Some(from), Some(to)) = (it.next(), it.next()) else { break };
            out.push((code, to.to_string(), Some(from.to_string())));
        } else if let Some(p) = it.next() {
            out.push((code, p.to_string(), None));
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn connector_and_commit() {
        assert_eq!(parse_graph_line("|\\  "), LogRow { graph: "|\\".into(), commit: None });
        let r = parse_graph_line("* \x1fabc\x1fa\x1f5\x1fN\x1fHEAD -> main, tag: v1\x1fs: x");
        let c = r.commit.unwrap();
        assert_eq!((r.graph.as_str(), c.refs.len(), c.subject.as_str()), ("*", 2, "s: x"));
    }

    #[test]
    fn stash_list() {
        let v = parse_stash_list("stash@{0}\x1fabc\x1f2 hours ago\x1fOn main: fix: a: b\nstash@{1}\x1fdef\x1f3 days ago\x1fWIP on feat: 123 s\n");
        assert_eq!((v[0].index, v[0].oid.as_str(), v[0].branch.as_deref(), v[0].message.as_str()), (0, "abc", Some("main"), "fix: a: b"));
        assert_eq!((v[1].reference.as_str(), v[1].age.as_str()), ("stash@{1}", "3 days ago"));
    }

    #[test]
    fn name_status() {
        let v = parse_name_status_z(b"M\0a.rs\0R087\0old\0new\0D\0gone\0");
        assert_eq!(v[1], ('R', "new".into(), Some("old".into())));
        assert_eq!(v.len(), 3);
    }
}
