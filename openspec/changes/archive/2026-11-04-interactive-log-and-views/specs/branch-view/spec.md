## Purpose

Gives `gw branch` a scannable overview of local branches: tracking state, divergence, age, and stale upstreams.

## ADDED Requirements

### Requirement: Branch overview
`gw branch` with no arguments on a TTY SHALL list local branches, current branch first and marked, then the others by most recent commit. Each line SHALL show the branch name (branch color; current one bold), its upstream (upstream color) or `no upstream` dim, `↑N ↓M` (ahead green, behind yellow, zero dim), the last commit's relative age, and its subject truncated to the terminal width.

#### Scenario: Branches with tracking
- **WHEN** `main` tracks `origin/main` and is 2 behind, and `feat` has no upstream
- **THEN** the list shows `main -> origin/main  ↑0 ↓2` and `feat  no upstream`

### Requirement: Gone upstreams
A branch whose configured upstream no longer exists SHALL show `gone` in red in place of the counts.

#### Scenario: Deleted remote branch
- **WHEN** `feat-x` tracked `origin/feat-x`, which was deleted and pruned
- **THEN** the line shows `feat-x -> origin/feat-x  gone`

### Requirement: Branch passthrough
`gw branch` with any argument (create, delete, rename, list options), or with stdout not a TTY, SHALL run plain `git branch [args]`.

#### Scenario: Delete branch
- **WHEN** the user runs `gw branch -d feat`
- **THEN** `git branch -d feat` runs with its normal output
