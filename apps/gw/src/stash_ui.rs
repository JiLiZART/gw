//! `gw stash -i`: stash browser and new-stash flow. State and drawing live in
//! `gw_render::stash`; this module runs git and owns the terminal.

use std::{
    io,
    process::{Command, Stdio},
    time::Duration,
};

use gw_parse::{Entry, STASH_FORMAT, Stash, parse_name_status_z, parse_numstat_z, parse_stash_list};
use gw_render::{
    stage::{Item, add_items},
    stash::{App, DiffSource, Effect, view},
    status::Group,
    style::no_color_env,
};
use ratatui::{
    DefaultTerminal,
    crossterm::event::{self, Event, KeyEventKind},
};

use crate::stage::{Repo, bytes_as_added, first_line, load_diff, out_lines};

/// Untracked blobs bigger than this show only their start.
const MAX_BLOB: usize = 1 << 20;

fn bytes(mut c: Command) -> Option<Vec<u8>> {
    let o = c.stdin(Stdio::null()).stderr(Stdio::null()).output().ok()?;
    o.status.success().then_some(o.stdout)
}

fn list(repo: &Repo) -> Result<Vec<Stash>, String> {
    let o = repo.git(&["stash", "list", STASH_FORMAT]).stdin(Stdio::null()).output().map_err(|e| e.to_string())?;
    if !o.status.success() {
        return Err(first_line(&o.stderr));
    }
    Ok(parse_stash_list(&String::from_utf8_lossy(&o.stdout)))
}

/// `stash@{N}^3`: the untracked-files commit, when the stash was made with `-u`.
fn untracked_rev(repo: &Repo, s: &Stash) -> Option<String> {
    let rev = format!("{}^3", s.oid);
    bytes(repo.git(&["rev-parse", "-q", "--verify", &rev])).map(|_| rev)
}

fn entry(x: char, path: String, orig: Option<String>) -> Entry {
    Entry { x, y: '.', unmerged: false, path, orig, submodule: None }
}

/// Tracked changes against the stash's base, then the untracked files it holds.
fn stash_files(repo: &Repo, s: &Stash) -> Vec<Item> {
    let base = format!("{}^1", s.oid);
    let names = bytes(repo.git(&["diff", "-M", "--name-status", "-z", &base, &s.oid])).unwrap_or_default();
    let stats = parse_numstat_z(&bytes(repo.git(&["diff", "-M", "--numstat", "-z", &base, &s.oid])).unwrap_or_default());
    let mut files: Vec<Item> = parse_name_status_z(&names)
        .into_iter()
        .map(|(x, path, orig)| Item { group: Group::Staged, stat: stats.get(&path).copied(), entry: entry(x, path, orig), check: None })
        .collect();
    if let Some(rev) = untracked_rev(repo, s) {
        let stats = parse_numstat_z(&bytes(repo.git(&["show", "--format=", "--numstat", "-z", &rev])).unwrap_or_default());
        let names = bytes(repo.git(&["ls-tree", "-r", "-z", "--name-only", &rev])).unwrap_or_default();
        files.extend(names.split(|&b| b == 0).filter(|p| !p.is_empty()).map(|p| {
            let path = String::from_utf8_lossy(p).into_owned();
            Item { group: Group::Untracked, stat: stats.get(&path).copied(), entry: entry('?', path, None), check: None }
        }));
    }
    files
}

fn stash_diff(repo: &Repo, s: &Stash, it: &Item) -> Vec<String> {
    if it.group == Group::Untracked {
        let Some(rev) = untracked_rev(repo, s) else { return vec![] };
        let blob = bytes(repo.git(&["cat-file", "blob", &format!("{rev}:{}", it.entry.path)])).unwrap_or_default();
        let mut lines = bytes_as_added(&blob[..blob.len().min(MAX_BLOB)]);
        if blob.len() > MAX_BLOB && !blob.contains(&0) {
            lines.push("… (first 1 MiB shown)".into());
        }
        return lines;
    }
    let base = format!("{}^1", s.oid);
    let mut args = vec!["diff", "--no-ext-diff", "--no-color", "-M", &base, &s.oid, "--", &it.entry.path];
    args.extend(it.entry.orig.as_deref());
    out_lines(repo.git(&args))
}

/// Runs git; `Err` is git's first error line (stderr, else stdout: conflicts print there).
fn run_reporting(mut c: Command) -> Result<(), String> {
    match c.stdin(Stdio::null()).output() {
        Ok(o) if o.status.success() => Ok(()),
        Ok(o) => {
            let err = first_line(&o.stderr);
            Err(if o.stderr.iter().all(u8::is_ascii_whitespace) { first_line(&o.stdout) } else { err })
        }
        Err(e) => Err(e.to_string()),
    }
}

