## 1. Shared pieces

- [x] 1.1 `gw_parse::Branch` gains `oid` and `remote`; a format with `%(objectname)` and `%(refname)`; `*/HEAD` skipped. Verify parser unit test with local + remote + `origin/HEAD` rows, and the `gw branch` snapshot unchanged.
- [x] 1.2 Lift the three-pane layout, one-pane narrow mode, modal prompt and diff drawing from `stash::view` into `pub(crate)` helpers. Verify stash snapshots unchanged.

## 2. State machine (`gw_render::branch`)

- [x] 2.1 `App` with rows (Local/Remote headers), selection skipping headers, focus cycle, `files_wanted`/`set_files`, `diff_wanted`/`set_diff`, empty state. Verify unit tests: header rows not selectable, selection change requests files once per oid, empty list message.
- [x] 2.2 Keys: `Enter` → `Checkout`; `n` prompt → `Create`; `d` → confirm → `Delete{force:false}` / `DeleteRemote`, force prompt via `ask_force(branch)`; `R` prompt prefilled → `Rename` (remote → message); `m` → confirm → `Merge` (current → message); `Ctrl-R`, `q`, `Esc`. Verify a unit test per key including cancel paths and current-branch refusals.

## 3. Drawing (`gw_render::branch::view`)

- [x] 3.1 Branch rows (marker, name, ↑↓/gone, age), files pane title (`<b> vs <current>`, `working tree`, `nothing new against <current>`), status line, hotkey bar, prompts. Verify `TestBackend` snapshots at 150 and 60 columns, delete prompt, force prompt, rename prompt.
- [x] 3.2 Sanitized names/subjects/paths; NO_COLOR render. Verify a test with `ESC[2J` in a subject and a `color: false` buffer check.

## 4. Git side (`apps/gw/src/branch_ui.rs`)

- [x] 4.1 Launch: `views::branch` routes `-i`/`--interactive` alone on a TTY to `branch_ui::run`, error 129 with other args, non-TTY passthrough. Verify integration tests: `gw branch -i | cat` equals `git branch -i | cat`; `gw branch -i -a` on a pty exits 129 with the message.
- [x] 4.2 Loaders: branch list, files of a branch (merge-base, name-status, numstat), current-branch working tree rows, diffs. Verify scratch-repo tests: feature branch lists `M`/`A` with counts; merged branch empty; current branch shows an unstaged file.
- [x] 4.3 Actions with oid guard: checkout local/remote/existing-local, create, delete (merged, unmerged → force), remote delete (against a local bare remote), rename. Verify scratch-repo tests for each, including dirty-tree checkout failure keeping the branch and a moved branch refusing delete.
- [x] 4.4 Event loop: restore on exit/panic, resize, reload after actions, `Merge` effect restores the terminal then runs the formatted merge and exits with its code. Verify a pty run: browse, checkout, new, rename, delete with force, merge with conflict exits 1 and shows merge output.

## 5. Switch picker

- [x] 5.1 `Mode::Pick` in the branch `App`: starts on the current branch; `Enter` → `Checkout` then quit on success, error keeps it open; `n`/`d`/`R`/`m` ignored; hotkey bar `Enter`, `Tab`, `^R`, `q`. Verify unit tests per key and a `TestBackend` snapshot of the bar.
- [x] 5.2 Route bare `gw switch` on a TTY to `branch_ui::run(Mode::Pick)`; piped or with arguments unchanged. Verify integration tests: `gw switch | cat` equals `git switch | cat` (output and exit code); pty run picking `feat` with `Enter` switches the branch and exits 0; `q` leaves the branch unchanged.

## 6. Tests and docs

- [x] 6.1 Run `cargo test --workspace` and `cargo clippy --workspace --all-targets -- -D warnings`. Verify both pass on macOS and in Docker `rust:1-trixie`.
- [x] 6.2 README: `gw branch -i` section and `gw switch` picker note, key table, screenshot `docs/screenshots/branch-interactive.svg`. Verify the SVG has no local paths.
- [ ] 6.3 Manual check on a dark theme at 80 and 200 columns. Verify panes read clearly and nothing overlaps.
