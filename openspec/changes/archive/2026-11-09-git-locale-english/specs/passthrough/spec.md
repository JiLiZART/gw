## ADDED Requirements

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
