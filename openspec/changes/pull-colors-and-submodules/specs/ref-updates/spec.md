## MODIFIED Requirements

### Requirement: Pull merge/rebase summary passthrough
For `pull`, output that git prints after fetching (merge/rebase/fast-forward summary and diffstat) SHALL be printed after the ref list, with every recognized line colored:
- `Updating <old>..<new>`: old SHA dim, new SHA bold.
- `Fast-forward`, `Merge made by ...`, `Successfully rebased and updated ...`: green.
- `Already up to date.`: dim.
- `CONFLICT ...` and `Automatic merge failed...`: red.
- Diffstat file lines: path in the default color, count dim, `+` bars green, `-` bars red. `Bin` lines show the byte change dim.
- The `N files changed, X insertions(+), Y deletions(-)` line: the insertions part green, the deletions part red.
- `create mode`, `delete mode`, `rename`, and `mode change` lines: the verb green (create), red (delete), or yellow (rename, mode change); the file mode dim.

Unrecognized lines SHALL be printed unchanged. Untrusted text (paths, messages) SHALL be sanitized as for other output. Under NO_COLOR the text SHALL be identical to git's.

#### Scenario: Fast-forward pull diffstat
- **WHEN** pull prints ` src/a.rs | 4 ++--`
- **THEN** the path is shown and `++` is green and `--` is red

#### Scenario: Updating line
- **WHEN** pull prints `Updating 8910aeb..e698aeb`
- **THEN** `8910aeb` is dim and `e698aeb` is bold

#### Scenario: Summary counts
- **WHEN** pull prints ` 2 files changed, 2 insertions(+), 1 deletion(-)`
- **THEN** `2 insertions(+)` is green and `1 deletion(-)` is red

#### Scenario: Conflict
- **WHEN** a merging pull prints `CONFLICT (content): Merge conflict in a.txt`
- **THEN** the line is red

#### Scenario: Create mode
- **WHEN** pull prints ` create mode 100644 new.txt`
- **THEN** `create` is green, `100644` is dim, and `new.txt` is plain

## ADDED Requirements

### Requirement: Ref updates grouped per repository
When one command fetches from several repositories (for example a pull or fetch that also fetches submodules), each repository's ref updates SHALL be printed under that repository's own target header, in the order git reported them. A header for a submodule's fetch SHALL name the submodule: `-> submodule <path>  <url>`. `Fetching submodule <path>` lines SHALL NOT be printed. A submodule fetch with no ref changes SHALL print nothing.

#### Scenario: Pull with submodule fetch
- **WHEN** a pull fetches `main` from the superproject URL, then prints `Fetching submodule lib` and a `From <lib-url>` block with one update
- **THEN** the superproject header and its `main -> origin/main` line come first
- **AND** a `-> submodule lib  <lib-url>` header follows with its own update line
- **AND** no `Fetching submodule` line is printed

#### Scenario: Superproject updates not attributed to submodule
- **WHEN** the superproject has updates and the submodule fetch comes last
- **THEN** the superproject's updates are not listed under the submodule's header

### Requirement: Submodules section
After a pull or clone that updates submodule checkouts, the system SHALL print a "Submodules" header followed by one line per submodule, built from git's `Submodule path '<path>': <verb> '<sha>'` lines. Each line SHALL show the path (branch color), the verb (`checked out`, `merged in`, `rebased into`) in green, and the short (7-character) SHA in yellow. The raw git lines SHALL NOT be printed. Clone's `Submodule '<name>' (<url>) registered for path '<path>'` and `Cloning into '<dir>'...` lines for submodules SHALL NOT be printed.

#### Scenario: Pull updates a submodule
- **WHEN** pull prints `Submodule path 'lib': checked out '5a1436f8eac61d0b300ccb2b5d3a45cdc7ef8562'`
- **THEN** the output has a "Submodules" header and the line `lib  checked out 5a1436f`

#### Scenario: No submodule changes
- **WHEN** pull prints no `Submodule path` lines
- **THEN** no "Submodules" header is printed

#### Scenario: Recursive clone
- **WHEN** `gw clone --recurse-submodules` checks out submodule `lib`
- **THEN** the output shows the "Submodules" section with `lib  checked out <sha7>`
- **AND** no `registered for path` line is shown
