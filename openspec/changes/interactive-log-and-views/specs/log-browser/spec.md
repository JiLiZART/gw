## Purpose

Defines the interactive `gw log` screen: a keyboard-driven commit browser with a branch graph, a changed-files sidebar, a diff pane, and a hotkey bar.

## ADDED Requirements

### Requirement: Launch and fallback
`gw log [args]` on a TTY SHALL open a full-screen browser and restore the terminal (screen contents, cursor, input mode) on exit, including after an error. When stdout is not a TTY, or the arguments request git's own output format (`--oneline`, `--format`, `--pretty`, `-p`, `--patch`, `--stat`, `--graph`, `--name-only`, `--name-status`), `gw` SHALL run plain `git log [args]`. Other arguments (revision ranges, `-- <path>`, `-n`) SHALL narrow the commits shown.

#### Scenario: Piped log
- **WHEN** the user runs `gw log | head`
- **THEN** the output is identical to `git log | head`

#### Scenario: Path argument
- **WHEN** the user runs `gw log -- src/`
- **THEN** the browser lists only commits that touch `src/`

#### Scenario: Quit restores terminal
- **WHEN** the user presses `q` in the browser
- **THEN** the previous terminal contents are back and the shell prompt works normally

### Requirement: Commit list with graph
The left pane SHALL list commits newest first, one per line: the branch graph, short SHA, relative date, author, ref chips (branches, remote branches, tags, `HEAD`), and the subject. Graph lines SHALL be colored per lane, and the commit node SHALL be drawn as a dot. The selected line SHALL be highlighted. Commits SHALL appear as they load, without waiting for the full history.

#### Scenario: Merge in graph
- **WHEN** the history contains a merge commit
- **THEN** the graph shows the branch splitting and joining, with each lane in its own color

#### Scenario: Large history
- **WHEN** the repository has 100,000 commits
- **THEN** the first screen of commits is shown within one second and scrolling stays responsive while the rest loads

### Requirement: Navigation
`↑`/`k` and `↓`/`j` SHALL move the selection by one line, `PgUp`/`PgDn` by a page, and `g`/`G` (or `Home`/`End`) to the first/last commit. The list SHALL scroll to keep the selection visible.

#### Scenario: Page down
- **WHEN** the selection is on the first commit and the user presses `PgDn`
- **THEN** the selection moves down by one screen height and the list scrolls

### Requirement: Changed-files sidebar
A right sidebar SHALL show the files changed by the selected commit, each with its change code (`A`, `M`, `D`, `R`) and `+N -M` counts (`bin` for binary), plus a total line. `Tab` SHALL move focus between the commit list and the sidebar; when the sidebar has focus, the navigation keys scroll and select within it. The focused pane SHALL be visually marked. When the terminal is narrower than 100 columns, the sidebar SHALL be hidden until `Tab` shows it over the list.

#### Scenario: Switch focus
- **WHEN** the user presses `Tab` while the commit list is focused
- **THEN** the sidebar border is highlighted and `↓` selects the next file

#### Scenario: Merge commit files
- **WHEN** the selected commit is a merge
- **THEN** the sidebar lists the files changed relative to its first parent

### Requirement: Diff pane
`Enter` on a file in the sidebar SHALL open a scrollable diff of that file in the selected commit, with added lines green, removed lines red, and hunk headers dimmed. `Esc` SHALL return to the browser with the same commit and file selected.

#### Scenario: Open and close diff
- **WHEN** the user presses `Enter` on `src/a.rs` and then `Esc`
- **THEN** the diff of `src/a.rs` is shown, and after `Esc` the browser shows the same commit with `src/a.rs` selected

### Requirement: Hotkey bar
The bottom line SHALL list the hotkeys available in the current context (list, sidebar, diff, modal, action panel), nano-style: the key highlighted, followed by a short label. Hotkeys that do not fit SHALL be dropped from the right.

#### Scenario: Context-dependent hotkeys
- **WHEN** the diff pane is open
- **THEN** the hotkey bar shows scrolling keys and `Esc Back`, not the filter keys

### Requirement: Safe display of repository text
Commit subjects, author names, ref names, file names, and diff content SHALL have control characters shown as `\xNN` (tab expanded to spaces in diffs), so repository content cannot inject terminal escape sequences.

#### Scenario: Escape in commit subject
- **WHEN** a commit subject contains `ESC[2J`
- **THEN** the list shows `\x1b[2J` and the screen is not cleared
