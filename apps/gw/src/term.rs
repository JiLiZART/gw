//! Handing the terminal from a full-screen view to git and back.

use std::{
    io::{self, BufRead, Read, Write},
    process::Stdio,
    thread,
};

use ratatui::{
    DefaultTerminal,
    crossterm::{
        execute,
        terminal::{EnterAlternateScreen, LeaveAlternateScreen, disable_raw_mode, enable_raw_mode},
    },
};

/// Leaves the full-screen UI, runs `git <args>` on the real terminal (editor, prompts, hooks),
/// waits for Enter, and comes back. Returns success and git's first stderr line.
pub fn run_on_terminal(term: &mut DefaultTerminal, args: &[String], screen: &str) -> (bool, String) {
    let _ = disable_raw_mode();
    let _ = execute!(io::stdout(), LeaveAlternateScreen);
    println!("\n$ git {}", args.join(" "));
    let child = crate::git().args(args).stdin(Stdio::inherit()).stdout(Stdio::inherit()).stderr(Stdio::piped()).spawn();
    let (ok, first_err) = match child {
        Ok(mut c) => {
            let mut err = c.stderr.take().unwrap();
            let tee = thread::spawn(move || {
                let mut all = Vec::new();
                let mut buf = [0u8; 4096];
                while let Ok(n @ 1..) = err.read(&mut buf) {
                    let _ = io::stderr().write_all(&buf[..n]);
                    all.extend_from_slice(&buf[..n]);
                }
                String::from_utf8_lossy(&all).lines().find(|l| !l.trim().is_empty()).unwrap_or("").to_string()
            });
            let ok = c.wait().is_ok_and(|s| s.success());
            (ok, tee.join().unwrap_or_default())
        }
        Err(e) => (false, e.to_string()),
    };
    print!("\nPress Enter to return to {screen} ");
    let _ = io::stdout().flush();
    let _ = io::stdin().lock().read_line(&mut String::new());
    let _ = enable_raw_mode();
    let _ = execute!(io::stdout(), EnterAlternateScreen);
    let _ = term.clear();
    (ok, first_err)
}
