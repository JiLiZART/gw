## Purpose

Defines how `gw` is published to crates.io and installed with cargo, so users get the `gw` command with one `cargo install` and future releases follow a repeatable procedure.

## ADDED Requirements

### Requirement: Install from crates.io
Running `cargo install git-gw` on a machine with a supported Rust toolchain SHALL build and install an executable named `gw`. The installed `gw` SHALL behave the same as one built from the repository at the same version.

#### Scenario: Fresh install
- **WHEN** a user with Rust 1.88 or newer runs `cargo install git-gw`
- **THEN** the install succeeds and `gw` is on the cargo bin path, and `gw` with no arguments prints its usage

#### Scenario: Locked install
- **WHEN** a user runs `cargo install git-gw --locked`
- **THEN** the build uses the dependency versions from the published lockfile

### Requirement: Published crate set
The project SHALL publish `gw-parse`, `gw-render`, and `git-gw` to crates.io, all with the same version. Each published crate SHALL depend on the other project crates by registry version, never by path or git only.

#### Scenario: Library dependencies resolve from the registry
- **WHEN** `git-gw` version X is installed from crates.io
- **THEN** it uses `gw-parse` and `gw-render` version X downloaded from crates.io

### Requirement: Package metadata
Every published crate SHALL declare a description, the MIT license, the repository URL `https://github.com/JiLiZART/gw`, and the minimum supported Rust version. `git-gw` SHALL also declare the README, keywords, and the `command-line-utilities` category. The repository SHALL contain the MIT license text.

#### Scenario: crates.io page
- **WHEN** a user opens the `git-gw` page on crates.io
- **THEN** it shows the description, MIT license, repository link, and the README

### Requirement: Minimum supported Rust version
The declared minimum supported Rust version SHALL be the oldest toolchain that builds the crates. A build with an older toolchain SHALL fail with cargo's rust-version error rather than a compile error.

#### Scenario: Toolchain too old
- **WHEN** a user with Rust 1.87 runs `cargo install git-gw`
- **THEN** cargo reports that the package requires a newer `rustc`

### Requirement: Package contents
Published packages SHALL contain only what is needed to build, test, and document the crate. They SHALL NOT include OpenSpec planning files, editor or agent configuration, or build output.

#### Scenario: Package listing
- **WHEN** the maintainer lists the files of the `git-gw` package
- **THEN** no `openspec/`, `.claude/`, `.remember/`, or `target/` paths appear

### Requirement: Documented install and release
The README SHALL show `cargo install git-gw` as the primary install method and building from source as an alternative, and SHALL state that the command is `gw`. The repository SHALL document a release procedure: bump the shared version, run a dry-run publish, publish all crates in dependency order, and tag the commit `v<version>`.

#### Scenario: Following the release procedure
- **WHEN** the maintainer follows the documented procedure for a new version
- **THEN** all three crates are published at that version and the commit is tagged `v<version>`
