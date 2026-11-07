## Purpose

Lets the user browse stashes with their files and diffs, apply, pop or drop them, and create a new stash from selected files with an optional message, all from one full-screen view.

## ADDED Requirements

### Requirement: Launch and fallback
`gw stash -i` and `gw stash --interactive` with stdout a TTY SHALL open a full-screen stash view and restore the terminal on exit, including after an error. Any other argument combined with `-i` SHALL make `gw` print an error naming the argument and exit with code 129. When stdout is not a TTY, `gw stash -i` SHALL run plain `git stash -i` (git reports its usage error). Outside a git repository, git's error SHALL be shown and its exit code returned. Quitting SHALL exit with code 0.

#### Scenario: Open and quit
- **WHEN** the user runs `gw stash -i` and presses `q`
- **THEN** the stash view opens, and after `q` the terminal is restored and the exit code is 0

#### Scenario: Extra argument
- **WHEN** the user runs `gw stash -i pop`
- **THEN** an error is printed, no view opens, and the exit code is 129

### Requirement: Layout
The view SHALL show three panes side by side: the stash list (about 20% of the width, at least 24 columns), the files of the selected stash (about 20%, at least 24 columns), and the diff of the selected file (the rest), with a hotkey bar on the bottom line. `Tab` SHALL move focus list → files → diff → list; `Shift-Tab` SHALL move it back. The focused pane SHALL be visually marked. Under 80 columns only the focused pane SHALL be shown.

#### Scenario: Wide terminal
- **WHEN** the view runs in a 150-column terminal
- **THEN** the stash list, the file list and the diff are side by side

#### Scenario: Narrow terminal
- **WHEN** the view runs in a 60-column terminal
- **THEN** only the focused pane is shown and `Tab` switches to the next one

### Requirement: Stash list
Each stash SHALL be one row, newest first: its index, relative age, branch it was made on, and message, the same fields as `gw stash list`. With no stashes the pane SHALL show `no stashes` and the other panes SHALL be empty. Navigation keys SHALL match `gw status -i`.

#### Scenario: Two stashes
- **WHEN** there are stashes `stash@{0}` on `main` and `stash@{1}` on `feat`
- **THEN** the list shows `0` first, then `1`, each with age, branch and message

### Requirement: Stash files and diff
The file pane SHALL list the files of the selected stash with their change code and `+N -M`: tracked changes compared to the stash's base commit, then untracked files stored in the stash (code `?`). The diff pane SHALL show the selected file's diff against the base commit (an untracked file as all added lines, `binary file` if binary), colored as in `gw status -i`. Both panes SHALL update when the selection changes.

#### Scenario: Stash with untracked file
- **WHEN** a stash made with `-u` holds a change to `a.rs` and an untracked `n.txt`
- **THEN** the file pane lists `M a.rs` and `? n.txt`, and selecting `n.txt` shows its content as added lines

### Requirement: Apply, pop and drop
With the stash list or file pane focused:
- `a` SHALL apply the selected stash to the work tree and keep it.
- `p` SHALL apply the selected stash and remove it when it applies cleanly.
- `d` SHALL open a confirmation modal naming the stash (index and message) and stating it will be deleted; only `y` SHALL confirm, any other key SHALL cancel.
Before running git, `gw` SHALL check that the stash index still points to the stash shown; if it does not, it SHALL reload the list and show a message instead of acting. After every action the view SHALL reload. A failing git command (for example a conflict on apply) SHALL show git's first error line in the status line; on a conflict during `p` the stash SHALL be kept.

#### Scenario: Apply
- **WHEN** the user selects `stash@{1}` and presses `a`
- **THEN** its changes are in the work tree and `stash@{1}` is still listed

#### Scenario: Drop with confirm
- **WHEN** the user selects `stash@{0}`, presses `d`, then `y`
- **THEN** the stash is deleted and the list no longer shows it

#### Scenario: Cancel drop
- **WHEN** the user presses `d` then `n`
- **THEN** no stash is deleted

#### Scenario: Stale index
- **WHEN** another program pushes a new stash while the view is open and the user presses `d` then `y` on the row shown as `0`
- **THEN** no stash is dropped, the list reloads, and a message says the list changed

#### Scenario: Pop conflict
- **WHEN** popping a stash conflicts with the work tree
- **THEN** git's error is shown and the stash is still listed

### Requirement: New stash
`n` SHALL open a picker listing the current changes (one row per path, as in the `gw add` checklist, including untracked files), all ticked, with the diff of the selected path on the right. `Space` SHALL toggle a row and `A` SHALL toggle all rows. `Enter` SHALL open a one-line message prompt (with nothing ticked it SHALL show a message instead). In the prompt, typed text SHALL be the stash message; `Enter` SHALL create the stash, and an empty message SHALL let git use its default message. `Esc` SHALL step back (prompt to picker, picker to stash list) without changing anything.

The stash SHALL contain exactly the ticked paths, staged and unstaged changes of each; ticked untracked files SHALL be included and unticked untracked files SHALL stay in the work tree. When every row is ticked, the result SHALL equal `git stash push [-u] [-m <msg>]` with no pathspec. With nothing to stash, `n` SHALL show a message instead of the picker. Paths SHALL be passed to git literally.

#### Scenario: Stash two of three files
- **WHEN** `a.rs`, `b.rs` and `c.rs` are modified, the user presses `n`, unticks `c.rs`, presses `Enter`, types `wip parser`, and presses `Enter`
- **THEN** a new `stash@{0}` with message `wip parser` holds `a.rs` and `b.rs`, and `c.rs` keeps its changes in the work tree

#### Scenario: Default message
- **WHEN** the user leaves the message empty and presses `Enter`
- **THEN** the stash is created with git's `WIP on <branch>: …` message

#### Scenario: Untracked included
- **WHEN** untracked `n.txt` is ticked
- **THEN** the new stash contains `n.txt` and it is removed from the work tree

#### Scenario: Cancel
- **WHEN** the user presses `n`, then `Esc`
- **THEN** no stash is created and the stash list is shown

### Requirement: Keys and display
The bottom line SHALL list the keys for the current screen: in the browser `a` apply, `p` pop, `d` drop, `n` new, `Tab` focus, `q` quit; in the picker `Space` toggle, `A` all, `Enter` next, `Esc` back; in the prompt `Enter` stash, `Esc` back. `Ctrl-R` SHALL reload. Stash messages, branch names, file names and diff content SHALL be shown with control characters replaced (as `\xNN`). Under `NO_COLOR` the view SHALL keep its layout without color, using reverse video for the selection.

#### Scenario: Escape in stash message
- **WHEN** a stash message contains `ESC[2J`
- **THEN** the list shows `\x1b[2J` and the screen is not cleared
