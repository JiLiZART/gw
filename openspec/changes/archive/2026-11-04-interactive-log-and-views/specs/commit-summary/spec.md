## Purpose

Replaces git's post-commit lines with a compact summary of the commit that was just created.

## ADDED Requirements

### Requirement: Commit then summarize
`gw commit [args]` on a TTY SHALL run `git commit [args]` with the user's terminal (editor, hooks, prompts work normally) and git's own summary suppressed. On success it SHALL print one line `✓ <short-sha> <branch>  <subject>` and one line `<K> files  +N -M`, using the status colors. On failure it SHALL exit with git's exit code and print nothing extra.

#### Scenario: Successful commit
- **WHEN** the user commits 2 files with 10 added and 3 removed lines on `main` with subject `fix: x`
- **THEN** output ends with `✓ a1b2c3d main  fix: x` and `2 files  +10 -3`

#### Scenario: Aborted commit
- **WHEN** the user closes the editor with an empty message
- **THEN** git's abort message is shown and `gw` exits with git's code, with no summary

### Requirement: Commit passthrough
`gw commit` with stdout not a TTY, or with `--dry-run`, `--porcelain`, `--short`, or `--long`, SHALL run plain `git commit [args]`.

#### Scenario: Dry run
- **WHEN** the user runs `gw commit --dry-run`
- **THEN** the output is identical to `git commit --dry-run`
