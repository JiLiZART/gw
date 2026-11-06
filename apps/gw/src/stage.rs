//! `gw status -i`: interactive staging. State and drawing live in `gw_render::stage`;
//! this module runs git and owns the terminal.

use std::{
    ffi::OsString,
    io::{self, Read},
    path::Path,
    process::{Command, Stdio},
    time::Duration,
};

use gw_parse::{Entry, parse_status_v2};
use gw_render::{
    stage::{App, Effect, Item, items, view},
    status::{Group, LineStats, render_status},
    style::no_color_env,
};
use ratatui::{
    DefaultTerminal,
    crossterm::event::{self, Event, KeyEventKind},
};

use crate::{exit_code, load_stats, term::run_on_terminal};

/// Untracked files bigger than this show only their start.
const MAX_FILE: u64 = 1 << 20;
/// Files listed in the revert modal for an untracked directory.
const MODAL_FILES: usize = 10;

/// Repository paths and the user's `git status` arguments.
struct Repo {
    top: String,
    prefix: String,
    args: Vec<OsString>,
}

impl Repo {
    /// `git -C <top> --literal-pathspecs <args>`: paths from porcelain are top-relative and literal.
    fn git(&self, args: &[&str]) -> Command {
        let mut c = Command::new("git");
        c.arg("-C").arg(&self.top).arg("--literal-pathspecs").args(args);
        c
    }

    /// Runs git quietly; `Err` carries git's first error line.
    fn run(&self, args: &[&str]) -> Result<(), String> {
        run_quiet(self.git(args))
    }

    /// The user's pathspecs (non-option arguments, everything after `--`), as given in cwd.
    fn pathspecs(&self) -> Vec<OsString> {
        let mut out = Vec::new();
        let mut after = false;
        for a in &self.args {
            if after || !a.to_string_lossy().starts_with('-') {
                out.push(a.clone());
            } else if a == "--" {
                after = true;
            }
        }
        out
    }

    /// `(items, branch line)` from a fresh `git status`.
    fn load(&self) -> Result<(Vec<Item>, String), String> {
        let out = Command::new("git").args(["status", "--porcelain=v2", "--branch", "-z"]).args(&self.args).output().map_err(|e| e.to_string())?;
        if !out.status.success() {
            return Err(first_line(&out.stderr));
        }
        let st = parse_status_v2(&out.stdout);
        let stats = load_stats(&st, &self.prefix);
        let head = render_status(&st, &self.prefix, &LineStats::default(), false).lines().next().unwrap_or_default().to_string();
        Ok((items(&st, &stats), head))
    }
}

fn run_quiet(mut c: Command) -> Result<(), String> {
    match c.stdin(Stdio::null()).stdout(Stdio::null()).stderr(Stdio::piped()).output() {
        Ok(o) if o.status.success() => Ok(()),
        Ok(o) => Err(first_line(&o.stderr)),
        Err(e) => Err(e.to_string()),
    }
}

fn first_line(b: &[u8]) -> String {
    String::from_utf8_lossy(b).lines().find(|l| !l.trim().is_empty()).unwrap_or("git failed").to_string()
}

fn out_lines(mut c: Command) -> Vec<String> {
    c.stderr(Stdio::null()).output().map(|o| String::from_utf8_lossy(&o.stdout).lines().map(str::to_string).collect()).unwrap_or_default()
}

/// Untracked files under `path` (a file or a `dir/`), top-relative.
fn untracked_under(repo: &Repo, path: &str) -> Vec<String> {
    let out = repo.git(&["ls-files", "--others", "--exclude-standard", "-z", "--", path]).stderr(Stdio::null()).output();
    out.map(|o| o.stdout.split(|&b| b == 0).filter(|p| !p.is_empty()).map(|p| String::from_utf8_lossy(p).into_owned()).collect()).unwrap_or_default()
}

