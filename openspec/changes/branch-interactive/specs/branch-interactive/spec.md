## Purpose

`gw branch -i` lets the user browse local and remote branches, see what each branch changed against the current branch, and check out, create, delete, rename or merge branches without leaving the screen.

## ADDED Requirements

### Requirement: Launch and fallback
`gw branch -i` (or `--interactive`) alone with stdin and stdout on a TTY SHALL open a full-screen view and restore the terminal on exit, on error and on panic. With stdout not a TTY it SHALL run plain `git branch -i` (git's own error and exit code). Outside a repository git's error SHALL be printed and its exit code returned.

#### Scenario: Piped
- **WHEN** the user runs `gw branch -i | cat`
- **THEN** the output and exit code equal `git branch -i | cat`

### Requirement: Layout
The screen SHALL show three panes side by side: branches, files, diff, with a hotkey bar and a status line at the bottom. `Tab`/`Shift-Tab` SHALL move focus between panes. Under 80 columns only the focused pane SHALL be drawn.

#### Scenario: Narrow terminal
- **WHEN** the terminal is 60 columns wide and focus is on the files pane
- **THEN** only the files pane is drawn

### Requirement: Branch list
The branch pane SHALL list local branches under a `Local` header, current branch first and marked `*`, others by most recent commit; then remote-tracking branches under a `Remote` header by most recent commit, without `<remote>/HEAD`. Each row SHALL show the name (current bold), ahead/behind against its upstream (`↑N ↓M`, local only) or `gone`, and the last commit's age. The status line SHALL show the selected branch's upstream, last commit subject and age. With no branches (unborn repository) the pane SHALL show `no branches yet`.

#### Scenario: Local and remote
- **WHEN** the repository has `main` (current), `feat`, and remote branches `origin/main`, `origin/fix`
- **THEN** the pane shows `Local`, `* main`, `feat`, then `Remote`, `origin/main`, `origin/fix`, and no `origin/HEAD`

### Requirement: Changed files
For a selected branch other than the current one, the files pane SHALL list the files changed between the merge-base of `HEAD` and that branch, and the branch tip, each with its status letter and `+N -M` line counts (`bin` for binary), titled `<branch> vs <current>`. When the branch has nothing beyond the merge-base the pane SHALL show `nothing new against <current>`. For the current branch the pane SHALL list the working tree changes (staged, unstaged and untracked) like `gw status`, titled `working tree`. With a detached `HEAD`, `<current>` SHALL read `HEAD`.

#### Scenario: Feature branch
- **WHEN** `feat` split from `main` and since then changed `src/a.rs` (+10 -1) and added `new.rs`, and `main` is current
- **THEN** the files pane titled `feat vs main` lists `M src/a.rs +10 -1` and `A new.rs`

#### Scenario: Current branch
- **WHEN** the current branch is selected and `b.txt` is modified, not staged
- **THEN** the files pane titled `working tree` lists `b.txt`

#### Scenario: Merged branch
- **WHEN** `old` is fully contained in the current branch
- **THEN** the files pane shows `nothing new against main`

### Requirement: Diff pane
The diff pane SHALL show the selected file's diff for the comparison of the files pane (merge-base to branch tip, or working tree against `HEAD`; an untracked file as all added), colored like the other diff panes, scrollable when focused.

#### Scenario: Diff of branch file
- **WHEN** `feat vs main` is shown and `src/a.rs` is selected
- **THEN** the diff pane shows the changes `feat` made to `src/a.rs` since the merge-base

### Requirement: Checkout
`Enter` on a branch row SHALL switch to it: a local branch with `git switch <name>`; a remote branch `<remote>/<name>` with `git switch --track <remote>/<name>` when no local `<name>` exists, otherwise with `git switch <name>` and the status line `switched to existing local <name>`. On failure the status line SHALL show git's first error line and nothing else SHALL change. After success the list SHALL reload with the new current branch selected.

#### Scenario: Dirty tree blocks checkout
- **WHEN** local changes would be overwritten by switching to `feat`
- **THEN** the status line shows git's `error:` line and the current branch is unchanged

#### Scenario: Remote branch
- **WHEN** the user presses `Enter` on `origin/fix` and no local `fix` exists
- **THEN** a local `fix` tracking `origin/fix` is created and becomes current

### Requirement: New branch
`n` SHALL open a name prompt; `Enter` with a name SHALL run `git switch -c <name> <selected>` (for a remote branch, the new branch does not track it); `Esc` SHALL cancel; an empty name SHALL do nothing. git's error (invalid or existing name) SHALL appear in the status line.

#### Scenario: Branch from feat
- **WHEN** `feat` is selected and the user types `n`, `feat-2`, `Enter`
- **THEN** `feat-2` is created at `feat`'s tip and becomes current

### Requirement: Delete
`d` SHALL ask `Delete branch <name>? y/n`; `y` SHALL run `git branch -d <name>`, any other key cancels. When git refuses because the branch is not fully merged, a second prompt `<name> is not fully merged. Force delete? y/n` SHALL run `git branch -D <name>` on `y`. On a remote branch the prompt SHALL be `Delete <remote>/<name> on the server? y/n` and `y` SHALL run `git push <remote> --delete <name>`, showing `deleting…` while it runs. Deleting the current branch SHALL be refused with a status message. Before deleting, the branch SHALL still point to the commit shown; otherwise `branch changed, nothing done` SHALL be shown.

#### Scenario: Unmerged branch
- **WHEN** the user deletes `wip`, which has unmerged commits, and answers `y` twice
- **THEN** `wip` is deleted

#### Scenario: Cancel
- **WHEN** the user presses `d` then `n`
- **THEN** nothing is deleted

#### Scenario: Remote delete
- **WHEN** the user presses `d` on `origin/old` and answers `y`
- **THEN** `git push origin --delete old` runs and the list reloads without `origin/old`

### Requirement: Rename
`R` on a local branch SHALL open a prompt filled with the current name; `Enter` SHALL run `git branch -m <old> <new>`. On a remote branch `R` SHALL show `rename works on local branches`.

#### Scenario: Rename
- **WHEN** the user renames `feat` to `feature`
- **THEN** the list shows `feature` selected and no `feat`

### Requirement: Merge into current
`m` on a branch other than the current one SHALL ask `Merge <branch> into <current>? y/n`. On `y` the screen SHALL close, restore the terminal, and run `gw merge --no-edit <branch>` with its normal formatted output; `gw` SHALL exit with merge's exit code. `m` on the current branch SHALL show `cannot merge a branch into itself`.

#### Scenario: Merge with conflict
- **WHEN** the user merges `feat` and it conflicts
- **THEN** the screen closes, the merge output (with its conflict box) is printed, and the exit code is 1

### Requirement: Keys and display
`↑`/`↓`/`j`/`k`, `PgUp`/`PgDn`, `Home`/`End` SHALL move in the focused pane; `Ctrl-R` SHALL reload; `q` and `Esc` (outside prompts) SHALL quit. The hotkey bar SHALL list `Enter`, `n`, `d`, `R`, `m`, `Tab`, `^R`, `q`. Branch names, subjects and file names SHALL be sanitized; with `NO_COLOR` set no color SHALL be used and selection SHALL be reverse video.

#### Scenario: Escape in a name
- **WHEN** a commit subject contains `ESC[2J`
- **THEN** it is shown escaped and the screen is not cleared
