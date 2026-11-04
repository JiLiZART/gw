# gw

`gw` runs git's network commands and `status` and renders their output in a structured, colored form:

- one live progress line (spinner + green bar with stage, `cur/total`, `pct%`) instead of git's progress noise,
- `remote:` server messages in a titled box ("Merge request", "Pull request", or "remote") with clickable links,
- ref updates as a colored list under the remote target,
- `gw status` as a compact header (`branch -> upstream  ↑N ↓M`) plus Conflicts / Staged / Changed / Untracked groups,
- `gw log` as a full-screen commit browser with a branch graph, changed files, diffs, filters, and commit actions,
- `gw branch`, `gw commit`, and `gw stash list` as compact summaries.

Requires `git` ≥ 2.41 on `PATH`.

## Install

```sh
cargo install git-gw --locked
```

The package is `git-gw` (the name `gw` is taken on crates.io); the installed command is `gw`.
Requires Rust 1.88 or newer.

From source:

```sh
git clone https://github.com/JiLiZART/gw.git
cd gw
cargo install --path apps/gw --locked
```

If you installed an earlier build from source, remove it first, or cargo refuses because both provide `gw`:

```sh
cargo uninstall gw
```

## Usage

```sh
gw push [git push args...]
gw pull [git pull args...]
gw fetch [git fetch args...]
gw clone [git clone args...]
gw status [pathspec...]
gw log [revisions] [-- paths]
gw branch
gw commit [git commit args...]
gw stash list
```

Arguments go to git unchanged. Any other subcommand (`gw diff`, `gw show`, ...) runs plain git.
`gw` always exits with git's exit code (`128 + signal` if git was killed, 127 if git is missing).

When stdout is not a terminal (pipes, scripts, CI), `gw` behaves exactly like plain `git`.
`gw status -s`, `--short`, `--porcelain`, `--long`, and `-z` also run plain `git status`.
`gw log` with an output-format flag (`--oneline`, `--format`, `--pretty`, `-p`, `--stat`, `--graph`, ...) runs plain `git log`.
`gw branch` with any argument, `gw commit --dry-run`/`--porcelain`/`--short`/`--long`, and every `gw stash` form except `list` run plain git.

## gw log

A full-screen commit browser: the commit list with a branch graph on the left, the selected commit's changed files on the right.

| Keys | Action |
|---|---|
| `j`/`k`, `↑`/`↓`, `PgUp`/`PgDn`, `g`/`G` | Move |
| `Tab` | Switch between commits and files |
| `Enter` (on a file) | Show its diff; `Esc` goes back |
| `/` or `Ctrl-F` | Fuzzy search by hash, subject, author, or ref; `Enter` keeps it, `Esc` clears it |
| `a` | Toggle current branch / all branches |
| `b` | Pick a branch |
| `d` | Date range (Since / Until, e.g. `2 weeks ago`, `2026-10-01`) |
| `u` | Pick an author, or type any author pattern |
| `x` | Clear all filters |
| `Space` | Actions on the selected commit |
| `q`, `Ctrl-C` | Quit |

Actions (`Space`, then a key): `p` cherry-pick, `v` revert, `o` checkout (detached), `n` new branch here, `r` reset (`s`oft / `m`ixed / `h`ard), `y` copy hash, `t` tag.
Cherry-pick, revert, checkout, and reset hand the terminal to git, so editors and conflict messages work; press Enter to come back.
Every reset asks for confirmation, and a hard reset lists the files whose uncommitted changes would be lost.
Copying uses the terminal clipboard (OSC 52); in tmux this needs `set -g set-clipboard on`.

## Git aliases

Use `gw` from git through shell aliases:

```sh
git config --global alias.p '!gw push'
git config --global alias.pl '!gw pull'
git config --global alias.f '!gw fetch'
git config --global alias.st '!gw status'
git config --global alias.l '!gw log'
git config --global alias.br '!gw branch'
git config --global alias.ci '!gw commit'

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
