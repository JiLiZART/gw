## MODIFIED Requirements

### Requirement: Raw passthrough when not a TTY
When stdout is not a terminal, `gw` SHALL behave exactly like plain `git <cmd> [args...]`: no extra flags injected, no formatting, no escape sequences. The one exception: for `gw status`, the gw-only flags `-i` and `--interactive` SHALL be removed before running git, since git does not accept them.

#### Scenario: Piped output
- **WHEN** the user runs `gw push | cat`
- **THEN** the output is identical to `git push | cat`

#### Scenario: Piped interactive status
- **WHEN** the user runs `gw status -i | cat`
- **THEN** the output is identical to `git status | cat`
