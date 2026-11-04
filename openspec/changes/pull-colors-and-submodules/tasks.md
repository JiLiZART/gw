## 1. Fixtures

- [x] 1.1 Capture fixtures (fixed author/committer env, `LC_ALL=C`, `protocol.file.allow=always`) from a superproject with submodule `lib`. Save them in `crates/gw-parse/tests/fixtures/`:
  - pull with `submodule.recurse=true`: stderr and stdout, separately
  - pull with a merge, and pull with a conflict: stdout
  - `clone --recurse-submodules`: stderr and stdout
  - `status --porcelain=v2 --branch -z`: one submodule with new commits, one with modified content, one with a staged pointer

  Verify the files exist and contain `Fetching submodule`, `Submodule path`, and `S.M.`/`SC..`.

## 2. Parsing (gw-parse)

- [x] 2.1 Add `Event::SubmoduleFetch(path)` to `classify` for `Fetching submodule <p>`. Verify with a unit test.
- [x] 2.2 Add `classify_pull_line`, covering:
  - `Updating`
  - the fast-forward, merge, rebased, up-to-date, conflict and failed lines
  - diffstat, `Bin` and summary lines
  - `create`/`delete`/`rename`/`mode change` lines
  - `Submodule path`, plus the clone noise lines
  - anything else

  Each variant keeps its original line. Verify with a fixture test over the pull/merge/conflict/clone stdout captures: every line is classified, and no line in those fixtures is `Other` except known ones.
- [x] 2.3 Parse the porcelain v2 `<sub>` field and `hH`/`hI` into `Entry`. Verify with a fixture test on the status capture: `lib` has `commit_changed`, the modified one has `modified`, and the staged one has `head_oid != index_oid`.
- [x] 2.4 Add a parser for `git submodule status` lines (`[ +-U]<sha> <path> (<desc>)`), returning path to SHA. Verify with a unit test, including a `-` (uninitialized) line.

## 3. Rendering (gw-render)

- [x] 3.1 Rewrite `render_pull_summary` to use `classify_pull_line`, coloring each line as the ref-updates spec says. Collect `Submodule` lines into a "Submodules" section printed last, and drop the noise lines. Verify:
  - snapshot tests for the colored output
  - a test that NO_COLOR output equals the input lines, minus the submodule lines, which become the section
- [x] 3.2 Change `render_refs` to take grouped targets with an optional submodule label: `-> submodule lib  <url>`. Skip label groups that have no refs. Verify with a snapshot of the superproject group plus the submodule group.
- [x] 3.3 Add the Submodules group to `render_status`. It sits between Changed and Untracked and carries the commit and state labels. Submodule lines have no stats, and Total is omitted when nothing has stats. Verify with snapshot tests for the three spec scenarios (new commits, modified content, staged) and the "only a submodule changed" case.

## 4. Wiring (apps/gw)

- [x] 4.1 Replace `target`/`refs` in the network loop with grouped targets. Pending submodule labels are consumed by the next `Target` and dropped at the next `SubmoduleFetch` or at end of stream. Push porcelain and fetch porcelain fill the same groups, and `all_up_to_date` and the remote-box branch list cover all groups. Verify: `cargo test`, and existing push/fetch integration assertions still pass.
- [x] 4.2 For clone, route stdout through the pull-summary renderer so the Submodules section and noise filtering apply. Drop `Cloning into '<sub>'` stderr lines for submodule paths. Verify with the integration test in 5.1.
- [x] 4.3 In `status()`:
  - Pull submodule entries out of Staged/Changed. Conflicted submodules stay in Conflicts.
  - When an entry has `commit_changed`, run `git -C <toplevel> submodule status -- <paths>` once for the current SHAs.
  - Pass the submodule entries to the renderer.

  Verify with the integration test in 5.1.

## 5. Tests and docs

- [x] 5.1 Extend `apps/gw/tests/integration.rs` with a submodule case: a superproject and a submodule remote, a recursive clone, then an upstream submodule bump. Check, all under NO_COLOR with no SGR:
  - `gw pull` with `submodule.recurse=true` shows the superproject's ref under its own header, a `-> submodule lib` header, and `Submodules` plus `lib  checked out <sha7>`, and never `Fetching submodule`.
  - `gw status` after editing inside `lib` shows `Submodules (1)` and `modified content`, with no `+0 -0`.
  - The clone output shows the Submodules section.

  Verify `cargo test --workspace` passes on macOS and in Docker `rust:1-trixie`.
- [x] 5.2 Run `cargo clippy --workspace --all-targets -- -D warnings`. Verify it is clean, fixing pre-existing collapsible-if and unit-arg findings too. (`cargo fmt --check` already fails across the repo on HEAD, so it is out of scope here.)
- [x] 5.3 Update README: in the feature list, mention the colored pull summary and submodule display. Verify by reading the README diff.
- [ ] 5.4 Manual check in a real terminal: run `gw pull` in a repo with submodules and `gw status` with a dirty submodule. Verify the colors read clearly on a dark theme.
