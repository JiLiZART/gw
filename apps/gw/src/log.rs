//! `gw log`: the interactive browser. State and drawing live in `gw_render::browser`;
//! this module runs git, feeds results back, and owns the terminal.

use std::{
    collections::{HashMap, VecDeque},
    ffi::OsString,
    io::{self, BufRead, BufReader, IsTerminal, Read, Write},
    process::{Child, Stdio},
    sync::mpsc::{self, Receiver, Sender},
    thread,
    time::{Duration, Instant, SystemTime, UNIX_EPOCH},
};

use gw_parse::{LOG_FORMAT, LogRow, parse_graph_line, parse_name_status_z, parse_numstat_z, parse_shortlog, parse_status_v2};
use gw_render::browser::{Action, App, Effect, FileEntry, Filters, Scope, view};
use ratatui::{
    DefaultTerminal,
    crossterm::{
        event::{self, Event, KeyEventKind},
    },
};

use crate::{passthrough, term::run_on_terminal};

/// Flags that ask for git's own output format: run plain `git log`.
const FORMAT_FLAGS: [&str; 10] = ["--oneline", "--format", "--pretty", "-p", "--patch", "--stat", "--graph", "--name-only", "--name-status", "-u"];

pub fn wants_passthrough(args: &[String]) -> bool {
    args.iter().take_while(|a| *a != "--").any(|a| {
        FORMAT_FLAGS.iter().any(|f| a == f || (f.starts_with("--") && a.starts_with(&format!("{f}="))))
    })
}

fn git(args: &[&str]) -> Option<String> {
    let o = crate::git().args(args).stderr(Stdio::null()).output().ok()?;
    o.status.success().then(|| String::from_utf8_lossy(&o.stdout).into_owned())
}

fn head_label() -> String {
    match git(&["symbolic-ref", "-q", "--short", "HEAD"]) {
        Some(b) => b.trim().to_string(),
        None => format!("detached at {}", git(&["rev-parse", "--short", "HEAD"]).unwrap_or_default().trim()),
    }
}

enum LoadMsg {
    Rows(u64, Vec<LogRow>),
    Done(u64, Option<String>),
}

/// Runs `git log --graph` in the background; a new `start` abandons the previous run.
struct Loader {
    run_id: u64,
    child: Option<Child>,
    tx: Sender<LoadMsg>,
    rx: Receiver<LoadMsg>,
}

impl Loader {
    fn new() -> Self {
        let (tx, rx) = mpsc::channel();
        Self { run_id: 0, child: None, tx, rx }
    }

    fn start(&mut self, filter_args: &[String]) {
        if let Some(mut c) = self.child.take() {
            let _ = c.kill();
            thread::spawn(move || c.wait());
        }
        self.run_id += 1;
        let run_id = self.run_id;
        let child = crate::git()
            .args(["log", "--graph", "--color=never", "--no-show-signature", "--decorate=short", LOG_FORMAT])
            .args(filter_args)
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn();
        let mut child = match child {
            Ok(c) => c,
            Err(e) => {
                let _ = self.tx.send(LoadMsg::Done(run_id, Some(e.to_string())));
                return;
            }
        };
        let (out, mut err) = (child.stdout.take().unwrap(), child.stderr.take().unwrap());
        let tx = self.tx.clone();
        thread::spawn(move || {
            let mut r = BufReader::new(out);
            let mut batch = Vec::new();
            let mut last = Instant::now();
            let mut buf = Vec::new();
            while matches!(r.read_until(b'\n', &mut buf), Ok(n) if n > 0) {
                let line = String::from_utf8_lossy(&buf);
                batch.push(parse_graph_line(line.trim_end_matches('\n')));
                buf.clear();
                // Small first batches so the first screen appears at once.
                if batch.len() >= 2000 || (last.elapsed() > Duration::from_millis(30) && !batch.is_empty()) {
                    if tx.send(LoadMsg::Rows(run_id, std::mem::take(&mut batch))).is_err() {
                        return;
                    }
                    last = Instant::now();
                }
            }
            let _ = tx.send(LoadMsg::Rows(run_id, batch));
            let mut e = String::new();
            let _ = err.read_to_string(&mut e);
            let e = e.lines().next().map(str::to_string);
            let _ = tx.send(LoadMsg::Done(run_id, e));
        });
        self.child = Some(child);
    }
}

impl Drop for Loader {
    fn drop(&mut self) {
        if let Some(mut c) = self.child.take() {
            let _ = c.kill();
            let _ = c.wait();
        }
    }
}

const DIFF_ARGS: [&str; 6] = ["show", "--format=", "--diff-merges=first-parent", "-M", "--no-ext-diff", "--no-textconv"];

/// File list with change codes and line counts for one commit (first parent for merges).
fn load_files(sha: &str) -> Vec<FileEntry> {
    let out = |extra: &[&str]| {
        crate::git().args(DIFF_ARGS).args(extra).arg(sha).stderr(Stdio::null()).output().map(|o| o.stdout).unwrap_or_default()
    };
    let names = parse_name_status_z(&out(&["--name-status", "-z"]));
    let stats = parse_numstat_z(&out(&["--numstat", "-z"]));
    names.into_iter().map(|(code, path, orig)| FileEntry { code, stat: stats.get(&path).copied(), path, orig }).collect()
}

