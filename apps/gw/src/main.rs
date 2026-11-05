//! `gw`: runs git and renders its output in a structured, colored form.

use std::{
    ffi::OsString,
    io::{self, IsTerminal, Read, Write},
    process::{Command, ExitStatus, Stdio, exit},
    sync::mpsc,
    thread,
    time::{Duration, Instant},
};

use gw_parse::{
    Event, Progress, PullLine, RefUpdate, Splitter, classify, classify_pull_line, is_pack_stats, parse_fetch_porcelain, parse_numstat_z,
    parse_push_porcelain, parse_status_v2, parse_submodule_status,
};
use gw_render::{
    ThrobberState,
    bar::{Animator, ProgressLine},
    refs::{RefGroup, all_up_to_date, render_messages, render_pull_summary, render_ref_groups, render_up_to_date},
    remote_box::render_box,
    status::{LineStats, relative, render_status},
    style::no_color_env,
};
use ratatui::{Terminal, TerminalOptions, Viewport, backend::CrosstermBackend, layout::Position};

mod log;
mod views;

const FORMATTED: [&str; 10] = ["push", "pull", "fetch", "clone", "merge", "status", "log", "branch", "commit", "stash"];
const TICK: Duration = Duration::from_millis(80);
/// Redraw interval while easing the bar.
const FRAME: Duration = Duration::from_millis(30);
/// Longest the bar keeps easing after git exits successfully.
const SETTLE_MAX: Duration = Duration::from_secs(1);
/// No progress for this long: show `Connecting…` so slow handshakes are not a blank wait.
const CONNECT_DELAY: Duration = Duration::from_millis(300);

fn main() {
    let args: Vec<OsString> = std::env::args_os().skip(1).collect();
    let Some(cmd) = args.first().map(|a| a.to_string_lossy().into_owned()) else {
        eprintln!("usage: gw <push|pull|fetch|clone|merge|status|log|branch|commit|stash list> [git args...]\nAny other subcommand runs plain git.");
        exit(1);
    };
    let code = if !FORMATTED.contains(&cmd.as_str()) || !io::stdout().is_terminal() {
        passthrough(&args)
    } else {
        match cmd.as_str() {
            "status" => status(&args[1..]),
            "log" => log::run(&args[1..]),
            "branch" => views::branch(&args[1..]),
            "commit" => views::commit(&args[1..]),
            "stash" => views::stash(&args[1..]),
            // These open an editor or print nothing worth formatting.
            "merge" if args[1..].iter().any(|a| matches!(a.to_str(), Some("-e" | "--edit" | "--continue" | "--abort" | "--quit"))) => passthrough(&args),
            _ => network(&cmd, &args[1..]),
        }
    };
    exit(code)
}

fn exit_code(r: io::Result<ExitStatus>) -> i32 {
    match r {
        Ok(s) => s.code().unwrap_or_else(|| {
            #[cfg(unix)]
            {
                use std::os::unix::process::ExitStatusExt;
                128 + s.signal().unwrap_or(0)
            }
            #[cfg(not(unix))]
            1
        }),
        Err(e) if e.kind() == io::ErrorKind::NotFound => {
            eprintln!("gw: git not found in PATH");
            127
        }
        Err(e) => {
            eprintln!("gw: failed to run git: {e}");
            1
        }
    }
}

fn passthrough(args: &[OsString]) -> i32 {
    exit_code(Command::new("git").args(args).status())
}

/// Ctrl-C reaches git through the process group; gw keeps running to finalize the screen.
/// Called after spawning, since an ignored disposition is inherited across exec.
#[cfg(unix)]
fn ignore_sigint() {
    unsafe extern "C" {
        fn signal(sig: i32, handler: usize) -> usize;
    }
    const SIGINT: i32 = 2;
    const SIG_IGN: usize = 1;
    unsafe { signal(SIGINT, SIG_IGN) };
}
#[cfg(not(unix))]
fn ignore_sigint() {}

fn term_width() -> usize {
    match ratatui::crossterm::terminal::size() {
        Ok((w, _)) if w > 0 => w as usize,
        _ => 80,
    }
}

type Live = Terminal<CrosstermBackend<io::Stdout>>;

fn draw(term: &mut Live, p: &Progress, fill: Option<u8>, th: &ThrobberState, done: Option<bool>, color: bool) {
    let _ = term.draw(|f| {
        let a = f.area();
        f.render_widget(ProgressLine { progress: p, fill, throbber: th, done, color }, a);
        // Keep the cursor visible (parked at line end) so an interrupted run never leaves it hidden.
        f.set_cursor_position(Position::new(a.x + a.width.saturating_sub(1), a.y));
    });
}

