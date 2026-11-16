## Context

See proposal.md for motivation and specs/status-interactive/spec.md for behavior.

What already exists and can be reused:
- `gw status` (apps/gw/src/main.rs `status()`) runs `git status --porcelain=v2 --branch -z`, two `git diff --numstat -z` calls, and `git submodule status`, then renders with `gw_render::status::render_status`. Grouping rules (Conflicts / Staged / Changed / Submodules / Untracked) live in that renderer.
- The `gw log` browser splits into a pure state machine (`gw_render::browser::App::key(KeyEvent) -> Effect`, tested without a terminal) and an event loop in apps/gw/src/log.rs that performs effects (runs git, loads files/diffs) and hands the terminal to git via `run_on_terminal`.
- `browser::view` has the diff line coloring, `pane_block`, `draw_modal`, the hotkey bar and the status line, all private to that module today.

## Goals / Non-Goals

**Goals:**
- Same architecture as `gw log`: pure `App` + `Effect` in gw-render, unit-tested by feeding keys; git I/O only in apps/gw.
- One source of truth for grouping, so the list and `gw status` never disagree.
- Every git call uses literal pathspecs and runs from the repository top level.

**Non-Goals:**
- Hunk or line staging (`git add -p`-style). Possible later on top of the diff pane.
- File watching / auto-refresh. `Ctrl-R` and post-action reload are enough.
- An inline commit-message editor. `c` uses git's editor.
- Stash, checkout, or branch actions from this view.

## Decisions

### 1. Module layout
- `crates/gw-render/src/stage/mod.rs`: `StageApp` (entries, selection, focus, diff, overlay, status message) with `key(KeyEvent) -> StageEffect`.
- `crates/gw-render/src/stage/view.rs`: `draw(f, &StageApp)`.
- `apps/gw/src/stage.rs`: `run(rest) -> i32`, event loop, effect handlers, git calls.
- `status()` in main.rs calls `stage::run` when `-i`/`--interactive` is present.

Alternative: add a status mode to the existing browser `App`. Rejected: that `App` is shaped around commit rows, filters and lazy log loading, and is already large; the two screens share widgets, not state.

### 2. Shared widgets
Make `pane_block`, `draw_modal`, the diff-line styling (as `diff_line(&str) -> Line`), and the hotkey bar drawing `pub(crate)` in `browser::view` and call them from `stage::view`. No new shared module unless a third screen appears.

### 3. Entries from one grouping function
Extract the grouping step of `render_status` into `status::groups(&Status) -> Vec<(Group, Vec<&Entry>)>` and use it from both `render_status` and the TUI, so ordering, colors and "file in both Staged and Changed" stay identical. A list entry is `(Group, Entry)`; the same path may appear twice with different groups. Selection identity for reload is `(group, path)`, falling back to `path`, then to the nearest index (spec: Reload and action feedback).

### 4. Git commands
All run as `git -C <toplevel> --literal-pathspecs ... -- <path>` with paths from porcelain v2 (top-level relative). Display paths stay cwd-relative like `gw status`.

| Action | Entry | Command |
|---|---|---|
| Stage | Changed / Untracked / Conflict / Submodule | `git add -A -- <path>` (`-A` so deletions stage) |
| Unstage | Staged | `git reset -q -- <path>` (works on an unborn branch); renames: both paths |
| Stage all | – | `git add -A -- <pathspecs or :/>` |
| Unstage all | – | `git reset -q -- <pathspecs or :/>` |
| Revert | Changed | `git restore --worktree -- <path>` |
| Revert | Staged `M`/`D`/`T` | `git restore --source=HEAD --staged --worktree -- <path>` |
| Revert | Staged `A` | `git rm -f -q -- <path>` |
| Revert | Staged `R` | `git rm -f -q -- <new>` then `git restore --source=HEAD --staged --worktree -- <old>` |
| Revert | Untracked | `git clean -f -q -- <path>` (`-fd` for a directory; never `-x`, so ignored files stay) |
| Commit | – | `git commit` on the real terminal |

`git reset -q -- <path>` is chosen over `git restore --staged` for unstage because `restore --staged` fails before the first commit (spec scenario "Stage in empty repository").

`git clean` rather than deleting from gw: git applies its own safety rules (no ignored files, refuses nested repositories without `-ff`), so a nested repo inside an untracked dir is not deleted.

The untracked-directory modal lists files from `git ls-files --others --exclude-standard -z -- <dir>`.

### 5. Diff sources
- Staged: `git diff --cached --no-ext-diff --no-color -- <path>` (against the empty tree is git's own behavior on an unborn branch).
- Changed / Conflict: `git diff --no-ext-diff --no-color -- <path>`.
- Untracked file: read the file in gw (bounded at 1 MiB, NUL ⇒ `binary file`) and prefix lines with `+`; avoids `git diff --no-index` exit-code quirks.
- Untracked dir: the `ls-files` list above.
- Submodule: `git diff --submodule=log -- <path>` plus the state labels from `SubmoduleState`.
- Diffs load synchronously on selection change; status queries are local and fast. If profiling shows lag on huge diffs, move loading to a thread like the log browser does.

### 6. Launch rules
- In `main.rs`, the non-TTY branch for `status` filters out `-i`/`--interactive` before `passthrough` (spec: passthrough delta).
- In `status()`: `-i` plus a format flag → `eprintln!("gw: status -i cannot be combined with <flag>")`, exit 129 (git's usage-error code).
- Remaining args go to the `git status --porcelain=v2` query; pathspecs also scope `a`/`u`.

### 7. Terminal handoff for commit
Reuse the log browser's `run_on_terminal` (move it to a small shared module in apps/gw, e.g. `term.rs`), with the return prompt naming the current screen. Before running, check the Staged group is non-empty (spec: Commit, nothing staged).

## Risks / Trade-offs

- [Revert destroys work] → Always a modal, `y` only, names the file; untracked deletes list the files; no revert for conflicts or submodules; `git clean` without `-x`.
- [Wrong file hit through pathspec magic] → `--literal-pathspecs` on every call; test with `f*.txt` beside `foo.txt`.
- [Exposed browser internals grow coupling] → only drawing helpers are shared, `pub(crate)`; state stays separate.
- [Large untracked file or diff] → 1 MiB read cap for untracked files; diff pane renders only visible lines.
- [Index lock from a concurrent git] → git's error shows in the status line; view stays open (spec: Git error shown).
- [Narrow terminals] → under 80 columns only one pane shows, toggled with `Tab`.