/// Whole content of an untracked file as added lines.
fn file_as_added(full: &Path) -> Vec<String> {
    let Ok(f) = std::fs::File::open(full) else { return vec!["(unreadable)".into()] };
    let mut bytes = Vec::new();
    let _ = f.take(MAX_FILE).read_to_end(&mut bytes);
    if bytes.contains(&0) {
        return vec!["binary file".into()];
    }
    let mut lines: Vec<String> = String::from_utf8_lossy(&bytes).lines().map(|l| format!("+{l}")).collect();
    if std::fs::metadata(full).is_ok_and(|m| m.len() > MAX_FILE) {
        lines.push("… (first 1 MiB shown)".into());
    }
    lines
}

fn submodule_labels(e: &Entry) -> Vec<String> {
    let Some(s) = &e.submodule else { return vec![] };
    let labels: Vec<&str> = [(s.commit_changed, "new commits"), (s.modified, "modified content"), (s.untracked, "untracked content")]
        .into_iter()
        .filter(|l| l.0)
        .map(|l| l.1)
        .collect();
    if labels.is_empty() { vec![] } else { vec![format!("submodule: {}", labels.join(", ")), String::new()] }
}

fn load_diff(repo: &Repo, it: &Item) -> Vec<String> {
    let e = &it.entry;
    let mut paths = vec![e.path.as_str()];
    let diff = |cached: bool, extra: &[&str], paths: &[&str]| {
        let mut args = vec!["diff", "--no-ext-diff", "--no-color", "-M"];
        if cached {
            args.push("--cached");
        }
        args.extend(extra);
        args.push("--");
        args.extend(paths);
        out_lines(repo.git(&args))
    };
    match it.group {
        Group::Staged => {
            paths.extend(e.orig.as_deref());
            diff(true, &[], &paths)
        }
        Group::Changed | Group::Conflicts => diff(false, &[], &paths),
        Group::Submodules => {
            let mut lines = submodule_labels(e);
            lines.extend(diff(false, &["--submodule=log"], &paths));
            lines.extend(diff(true, &["--submodule=log"], &paths));
            lines
        }
        Group::Untracked if e.path.ends_with('/') => {
            let files = untracked_under(repo, &e.path);
            let mut lines = vec![format!("{} untracked file{}:", files.len(), if files.len() == 1 { "" } else { "s" })];
            lines.extend(files.into_iter().map(|f| format!("  {f}")));
            lines
        }
        Group::Untracked => file_as_added(&Path::new(&repo.top).join(&e.path)),
    }
}

/// Git commands for a revert (design.md decision 4), run in order.
fn revert_steps(it: &Item) -> Vec<Vec<String>> {
    let e = &it.entry;
    let p = e.path.clone();
    let v = |a: &[&str]| a.iter().map(|s| s.to_string()).collect::<Vec<_>>();
    let restore_head = |path: &str| [v(&["restore", "--source=HEAD", "--staged", "--worktree", "--"]), vec![path.to_string()]].concat();
    match (it.group, e.x) {
        (Group::Changed, _) => vec![[v(&["restore", "--worktree", "--"]), vec![p]].concat()],
        (Group::Staged, 'A' | 'C') => vec![[v(&["rm", "-f", "-q", "--"]), vec![p]].concat()],
        (Group::Staged, 'R') => {
            let mut steps = vec![[v(&["rm", "-f", "-q", "--"]), vec![p]].concat()];
            steps.extend(e.orig.as_deref().map(restore_head));
            steps
        }
        (Group::Staged, _) => vec![restore_head(&p)],
        (Group::Untracked, _) if p.ends_with('/') => vec![[v(&["clean", "-f", "-d", "-q", "--"]), vec![p]].concat()],
        (Group::Untracked, _) => vec![[v(&["clean", "-f", "-q", "--"]), vec![p]].concat()],
        (Group::Conflicts | Group::Submodules, _) => vec![],
    }
}

