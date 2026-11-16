# stash-view Specification

## Purpose

Gives `gw stash list` a readable list of stashes with their age, origin, and size.

## Requirements

### Requirement: Stash overview
`gw stash list` on a TTY SHALL print one line per stash, newest first: its index (`0`, `1`, ...), relative age, the branch it was created on (branch color), its message, and `<K> files +N -M` for tracked changes. With no stashes it SHALL print a dim `no stashes`.

#### Scenario: Two stashes
- **WHEN** there are stashes made on `main` 2 hours ago and on `feat` 3 days ago
- **THEN** two lines are shown, the `main` stash first, each with its age, branch, message, and counts

### Requirement: Stash passthrough
Every other `gw stash` form (no subcommand, `push`, `pop`, `apply`, `drop`, `show`, `list` with options), and `gw stash list` with stdout not a TTY, SHALL run plain `git stash [args]`. The one exception: `gw stash -i` / `gw stash --interactive` with stdout a TTY SHALL open the interactive stash view (see `stash-interactive`).

#### Scenario: Pop
- **WHEN** the user runs `gw stash pop`
- **THEN** `git stash pop` runs with its normal output

#### Scenario: Bare stash unchanged
- **WHEN** the user runs `gw stash` on a TTY
- **THEN** `git stash` runs and stashes the changes immediately
