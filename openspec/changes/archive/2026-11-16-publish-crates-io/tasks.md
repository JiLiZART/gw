## 1. Package metadata

- [x] 1.1 Add `LICENSE` (MIT, holder Nikolay Kostyurin, 2026) at the root and a `LICENSE` symlink to it in `apps/gw`, `crates/gw-parse`, and `crates/gw-render`; verify `cat crates/gw-parse/LICENSE` prints the license text
- [x] 1.2 Move `license`, `repository`, `homepage`, `authors`, and `rust-version = "1.88"` into `[workspace.package]` and inherit them in all three crates; give each crate its own `description` (libraries say they are internal to `git-gw`); verify `cargo metadata --no-deps --format-version 1` shows the fields on all three packages
- [x] 1.3 Rename the binary package to `git-gw` with `[[bin]] name = "gw"`, and add `readme = "../../README.md"`, keywords, and categories; verify `cargo build --workspace` produces `target/debug/gw` and `cargo test --workspace` passes (the integration test still finds `CARGO_BIN_EXE_gw`)
- [x] 1.4 Give the workspace path dependencies exact versions (`version = "=0.1.0"`); verify `cargo tree -p git-gw` still resolves `gw-parse` and `gw-render` from their paths

## 2. Packaging checks

- [x] 2.1 Run `cargo package --list` for each crate; verify no `openspec/`, `.claude/`, `.remember/`, or `target/` paths appear, `LICENSE` is present in all three, `README.md` is present in `git-gw`, and `gw-parse` includes `tests/fixtures`
- [x] 2.2 Check the minimum Rust version: `cargo +1.88 check --workspace` succeeds (install the toolchain with rustup if missing); verify the result is recorded in the task note
  - Note (2026-11-03): `cargo +1.88 check --workspace --all-targets` succeeds; `cargo +1.87 check` fails with "gw-parse@0.1.0 requires rustc 1.88".
- [x] 2.3 Run `cargo publish --workspace --dry-run`; verify all three crates package and build from their packaged sources in the order `gw-parse`, `gw-render`, `git-gw`

## 3. Documentation

- [x] 3.1 Update `README.md`: `cargo install git-gw` (and `--locked`) as the main install, build from source as an alternative, the command is `gw`, the minimum Rust version, and `cargo uninstall gw` for users of the old `--path` install; verify the commands in the README match the package and binary names
- [x] 3.2 Add `RELEASING.md` with the release steps (bump version and `=` pins, test, dry run, publish, tag `v<version>`, push the tag, yank to roll back); verify each command in it is valid against this repository with a dry run
- [x] 3.3 Add `.github/workflows/ci.yml`: `cargo build` + `cargo test` (`--locked`) on ubuntu-latest and macos-latest, and `cargo check` on Rust 1.88; verify the test suite passes in a Linux container with git ≥ 2.41 (passed on Debian trixie, git 2.47; git 2.39 fails the fetch test as expected)

## 4. Publish (maintainer)

- [x] 4.1 Maintainer: recheck that `git-gw`, `gw-parse`, and `gw-render` are still free on crates.io, run `cargo login` if needed, then run `cargo publish --workspace`; verify all three crates show version 0.1.0 on crates.io
- [x] 4.2 Tag the release `v0.1.0` and push the tag; verify the tag is on GitHub
- [x] 4.3 In a clean environment (e.g. a fresh `CARGO_HOME` directory), run `cargo install git-gw --locked`; verify `gw` installs and `gw` with no arguments prints its usage
