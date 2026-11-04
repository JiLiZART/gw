# status-view Specification

## Purpose

Gives `gw status` a compact, grouped, colored view of the working tree and branch state, without git's instructional hint text.

## Requirements

### Requirement: Branch header
`gw status` SHALL print a header line with the current branch (branch color, bold), its upstream if any (in an upstream color distinct from the branch color), and ahead/behind counts shown as `↑N ↓M` (ahead in green, behind in yellow; zero counts dim). A detached HEAD SHALL show `detached at <short-sha>` in yellow. No upstream SHALL show `no upstream` dim.

#### Scenario: Branch ahead of upstream
- **WHEN** branch `feat` tracks `origin/feat` and is 2 ahead, 0 behind
- **THEN** the header shows `feat -> origin/feat  ↑2 ↓0`

#### Scenario: Detached HEAD
- **WHEN** HEAD is detached at `a22b7ed`
- **THEN** the header shows `detached at a22b7ed` in yellow

### Requirement: Grouped file lists
Files SHALL be listed in groups, in this order, each with a header and count, omitting empty groups: "Conflicts" (red), "Staged" (green), "Changed" (yellow), "Untracked" (dim). Each file line SHALL show a one-letter change code (`M`, `A`, `D`, `R`, `C`, `U`, `?`) and the path; renames SHALL show `old -> new`. A file both staged and changed SHALL appear in both groups.

#### Scenario: Mixed working tree
- **WHEN** `a.rs` is staged-modified, `b.rs` is unstaged-modified, and `n.txt` is untracked
- **THEN** output shows "Staged (1)" with `M a.rs`, "Changed (1)" with `M b.rs`, and "Untracked (1)" with `? n.txt`, in that order

#### Scenario: Rename
- **WHEN** `old.rs` was renamed to `new.rs` and staged
- **THEN** the Staged group shows `R old.rs -> new.rs`

### Requirement: Clean tree
When there are no changes, the system SHALL print the branch header followed by a single dim line "clean".

#### Scenario: Clean repository
- **WHEN** the working tree has no changes or untracked files
- **THEN** output is the header and "clean"

### Requirement: No hint text
`gw status` SHALL NOT print git's advice lines (e.g. `(use "git add <file>..." to ...)`).

#### Scenario: Hints suppressed
- **WHEN** there are unstaged changes
- **THEN** no line containing `(use "git` is printed

### Requirement: Paths relative to current directory
File paths SHALL be displayed relative to the current working directory, matching `git status` behavior.

#### Scenario: Running from subdirectory
- **WHEN** `gw status` runs in `src/` and `src/a.rs` is changed
- **THEN** the path is shown as `a.rs`

#### Scenario: Untracked directory
- **WHEN** `gw status` runs in `src/` and `src/` itself is reported as an untracked directory `src/`
- **THEN** the entry is shown as `../src/`, keeping the trailing slash

### Requirement: Per-file line stats
Each file in the Staged, Changed, and Untracked groups SHALL show its line counts as `+N -M` after the path, aligned in one column per group, with `+N` green and `-M` red (a zero count dim). Staged counts SHALL come from `git diff --cached --numstat`, Changed counts from `git diff --numstat`, and Untracked files SHALL count every line as added. Binary files SHALL show `bin`. Conflicts and untracked directories SHALL show no stats.

#### Scenario: Changed file stats
- **WHEN** `b.rs` has 12 lines added and 3 removed in the working tree
- **THEN** the Changed group shows `M b.rs  +12 -3`

#### Scenario: Binary file
- **WHEN** a staged file is binary
- **THEN** its stat shows `bin`

### Requirement: Total line
After the groups, `gw status` SHALL print one line `Total +X -Y`, where X and Y are the sums of all counts shown. The line SHALL be omitted for a clean tree.

#### Scenario: Total of mixed tree
- **WHEN** shown stats are `+1 -0`, `+12 -3`, and `+2 -0`
- **THEN** the last line is `Total +15 -3`

### Requirement: Safe file names
Control characters (C0, DEL, C1) in paths and branch names SHALL be shown as `\xNN`, and tabs as a space, so file names cannot inject terminal escape sequences.

#### Scenario: Escape sequence in a file name
- **WHEN** an untracked file is named `a<ESC>[31mb`
- **THEN** it is shown as `a\x1b[31mb` and no escape sequence reaches the terminal
