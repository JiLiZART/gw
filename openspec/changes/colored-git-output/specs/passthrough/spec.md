## Purpose

Defines how `gw` dispatches subcommands to the real git, keeps git's semantics (arguments, prompts, exit codes), and degrades safely in non-interactive or colorless environments.

## ADDED Requirements

### Requirement: Subcommand dispatch and argument passthrough
`gw <cmd> [args...]` SHALL run the real `git <cmd> [args...]` for `cmd` in {`push`, `pull`, `fetch`, `clone`, `status`}, passing every user argument unchanged and in order. Any other subcommand SHALL be executed as plain `git <cmd> [args...]` with inherited stdio and no formatting.

#### Scenario: Arguments forwarded
- **WHEN** the user runs `gw push -u origin feat --force-with-lease`
- **THEN** git receives `push -u origin feat --force-with-lease` (plus any flags `gw` needs for parsing that do not change git's behavior)

#### Scenario: Unknown subcommand
- **WHEN** the user runs `gw log --oneline`
- **THEN** `git log --oneline` runs with its normal output and pager

#### Scenario: No subcommand
- **WHEN** the user runs `gw` with no arguments
- **THEN** a short usage message listing the formatted subcommands is printed and the exit code is non-zero

### Requirement: Exit code propagation
`gw` SHALL exit with the same exit code as the git process it ran. If git is terminated by a signal, `gw` SHALL exit with `128 + signal`.

#### Scenario: Rejected push
- **WHEN** git push exits with code 1
- **THEN** `gw push` exits with code 1

#### Scenario: git not found
- **WHEN** no `git` executable is on `PATH`
- **THEN** `gw` prints an error to stderr and exits with code 127

### Requirement: Interactive prompts keep working
Credential, SSH passphrase, and host-key prompts issued by git or its helpers SHALL remain usable while `gw` is running.

#### Scenario: SSH passphrase prompt
- **WHEN** `gw push` triggers an SSH key passphrase prompt
- **THEN** the prompt is visible, the user can type the passphrase, and the push proceeds

### Requirement: Raw passthrough when not a TTY
When stdout is not a terminal, `gw` SHALL behave exactly like plain `git <cmd> [args...]`: no extra flags injected, no formatting, no escape sequences.

#### Scenario: Piped output
- **WHEN** the user runs `gw push | cat`
- **THEN** the output is identical to `git push | cat`

### Requirement: NO_COLOR
When the `NO_COLOR` environment variable is set to a non-empty value, `gw` SHALL keep its structured layout (progress line, box, grouped lists) but emit no color or style escape sequences; the progress bar SHALL use `#` for filled and `.` for empty cells. Hyperlinks SHALL still be emitted.

#### Scenario: NO_COLOR push
- **WHEN** `NO_COLOR=1 gw push` runs on a TTY
- **THEN** output contains the bordered box and a `[####....]`-style bar and no SGR color codes

### Requirement: git alias usage
`gw` SHALL work when invoked from a git shell alias (e.g. `git config alias.p '!gw push'`), forwarding the alias's arguments. Documentation SHALL state that aliases must use names that do not shadow built-in git commands, because git ignores such aliases.

#### Scenario: Alias with arguments
- **WHEN** alias `p` is `!gw push` and the user runs `git p origin feat`
- **THEN** `gw push origin feat` runs with formatted output
