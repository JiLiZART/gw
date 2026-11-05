//! Lines git prints on stdout after a pull's fetch (merge/rebase summary, diffstat),
//! and submodule lines from pull/clone.

use std::collections::HashMap;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PullResult {
    FastForward,
    Merge,
    Rebased,
    UpToDate,
    Conflict,
    Failed,
}

/// One classified stdout line. Each variant holds enough to rebuild the line exactly.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PullLine {
    /// `Updating <old>..<new>`
    Updating { old: String, new: String },
    Result(PullResult),
    /// `<path> | <count> <+++-->`; `path` keeps git's leading space and padding, `count` its padding.
    Diffstat { path: String, count: String, plus: usize, minus: usize },
    /// `<path> | Bin <a> -> <b> bytes`
    BinStat { path: String, rest: String },
    /// ` N files changed`, optional `X insertions(+)`, optional `Y deletions(-)`.
    Summary { files: String, ins: Option<String>, del: Option<String> },
    /// ` create mode 100644 x`: verb (`create mode`, `delete mode`, `rename`, `mode change`), dim part, rest.
    Mode { verb: String, mode: String, rest: String },
    /// `Submodule path '<path>': <verb> '<sha>'`
    Submodule { path: String, verb: String, sha: String },
    /// Merge's per-path work: `Auto-merging <path>`, `Removing <path>`.
    Action { verb: String, path: String },
    /// Clone's `Submodule '<name>' (<url>) registered for path '<path>'`.
    SubmoduleNoise,
    Other,
}

pub fn classify_pull_line(line: &str) -> PullLine {
    if let Some((old, new)) = line.strip_prefix("Updating ").and_then(|r| r.split_once(".."))
        && !old.is_empty()
        && !new.is_empty()
        && !new.contains(' ')
    {
        return PullLine::Updating { old: old.into(), new: new.into() };
    }
    let result = match line {
        "Fast-forward" => Some(PullResult::FastForward),
        "Already up to date." | "Already up-to-date." => Some(PullResult::UpToDate),
        l if l.starts_with("Merge made by ") => Some(PullResult::Merge),
        l if l.starts_with("Successfully rebased and updated ") => Some(PullResult::Rebased),
        l if l.starts_with("CONFLICT ") => Some(PullResult::Conflict),
        l if l.starts_with("Automatic merge failed") || l.starts_with("Failed to merge submodule ") => Some(PullResult::Failed),
        _ => None,
    };
    if let Some(r) = result {
        return PullLine::Result(r);
    }
    if let Some(rest) = line.strip_prefix("Submodule path '")
        && let Some((path, rest)) = rest.split_once("': ")
        && let Some((verb, sha)) = rest.rsplit_once(" '")
        && let Some(sha) = sha.strip_suffix('\'')
    {
        return PullLine::Submodule { path: path.into(), verb: verb.into(), sha: sha.into() };
    }
    for verb in ["Auto-merging", "Removing"] {
        if let Some(path) = line.strip_prefix(verb).and_then(|r| r.strip_prefix(' ')) {
            return PullLine::Action { verb: verb.into(), path: path.into() };
        }
    }
    if line.starts_with("Submodule '") && line.contains(") registered for path '") {
        return PullLine::SubmoduleNoise;
    }
    if !line.starts_with(' ') {
        return PullLine::Other;
    }
    if let Some((path, stat)) = line.rsplit_once(" | ") {
        if let Some(rest) = stat.strip_prefix("Bin ") {
            return PullLine::BinStat { path: path.into(), rest: rest.into() };
        }
        let (count, bars) = stat.rsplit_once(' ').filter(|(c, _)| !c.trim().is_empty()).unwrap_or((stat, ""));
        let plus = bars.chars().take_while(|&c| c == '+').count();
        let minus = bars.len() - plus;
        if !count.trim().is_empty()
            && count.trim().chars().all(|c| c.is_ascii_digit())
            && bars.chars().skip(plus).all(|c| c == '-')
        {
            return PullLine::Diffstat { path: path.into(), count: count.into(), plus, minus };
        }
    }
    let mut parts = line.split(", ");
    if let Some(files) = parts.next().filter(|f| f.ends_with(" changed") && f.contains(" file")) {
        let (mut ins, mut del) = (None, None);
        for p in parts {
            if p.ends_with("(+)") && ins.is_none() && del.is_none() {
                ins = Some(p.to_string());
            } else if p.ends_with("(-)") && del.is_none() {
                del = Some(p.to_string());
            } else {
                return PullLine::Other;
            }
        }
        return PullLine::Summary { files: files.into(), ins, del };
    }
    for verb in ["create mode", "delete mode"] {
        if let Some((mode, rest)) = line.strip_prefix(&format!(" {verb} ")).and_then(|r| r.split_once(' ')) {
            return PullLine::Mode { verb: verb.into(), mode: mode.into(), rest: rest.into() };
        }
    }
    if let Some(r) = line.strip_prefix(" mode change ") {
        // `100644 => 100755 path`
        let mut it = r.splitn(4, ' ');
        if let (Some(a), Some("=>"), Some(b), Some(path)) = (it.next(), it.next(), it.next(), it.next()) {
            return PullLine::Mode { verb: "mode change".into(), mode: format!("{a} => {b}"), rest: path.into() };
        }
    }
    if let Some(r) = line.strip_prefix(" rename ") {
        return PullLine::Mode { verb: "rename".into(), mode: String::new(), rest: r.into() };
    }
    PullLine::Other
}

