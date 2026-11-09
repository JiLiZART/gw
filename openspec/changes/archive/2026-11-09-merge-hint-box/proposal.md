## Why

After a failed `gw merge` the important part (which files conflict, what to run next, git's `hint:` advice) is plain text mixed with other lines. A box makes it stand out, as the remote box does for server messages.

## What Changes

- On a merge conflict (`gw merge`, and `gw pull` when its merge or rebase stops on conflicts) a red-titled **Merge conflict** box lists the conflicted files and the commands to continue or abort. It replaces git's `Automatic merge failed; …` line.
- Consecutive `hint:` lines from `merge` and `pull` are shown in a dim-titled **hint** box, `hint: ` prefix removed, long lines wrapped to the terminal width. `error:`/`fatal:` lines stay outside the box, in their place and colors.
- Other commands keep printing `hint:` lines as before.
- Relies on the English git output that `gw` already forces for merge and pull (spec: `git-locale-english`).

## Capabilities

### New Capabilities
- `merge-output`: conflict box and hint box for `gw merge` and `gw pull`.

### Modified Capabilities

## Impact

- `crates/gw-render`: a titled, wrapping box renderer (beside `remote_box`), a conflict box and hint grouping.
- `apps/gw/src/main.rs`: after merge/pull exit non-zero, read the conflicted paths and in-progress operation.