/// Runs one effect. Returns `false` to quit.
fn handle(effect: Effect, app: &mut App, repo: &Repo, term: &mut DefaultTerminal) -> bool {
    let result = match effect {
        Effect::None => return true,
        Effect::Quit => return false,
        Effect::Reload => Ok(()),
        Effect::Stage(it) => repo.run(&["add", "-A", "--", &it.entry.path]),
        Effect::Unstage(it) => {
            let mut args = vec!["reset", "-q", "--", it.entry.path.as_str()];
            args.extend(it.entry.orig.as_deref());
            repo.run(&args)
        }
        // From the cwd, without literal pathspecs: the user's pathspecs mean what they meant to `git status`.
        Effect::StageAll => run_quiet({
            let mut c = Command::new("git");
            c.args(["add", "-A"]);
            let ps = repo.pathspecs();
            if !ps.is_empty() {
                c.arg("--").args(ps);
            }
            c
        }),
        Effect::UnstageAll => run_quiet({
            let mut c = Command::new("git");
            c.args(["reset", "-q"]);
            let ps = repo.pathspecs();
            if !ps.is_empty() {
                c.arg("--").args(ps);
            }
            c
        }),
        Effect::PrepareRevert(it) => {
            let (files, total) = if it.group == Group::Untracked {
                let all = untracked_under(repo, &it.entry.path);
                let total = all.len();
                (all.into_iter().take(MODAL_FILES).collect(), total)
            } else {
                (vec![], 0)
            };
            app.confirm_revert(it, files, total);
            return true;
        }
        Effect::Revert(it) => revert_steps(&it).iter().try_for_each(|step| repo.run(&step.iter().map(String::as_str).collect::<Vec<_>>())),
        Effect::Commit => {
            let (ok, err) = run_on_terminal(term, &["commit".to_string()], "gw status -i");
            if ok {
                reload(app, repo);
                app.status = Some(("commit created".into(), false));
                return true;
            }
            Err(if err.is_empty() { "commit failed".into() } else { err })
        }
    };
    reload(app, repo);
    if let Err(e) = result {
        app.status = Some((e, true));
    }
    true
}

fn reload(app: &mut App, repo: &Repo) {
    match repo.load() {
        Ok((items, head)) => {
            app.head = head;
            app.replace_items(items);
        }
        Err(e) => app.status = Some((e, true)),
    }
}

