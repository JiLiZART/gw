# gw

`gw` runs git's network commands and `status` and renders their output in a structured, colored form:

- one live progress line (spinner + green bar with stage, `cur/total`, `pct%`) instead of git's progress noise,
- `remote:` server messages in a titled box ("Merge request", "Pull request", or "remote") with clickable links,
- ref updates as a colored list under the remote target,
- `gw status` as a compact header (`branch -> upstream  ↑N ↓M`) plus Conflicts / Staged / Changed / Untracked groups.

Requires `git` ≥ 2.41 on `PATH`.

## Install

```sh
cargo install --path apps/gw
```

## Usage

```sh
gw push [git push args...]
gw pull [git pull args...]
gw fetch [git fetch args...]
gw clone [git clone args...]
gw status [pathspec...]
```

Arguments go to git unchanged. Any other subcommand (`gw log`, `gw diff`, ...) runs plain git.
`gw` always exits with git's exit code (`128 + signal` if git was killed, 127 if git is missing).

When stdout is not a terminal (pipes, scripts, CI), `gw` behaves exactly like plain `git`.
`gw status -s`, `--short`, `--porcelain`, `--long`, and `-z` also run plain `git status`.

## Git aliases

Use `gw` from git through shell aliases:

```sh
git config --global alias.p '!gw push'
git config --global alias.pl '!gw pull'
git config --global alias.f '!gw fetch'
git config --global alias.st '!gw status'

git p origin feat    # runs: gw push origin feat
```

Alias names must not shadow built-in git commands. git ignores an alias named `push`, `pull`, `status`, and so on.

## NO_COLOR

Set `NO_COLOR` to a non-empty value to turn off colors. The layout stays the same, the bar uses `#` and `.`, and links stay clickable.

## Layout

```
apps/gw          binary: dispatch, runs git, live progress line, exit code
crates/gw-parse  pure parsers for git output (tested against captured fixtures)
crates/gw-render progress bar widget and static ANSI output
```
