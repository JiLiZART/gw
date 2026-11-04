## Purpose

Defines the actions the user can run on the selected commit from the `gw log` action panel, and the confirmations that protect against losing work.

## ADDED Requirements

### Requirement: Action panel
`Space` SHALL open an action panel along the bottom of the screen listing the actions for the selected commit with their keys: `p` cherry-pick, `v` revert, `o` checkout, `n` new branch, `r` reset, `y` copy hash, `t` tag. `Esc` SHALL close it.

#### Scenario: Open panel
- **WHEN** the user presses `Space` on commit `787e7fa`
- **THEN** the bottom panel shows the actions with their keys and names the commit

### Requirement: Running git actions
Cherry-pick, revert, checkout, new branch, reset, and tag SHALL run the corresponding git command. While it runs, the browser SHALL hand the terminal to git, so an editor, hook output, or conflict message works normally, then return to the browser, reload the history, and show git's result (success, or the error and its first lines of output) in the status line.

#### Scenario: Revert opens editor
- **WHEN** the user reverts a commit and git opens the commit-message editor
- **THEN** the editor works normally, and after saving the browser shows the new revert commit at the top

#### Scenario: Cherry-pick conflict
- **WHEN** a cherry-pick stops with a conflict
- **THEN** the status line says the cherry-pick stopped with conflicts and names `git cherry-pick --continue` / `--abort`

### Requirement: Branch and tag names via modal
New branch and tag SHALL open a centered modal with a name input. An invalid or existing name SHALL show git's error in the modal and keep it open.

#### Scenario: Create tag
- **WHEN** the user presses `t`, types `v0.2.0`, and presses `Enter`
- **THEN** a tag `v0.2.0` points at the selected commit and appears as a chip on it

### Requirement: Reset with confirmation
`r` SHALL offer soft, mixed, and hard reset of the current branch to the selected commit. Every reset SHALL ask for confirmation in a modal naming the branch and the commit. A hard reset SHALL also list the tracked files with uncommitted changes (staged or not) that would be lost, and require typing `y` to proceed; untracked files are not affected and are not listed.

#### Scenario: Hard reset with local changes
- **WHEN** the user chooses hard reset while `a.rs` has uncommitted changes
- **THEN** the modal warns that changes in `a.rs` will be lost and nothing happens unless the user confirms

#### Scenario: Reset cancelled
- **WHEN** the user presses `Esc` in the reset confirmation
- **THEN** the repository is unchanged

### Requirement: Checkout warning
Checkout SHALL warn in a confirmation modal that it leaves HEAD detached. If git refuses because of local changes, the status line SHALL show git's reason.

#### Scenario: Detached checkout
- **WHEN** the user confirms checkout of `787e7fa`
- **THEN** HEAD is detached at `787e7fa` and the header shows `detached at 787e7fa`

### Requirement: Copy hash
`y` SHALL copy the full commit hash to the system clipboard through the terminal (OSC 52) and show `Copied <short-sha>` in the status line.

#### Scenario: Copy
- **WHEN** the user presses `y` on `a22b7ed`
- **THEN** the full 40-character hash is sent to the clipboard and the status line confirms it
