# Releasing

`gw` ships as three crates that always share one version: `gw-parse`, `gw-render`, and `git-gw` (the `gw` binary).
Publishing is permanent: a version can be yanked, never deleted or re-uploaded.

Releases are automated:

1. Write commits in [Conventional Commits](https://www.conventionalcommits.org) style. On `0.x`: `feat:` bumps the minor version, `fix:` (and others) the patch.
2. Every push to `main` runs release-plz (`.github/workflows/release-plz.yml`), which opens or updates a **`chore: release`** PR with the version bump and `CHANGELOG.md` entries. Edit the changelog in that PR if you want.
3. Merge the PR. release-plz then:
   - publishes `gw-parse`, `gw-render`, `git-gw` to crates.io (trusted publishing, no token),
   - pushes the `v<version>` tag,
   - starts the dist workflow (`.github/workflows/release.yml`), which builds `gw` for macOS, Linux, and Windows and creates the GitHub Release with the binaries, shell/PowerShell installers, and the changelog section as notes.

PRs created by release-plz use `GITHUB_TOKEN`, so CI does not run on them automatically. Close and reopen the PR, or push to its branch, if you want the checks.

## One-time setup

crates.io trusted publishing can't create new crates, so the first release (0.1.0) uses a token:

1. Create a crates.io API token (scopes `publish-new`, `publish-update`; crates `git-gw`, `gw-parse`, `gw-render`; short expiry) and add it as the repository secret `CARGO_REGISTRY_TOKEN`.
2. On GitHub: Settings → Actions → General → Workflow permissions → enable **Allow GitHub Actions to create and approve pull requests**.
3. Push to `main`. The release job publishes 0.1.0, pushes `v0.1.0`, and starts the dist workflow.
4. On crates.io, for each of `gw-parse`, `gw-render`, `git-gw`: Settings → Trusted Publishing → add GitHub, repository `JiLiZART/gw`, workflow `release-plz.yml`.
5. Remove the `CARGO_REGISTRY_TOKEN` line from `.github/workflows/release-plz.yml`, delete the secret, and revoke the token on crates.io.

## Manual release (fallback)

1. Bump `[workspace.package].version` and the two `=` pins in `[workspace.dependencies]` in the root `Cargo.toml`, and add a `CHANGELOG.md` section.
2. `cargo test --workspace && cargo +1.88 check --workspace --all-targets`
3. Commit, then `cargo publish --workspace --dry-run` and `cargo publish --workspace`.
   If it stops midway, publish the rest one by one in order with `cargo publish -p <crate>`.
4. `git tag v<version> && git push origin v<version>`, then run the **Release** workflow with that tag.

## Rolling back

Yank the bad version of every affected crate, then release a fixed version:

```sh
cargo yank --version <version> git-gw
cargo yank --version <version> gw-render
cargo yank --version <version> gw-parse
```
