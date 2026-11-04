## Context

Greenfield repo. Motivation and scope: see `proposal.md`; behavior contracts: see `specs/`.

Facts about git that shape the design:

- `push`/`fetch`/`pull`/`clone` write progress, `remote:` sideband text, and the human ref summary to **stderr**. Progress stages redraw with `\r`, not `\n`.
- git prints progress only when stderr is a TTY, unless `--progress` is passed.
- `git push --porcelain` writes ref results to stdout as `<flag>\t<src>:<dst>\t<summary>`, preceded by `To <url>` and followed by `Done`. `git fetch --porcelain` (git ≥ 2.41) writes `<flag> <old> <new> <ref>`. `pull` has no porcelain mode.
- `git status --porcelain=v2 --branch -z` is stable machine output, but its paths are **repo-root relative**.
- Human messages are localized; parsing them under a non-English locale would break.
- Credential/SSH prompts are read from `/dev/tty` by git and ssh, so piped stdio does not break them, *unless* our terminal drawing overwrites the prompt.

## Goals / Non-Goals

**Goals:**
- Parsing is pure and snapshot-testable from captured git output, independent of a terminal.
- Zero behavior change for non-TTY use (scripts, CI, pipes).

**Non-Goals:**
- A `git` shim on PATH that intercepts plain `git push`.
- Formatting `diff`/`log`/`show` (existing pagers such as delta cover these).
- An alias installer command; aliases are documented only.
- Configuration files or themes; colors are fixed constants.
- Windows support beyond "it compiles"; target is macOS/Linux terminals.

## Decisions

### D1. Workspace layout: one app, two crates
```
Cargo.toml            [workspace] members = ["apps/*", "crates/*"]
apps/gw/              binary: dispatch, spawn git, event loop, exit code
crates/gw-parse/      no deps: bytes -> Event, porcelain parsers
crates/gw-render/     ratatui gauge widget + static ANSI output (box, refs, status)
```
Parsing has no IO or terminal deps so it is tested with captured fixtures. Rendering is separate so the parser never depends on ratatui. *Alternative:* a single crate. Rejected because the user asked for an `apps/` + `crates/` monorepo, and this two-crate split is the smallest one that isolates the pure parser. No further splitting.

### D2. Spawn git with pipes, not a PTY
On the formatted path, `gw` spawns `git` with stdout and stderr piped, stdin inherited, and injects:
- `--progress` (push/fetch/pull/clone) so progress survives piping,
- `--porcelain` for `push` and `fetch`, so ref results are machine-readable,
- `LC_ALL=C` in the child env so human lines (progress labels, `remote:`, pull summary) are parseable.

*Alternative:* run git in a PTY (`portable-pty`). That preserves git's TTY behavior but merges stdout and stderr, adds a dependency, and complicates exit and signal handling. Rejected: the flags give us everything we need.

`pull` and `clone` have no porcelain mode. Their ref lines (`   old..new  src -> dst`, `From <url>`, `* [new branch] ...`) are parsed from the C-locale human output. Unrecognized lines go through as `Event::Other`.

### D3. Event model (gw-parse)
A streaming splitter yields a line on either `\r` or `\n`, decoded as lossy UTF-8. Each line is classified into:
```
Progress { remote: bool, label, cur: Option<u64>, total: Option<u64>, pct: Option<u8>, tail }
Remote(String)                // "remote: " stripped, non-progress
Target(String)                // "To <url>" / "From <url>"
RefUpdate { kind, old, new, src, dst, reason }
UpToDate | Other(String)
```
`kind` is one of `FastForward | Forced | New | Deleted | Rejected | UpToDate`. Parsers: `parse_push_porcelain`, `parse_fetch_porcelain`, `parse_human_ref_line`, and `parse_status_v2(bytes) -> Status`. All are pure functions tested with `insta` snapshots of real captured output in `crates/gw-parse/tests/fixtures/`.

A `remote:` line is classified as progress when its stripped body matches `<Label>: <pct>% (<cur>/<total>)` or ends with `, done.`. Otherwise it is remote text (spec `remote-box`).

Two special cases: pack statistics lines (`Total N (delta M), reused ...`, local or `remote:`) are dropped, since they are neither progress nor a server message. `Delta compression using up to N threads` is a progress stage without a percentage. Other unrecognized stderr lines are printed unchanged to stderr after all formatted sections.

