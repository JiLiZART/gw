## Context

How the code works today:

- The network loop in `apps/gw/src/main.rs` classifies git's stderr into `Event`s (`Target`, `RefUpdate`, `Remote`, `Other`).
- It keeps one `target: Option<String>`. Every `From <url>` overwrites it. A pull with `submodule.recurse` or `--recurse-submodules` therefore shows the superproject's updates under the submodule's URL.
- Pull's stdout (`Updating`, `Fast-forward`, the diffstat, `Submodule path …`) is collected in `stdout_lines` and passed to `render_pull_summary`. That function only colors diffstat bars.
- `Fetching submodule <p>` arrives on stderr as `Event::Other` and is printed at the end, uncolored.
- `gw status` parses `git status --porcelain=v2 --branch -z` and ignores the `<sub>` field (`N...` or `S<c><m><u>`).
  - A submodule therefore lands in Staged or Changed like a file, with numstat `+0 -0` or `+1 -1` taken from the `Subproject commit` pseudo-diff.

Captured real output (git 2.4x, C locale):

```
stderr: From <app-url>
           e232646..8910aeb  main -> origin/main
        Fetching submodule lib
        From <lib-url>
           57bb210..256f977  main -> origin/main
stdout: Updating e232646..8910aeb
        Fast-forward
         lib | 2 +-
         x   | 1 +
         2 files changed, 2 insertions(+), 1 deletion(-)
         create mode 100644 new
        Submodule path 'lib': checked out '256f977a…'
```

Porcelain v2 for a submodule: `1 .M S.M. 160000 160000 160000 <hH> <hI> lib`.

## Goals / Non-Goals

**Goals:**
- Every line git prints after a pull is recognized and colored, or passed through unchanged.
- Ref updates are attributed to the right repository.
- Submodule state appears in pull, clone and status output, including commit hashes.

**Non-Goals:**
- Recursing into nested submodules in `gw status`. Only top-level submodules are shown, matching `git status`.
- A submodule view inside `gw log`, and a `gw submodule` command.
- Old→new hashes in the pull Submodules section. git only prints the new SHA, and capturing the old one needs a pre-pull snapshot.
- Translating merge-strategy messages in non-C locales. `LC_ALL=C` is already forced.

## Decisions

### Parse (gw-parse)

1. **`Event::SubmoduleFetch(path)`.** `classify` maps `Fetching submodule <p>` to this new variant. The loop stores the path as a pending label, and the next `Target` takes it. This is simpler than inferring submodules from URLs.

2. **`classify_pull_line(&str) -> PullLine`.** It is a pure function over one stdout line and returns:
   - `Updating{old,new}`, `Result(kind)` (fast-forward, merge, rebased, up-to-date, conflict, failed)
   - `Diffstat{path, count, plus, minus}`, `BinStat`, `Summary{files, ins, del}`
   - `Mode{verb, mode, path}`
   - `Submodule{path, verb, sha}`, `SubmoduleNoise` (`Submodule '…' registered for path`)
   - `Other`

   The renderer maps each variant to a style. Under NO_COLOR it rebuilds the original text exactly, so the NO_COLOR guarantee holds. Each variant keeps its original line, and NO_COLOR prints that line. This avoids reconstruction bugs.

3. **Porcelain v2 submodule field.** `Entry` gains `submodule: Option<SubmoduleState { commit_changed, modified, untracked }>` plus `head_oid`/`index_oid` (`hH`/`hI`), parsed from the existing fields 3, 7 and 8. Unmerged (`u`) entries keep their current behavior.

### Data flow (apps/gw)

4. **`groups: Vec<(Option<String> /*label*/, String /*target*/, Vec<RefUpdate>)>`** replaces `target`/`refs`.
   - A `Target` event pushes a new group, consuming any pending submodule label.
   - A `RefUpdate` appends to the last group, or to a target-less group if there is none.
   - Push porcelain and fetch porcelain populate the same structure.
   - `all_up_to_date` and the branch list for the remote box iterate over all groups.

5. **Current submodule commit for status.** Only when an entry has `commit_changed`, run `git submodule status -- <paths…>` once from the repo top level. Its output is `[ +-U]<sha> <path> (<desc>)`; key it by path. Other submodules use `hI` as the current commit. If the call fails, show the recorded commit only, as the spec allows.

6. **Where submodule lines are stripped:**
   - In the stdout pass: `Submodule path` lines go to the Submodules section.
   - `registered for path` is printed by `git clone` on stderr and is dropped.
   - After the first `registered for path` line, later `Cloning into '…'...` and `done.` lines on stderr are submodule clones and are dropped. The top-level `Cloning into`/`done.` comes earlier and is kept.
   - `git fetch --porcelain --recurse-submodules` is rejected by git, so `gw fetch` omits `--porcelain` when `--recurse-submodules` (other than `=no`) is given and parses the stderr ref lines instead.
   - `Fetching submodule` never reaches `others`.

### Render (gw-render)

7. **Colors.** Reuse the existing constants: `DIM`, `BOLD`, `GREEN`, `RED`, `YELLOW`, `BRANCH`. The Submodules header in status uses `BRANCH` (cyan), matching the Conflicts/Staged/Changed header style. No new palette entries.

8. **Status layout.** A Submodules line is `<code> <path>  <commit>  <labels>`, with the code `M`, or `A` for a newly added submodule. Paths are aligned per group, like the other groups. Submodule entries are excluded before numstat stats are attached, so Total ignores them, and Total is omitted when its sum covers zero stat-bearing lines.

## Risks / Trade-offs

- **Unknown pull lines from other git versions.** They pass through unchanged, as today. Fixtures pin the known ones.
- **A `Fetching submodule` line followed by no `From` block** (the submodule is up to date). The pending label is dropped at the next `Fetching submodule` line or at end of stream, so it never mislabels a later superproject target.
- **Pending label attached to the wrong target.** git prints the label and the `From` block back to back, but recursive fetch can interleave with parallel jobs (`submodule.fetchJobs` > 1). Mitigation: a label applies only to the next `Target`. Under parallelism a mislabel is cosmetic, and the refs still sit under the correct URL.
- **One extra git call in `gw status`,** made only when a submodule has a changed commit. It is negligible.
