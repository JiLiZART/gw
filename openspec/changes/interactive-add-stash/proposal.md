## Why

Staging several files today means calling `git add <path>` once per file, and checking what is already staged takes a separate `gw status`. Stashes are worse: `git stash list` shows only messages, so picking the right stash to apply or drop means running `git stash show -p stash@{N}` again and again, and stashing only some files needs a pathspec typed by hand. `gw status -i` already has the list + diff screen, the key handling and the git plumbing; two more screens built on it remove these manual loops.

## What Changes

- **New `gw add` with no arguments (TTY only): an interactive checklist.**
  - One row per changed path (tracked and untracked) with a checkbox: `[x]` fully staged, `[~]` partly staged, `[ ]` not staged. Right pane shows the diff of the selected path.
  - `Space` toggles the selected path immediately (`git add -A` / `git reset`), `a` stages all, `u` unstages all, `q`/`Esc` quits.
  - No revert or commit keys here; those stay in `gw status -i`.
  - `gw add <args>` and `gw add -i` are unchanged (git's own behavior). When stdout is not a TTY, `gw add` runs plain `git add` (which prints git's "Nothing specified" message).
- **New `gw stash -i` (`--interactive`, TTY only): a stash browser.**
  - Three panes: stash list (about 20%), files in the selected stash (about 20%), the selected file's diff (the rest).
  - `a` applies, `p` pops, `d` drops (confirmed in a modal) the selected stash.
  - `n` starts a new stash: a checklist of the current changes (all ticked by default), `Space` toggles a file, `A` toggles all; `Enter` asks for an optional message; `Enter` again runs `git stash push` with the chosen files (untracked files included when ticked). An empty message lets git pick its default `WIP on …` message.
  - Bare `gw stash`, `gw stash push`, `gw stash list` and every other form are unchanged. When stdout is not a TTY, `gw stash -i` runs plain `git stash -i` (git rejects the flag with its usage error).

## Capabilities

### New Capabilities
- `add-interactive`: the `gw add` checklist screen: launch rules, per-path checkbox states, immediate stage/unstage, diff pane.
- `stash-interactive`: the `gw stash -i` screen: launch rules, three-pane layout, apply / pop / drop with confirmation and stale-index guard, and the new-stash flow (file selection, optional message).

### Modified Capabilities
- `passthrough`: `add` joins the list of commands `gw` handles itself (only the no-argument TTY form; everything else passes through).
- `stash-view`: the "Stash passthrough" requirement gains the `-i`/`--interactive` exception on a TTY.

## Impact

- `crates/gw-render`: `stage` module gains an add mode (checkbox rows, reduced key set); new `stash` module (app state, keys, three-pane drawing, message input) reusing `browser::view` helpers and `stage` list drawing.
- `crates/gw-parse`: reuses `parse_status_v2`, `parse_numstat_z`, `parse_stash_list`; no new parser expected.
- `apps/gw`: `main.rs` adds `add` to the formatted commands; `stage.rs` serves both `gw status -i` and `gw add`; new `stash_ui.rs` with the event loop and git calls; `views::stash` routes `-i`.
- No new dependencies.
- `drop` permanently removes a stash (recoverable only via its SHA from the reflog/fsck); always confirmed. `pop`/`apply` can produce conflicts; git's message is shown and `pop` keeps the stash on conflict (git behavior).
