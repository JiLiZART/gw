## Why

gw finds git's lines by their English text. Under a non-English locale (checked with `uk_UA` and `de_DE`, git 2.51) progress lines with non-Latin labels (`remote: Підрахунок обʼєктів: 11% (1/9)`) are not seen as progress and fill the remote box line by line, `From <url>` (`Від …`) loses the fetch target, and merge/pull result lines (`КОНФЛІКТ`, `Aktualisiere`) lose their colors and conflict detection.

## What Changes

- The git process behind the formatted output of `push`, `pull`, `fetch`, `clone` and `merge` runs with git's messages in English, keeping the user's character set.
- Plain passthrough commands, editors, hooks and the interactive screens keep the user's locale.

## Capabilities

### New Capabilities

### Modified Capabilities
- `passthrough`: new requirement that git's messages are English for the commands whose output gw parses.

## Impact

- `apps/gw/src/main.rs`: the formatted-command spawn sets one environment variable.
- Users of these five commands see git's own error and hint lines in English.
