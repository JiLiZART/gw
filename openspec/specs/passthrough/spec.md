# passthrough Specification

## Purpose

Defines how `gw` dispatches subcommands to the real git, keeps git's semantics (arguments, prompts, exit codes), and degrades safely in non-interactive or colorless environments.

## Requirements

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
When the `NO_COLOR` environment variable is set to a non-empty value, `gw` SHALL keep its structured layout (progress line, box, grouped lists) but emit no SGR color or style codes (bare reset sequences such as `ESC[0m` from the bar renderer are allowed); the progress bar SHALL use `#` for filled and `.` for empty cells. Hyperlinks SHALL still be emitted.

#### Scenario: NO_COLOR push
- **WHEN** `NO_COLOR=1 gw push` runs on a TTY
- **THEN** output contains the bordered box and a `[####....]`-style bar and no SGR color codes

### Requirement: git alias usage
`gw` SHALL work when invoked from a git shell alias (e.g. `git config alias.p '!gw push'`), forwarding the alias's arguments. Documentation SHALL state that aliases must use names that do not shadow built-in git commands, because git ignores such aliases.

#### Scenario: Alias with arguments
- **WHEN** alias `p` is `!gw push` and the user runs `git p origin feat`
- **THEN** `gw push origin feat` runs with formatted output

### Requirement: Unrecognized git messages
stderr lines from git that `gw` does not recognize (e.g. `error:`, `hint:`, `Cloning into ...`) SHALL be printed unchanged to stderr, after all formatted sections.

#### Scenario: Rejected push hints
- **WHEN** git push is rejected and prints `error:` and `hint:` lines
- **THEN** those lines appear unchanged after the ref list

### Requirement: English git messages for parsed output
When `gw` runs `push`, `pull`, `fetch`, `clone` or `merge` with formatted output, the git process SHALL print its messages in English whatever the user's locale (`LC_ALL`, `LANG`, `LANGUAGE`), and non-ASCII file and branch names SHALL print unchanged. Commands `gw` runs as plain passthrough SHALL keep the user's locale.

#### Scenario: Ukrainian locale conflict
- **WHEN** `LC_ALL=uk_UA.UTF-8 LANGUAGE=uk` and `gw merge feat` hits a conflict in `f`
- **THEN** the output contains `CONFLICT (content): Merge conflict in f` and the exit code is 1

#### Scenario: Passthrough keeps locale
- **WHEN** `LC_ALL=uk_UA.UTF-8` and the user runs a passthrough command (e.g. an alias `!echo "[$LC_ALL]"`)
- **THEN** it sees `LC_ALL=uk_UA.UTF-8`

#### Scenario: Non-ASCII names
- **WHEN** a `gw merge` under `LC_ALL=uk_UA.UTF-8` conflicts in `файл.txt`
- **THEN** the output shows `файл.txt` unchanged