fn load_diff(sha: &str, path: &str) -> Vec<String> {
    let o = crate::git().args(DIFF_ARGS).args(["--no-color", sha, "--", path]).stderr(Stdio::null()).output();
    String::from_utf8_lossy(&o.map(|o| o.stdout).unwrap_or_default()).lines().map(str::to_string).collect()
}

/// git never rejects a date: unknown text becomes "now". Treat that as not understood.
fn validate_date(v: &str) -> Result<(), String> {
    let v = v.trim();
    if v.is_empty() || matches!(v, "now" | "today") {
        return Ok(());
    }
    let out = git(&["rev-parse", &format!("--since={v}")]).unwrap_or_default();
    let ts: i64 = out.trim().strip_prefix("--max-age=").and_then(|t| t.parse().ok()).unwrap_or(0);
    let now = SystemTime::now().duration_since(UNIX_EPOCH).map_or(0, |d| d.as_secs() as i64);
    if (now - ts).abs() <= 2 { Err(format!("date not understood: {v}")) } else { Ok(()) }
}

/// Standard base64 for OSC 52.
pub fn base64(data: &[u8]) -> String {
    const T: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut s = String::new();
    for c in data.chunks(3) {
        let n = (c[0] as u32) << 16 | (*c.get(1).unwrap_or(&0) as u32) << 8 | *c.get(2).unwrap_or(&0) as u32;
        for i in 0..4 {
            s.push(if i <= c.len() { T[(n >> (18 - 6 * i) & 63) as usize] as char } else { '=' });
        }
    }
    s
}

/// OSC 52 "set clipboard" sequence.
pub fn osc52(text: &str) -> String {
    format!("\x1b]52;c;{}\x07", base64(text.as_bytes()))
}

/// Leaves the TUI, runs git on the real terminal (editors, conflicts work), waits for Enter, comes back.
/// stderr is also captured (tee) so a failure can be summarized in the status line.
fn action_message(action: &Action, ok: bool, err: String) -> String {
    let verb = match action {
        Action::CherryPick(_) => "cherry-pick",
        Action::Revert(_) => "revert",
        _ => "",
    };
    if ok {
        return match action {
            Action::NewBranch { name, .. } => format!("Created branch {name}"),
            Action::Tag { name, .. } => format!("Created tag {name}"),
            a => format!("✓ git {}", a.git_args().join(" ")),
        };
    }
    let head = if verb == "revert" { "REVERT_HEAD" } else { "CHERRY_PICK_HEAD" };
    if !verb.is_empty() && git(&["rev-parse", "-q", "--verify", head]).is_some() {
        return format!("{verb} stopped with conflicts: resolve them, then git {verb} --continue (or --abort)");
    }
    if err.is_empty() { "git failed".into() } else { err }
}

fn handle(effect: Effect, app: &mut App, loader: &mut Loader, term: &mut DefaultTerminal) -> bool {
    match effect {
        Effect::None | Effect::LoadFiles(_) => {}
        Effect::Quit => return false,
        Effect::Reload => {
            app.begin_load();
            loader.start(&app.filters.git_args());
        }
        Effect::LoadDiff { sha, path } => {
            let lines = load_diff(&sha, &path);
            app.set_diff(sha, path, lines);
        }
        Effect::LoadBranches => {
            let out = git(&["for-each-ref", "--format=%(refname:short)", "refs/heads", "refs/remotes"]).unwrap_or_default();
            // Names starting with '-' (possible via update-ref or a hostile remote) would act as git options.
            app.open_branches(out.lines().filter(|l| !l.ends_with("/HEAD") && *l != "origin" && !l.starts_with('-')).map(str::to_string).collect());
        }
        Effect::LoadAuthors => {
            let scope = Filters { author: None, since: None, until: None, ..app.filters.clone() };
            let mut args = vec!["shortlog".to_string(), "-sne".into()];
            args.extend(scope.git_args());
            let refs: Vec<&str> = args.iter().map(String::as_str).collect();
            app.open_authors(parse_shortlog(&git(&refs).unwrap_or_default()));
        }
        Effect::ValidateDate { since, until } => match validate_date(&since).and(validate_date(&until)) {
            Ok(()) => {
                app.apply_date(since, until);
                return handle(Effect::Reload, app, loader, term);
            }
            Err(e) => app.date_error(e),
        },
        Effect::PrepareReset(mode, sha) => {
            let branch = git(&["symbolic-ref", "-q", "--short", "HEAD"]).map_or("detached HEAD".into(), |b| b.trim().to_string());
            let out = crate::git().args(["status", "--porcelain=v2", "-z", "--untracked-files=no"]).output().map(|o| o.stdout).unwrap_or_default();
            let at_risk = parse_status_v2(&out).entries.into_iter().map(|e| e.path).collect();
            app.confirm_reset(mode, sha, branch, at_risk);
        }
        Effect::Run(action) => {
            let args = action.git_args();
            let (ok, err) = if action.needs_terminal() {
                run_on_terminal(term, &args, "gw log")
            } else {
                match crate::git().args(&args).output() {
                    Ok(o) => (o.status.success(), String::from_utf8_lossy(&o.stderr).lines().next().unwrap_or("").to_string()),
                    Err(e) => (false, e.to_string()),
                }
            };
            let msg = action_message(&action, ok, err);
            app.action_result(ok, msg);
            if ok || action.needs_terminal() {
                app.head = head_label();
                return handle(Effect::Reload, app, loader, term);
            }
        }
        Effect::Copy(sha) => {
            let _ = io::stdout().write_all(osc52(&sha).as_bytes());
            let _ = io::stdout().flush();
        }
    }
    true
}

