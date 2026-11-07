//! `gw status -i` and `gw add`: pure state + key handling for the interactive staging view.
//! Git work is requested through [`Effect`]s; the binary runs them and feeds results back.

pub mod view;

use gw_parse::{Entry, Status};
use ratatui::crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

use crate::status::{Group, LineStats, groups};

/// One selectable list entry. The same path can appear in two groups (staged and changed again).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Item {
    pub group: Group,
    pub entry: Entry,
    /// `+N -M` for the diff title; `Some(None)` binary, `None` unknown.
    pub stat: Option<Option<(u64, u64)>>,
    /// Checkbox state in `gw add` (one row per path); `None` in `gw status -i`.
    pub check: Option<Check>,
}

/// `[x]` staged, `[~]` staged and changed again, `[ ]` nothing staged.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Check {
    On,
    Partial,
    Off,
}

impl Check {
    pub fn mark(self) -> &'static str {
        match self {
            Check::On => "[x]",
            Check::Partial => "[~]",
            Check::Off => "[ ]",
        }
    }
}

/// Which screen the stage view serves.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Mode {
    /// `gw status -i`: grouped list, revert and commit.
    Status,
    /// `gw add`: one checkbox row per path, stage/unstage only.
    Add,
}

impl Item {
    pub fn key(&self) -> (Group, &str) {
        (self.group, &self.entry.path)
    }

    pub fn code(&self) -> char {
        self.group.code(&self.entry)
    }
}

/// Builds the list in `gw status` order.
pub fn items(st: &Status, stats: &LineStats) -> Vec<Item> {
    groups(st)
        .into_iter()
        .flat_map(|(group, entries)| {
            entries.into_iter().map(move |e| {
                let stat = match group {
                    Group::Staged => stats.staged.get(&e.path).copied(),
                    Group::Changed => stats.unstaged.get(&e.path).copied(),
                    Group::Untracked => stats.untracked.get(&e.path).copied(),
                    _ => None,
                };
                Item { group, entry: e.clone(), stat, check: None }
            })
        })
        .collect()
}

