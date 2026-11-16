//! `gw branch -i` and the bare `gw switch` picker. State and drawing live in
//! `gw_render::branch`; this module runs git and owns the terminal.

use std::{io, process::Stdio, time::Duration};

use gw_parse::{BRANCH_FORMAT, Branch, Entry, parse_name_status_z, parse_numstat_z};
use gw_render::{
    branch::{App, Effect, view},
    stage::{Item, items},
    status::Group,
    style::no_color_env,
};
use ratatui::{
    DefaultTerminal,
    crossterm::event::{self, Event, KeyEventKind},
};

use crate::stage::{Repo, first_line, load_diff, out_lines};
use crate::stash_ui::run_reporting;

/// `git hash-object -t tree /dev/null`: base for a branch with no common history.
const EMPTY_TREE: &str = "4b825dc642cb6eb9a060e54bf8d69288fbee4904";

fn bytes(repo: &Repo, args: &[&str]) -> Option<Vec<u8>> {
    let o = repo.git(args).stdin(Stdio::null()).stderr(Stdio::null()).output().ok()?;
    o.status.success().then_some(o.stdout)
}

fn list(repo: &Repo) -> Result<Vec<Branch>, String> {
    let o = repo
        .git(&["for-each-ref", "--sort=-committerdate", BRANCH_FORMAT, "refs/heads", "refs/remotes"])
        .stdin(Stdio::null())
        .output()
        .map_err(|e| e.to_string())?;
    if !o.status.success() {
        return Err(first_line(&o.stderr));
    }
    Ok(gw_parse::parse_branches(&String::from_utf8_lossy(&o.stdout)))
}

/// Where `b` split from `HEAD`; the empty tree without common history or commits.
fn base(repo: &Repo, b: &Branch) -> String {
    bytes(repo, &["merge-base", "HEAD", &b.oid]).map_or(EMPTY_TREE.into(), |o| String::from_utf8_lossy(&o).trim().to_string())
}

fn branch_files(repo: &Repo, b: &Branch) -> Result<Vec<Item>, String> {
    if b.current {
        let (st, stats, _) = repo.status()?;
        return Ok(items(&st, &stats));
    }
    let base = base(repo, b);
    let names = bytes(repo, &["diff", "-M", "--name-status", "-z", &base, &b.oid]).unwrap_or_default();
    let stats = parse_numstat_z(&bytes(repo, &["diff", "-M", "--numstat", "-z", &base, &b.oid]).unwrap_or_default());
    Ok(parse_name_status_z(&names)
        .into_iter()
        .map(|(x, path, orig)| Item {
            group: Group::Staged,
            stat: stats.get(&path).copied(),
            entry: Entry { x, y: '.', unmerged: false, path, orig, submodule: None },
            check: None,
        })
        .collect())
}

fn branch_diff(repo: &Repo, b: &Branch, it: &Item) -> Vec<String> {
    if b.current {
        return load_diff(repo, it);
    }
    let base = base(repo, b);
    let mut args = vec!["diff", "--no-ext-diff", "--no-color", "-M", &base, &b.oid, "--", &it.entry.path];
    args.extend(it.entry.orig.as_deref());
    out_lines(repo.git(&args))
}

/// `refs/<kind>/<name>` still points to the commit on screen.
fn unmoved(repo: &Repo, b: &Branch) -> Result<(), String> {
    let r = format!("refs/{}/{}", if b.remote { "remotes" } else { "heads" }, b.name);
    match bytes(repo, &["rev-parse", "-q", "--verify", &r]) {
        Some(o) if String::from_utf8_lossy(&o).trim() == b.oid => Ok(()),
        _ => Err("branch changed, nothing done".into()),
    }
}

fn run(repo: &Repo, args: &[&str]) -> Result<(), String> {
    run_reporting(repo.git(args))
}

/// `Enter`: returns the local branch now checked out and an optional note.
fn checkout(repo: &Repo, b: &Branch) -> Result<(String, Option<String>), String> {
    if !b.remote {
        return run(repo, &["switch", &b.name]).map(|_| (b.name.clone(), None));
    }
    let (_, local) = view::remote_of(&b.name);
    if bytes(repo, &["rev-parse", "-q", "--verify", &format!("refs/heads/{local}")]).is_some() {
        run(repo, &["switch", local])?;
        return Ok((local.into(), Some(format!("switched to existing local {local}"))));
    }
    run(repo, &["switch", "--track", &b.name]).map(|_| (local.into(), None))
}