fn network(cmd: &str, rest: &[OsString]) -> i32 {
    let mut git_args: Vec<OsString> = vec![cmd.into(), "--progress".into()];
    // git rejects `fetch --porcelain --recurse-submodules`; then the human stderr ref lines are parsed instead.
    let recurse = rest.iter().any(|a| {
        let a = a.to_string_lossy();
        a == "--recurse-submodules" || a.strip_prefix("--recurse-submodules=").is_some_and(|v| v != "no")
    });
    if cmd == "push" || (cmd == "fetch" && !recurse) {
        git_args.push("--porcelain".into());
    }
    git_args.extend(rest.iter().cloned());
    let child = Command::new("git")
        .args(&git_args)
        .env("LC_ALL", "C")
        .stdin(Stdio::inherit())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn();
    let mut child = match child {
        Ok(c) => c,
        Err(e) => return exit_code(Err(e)),
    };
    ignore_sigint();

    // (is_stderr, line)
    let (tx, rx) = mpsc::channel::<(bool, String)>();
    let readers: [(bool, Box<dyn Read + Send>); 2] =
        [(false, Box::new(child.stdout.take().unwrap())), (true, Box::new(child.stderr.take().unwrap()))];
    for (is_err, mut r) in readers {
        let tx = tx.clone();
        thread::spawn(move || {
            let mut sp = Splitter::default();
            let mut buf = [0u8; 8192];
            while let Ok(n @ 1..) = r.read(&mut buf) {
                for l in sp.push(&buf[..n]) {
                    let _ = tx.send((is_err, l));
                }
            }
            if let Some(l) = sp.finish() {
                let _ = tx.send((is_err, l));
            }
        });
    }
    drop(tx);

    let color = !no_color_env();
    let mut th = ThrobberState::default();
    let mut anim = Animator::new(Instant::now());
    let mut term: Option<Live> = None;
    let mut term_tried = false;
    // True once git sent real progress; a `Connecting…`-only line is cleared at the end.
    let mut real_progress = false;
    let started = Instant::now();
    let start_live = || Terminal::with_options(CrosstermBackend::new(io::stdout()), TerminalOptions { viewport: Viewport::Inline(1) }).ok();
    let mut last_tick = Instant::now();
    let (mut remote, mut others, mut stdout_lines) = (Vec::new(), Vec::new(), Vec::new());
    // One group per fetched repository; `Fetching submodule <p>` labels the next `From`.
    let mut groups: Vec<RefGroup> = Vec::new();
    let mut pending_sub: Option<String> = None;
    // Recursive clone: after `registered for path`, its `Cloning into`/`done.` lines are submodule noise.
    let mut sub_clone = false;

    loop {
        match rx.recv_timeout(FRAME) {
            Ok((false, line)) => stdout_lines.push(line),
            Ok((true, line)) => match classify(&line) {
                Event::Progress(p) => {
                    anim.push(p, Instant::now());
                    real_progress = true;
                    if !term_tried {
                        term_tried = true;
                        term = start_live();
                    }
                }
                Event::Remote(r) => remote.push(r),
                Event::Target(t) => groups.push(RefGroup { submodule: pending_sub.take(), target: Some(t), refs: vec![] }),
                Event::RefUpdate(r) => add_ref(&mut groups, r),
                Event::SubmoduleFetch(p) => pending_sub = Some(p),
                Event::UpToDate => {}
                Event::Other(l) if is_pack_stats(&l) => {}
                Event::Other(l) if classify_pull_line(&l) == PullLine::SubmoduleNoise => sub_clone = true,
                Event::Other(l) if sub_clone && (l.starts_with("Cloning into ") || l == "done.") => {}
                Event::Other(l) => others.push(l),
            },
            Err(mpsc::RecvTimeoutError::Timeout) => {}
            Err(mpsc::RecvTimeoutError::Disconnected) => break,
        }
        // Delayed start: fast commands (up-to-date push) never draw a live line.
        // Prompts go to /dev/tty, unseen by gw; one appearing after the delay may be overdrawn (accepted risk).
        if !term_tried && cmd != "merge" && started.elapsed() >= CONNECT_DELAY {
            term_tried = true;
            anim.push(Progress { label: "Connecting…".into(), ..Default::default() }, Instant::now());
            term = start_live();
        }
        if last_tick.elapsed() >= TICK {
            th.calc_next();
            last_tick = Instant::now();
        }
        anim.tick(Instant::now());
        if let (Some(t), Some((p, fill))) = (term.as_mut(), anim.current()) {
            draw(t, p, Some(fill), &th, None, color);
        }
    }
    let code = exit_code(child.wait());

    // Let fast stages play out (bounded); a failure shows its real state at once.
    if code == 0 && real_progress && term.is_some() {
        let deadline = Instant::now() + SETTLE_MAX;
        while !anim.settled() && Instant::now() < deadline {
            thread::sleep(FRAME);
            if last_tick.elapsed() >= TICK {
                th.calc_next();
                last_tick = Instant::now();
            }
            anim.tick(Instant::now());
            if let (Some(t), Some((p, fill))) = (term.as_mut(), anim.current()) {
                draw(t, p, Some(fill), &th, None, color);
            }
        }
    }

    let mut out = io::stdout().lock();
    if let (Some(mut t), Some(p)) = (term.take(), anim.latest()) {
        if real_progress {
            draw(&mut t, p, None, &th, Some(code == 0), color);
            drop(t);
            let _ = write!(out, "\r\n");
        } else {
            // Only `Connecting…` was shown: erase it so e.g. "Everything up-to-date" stands alone.
            let _ = t.clear();
            drop(t);
            // The cursor is parked at the line end (see `draw`): return to column 1.
            let _ = write!(out, "\r");
        }
    }

    match cmd {
        "push" => {
            for e in parse_push_porcelain(&stdout_lines.join("\n")) {
                match e {
                    Event::Target(t) => groups.push(RefGroup { target: Some(t), ..Default::default() }),
                    Event::RefUpdate(r) => add_ref(&mut groups, r),
                    _ => {}
                }
            }
        }
        "fetch" => parse_fetch_porcelain(&stdout_lines.join("\n")).into_iter().for_each(|r| add_ref(&mut groups, r)),
        _ => {}
    }

    let refs: Vec<RefUpdate> = groups.iter().flat_map(|g| g.refs.iter().cloned()).collect();
    let mut branches: Vec<String> = Vec::new();
    for r in &refs {
        branches.extend([r.src.clone(), r.dst.clone()]);
        if let Some((_, b)) = r.dst.split_once('/') {
            branches.push(b.to_string());
        }
    }
    let _ = write!(out, "{}", render_box(&remote, &branches, term_width(), color));

    let nothing = all_up_to_date(&refs);
    match cmd {
        "push" | "fetch" if nothing && code == 0 => {
            let _ = write!(out, "{}", render_up_to_date(color));
        }
        "pull" if nothing => {}
        _ if refs.is_empty() => {}
        _ => {
            let _ = write!(out, "{}", render_ref_groups(&groups, color));
        }
    }
    if matches!(cmd, "pull" | "clone" | "merge") {
        let _ = write!(out, "{}", render_pull_summary(&stdout_lines, color));
    }
    let _ = out.flush();
    drop(out);

    let _ = write!(io::stderr().lock(), "{}", render_messages(&others, color));
    code
}

