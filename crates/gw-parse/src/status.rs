/// Parsed `git status --porcelain=v2 --branch -z`.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Status {
    pub oid: Option<String>,
    /// `None` when HEAD is detached.
    pub head: Option<String>,
    pub upstream: Option<String>,
    pub ahead_behind: Option<(u32, u32)>,
    pub entries: Vec<Entry>,
}

/// `x`/`y` are git's XY codes (`.` = unchanged). Untracked is `?`/`?`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Entry {
    pub x: char,
    pub y: char,
    pub unmerged: bool,
    /// Repo-root relative.
    pub path: String,
    /// Source path of a rename/copy.
    pub orig: Option<String>,
    /// Set for a submodule (porcelain v2 `S<c><m><u>`), except in conflicts.
    pub submodule: Option<SubmoduleState>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SubmoduleState {
    /// Checked-out commit differs from the index (`C`).
    pub commit_changed: bool,
    /// Tracked files inside are modified (`M`).
    pub modified: bool,
    /// Untracked files inside (`U`).
    pub untracked: bool,
    /// Commit recorded in HEAD (`hH`) and in the index (`hI`).
    pub head: String,
    pub index: String,
}

/// Per-file line counts keyed by repo-relative path; `None` means binary.
pub type Numstat = std::collections::HashMap<String, Option<(u64, u64)>>;

/// `git diff [--cached] --numstat -z`: `added\tdeleted\tpath\0`, renames as `added\tdeleted\t\0src\0dst\0`.
pub fn parse_numstat_z(bytes: &[u8]) -> Numstat {
    let text = String::from_utf8_lossy(bytes);
    let mut recs = text.split('\0');
    let mut out = Numstat::new();
    while let Some(rec) = recs.next() {
        let mut f = rec.splitn(3, '\t');
        let (Some(a), Some(d), Some(p)) = (f.next(), f.next(), f.next()) else { continue };
        let path = if p.is_empty() {
            recs.next();
            recs.next().unwrap_or("")
        } else {
            p
        };
        out.insert(path.to_string(), a.parse().ok().zip(d.parse().ok()));
    }
    out
}

pub fn parse_status_v2(bytes: &[u8]) -> Status {
    let text = String::from_utf8_lossy(bytes);
    let mut recs = text.split('\0');
    let mut st = Status::default();
    while let Some(rec) = recs.next() {
        if let Some(h) = rec.strip_prefix("# ") {
            let (k, v) = h.split_once(' ').unwrap_or((h, ""));
            match k {
                "branch.oid" if v != "(initial)" => st.oid = Some(v.to_string()),
                "branch.head" if v != "(detached)" => st.head = Some(v.to_string()),
                "branch.upstream" => st.upstream = Some(v.to_string()),
                "branch.ab" => {
                    let mut it = v.split(' ').map(|n| n.trim_start_matches(['+', '-']).parse().unwrap_or(0));
                    st.ahead_behind = Some((it.next().unwrap_or(0), it.next().unwrap_or(0)));
                }
                _ => {}
            }
            continue;
        }
        let fields: Vec<&str> = rec.split(' ').collect();
        let xy = |s: &str| {
            let mut c = s.chars();
            (c.next().unwrap_or('.'), c.next().unwrap_or('.'))
        };
        // Path is the last field and may contain spaces, so take it with splitn.
        let path_after = |n: usize| rec.splitn(n + 1, ' ').nth(n).unwrap_or("").to_string();
        // `S<c><m><u>` in field 2, hH/hI in fields 6/7 (same layout for `1` and `2` records).
        let sub = || {
            let f = fields.get(2)?.strip_prefix('S')?.as_bytes();
            Some(SubmoduleState {
                commit_changed: f.first() == Some(&b'C'),
                modified: f.get(1) == Some(&b'M'),
                untracked: f.get(2) == Some(&b'U'),
                head: fields.get(6)?.to_string(),
                index: fields.get(7)?.to_string(),
            })
        };
        let entry = match fields.first() {
            Some(&"1") => {
                let (x, y) = xy(fields[1]);
                Entry { x, y, unmerged: false, path: path_after(8), orig: None, submodule: sub() }
            }
            Some(&"2") => {
                let (x, y) = xy(fields[1]);
                let orig = recs.next().map(str::to_string);
                Entry { x, y, unmerged: false, path: path_after(9), orig, submodule: sub() }
            }
            Some(&"u") => {
                let (x, y) = xy(fields[1]);
                Entry { x, y, unmerged: true, path: path_after(10), orig: None, submodule: None }
            }
            Some(&"?") => Entry { x: '?', y: '?', unmerged: false, path: path_after(1), orig: None, submodule: None },
            _ => continue,
        };
        st.entries.push(entry);
    }
    st
}
