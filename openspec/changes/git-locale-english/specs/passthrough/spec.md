## ADDED Requirements

### Requirement: English git messages for parsed output
When `gw` runs `push`, `pull`, `fetch`, `clone` or `merge` with formatted output, the git process SHALL print its messages in English whatever the user's locale, and SHALL keep the user's character set so non-ASCII file and branch names print unchanged. Commands `gw` runs as plain passthrough SHALL keep the user's locale.

#### Scenario: Ukrainian locale progress
- **WHEN** `LC_ALL=uk_UA.UTF-8` and the user runs `gw clone <url>`
- **THEN** the live progress line shows the clone stages and no progress line appears in the remote box

#### Scenario: German locale conflict
- **WHEN** `LANG=de_DE.UTF-8` and `gw merge feat` hits a conflict
- **THEN** the conflict is detected and shown as in an English locale

#### Scenario: Passthrough keeps locale
- **WHEN** `LC_ALL=uk_UA.UTF-8` and the user runs `gw diff --stat`
- **THEN** git's output is in Ukrainian, as with plain `git diff --stat`

#### Scenario: Non-ASCII names
- **WHEN** a merge under `LC_ALL=uk_UA.UTF-8` changes `файл.txt` with `core.quotePath=false`
- **THEN** the diffstat shows `файл.txt` unchanged
