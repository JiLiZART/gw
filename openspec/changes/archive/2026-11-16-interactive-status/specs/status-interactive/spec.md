## ADDED Requirements

### Requirement: Launch and fallback
`gw status -i` and `gw status --interactive` on a TTY SHALL open a full-screen view of the working tree and restore the terminal (screen contents, cursor, input mode) on exit, including after an error. Other arguments SHALL be passed to git's status query (pathspecs and `-u`/`--untracked-files` narrow the entries shown). When `-i` is combined with an output-format flag (`-s`, `--short`, `--long`, `--porcelain`, `-z`, `--null`), `gw` SHALL print an error naming the conflict and exit with code 129 without opening the view. Outside a git repository, git's error SHALL be shown and its exit code returned. Quitting SHALL exit with code 0.

#### Scenario: Open and quit
- **WHEN** the user runs `gw status -i` in a repository and presses `q`
- **THEN** the full-screen view opens, and after `q` the previous terminal contents are back and the exit code is 0

#### Scenario: Pathspec narrows
- **WHEN** the user runs `gw status -i -- src/`
- **THEN** only entries under `src/` are listed

#### Scenario: Conflicting flags
- **WHEN** the user runs `gw status -i -s`
- **THEN** an error is printed, no view opens, and the exit code is 129

### Requirement: Layout
The view SHALL show a file list on the left (about 20% of the width, at least 24 columns) and the diff of the selected entry on the right (the rest), with a hotkey bar on the bottom line. The focused pane SHALL be visually marked. When the terminal is narrower than 80 columns, only the focused pane SHALL be shown, and `Tab` SHALL switch between them.

#### Scenario: Wide terminal
- **WHEN** the view runs in a 150-column terminal
- **THEN** the file list and the diff are side by side, the list about 30 columns wide

#### Scenario: Narrow terminal
- **WHEN** the view runs in a 60-column terminal
- **THEN** only the file list is shown until the user presses `Tab`, which shows only the diff

### Requirement: File list
The list SHALL show the same groups as `gw status`, in the same order and colors (Conflicts, Staged, Changed, Submodules, Untracked), each with a header and count, omitting empty groups. Each entry SHALL show its change code and its path relative to the current directory; renames SHALL show `old -> new`. A file both staged and changed SHALL appear in both groups as separate entries. Headers SHALL NOT be selectable. `↑`/`k`, `↓`/`j`, `PgUp`/`PgDn`, `g`/`G` (`Home`/`End`) SHALL move the selection, and the list SHALL scroll to keep it visible. When the tree is clean, the list SHALL show `nothing to commit, working tree clean` and the diff pane SHALL be empty.

#### Scenario: Mixed working tree
- **WHEN** `a.rs` is staged-modified and also changed again in the work tree, and `n.txt` is untracked
- **THEN** the list shows "Staged (1)" with `M a.rs`, "Changed (1)" with `M a.rs`, and "Untracked (1)" with `? n.txt`

#### Scenario: Headers skipped
- **WHEN** the selection is on the last Staged entry and the user presses `↓`
- **THEN** the selection moves to the first Changed entry, not to the "Changed" header

### Requirement: Diff pane
The right pane SHALL show the diff of the selected entry, with added lines green, removed lines red, and hunk headers dimmed, and a title showing the path and its `+N -M` counts (`bin` for binary):
- Staged entry: the index against `HEAD` (against the empty tree before the first commit).
- Changed or Conflict entry: the work tree against the index.
- Untracked file: its whole content as added lines; a binary file SHALL show `binary file` instead.
- Untracked directory: the list of untracked files under it.
- Submodule: the submodule's commit change as a short commit log (`git diff --submodule=log` style) plus its state labels.

The diff SHALL update when the selection changes. `Tab` SHALL move focus to the diff pane, where the navigation keys scroll it; `Tab` again SHALL return focus to the list.

#### Scenario: Staged and changed versions
- **WHEN** `a.rs` is in both Staged and Changed and the user selects each entry in turn
- **THEN** the Staged entry shows the staged changes and the Changed entry shows only the unstaged changes

#### Scenario: Untracked file
- **WHEN** the user selects untracked `n.txt` with 3 lines
- **THEN** the diff pane shows its 3 lines as added lines and the title shows `+3 -0`

#### Scenario: Scroll diff
- **WHEN** the diff is longer than the pane, the user presses `Tab` then `PgDn`
- **THEN** the diff scrolls by one page and the file list selection does not change

