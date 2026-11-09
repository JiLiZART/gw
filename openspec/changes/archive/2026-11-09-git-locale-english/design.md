## Context

`network()` in `apps/gw/src/main.rs` spawns git for `push`, `pull`, `fetch`, `clone` and `merge` with `.env("LC_ALL", "C")`. The explore step tested plain `git` under `uk_UA` and saw translated output; re-testing `gw` itself on a pty showed English output, including the non-ASCII name `файл.txt` printed unchanged. Interactive screens use porcelain/format output; `for-each-ref`'s `%(upstream:track)` is English in all locales tested (de, ru, uk, fr, es, zh, ja, pt).

## Goals / Non-Goals

**Goals:** state and guard the existing behavior.

**Non-Goals:** switching to `LANGUAGE=en`; localizing relative dates (`25 хвилин тому` in `gw branch`/stash list is display-only); parsing `remote:` progress sent by a non-English server.

## Decisions

- **Keep `LC_ALL=C`.** It already works on macOS and Linux and overrides any `LANG`/`LANGUAGE`. `LANGUAGE=en` would keep the user's charset, but git prints path bytes unchanged under `C` too, so there is nothing to gain.
- **Test self-skips** when plain git under `uk_UA.UTF-8` already prints English (locale or catalog missing), so CI without locales stays green; the Docker run generates the locale to exercise it.

## Risks / Trade-offs

- [Users see git's own `error:`/`hint:` text in English for five commands] → documented in README.
- [A server sending non-Latin `remote:` progress labels still lands in the remote box] → out of scope; gw cannot set the server's locale.
