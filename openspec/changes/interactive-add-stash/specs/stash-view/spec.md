## MODIFIED Requirements

### Requirement: Stash passthrough
Every other `gw stash` form (no subcommand, `push`, `pop`, `apply`, `drop`, `show`, `list` with options), and `gw stash list` with stdout not a TTY, SHALL run plain `git stash [args]`. The one exception: `gw stash -i` / `gw stash --interactive` with stdout a TTY SHALL open the interactive stash view (see `stash-interactive`).

#### Scenario: Pop
- **WHEN** the user runs `gw stash pop`
- **THEN** `git stash pop` runs with its normal output

#### Scenario: Bare stash unchanged
- **WHEN** the user runs `gw stash` on a TTY
- **THEN** `git stash` runs and stashes the changes immediately
