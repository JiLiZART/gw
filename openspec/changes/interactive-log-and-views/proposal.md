## Why

`git log` is a flat stream: finding a commit by author or date, seeing which files it touched, and then acting on it (cherry-pick, revert, tag) takes several commands and copy-pasted hashes. `gw` already makes push/pull/status scannable. The history needs an interactive browser in the same style, and a few more everyday commands (`branch`, `commit`, `stash list`) still print git's dense default output.

## What Changes

- New `gw log`: a full-screen, keyboard-driven commit browser.
  - Commit list with a colored branch graph on the left; refs shown as chips.
  - Right sidebar with the selected commit's changed files and `+N -M` counts; `Tab` moves focus to it, and it scrolls.
  - `Enter` on a file opens a scrollable colored diff pane; `Esc` returns.
  - A hotkey bar along the bottom, and an action panel that opens at the bottom (nano-style).
  - Branch filter: current branch by default, a toggle for all branches, and a branch-select modal.
  - Fuzzy search (hotkey) over hash, subject, author, and refs.
  - Date filter modal (since / until inputs) and author modal (list of known authors plus free input).
  - Actions on the selected commit: cherry-pick, revert, checkout (detached), new branch here, reset (soft / mixed / hard), copy hash, create tag (name entered in a modal). Anything that rewrites history or can lose work asks for confirmation first.
- New `gw branch` (no arguments): local branches with upstream, ahead/behind, last commit age, and `gone` upstreams flagged.
- New `gw commit`: runs `git commit` as usual (editor, hooks) and then prints a compact summary: SHA, branch, subject, `+N -M, K files`.
- New `gw stash list`: stashes with age, source branch, and file/line counts.
- These commands fall back to plain git when stdout is not a TTY, or when arguments ask for git's own output format or a different operation.
- Future ideas, not in this change: `gw switch`/`checkout` one-line summaries, `gw merge`/`rebase` conflict summaries, `gw tag` list, `gw remote -v`.

## Capabilities

### New Capabilities
- `log-browser`: the interactive `gw log` screen, including the graph list, file sidebar, diff pane, hotkey bar, focus, and scrolling.
- `log-filters`: branch scope toggle and select, fuzzy search, and the date and author filter modals.
- `log-actions`: the bottom action panel and the commit actions with their confirmations.
- `branch-view`: the `gw branch` overview.
- `commit-summary`: the `gw commit` post-commit summary.
- `stash-view`: the `gw stash list` overview.

### Modified Capabilities
<!-- none in openspec/specs yet. Note: the unarchived change `colored-git-output` defines the `passthrough` capability, whose dispatch requirement lists only push/pull/fetch/clone/status. Once that change is archived, its dispatch list must be extended with log/branch/commit/stash (see tasks). -->

## Impact

- `apps/gw`: new dispatch entries, the TUI event loop for `gw log` (raw mode and alternate screen, which the network commands deliberately avoid), and runners for branch/commit/stash.
- `crates/gw-parse`: parsers for graph log lines, numstat, for-each-ref, stash list, author list.
- `crates/gw-render`: the log browser widgets (list, sidebar, diff, modals, hotkey bar, action panel) and the static branch/commit/stash renderers.
- No new crates expected; ratatui/crossterm already provide raw mode, events, and the alternate screen.
- Reverses the earlier non-goal "formatting `log`" from `colored-git-output`'s design. `diff` and `show` stay out of scope, apart from the in-browser diff pane.
- Commit actions run real git commands that change the repository; reset `--hard` can discard work and is guarded by a confirmation.
