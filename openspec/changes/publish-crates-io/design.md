## Context

The workspace has one binary package (`apps/gw`, package name `gw`) and two libraries (`crates/gw-parse`, `crates/gw-render`) linked by path dependencies, with shared `version`, `edition`, and `license` in `[workspace.package]`. On crates.io, `gw` is taken; `git-gw`, `gw-parse`, and `gw-render` are free (checked 2026-11-03). Dependencies set the toolchain floor: `ratatui` 0.30 and `throbber-widgets-tui` 0.11 declare `rust-version = "1.88.0"`. The local toolchain is cargo 1.100, which supports `cargo publish --workspace`.

## Goals / Non-Goals

**Goals:**
- One command to publish all three crates in the right order, with a dry run first.
- Keep the `apps/` + `crates/` layout and the `gw` command name.

**Non-Goals:**
- CI-driven releases, prebuilt binaries (`cargo binstall`, Homebrew, GitHub Releases).
- A stable public API for `gw-parse` / `gw-render`: they are published only because `git-gw` needs them, and their docs say so.
- Renaming the `apps/gw` directory.

## Decisions

### D1. Package `git-gw`, binary `gw`
`apps/gw/Cargo.toml` sets `name = "git-gw"` and declares `[[bin]] name = "gw", path = "src/main.rs"`. The integration test keeps using `CARGO_BIN_EXE_gw`, because that name follows the bin target, not the package. *Alternative:* a package and binary both named `git-gw`. Rejected: it changes the command users type, and git would also expose it as `git gw`, which the docs do not cover.

### D2. Publish all three crates with one shared version
The version stays in `[workspace.package]` and every crate inherits it. Path dependencies get a version, so cargo uses the path locally and the registry version in published crates: `gw-parse = { path = "crates/gw-parse", version = "=0.1.0" }`. The exact `=` pin keeps the three crates in lockstep: a `git-gw` release never mixes with other library versions. *Alternative:* fold the libraries into the binary crate. Rejected by the user to keep the workspace split.

### D3. Metadata in `[workspace.package]`
`license`, `repository`, `rust-version = "1.88"`, `homepage`, and `authors` move to `[workspace.package]` and are inherited with `field.workspace = true`. Each crate sets its own `description`. `git-gw` also sets `readme = "../../README.md"`, `keywords = ["git", "cli", "progress", "terminal", "status"]`, and `categories = ["command-line-utilities", "development-tools"]`. The libraries' descriptions state that they are internal to `git-gw`.

### D4. Publish with `cargo publish --workspace`
`cargo publish --workspace --dry-run` packages and verifies all three crates in dependency order without uploading. The real run is `cargo publish --workspace`. Cargo orders the uploads (`gw-parse`, `gw-render`, `git-gw`) and waits for each to appear in the index before the next. *Alternative:* three manual `cargo publish -p` calls. That works too and is the documented fallback for older cargo, but it is easier to get the order wrong.

### D5. Package contents
Each crate is in its own directory, so its package contains only that directory. Cargo copies the file named by `readme` into the package even when it lives outside, so `git-gw` uses `readme = "../../README.md"`. `include` cannot reach outside a crate, so each crate directory gets a `LICENSE` symlink to the root file; cargo packages the target's contents. The MIT terms require the license text to ship with the code. `gw-parse` keeps its `tests/fixtures` (about 20 KB) so `cargo test` works from the published source. `cargo package --list -p <crate>` verifies that no `openspec/`, `.claude/`, `.remember/`, or `target/` paths are included.

### D6. Release procedure in `RELEASING.md`
A short `RELEASING.md` in the repository root lists the steps: bump `[workspace.package].version` and the three `=` pins, run `cargo test --workspace`, run `cargo publish --workspace --dry-run`, run `cargo publish --workspace`, then `git tag v<version>` and push the tag.

## Risks / Trade-offs

- [Publishing is permanent: a version cannot be deleted or re-uploaded, only yanked] → Always dry-run first. The maintainer runs the real publish personally after review; tasks stop before it.
- [A failed upload midway leaves some crates at the new version] → Rerun `cargo publish --workspace`; already-published crates are skipped with an error, then publish the remaining ones with `-p`.
- [Someone registers `git-gw`, `gw-parse`, or `gw-render` before the first publish] → Recheck availability right before publishing; if taken, pick another name and update the spec.
- [The exact `=` pins force all three crates to release together, even for a binary-only fix] → Accepted: the libraries are internal, lockstep releases are simpler.
- [Library crates appear on crates.io as if they were general-purpose] → Their descriptions and docs say they are internal to `git-gw`.

## Migration Plan

Users who installed from a checkout (`cargo install --path apps/gw`) have a package named `gw` installed. After this change, installing `git-gw` places a second package that also provides the `gw` binary, and cargo refuses because of the conflict. The README tells them to run `cargo uninstall gw` first. Rollback for a bad release: `cargo yank --version <v>` on the affected crates.
