## 1. Locale

- [ ] 1.1 Set `LANGUAGE=en` on the formatted-command git child in `apps/gw/src/main.rs`. Verify an integration test: with `LC_ALL` set to an installed non-English locale (skip when none), `gw merge` on a conflict prints `CONFLICT` and exits 1.
- [ ] 1.2 Integration test that passthrough keeps the locale: in a scratch repo set `alias.lang = !echo \"[$LANGUAGE]\"`, run `gw lang` with `LANGUAGE=uk`. Verify it prints `[uk]`.
- [ ] 1.3 Manual check: `LC_ALL=uk_UA.UTF-8 gw clone` of a local repo shows the progress bar and no boxed progress lines; same in Docker `rust:1-trixie` with the locale generated.

## 2. Docs

- [ ] 2.1 README: one line that push/pull/fetch/clone/merge show git's messages in English. Verify by reading the diff.