### D4. Event loop: reader threads + channel + inline viewport
```
git stdout --> thread --+
git stderr --> thread --+--> mpsc::Sender<Event>
main: loop { recv_timeout(80ms) -> update state | tick spinner; draw }
on child exit: drain channel, final frame (check/cross), drop terminal,
               print static sections, exit(code)
```
- ratatui `Terminal` with `Viewport::Inline(1)` on the **stdout** backend, **without raw mode**. Raw mode would break typed SSH/credential prompts, and the live line needs no key input.
- **Delayed start:** the live line is not drawn until the first `Progress` event, or until 300 ms pass with no progress, in which case it shows `Connecting…` with an empty bar. A short delay keeps fast commands (up-to-date push) free of a live line. Prompts are written to `/dev/tty`, so `gw` cannot see them; a prompt that appears after 300 ms may be partly overdrawn (see Risks).
- **Smooth fill:** a pure `Animator` in gw-render holds the displayed stage, its eased fill (400 %/s), and a queue of later stages (consecutive updates of one stage are merged). A stage advances once its fill reaches git's percentage and it has been shown 120 ms. After a successful exit, the loop keeps ticking until the animator settles, capped at 1 s; after a failure it draws the last real state at once.
- The spinner uses `throbber-widgets-tui` (`ThrobberState::calc_next` on each tick).
- The bar is a small custom widget: ratatui `Gauge` centers its label, but the spec needs the label left-aligned and `cur/total pct%` right-aligned. It computes `fill = width * pct / 100`, writes the text into the cells, and sets each cell's bg to 256-color dark green (28) if `x < fill`, else charcoal (237), with bold white / light gray text. Fixed indexed colors avoid theme-dependent ANSI green and black, which proved unreadable. In `NO_COLOR` mode it renders `#`/`.` instead.

### D5. Static output bypasses ratatui
The remote box, target/ref list, pull summary, and status are printed after the viewport is dropped, as plain strings styled with `crossterm::style` (re-exported by ratatui, so no extra dependency). Reason: OSC 8 hyperlink escapes inside ratatui buffer cells corrupt width calculation (a known ratatui limitation). Static content does not need a diffing renderer anyway. Ordering is fixed: progress line, then box, then target and refs, then pull summary.

Box drawing: thin `┌─┐│└┘` borders, title inlined into the top border (`┌ Merge request ───┐`). Width is `min(terminal width, longest line + 4)`, and display width is measured with `unicode-width` on the visible text, excluding escapes. URLs are found by scanning whitespace-separated tokens that start with `http://` or `https://` (trailing `.,)` trimmed) and emitted as `ESC]8;;URL ESC\ text ESC]8;; ESC\`, with visible text truncated with `…` to fit.

### D6. Status uses porcelain v2 + prefix
`gw status [args]` runs `git status --porcelain=v2 --branch -z [args]` and `git rev-parse --show-prefix`. It then rewrites repo-relative paths to be cwd-relative (`../` for paths outside the prefix). User-supplied pathspec args are forwarded. Untracked directory entries (`dir/`) keep their trailing slash after rewriting.

Line stats: `git diff --cached --numstat -z` (Staged) and `git diff --numstat -z` (Changed), parsed by a pure `parse_numstat_z` in gw-parse. Untracked files are read by `gw` and every line counts as added; a file containing NUL counts as binary (`bin`). The renderer aligns stats per group and prints `Total +X -Y`, the sum of all shown counts. Flags that change the output format (`-s`, `--porcelain`, `--long`) make `gw` fall back to plain `git status` passthrough.

### D7. Dispatch and passthrough (apps/gw)
`std::env::args` is matched by hand (no clap): the first arg is the subcommand, and the rest are forwarded verbatim. Non-TTY stdout or an unknown subcommand leads to `Command::new("git").args(..).status()` with inherited stdio, exiting with the same code. Exit code: `status.code()`, or `128 + signal` via `ExitStatusExt::signal()`. A spawn error of kind NotFound exits with 127.

### D8. Untrusted text is sanitized
File names from `-z` status output are unquoted, and `remote:` text comes from the server. Both can carry escape sequences. A single `sanitize` helper in gw-render shows control characters (C0, DEL, C1) as `\xNN` and tabs as a space. It runs on status paths and branch names, and on box lines before link detection, so a link can never contain ESC or BEL. Ref names are not sanitized: git forbids control characters in them.

## Risks / Trade-offs

- [`LC_ALL=C` also changes the locale of hooks and server-side text relayed back] → It only affects message language and byte classification. Bytes pass through as lossy UTF-8, so non-ASCII server text still renders.
- [Human output of `pull`/`clone` changes across git versions] → Unrecognized lines go through as `Other` unchanged, never dropped. Fixtures pin known formats.
- [Ctrl-C while the live line is drawn may leave the cursor hidden or the line half-drawn] → Never hide the cursor; on any exit path, print `\r\n` after the viewport. SIGINT reaches git via the process group, and `gw` then sees git exit non-zero and finalizes normally.
- [stdout/stderr ordering is not preserved across the two pipes] → Static sections are printed only after exit in a fixed order, so interleaving does not matter.
- [`--porcelain` push prints `Done` and changes stdout for hooks that expect human output] → `pre-push` hooks receive refs on stdin regardless of the flag, so they are unaffected.
- [Terminals without OSC 8 support show the link text only] → The link is still visible and selectable. Acceptable.
- [A spinner without raw mode and a prompt appearing *after* the live line starts could collide. With the 300 ms `Connecting…` line this includes slow credential or SSH passphrase prompts] → Accepted (user decision). Revisit if it shows up in practice.
- [Line stats add two `git diff` runs and read every untracked file] → May slow `gw status` on large repos or with large untracked files. Accepted.

## Migration Plan

New tool, nothing to migrate. Install with `cargo install --path apps/gw`. Rollback means uninstalling the binary or removing the aliases.