/// `stash@{N}` still names the stash on screen.
fn still_there(repo: &Repo, s: &Stash) -> bool {
    bytes(repo.git(&["rev-parse", "-q", "--verify", &s.reference])).is_some_and(|o| String::from_utf8_lossy(&o).trim() == s.oid)
}

fn on_stash(repo: &Repo, s: &Stash, verb: &str) -> Result<(), String> {
    if !still_there(repo, s) {
        return Err("stash list changed, nothing done".into());
    }
    run_reporting(repo.git(&["stash", verb, "-q", &s.reference]))
}

fn push_args(paths: &[String], untracked: bool, message: Option<&str>, all: bool) -> Vec<String> {
    let mut args: Vec<String> = vec!["stash".into(), "push".into(), "-q".into()];
    if untracked {
        args.push("-u".into());
    }
    if let Some(m) = message {
        args.extend(["-m".into(), m.into()]);
    }
    if !all {
        args.push("--".into());
        args.extend(paths.iter().cloned());
    }
    args
}

/// Runs one effect. Returns `false` to quit.
fn handle(effect: Effect, app: &mut App, repo: &Repo) -> bool {
    let (result, ok) = match effect {
        Effect::None => return true,
        Effect::Quit => return false,
        Effect::Reload => (Ok(()), None),
        Effect::Apply(s) => (on_stash(repo, &s, "apply"), Some(format!("applied {}", s.reference))),
        Effect::Pop(s) => (on_stash(repo, &s, "pop"), Some(format!("popped {}", s.reference))),
        Effect::Drop(s) => (on_stash(repo, &s, "drop"), Some(format!("dropped {}", s.reference))),
        Effect::OpenPicker => {
            match repo.status() {
                Ok((st, stats, _)) => app.open_picker(add_items(&st, &stats)),
                Err(e) => app.status = Some((e, true)),
            }
            return true;
        }
        Effect::Push { paths, untracked, message, all } => {
            let args = push_args(&paths, untracked, message.as_deref(), all);
            (run_reporting(repo.git(&args.iter().map(String::as_str).collect::<Vec<_>>())), Some("stash created".into()))
        }
    };
    match list(repo) {
        Ok(l) => app.replace_stashes(l),
        Err(e) => app.status = Some((e, true)),
    }
    match result {
        Err(e) => app.status = Some((e, true)),
        Ok(()) => app.status = ok.map(|m| (m, false)),
    }
    true
}

fn event_loop(term: &mut DefaultTerminal, app: &mut App, repo: &Repo) -> io::Result<()> {
    let mut dirty = true;
    loop {
        if let Some(s) = app.files_wanted().cloned() {
            let files = stash_files(repo, &s);
            app.set_files(&s.oid, files);
            dirty = true;
        }
        if let Some(src) = app.diff_wanted() {
            let lines = match &src {
                DiffSource::Stash { stash, file } => stash_diff(repo, stash, file),
                DiffSource::Work(it) => load_diff(repo, it),
            };
            app.set_diff(&src, lines);
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
                    if !handle(app.key(k), app, repo) {
                        return Ok(());
                    }
                    dirty = true;
                }
                Event::Resize(..) => dirty = true,
                _ => {}
            }
        }
    }
}

