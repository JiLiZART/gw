## 1. Shared pieces

- [x] 1.1 Add `oid` to `gw_parse::Stash` and `%H` as a fourth field of `STASH_FORMAT`; `parse_stash_list` uses `splitn(4)`. Verify parser unit test with a message containing `:` and the `gw stash list` snapshot unchanged.
- [x] 1.2 Split `file_as_added` into `bytes_as_added(&[u8])` plus the capped file read; make `Repo`, `run_quiet`, `load`, `first_line` `pub(crate)` in apps/gw/src/stage.rs. Verify existing stage tests pass.
- [x] 1.3 Pull the list row drawing in `stage::view` into `pub(crate) fn item_line(item, check, width, color)`. Verify stage snapshots unchanged.

## 2. `gw add` (stage add mode)

- [x] 2.1 `Check { On, Partial, Off }`, `Item.check`, `add_items(&Status, &LineStats)` (one item per path, sorted). Verify with a unit test: staged `a.rs`, staged+changed `b.rs`, changed `c.rs`, untracked `n.txt` give `On`, `Partial`, `Off`, `Off`.
- [x] 2.2 `Mode::Add` in `App`: `Space` → `Stage`/`Unstage` by check state, `a`/`u` unchanged, `r`/`c` ignored, hotkeys without revert/commit. Verify unit tests per key.
- [x] 2.3 Add-mode drawing: checkbox rows, no group headers; diff = cached diff followed by work-tree diff. Verify with a `TestBackend` snapshot and a `[~]` diff test on a scratch repo.
- [x] 2.4 Launch: `"add"` in `FORMATTED`; bare `gw add` on a TTY → `stage::run_mode(Mode::Add)`; any argument → passthrough. Verify integration tests: `gw add | cat` equals `git add | cat` (output and exit code); `gw add -n .` equals `git add -n .`; scratch-repo test that toggling `[x]`, `[~]`, `[ ]` runs the right commands, including `f*.txt` beside `foo.txt` and an unborn branch.

## 3. Stash state machine (`gw_render::stash`)

- [x] 3.1 `App` with stashes, selection, files, diff, focus (Stashes/Files/Diff with `Tab`/`Shift-Tab`), `files_wanted`/`set_files`, `diff_wanted`/`set_diff`, empty-list state. Verify unit tests: focus cycle, selection change requests new files, empty list has no selection.
- [x] 3.2 Browse keys: `a` → `Apply`, `p` → `Pop`, `d` → confirm overlay, `y` → `Drop`, other key cancels; `Ctrl-R` → `Reload`; `q` → `Quit`. Verify unit tests including cancel.
- [x] 3.3 New stash: `n` → `OpenPicker`; `open_picker(items)` (empty → message); picker `Space`/`A`/`Enter` (nothing ticked → message)/`Esc`; message prompt typing, `Backspace`, `Enter` → `Push { paths, untracked, message, all }`, `Esc` back to picker. Verify unit tests: untick one of three gives `all: false` and two paths (rename gives both paths); all ticked gives `all: true`; empty message gives `None`.

## 4. Stash drawing (`gw_render::stash::view`)

- [x] 4.1 Three-pane layout with hotkey bar; one pane under 80 columns; picker screen (rows + diff); prompt modal; drop modal. Verify `TestBackend` snapshots at 150 and 60 columns, picker, prompt, drop modal.
- [x] 4.2 Sanitized stash messages/branches/file names; NO_COLOR render without colors and reverse-video selection. Verify a test with `ESC[2J` in a message and a `color: false` buffer check.

## 5. Stash git side (`apps/gw/src/stash_ui.rs`)

- [x] 5.1 Launch: `views::stash` routes `-i`/`--interactive` alone to `stash_ui::run`, with other args prints the error and returns 129; non-TTY passes through. Verify integration tests: `gw stash -i | cat` matches `git stash -i | cat` exit code; `gw stash -i pop` on a pty exits 129.
- [x] 5.2 Loaders: stash list, files (tracked name-status + numstat, untracked from `^3`), diffs (tracked, untracked blob). Verify scratch-repo tests for a `-u` stash with a modified, a deleted, and an untracked file.
- [x] 5.3 Actions with oid guard: apply, pop, drop, push (all / subset / with untracked / with message / default message). Verify scratch-repo tests: subset stash leaves unticked file in the work tree; untracked ticked is removed; shifted list refuses to drop; pop conflict keeps the stash and returns git's error line.
- [x] 5.4 Event loop: restore on exit and panic, resize, reload after each action. Verify by a manual run in a real terminal: browse, apply, pop, drop with confirm, new stash with and without message, quit.

## 6. Tests and docs

- [x] 6.1 Run `cargo test --workspace` and `cargo clippy --workspace --all-targets -- -D warnings`. Verify both pass on macOS and in Docker `rust:1-trixie`.
- [x] 6.2 README: `gw add` and `gw stash -i` sections with key tables and screenshots `docs/screenshots/add-interactive.svg`, `docs/screenshots/stash-interactive.svg`. Verify by reading the README diff and opening the SVGs (no local paths).
- [ ] 6.3 Manual check on a dark theme at 80 and 200 columns. Verify panes read clearly and nothing overlaps.