fn event_loop(term: &mut DefaultTerminal, app: &mut App) -> io::Result<()> {
    let mut loader = Loader::new();
    loader.start(&app.filters.git_args());
    let mut cache: HashMap<String, Vec<FileEntry>> = HashMap::new();
    let mut order: VecDeque<String> = VecDeque::new();
    let mut last_sel: Option<String> = None;
    let mut sel_changed = Instant::now();
    let mut dirty = true;
    #[cfg(debug_assertions)]
    let panic_test = std::env::var_os("GW_PANIC_TEST").is_some();
    loop {
        if dirty {
            let size = term.size()?;
            app.resize(size.width, size.height);
            term.draw(|f| view::draw(f, app))?;
            dirty = false;
            #[cfg(debug_assertions)]
            if panic_test {
                panic!("GW_PANIC_TEST");
            }
        }
        // Debounced file list for the selected commit.
        let sel = app.selected_hash().map(str::to_string);
        if sel != last_sel {
            last_sel = sel;
            sel_changed = Instant::now();
        }
        if let Some(sha) = app.files_wanted().map(str::to_string) {
            if let Some(files) = cache.get(&sha) {
                app.set_files(sha, files.clone());
                dirty = true;
            } else if sel_changed.elapsed() >= Duration::from_millis(50) {
                let files = load_files(&sha);
                if order.len() >= 256
                    && let Some(old) = order.pop_front()
                {
                    cache.remove(&old);
                }
                order.push_back(sha.clone());
                cache.insert(sha.clone(), files.clone());
                app.set_files(sha, files);
                dirty = true;
            }
        }
        if event::poll(Duration::from_millis(30))? {
            match event::read()? {
                Event::Key(k) if k.kind != KeyEventKind::Release => {
                    let effect = app.key(k);
                    if !handle(effect, app, &mut loader, term) {
                        return Ok(());
                    }
                    dirty = true;
                }
                Event::Resize(..) => dirty = true,
                _ => {}
            }
        }
        while let Ok(msg) = loader.rx.try_recv() {
            match msg {
                LoadMsg::Rows(g, rows) if g == loader.run_id => {
                    app.push_rows(rows);
                    dirty = true;
                }
                LoadMsg::Done(g, err) if g == loader.run_id => {
                    app.finish_load();
                    if let (Some(e), true) = (err, app.rows.is_empty()) {
                        app.status = Some((e, true));
                    }
                    dirty = true;
                }
                _ => {}
            }
        }
    }
}

pub fn run(args: &[OsString]) -> i32 {
    let user: Vec<String> = args.iter().map(|a| a.to_string_lossy().into_owned()).collect();
    if !io::stdout().is_terminal() || wants_passthrough(&user) || git(&["rev-parse", "--git-dir"]).is_none() {
        let mut all: Vec<OsString> = vec!["log".into()];
        all.extend(args.iter().cloned());
        return passthrough(&all);
    }
    let remotes = git(&["remote"]).unwrap_or_default().lines().map(str::to_string).collect();
    let now = SystemTime::now().duration_since(UNIX_EPOCH).map_or(0, |d| d.as_secs() as i64);
    let mut app = App::new(Filters::new(user), head_label(), remotes, now);
    debug_assert_eq!(app.filters.scope, Scope::Head);
    // ratatui::init: raw mode + alternate screen + a panic hook that restores the terminal.
    let mut term = ratatui::init();
    let res = event_loop(&mut term, &mut app);
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

    #[test]
    fn osc52_bytes() {
        assert_eq!(osc52("a22b7ed"), "\x1b]52;c;YTIyYjdlZA==\x07");
        assert_eq!(base64(b"abc"), "YWJj");
        assert_eq!(base64(b"ab"), "YWI=");
    }

    #[test]
    fn passthrough_flags() {
        let s = |v: &[&str]| v.iter().map(|x| x.to_string()).collect::<Vec<_>>();
        assert!(wants_passthrough(&s(&["--oneline"])));
        assert!(wants_passthrough(&s(&["--format=%h"])));
        assert!(!wants_passthrough(&s(&["-n", "5", "--", "--stat"])), "after -- it is a path");
        assert!(!wants_passthrough(&s(&["main..feat"])));
    }
}
