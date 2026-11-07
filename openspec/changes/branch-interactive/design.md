## Context

`gw stash -i` (change `interactive-add-stash`) built the pattern this screen copies: a pure state machine in `gw_render::stash` (`App::key -> Effect`), drawing in `stash::view` (three panes, one pane under 80 columns, modal prompts), git I/O in `apps/gw/src/stash_ui.rs`. File rows and stats come from `gw_render::stage` (`item_line`, `stat_text`), diffs from `stage.rs::load_diff`. Branch rows come from `gw_parse::parse_branches` with `BRANCH_FORMAT`.

## Goals / Non-Goals

**Goals:** the spec's screen and actions with the least new code, by reusing stash/stage pieces.

**Non-Goals:** fetch/push from the screen, upstream editing, branch filtering/search, tags, worktrees, comparing two arbitrary branches.

## Decisions

- **Module shape mirrors stash.** `gw_render::branch::{App, Effect, view}` and `apps/gw/src/branch_ui.rs`. Effects: `Quit, Reload, Checkout(Branch), Create{name, from}, Delete{branch, force}, DeleteRemote(Branch), Rename{from, to}, Merge(Branch)`. Shared pane layout and diff drawing are lifted from `stash::view` into `pub(crate)` helpers instead of copied.
- **Data.** One `for-each-ref --sort=-committerdate refs/heads refs/remotes` with a format that adds `%(objectname)` and the full refname; `parse_branches` gains `oid` and `remote: bool` (from `refs/remotes/`), and drops `*/HEAD`. `gw branch` output stays the same (snapshot unchanged).
- **Files of a branch.** `git diff --name-status -z <base> <tip>` and `--numstat -z`, base from `git merge-base HEAD <tip>` (no base → empty-tree diff). Three-dot `HEAD...tip` would work too but the explicit base is reused by the diff pane. Current branch reuses `stage.rs` status loading (`Repo::status`, `add_items`-style rows without checkboxes).
- **Loading on selection, cached per oid**, like `stash` `files_wanted`/`set_files`, so moving quickly does not re-run git for the same branch.
- **Guard by oid.** Before delete/rename, `rev-parse -q --verify <ref>` must equal the shown oid (same as stash's guard). `git branch -d` keeps its own merged check; its refusal (exit 1 with `not fully merged`) triggers the force prompt. Detect by exit code plus `LANGUAGE=en` on these calls, so the check is not locale-bound.
- **Merge leaves the TUI.** `Effect::Merge` restores the terminal and calls the existing formatted merge path (`run("merge", ["--no-edit", b])`), so merge output, conflict box and hints are the same as `gw merge`. Avoids drawing merge results inside the TUI.
- **Remote delete** runs `git push <remote> --delete <name>` with captured output (no live progress), drawing `deleting…` first; failure shows git's first error line.

## Risks / Trade-offs

- [Big repos with many remote branches: `for-each-ref` is fast, but `merge-base` + diff per selection can take time] → loaded on selection only and cached; no precomputation.
- [`git switch` with a dirty tree may carry changes over silently] → that is git's behavior; status line shows the result.
- [Remote delete is outward-facing] → explicit `y` with the server named in the prompt; never batch.
- [Hooks (`post-checkout`) print to the terminal under the TUI] → output captured; first error line shown, rest dropped. Accepted.
