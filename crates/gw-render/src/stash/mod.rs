//! `gw stash -i`: pure state + key handling for the stash browser and the new-stash flow.
//! Git work is requested through [`Effect`]s; the binary runs them and feeds results back.

pub mod view;

use gw_parse::Stash;
use ratatui::crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

use crate::stage::Item;
use crate::status::Group;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Focus {
    Stashes,
    Files,
    Diff,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Screen {
    Browse,
    /// Picking work-tree files for a new stash.
    Pick,
    /// Typing the new stash's message.
    Message,
}

/// What the diff pane should show.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DiffSource {
    /// A file inside a stash.
    Stash { stash: Stash, file: Item },
    /// A work-tree row in the picker.
    Work(Item),
}

impl DiffSource {
    fn id(&self) -> (String, String) {
        match self {
            DiffSource::Stash { stash, file } => (stash.oid.clone(), file.entry.path.clone()),
            DiffSource::Work(it) => (String::new(), it.entry.path.clone()),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Effect {
    None,
    Quit,
    Reload,
    Apply(Stash),
    Pop(Stash),
    Drop(Stash),
    /// Load the work-tree rows, then call [`App::open_picker`].
    OpenPicker,
    /// `git stash push`: `all` means no pathspec; `untracked` adds `-u`.
    Push { paths: Vec<String>, untracked: bool, message: Option<String>, all: bool },
}

pub struct App {
    pub stashes: Vec<Stash>,
    pub sel: usize,
    pub scroll: usize,
    /// Files of `files_for` (a stash oid).
    pub files: Vec<Item>,
    pub files_for: Option<String>,
    pub file_sel: usize,
    pub file_scroll: usize,
    pub focus: Focus,
    pub diff: Vec<String>,
    diff_for: Option<(String, String)>,
    pub diff_scroll: usize,
    pub screen: Screen,
    /// Picker rows and whether each is ticked.
    pub pick: Vec<(Item, bool)>,
    pub pick_sel: usize,
    pub pick_scroll: usize,
    pub input: String,
    /// Index into `stashes` awaiting drop confirmation.
    pub confirm_drop: Option<usize>,
    /// (message, is_error)
    pub status: Option<(String, bool)>,
    pub prefix: String,
    pub width: u16,
    pub height: u16,
    pub color: bool,
}

fn scroll_to(sel: usize, scroll: &mut usize, h: usize) {
    if sel < *scroll {
        *scroll = sel;
    } else if sel >= *scroll + h {
        *scroll = sel + 1 - h;
    }
}

fn moved(cur: usize, d: isize, len: usize) -> usize {
    (cur as isize).saturating_add(d).clamp(0, len.saturating_sub(1) as isize) as usize
}

impl App {
    pub fn new(stashes: Vec<Stash>, prefix: String) -> Self {
        App {
            stashes,
            sel: 0,
            scroll: 0,
            files: Vec::new(),
            files_for: None,
            file_sel: 0,
            file_scroll: 0,
            focus: Focus::Stashes,
            diff: Vec::new(),
            diff_for: None,
            diff_scroll: 0,
            screen: Screen::Browse,
            pick: Vec::new(),
            pick_sel: 0,
            pick_scroll: 0,
            input: String::new(),
            confirm_drop: None,
            status: None,
            prefix,
            width: 80,
            height: 24,
            color: true,
        }
    }

    pub fn resize(&mut self, w: u16, h: u16) {
        self.width = w;
        self.height = h;
        let bh = self.body_height();
        scroll_to(self.sel, &mut self.scroll, bh);
        scroll_to(self.file_sel, &mut self.file_scroll, bh);
        scroll_to(self.pick_sel, &mut self.pick_scroll, bh);
    }

    /// Lines inside a pane: screen minus status line, hotkey bar and borders.
    pub fn body_height(&self) -> usize {
        (self.height as usize).saturating_sub(4).max(1)
    }

    pub fn selected(&self) -> Option<&Stash> {
        self.stashes.get(self.sel)
    }

    /// The stash whose files should be loaded, when they are not loaded yet.
    pub fn files_wanted(&self) -> Option<&Stash> {
        let s = self.selected()?;
        (self.screen == Screen::Browse && self.files_for.as_deref() != Some(&s.oid)).then_some(s)
    }

    pub fn set_files(&mut self, oid: &str, files: Vec<Item>) {
        self.files = files;
        self.files_for = Some(oid.to_string());
        self.file_sel = 0;
        self.file_scroll = 0;
    }

    fn diff_source(&self) -> Option<DiffSource> {
        match self.screen {
            Screen::Browse => {
                let stash = self.selected()?;
                if self.files_for.as_deref() != Some(&stash.oid) {
                    return None;
                }
                Some(DiffSource::Stash { stash: stash.clone(), file: self.files.get(self.file_sel)?.clone() })
            }
            Screen::Pick | Screen::Message => Some(DiffSource::Work(self.pick.get(self.pick_sel)?.0.clone())),
        }
    }

    /// The diff to load, when it is not loaded yet.
    pub fn diff_wanted(&self) -> Option<DiffSource> {
        self.diff_source().filter(|s| self.diff_for.as_ref() != Some(&s.id()))
    }

    pub fn set_diff(&mut self, src: &DiffSource, lines: Vec<String>) {
        self.diff = lines;
        self.diff_for = Some(src.id());
        self.diff_scroll = 0;
    }

    /// The file whose diff is shown (for the pane title).
    pub fn diff_item(&self) -> Option<&Item> {
        match self.screen {
            Screen::Browse => self.files.get(self.file_sel).filter(|_| self.selected().is_some()),
            _ => self.pick.get(self.pick_sel).map(|r| &r.0),
        }
    }

    /// New list after a reload or an action: keep the same stash selected (by oid), else the
    /// same position; back to browsing.
    pub fn replace_stashes(&mut self, stashes: Vec<Stash>) {
        // Back from the new-stash flow: the new stash is `0` and the list has focus.
        let from_pick = self.screen != Screen::Browse;
        let old = self.selected().map(|s| s.oid.clone()).filter(|_| !from_pick);
        if from_pick {
            self.sel = 0;
            self.focus = Focus::Stashes;
        }
        self.stashes = stashes;
        self.sel = old.and_then(|o| self.stashes.iter().position(|s| s.oid == o)).unwrap_or(self.sel.min(self.stashes.len().saturating_sub(1)));
        self.files_for = None;
        self.files.clear();
        self.diff_for = None;
        self.diff.clear();
        self.screen = Screen::Browse;
        self.confirm_drop = None;
        if self.stashes.is_empty() {
            self.focus = Focus::Stashes;
        }
        let h = self.body_height();
        scroll_to(self.sel, &mut self.scroll, h);
    }

    /// Opens the new-stash picker with every row ticked.
    pub fn open_picker(&mut self, items: Vec<Item>) {
        if items.is_empty() {
            self.status = Some(("no local changes to stash".into(), true));
            return;
        }
        self.pick = items.into_iter().map(|it| (it, true)).collect();
        self.pick_sel = 0;
        self.pick_scroll = 0;
        self.input.clear();
        self.diff_for = None;
        self.diff.clear();
        self.screen = Screen::Pick;
        self.focus = Focus::Files;
    }

    fn push_effect(&self) -> Effect {
        let all = self.pick.iter().all(|r| r.1);
        let ticked: Vec<&Item> = self.pick.iter().filter(|r| r.1).map(|r| &r.0).collect();
        let untracked = ticked.iter().any(|it| it.group == Group::Untracked);
        let paths = if all {
            vec![]
        } else {
            ticked.iter().flat_map(|it| std::iter::once(it.entry.path.clone()).chain(it.entry.orig.clone())).collect()
        };
        let message = Some(self.input.trim().to_string()).filter(|m| !m.is_empty());
        Effect::Push { paths, untracked, message, all }
    }

    fn nav_delta(&self, k: &KeyEvent) -> Option<isize> {
        let page = self.body_height() as isize;
        match k.code {
            KeyCode::Up | KeyCode::Char('k') => Some(-1),
            KeyCode::Down | KeyCode::Char('j') => Some(1),
            KeyCode::PageUp => Some(-page),
            KeyCode::PageDown => Some(page),
            KeyCode::Home | KeyCode::Char('g') => Some(isize::MIN / 2),
            KeyCode::End | KeyCode::Char('G') => Some(isize::MAX / 2),
            _ => None,
        }
    }

    fn scroll_diff(&mut self, d: isize) {
        let max = self.diff.len().saturating_sub(self.body_height()) as isize;
        self.diff_scroll = (self.diff_scroll as isize).saturating_add(d).clamp(0, max.max(0)) as usize;
    }

    pub fn key(&mut self, k: KeyEvent) -> Effect {
        let ctrl = k.modifiers.contains(KeyModifiers::CONTROL);
        if ctrl && k.code == KeyCode::Char('c') {
            return Effect::Quit;
        }
        if let Some(i) = self.confirm_drop.take() {
            return match (k.code, self.stashes.get(i)) {
                (KeyCode::Char('y'), Some(s)) => Effect::Drop(s.clone()),
                _ => Effect::None,
            };
        }
        self.status = None;
        match self.screen {
            Screen::Message => self.key_message(k),
            Screen::Pick => self.key_pick(k, ctrl),
            Screen::Browse => self.key_browse(k, ctrl),
        }
    }

    fn key_message(&mut self, k: KeyEvent) -> Effect {
        match k.code {
            KeyCode::Esc => self.screen = Screen::Pick,
            KeyCode::Enter => return self.push_effect(),
            KeyCode::Backspace => {
                self.input.pop();
            }
            KeyCode::Char(c) if !k.modifiers.contains(KeyModifiers::CONTROL) => self.input.push(c),
            _ => {}
        }
        Effect::None
    }

    fn key_pick(&mut self, k: KeyEvent, ctrl: bool) -> Effect {
        if let Some(d) = self.nav_delta(&k) {
            if self.focus == Focus::Diff {
                self.scroll_diff(d);
            } else {
                self.pick_sel = moved(self.pick_sel, d, self.pick.len());
                let h = self.body_height();
                scroll_to(self.pick_sel, &mut self.pick_scroll, h);
            }
            return Effect::None;
        }
        match k.code {
            KeyCode::Char('r') if ctrl => return Effect::Reload,
            KeyCode::Tab | KeyCode::BackTab => self.focus = if self.focus == Focus::Diff { Focus::Files } else { Focus::Diff },
            KeyCode::Esc if self.focus == Focus::Diff => self.focus = Focus::Files,
            KeyCode::Esc => {
                self.screen = Screen::Browse;
                self.focus = Focus::Stashes;
                self.diff_for = None;
            }
            KeyCode::Char('q') => return Effect::Quit,
            KeyCode::Char(' ') => {
                if let Some(r) = self.pick.get_mut(self.pick_sel) {
                    r.1 = !r.1;
                }
            }
            KeyCode::Char('A') => {
                let all = self.pick.iter().all(|r| r.1);
                self.pick.iter_mut().for_each(|r| r.1 = !all);
            }
            KeyCode::Enter if !self.pick.iter().any(|r| r.1) => self.status = Some(("nothing selected to stash".into(), true)),
            KeyCode::Enter => self.screen = Screen::Message,
            _ => {}
        }
        Effect::None
    }

    fn key_browse(&mut self, k: KeyEvent, ctrl: bool) -> Effect {
        if let Some(d) = self.nav_delta(&k) {
            let h = self.body_height();
            match self.focus {
                Focus::Stashes => {
                    self.sel = moved(self.sel, d, self.stashes.len());
                    scroll_to(self.sel, &mut self.scroll, h);
                }
                Focus::Files => {
                    self.file_sel = moved(self.file_sel, d, self.files.len());
                    scroll_to(self.file_sel, &mut self.file_scroll, h);
                }
                Focus::Diff => self.scroll_diff(d),
            }
            return Effect::None;
        }
        let stash = self.selected().cloned();
        match k.code {
            KeyCode::Char('r') if ctrl => Effect::Reload,
            KeyCode::Char('q') => Effect::Quit,
            KeyCode::Esc if self.focus == Focus::Stashes => Effect::Quit,
            KeyCode::Esc => {
                self.focus = Focus::Stashes;
                Effect::None
            }
            KeyCode::Tab | KeyCode::BackTab if stash.is_some() => {
                let order = [Focus::Stashes, Focus::Files, Focus::Diff];
                let i = order.iter().position(|f| *f == self.focus).unwrap_or(0);
                let step = if k.code == KeyCode::Tab { 1 } else { 2 };
                self.focus = order[(i + step) % 3];
                Effect::None
            }
            KeyCode::Char('n') => Effect::OpenPicker,
            KeyCode::Char('a') if self.focus != Focus::Diff => stash.map_or(Effect::None, Effect::Apply),
            KeyCode::Char('p') if self.focus != Focus::Diff => stash.map_or(Effect::None, Effect::Pop),
            KeyCode::Char('d') if self.focus != Focus::Diff && stash.is_some() => {
                self.confirm_drop = Some(self.sel);
                Effect::None
            }
            _ => Effect::None,
        }
    }

    pub fn hotkeys(&self) -> Vec<(&'static str, &'static str)> {
        if self.confirm_drop.is_some() {
            return vec![("y", "Drop"), ("any key", "Cancel")];
        }
        match (self.screen, self.focus) {
            (Screen::Message, _) => vec![("Enter", "Stash"), ("Esc", "Back")],
            (Screen::Pick, Focus::Diff) => vec![("j/k", "Scroll"), ("Tab", "Files"), ("Esc", "Back")],
            (Screen::Pick, _) => vec![("Space", "Toggle"), ("A", "All"), ("Enter", "Next"), ("Tab", "Diff"), ("Esc", "Back")],
            (Screen::Browse, Focus::Diff) => vec![("j/k", "Scroll"), ("PgUp/PgDn", "Page"), ("Tab", "Stashes"), ("Esc", "Back"), ("q", "Quit")],
            (Screen::Browse, _) => vec![("a", "Apply"), ("p", "Pop"), ("d", "Drop"), ("n", "New"), ("Tab", "Focus"), ("^R", "Reload"), ("q", "Quit")],
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use gw_parse::Entry;

    fn stash(i: usize) -> Stash {
        Stash { reference: format!("stash@{{{i}}}"), index: i, oid: format!("oid{i}"), age: "1 hour ago".into(), branch: Some("main".into()), message: format!("m{i}") }
    }

    fn item(group: Group, path: &str) -> Item {
        let x = if group == Group::Untracked { '?' } else { 'M' };
        Item { group, entry: Entry { x, y: '.', unmerged: false, path: path.into(), orig: None, submodule: None }, stat: None, check: None }
    }

    fn key(c: KeyCode) -> KeyEvent {
        KeyEvent::new(c, KeyModifiers::NONE)
    }

    fn ch(c: char) -> KeyEvent {
        key(KeyCode::Char(c))
    }

    fn app() -> App {
        let mut a = App::new(vec![stash(0), stash(1)], String::new());
        a.resize(150, 30);
        a
    }

    #[test]
    fn focus_cycle_and_files_requests() {
        let mut a = app();
        assert_eq!(a.files_wanted().unwrap().oid, "oid0");
        a.set_files("oid0", vec![item(Group::Staged, "a.rs"), item(Group::Untracked, "n.txt")]);
        assert!(a.files_wanted().is_none());
        assert!(matches!(a.diff_wanted(), Some(DiffSource::Stash { file, .. }) if file.entry.path == "a.rs"));
        a.key(key(KeyCode::Tab));
        assert_eq!(a.focus, Focus::Files);
        a.key(ch('j'));
        assert!(matches!(a.diff_wanted(), Some(DiffSource::Stash { file, .. }) if file.entry.path == "n.txt"));
        a.key(key(KeyCode::Tab));
        assert_eq!(a.focus, Focus::Diff);
        a.key(key(KeyCode::Tab));
        assert_eq!(a.focus, Focus::Stashes);
        a.key(key(KeyCode::BackTab));
        assert_eq!(a.focus, Focus::Diff);
        a.key(key(KeyCode::Esc));
        a.key(ch('j'));
        assert_eq!(a.files_wanted().unwrap().oid, "oid1", "new selection wants its files");
        assert!(a.diff_wanted().is_none(), "no diff until files are loaded");
    }

    #[test]
    fn empty_list() {
        let mut a = App::new(vec![], String::new());
        assert!(a.selected().is_none() && a.files_wanted().is_none());
        for c in ['a', 'p', 'd'] {
            assert_eq!(a.key(ch(c)), Effect::None);
        }
        a.key(key(KeyCode::Tab));
        assert_eq!(a.focus, Focus::Stashes);
        assert_eq!(a.key(ch('n')), Effect::OpenPicker);
    }

    #[test]
    fn apply_pop_drop() {
        let mut a = app();
        a.key(ch('j'));
        assert_eq!(a.key(ch('a')), Effect::Apply(stash(1)));
        assert_eq!(a.key(ch('p')), Effect::Pop(stash(1)));
        a.key(ch('d'));
        assert_eq!(a.hotkeys()[0], ("y", "Drop"));
        assert_eq!(a.key(ch('y')), Effect::Drop(stash(1)));
        for cancel in [ch('n'), key(KeyCode::Esc)] {
            a.key(ch('d'));
            assert_eq!(a.key(cancel), Effect::None);
            assert!(a.confirm_drop.is_none());
        }
        assert_eq!(a.key(KeyEvent::new(KeyCode::Char('r'), KeyModifiers::CONTROL)), Effect::Reload);
        assert_eq!(a.key(ch('q')), Effect::Quit);
    }

    #[test]
    fn reload_keeps_stash_by_oid() {
        let mut a = app();
        a.key(ch('j'));
        // A new stash pushed in front: oid1 is now index 2.
        a.replace_stashes(vec![stash(9), stash(0), stash(1)]);
        assert_eq!(a.selected().unwrap().oid, "oid1");
    }

    #[test]
    fn new_stash_flow() {
        let mut a = app();
        a.open_picker(vec![]);
        assert_eq!(a.screen, Screen::Browse);
        assert!(a.status.as_ref().unwrap().0.contains("no local changes"));
        let mut r = item(Group::Staged, "new.rs");
        r.entry.x = 'R';
        r.entry.orig = Some("old.rs".into());
        a.open_picker(vec![item(Group::Changed, "a.rs"), r, item(Group::Untracked, "n.txt")]);
        assert_eq!(a.screen, Screen::Pick);
        assert!(matches!(a.diff_wanted(), Some(DiffSource::Work(it)) if it.entry.path == "a.rs"));
        // Untick n.txt: subset with both rename paths, no -u.
        a.key(ch('G'));
        a.key(ch(' '));
        a.key(key(KeyCode::Enter));
        assert_eq!(a.screen, Screen::Message);
        for c in "wip x".chars() {
            a.key(ch(c));
        }
        a.key(key(KeyCode::Backspace));
        assert_eq!(
            a.key(key(KeyCode::Enter)),
            Effect::Push { paths: vec!["a.rs".into(), "new.rs".into(), "old.rs".into()], untracked: false, message: Some("wip".into()), all: false }
        );
        // Esc steps back: prompt -> picker -> browse.
        a.key(key(KeyCode::Esc));
        assert_eq!(a.screen, Screen::Pick);
        // All ticked, empty message.
        a.key(ch(' '));
        a.key(key(KeyCode::Enter));
        a.input.clear();
        assert_eq!(a.key(key(KeyCode::Enter)), Effect::Push { paths: vec![], untracked: true, message: None, all: true });
        a.key(key(KeyCode::Esc));
        // A toggles all; nothing ticked: Enter refuses.
        a.key(ch('A'));
        assert!(a.pick.iter().all(|r| !r.1));
        a.key(key(KeyCode::Enter));
        assert_eq!(a.screen, Screen::Pick);
        assert!(a.status.as_ref().unwrap().0.contains("nothing selected"));
        a.key(key(KeyCode::Esc));
        assert_eq!(a.screen, Screen::Browse);
        // After the push the loop reloads: new stash selected, list focused.
        a.key(ch('j'));
        a.open_picker(vec![item(Group::Changed, "a.rs")]);
        a.replace_stashes(vec![stash(7), stash(0), stash(1)]);
        assert_eq!((a.sel, a.focus, a.screen), (0, Focus::Stashes, Screen::Browse));
    }
}
