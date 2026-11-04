//! End-to-end: temp repo + bare remote, `gw` run on a pseudo-terminal via `script(1)`.
#![cfg(unix)]

use std::{
    path::{Path, PathBuf},
    process::Command,
};

const GW: &str = env!("CARGO_BIN_EXE_gw");

fn git_env(c: &mut Command) -> &mut Command {
    c.env("GIT_CONFIG_GLOBAL", "/dev/null")
        .env("GIT_CONFIG_NOSYSTEM", "1")
        .env("GIT_AUTHOR_NAME", "T")
        .env("GIT_AUTHOR_EMAIL", "t@e.x")
        .env("GIT_COMMITTER_NAME", "T")
        .env("GIT_COMMITTER_EMAIL", "t@e.x")
}

fn git(dir: &Path, args: &[&str]) {
    let ok = git_env(Command::new("git").args(args).current_dir(dir)).output().unwrap();
    assert!(ok.status.success(), "git {args:?}: {}", String::from_utf8_lossy(&ok.stderr));
}

/// Runs `gw args` on a pty. Returns (exit code, combined output).
fn gw(dir: &Path, args: &[&str], no_color: bool) -> (i32, String) {
    let mut c = if cfg!(target_os = "macos") {
        let mut c = Command::new("script");
        c.args(["-q", "/dev/null", GW]).args(args);
        c
    } else {
        let mut c = Command::new("script");
        c.args(["-qec", &format!("{GW} {}", args.join(" ")), "/dev/null"]);
        c
    };
    c.current_dir(dir).env_remove("NO_COLOR");
    if no_color {
        c.env("NO_COLOR", "1");
    }
    let out = git_env(&mut c).output().unwrap();
    (out.status.code().unwrap_or(-1), String::from_utf8_lossy(&out.stdout).into_owned())
}

/// True if `s` contains an SGR sequence (`ESC [ params m`).
fn has_sgr(s: &str) -> bool {
    s.split("\x1b[").skip(1).any(|t| {
        let end = t.find(|c: char| !(c.is_ascii_digit() || c == ';'));
        end.is_some_and(|i| t[i..].starts_with('m'))
    })
}

