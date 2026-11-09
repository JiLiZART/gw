## 1. Guard the existing behavior

- [x] 1.1 Integration test `locale`: under `LC_ALL=uk_UA.UTF-8 LANGUAGE=uk`, `gw merge` on a conflict prints `CONFLICT (content): Merge conflict in f` and exits 1; skips when plain git is English there. Verify it passes, and fails with `LC_ALL=C` removed from `network()`.
- [x] 1.2 Same test: alias `lang = !echo "[$LC_ALL]"` run as `gw lang` with `LC_ALL=uk_UA.UTF-8` prints `[uk_UA.UTF-8]`. Verify it passes.
- [x] 1.3 Run the test in Docker `rust:1-trixie` with `uk_UA.UTF-8` generated. Verify it passes without skipping (git there prints Ukrainian).

## 2. Docs

- [x] 2.1 README: a line under "Safe to drop in" that push/pull/fetch/clone/merge run git with `LC_ALL=C` and other commands keep the user's language. Verify by reading the diff.
