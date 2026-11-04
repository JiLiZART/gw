# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

## [0.1.0] - 2026-11-04

### Added

- `gw push`, `pull`, `fetch`, `clone`: one live progress line, `remote:` messages in a titled box with clickable links, colored ref updates grouped per repository (submodules included), and a colored pull summary.
- `gw status`: grouped Conflicts / Staged / Changed / Submodules / Untracked lists with `+N -M` line counts and a Total line.
- `gw log`: full-screen commit browser with a branch graph, changed files, diffs, fuzzy search, branch / date / author filters, and commit actions.
- `gw branch`, `gw commit`, `gw stash list`: compact colored summaries.
- Plain `git` passthrough for non-terminal output, machine-readable flags, and every other subcommand; git's exit code is kept.
