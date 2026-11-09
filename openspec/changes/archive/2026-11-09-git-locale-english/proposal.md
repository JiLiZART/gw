## Why

gw finds git's lines by their English text. Under a non-English locale plain git prints `КОНФЛІКТ`, `Aktualisiere`, `remote: Підрахунок обʼєктів: …`, which gw's parsers would not recognize. gw already runs the parsed commands with `LC_ALL=C` (since the first commit), but no spec states it and no test guards it, so a refactor of the spawn could silently break every non-English user.

## What Changes

- Spec: the git process behind the formatted output of `push`, `pull`, `fetch`, `clone` and `merge` prints English messages whatever the user's locale; passthrough commands keep the user's locale.
- Integration test that fails if the spawn stops forcing English (checked by removing `LC_ALL=C`), and that a passthrough command still sees the user's `LC_ALL`.
- README line stating the behavior.
- No runtime code change.

## Capabilities

### New Capabilities

### Modified Capabilities
- `passthrough`: new requirement that git's messages are English for the commands whose output gw parses.

## Impact

- `apps/gw/tests/integration.rs`: `gw_env` helper and a `locale` test.
- `README.md`: one line under "Safe to drop in".