/// Appends to the last repository's group (a target-less one if none yet).
fn add_ref(groups: &mut Vec<RefGroup>, r: RefUpdate) {
    if groups.is_empty() {
        groups.push(RefGroup::default());
    }
    groups.last_mut().unwrap().refs.push(r);
}

fn status(rest: &[OsString]) -> i32 {
    let format_flag = rest.iter().any(|a| {
        let a = a.to_string_lossy();
        matches!(a.as_ref(), "-s" | "--short" | "--long" | "-z" | "--null") || a.starts_with("--porcelain")
    });
    if format_flag {
        let mut args: Vec<OsString> = vec!["status".into()];
        args.extend(rest.iter().cloned());
        return passthrough(&args);
    }
    let out = Command::new("git").args(["status", "--porcelain=v2", "--branch", "-z"]).args(rest).output();
    let out = match out {
        Ok(o) if o.status.success() => o,
        Ok(o) => {
            let _ = io::stderr().write_all(&o.stderr);
            return exit_code(Ok(o.status));
        }
        Err(e) => return exit_code(Err(e)),
    };
    let _ = io::stderr().write_all(&out.stderr);
    let prefix = Command::new("git")
        .args(["rev-parse", "--show-prefix"])
        .output()
        .map(|o| String::from_utf8_lossy(&o.stdout).trim().to_string())
        .unwrap_or_default();
    let st = parse_status_v2(&out.stdout);
    let numstat = |cached: bool| {
        let mut c = Command::new("git");
        c.args(["diff", "--numstat", "-z", "--no-ext-diff", "--no-textconv"]);
        if cached {
            c.arg("--cached");
        }
        c.output().map(|o| parse_numstat_z(&o.stdout)).unwrap_or_default()
    };
    let mut stats = LineStats { staged: numstat(true), unstaged: numstat(false), ..Default::default() };
    for e in st.entries.iter().filter(|e| e.x == '?' && !e.path.ends_with('/')) {
        stats.untracked.insert(e.path.clone(), count_lines(&relative(&e.path, &prefix)));
    }
    // Checked-out commits are only needed when a submodule moved off its recorded commit.
    let moved: Vec<&str> = st.entries.iter().filter(|e| e.submodule.as_ref().is_some_and(|s| s.commit_changed)).map(|e| e.path.as_str()).collect();
    if !moved.is_empty()
        && let Ok(top) = Command::new("git").args(["rev-parse", "--show-toplevel"]).output()
        && let Ok(o) = Command::new("git")
            .arg("-C")
            .arg(String::from_utf8_lossy(&top.stdout).trim())
            .args(["submodule", "status", "--"])
            .args(&moved)
            .stderr(Stdio::null())
            .output()
    {
        stats.checkouts = parse_submodule_status(&String::from_utf8_lossy(&o.stdout));
    }
    print!("{}", render_status(&st, &prefix, &stats, !no_color_env()));
    0
}

/// New-file line count for an untracked file; `None` (shown as `bin`) when it contains NUL.
fn count_lines(path: &str) -> Option<(u64, u64)> {
    let bytes = std::fs::read(path).ok()?;
    if bytes.contains(&0) {
        return None;
    }
    let n = bytes.iter().filter(|&&b| b == b'\n').count() + usize::from(bytes.last().is_some_and(|&b| b != b'\n'));
    Some((n as u64, 0))
}
