# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

## [0.4.0](https://github.com/JiLiZART/gw/compare/v0.3.0...v0.4.0) - 2026-11-14

### Added

- *(cli)* --git <path> flag and GW_GIT to pick the git binary
- *(merge)* conflict box and hint box for gw merge and gw pull
- interactive gw add and gw stash -i

### Fixed

- *(render)* box wrap no longer overflows on indentation wider than the box

## [0.3.0](https://github.com/JiLiZART/gw/compare/v0.2.0...v0.3.0) - 2026-11-06

### Added

- *(status)* interactive staging with gw status -i
- *(progress)* stage name before the bar, counters inside it

### Other

- screenshots for gw status -i, gw merge, and the new progress line

## [0.2.0](https://github.com/JiLiZART/gw/compare/v0.1.0...v0.2.0) - 2026-11-05

### Added

- colored gw merge output and git error/warning/hint messages

## [0.1.0] - 2026-11-04

### Added

- `gw push`, `pull`, `fetch`, `clone`: one live progress line, `remote:` messages in a titled box with clickable links, colored ref updates grouped per repository (submodules included), and a colored pull summary.
- `gw status`: grouped Conflicts / Staged / Changed / Submodules / Untracked lists with `+N -M` line counts and a Total line.
- `gw log`: full-screen commit browser with a branch graph, changed files, diffs, fuzzy search, branch / date / author filters, and commit actions.
- `gw branch`, `gw commit`, `gw stash list`: compact colored summaries.
- Plain `git` passthrough for non-terminal output, machine-readable flags, and every other subcommand; git's exit code is kept.