fn event_loop(term: &mut DefaultTerminal, app: &mut App, repo: &Repo) -> io::Result<()> {
    let mut dirty = true;
    loop {
        if let Some(it) = app.diff_wanted().cloned() {
            let lines = load_diff(repo, &it);
            app.set_diff(&it, lines);
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
                    if !handle(app.key(k), app, repo, term) {
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

pub fn run(args: &[OsString]) -> i32 {
    // Outside a repository: git's own error and exit code.
    let top = match Command::new("git").args(["rev-parse", "--show-toplevel"]).stderr(Stdio::inherit()).output() {
        Ok(o) if o.status.success() => String::from_utf8_lossy(&o.stdout).trim_end_matches('\n').to_string(),
        Ok(o) => return exit_code(Ok(o.status)),
        Err(e) => return exit_code(Err(e)),
    };
    let prefix = Command::new("git").args(["rev-parse", "--show-prefix"]).output().map(|o| String::from_utf8_lossy(&o.stdout).trim().to_string()).unwrap_or_default();
    let repo = Repo { top, prefix: prefix.clone(), args: args.to_vec() };
    let (items, head) = match repo.load() {
        Ok(v) => v,
        Err(e) => {
            eprintln!("{e}");
            return 128;
        }
    };
    let mut app = App::new(items, prefix, head);
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
    use gw_parse::parse_status_v2;

    /// A scratch repository with one commit; `GIT_*` identity set for commits.
    fn repo() -> (tempdir::Dir, Repo) {
        let d = tempdir::Dir::new();
        let g = |args: &[&str]| assert!(Command::new("git").current_dir(&d.0).args(args).envs(ENV).status().unwrap().success(), "{args:?}");
        g(&["init", "-q", "-b", "main"]);
        std::fs::write(d.0.join("a.rs"), "one\ntwo\n").unwrap();
        std::fs::write(d.0.join("old.rs"), "x\n").unwrap();
        g(&["add", "."]);
        g(&["commit", "-q", "-m", "init"]);
        let r = Repo { top: d.0.to_string_lossy().into_owned(), prefix: String::new(), args: vec![] };
        (d, r)
    }

    const ENV: [(&str, &str); 6] = [
        ("GIT_AUTHOR_NAME", "t"),
        ("GIT_AUTHOR_EMAIL", "t@e.x"),
        ("GIT_COMMITTER_NAME", "t"),
        ("GIT_COMMITTER_EMAIL", "t@e.x"),
        ("GIT_CONFIG_GLOBAL", "/dev/null"),
        ("GIT_CONFIG_NOSYSTEM", "1"),
    ];

    mod tempdir {
        pub struct Dir(pub std::path::PathBuf);
        impl Dir {
            pub fn new() -> Self {
                static N: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);
                let n = N.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
                let p = std::env::temp_dir().join(format!("gw-stage-{}-{n}", std::process::id()));
                let _ = std::fs::remove_dir_all(&p);
                std::fs::create_dir_all(&p).unwrap();
                Dir(p.canonicalize().unwrap())
            }
        }
        impl Drop for Dir {
            fn drop(&mut self) {
                let _ = std::fs::remove_dir_all(&self.0);
            }
        }
    }

    /// Items as `(group, code, path)`, read with `git -C top status`.
    fn state(r: &Repo) -> Vec<(Group, char, String)> {
        let o = r.git(&["status", "--porcelain=v2", "-z"]).output().unwrap();
        let st = parse_status_v2(&o.stdout);
        items(&st, &LineStats::default()).into_iter().map(|i| (i.group, i.code(), i.entry.path)).collect()
    }

    fn item(r: &Repo, g: Group, path: &str) -> Item {
        let o = r.git(&["status", "--porcelain=v2", "-z"]).output().unwrap();
        items(&parse_status_v2(&o.stdout), &LineStats::default()).into_iter().find(|i| i.group == g && i.entry.path == path).unwrap()
    }

    fn revert(r: &Repo, it: &Item) {
        for step in revert_steps(it) {
            r.run(&step.iter().map(String::as_str).collect::<Vec<_>>()).unwrap();
        }
    }

    fn write(r: &Repo, p: &str, s: &str) {
        let full = Path::new(&r.top).join(p);
        std::fs::create_dir_all(full.parent().unwrap()).unwrap();
        std::fs::write(full, s).unwrap();
    }

    #[test]
    fn stage_unstage_and_literal_paths() {
        let (_d, r) = repo();
        write(&r, "f*.txt", "a\n");
        write(&r, "foo.txt", "b\n");
        r.run(&["add", "-A", "--", "f*.txt"]).unwrap();
        let s = state(&r);
        assert!(s.contains(&(Group::Staged, 'A', "f*.txt".into())), "{s:?}");
        assert!(s.contains(&(Group::Untracked, '?', "foo.txt".into())), "glob not expanded: {s:?}");
        r.run(&["reset", "-q", "--", "f*.txt"]).unwrap();
        assert!(state(&r).contains(&(Group::Untracked, '?', "f*.txt".into())));
    }

    #[test]
    fn unstage_in_empty_repo() {
        let d = tempdir::Dir::new();
        assert!(Command::new("git").current_dir(&d.0).args(["init", "-q"]).status().unwrap().success());
        let r = Repo { top: d.0.to_string_lossy().into_owned(), prefix: String::new(), args: vec![] };
        write(&r, "n.txt", "x\n");
        r.run(&["add", "-A", "--", "n.txt"]).unwrap();
        assert_eq!(state(&r), vec![(Group::Staged, 'A', "n.txt".into())]);
        r.run(&["reset", "-q", "--", "n.txt"]).unwrap();
        assert_eq!(state(&r), vec![(Group::Untracked, '?', "n.txt".into())]);
        // Staged diff before the first commit is against the empty tree.
        r.run(&["add", "-A", "--", "n.txt"]).unwrap();
        assert!(load_diff(&r, &item(&r, Group::Staged, "n.txt")).contains(&"+x".to_string()));
    }

    #[test]
    fn reverts() {
        let (_d, r) = repo();
        // Changed: work tree back to the index, staged part kept.
        write(&r, "a.rs", "one\nTWO\n");
        r.run(&["add", "a.rs"]).unwrap();
        write(&r, "a.rs", "one\nTWO\nthree\n");
        revert(&r, &item(&r, Group::Changed, "a.rs"));
        assert_eq!(std::fs::read_to_string(Path::new(&r.top).join("a.rs")).unwrap(), "one\nTWO\n");
        assert_eq!(state(&r), vec![(Group::Staged, 'M', "a.rs".into())]);
        // Staged M: back to HEAD in index and work tree.
        revert(&r, &item(&r, Group::Staged, "a.rs"));
        assert_eq!(std::fs::read_to_string(Path::new(&r.top).join("a.rs")).unwrap(), "one\ntwo\n");
        assert!(state(&r).is_empty());
        // Staged A: removed and deleted.
        write(&r, "n.rs", "n\n");
        r.run(&["add", "n.rs"]).unwrap();
        revert(&r, &item(&r, Group::Staged, "n.rs"));
        assert!(!Path::new(&r.top).join("n.rs").exists());
        assert!(state(&r).is_empty());
        // Staged R: new path gone, old path back.
        r.run(&["mv", "old.rs", "new.rs"]).unwrap();
        revert(&r, &item(&r, Group::Staged, "new.rs"));
        assert!(Path::new(&r.top).join("old.rs").exists() && !Path::new(&r.top).join("new.rs").exists());
        assert!(state(&r).is_empty());
        // Untracked file and directory.
        write(&r, "u.txt", "u\n");
        write(&r, "tmp/a", "a\n");
        write(&r, "tmp/b/c", "c\n");
        assert_eq!(untracked_under(&r, "tmp/").len(), 2);
        revert(&r, &item(&r, Group::Untracked, "u.txt"));
        revert(&r, &item(&r, Group::Untracked, "tmp/"));
        assert!(!Path::new(&r.top).join("tmp").exists() && !Path::new(&r.top).join("u.txt").exists());
        assert!(state(&r).is_empty());
    }

    #[test]
    fn diffs_per_kind() {
        let (_d, r) = repo();
        write(&r, "a.rs", "one\nTWO\n");
        r.run(&["add", "a.rs"]).unwrap();
        write(&r, "a.rs", "one\nTWO\nthree\n");
        let staged = load_diff(&r, &item(&r, Group::Staged, "a.rs"));
        let changed = load_diff(&r, &item(&r, Group::Changed, "a.rs"));
        assert!(staged.contains(&"+TWO".into()) && !staged.contains(&"+three".into()), "{staged:?}");
        assert!(changed.contains(&"+three".into()) && !changed.contains(&"+TWO".into()), "{changed:?}");
        write(&r, "n.txt", "1\n2\n3\n");
        assert_eq!(load_diff(&r, &item(&r, Group::Untracked, "n.txt")), vec!["+1", "+2", "+3"]);
        std::fs::write(Path::new(&r.top).join("bin.dat"), b"a\0b").unwrap();
        assert_eq!(load_diff(&r, &item(&r, Group::Untracked, "bin.dat")), vec!["binary file"]);
        write(&r, "d/x", "x\n");
        write(&r, "d/y", "y\n");
        assert_eq!(load_diff(&r, &item(&r, Group::Untracked, "d/")), vec!["2 untracked files:", "  d/x", "  d/y"]);
    }

    #[test]
    fn pathspecs_from_args() {
        let r = Repo { top: String::new(), prefix: String::new(), args: ["-uall", "src/", "--", "-odd"].map(OsString::from).to_vec() };
        assert_eq!(r.pathspecs(), ["src/", "-odd"].map(OsString::from).to_vec());
    }
}
