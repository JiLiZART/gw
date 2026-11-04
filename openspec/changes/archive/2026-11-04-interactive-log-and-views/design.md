## Context

`gw` is a Rust workspace: `apps/gw` (dispatch, git runners), `crates/gw-parse` (pure parsers, no deps), `crates/gw-render` (ratatui widget + static ANSI output, with `sanitize` for untrusted text). The network commands use an inline viewport *without* raw mode so credential prompts keep working. `gw log` is the first full-screen, interactive mode. Motivation and scope: see `proposal.md`; behavior: see `specs/`.

## Goals / Non-Goals

**Goals:**
- Browser stays responsive on large histories (first screen fast, history streams in).
- All repository data comes from git plumbing / stable formats; parsing stays pure and fixture-tested.
- The terminal is always restored, even on panic or a failed git action.

**Non-Goals:**
- Mouse support, configurable keymaps or themes.
- Staging, committing, or editing from the browser; interactive rebase.
- A general diff viewer for `gw diff` / `gw show` (only the in-browser diff pane).
- Windows-specific terminal handling beyond what crossterm provides.

## Decisions

### D1. Full-screen mode: alternate screen + raw mode, with a guard
`gw log` enters the alternate screen and raw mode through ratatui's crossterm backend. A guard object restores both on drop, and a panic hook restores them before printing the panic. This differs from the network commands on purpose: the log browser needs key input, and git prompts never occur while browsing (actions suspend the TUI, see D6).

