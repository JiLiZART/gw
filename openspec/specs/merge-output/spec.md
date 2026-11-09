# merge-output Specification

## Purpose

Makes the outcome of a failed `gw merge` or `gw pull` easy to act on: conflicted files and git's hints are shown in titled boxes.

## Requirements

### Requirement: Merge conflict box
When `gw merge` or `gw pull` exits non-zero and the repository has unmerged paths, `gw` SHALL print, after the merge summary, a box titled `Merge conflict` (title red) listing each unmerged path (relative to the repository top, sanitized), a blank line, and the next steps for the operation in progress: for a merge `gw add <file> && gw commit` and `gw merge --abort`; for a rebase `gw add <file> && git rebase --continue` and `git rebase --abort`. git's `Automatic merge failed; …` line SHALL NOT be printed when the box is shown. `CONFLICT (…)` lines SHALL still be printed.

#### Scenario: Content conflict
- **WHEN** `gw merge feat` conflicts in `src/a.rs` and `b.txt`
- **THEN** the output shows the `CONFLICT` lines, then a `Merge conflict` box containing `src/a.rs`, `b.txt` and the merge next steps, and the exit code is 1

#### Scenario: Pull with rebase conflict
- **WHEN** `gw pull --rebase` stops on a conflict in `a.rs`
- **THEN** the box lists `a.rs` and the rebase next steps

#### Scenario: Failure without conflict
- **WHEN** `gw merge feat` fails because local changes would be overwritten
- **THEN** no `Merge conflict` box is printed

### Requirement: Hint box
For `gw merge` and `gw pull`, each run of consecutive stderr lines starting with `hint:` SHALL be printed as one box titled `hint` (dim border and title) at the position of the run among the other stderr lines. Inside the box the `hint:` prefix and one following space SHALL be removed, tabs SHALL become four spaces, empty `hint:` lines SHALL stay as blank lines, and lines longer than the box SHALL wrap at the terminal width. Other stderr lines SHALL keep their order and colors.

#### Scenario: Diverging branches
- **WHEN** `gw merge --ff-only feat` prints nine `hint:` lines then `fatal: Not possible to fast-forward, aborting.`
- **THEN** one `hint` box holds the nine lines without prefixes, and the `fatal:` line follows the box in red

#### Scenario: Error, hints, fatal
- **WHEN** git prints `error: Merging is not possible…`, two `hint:` lines, then `fatal: Exiting because of an unresolved conflict.`
- **THEN** the output is the `error:` line, a `hint` box with the two lines, then the `fatal:` line

#### Scenario: Narrow terminal
- **WHEN** the terminal is 40 columns and a hint line is 70 characters
- **THEN** the line wraps inside the box and no line exceeds 40 columns

#### Scenario: Other commands
- **WHEN** `gw push` is rejected with `hint:` lines
- **THEN** the hints print unchanged, without a box

### Requirement: Boxes without color
With `NO_COLOR` set, both boxes SHALL be drawn with the same border characters and no color codes. Text inside the boxes SHALL be sanitized like other repository text.

#### Scenario: NO_COLOR conflict
- **WHEN** `NO_COLOR=1 gw merge feat` conflicts
- **THEN** the `Merge conflict` box is printed with no escape sequences
