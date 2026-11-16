## MODIFIED Requirements

### Requirement: Branch passthrough
`gw branch` with any argument (create, delete, rename, list options), or with stdout not a TTY, SHALL run plain `git branch [args]`. The exception is `-i`/`--interactive` given alone on a TTY, which opens the interactive branch screen; `-i` combined with any other argument SHALL print `gw: branch -i cannot be combined with <arg>` and exit 129.

#### Scenario: Delete branch
- **WHEN** the user runs `gw branch -d feat`
- **THEN** `git branch -d feat` runs with its normal output

#### Scenario: Interactive flag
- **WHEN** the user runs `gw branch -i` on a TTY
- **THEN** the interactive branch screen opens

#### Scenario: Interactive flag with arguments
- **WHEN** the user runs `gw branch -i -a` on a TTY
- **THEN** `gw: branch -i cannot be combined with -a` is printed and the exit code is 129
