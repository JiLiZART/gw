## 1. Shared pieces

- [x] 1.1 Extract the grouping step of `render_status` into `gw_render::status::groups(&Status) -> Vec<(Group, Vec<&Entry>)>` and use it inside `render_status`. Verify the existing `status_views` snapshot is unchanged (`cargo test -p gw-render`).
- [x] 1.2 Make the browser drawing helpers `pub(crate)`: `pane_block`, `draw_modal`, a new `diff_line(&str) -> Line` taken from `draw_diff`, and the hotkey bar drawing (taking a `&[(&str, &str)]`). Verify the browser tests and snapshots are unchanged.
- [x] 1.3 Move `run_on_terminal` from apps/gw/src/log.rs into a shared apps/gw module, with the return prompt's screen name as a parameter. Verify `gw log` actions still hand off and return (existing tests pass; manual cherry-pick in a scratch repo).

## 2. State machine (`gw_render::stage`)

- [x] 2.1 `StageApp` with entries `(Group, Entry)` built from `groups()`, selectable entries only (headers skipped), `sel`/`scroll`, `focus` (List / Diff), diff lines + scroll, overlay (None / Confirm), status message, and width/height. Verify with unit tests: `↓` from the last Staged entry lands on the first Changed entry; `g`/`G`/`PgDn` clamp; a clean tree has no selection.
- [x] 2.2 `key()` returns `StageEffect`: `Quit`, `Stage(entry)`, `Unstage(entry)`, `StageAll`, `UnstageAll`, `PrepareRevert(entry)`, `Revert(entry)`, `Commit`, `Reload`. `Space` picks Stage or Unstage by group; `r` on Conflict/Submodule sets a message instead; `c` with no Staged entries sets "nothing staged". The diff is pulled by the loop through `diff_wanted()` / `set_diff()` (no effect needed). Verify with unit tests per key and group.
- [x] 2.3 Revert modal: `PrepareRevert` result (file list + count for untracked dirs) opens `Confirm`; only `y` emits `Revert`, any other key closes it. Verify with unit tests for `y`, `n` and `Esc`.
- [x] 2.4 `replace_entries(new)` keeps the selection by `(group, path)`, then by `path`, then the nearest index. Verify with unit tests: staging `b.rs` keeps it selected in Staged; reverting the last entry selects the previous one.

## 3. Drawing (`gw_render::stage::view`)

- [x] 3.1 Layout: list pane max(24, 20%) and diff pane the rest; under 80 columns only the focused pane; hotkey bar and status line at the bottom. Verify with `TestBackend` tests at 150 and 60 columns.
- [x] 3.2 List pane: group headers with counts and colors from `gw status`, `code path` lines (cwd-relative, `old -> new`), reverse-video selection, sanitized names. Verify with a `TestBackend` snapshot of a mixed tree, and a test that a name with `ESC[2J` is shown as `\x1b[2J`.
- [x] 3.3 Diff pane: title `path +N -M` (`bin` for binary), colored lines via `diff_line`, scroll; confirmation modal naming the file and listing up to 10 files plus a count. Verify with a `TestBackend` snapshot including the open modal.
- [x] 3.4 NO_COLOR: no colors, selection in reverse video. Verify a `TestBackend` render with `color: false` has no foreground or background colors set.

## 4. Git side (`apps/gw/src/stage.rs`)

- [x] 4.1 Launch: in `status()`, `-i`/`--interactive` goes to `stage::run`; with a format flag print the error and return 129; in `main.rs` drop `-i`/`--interactive` for `status` on the non-TTY path. Verify with integration tests: `gw status -i | cat` equals `git status | cat`; `gw status -i -s` exits 129.
- [x] 4.2 Load: toplevel, prefix, porcelain v2 status with the user's args, numstat for both sides, submodule checkouts (reuse the `gw status` code paths). Verify the entry list for a fixture repo matches `gw status` output order.
- [x] 4.3 Diff loaders per entry kind (staged, changed/conflict, untracked file with 1 MiB cap and binary check, untracked dir via `ls-files`, submodule `--submodule=log`). Verify with unit tests on a scratch repo for each kind.
- [x] 4.4 Action runners with the commands from design.md decision 4, all with `-C <toplevel> --literal-pathspecs` and `--`; git's first error line goes to the status message. Verify with integration tests on a scratch repo: stage, unstage (including unborn branch), stage all / unstage all, revert of Changed, Staged `M`, Staged `A`, Staged `R`, untracked file and untracked dir, and that `f*.txt` does not touch `foo.txt`.
- [x] 4.5 Event loop: alternate screen + raw mode with restore on exit and on panic (as in `gw log`), resize handling, `Ctrl-R` reload, commit via the shared terminal handoff and reload after. Verify by a manual run in a real terminal: open, stage, unstage, revert with confirm, commit through the editor, quit; the terminal is restored each time.

## 5. Tests and docs

- [x] 5.1 Run `cargo test --workspace` and `cargo clippy --workspace --all-targets -- -D warnings`. Verify both pass on macOS and in Docker `rust:1-trixie`.
- [x] 5.2 README: add a `gw status -i` section with the key table and a screenshot `docs/screenshots/status-interactive.svg`. Verify by reading the README diff and opening the SVG.
- [ ] 5.3 Manual check on a dark theme at 80 and 200 columns, including a conflict and a dirty submodule. Verify the list and diff read clearly and nothing overlaps.
