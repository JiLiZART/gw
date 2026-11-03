## Purpose

Replaces git's multi-line, carriage-return-redrawn progress text with a single live progress line so users see what stage git is in and how far along it is.

## ADDED Requirements

### Requirement: Single live progress line
While a wrapped network command (`push`, `pull`, `fetch`, `clone`) runs on a TTY, the system SHALL render git's progress as exactly one live line of the form `{spinner} [<label> <bar> <cur>/<total> <pct>%]`, where `<label>` is the current git stage name (e.g. "Writing objects"), and SHALL NOT print git's raw progress lines.

#### Scenario: Progress during push
- **WHEN** git reports `Writing objects:  44% (4/9)`
- **THEN** the live line shows the label "Writing objects", the count `4/9`, `44%`, and a bar filled to 44% of its width

#### Scenario: Carriage-return updates
- **WHEN** git emits several progress updates for one stage separated by `\r`
- **THEN** the same live line is updated in place, with no extra lines printed

### Requirement: Filled bar styling
The filled portion of the bar SHALL be drawn with a green background and the unfilled portion with a dim background; the label SHALL be left-aligned and `cur/total pct%` right-aligned inside the bar, remaining readable over both portions.

#### Scenario: Label over partially filled bar
- **WHEN** the bar is 50% filled and the label spans the fill edge
- **THEN** the label characters remain visible with contrasting foreground on both the green and dim portions

### Requirement: Per-stage refill
The bar SHALL restart from 0% when git moves to a new stage that reports a percentage (e.g. Counting → Compressing → Writing → remote Resolving deltas). Stages without a percentage (e.g. "Enumerating objects", "Delta compression") SHALL update the label with an indeterminate (empty) bar while the spinner keeps animating.

#### Scenario: Stage transition
- **WHEN** "Compressing objects" reaches 100% and "Writing objects: 11% (1/9)" arrives
- **THEN** the label changes to "Writing objects" and the bar shows 11%

#### Scenario: Remote progress is not boxed
- **WHEN** git emits `remote: Resolving deltas:  50% (3/6)`
- **THEN** it drives the progress line and does NOT appear in the remote box

### Requirement: Animated spinner and final mark
The spinner SHALL animate at a steady rate while git runs, even when git produces no output. When git exits, the spinner SHALL be replaced by a green success mark on exit code 0 or a red failure mark otherwise, and the final line SHALL remain on screen showing the last stage's state.

#### Scenario: Successful completion
- **WHEN** git exits with code 0 after "Writing objects: 100% (9/9), 988 bytes"
- **THEN** the final line shows a green success mark and a full bar with `9/9 100%`

#### Scenario: Failure mid-stage
- **WHEN** git exits non-zero while the bar shows 44%
- **THEN** the final line shows a red failure mark and the bar remains at 44%

### Requirement: Narrow terminals
When the terminal is narrower than 40 columns, the system SHALL omit the label and render only the spinner, bar, and percentage.

#### Scenario: 30-column terminal
- **WHEN** the terminal width is 30 columns
- **THEN** the progress line fits on one line and contains no stage label
