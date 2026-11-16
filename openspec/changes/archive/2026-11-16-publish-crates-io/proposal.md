## Why

`gw` can only be installed from a local checkout (`cargo install --path apps/gw`). Publishing it to crates.io lets anyone install it with one `cargo install` command and get updates the same way. The name `gw` is already taken on crates.io, so the package needs a different name while the command stays `gw`.

## What Changes

- Rename the binary package from `gw` to `git-gw`; the installed executable stays `gw` (`[[bin]] name = "gw"`).
- Publish three crates to crates.io in dependency order, all at the same version: `gw-parse`, `gw-render`, `git-gw`.
- Add the metadata crates.io requires or shows: description, license, repository, readme, keywords, categories, and the minimum supported Rust version.
- Give the workspace path dependencies explicit versions so the published crates resolve each other from the registry.
- Add a `LICENSE` file (MIT, matching the existing `license = "MIT"`).
- Update the README: `cargo install git-gw` as the main install method, the build-from-source path kept as an alternative.
- Document a repeatable release procedure: version bump, dry run, publish, git tag.

## Capabilities

### New Capabilities
- `distribution`: how `gw` is packaged, published to crates.io, installed with cargo, and released.

### Modified Capabilities
<!-- none: no existing behavior requirement changes -->

## Impact

- `Cargo.toml` (workspace): shared package metadata, versioned path dependencies.
- `apps/gw/Cargo.toml`: package renamed to `git-gw`, explicit `[[bin]] name = "gw"`.
- `crates/gw-parse/Cargo.toml`, `crates/gw-render/Cargo.toml`: per-crate descriptions.
- New `LICENSE`; `README.md` install section.
- External: three new crate names on crates.io owned by the publisher's account. Publishing is permanent: a version can be yanked but never deleted or re-uploaded.
- Requires a crates.io API token (`cargo login`) on the publishing machine.
