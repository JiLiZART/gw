## Purpose

Replaces git's multi-line, carriage-return-redrawn progress text with a single live progress line so users see what stage git is in and how far along it is.

## ADDED Requirements

### Requirement: Single live progress line
While a wrapped network command (`push`, `pull`, `fetch`, `clone`) runs on a TTY, the system SHALL render git's progress as exactly one live line of the form `{spinner} [<label> <bar> <cur>/<total> <pct>%]`, where `<label>` is the current git stage name (e.g. "Writing objects"), and SHALL NOT print git's raw progress lines.

#### Scenario: Progress during push
- **WHEN** git reports `Writing objects:  44% (4/9)`
- **THEN** the live line shows the label "Writing objects", the count `4/9`, `44%`, and a bar that fills to 44% of its width (eased, see Smooth fill)

#### Scenario: Carriage-return updates
- **WHEN** git emits several progress updates for one stage separated by `\r`
- **THEN** the same live line is updated in place, with no extra lines printed

### Requirement: Filled bar styling
The filled portion of the bar SHALL be drawn with a fixed dark green background (256-color, not the theme's ANSI green) with bold white text, and the unfilled portion with a fixed charcoal background with light gray text; the label SHALL be left-aligned and `cur/total pct%` right-aligned inside the bar, remaining readable over both portions.

#### Scenario: Label over partially filled bar
- **WHEN** the bar is 50% filled and the label spans the fill edge
- **THEN** the label characters remain visible with contrasting foreground on both the green and dim portions

### Requirement: Per-stage refill
The bar SHALL restart from 0% when git moves to a new stage that reports a percentage (e.g. Counting → Compressing → Writing → remote Resolving deltas). Stages without a percentage (e.g. "Enumerating objects", "Delta compression") SHALL update the label with an indeterminate (empty) bar while the spinner keeps animating.

#### Scenario: Stage transition
- **WHEN** "Compressing objects" reaches 100% and "Writing objects: 11% (1/9)" arrives
- **THEN** the label changes to "Writing objects" and the bar shows 11%

#### Scenario: Stage without percentage
- **WHEN** git emits `Delta compression using up to 14 threads`
- **THEN** the label changes to "Delta compression" with an indeterminate bar

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

### Requirement: Connecting indicator
When no progress has arrived within 300 ms of starting git, the system SHALL show the live line with the spinner, the label `Connecting…`, and an empty bar. The first progress event SHALL replace it. If git exits before 300 ms without progress, no live line SHALL be drawn.

#### Scenario: Slow server handshake
- **WHEN** `gw push` runs and git emits no progress for 1 second
- **THEN** the live line shows the animated spinner and `Connecting…` until the first progress event

#### Scenario: Fast up-to-date push
- **WHEN** git prints "Everything up-to-date" and exits within 300 ms
- **THEN** no progress line is printed

### Requirement: Smooth fill
The displayed fill SHALL ease toward git's reported percentage at a fixed rate (0 to 100% in about 250 ms), so fast stages remain visible. Each stage SHALL stay on screen until its fill reaches the reported percentage and for at least 120 ms; later stages wait in order. The counters show git's real values. After git exits successfully, the display SHALL finish easing for at most 1 second before the final frame. After a failure, the final frame SHALL appear immediately with the last reported state.

#### Scenario: Instant small push
- **WHEN** git reports Compressing, Writing, and remote Resolving deltas all at 100% within 10 ms
- **THEN** each stage is shown in order with its bar visibly filling, and the final line appears within 1 second after git exits

#### Scenario: Failure skips easing
- **WHEN** git exits non-zero while easing is in progress
- **THEN** the final frame shows the red failure mark and git's last reported percentage immediately