/// `git submodule status`: `[ +-U]<sha> <path>[ (<describe>)]` -> path to checked-out sha.
/// Uninitialized (`-`) submodules have no checkout and are left out.
pub fn parse_submodule_status(text: &str) -> HashMap<String, String> {
    text.lines()
        .filter(|l| !l.starts_with('-'))
        .filter_map(|l| {
            let (sha, rest) = l.get(1..)?.split_once(' ')?;
            let path = match rest.rsplit_once(" (") {
                Some((p, d)) if d.ends_with(')') => p,
                _ => rest,
            };
            Some((path.to_string(), sha.to_string()))
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lines() {
        assert_eq!(classify_pull_line("Updating 8910aeb..e698aeb"), PullLine::Updating { old: "8910aeb".into(), new: "e698aeb".into() });
        assert_eq!(classify_pull_line(" a.rs |  4 ++--"), PullLine::Diffstat { path: " a.rs".into(), count: " 4".into(), plus: 2, minus: 2 });
        assert_eq!(classify_pull_line(" a.rs | 0"), PullLine::Diffstat { path: " a.rs".into(), count: "0".into(), plus: 0, minus: 0 });
        assert!(matches!(classify_pull_line(" img.png | Bin 0 -> 12 bytes"), PullLine::BinStat { .. }));
        assert_eq!(
            classify_pull_line(" 1 file changed, 1 deletion(-)"),
            PullLine::Summary { files: " 1 file changed".into(), ins: None, del: Some("1 deletion(-)".into()) }
        );
        assert_eq!(
            classify_pull_line(" mode change 100644 => 100755 run.sh"),
            PullLine::Mode { verb: "mode change".into(), mode: "100644 => 100755".into(), rest: "run.sh".into() }
        );
        assert_eq!(classify_pull_line("CONFLICT (content): Merge conflict in a"), PullLine::Result(PullResult::Conflict));
        assert_eq!(
            classify_pull_line("Submodule path 'a b': merged in '1234567890'"),
            PullLine::Submodule { path: "a b".into(), verb: "merged in".into(), sha: "1234567890".into() }
        );
        assert_eq!(classify_pull_line("Auto-merging new"), PullLine::Action { verb: "Auto-merging".into(), path: "new".into() });
        assert_eq!(classify_pull_line("Failed to merge submodule a (commits don't follow merge-base)"), PullLine::Result(PullResult::Failed));
        assert_eq!(classify_pull_line("Already merged"), PullLine::Other);
    }

    #[test]
    fn submodule_status() {
        let m = parse_submodule_status("+82aa575 doc (heads/main-1-g82aa575)\n 26bb4d4 my lib\n-1111111 gone\n");
        assert_eq!(m.get("doc").map(String::as_str), Some("82aa575"));
        assert_eq!(m.get("my lib").map(String::as_str), Some("26bb4d4"));
        assert!(!m.contains_key("gone"));
    }
}
