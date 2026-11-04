## Why

Most of `gw pull`'s output after the fetch is still plain git text. Only the diffstat `+`/`-` bars are colored. `Updating a..b`, `Fast-forward`, the `N files changed` summary, `create mode`/`delete mode` lines, and merge/rebase results look like raw git. Submodules are not handled at all:

- `gw status` lists a submodule as a file with a meaningless `+0 -0`.
- Pull and fetch print the submodule's fetch under one shared target, so the main repo's ref updates appear under the submodule's URL.
- Messages such as `Fetching submodule lib` and `Submodule path 'lib': checked out '<sha>'` fall through uncolored, some of them to stderr.

## What Changes

- **Pull summary.** Every recognized line after the fetch gets color:
  - The `Updating <old>..<new>` hashes: old dim, new bold.
  - The result lines: `Fast-forward`, `Merge made by …` and `Successfully rebased …` in green, `Already up to date.` dim.
  - Conflict and failure lines (`CONFLICT …`, `Automatic merge failed…`) in red.
  - In the diffstat, file paths in the default color, change bars green/red, and the counts in the `N files changed, X insertions(+), Y deletions(-)` summary green/red.
  - `create mode`, `delete mode` and `rename` lines with a colored verb and a dim mode.
- **One ref list per remote.** Pull, fetch and clone keep each fetched repository's ref updates under that repository's own `->` header. Today all updates share the last header seen. A submodule's header names the submodule (`-> submodule lib  <url>`).
- **Submodules section after pull and clone.** It replaces the raw `Submodule path 'lib': checked out '<sha>'` lines with `lib  checked out 5a1436f`, listing each updated submodule with its short commit hash.
- **No stray submodule lines on stderr.** `Fetching submodule <name>` lines are consumed rather than printed.
- **Submodules group in `gw status`.** Changed submodules move out of Staged and Changed into their own group, shown before Untracked:
  - Each line shows the recorded commit and the checked-out commit (`lib  256f977 → 5a1436f`) when they differ.
  - Each line also shows the state: `new commits`, `modified content`, `untracked content`, `staged`.
  - No `+N -M` stats on submodule lines. Submodules no longer count toward the Total line.

## Capabilities

### New Capabilities
<!-- none -->

### Modified Capabilities
- `ref-updates`: the pull summary requirement is broadened to color all recognized lines. Two requirements are added: ref updates grouped per fetched repository (submodule headers included), and a Submodules section for pull and clone.
- `status-view`: the group list gains a Submodules group. Line stats and the Total line exclude submodules. A new requirement covers how a submodule line is shown.

## Impact

- `crates/gw-parse`:
  - Parse submodule entries in `--porcelain=v2` (the `S<c><m><u>` field).
  - Parse `Submodule path '<p>': <verb> '<sha>'` and `Fetching submodule <p>` lines.
- `crates/gw-render`: a pull summary rewrite in `refs.rs`, grouped targets in `render_refs`, and a Submodules group in `status.rs`. Snapshot tests are updated.
- `apps/gw/src/main.rs`:
  - The network loop keeps a list of (target, refs) instead of a single target.
  - `status()` reads checked-out submodule commits with `git submodule status`, one extra git call made only when a submodule is listed.
- New fixtures: pull with a submodule, and status with a submodule. The integration test gets submodule cases.
- No new dependencies. Non-TTY passthrough is unchanged.
