## MODIFIED Requirements

### Requirement: Subcommand dispatch and argument passthrough
`gw <cmd> [args...]` SHALL run the real `git <cmd> [args...]` for `cmd` in {`push`, `pull`, `fetch`, `clone`, `status`, `log`, `branch`, `commit`, `stash`}, passing every user argument unchanged and in order. Each of these commands SHALL fall back to plain `git <cmd> [args...]` under its own rule (output-format flags, arguments, or subcommands it does not format). Any other subcommand SHALL be executed as plain `git <cmd> [args...]` with inherited stdio and no formatting.

#### Scenario: Arguments forwarded
- **WHEN** the user runs `gw push -u origin feat --force-with-lease`
- **THEN** git receives `push -u origin feat --force-with-lease` (plus any flags `gw` needs for parsing that do not change git's behavior)

#### Scenario: Unknown subcommand
- **WHEN** the user runs `gw diff --stat`
- **THEN** `git diff --stat` runs with its normal output and pager

#### Scenario: Formatted command falls back
- **WHEN** the user runs `gw log --oneline`
- **THEN** `git log --oneline` runs with its normal output and pager

#### Scenario: No subcommand
- **WHEN** the user runs `gw` with no arguments
- **THEN** a short usage message listing the formatted subcommands is printed and the exit code is non-zero
