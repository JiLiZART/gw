## Purpose

Presents messages sent by the git server (`remote:` lines) as a distinct, titled, bordered block so links like merge-request URLs stand out and are clickable.

## ADDED Requirements

### Requirement: Remote messages collected into a box
The system SHALL collect all non-progress `remote:` lines from a wrapped command, strip the `remote:` prefix (and one following space), and render them after the progress line inside a box with a thin single-line border. Leading and trailing blank lines inside the collected block SHALL be dropped; inner blank lines and indentation SHALL be preserved.

#### Scenario: GitLab push message
- **WHEN** git emits
  ```
  remote:
  remote: View merge request for feat-forms-constructor:
  remote:   https://gitlab.example.com/group/repo/-/merge_requests/2
  remote:
  ```
- **THEN** a single bordered box is printed containing exactly two lines: `View merge request for feat-forms-constructor:` and `  https://gitlab.example.com/group/repo/-/merge_requests/2`, with no `remote:` prefixes

#### Scenario: Pack statistics are not boxed
- **WHEN** git emits `remote: Total 6 (delta 3), reused 0 (delta 0)`
- **THEN** the line does not appear in the box

#### Scenario: No remote messages
- **WHEN** the command produces no non-progress `remote:` lines
- **THEN** no box is printed

### Requirement: Host-aware box title
The box title SHALL be "Merge request" when the content mentions a merge request (GitLab), "Pull request" when it mentions a pull request (GitHub, Gitea, Bitbucket), and "remote" otherwise.

#### Scenario: GitHub pull request hint
- **WHEN** remote content contains `Create a pull request for 'feat' on GitHub by visiting:`
- **THEN** the box title is "Pull request"

#### Scenario: Unrecognized server message
- **WHEN** remote content is `Processing changes: refs: 1, done`
- **THEN** the box title is "remote"

### Requirement: Clickable, highlighted links
Every `http://` or `https://` URL in the box SHALL be emitted as an OSC 8 terminal hyperlink pointing at the full URL, styled underlined and in a link color. When the URL is wider than the box interior, its visible text SHALL be truncated with `…` while the hyperlink target remains the full URL.

#### Scenario: Long URL in narrow terminal
- **WHEN** a 120-character URL is rendered in an 80-column terminal
- **THEN** the box border stays aligned, the visible URL ends with `…`, and clicking it opens the full 120-character URL

### Requirement: Highlighted branch names
Branch names that also appear in the command's ref updates SHALL be highlighted inside the box using the same color used for branches in the ref update list.

#### Scenario: Branch mentioned in MR message
- **WHEN** the pushed branch is `feat-forms-constructor` and the box line says `View merge request for feat-forms-constructor:`
- **THEN** `feat-forms-constructor` is rendered in the branch color

### Requirement: Untrusted server text is inert
Before link detection, control characters (C0, DEL, C1) in collected `remote:` text SHALL be shown as `\xNN`, and tabs as a space.

#### Scenario: Injected escape in server message
- **WHEN** a `remote:` line contains `ESC]8;;https://evil` or `ESC[2J`
- **THEN** the box shows the escape as `\x1b` text and no hyperlink or screen command reaches the terminal
