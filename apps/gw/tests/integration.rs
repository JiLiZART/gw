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
        // Local submodule URLs are file transport, blocked by default since git 2.38.
        .env("GIT_CONFIG_COUNT", "1")
        .env("GIT_CONFIG_KEY_0", "protocol.file.allow")
        .env("GIT_CONFIG_VALUE_0", "always")
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

    // gw status -i piped: `-i` dropped, identical to git status
    std::fs::write(w.join("a.txt"), "1\nx\n").unwrap();
    for args in [&["status", "-i"][..], &["status", "--interactive", "-s"]] {
        let piped = git_env(Command::new(GW).args(args).current_dir(&w)).output().unwrap();
        let rest: Vec<&str> = args.iter().copied().filter(|a| !matches!(*a, "-i" | "--interactive")).collect();
        let plain = git_env(Command::new("git").args(&rest).current_dir(&w)).output().unwrap();
        assert!(piped.status.success(), "{args:?}: {}", String::from_utf8_lossy(&piped.stderr));
        assert_eq!(piped.stdout, plain.stdout, "{args:?}");
    }
    // On a terminal, -i with a format flag is a usage error.
    let (code, out) = gw(&w, &["status", "-i", "-s"], true);
    assert_eq!(code, 129, "{out}");
    assert!(out.contains("status -i cannot be combined with -s"), "{out}");

    // gw add / gw stash -i piped: plain git (same output and exit code)
    for args in [&["add"][..], &["stash", "-i"]] {
        let piped = git_env(Command::new(GW).args(args).current_dir(&w)).output().unwrap();
        let plain = git_env(Command::new("git").args(args).current_dir(&w)).output().unwrap();
        assert_eq!((piped.status.code(), &piped.stdout, &piped.stderr), (plain.status.code(), &plain.stdout, &plain.stderr), "{args:?}");
    }
    // gw add with arguments is git's add, also on a terminal
    let (code, out) = gw(&w, &["add", "-n", "."], true);
    assert_eq!(code, 0, "{out}");
    assert!(out.contains("add 'a.txt'"), "{out}");
    // gw stash -i with another argument is a usage error
    let (code, out) = gw(&w, &["stash", "-i", "pop"], true);
    assert_eq!(code, 129, "{out}");
    assert!(out.contains("stash -i cannot be combined with pop"), "{out}");
    git(&w, &["checkout", "--", "a.txt"]);

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

#[test]
fn submodules() {
    let tmp = TempDir(std::env::temp_dir().join(format!("gw-it3-{}", std::process::id())));
    let root = &tmp.0;
    let _ = std::fs::remove_dir_all(root);
    std::fs::create_dir_all(root).unwrap();
    git(root, &["init", "-q", "--bare", "-b", "main", "lib.git"]);
    git(root, &["init", "-q", "--bare", "-b", "main", "app.git"]);
    git(root, &["clone", "-q", "lib.git", "libw"]);
    let libw = root.join("libw");
    std::fs::write(libw.join("a"), "1\n").unwrap();
    git(&libw, &["add", "."]);
    git(&libw, &["commit", "-qm", "l1"]);
    git(&libw, &["push", "-q", "origin", "main"]);
    git(root, &["clone", "-q", "app.git", "w"]);
    let w = root.join("w");
    std::fs::write(w.join("x"), "1\n").unwrap();
    git(&w, &["add", "."]);
    git(&w, &["commit", "-qm", "a1"]);
    git(&w, &["submodule", "add", "-q", "../lib.git", "lib"]);
    git(&w, &["commit", "-qm", "sub"]);
    git(&w, &["push", "-q", "origin", "main"]);

    // recursive clone: Submodules section, no registration noise
    let (code, out) = gw(root, &["clone", "--recurse-submodules", "app.git", "o"], true);
    assert_eq!(code, 0, "{out}");
    assert!(out.contains("Submodules") && out.contains("lib  checked out "), "{out}");
    assert!(!out.contains("registered for path"), "{out}");
    assert!(!has_sgr(&out), "{out:?}");
    let o = root.join("o");

    // upstream bumps the submodule
    std::fs::write(libw.join("a"), "1\n2\n").unwrap();
    git(&libw, &["commit", "-qam", "l2"]);
    git(&libw, &["push", "-q"]);
    git(&w, &["submodule", "update", "-q", "--remote", "lib"]);
    std::fs::write(w.join("x"), "1\n2\n").unwrap();
    git(&w, &["commit", "-qam", "a2"]);
    git(&w, &["push", "-q"]);
    let lib_sha = String::from_utf8(git_env(Command::new("git").args(["-C", "lib", "rev-parse", "--short=7", "HEAD"]).current_dir(&w)).output().unwrap().stdout).unwrap();

    let (code, out) = gw(&o, &["pull", "--recurse-submodules"], true);
    assert_eq!(code, 0, "{out}");
    assert!(out.contains("-> submodule lib"), "{out}");
    let app_header = out.find("/app\r").or_else(|| out.find("/app\n")).expect(&out);
    assert!(app_header < out.find("-> submodule lib").unwrap(), "{out}");
    assert!(out.contains(&format!("lib  checked out {}", lib_sha.trim())), "{out}");
    assert!(!out.contains("Fetching submodule") && !out.contains("Submodule path"), "{out}");
    assert!(!has_sgr(&out), "{out:?}");

    // status: modified content inside the submodule, no line stats
    std::fs::write(o.join("lib/a"), "changed\n").unwrap();
    let (code, out) = gw(&o, &["status"], true);
    assert_eq!(code, 0, "{out}");
    assert!(out.contains("Submodules (1)") && out.contains("modified content"), "{out}");
    assert!(!out.contains("+0 -0") && !out.contains("Total"), "{out}");
    assert!(!has_sgr(&out), "{out:?}");
}
