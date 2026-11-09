## Context

`run` in `apps/gw/src/main.rs` collects git's stdout (`stdout_lines`) and unrecognized stderr (`others`), then prints `render_pull_summary` for pull/clone/merge and `render_messages` for the rest. `remote_box::render_box` draws a titled box but truncates long lines and derives its title from the remote host.

## Goals / Non-Goals

**Goals:** conflict box and hint box for merge and pull; reuse the box border style.

**Non-Goals:** hint boxes for push/fetch/clone; interactive conflict resolution; mapping each `CONFLICT` kind to custom advice.

## Decisions

- **Unmerged paths from git, not from text.** After merge/pull exit non-zero, run `git diff --name-only --diff-filter=U -z`. Empty → no box. This survives any message wording; the `Automatic merge failed` line is dropped only when the box is shown.
- **Operation from state, not text.** `rev-parse -q --verify MERGE_HEAD` → merge steps; `.git/rebase-merge` or `.git/rebase-apply` (`git rev-parse --git-path`) → rebase steps. Neither → list files only.
- **A small `titled_box(title, title_sgr, lines, width, color)` in gw-render** that wraps by display width (`unicode-width`, already a dependency) and shares the border glyphs with `remote_box`. `render_box` is left alone (its host title and link/branch highlighting are remote-specific).
- **Hint grouping inside `render_messages`** behind a flag (`box_hints: bool`), so the order of `error:`/`hint:`/`fatal:` lines is kept and other commands are unchanged.
- **Conflict box printed to stdout after the summary**, hint/error lines stay on stderr, matching where git writes them.

## Risks / Trade-offs

- [Extra git calls after a failed merge] → only on non-zero exit; two fast local commands.
- [Paths outside the cwd shown top-relative] → matches `git diff --name-only`; acceptable.
- [Pull's `--rebase` autostash or submodule conflicts print other lines] → they pass through unchanged; the box still lists unmerged paths.