struct TempDir(PathBuf);
impl Drop for TempDir {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

#[test]
fn network_and_status_commands() {
    let tmp = TempDir(std::env::temp_dir().join(format!("gw-it-{}", std::process::id())));
    let root = &tmp.0;
    let _ = std::fs::remove_dir_all(root);
    std::fs::create_dir_all(root).unwrap();
    git(root, &["init", "-q", "--bare", "-b", "main", "remote.git"]);
    let hook = root.join("remote.git/hooks/post-receive");
    std::fs::write(
        &hook,
        "#!/bin/sh\necho\necho 'View merge request for feat:'\necho '  https://gitlab.example.com/g/r/-/merge_requests/1'\necho\n",
    )
    .unwrap();
    std::fs::set_permissions(&hook, std::os::unix::fs::PermissionsExt::from_mode(0o755)).unwrap();

    git(root, &["init", "-q", "-b", "main", "work"]);
    let work = root.join("work");
    git(&work, &["remote", "add", "origin", "../remote.git"]);
    for i in 0..20 {
        std::fs::write(work.join(format!("f{i}.txt")), format!("{i}\n").repeat(500)).unwrap();
    }
    git(&work, &["add", "."]);
    git(&work, &["commit", "-qm", "init"]);

    // push (new branch) with remote box, NO_COLOR
    let (code, out) = gw(&work, &["push", "-u", "origin", "main"], true);
    assert_eq!(code, 0, "{out}");
    assert!(out.contains("┌ Merge request"), "{out}");
    assert!(out.contains("+ new main -> main"), "{out}");
    assert!(out.contains("\x1b]8;;https://gitlab.example.com/g/r/-/merge_requests/1"), "OSC 8 kept under NO_COLOR");
    assert!(!has_sgr(&out), "NO_COLOR output has SGR: {out:?}");

    // push again: up to date
    let (code, out) = gw(&work, &["push"], true);
    assert_eq!(code, 0);
    assert!(out.contains("Everything up-to-date"), "{out}");

    // clone
    let (code, out) = gw(root, &["clone", "remote.git", "other"], true);
    assert_eq!(code, 0, "{out}");
    let other = root.join("other");
    assert!(other.join("f0.txt").exists());

    // new commit upstream, then fetch + pull from `other`
    std::fs::write(work.join("f1.txt"), "changed\n").unwrap();
    git(&work, &["commit", "-qam", "change"]);
    let (code, out) = gw(&work, &["push"], false);
    assert_eq!(code, 0, "{out}");
    assert!(has_sgr(&out), "colored by default");

    let (code, out) = gw(&other, &["fetch"], true);
    assert_eq!(code, 0, "{out}");
    assert!(out.contains("origin/main"), "{out}");
    assert!(!has_sgr(&out), "{out:?}");

    let (code, out) = gw(&other, &["pull"], true);
    assert_eq!(code, 0, "{out}");
    assert!(out.contains("Fast-forward"), "{out}");
    assert!(out.contains("f1.txt |"), "{out}");
    assert!(!has_sgr(&out), "{out:?}");

    // status
    std::fs::write(other.join("new.txt"), "n").unwrap();
    std::fs::write(other.join("f2.txt"), "m").unwrap();
    let (code, out) = gw(&other, &["status"], true);
    assert_eq!(code, 0, "{out}");
    assert!(out.contains("main -> origin/main"), "{out}");
    assert!(out.contains("Changed (1)") && out.contains("Untracked (1)"), "{out}");
    assert!(!has_sgr(&out), "{out:?}");

    // rejected push exits 1: remote moves on, `other` diverges
    std::fs::write(work.join("f3.txt"), "ahead\n").unwrap();
    git(&work, &["commit", "-qam", "ahead"]);
    git(&work, &["push", "-q"]);
    git(&other, &["commit", "-qam", "diverge"]);
    let (code, out) = gw(&other, &["push"], true);
    assert_eq!(code, 1, "{out}");
    assert!(out.contains("✗ rejected"), "{out}");
}

#[test]
fn log_branch_commit_stash() {
    let tmp = TempDir(std::env::temp_dir().join(format!("gw-it2-{}", std::process::id())));
    let root = &tmp.0;
    let _ = std::fs::remove_dir_all(root);
    std::fs::create_dir_all(root).unwrap();
    git(root, &["init", "-q", "-b", "main", "w"]);
    let w = root.join("w");
    std::fs::write(w.join("a.txt"), "1\n").unwrap();
    git(&w, &["add", "."]);
    git(&w, &["commit", "-qm", "init"]);

    // gw log piped: identical to git log
    let piped = git_env(Command::new(GW).args(["log", "--format=%H %s"]).current_dir(&w)).output().unwrap();
    let plain = git_env(Command::new("git").args(["log", "--format=%H %s"]).current_dir(&w)).output().unwrap();
    assert_eq!(piped.stdout, plain.stdout);
    let piped = git_env(Command::new(GW).arg("log").current_dir(&w)).output().unwrap();
    let plain = git_env(Command::new("git").arg("log").current_dir(&w)).output().unwrap();
    assert_eq!(piped.stdout, plain.stdout, "non-TTY gw log is plain git log");

    // gw commit -m: summary, exit 0, no SGR under NO_COLOR
    std::fs::write(w.join("a.txt"), "1\n2\n").unwrap();
    std::fs::write(w.join("b.txt"), "b\n").unwrap();
    git(&w, &["add", "."]);
    let (code, out) = gw(&w, &["commit", "-m", "feat-two"], true);
    assert_eq!(code, 0, "{out}");
    assert!(out.contains("main  feat-two") && out.contains("2 files  +2 -0"), "{out}");
    assert!(!has_sgr(&out), "{out:?}");

    // nothing to commit: git's exit code, no summary
    let (code, out) = gw(&w, &["commit", "-m", "empty"], true);
    assert_eq!(code, 1, "{out}");
    assert!(!out.contains('✓'), "{out}");

    // gw branch
    git(&w, &["branch", "side"]);
    let (code, out) = gw(&w, &["branch"], true);
    assert_eq!(code, 0, "{out}");
    assert!(out.contains("* main"), "current branch marked: {out}");
    assert!(out.contains("side") && out.contains("no upstream"), "{out}");
    assert!(!has_sgr(&out), "{out:?}");

    // gw stash list
    std::fs::write(w.join("a.txt"), "changed\n").unwrap();
    git(&w, &["stash", "push", "-q", "-m", "try-it"]);
    let (code, out) = gw(&w, &["stash", "list"], true);
    assert_eq!(code, 0, "{out}");
    assert!(out.contains("main  try-it  1 file +1 -2"), "{out}");
    assert!(!has_sgr(&out), "{out:?}");
    let (code, _) = gw(&w, &["stash", "pop"], true);
    assert_eq!(code, 0);
}
