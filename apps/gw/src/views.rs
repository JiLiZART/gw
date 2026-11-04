//! `gw branch`, `gw commit`, `gw stash list`: run git, print a compact colored view.

use std::{
    ffi::OsString,
    process::{Command, Stdio},
};

use gw_parse::{BRANCH_FORMAT, STASH_FORMAT, parse_branches, parse_numstat_z, parse_stash_list};
use gw_render::{
    style::no_color_env,
    views::{render_branches, render_commit_summary, render_stash_list},
};

use crate::{exit_code, passthrough, term_width};

fn with(cmd: &str, args: &[OsString]) -> Vec<OsString> {
    let mut v: Vec<OsString> = vec![cmd.into()];
    v.extend(args.iter().cloned());
    v
}

fn git_out(args: &[&str]) -> Option<Vec<u8>> {
    let o = Command::new("git").args(args).stderr(Stdio::null()).output().ok()?;
    o.status.success().then_some(o.stdout)
}

/// (files, added, deleted) of a diff given as `--numstat -z` output.
fn totals(numstat: &[u8]) -> (usize, u64, u64) {
    let m = parse_numstat_z(numstat);
    let (a, d) = m.values().filter_map(|s| *s).fold((0, 0), |t, (a, d)| (t.0 + a, t.1 + d));
    (m.len(), a, d)
}

pub fn branch(args: &[OsString]) -> i32 {
    if !args.is_empty() {
        return passthrough(&with("branch", args));
    }
    let out = Command::new("git").args(["for-each-ref", "--sort=-committerdate", BRANCH_FORMAT, "refs/heads"]).output();
    match out {
        Ok(o) if o.status.success() => {
            print!("{}", render_branches(&parse_branches(&String::from_utf8_lossy(&o.stdout)), term_width(), !no_color_env()));
            0
        }
        // Not a repo, etc.: let git print its own error.
        _ => passthrough(&with("branch", args)),
    }
}

const COMMIT_FORMAT_FLAGS: [&str; 5] = ["--dry-run", "--porcelain", "--short", "--long", "-z"];

pub fn commit(args: &[OsString]) -> i32 {
    if args.iter().any(|a| COMMIT_FORMAT_FLAGS.contains(&a.to_string_lossy().as_ref())) {
        return passthrough(&with("commit", args));
    }
    // Real terminal for the editor and hooks; --quiet drops git's own summary.
    let code = exit_code(Command::new("git").arg("commit").arg("--quiet").args(args).status());
    if code != 0 {
        return code;
    }
    let info = git_out(&["log", "-1", "--format=%h%x1f%s"]).unwrap_or_default();
    let info = String::from_utf8_lossy(&info);
    let (short, subject) = info.trim_end().split_once('\x1f').unwrap_or(("", ""));
    let branch = git_out(&["symbolic-ref", "-q", "--short", "HEAD"])
        .map(|b| String::from_utf8_lossy(&b).trim().to_string())
        .unwrap_or_else(|| "detached".into());
    let stat = git_out(&["show", "--format=", "--diff-merges=first-parent", "-M", "--numstat", "-z", "HEAD"]).unwrap_or_default();
    let (files, add, del) = totals(&stat);
    print!("{}", render_commit_summary(short, &branch, subject, files, add, del, !no_color_env()));
    0
}

pub fn stash(args: &[OsString]) -> i32 {
    if args.len() != 1 || args[0] != "list" {
        return passthrough(&with("stash", args));
    }
    let Some(out) = git_out(&["stash", "list", STASH_FORMAT]) else {
        return passthrough(&with("stash", args));
    };
    let rows: Vec<_> = parse_stash_list(&String::from_utf8_lossy(&out))
        .into_iter()
        .map(|s| {
            let stat = git_out(&["stash", "show", "--numstat", "-z", &s.reference]).unwrap_or_default();
            let (f, a, d) = totals(&stat);
            (s, f, a, d)
        })
        .collect();
    print!("{}", render_stash_list(&rows, !no_color_env()));
    0
}
