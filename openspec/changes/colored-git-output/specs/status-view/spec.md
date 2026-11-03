## Purpose

Gives `gw status` a compact, grouped, colored view of the working tree and branch state, without git's instructional hint text.

## ADDED Requirements

### Requirement: Branch header
`gw status` SHALL print a header line with the current branch (branch color, bold), its upstream if any, and ahead/behind counts shown as `↑N ↓M` (ahead in green, behind in yellow; zero counts dim). A detached HEAD SHALL show `detached at <short-sha>` in yellow. No upstream SHALL show `no upstream` dim.

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
