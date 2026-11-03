## Why

Git's network commands (`push`, `pull`, `fetch`, `clone`) print flat, uncolored text: progress noise, server messages prefixed with `remote:`, and ref updates that are hard to scan. Important things — like the merge-request URL GitLab prints after a push — get lost in the wall of text. `git status` is colorable but verbose and hint-heavy. We want a Rust CLI, `gw`, that runs these git commands and renders their output in a structured, colored, scannable form.

## What Changes

- New Rust Cargo workspace with `apps/` and `crates/` folders.
- New binary `gw` that wraps `git push`, `git pull`, `git fetch`, `git clone`, and `git status`, passing all user arguments through to the real `git`.
- Progress output replaced by a single live line: a ratatui throbber followed by a green-filled bar containing the stage label, `cur/total` and percentage; it refills per git stage and ends with a success/failure mark.
- All `remote:` server messages collected (prefix stripped) and rendered in a thin-bordered box, with a host-aware title ("Merge request", "Pull request", or "remote").
- URLs rendered as clickable OSC 8 hyperlinks; branch names and SHAs highlighted.
- Ref updates (`old..new src -> dst`) rendered as a colored list grouped under the remote URL.
- `gw status` renders branch/upstream with ahead/behind counts and files grouped into Staged / Changed / Untracked.
- Raw passthrough when stdout is not a TTY; `NO_COLOR` respected; git's exit code always propagated.
- Documented usage through git aliases under new names (e.g. `git p` → `gw push`), since git ignores aliases that shadow built-in commands.

## Capabilities

### New Capabilities
- `progress-display`: live throbber + green filled progress bar for git's local and remote progress stages.
- `remote-box`: collecting `remote:` messages into a titled thin-bordered box with clickable links and highlighted branches.
- `ref-updates`: parsing and colored rendering of pushed/fetched/pulled ref updates and the target remote.
- `status-view`: structured, grouped, colored rendering of `git status`.
- `passthrough`: command dispatch, argument passthrough, non-TTY / `NO_COLOR` behavior, exit code propagation, and alias usage.

### Modified Capabilities
<!-- none: greenfield project -->

## Impact

- New code: `Cargo.toml` (workspace), `apps/gw`, `crates/gw-parse`, `crates/gw-render`.
- New dependencies: `ratatui` (with `crossterm`), `throbber-widgets-tui`; dev: `insta` for snapshot tests.
- Runtime dependency on a `git` binary in `PATH` (porcelain v2 status and `--porcelain` push/fetch require git ≥ 2.41).
- No effect on the user's git configuration; aliases are opt-in and documented, not installed.
