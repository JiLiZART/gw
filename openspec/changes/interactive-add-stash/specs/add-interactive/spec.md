## Purpose

Lets the user pick which changed and untracked files are staged from one full-screen checklist, instead of calling `git add` or `git reset` once per file.

## ADDED Requirements

### Requirement: Launch and fallback
`gw add` with no arguments and stdout a TTY SHALL open a full-screen checklist of the working tree and restore the terminal (screen contents, cursor, input mode) on exit, including after an error. `gw add` with any argument (including `-i`, `-p`, pathspecs) SHALL run plain `git add [args]`. When stdout is not a TTY, `gw add` SHALL run plain `git add`. Outside a git repository, git's error SHALL be shown and its exit code returned. Quitting SHALL exit with code 0.

#### Scenario: Open and quit
- **WHEN** the user runs `gw add` in a repository with changes and presses `q`
- **THEN** the checklist opens, and after `q` the previous terminal contents are back and the exit code is 0

#### Scenario: Arguments pass through
- **WHEN** the user runs `gw add -i`
- **THEN** git's own interactive add menu runs

#### Scenario: Piped
- **WHEN** the user runs `gw add | cat`
- **THEN** the output and exit code equal `git add | cat`

### Requirement: Checklist rows
The left pane SHALL show one row per changed path (staged, unstaged, untracked, conflicted, or submodule), sorted by path, each with a checkbox, its change code and its path relative to the current directory (renames as `old -> new`):
- `[x]` the path has staged changes and no unstaged changes,
- `[~]` the path has both staged and unstaged changes,
- `[ ]` the path has no staged changes (unstaged, untracked or conflicted).
A path SHALL appear only once. Navigation keys SHALL be the same as in `gw status -i`. When there is nothing to stage or unstage, the list SHALL show `nothing to commit, working tree clean`.

#### Scenario: Mixed tree
- **WHEN** `a.rs` is staged, `b.rs` is staged and edited again, `c.rs` is modified, and `n.txt` is untracked
- **THEN** the rows are `[x] a.rs`, `[~] b.rs`, `[ ] c.rs`, `[ ] n.txt`

### Requirement: Toggle
`Space` SHALL act on the selected row immediately: `[ ]` and `[~]` SHALL stage the whole path (including deletions and untracked files; a conflicted path is marked resolved); `[x]` SHALL unstage the path without touching the work tree (also before the first commit). `a` SHALL stage every row; `u` SHALL unstage every row. Paths SHALL be passed to git literally. After every action the rows SHALL reload from git and the selection SHALL stay on the same path. A failing git command SHALL show git's first error line in the status line and the view SHALL stay open.

#### Scenario: Stage untracked
- **WHEN** the selection is on `[ ] n.txt` and the user presses `Space`
- **THEN** `n.txt` is staged and its row shows `[x]`

#### Scenario: Finish partial
- **WHEN** the selection is on `[~] b.rs` and the user presses `Space`
- **THEN** the unstaged part is staged and the row shows `[x]`

#### Scenario: Unstage
- **WHEN** the selection is on `[x] a.rs` and the user presses `Space`
- **THEN** `a.rs` is unstaged, its work-tree content is unchanged, and the row shows `[ ]`

#### Scenario: Literal path
- **WHEN** `f*.txt` and `foo.txt` are both modified and the user toggles `f*.txt`
- **THEN** only `f*.txt` is staged

### Requirement: Diff pane
The right pane SHALL show the selected path's changes with the same coloring and title (`path +N -M`, `bin` for binary) as `gw status -i`: the staged diff followed by the unstaged diff for a tracked path, the whole content as added lines for an untracked file (`binary file` if binary), the file list for an untracked directory, and the commit log for a submodule. Layout, `Tab` focus switching and narrow-terminal behavior SHALL match `gw status -i`.

#### Scenario: Partial file
- **WHEN** the selection is on `[~] b.rs`
- **THEN** the diff pane shows both the staged and the unstaged changes of `b.rs`

### Requirement: Keys and display
The bottom line SHALL list `Space` toggle, `a` stage all, `u` unstage all, `Tab` focus, `q` quit. The checklist SHALL NOT offer revert or commit. `Ctrl-R` SHALL reload. File names and diff content SHALL be shown with control characters replaced (as `\xNN`). Under `NO_COLOR` the view SHALL keep its layout without color, using reverse video for the selection.

#### Scenario: No revert key
- **WHEN** the user presses `r` in the checklist
- **THEN** nothing changes on disk
