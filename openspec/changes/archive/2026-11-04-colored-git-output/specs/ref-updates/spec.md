## Purpose

Shows which refs a push, fetch, or pull changed, against which remote, as a compact colored list that is easy to scan.

## ADDED Requirements

### Requirement: Remote target header
After the remote box (or after the progress line if there is no box), the system SHALL print the remote target (e.g. `gitlab.example:group/repo.git`) on its own line, prefixed by an arrow marker, with any URL form rendered as a clickable link.

#### Scenario: Push target
- **WHEN** git reports pushing `To gitlab.example:group/repo.git`
- **THEN** a line `-> gitlab.example:group/repo.git` (arrow marker styled) is printed before the ref list

### Requirement: Colored ref update lines
Each updated ref SHALL be printed indented under the target as `<old>..<new>  <src> -> <dst>`, with the old SHA dim, the new SHA bold, and both branch names in the branch color. The update kind SHALL be indicated by a colored marker: fast-forward (green), forced update (yellow), new ref (green, labeled "new"), deleted ref (red, labeled "deleted"), rejected (red, with git's reason), up to date (dim).

#### Scenario: Fast-forward push
- **WHEN** a push updates `feat-forms-constructor` from `787e7fa` to `a22b7ed`
- **THEN** a line shows `787e7fa..a22b7ed  feat-forms-constructor -> feat-forms-constructor` with a green fast-forward marker

#### Scenario: Rejected push
- **WHEN** the server rejects `main` as non-fast-forward
- **THEN** the line shows a red rejected marker, `main -> main`, and the reason `fetch first` (or git's stated reason)

#### Scenario: New branch
- **WHEN** a push creates a new remote branch `feat-x`
- **THEN** the line shows a green "new" marker and `feat-x -> feat-x`

### Requirement: Nothing to update
When git reports no ref changes ("Everything up-to-date" or an empty fetch), the system SHALL print a single dim line stating that and no progress line or ref list.

#### Scenario: Up-to-date push
- **WHEN** `gw push` runs and git prints `Everything up-to-date`
- **THEN** output is one dim line "Everything up-to-date"

### Requirement: Pull merge/rebase summary passthrough
For `pull`, output that git prints after fetching (merge/rebase/fast-forward summary and diffstat) SHALL be printed after the ref list, with file paths and `+`/`-` counts colored green/red; unrecognized lines SHALL be printed unchanged.

#### Scenario: Fast-forward pull diffstat
- **WHEN** pull prints ` src/a.rs | 4 ++--`
- **THEN** the path is shown and `++` is green and `--` is red