### Requirement: Stage and unstage
`Space` SHALL toggle the selected entry: a Changed, Untracked or Submodule entry SHALL be staged (`git add`); a Conflict entry SHALL be marked resolved by staging it; a Staged entry SHALL be unstaged (its index entry reset to `HEAD`, or removed from the index before the first commit) without touching the work tree. `a` SHALL stage every change in the shown pathspec, including untracked files and deletions. `u` SHALL unstage everything in the shown pathspec. Paths SHALL be passed to git literally (no glob expansion), so file names containing `*`, `?` or `[` affect only that file.

#### Scenario: Stage a file
- **WHEN** the selection is on Changed `M b.rs` and the user presses `Space`
- **THEN** `b.rs` moves to the Staged group and the selection stays on `b.rs` in its new group

#### Scenario: Unstage a file
- **WHEN** the selection is on Staged `M a.rs` and the user presses `Space`
- **THEN** `a.rs` moves to the Changed group and its work-tree content is unchanged

#### Scenario: Literal path
- **WHEN** files `f*.txt` and `foo.txt` are both changed and the user stages `f*.txt`
- **THEN** only `f*.txt` is staged

#### Scenario: Stage in empty repository
- **WHEN** the repository has no commits, `n.txt` is staged, and the user presses `Space` on it
- **THEN** `n.txt` returns to Untracked

### Requirement: Revert with confirmation
`r` SHALL open a confirmation modal that names the selected entry and states that its changes will be permanently lost. Only `y` SHALL confirm; any other key SHALL cancel. On confirmation:
- Changed entry: the work-tree file SHALL be restored from the index (staged changes kept).
- Staged entry: index and work tree SHALL both be restored to `HEAD`; a file added since `HEAD` SHALL be removed from the index and deleted; a staged rename SHALL be undone (new path deleted, old path restored).
- Untracked entry: the file or directory SHALL be deleted; the modal SHALL list up to 10 of the files that will be deleted and the total count.
- Conflict and Submodule entries SHALL NOT be reverted; `r` SHALL show a message explaining why instead of the modal.

#### Scenario: Revert changed file
- **WHEN** `b.rs` has unstaged edits, the user presses `r` on Changed `M b.rs` and then `y`
- **THEN** `b.rs` matches the index and it leaves the Changed group

#### Scenario: Cancel revert
- **WHEN** the user presses `r` on an entry and then `n`
- **THEN** nothing changes on disk and the modal closes

#### Scenario: Revert untracked directory
- **WHEN** the user presses `r` on Untracked `tmp/` containing 25 files
- **THEN** the modal lists 10 of the files and says 25 files will be deleted, and after `y` the directory is gone

#### Scenario: Conflict not revertable
- **WHEN** the user presses `r` on a Conflict entry
- **THEN** no modal opens and a message says conflicts must be resolved or the merge aborted

### Requirement: Commit
`c` SHALL hand the terminal to `git commit` (so the editor and hooks work), then return to the view with the list reloaded and show whether the commit succeeded. When nothing is staged, `c` SHALL show a message instead of running git.

#### Scenario: Commit from the view
- **WHEN** `a.rs` is staged and the user presses `c`, writes a message in the editor, and saves
- **THEN** the commit is created, the view returns, and `a.rs` is no longer in the Staged group

#### Scenario: Nothing staged
- **WHEN** the Staged group is empty and the user presses `c`
- **THEN** no editor opens and a message says nothing is staged

### Requirement: Reload and action feedback
After every action the list and diff SHALL reload from git, and the selection SHALL stay on the same path (preferring the same group, else the group the path moved to, else the nearest remaining entry). `Ctrl-R` SHALL reload on demand. A failing git command SHALL show git's first error line in the status line; the view SHALL stay open.

#### Scenario: External change
- **WHEN** another program edits `c.rs` while the view is open and the user presses `Ctrl-R`
- **THEN** `c.rs` appears in the Changed group

#### Scenario: Git error shown
- **WHEN** an action fails because the index is locked (`index.lock` exists)
- **THEN** the status line shows git's error and the list is unchanged

### Requirement: Hotkey bar
The bottom line SHALL list the available keys for the focused pane: `Space` stage/unstage, `a` stage all, `u` unstage all, `r` revert, `c` commit, `Tab` focus, `q` quit. Under `NO_COLOR` the view SHALL keep its layout without color, using reverse video for the selection.

#### Scenario: Keys listed
- **WHEN** the view opens with the list focused
- **THEN** the bottom line shows `Space`, `a`, `u`, `r`, `c`, `Tab` and `q` with their actions

### Requirement: Safe display
File names and diff content SHALL be shown with control characters replaced (as `\xNN`, tab as spaces), so repository content cannot inject terminal escape sequences.

#### Scenario: Escape in file name
- **WHEN** an untracked file name contains `ESC[2J`
- **THEN** the list shows `\x1b[2J` and the screen is not cleared
