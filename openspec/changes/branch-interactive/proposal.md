## Why

`gw branch` prints a static list. To see what a branch would bring in, switch to it, or clean up old branches, the user runs several git commands per branch. A screen like `gw stash -i` puts the list, the changed files and the diff side by side, with the branch actions one key away.

## What Changes

- New `gw branch -i` (`--interactive`): a full-screen view with three panes: branches (local, then remote), the files the selected branch changed since it split from the current branch, and the selected file's diff. For the current branch the files pane shows the working tree changes instead.
- Actions: checkout (a remote branch gets a local tracking branch), new branch from the selected one, delete with confirmation (a remote branch is deleted on the server), rename (local), merge the selected branch into the current one.
- `gw branch` with other arguments, and bare `gw branch`, are unchanged; `-i` with other arguments is an error (exit 129), as for `gw stash -i`.

## Capabilities

### New Capabilities
- `branch-interactive`: the `gw branch -i` screen, its panes and actions.

### Modified Capabilities
- `branch-view`: "Branch passthrough" gains the `-i` exception.

## Impact

- `crates/gw-parse`: branch rows also carry the commit id and whether the ref is remote.
- `crates/gw-render`: new `branch` module (state machine + view), reusing stage's file rows and the stash screen's pane layout and diff drawing.
- `apps/gw`: new `branch_ui.rs` (git loaders and actions), dispatch in `views::branch`.
- README section, key table and screenshot.
