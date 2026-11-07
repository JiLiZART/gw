## 1. Rendering

- [ ] 1.1 `titled_box` in gw-render: border like `remote_box`, wrap by display width, NO_COLOR plain. Verify unit tests: 70-char line in 40 columns wraps and no line exceeds 40; NO_COLOR output has no `\x1b`.
- [ ] 1.2 `render_messages` with hint boxing: consecutive `hint:` runs become one `hint` box at their position; prefix, tabs and empty hints handled. Verify unit tests for the diverging-branches and error/hint/fatal scenarios; existing push test unchanged.
- [ ] 1.3 `render_conflict_box(paths, op, width, color)` with merge/rebase/none steps. Verify snapshot tests for each op.

## 2. Wiring

- [ ] 2.1 In `run` for `merge`/`pull`: on non-zero exit read unmerged paths and the operation; drop `Automatic merge failed` when paths exist; print the conflict box after the summary; pass `box_hints` for merge/pull only. Verify integration tests on scratch repos: content conflict (box lists both files, exit 1), dirty-tree failure (no box), `--ff-only` divergence (hint box then `fatal:`), `pull --rebase` conflict (rebase steps).
- [ ] 2.2 Run `cargo test --workspace` and `cargo clippy --workspace --all-targets -- -D warnings`. Verify both pass.

## 3. Docs

- [ ] 3.1 README: a merge conflict screenshot `docs/screenshots/merge-conflict.svg` with both boxes. Verify the SVG has no local paths.
