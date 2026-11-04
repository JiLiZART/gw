# log-filters Specification

## Purpose

Defines how the user narrows the commits shown in `gw log`: branch scope, fuzzy search, and the date and author filter modals.

## Requirements

### Requirement: Branch scope
By default the browser SHALL show commits reachable from the current branch (`HEAD`). The `a` key SHALL toggle between that and all refs (all local and remote branches and tags). The current scope SHALL be shown in the header.

#### Scenario: Toggle all branches
- **WHEN** the user presses `a` on branch `feat`
- **THEN** commits from all branches are listed and the header shows `all branches`; pressing `a` again shows only `feat`

### Requirement: Branch select modal
The `b` key SHALL open a centered modal with an input field and a list of local and remote branches, filtered as the user types. `Enter` SHALL show the commits of the chosen branch; `Esc` SHALL close the modal without changes.

#### Scenario: Pick another branch
- **WHEN** the user presses `b`, types `rel`, and presses `Enter` on `release/1.2`
- **THEN** the list shows commits reachable from `release/1.2` and the header names it

### Requirement: Fuzzy search
`/` (and `Ctrl-F`) SHALL open a search input. As the user types, the list SHALL show only loaded commits whose hash, subject, author, or ref names fuzzy-match the query, best matches first, with matched characters highlighted. `Enter` SHALL keep the filter and return focus to the list; `Esc` SHALL clear it.

#### Scenario: Search by author fragment
- **WHEN** the user types `nik fix`
- **THEN** commits by "Nikolay" with "fix" in the subject are listed first

#### Scenario: Search by hash prefix
- **WHEN** the user types `a22b`
- **THEN** commit `a22b7ed` is listed first

### Requirement: Date filter modal
The `d` key SHALL open a centered modal with two input fields, "Since" and "Until", each accepting any date git understands (e.g. `2 weeks ago`, `2026-10-01`). `Enter` SHALL apply them; an empty field means no bound. An input git cannot understand SHALL show an error in the modal and keep it open. git never rejects a date: it reads unknown text as "now", so an input other than `now` or `today` that resolves to the current time counts as not understood.

#### Scenario: Last two weeks
- **WHEN** the user enters `2 weeks ago` in Since and presses `Enter`
- **THEN** only commits from the last two weeks are shown and the header shows the date range

### Requirement: Author filter modal
The `u` key SHALL open a centered modal listing the authors of the current scope, most commits first, with an input box. Typing SHALL filter the list; `Enter` on a listed author SHALL filter by that author, and `Enter` with no list match SHALL filter by the typed text as an author pattern.

#### Scenario: Unlisted author
- **WHEN** the user types `alice@example` and no listed author matches
- **THEN** `Enter` filters commits whose author matches `alice@example`

### Requirement: Clear filters
The header SHALL list all active filters. The `x` key SHALL clear search, date, and author filters and return to the current-branch scope.

#### Scenario: Clear everything
- **WHEN** a date filter and an author filter are active and the user presses `x`
- **THEN** both are removed and the full current-branch history is shown
