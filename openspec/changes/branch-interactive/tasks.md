## 1. Shared pieces

- [ ] 1.1 `gw_parse::Branch` gains `oid` and `remote`; a format with `%(objectname)` and `%(refname)`; `*/HEAD` skipped. Verify parser unit test with local + remote + `origin/HEAD` rows, and the `gw branch` snapshot unchanged.
- [ ] 1.2 Lift the three-pane layout, one-pane narrow mode, modal prompt and diff drawing from `stash::view` into `pub(crate)` helpers. Verify stash snapshots unchanged.

## 2. State machine (`gw_render::branch`)

- [ ] 2.1 `App` with rows (Local/Remote headers), selection skipping headers, focus cycle, `files_wanted`/`set_files`, `diff_wanted`/`set_diff`, empty state. Verify unit tests: header rows not selectable, selection change requests files once per oid, empty list message.
- [ ] 2.2 Keys: `Enter` → `Checkout`; `n` prompt → `Create`; `d` → confirm → `Delete{force:false}` / `DeleteRemote`, force prompt via `ask_force(branch)`; `R` prompt prefilled → `Rename` (remote → message); `m` → confirm → `Merge` (current → message); `Ctrl-R`, `q`, `Esc`. Verify a unit test per key including cancel paths and current-branch refusals.

## 3. Drawing (`gw_render::branch::view`)

- [ ] 3.1 Branch rows (marker, name, ↑↓/gone, age), files pane title (`<b> vs <current>`, `working tree`, `nothing new against <current>`), status line, hotkey bar, prompts. Verify `TestBackend` snapshots at 150 and 60 columns, delete prompt, force prompt, rename prompt.
- [ ] 3.2 Sanitized names/subjects/paths; NO_COLOR render. Verify a test with `ESC[2J` in a subject and a `color: false` buffer check.

## 4. Git side (`apps/gw/src/branch_ui.rs`)

- [ ] 4.1 Launch: `views::branch` routes `-i`/`--interactive` alone on a TTY to `branch_ui::run`, error 129 with other args, non-TTY passthrough. Verify integration tests: `gw branch -i | cat` equals `git branch -i | cat`; `gw branch -i -a` on a pty exits 129 with the message.
- [ ] 4.2 Loaders: branch list, files of a branch (merge-base, name-status, numstat), current-branch working tree rows, diffs. Verify scratch-repo tests: feature branch lists `M`/`A` with counts; merged branch empty; current branch shows an unstaged file.
- [ ] 4.3 Actions with oid guard: checkout local/remote/existing-local, create, delete (merged, unmerged → force), remote delete (against a local bare remote), rename. Verify scratch-repo tests for each, including dirty-tree checkout failure keeping the branch and a moved branch refusing delete.
- [ ] 4.4 Event loop: restore on exit/panic, resize, reload after actions, `Merge` effect restores the terminal then runs the formatted merge and exits with its code. Verify a pty run: browse, checkout, new, rename, delete with force, merge with conflict exits 1 and shows merge output.

## 5. Tests and docs

- [ ] 5.1 Run `cargo test --workspace` and `cargo clippy --workspace --all-targets -- -D warnings`. Verify both pass on macOS and in Docker `rust:1-trixie`.
- [ ] 5.2 README: `gw branch -i` section, key table, screenshot `docs/screenshots/branch-interactive.svg`. Verify the SVG has no local paths.
- [ ] 5.3 Manual check on a dark theme at 80 and 200 columns. Verify panes read clearly and nothing overlaps.
