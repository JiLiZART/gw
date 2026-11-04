## 1. Parsing (crates/gw-parse)

- [x] 1.1 Capture fixtures with `LC_ALL=C`: `git log --graph` in the D2 format for a linear history, a merge, an octopus merge, and `--all` with three branches; `git for-each-ref` branch output with ahead/behind/gone/no-upstream; `git stash list` with two stashes; `git shortlog -sne`; verify each file is non-empty
- [x] 1.2 Implement `parse_graph_line` (graph prefix + fields, or connector-only line, refs split from `%D`); verify insta snapshots over the graph fixtures
- [x] 1.3 Implement `parse_branches` (for-each-ref, including `gone`), `parse_stash_list` (index, age, branch, message from `%gs`), and `parse_shortlog`; verify snapshot tests on fixtures

## 2. Browser core (crates/gw-render + apps/gw)

- [x] 2.1 Add the terminal guard (alternate screen + raw mode, restored on drop and in a panic hook); verify by forcing a panic in a debug build that the terminal is restored
- [x] 2.2 Implement `App` state (rows, selection/scroll per pane, focus, filters, overlay) and key dispatch for navigation and `Tab` focus; verify unit tests for j/k/PgUp/PgDn/g/G and focus switching
- [x] 2.3 Render the commit list (graph with `●` nodes and per-lane colors, short SHA, age, author, ref chips, subject, selection highlight, sanitized text); verify `TestBackend` snapshots for linear and merge fixtures and an escape-in-subject case
- [x] 2.4 Stream `git log` rows from a reader thread into the app, restarting on filter change; verify that on a generated 100k-commit repo the first screen draws within 1 s
- [x] 2.5 Sidebar: on-demand numstat/name-status with debounce and LRU cache, change codes, `+N -M`/`bin`, total line, hidden under 100 columns until `Tab`; verify snapshots at 120 and 80 columns
- [x] 2.6 Diff pane: `Enter` opens the colored, sanitized, tab-expanded file diff, scrollable, `Esc` returns with the selection kept; verify a snapshot and a key-sequence test
- [x] 2.7 Context hotkey bar (list/sidebar/diff/modal/action panel), dropping keys from the right when narrow; verify snapshots per context
- [x] 2.8 `gw log` dispatch: TUI on a TTY, passthrough for non-TTY and format flags, user args appended; verify `gw log | head` equals `git log | head` and `gw log --oneline` passes through

## 3. Filters

- [x] 3.1 Scope toggle `a` (HEAD vs `--all`) and header showing scope and active filters; verify a key-sequence test that the git arguments change
- [x] 3.2 Branch select modal `b` (input + filtered list from for-each-ref); verify snapshot and selection test
- [x] 3.3 Fuzzy matcher (subsequence scoring, multi-term, match highlighting) and the `/` / `Ctrl-F` search overlay; verify unit tests for the hash prefix, author+subject, and no-match cases
- [x] 3.4 Date modal `d` (Since/Until, validated via `git rev-parse --since`), with an error shown in the modal; verify a valid range and an invalid input
- [x] 3.5 Author modal `u` (shortlog list + free input) and `x` to clear filters; verify listed and unlisted author selection

## 4. Actions

- [x] 4.1 Action panel on `Space` with keys p/v/o/n/r/y/t; verify a snapshot
- [x] 4.2 Suspend/resume around git actions, with reload and a status-line result; implement cherry-pick and revert; verify in a temp repo that a revert opening `GIT_EDITOR=true` lands, and a conflicting cherry-pick reports conflicts
- [x] 4.3 Checkout (detached, with confirmation), new branch and tag (name modals, git errors kept in the modal); verify in a temp repo, including an invalid tag name
- [x] 4.4 Reset soft/mixed/hard with confirmation; hard lists files at risk and needs `y`; verify `Esc` leaves the repo unchanged and a confirmed hard reset moves the branch
- [x] 4.5 Copy hash via OSC 52; verify the emitted bytes in a unit test

## 5. Static views

- [x] 5.1 `gw branch` view and passthrough rules; verify snapshot from fixtures (ahead/behind, gone, no upstream) and `gw branch -d x` passthrough
- [x] 5.2 `gw commit`: run `git commit --quiet [args]` with inherited stdio, then print the summary; passthrough for non-TTY and format flags; verify in a temp repo with `GIT_EDITOR=true` and `-m`, and that an aborted commit prints no summary and keeps git's exit code
- [x] 5.3 `gw stash list` view and passthrough for other stash forms; verify snapshot from fixtures and `gw stash pop` passthrough

## 6. Integration and docs

- [x] 6.1 Extend the integration test: `gw log` passthrough when piped, `gw branch`/`gw commit -m`/`gw stash list` on a pty with exit codes and no SGR under `NO_COLOR`; verify `cargo test --workspace` passes
- [x] 6.2 README: `gw log` keys (navigation, Tab, Enter/Esc, filters, Space actions), new commands, alias examples (`git config --global alias.l '!gw log'`); verify the commands match the implementation
- [ ] 6.3 After `colored-git-output` is archived, extend its `passthrough` dispatch requirement with log/branch/commit/stash (via `/opsx:update`); verify `openspec validate` passes
