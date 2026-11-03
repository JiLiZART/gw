# Releasing

`gw` ships as three crates that always share one version: `gw-parse`, `gw-render`, and `git-gw` (the `gw` binary).
Publishing is permanent: a version can be yanked, never deleted or re-uploaded.

1. Bump the version in the root `Cargo.toml`: `[workspace.package].version` and the two `=` pins in `[workspace.dependencies]` (`gw-parse`, `gw-render`).
2. Check everything builds and passes:

   ```sh
   cargo test --workspace
   cargo +1.88 check --workspace --all-targets
   ```

3. Commit the bump, then dry-run the publish:

   ```sh
   cargo publish --workspace --dry-run
   ```

4. Publish (needs `cargo login` with a crates.io token). Cargo uploads `gw-parse`, `gw-render`, then `git-gw`:

   ```sh
   cargo publish --workspace
   ```

   If it stops midway, publish the remaining crates one by one in that order with `cargo publish -p <crate>`.

5. Tag and push the release:

   ```sh
   git tag v<version>
   git push origin v<version>
   ```

## Rolling back

Yank the bad version of every affected crate, then release a fixed version:

```sh
cargo yank --version <version> git-gw
cargo yank --version <version> gw-render
cargo yank --version <version> gw-parse
```