/// One row per path, sorted by path, for the `gw add` checklist and the new-stash picker.
pub fn add_items(st: &Status, stats: &LineStats) -> Vec<Item> {
    let mut entries: Vec<&Entry> = st.entries.iter().collect();
    entries.sort_by(|a, b| a.path.cmp(&b.path));
    entries
        .into_iter()
        .map(|e| {
            let staged = !e.unmerged && !matches!(e.x, '.' | '?');
            let unstaged = e.unmerged || e.x == '?' || e.y != '.';
            let (group, check) = match () {
                _ if e.unmerged => (Group::Conflicts, Check::Off),
                _ if e.x == '?' => (Group::Untracked, Check::Off),
                _ if e.submodule.is_some() => (Group::Submodules, if !staged { Check::Off } else if unstaged { Check::Partial } else { Check::On }),
                _ if staged && unstaged => (Group::Changed, Check::Partial),
                _ if staged => (Group::Staged, Check::On),
                _ => (Group::Changed, Check::Off),
            };
            let stat = match group {
                Group::Staged => stats.staged.get(&e.path).copied(),
                Group::Untracked => stats.untracked.get(&e.path).copied(),
                Group::Changed => match (stats.staged.get(&e.path).copied(), stats.unstaged.get(&e.path).copied()) {
                    (Some(Some((a1, d1))), Some(Some((a2, d2)))) => Some(Some((a1 + a2, d1 + d2))),
                    (Some(None), _) | (_, Some(None)) => Some(None),
                    (s, None) | (None, s) => s,
                },
                _ => None,
            };
            Item { group, entry: e.clone(), stat, check: Some(check) }
        })
        .collect()
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Focus {
    List,
    Diff,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Effect {
    None,
    Quit,
    Reload,
    Stage(Item),
    Unstage(Item),
    StageAll,
    UnstageAll,
    /// Collect what a revert would delete, then call [`App::confirm_revert`].
    PrepareRevert(Item),
    Revert(Item),
    Commit,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Overlay {
    None,
    /// `files`: up to 10 paths that will be deleted, `total` their full count (untracked dirs).
    Confirm { item: Item, files: Vec<String>, total: usize },
}

/// A list line: a group header (with its count) or an item index.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Row {
    Header(Group, usize),
    Item(usize),
}

pub struct App {
    pub items: Vec<Item>,
    pub mode: Mode,
    pub sel: usize,
    /// First visible list row.
    pub scroll: usize,
    pub focus: Focus,
    /// Diff lines for `diff_for`.
    pub diff: Vec<String>,
    pub diff_for: Option<(Group, String)>,
    pub diff_scroll: usize,
    pub overlay: Overlay,
    /// (message, is_error)
    pub status: Option<(String, bool)>,
    /// `git rev-parse --show-prefix`, for cwd-relative display.
    pub prefix: String,
    /// Branch line shown in the list title.
    pub head: String,
    pub width: u16,
    pub height: u16,
    /// False under `NO_COLOR`.
    pub color: bool,
}

/// Width under which only the focused pane is shown.
pub const NARROW: u16 = 80;

impl App {
    pub fn new(items: Vec<Item>, prefix: String, head: String) -> Self {
        App {
            items,
            mode: Mode::Status,
            sel: 0,
            scroll: 0,
            focus: Focus::List,
            diff: Vec::new(),
            diff_for: None,
            diff_scroll: 0,
            overlay: Overlay::None,
            status: None,
            prefix,
            head,
            width: 80,
            height: 24,
            color: true,
        }
    }

    pub fn resize(&mut self, w: u16, h: u16) {
        self.width = w;
        self.height = h;
        self.keep_visible();
    }

    /// Lines inside a pane: screen minus status line, hotkey bar and borders.
    pub fn body_height(&self) -> usize {
        (self.height as usize).saturating_sub(4).max(1)
    }

    pub fn selected(&self) -> Option<&Item> {
        self.items.get(self.sel)
    }

    pub fn rows(&self) -> Vec<Row> {
        if self.mode == Mode::Add {
            return (0..self.items.len()).map(Row::Item).collect();
        }
        let mut rows = Vec::new();
        let mut i = 0;
        while i < self.items.len() {
            let g = self.items[i].group;
            let n = self.items[i..].iter().take_while(|it| it.group == g).count();
            rows.push(Row::Header(g, n));
            rows.extend((i..i + n).map(Row::Item));
            i += n;
        }
        rows
    }

    fn sel_row(&self) -> usize {
        self.rows().iter().position(|r| *r == Row::Item(self.sel)).unwrap_or(0)
    }

    fn keep_visible(&mut self) {
        let h = self.body_height();
        let row = self.sel_row();
        // Show the group header above the first entry.
        let top = if self.sel == 0 { 0 } else { row };
        if top < self.scroll {
            self.scroll = top;
        } else if row >= self.scroll + h {
            self.scroll = row + 1 - h;
        }
    }

    /// The item whose diff should be shown, when it is not loaded yet.
    pub fn diff_wanted(&self) -> Option<&Item> {
        let it = self.selected()?;
        let loaded = self.diff_for.as_ref().is_some_and(|(g, p)| *g == it.group && *p == it.entry.path);
        (!loaded).then_some(it)
    }

    pub fn set_diff(&mut self, item: &Item, lines: Vec<String>) {
        self.diff = lines;
        self.diff_for = Some((item.group, item.entry.path.clone()));
        self.diff_scroll = 0;
    }

    /// New list after a reload: keep the same entry selected (same group and path, else same
    /// path in another group, else the nearest position).
    pub fn replace_items(&mut self, items: Vec<Item>) {
        let old = self.selected().map(|it| (it.group, it.entry.path.clone()));
        let old_sel = self.sel;
        self.items = items;
        self.sel = old
            .and_then(|(g, p)| {
                self.items
                    .iter()
                    .position(|it| it.group == g && it.entry.path == p)
                    .or_else(|| self.items.iter().position(|it| it.entry.path == p))
            })
            .unwrap_or(old_sel.min(self.items.len().saturating_sub(1)));
        self.diff_for = None;
        if self.items.is_empty() {
            self.diff.clear();
            self.focus = Focus::List;
        }
        self.keep_visible();
    }

    pub fn confirm_revert(&mut self, item: Item, files: Vec<String>, total: usize) {
        self.overlay = Overlay::Confirm { item, files, total };
    }

    pub fn key(&mut self, k: KeyEvent) -> Effect {
        let ctrl = k.modifiers.contains(KeyModifiers::CONTROL);
        if ctrl && k.code == KeyCode::Char('c') {
            return Effect::Quit;
        }
        if let Overlay::Confirm { item, .. } = &self.overlay {
            let item = item.clone();
            self.overlay = Overlay::None;
            return if k.code == KeyCode::Char('y') { Effect::Revert(item) } else { Effect::None };
        }
        self.status = None;
        if ctrl && k.code == KeyCode::Char('r') {
            return Effect::Reload;
        }
        match k.code {
            KeyCode::Char('q') => return Effect::Quit,
            KeyCode::Tab | KeyCode::BackTab => {
                if !self.items.is_empty() {
                    self.focus = if self.focus == Focus::List { Focus::Diff } else { Focus::List };
                }
                return Effect::None;
            }
            KeyCode::Esc if self.focus == Focus::Diff => {
                self.focus = Focus::List;
                return Effect::None;
            }
            KeyCode::Esc => return Effect::Quit,
            _ => {}
        }
        let page = self.body_height() as isize;
        let delta = match k.code {
            KeyCode::Up | KeyCode::Char('k') => Some(-1),
            KeyCode::Down | KeyCode::Char('j') => Some(1),
            KeyCode::PageUp => Some(-page),
            KeyCode::PageDown => Some(page),
            KeyCode::Home | KeyCode::Char('g') => Some(isize::MIN / 2),
            KeyCode::End | KeyCode::Char('G') => Some(isize::MAX / 2),
            _ => None,
        };
        if let Some(d) = delta {
            if self.focus == Focus::Diff {
                let max = self.diff.len().saturating_sub(self.body_height()) as isize;
                self.diff_scroll = (self.diff_scroll as isize).saturating_add(d).clamp(0, max.max(0)) as usize;
            } else if !self.items.is_empty() {
                self.sel = (self.sel as isize).saturating_add(d).clamp(0, self.items.len() as isize - 1) as usize;
                self.keep_visible();
            }
            return Effect::None;
        }
        if self.mode == Mode::Add {
            return match k.code {
                KeyCode::Char('a') => Effect::StageAll,
                KeyCode::Char('u') => Effect::UnstageAll,
                KeyCode::Char(' ') => match self.selected() {
                    Some(it) if it.check == Some(Check::On) => Effect::Unstage(it.clone()),
                    Some(it) => Effect::Stage(it.clone()),
                    None => Effect::None,
                },
                _ => Effect::None,
            };
        }
        match k.code {
            KeyCode::Char('a') => Effect::StageAll,
            KeyCode::Char('u') => Effect::UnstageAll,
            KeyCode::Char('c') if !self.items.iter().any(|it| it.group == Group::Staged) => {
                self.status = Some(("nothing staged to commit".into(), true));
                Effect::None
            }
            KeyCode::Char('c') => Effect::Commit,
            KeyCode::Char(' ') => match self.selected() {
                Some(it) if it.group == Group::Staged => Effect::Unstage(it.clone()),
                Some(it) => Effect::Stage(it.clone()),
                None => Effect::None,
            },
            KeyCode::Char('r') => match self.selected() {
                Some(it) if it.group == Group::Conflicts => {
                    self.status = Some(("conflicts can't be reverted here: resolve them, or abort the merge".into(), true));
                    Effect::None
                }
                Some(it) if it.group == Group::Submodules => {
                    self.status = Some(("submodules can't be reverted here: update or reset inside the submodule".into(), true));
                    Effect::None
                }
                Some(it) => Effect::PrepareRevert(it.clone()),
                None => Effect::None,
            },
            _ => Effect::None,
        }
    }

    pub fn hotkeys(&self) -> Vec<(&'static str, &'static str)> {
        match (&self.overlay, self.focus) {
            (Overlay::Confirm { .. }, _) => vec![("y", "Revert"), ("any key", "Cancel")],
            (Overlay::None, Focus::Diff) => vec![("j/k", "Scroll"), ("PgUp/PgDn", "Page"), ("Tab", "Files"), ("Esc", "Back"), ("q", "Quit")],
            (Overlay::None, Focus::List) if self.mode == Mode::Add => {
                vec![("Space", "Toggle"), ("a", "Stage all"), ("u", "Unstage all"), ("Tab", "Diff"), ("^R", "Reload"), ("q", "Quit")]
            }
            (Overlay::None, Focus::List) => vec![
                ("Space", "Stage/Unstage"),
                ("a", "Stage all"),
                ("u", "Unstage all"),
                ("r", "Revert"),
                ("c", "Commit"),
                ("Tab", "Diff"),
                ("^R", "Reload"),
                ("q", "Quit"),
            ],
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn e(x: char, y: char, path: &str) -> Entry {
        Entry { x, y, unmerged: false, path: path.into(), orig: None, submodule: None }
    }

    fn st(entries: Vec<Entry>) -> Status {
        Status { entries, ..Default::default() }
    }

    fn key(c: KeyCode) -> KeyEvent {
        KeyEvent::new(c, KeyModifiers::NONE)
    }

    fn ch(c: char) -> KeyEvent {
        key(KeyCode::Char(c))
    }

    /// Staged a.rs (also changed), changed b.rs, untracked n.txt, conflict u.rs.
    fn app() -> App {
        let mut conflict = e('U', 'U', "u.rs");
        conflict.unmerged = true;
        let s = st(vec![e('M', 'M', "a.rs"), e('.', 'M', "b.rs"), e('?', '?', "n.txt"), conflict]);
        let mut a = App::new(items(&s, &LineStats::default()), String::new(), "main".into());
        a.resize(100, 30);
        a
    }

    fn sel(a: &App) -> (Group, &str) {
        a.selected().unwrap().key()
    }

    #[test]
    fn list_order_and_headers() {
        let a = app();
        let keys: Vec<_> = a.items.iter().map(|i| i.key()).collect();
        assert_eq!(
            keys,
            vec![(Group::Conflicts, "u.rs"), (Group::Staged, "a.rs"), (Group::Changed, "a.rs"), (Group::Changed, "b.rs"), (Group::Untracked, "n.txt")]
        );
        let rows = a.rows();
        assert_eq!(rows[0], Row::Header(Group::Conflicts, 1));
        assert_eq!(rows[4], Row::Header(Group::Changed, 2));
        assert_eq!(rows.len(), 9);
    }

    #[test]
    fn navigation_skips_headers_and_clamps() {
        let mut a = app();
        a.key(ch('j'));
        assert_eq!(sel(&a), (Group::Staged, "a.rs"));
        a.key(ch('j'));
        assert_eq!(sel(&a), (Group::Changed, "a.rs"), "last Staged -> first Changed, header skipped");
        a.key(ch('G'));
        assert_eq!(sel(&a), (Group::Untracked, "n.txt"));
        a.key(ch('j'));
        assert_eq!(sel(&a), (Group::Untracked, "n.txt"));
        a.key(key(KeyCode::PageUp));
        assert_eq!(a.sel, 0);
    }

    #[test]
    fn clean_tree_has_no_selection() {
        let mut a = App::new(vec![], String::new(), "main".into());
        assert!(a.selected().is_none());
        assert_eq!(a.key(ch(' ')), Effect::None);
        assert_eq!(a.key(ch('r')), Effect::None);
        a.key(key(KeyCode::Tab));
        assert_eq!(a.focus, Focus::List);
    }

    #[test]
    fn space_stages_or_unstages_by_group() {
        let mut a = app();
        a.key(ch('j'));
        assert!(matches!(a.key(ch(' ')), Effect::Unstage(it) if it.key() == (Group::Staged, "a.rs")));
        a.key(ch('j'));
        assert!(matches!(a.key(ch(' ')), Effect::Stage(it) if it.key() == (Group::Changed, "a.rs")));
        a.key(ch('g'));
        assert!(matches!(a.key(ch(' ')), Effect::Stage(it) if it.group == Group::Conflicts), "staging marks resolved");
        assert_eq!(a.key(ch('a')), Effect::StageAll);
        assert_eq!(a.key(ch('u')), Effect::UnstageAll);
        assert_eq!(a.key(KeyEvent::new(KeyCode::Char('r'), KeyModifiers::CONTROL)), Effect::Reload);
    }

    #[test]
    fn revert_rules() {
        let mut a = app();
        assert_eq!(a.key(ch('r')), Effect::None, "conflict");
        assert!(a.status.as_ref().unwrap().0.contains("conflicts"));
        a.key(ch('G'));
        let Effect::PrepareRevert(it) = a.key(ch('r')) else { panic!() };
        a.confirm_revert(it, vec!["n.txt".into()], 1);
        assert_eq!(a.hotkeys()[0], ("y", "Revert"));
        assert!(matches!(a.key(ch('y')), Effect::Revert(it) if it.key() == (Group::Untracked, "n.txt")));
        assert_eq!(a.overlay, Overlay::None);
        for cancel in [ch('n'), key(KeyCode::Esc)] {
            let Effect::PrepareRevert(it) = a.key(ch('r')) else { panic!() };
            a.confirm_revert(it, vec![], 0);
            assert_eq!(a.key(cancel), Effect::None);
            assert_eq!(a.overlay, Overlay::None);
        }
    }

    #[test]
    fn submodule_not_revertable() {
        let mut s = e('.', 'M', "lib");
        s.submodule = Some(gw_parse::SubmoduleState { commit_changed: true, modified: false, untracked: false, head: "a".into(), index: "a".into() });
        let mut a = App::new(items(&st(vec![s]), &LineStats::default()), String::new(), "main".into());
        assert_eq!(a.key(ch('r')), Effect::None);
        assert!(a.status.as_ref().unwrap().0.contains("submodule"));
    }

    #[test]
    fn commit_needs_staged() {
        let mut a = app();
        assert_eq!(a.key(ch('c')), Effect::Commit);
        let mut b = App::new(items(&st(vec![e('.', 'M', "b.rs")]), &LineStats::default()), String::new(), "main".into());
        assert_eq!(b.key(ch('c')), Effect::None);
        assert_eq!(b.status.as_ref().unwrap().0, "nothing staged to commit");
    }

    #[test]
    fn reload_keeps_selection() {
        let mut a = app();
        a.key(ch('j'));
        a.key(ch('j'));
        a.key(ch('j'));
        assert_eq!(sel(&a), (Group::Changed, "b.rs"));
        // b.rs staged: it moves to Staged and stays selected.
        let mut c = e('U', 'U', "u.rs");
        c.unmerged = true;
        a.replace_items(items(&st(vec![e('M', 'M', "a.rs"), e('M', '.', "b.rs"), e('?', '?', "n.txt"), c]), &LineStats::default()));
        assert_eq!(sel(&a), (Group::Staged, "b.rs"));
        // Last entry reverted: selection moves to the new last one.
        a.key(ch('G'));
        a.replace_items(items(&st(vec![e('M', '.', "a.rs")]), &LineStats::default()));
        assert_eq!(sel(&a), (Group::Staged, "a.rs"));
        a.replace_items(vec![]);
        assert!(a.selected().is_none());
    }

    fn add_app(entries: Vec<Entry>) -> App {
        let mut a = App::new(add_items(&st(entries), &LineStats::default()), String::new(), "main".into());
        a.mode = Mode::Add;
        a.resize(100, 30);
        a
    }

    #[test]
    fn add_rows_one_per_path() {
        let a = add_app(vec![e('?', '?', "n.txt"), e('.', 'M', "c.rs"), e('M', 'M', "b.rs"), e('M', '.', "a.rs")]);
        let rows: Vec<_> = a.items.iter().map(|i| (i.entry.path.as_str(), i.check.unwrap())).collect();
        assert_eq!(rows, vec![("a.rs", Check::On), ("b.rs", Check::Partial), ("c.rs", Check::Off), ("n.txt", Check::Off)]);
        assert_eq!(a.rows(), (0..4).map(Row::Item).collect::<Vec<_>>(), "no headers");
    }

    #[test]
    fn add_mode_keys() {
        let mut a = add_app(vec![e('M', '.', "a.rs"), e('M', 'M', "b.rs"), e('?', '?', "n.txt")]);
        assert!(matches!(a.key(ch(' ')), Effect::Unstage(it) if it.entry.path == "a.rs"));
        a.key(ch('j'));
        assert!(matches!(a.key(ch(' ')), Effect::Stage(it) if it.entry.path == "b.rs"), "partial stages the rest");
        a.key(ch('j'));
        assert!(matches!(a.key(ch(' ')), Effect::Stage(it) if it.entry.path == "n.txt"));
        assert_eq!(a.key(ch('a')), Effect::StageAll);
        assert_eq!(a.key(ch('u')), Effect::UnstageAll);
        assert_eq!(a.key(ch('r')), Effect::None, "no revert in add");
        assert_eq!(a.key(ch('c')), Effect::None, "no commit in add");
        let keys: Vec<_> = a.hotkeys().iter().map(|k| k.0).collect();
        assert!(!keys.contains(&"r") && !keys.contains(&"c") && keys.contains(&"Space"), "{keys:?}");
        // Staging b.rs keeps it selected (same path, new group).
        a.key(ch('k'));
        a.replace_items(add_items(&st(vec![e('M', '.', "a.rs"), e('M', '.', "b.rs"), e('?', '?', "n.txt")]), &LineStats::default()));
        assert_eq!(a.selected().unwrap().entry.path, "b.rs");
    }

    #[test]
    fn diff_focus_scrolls_without_moving_selection() {
        let mut a = app();
        a.resize(100, 10);
        let it = a.selected().unwrap().clone();
        assert!(a.diff_wanted().is_some());
        a.set_diff(&it, (0..50).map(|i| format!("+{i}")).collect());
        assert!(a.diff_wanted().is_none());
        a.key(key(KeyCode::Tab));
        a.key(key(KeyCode::PageDown));
        assert_eq!(a.diff_scroll, 6);
        assert_eq!(a.sel, 0);
        a.key(ch('G'));
        assert_eq!(a.diff_scroll, 44);
        a.key(key(KeyCode::Esc));
        assert_eq!(a.focus, Focus::List);
        assert_eq!(a.key(key(KeyCode::Esc)), Effect::Quit);
    }
}
