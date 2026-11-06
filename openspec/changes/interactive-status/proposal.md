## Why

Preparing a commit today means switching between `gw status`, `git diff <file>`, `git add`, `git restore --staged` and `git restore` by hand, retyping paths each time. A full-screen view that shows the status list, the selected file's diff, and one-key stage / unstage / revert makes reviewing and shaping a commit fast. Most of the pieces already exist: the status grouping from `gw status`, and the list + diff panes, modals and terminal handoff from the `gw log` browser.

## What Changes

- **New `gw status -i` (`--interactive`).** Opens a full-screen TUI:
  - Left pane (about 20% of the width): the `gw status` groups (Conflicts, Staged, Changed, Submodules, Untracked) as a navigable list.
  - Right pane (about 80%): the selected entry's diff, colored. Staged entries show the index diff, Changed entries the work-tree diff, Untracked files their whole content as added lines.
- **Hotkeys:**
  - `Space`: stage the selected Changed / Untracked / Conflict entry, or unstage the selected Staged entry.
  - `a` stages everything; `u` unstages everything.
  - `r` reverts the selected entry to `HEAD` (discarding its changes, deleting it if it is new). Always confirmed in a modal that names the file.
  - `c` commits: hands the terminal to `git commit` (editor, hooks), then returns.
  - Navigation (`j`/`k`, arrows, `PgUp`/`PgDn`, `g`/`G`), `Tab` to move focus to the diff pane and scroll it, `Ctrl-R` to reload, `q`/`Esc` to quit.
- After every action the list reloads and the cursor stays on the same path when it still exists.
- `-i` is a gw-only flag. When stdout is not a terminal, `gw status -i` drops `-i` and runs plain `git status` with the remaining arguments, since git has no such flag.
- Plain `gw status` is unchanged.

## Capabilities

### New Capabilities
- `status-interactive`: the `gw status -i` TUI: launch rules, layout, list and diff panes, stage / unstage / revert / commit actions, confirmation, reload behavior, and safe display of file names.

### Modified Capabilities
- `passthrough`: the "Raw passthrough when not a TTY" rule gains one exception: the gw-only `-i`/`--interactive` flag of `gw status` is removed before running git.

## Impact

- `crates/gw-render`: new `stage` module (app state, key handling, drawing), reusing helpers from `browser` (pane blocks, modal, diff line coloring, hotkey bar).
- `crates/gw-parse`: reuses `parse_status_v2` and `parse_numstat_z`; no new parser expected.
- `apps/gw`: `status()` detects `-i`/`--interactive`; new `stage.rs` with the event loop and the git calls; `main.rs` non-TTY path strips `-i` for `status`.
- No new dependencies.
- Revert permanently discards uncommitted work, including deleting untracked files. It is always confirmed.