### D2. Data loading: `git log --graph` streamed from a reader thread
The list comes from one `git log --graph` command whose format separates fields with the unit separator (`%x1f`). `-z` is not used, because it does not combine well with graph lines:
`git log --graph --color=never --format=%x1f%H%x1f%h%x1f%at%x1f%an%x1f%D%x1f%s [scope] [filters] [user args]`.
Each output line is `<graph prefix><US-separated fields>`, or a graph-only connector line. A pure `parse_graph_line` splits the graph prefix from the fields. The prefix characters (`*|/\_ `) are kept: the renderer draws `*` as `●`, maps `|/\` to box-drawing characters, and colors each column by lane index. *Alternative:* compute the graph layout ourselves from parent hashes. Rejected: git's layout is correct for every edge case (octopus merges, `--date-order`), and reimplementing it is a large, bug-prone job.

A reader thread sends parsed rows over a channel; the UI appends them as they arrive and renders only the visible window. Changing a filter kills the running `git log` child and starts a new one. Memory grows with history size (about 200 B per commit). That is accepted (see Risks).

### D3. Sidebar and diff: on demand, cached
For the selected commit, `git show --numstat -z --format= --first-parent -M <sha>` runs (via `parse_numstat_z`, which already exists) plus `--name-status` for change codes. The result is cached per SHA (small LRU, 256 entries). Selection changes are debounced (~50 ms), so holding `↓` does not spawn hundreds of processes. The diff pane runs `git show --format= --first-parent -M --no-color --no-ext-diff <sha> -- <path>` and colors lines itself (`+` green, `-` red, `@@` dim), after `sanitize`, with tabs expanded.

### D4. Filters map to git arguments; search is client-side
| Filter | git argument |
|---|---|
| scope: current / all / branch | `HEAD` / `--all` / `<branch>` |
| date | `--since=<v>` / `--until=<v>` |
| author | `--author=<v>` |
| user args | appended unchanged (paths after `--`) |

Date input is validated by running `git rev-parse --since=<v>`, which prints `--max-age=<ts>`. git never fails here: unparseable text resolves to the current time. So a result within 2 s of now, for an input other than `now`/`today`, is reported as "date not understood". Branch modal: `git for-each-ref --format=%(refname:short) refs/heads refs/remotes`. Author modal: `git shortlog -sne <scope>`. Fuzzy search runs in memory over loaded rows. The matcher is a small built-in subsequence scorer: consecutive-run and word-start bonuses, query split on spaces, every term must match some field. *Alternative:* the `nucleo-matcher` crate. Rejected for now: a ~60-line scorer covers hash, author, and subject matching; revisit if quality is poor.

### D5. UI state machine
One `App` struct holds rows, selection/scroll per pane, focus (`List | Sidebar | Diff`), the active filters, and an optional overlay (`Search | BranchModal | DateModal | AuthorModal | ActionPanel | NameModal(Branch|Tag) | Confirm(action)`). Key handling dispatches on overlay first, then focus. Each context provides its hotkey list to the bottom bar, so the bar always matches what works. Rendering is a pure function of `App` into a ratatui `Frame`. That makes every screen testable with `TestBackend` snapshots.

### D6. Actions suspend the TUI
To run a git action, the guard leaves the alternate screen and raw mode, runs git with inherited stdio, and on return re-enters and redraws. This lets editors (revert/cherry-pick messages), hooks, and conflict output work normally. git's output stays visible in the normal screen. After git exits, `gw` always waits for Enter ("Press Enter to return to gw log"): git writes straight to the terminal, so `gw` cannot tell whether it printed anything worth reading. stderr is also teed through a pipe, so a failure is summarized in the status line. Then the log reloads. Branch and tag creation do not need the terminal; they run captured, so git's error can stay in the name modal. Commands:
- cherry-pick: `git cherry-pick <sha>`; revert: `git revert <sha>`
- checkout: `git checkout --detach <sha>`; new branch: `git branch <name> <sha>`
- reset: `git reset --soft|--mixed|--hard <sha>`; tag: `git tag <name> <sha>`
- copy: OSC 52 (`ESC]52;c;<base64>BEL`) written directly, no git.

The hard-reset confirmation lists files from `git status --porcelain=v2 -z --untracked-files=no` (already parsed by `parse_status_v2`).

### D7. Static views reuse the status style
- `gw branch`: `git for-each-ref --sort=-committerdate --format=... refs/heads` with `%(HEAD)`, `%(upstream:short)`, `%(upstream:track,nobracket)` (gives `ahead N, behind M` or `gone`), `%(committerdate:relative)`, and `%(subject)`. Rendered with the existing branch/upstream colors.
- `gw commit`: `git commit --quiet [args]` with inherited stdio (editors need a real terminal on stdout, so the output cannot be captured). Then `git log -1 --format=...` and `git show --numstat -z --format= HEAD` build the summary.
- `gw stash list`: `git stash list --format=%gd%x1f%cr%x1f%gs`, with branch and message split from `%gs` (`WIP on <branch>: ...` / `On <branch>: ...`), plus `git stash show --numstat stash@{N}` per stash.

### D8. Dispatch
`FORMATTED` grows to include `log`, `branch`, `commit`, `stash`. Each command checks its own fallback rule (non-TTY, format flags, arguments, subcommand) and calls `passthrough` when it applies, just as `status` already does.

## Risks / Trade-offs

- [Raw mode leaks if `gw` is killed with SIGKILL] → Unavoidable; `reset` fixes the terminal. Panics and normal errors are covered by the guard and panic hook.
- [Huge histories use memory proportional to the commit count] → About 20 MB for 100k commits; acceptable. A `-n` user argument limits it.
- [Graph prefix parsing depends on git's `--graph` characters] → Pinned with fixtures (linear, merge, octopus, `--all` with several branches). Unknown characters render as-is.
- [OSC 52 clipboard is disabled in some terminals or over tmux without `set-clipboard on`] → The status line still says "Copied" (the terminal gives no acknowledgement); the hint mentions the terminal setting. Accepted.
- [Fuzzy search only covers commits loaded so far] → The search header shows `searching N loaded commits` while loading continues.
- [Actions change the repository] → Reset always confirms, and hard reset lists the files at risk. Checkout warns about detached HEAD. Other actions are what the user explicitly chose and are reversible with git.
- [`gw commit` uses `--quiet`, which also hides the `create mode` / `rename` lines] → The summary's file and line counts replace them.

## Open Questions

- Default for `--date-order` vs git's default topological order in the graph: start with git's default and adjust after trying it on real repos. This affects ordering only, not the specs.
