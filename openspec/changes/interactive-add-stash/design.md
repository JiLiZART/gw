## Context

See proposal.md for motivation and specs/ for behavior.

What exists and is reused:
- `gw status -i` (change `interactive-status`): `gw_render::stage` has `Item { group, entry, stat }`, `items(&Status, &LineStats)`, `App::key(KeyEvent) -> Effect`, `replace_items` (keeps selection), and `stage::view::draw` with `list_width`, `truncate_left`, NO_COLOR handling. `apps/gw/src/stage.rs` has `Repo { top, prefix, args }`, `git()` (adds `-C <top> --literal-pathspecs`), `run_quiet`, `load()`, `load_diff()`, `file_as_added()`, the Stage/Unstage/StageAll/UnstageAll runners, and the event loop with `ratatui::init`/`restore`.
- `browser::view` shares `pane_block`, `draw_modal`, `diff_line`, `draw_keys`, `centered`, `fg`, `SEL_BG`, `FOCUS`, `DIM` as `pub(crate)`.
- `gw stash list`: `views::stash`, `STASH_FORMAT`, `gw_parse::parse_stash_list -> Vec<Stash { reference, index, age, branch, message }>`, `render_stash_list`.
- The browser search box (`browser/mod.rs`) already handles `Char`/`Backspace` text input; the stash message prompt copies that pattern.

## Goals / Non-Goals

**Goals:**
- Same architecture as `gw log` / `gw status -i`: pure `App` + `Effect` in gw-render, unit-tested with keys; git I/O only in apps/gw.
- `gw add` is a mode of the existing stage screen, not a new screen.
- The new-stash picker and the add checklist share one row builder, so both show the same rows for the same tree.
- Every git call keeps `-C <top> --literal-pathspecs`.

**Non-Goals:**
- Hunk-level selection for add or stash (`-p`).
- `stash push --keep-index`, `--staged`, `stash branch`, `stash show` options.
- Editing an existing stash's message.
- Pathspec arguments for `gw add` (any argument means plain git).

## Decisions

### 1. `gw add` = stage screen in `Mode::Add`
`stage::App` gets `mode: Mode { Status, Add }` and `Item` gets `check: Option<Check { On, Partial, Off }>` (`None` in status mode). `add_items(&Status, &LineStats) -> Vec<Item>` folds the `groups()` output into one item per path, sorted by path:
- in Staged only → `On` (group Staged),
- in Staged and Changed → `Partial` (group Changed, so `Space` stages),
- otherwise → `Off` (its own group: Changed / Untracked / Conflicts / Submodules).

`key()` in `Mode::Add`: `Space` returns `Stage` for `Off`/`Partial` and `Unstage` for `On`; `a`/`u` as in status mode; `r` and `c` are ignored. `hotkeys()` drops `r`/`c`. `replace_items` already falls back to matching by path, which is the identity in add mode.

The view draws `[x]`/`[~]`/`[ ]` before the code and skips group headers in add mode. Diff in add mode: `diff --cached` output followed by `diff` output for tracked paths (either may be empty); untracked/submodule cases reuse `load_diff` unchanged.

Alternative: separate `add` module. Rejected: same list, diff, actions, reload; only rows and keys differ.

Launch: `main.rs` adds `"add"` to `FORMATTED`; `add(args)` calls `stage::run_mode(Mode::Add, &[])` when `args` is empty, else `passthrough`. The non-TTY path is already plain passthrough.

### 2. `gw stash -i` = new `stash` module
- `crates/gw-render/src/stash/mod.rs`: `App` with `stashes: Vec<Stash>`, `sel`, `files: Vec<Item>`, `file_sel`, `diff`, `focus: Focus { Stashes, Files, Diff }`, `screen: Screen { Browse, Pick { rows: Vec<(Item, bool)>, sel }, Message { input: String } }`, `overlay: Overlay { None, ConfirmDrop(usize) }`, status message, size.
  - `key() -> Effect { None, Quit, Reload, Apply(i), Pop(i), Drop(i), OpenPicker, Push { paths, untracked, message, all } }`.
  - Files and diff are pulled like `stage`: `files_wanted() -> Option<usize>` / `set_files()`, `diff_wanted()` / `set_diff()`.
  - `OpenPicker` makes the loop load `add_items` for the work tree and call `app.open_picker(items)` (empty → message).
- `crates/gw-render/src/stash/view.rs`: three columns `max(24, w/5)`, `max(24, w/5)`, rest; under 80 columns only the focused pane. The picker screen reuses the stage add-mode row drawing (pull it into a `pub(crate) fn item_line(item, check, width, color)` in `stage::view`) beside the diff pane. The prompt is a one-line `draw_modal`.
- `apps/gw/src/stash_ui.rs`: event loop and git calls, reusing `stage::Repo`, `run_quiet`, `file_as_added`, `load` (make them `pub(crate)`).
- `views::stash` routes `-i`/`--interactive`: alone → `stash_ui::run()`, with any other argument → error + 129.

### 3. Stash identity guard
`Stash` gets an `oid` field; `STASH_FORMAT` gains `%H` as a fourth `\x1f` field (`parse_stash_list` uses `splitn(4)`; `gw stash list` output unchanged). Before `apply`/`pop`/`drop`, the loop runs `git rev-parse -q --verify stash@{N}` and compares with the shown `oid`; on mismatch it reloads and shows `stash list changed, nothing done`. Commands then use the ref `stash@{N}` (git accepts only refs for `drop`).

### 4. Git commands

| Action | Command |
|---|---|
| list | `git stash list --format=<STASH_FORMAT>` |
| files (tracked) | `git diff -M --name-status -z stash@{N}^1 stash@{N}` + `--numstat -z` for counts |
| files (untracked) | `git rev-parse -q --verify stash@{N}^3` then `git ls-tree -r -z --name-only stash@{N}^3` |
| diff (tracked) | `git diff -M --no-ext-diff --no-color stash@{N}^1 stash@{N} -- <path>` |
| diff (untracked) | `git cat-file blob stash@{N}^3:<path>` → same "added lines / binary file" rendering as `file_as_added` |
| apply / pop | `git stash apply -q stash@{N}` / `git stash pop -q stash@{N}` |
| drop | `git stash drop -q stash@{N}` |
| push (all ticked) | `git stash push [-u] [-m <msg>]` (`-u` when any untracked row exists) |
| push (subset) | `git stash push [-u] [-m <msg>] -- <paths>` (`-u` when a ticked row is untracked; renames pass both paths) |

Error text: first non-empty stderr line, else first stdout line (conflicts print `CONFLICT …` on stdout).

### 5. Shared file-as-added
`file_as_added` currently reads a path. Split it into a byte-slice function (`bytes_as_added(&[u8]) -> Vec<String>`) used by both the untracked-file reader (1 MiB cap kept) and the stash untracked blob.

## Risks / Trade-offs

- [Drop removes a stash] → confirm modal with index + message, `y` only, oid guard against a shifted list.
- [Wrong stash after external change] → oid guard on every action.
- [`stash push -- <paths>` edge cases] (a path both staged and changed is stashed whole; a deleted file needs its path) → acceptance tests for partial, deleted, renamed and untracked paths.
- [Pop conflict] → git keeps the stash; we show git's first error line and reload.
- [`gw add` shadowing habit] → only the bare form is taken; any argument (including `-i`) is git's.
- [Large stash diffs] → synchronous loading like `gw status -i`; move to a thread only if it lags.
