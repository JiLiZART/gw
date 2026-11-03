## 1. Workspace setup

- [x] 1.1 Create root `Cargo.toml` workspace (`members = ["apps/*", "crates/*"]`, shared `[workspace.dependencies]`), `apps/gw` (bin), `crates/gw-parse` (lib, no deps), `crates/gw-render` (lib: `ratatui`, `throbber-widgets-tui`, `unicode-width`); add `target/` to `.gitignore`; verify `cargo build --workspace` succeeds
- [x] 1.2 Capture real git output fixtures into `crates/gw-parse/tests/fixtures/` (push fast-forward with GitLab MR message, push new branch, push rejected, push up-to-date, fetch porcelain, pull fast-forward with diffstat, clone, status v2 mixed/rename/detached/clean) using `LC_ALL=C` and `--progress`; verify each file is non-empty and contains `\r` where progress was emitted

## 2. Parsing (crates/gw-parse)

- [x] 2.1 Implement streaming splitter that yields lines on `\r` or `\n` across chunk boundaries with lossy UTF-8; verify unit tests for split-across-chunks and `\r\n`
- [x] 2.2 Implement line classifier → `Progress` (local and `remote:`), `Remote`, `Target`, `UpToDate`, `Other` per design D3; verify insta snapshots over the push and clone fixtures
- [x] 2.3 Implement `parse_push_porcelain` and `parse_fetch_porcelain` → `RefUpdate` with all kinds (fast-forward, forced, new, deleted, rejected+reason, up-to-date); verify snapshot tests on fixtures
- [x] 2.4 Implement `parse_human_ref_line` for pull/clone (`old..new  src -> dst`, `* [new branch]`, `+ old...new (forced update)`, `From <url>`); verify snapshot on pull fixture
- [x] 2.5 Implement `parse_status_v2` (branch.head/upstream/ab, ordinary `1`, rename `2`, unmerged `u`, untracked `?`) with `-z` input; verify snapshots on status fixtures

## 3. Rendering (crates/gw-render)

- [x] 3.1 Implement style helper honoring `NO_COLOR` (fixed color constants, OSC 8 link emitter, visible-width measuring that ignores escapes); verify unit tests that `NO_COLOR` output contains no SGR codes but does contain OSC 8
- [x] 3.2 Implement progress bar widget (spinner cell + `[label … cur/total pct%]`, green fill bg / dim bg, contrasting fg, `#`/`.` in NO_COLOR, label dropped under 40 cols, success/failure final mark); verify with ratatui `TestBackend` buffer assertions at 0%, 44%, 100%, failed, and width 30
- [x] 3.3 Implement remote box renderer (strip blank edges, thin border, title Merge request / Pull request / remote, URL truncation with `…` keeping full OSC 8 target, branch highlighting); verify insta snapshots for GitLab, GitHub, generic, and long-URL-at-80-cols cases, asserting all border lines have equal visible width
- [x] 3.4 Implement target + ref update list renderer with per-kind markers/colors and up-to-date single dim line; verify snapshots from 2.3/2.4 fixtures
- [x] 3.5 Implement pull summary renderer (diffstat `+` green / `-` red, other lines unchanged); verify snapshot on pull fixture
- [x] 3.6 Implement status renderer (header with ↑/↓, detached, no upstream; groups Conflicts/Staged/Changed/Untracked in order with counts; rename `old -> new`; "clean"; cwd-relative path rewrite given a prefix); verify snapshots on status fixtures including a non-empty prefix

## 4. CLI (apps/gw)

- [x] 4.1 Implement dispatch: no args → usage + non-zero exit; unknown subcommand or non-TTY stdout → plain `git` with inherited stdio; exit code propagation incl. `128+signal` and 127 when git is missing; verify by running `gw log -1`, `gw push --dry-run | cat` (identical to git), and `PATH= gw status` (exit 127)
- [x] 4.2 Implement network command runner: inject `--progress`, `--porcelain` (push/fetch), `LC_ALL=C`; reader threads → mpsc; 80ms tick loop; inline viewport on stdout without raw mode; lazy start on first `Progress`; final frame; then print box, target/refs, pull summary in order; verify against a local bare repo (`git init --bare`) that `gw push` shows bar, ref list, and correct exit code, and a rejected push exits 1 with red mark
- [x] 4.3 Implement `gw status`: run porcelain v2 + `rev-parse --show-prefix`, forward pathspecs, fall back to passthrough for `-s`/`--porcelain`/`--long`; verify in a temp repo with staged/changed/untracked files and from a subdirectory

## 5. Integration and docs

- [x] 5.1 Add integration test script/test that creates a temp repo + bare remote, exercises `gw push`, `gw fetch`, `gw pull`, `gw clone`, `gw status`, and asserts exit codes and that `NO_COLOR=1` output has no SGR codes; verify `cargo test --workspace` passes
- [ ] 5.2 Manually verify against the real GitLab remote: MR box titled "Merge request", link clickable in terminal, SSH passphrase prompt still usable; record result in the change notes
- [x] 5.3 Write `README.md`: install (`cargo install --path apps/gw`), usage, alias examples (`git config --global alias.p '!gw push'`), note that aliases cannot shadow built-in commands, `NO_COLOR`; verify the alias example works as written