/// `Err((msg, unmerged))`: `unmerged` asks for the force prompt.
fn delete(repo: &Repo, b: &Branch, force: bool) -> Result<(), (String, bool)> {
    unmoved(repo, b).map_err(|e| (e, false))?;
    let mut c = repo.git(&["branch", if force { "-D" } else { "-d" }, &b.name]);
    // English message to detect "not fully merged" whatever the user's locale.
    c.env("LC_ALL", "C").env("LANGUAGE", "en");
    run_reporting(c).map_err(|e| {
        let unmerged = !force && e.contains("not fully merged");
        (e, unmerged)
    })
}

/// What to do after an effect.
enum After {
    Stay,
    Quit(i32),
    /// Leave the screen, then run `gw merge --no-edit <branch>`.
    Merge(String),
}

fn handle(effect: Effect, app: &mut App, repo: &Repo, term: &mut DefaultTerminal) -> io::Result<After> {
    let mut select = None;
    let result: Result<String, String> = match effect {
        Effect::None => return Ok(After::Stay),
        Effect::Quit => return Ok(After::Quit(0)),
        Effect::Merge(b) => return Ok(After::Merge(b.name)),
        Effect::Reload => Ok(String::new()),
        Effect::Checkout(b) => match checkout(repo, &b) {
            Ok((name, note)) => {
                if app.pick {
                    return Ok(After::Quit(0));
                }
                let msg = note.unwrap_or_else(|| format!("switched to {name}"));
                select = Some(name);
                Ok(msg)
            }
            Err(e) => Err(e),
        },
        Effect::Create { name, from } => {
            let r = run(repo, &["switch", "--no-track", "-c", &name, &from.oid]).map(|_| format!("created {name}"));
            select = Some(name);
            r
        }
        Effect::Delete { branch, force } => match delete(repo, &branch, force) {
            Ok(()) => Ok(format!("deleted {}", branch.name)),
            Err((_, true)) => {
                app.ask_force(branch);
                return Ok(After::Stay);
            }
            Err((e, false)) => Err(e),
        },
        Effect::DeleteRemote(b) => {
            app.status = Some(("deleting…".into(), false));
            term.draw(|f| view::draw(f, app))?;
            let (remote, name) = view::remote_of(&b.name);
            unmoved(repo, &b).and_then(|_| run(repo, &["push", "-q", remote, "--delete", name])).map(|_| format!("deleted {} on {remote}", b.name))
        }
        Effect::Rename { from, to } => {
            let r = unmoved(repo, &from).and_then(|_| run(repo, &["branch", "-m", &from.name, &to])).map(|_| format!("renamed to {to}"));
            if r.is_ok() {
                select = Some(to);
            }
            r
        }
    };
    match list(repo) {
        Ok(l) => app.replace_branches(l, select.as_deref()),
        Err(e) => app.status = Some((e, true)),
    }
    app.status = match result {
        Err(e) => Some((e, true)),
        Ok(m) if m.is_empty() => None,
        Ok(m) => Some((m, false)),
    };
    Ok(After::Stay)
}

fn event_loop(term: &mut DefaultTerminal, app: &mut App, repo: &Repo) -> io::Result<After> {
    let mut dirty = true;
    loop {
        if let Some(b) = app.files_wanted() {
            match branch_files(repo, &b) {
                Ok(files) => app.set_files(&b, files),
                Err(e) => {
                    app.set_files(&b, vec![]);
                    app.status = Some((e, true));
                }
            }
            dirty = true;
        }
        if let Some((b, it)) = app.diff_wanted() {
            let lines = branch_diff(repo, &b, &it);
            app.set_diff(&b, &it, lines);
            dirty = true;
        }
        if dirty {
            let size = term.size()?;
            app.resize(size.width, size.height);
            term.draw(|f| view::draw(f, app))?;
            dirty = false;
        }
        if event::poll(Duration::from_millis(100))? {
            match event::read()? {
                Event::Key(k) if k.kind != KeyEventKind::Release => {
                    let effect = app.key(k);
                    match handle(effect, app, repo, term)? {
                        After::Stay => dirty = true,
                        done => return Ok(done),
                    }
                }
                Event::Resize(..) => dirty = true,
                _ => {}
            }
        }
    }
}

