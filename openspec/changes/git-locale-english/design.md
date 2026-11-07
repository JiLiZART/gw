## Context

All formatted output for `push`, `pull`, `fetch`, `clone` and `merge` comes from one git spawn in `apps/gw/src/main.rs` (`run`, the `Command::new("git")` with piped stdout/stderr). Parsers in `gw-parse` (`classify`, `parse_progress`, `classify_pull_line`) match English text.

## Goals / Non-Goals

**Goals:** correct parsing under any locale with one change at the spawn.

**Non-Goals:** translated gw output; locale-independent parsing; changing the interactive screens (they use porcelain formats already).

## Decisions

- **`LANGUAGE=en` on the child, not `LC_ALL=C`.** gettext reads `LANGUAGE` before `LC_ALL`/`LC_MESSAGES` unless the locale is `C`; with no `en` catalog git falls back to its English msgids. The character set stays the user's, so UTF-8 names are not mangled. Checked: `LC_ALL=uk_UA.UTF-8 LANGUAGE=en git merge x` prints `Auto-merging`/`CONFLICT`. `LC_ALL=C` also works but sets an ASCII charset.
  - Edge: when the user's locale is `C`/`POSIX`, gettext ignores `LANGUAGE`, but git is then English anyway.
- **Not shape-based parsing.** Shape matching could fix progress, but result lines (`CONFLICT`, `Updating`, `From`) would need per-language tables.
- **Only the formatted spawn.** Passthrough, `commit`, editors and hooks keep the user's language.

## Risks / Trade-offs

- [Users lose native-language `error:`/`hint:` text for five commands] → accepted; documented in README.
- [Remote server messages (`remote:`) use the server's language] → out of gw's control; non-Latin remote progress still fails `parse_progress`'s uppercase check. Noted, not fixed here.
- [gettext behavior on Linux/musl differs] → verify in Docker `rust:1-trixie` with a `uk_UA.UTF-8` locale installed.