pub fn run() -> i32 {
    let repo = match Repo::open(&[]) {
        Ok(r) => r,
        Err(code) => return code,
    };
    let stashes = match list(&repo) {
        Ok(l) => l,
        Err(e) => {
            eprintln!("{e}");
            return 128;
        }
    };
    let mut app = App::new(stashes, repo.prefix.clone());
    app.color = !no_color_env();
    // ratatui::init: raw mode + alternate screen + a panic hook that restores the terminal.
    let mut term = ratatui::init();
    let res = event_loop(&mut term, &mut app, &repo);
    ratatui::restore();
    match res {
        Ok(()) => 0,
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

    /// Scratch repo with `a.rs`, `b.rs`, `c.rs`, `gone.rs` committed. Stash commands need an
    /// identity, so `repo.git` is wrapped with the test env by [`git`].
    fn repo(name: &str) -> (Dir, Repo) {
        let p = std::env::temp_dir().join(format!("gw-stash-{}-{name}", std::process::id()));
        let _ = std::fs::remove_dir_all(&p);
        std::fs::create_dir_all(&p).unwrap();
        let d = Dir(p.canonicalize().unwrap());
        let r = Repo { top: d.0.to_string_lossy().into_owned(), prefix: String::new(), args: vec![] };
        git(&r, &["init", "-q", "-b", "main"]).unwrap();
        for f in ["a.rs", "b.rs", "c.rs", "gone.rs"] {
            write(&r, f, "1\n");
        }
        git(&r, &["add", "."]).unwrap();
        git(&r, &["commit", "-q", "-m", "init"]).unwrap();
        (d, r)
    }

    fn git(r: &Repo, args: &[&str]) -> Result<(), String> {
        let mut c = r.git(args);
        c.envs(ENV);
        run_reporting(c)
    }

    fn write(r: &Repo, p: &str, s: &str) {
        std::fs::write(Path::new(&r.top).join(p), s).unwrap();
    }

    fn read(r: &Repo, p: &str) -> Option<String> {
        std::fs::read_to_string(Path::new(&r.top).join(p)).ok()
    }

    fn push(r: &Repo, paths: &[&str], untracked: bool, msg: Option<&str>, all: bool) {
        let paths: Vec<String> = paths.iter().map(|s| s.to_string()).collect();
        let args = push_args(&paths, untracked, msg, all);
        git(r, &args.iter().map(String::as_str).collect::<Vec<_>>()).unwrap();
    }

    #[test]
    fn files_and_diffs_of_a_u_stash() {
        let (_d, r) = repo("files");
        write(&r, "a.rs", "1\n2\n");
        std::fs::remove_file(Path::new(&r.top).join("gone.rs")).unwrap();
        write(&r, "n.txt", "x\ny\n");
        push(&r, &[], true, Some("all"), true);
        let s = &list(&r).unwrap()[0];
        assert_eq!(s.message, "all");
        let files: Vec<_> = stash_files(&r, s).into_iter().map(|i| (i.code(), i.entry.path, i.stat)).collect();
        assert_eq!(
            files,
            vec![('M', "a.rs".into(), Some(Some((1, 0)))), ('D', "gone.rs".into(), Some(Some((0, 1)))), ('?', "n.txt".into(), Some(Some((2, 0))))]
        );
        let items = stash_files(&r, s);
        assert!(stash_diff(&r, s, &items[0]).contains(&"+2".to_string()));
        assert_eq!(stash_diff(&r, s, &items[2]), vec!["+x", "+y"]);
    }

    #[test]
    fn subset_push_keeps_the_rest() {
        let (_d, r) = repo("subset");
        write(&r, "a.rs", "A\n");
        write(&r, "b.rs", "B\n");
        write(&r, "c.rs", "C\n");
        write(&r, "n.txt", "n\n");
        write(&r, "m.txt", "m\n");
        push(&r, &["a.rs", "b.rs", "n.txt"], true, Some("wip parser"), false);
        assert_eq!(read(&r, "a.rs").as_deref(), Some("1\n"));
        assert_eq!(read(&r, "c.rs").as_deref(), Some("C\n"), "unticked file keeps its changes");
        assert!(read(&r, "n.txt").is_none(), "ticked untracked file is stashed");
        assert_eq!(read(&r, "m.txt").as_deref(), Some("m\n"), "unticked untracked file stays");
        let s = list(&r).unwrap().remove(0);
        assert_eq!(s.message, "wip parser");
        // Default message without -m.
        push(&r, &["c.rs"], false, None, false);
        let wip = s_at(&r, 0);
        assert!(wip.branch.as_deref() == Some("main") && wip.message.ends_with(" init"), "git's WIP message: {wip:?}");
        // Apply keeps, pop removes.
        on_stash(&r, &s_at(&r, 1), "apply").unwrap();
        assert_eq!(read(&r, "a.rs").as_deref(), Some("A\n"));
        assert_eq!(list(&r).unwrap().len(), 2);
        on_stash(&r, &s_at(&r, 0), "pop").unwrap();
        assert_eq!(read(&r, "c.rs").as_deref(), Some("C\n"));
        assert_eq!(list(&r).unwrap().len(), 1);
    }

    fn s_at(r: &Repo, i: usize) -> Stash {
        list(r).unwrap().remove(i)
    }

    #[test]
    fn guard_refuses_shifted_list() {
        let (_d, r) = repo("guard");
        write(&r, "a.rs", "A\n");
        push(&r, &[], false, Some("first"), true);
        let shown = s_at(&r, 0);
        write(&r, "b.rs", "B\n");
        push(&r, &[], false, Some("second"), true);
        let err = on_stash(&r, &shown, "drop").unwrap_err();
        assert!(err.contains("stash list changed"), "{err}");
        assert_eq!(list(&r).unwrap().len(), 2, "nothing dropped");
        on_stash(&r, &s_at(&r, 1), "drop").unwrap();
        assert_eq!(list(&r).unwrap()[0].message, "second");
    }

    #[test]
    fn pop_conflict_keeps_stash() {
        let (_d, r) = repo("conflict");
        write(&r, "a.rs", "stashed\n");
        push(&r, &[], false, Some("x"), true);
        write(&r, "a.rs", "local\n");
        git(&r, &["commit", "-q", "-am", "local"]).unwrap();
        write(&r, "a.rs", "dirty\n");
        let err = on_stash(&r, &s_at(&r, 0), "pop").unwrap_err();
        assert!(!err.is_empty() && err != "git failed", "{err}");
        assert_eq!(list(&r).unwrap().len(), 1, "stash kept");
    }
}