/// `pick`: the `gw switch` picker.
pub fn run_ui(pick: bool) -> i32 {
    let repo = match Repo::open(&[]) {
        Ok(r) => r,
        Err(code) => return code,
    };
    let branches = match list(&repo) {
        Ok(l) => l,
        Err(e) => {
            eprintln!("{e}");
            return 128;
        }
    };
    let mut app = App::new(branches, repo.prefix.clone());
    app.color = !no_color_env();
    app.pick = pick;
    // ratatui::init: raw mode + alternate screen + a panic hook that restores the terminal.
    let mut term = ratatui::init();
    let res = event_loop(&mut term, &mut app, &repo);
    ratatui::restore();
    match res {
        Ok(After::Quit(code)) => code,
        Ok(After::Merge(b)) => crate::network("merge", &["--no-edit".into(), b.into()]),
        Ok(After::Stay) => 0,
        Err(e) => {
            eprintln!("gw: {e}");
            1
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::Path;

    const ENV: [(&str, &str); 6] = [
        ("GIT_AUTHOR_NAME", "t"),
        ("GIT_AUTHOR_EMAIL", "t@e.x"),
        ("GIT_COMMITTER_NAME", "t"),
        ("GIT_COMMITTER_EMAIL", "t@e.x"),
        ("GIT_CONFIG_GLOBAL", "/dev/null"),
        ("GIT_CONFIG_NOSYSTEM", "1"),
    ];

    struct Dir(std::path::PathBuf);
    impl Drop for Dir {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }

    fn scratch(name: &str) -> (Dir, Repo) {
        let p = std::env::temp_dir().join(format!("gw-branch-{}-{name}", std::process::id()));
        let _ = std::fs::remove_dir_all(&p);
        std::fs::create_dir_all(&p).unwrap();
        let d = Dir(p.canonicalize().unwrap());
        let r = Repo { top: d.0.to_string_lossy().into_owned(), prefix: String::new(), args: vec![] };
        (d, r)
    }

    /// Repo on `main` with `a.rs` committed; `feat` adds a line to `a.rs` and a `new.rs`.
    fn repo(name: &str) -> (Dir, Repo) {
        let (d, r) = scratch(name);
        git(&r, &["init", "-q", "-b", "main"]);
        write(&r, "a.rs", "1\n");
        git(&r, &["add", "."]);
        git(&r, &["commit", "-q", "-m", "init"]);
        git(&r, &["switch", "-q", "-c", "feat"]);
        write(&r, "a.rs", "1\n2\n");
        write(&r, "new.rs", "n\n");
        git(&r, &["add", "."]);
        git(&r, &["commit", "-q", "-m", "feat work"]);
        git(&r, &["switch", "-q", "main"]);
        (d, r)
    }

    fn git(r: &Repo, args: &[&str]) {
        let mut c = r.git(args);
        c.envs(ENV);
        run_reporting(c).unwrap();
    }

    fn write(r: &Repo, p: &str, s: &str) {
        std::fs::write(Path::new(&r.top).join(p), s).unwrap();
    }

    fn get(r: &Repo, name: &str) -> Branch {
        list(r).unwrap().into_iter().find(|b| b.name == name).unwrap()
    }

    fn current(r: &Repo) -> String {
        list(r).unwrap().into_iter().find(|b| b.current).unwrap().name
    }

    #[test]
    fn files_of_branches() {
        let (_d, r) = repo("files");
        let feat = get(&r, "feat");
        let files: Vec<_> = branch_files(&r, &feat).unwrap().into_iter().map(|i| (i.code(), i.entry.path, i.stat)).collect();
        assert_eq!(files, vec![('M', "a.rs".into(), Some(Some((1, 0)))), ('A', "new.rs".into(), Some(Some((1, 0))))]);
        assert!(branch_diff(&r, &feat, &branch_files(&r, &feat).unwrap()[0]).contains(&"+2".to_string()));
        // Merged: nothing new.
        git(&r, &["branch", "old", "main"]);
        assert!(branch_files(&r, &get(&r, "old")).unwrap().is_empty());
        // Current: the working tree (`Repo::status` reads the cwd, so build the rows with `-C`).
        write(&r, "a.rs", "dirty\n");
        let main = get(&r, "main");
        let st = gw_parse::parse_status_v2(&r.git(&["status", "--porcelain=v2", "-z"]).output().unwrap().stdout);
        let work = items(&st, &Default::default());
        assert_eq!(work.iter().map(|i| i.entry.path.as_str()).collect::<Vec<_>>(), ["a.rs"]);
        assert!(branch_diff(&r, &main, &work[0]).contains(&"+dirty".to_string()));
    }

    #[test]
    fn checkout_create_rename_delete() {
        let (_d, r) = repo("actions");
        checkout(&r, &get(&r, "feat")).unwrap();
        assert_eq!(current(&r), "feat");
        // Dirty tree blocks the switch and keeps the branch.
        write(&r, "a.rs", "conflict\n");
        let err = checkout(&r, &get(&r, "main")).unwrap_err();
        assert!(err.starts_with("error:"), "{err}");
        assert_eq!(current(&r), "feat");
        git(&r, &["checkout", "-q", "--", "a.rs"]);
        // New branch from main, no tracking.
        run(&r, &["switch", "--no-track", "-c", "fix", &get(&r, "main").oid]).unwrap();
        assert_eq!((current(&r), get(&r, "fix").oid), ("fix".into(), get(&r, "main").oid));
        // Rename with the guard.
        let fix = get(&r, "fix");
        git(&r, &["switch", "-q", "main"]);
        unmoved(&r, &fix).unwrap();
        run(&r, &["branch", "-m", "fix", "fix2"]).unwrap();
        assert!(unmoved(&r, &fix).is_err(), "renamed away");
        // Merged deletes; unmerged asks to force.
        delete(&r, &get(&r, "fix2"), false).unwrap();
        let feat = get(&r, "feat");
        assert!(delete(&r, &feat, false).unwrap_err().1, "not fully merged");
        delete(&r, &feat, true).unwrap();
        assert!(list(&r).unwrap().iter().all(|b| b.name != "feat"));
    }

    #[test]
    fn moved_branch_refuses_delete() {
        let (_d, r) = repo("moved");
        let shown = get(&r, "feat");
        git(&r, &["branch", "-f", "feat", "main"]);
        let (e, unmerged) = delete(&r, &shown, true).unwrap_err();
        assert!(e.contains("branch changed") && !unmerged, "{e}");
        assert!(list(&r).unwrap().iter().any(|b| b.name == "feat"));
    }

    #[test]
    fn remote_checkout_and_delete() {
        let (_o, origin) = repo("origin");
        let (_d, r) = scratch("clone");
        let mut c = crate::git();
        c.args(["clone", "-q", &origin.top, &r.top]).envs(ENV);
        run_reporting(c).unwrap();
        let names: Vec<_> = list(&r).unwrap().into_iter().map(|b| (b.name, b.remote)).collect();
        assert!(names.contains(&("origin/feat".into(), true)) && !names.iter().any(|n| n.0.ends_with("HEAD")), "{names:?}");
        // No local feat: a tracking branch is made.
        let (local, note) = checkout(&r, &get(&r, "origin/feat")).unwrap();
        assert_eq!((local.as_str(), note), ("feat", None));
        assert_eq!(get(&r, "feat").upstream.as_deref(), Some("origin/feat"));
        // Local exists now: plain switch with a note.
        git(&r, &["switch", "-q", "main"]);
        let (_, note) = checkout(&r, &get(&r, "origin/feat")).unwrap();
        assert_eq!(note.as_deref(), Some("switched to existing local feat"));
        // Delete on the server.
        let b = get(&r, "origin/feat");
        unmoved(&r, &b).unwrap();
        run(&r, &["push", "-q", "origin", "--delete", "feat"]).unwrap();
        assert!(list(&r).unwrap().iter().all(|b| b.name != "origin/feat"));
        assert!(list(&origin).unwrap().iter().all(|b| b.name != "feat"));
    }
}
