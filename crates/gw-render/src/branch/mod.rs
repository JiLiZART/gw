//! `gw branch -i` and the bare `gw switch` picker: pure state + key handling.
//! Git work is requested through [`Effect`]s; the binary runs them and feeds results back.

pub mod view;

use std::collections::HashMap;

use gw_parse::Branch;
use ratatui::crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

use crate::stage::Item;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Focus {
    Branches,
    Files,
    Diff,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Prompt {
    /// Name for a new branch from the selected one.
    Create,
    /// New name, prefilled with the old one.
    Rename,
    Delete,
    /// `git branch -d` refused: not fully merged.
    Force,
    DeleteRemote,
    Merge,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Effect {
    None,
    Quit,
    Reload,
    Checkout(Branch),
    Create { name: String, from: Branch },
    Delete { branch: Branch, force: bool },
    DeleteRemote(Branch),
    Rename { from: Branch, to: String },
    Merge(Branch),
}

/// Rows of the branch pane.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Row {
    Header(&'static str),
    Branch(usize),
}

pub struct App {
    /// Current first, then local by recency, then remote by recency.
    pub branches: Vec<Branch>,
    pub sel: usize,
    pub scroll: usize,
    pub files: Vec<Item>,
    files_for: Option<String>,
    cache: HashMap<String, Vec<Item>>,
    pub file_sel: usize,
    pub file_scroll: usize,
    pub focus: Focus,
    pub diff: Vec<String>,
    diff_for: Option<(String, String)>,
    pub diff_scroll: usize,
    pub prompt: Option<Prompt>,
    /// Branch the open prompt acts on.
    pub target: Option<Branch>,
    pub input: String,
    /// (message, is_error)
    pub status: Option<(String, bool)>,
    /// Current branch name, `HEAD` when detached.
    pub current: String,
    /// `gw switch` picker: only `Enter` acts.
    pub pick: bool,
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

fn ordered(mut b: Vec<Branch>) -> Vec<Branch> {
    // Stable: keeps git's committer-date order inside each group.
    b.sort_by_key(|b| (b.remote, !b.current));
    b
}

/// Cache key of the files pane: the working tree for the current branch, else the tip.
fn files_key(b: &Branch) -> String {
    if b.current { "\0work".into() } else { b.oid.clone() }
}

impl App {
    pub fn new(branches: Vec<Branch>, prefix: String) -> Self {
        let branches = ordered(branches);
        let current = branches.iter().find(|b| b.current).map_or("HEAD".into(), |b| b.name.clone());
        App {
            branches,
            sel: 0,
            scroll: 0,
            files: Vec::new(),
            files_for: None,
            cache: HashMap::new(),
            file_sel: 0,
            file_scroll: 0,
            focus: Focus::Branches,
            diff: Vec::new(),
            diff_for: None,
            diff_scroll: 0,
            prompt: None,
            target: None,
            input: String::new(),
            status: None,
            current,
            pick: false,
            prefix,
            width: 80,
            height: 24,
            color: true,
        }
    }

    pub fn resize(&mut self, w: u16, h: u16) {
        self.width = w;
        self.height = h;
        self.follow();
        let h = self.body_height();
        scroll_to(self.file_sel, &mut self.file_scroll, h);
    }

    /// Lines inside a pane: screen minus status line, hotkey bar and borders.
    pub fn body_height(&self) -> usize {
        (self.height as usize).saturating_sub(4).max(1)
    }

    pub fn rows(&self) -> Vec<Row> {
        let mut out = Vec::new();
        for (i, b) in self.branches.iter().enumerate() {
            if i == 0 && !b.remote {
                out.push(Row::Header("Local"));
            }
            if b.remote && (i == 0 || !self.branches[i - 1].remote) {
                out.push(Row::Header("Remote"));
            }
            out.push(Row::Branch(i));
        }
        out
    }

    fn row_of_sel(&self) -> usize {
        self.rows().iter().position(|r| *r == Row::Branch(self.sel)).unwrap_or(0)
    }

    fn follow(&mut self) {
        let (r, h) = (self.row_of_sel(), self.body_height());
        scroll_to(r, &mut self.scroll, h);
        // Keep the group header in view at the top.
        if self.scroll + 1 == r && r > 0 && matches!(self.rows()[r - 1], Row::Header(_)) {
            self.scroll = r - 1;
        }
    }

    pub fn selected(&self) -> Option<&Branch> {
        self.branches.get(self.sel)
    }

    /// The branch whose files should be loaded, when they are not loaded or cached yet.
    pub fn files_wanted(&mut self) -> Option<Branch> {
        let b = self.selected()?.clone();
        let key = files_key(&b);
        if self.files_for.as_deref() == Some(&key) {
            return None;
        }
        if let Some(files) = self.cache.get(&key).cloned() {
            self.show_files(key, files);
            return None;
        }
        Some(b)
    }

    pub fn set_files(&mut self, b: &Branch, files: Vec<Item>) {
        let key = files_key(b);
        self.cache.insert(key.clone(), files.clone());
        self.show_files(key, files);
    }

    fn show_files(&mut self, key: String, files: Vec<Item>) {
        self.files = files;
        self.files_for = Some(key);
        self.file_sel = 0;
        self.file_scroll = 0;
    }

    /// Files are loaded for the selected branch.
    pub fn files_ready(&self) -> bool {
        self.selected().is_some_and(|b| self.files_for.as_deref() == Some(&files_key(b)))
    }

    /// `(branch, file)` whose diff should be loaded, when it is not loaded yet.
    pub fn diff_wanted(&self) -> Option<(Branch, Item)> {
        if !self.files_ready() {
            return None;
        }
        let b = self.selected()?;
        let it = self.files.get(self.file_sel)?;
        let id = (files_key(b), format!("{:?}{}", it.group, it.entry.path));
        (self.diff_for.as_ref() != Some(&id)).then(|| (b.clone(), it.clone()))
    }

    pub fn set_diff(&mut self, b: &Branch, it: &Item, lines: Vec<String>) {
        self.diff = lines;
        self.diff_for = Some((files_key(b), format!("{:?}{}", it.group, it.entry.path)));
        self.diff_scroll = 0;
    }

    /// The file whose diff is shown (for the pane title).
    pub fn diff_item(&self) -> Option<&Item> {
        self.files.get(self.file_sel).filter(|_| self.files_ready())
    }

    /// New list after a reload or an action. Selects `select` by name when given, else the
    /// same branch by name, else the same position.
    pub fn replace_branches(&mut self, branches: Vec<Branch>, select: Option<&str>) {
        let keep = select.map(str::to_string).or_else(|| self.selected().map(|b| b.name.clone()));
        self.branches = ordered(branches);
        self.current = self.branches.iter().find(|b| b.current).map_or("HEAD".into(), |b| b.name.clone());
        self.sel = keep
            .and_then(|n| self.branches.iter().position(|b| b.name == n))
            .unwrap_or(self.sel.min(self.branches.len().saturating_sub(1)));
        self.files_for = None;
        self.files.clear();
        self.cache.clear();
        self.diff_for = None;
        self.diff.clear();
        self.prompt = None;
        self.target = None;
        if self.branches.is_empty() {
            self.focus = Focus::Branches;
        }
        self.follow();
    }

    /// After `git branch -d` refused an unmerged branch.
    pub fn ask_force(&mut self, b: Branch) {
        self.target = Some(b);
        self.prompt = Some(Prompt::Force);
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

    fn open(&mut self, p: Prompt, b: Branch, input: &str) {
        self.prompt = Some(p);
        self.target = Some(b);
        self.input = input.into();
    }

    pub fn key(&mut self, k: KeyEvent) -> Effect {
        let ctrl = k.modifiers.contains(KeyModifiers::CONTROL);
        if ctrl && k.code == KeyCode::Char('c') {
            return Effect::Quit;
        }
        if let Some(p) = self.prompt {
            return self.key_prompt(p, k);
        }
        self.status = None;
        if let Some(d) = self.nav_delta(&k) {
            let h = self.body_height();
            match self.focus {
                Focus::Branches => {
                    self.sel = moved(self.sel, d, self.branches.len());
                    self.follow();
                }
                Focus::Files => {
                    self.file_sel = moved(self.file_sel, d, self.files.len());
                    scroll_to(self.file_sel, &mut self.file_scroll, h);
                }
                Focus::Diff => {
                    let max = self.diff.len().saturating_sub(h) as isize;
                    self.diff_scroll = (self.diff_scroll as isize).saturating_add(d).clamp(0, max.max(0)) as usize;
                }
            }
            return Effect::None;
        }
        let Some(b) = self.selected().cloned() else {
            return match k.code {
                KeyCode::Char('r') if ctrl => Effect::Reload,
                KeyCode::Char('q') | KeyCode::Esc => Effect::Quit,
                _ => Effect::None,
            };
        };
        let msg = |s: &mut Self, m: &str| {
            s.status = Some((m.into(), true));
            Effect::None
        };
        match k.code {
            KeyCode::Char('r') if ctrl => Effect::Reload,
            KeyCode::Char('q') | KeyCode::Esc => Effect::Quit,
            KeyCode::Tab | KeyCode::BackTab => {
                let order = [Focus::Branches, Focus::Files, Focus::Diff];
                let i = order.iter().position(|f| *f == self.focus).unwrap_or(0);
                self.focus = order[(i + if k.code == KeyCode::Tab { 1 } else { 2 }) % 3];
                Effect::None
            }
            KeyCode::Enter => Effect::Checkout(b),
            _ if self.pick => Effect::None,
            KeyCode::Char('n') => {
                self.open(Prompt::Create, b, "");
                Effect::None
            }
            KeyCode::Char('d') if b.current => msg(self, "cannot delete the current branch"),
            KeyCode::Char('d') => {
                self.open(if b.remote { Prompt::DeleteRemote } else { Prompt::Delete }, b, "");
                Effect::None
            }
            KeyCode::Char('R') if b.remote => msg(self, "rename works on local branches"),
            KeyCode::Char('R') => {
                let name = b.name.clone();
                self.open(Prompt::Rename, b, &name);
                Effect::None
            }
            KeyCode::Char('m') if b.current => msg(self, "cannot merge a branch into itself"),
            KeyCode::Char('m') => {
                self.open(Prompt::Merge, b, "");
                Effect::None
            }
            _ => Effect::None,
        }
    }

    fn key_prompt(&mut self, p: Prompt, k: KeyEvent) -> Effect {
        if matches!(p, Prompt::Create | Prompt::Rename) {
            match k.code {
                KeyCode::Esc => self.prompt = None,
                KeyCode::Enter => {
                    self.prompt = None;
                    let name = self.input.trim().to_string();
                    let Some(b) = self.target.take() else { return Effect::None };
                    if name.is_empty() || (p == Prompt::Rename && name == b.name) {
                        return Effect::None;
                    }
                    return if p == Prompt::Create { Effect::Create { name, from: b } } else { Effect::Rename { from: b, to: name } };
                }
                KeyCode::Backspace => {
                    self.input.pop();
                }
                KeyCode::Char(c) if !k.modifiers.contains(KeyModifiers::CONTROL) => self.input.push(c),
                _ => {}
            }
            return Effect::None;
        }
        self.prompt = None;
        match (k.code, self.target.take()) {
            (KeyCode::Char('y'), Some(b)) => match p {
                Prompt::Delete => Effect::Delete { branch: b, force: false },
                Prompt::Force => Effect::Delete { branch: b, force: true },
                Prompt::DeleteRemote => Effect::DeleteRemote(b),
                _ => Effect::Merge(b),
            },
            _ => Effect::None,
        }
    }

    pub fn hotkeys(&self) -> Vec<(&'static str, &'static str)> {
        match self.prompt {
            Some(Prompt::Create | Prompt::Rename) => return vec![("Enter", "OK"), ("Esc", "Cancel")],
            Some(_) => return vec![("y", "Yes"), ("any key", "Cancel")],
            None => {}
        }
        if self.pick {
            return vec![("Enter", "Switch"), ("Tab", "Focus"), ("^R", "Reload"), ("q", "Quit")];
        }
        vec![("Enter", "Checkout"), ("n", "New"), ("d", "Delete"), ("R", "Rename"), ("m", "Merge"), ("Tab", "Focus"), ("^R", "Reload"), ("q", "Quit")]
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::status::Group;
    use gw_parse::Entry;

    pub(crate) fn br(name: &str, current: bool, remote: bool) -> Branch {
        Branch {
            name: name.into(),
            current,
            upstream: None,
            ahead: 0,
            behind: 0,
            gone: false,
            age: "1 day ago".into(),
            subject: format!("{name} work"),
            oid: format!("oid-{name}"),
            remote,
        }
    }

    fn item(path: &str) -> Item {
        Item { group: Group::Staged, entry: Entry { x: 'M', y: '.', unmerged: false, path: path.into(), orig: None, submodule: None }, stat: None, check: None }
    }

    fn key(c: KeyCode) -> KeyEvent {
        KeyEvent::new(c, KeyModifiers::NONE)
    }

    fn ch(c: char) -> KeyEvent {
        key(KeyCode::Char(c))
    }

    fn app() -> App {
        // git order (by date) with the current branch in the middle.
        let mut a = App::new(vec![br("feat", false, false), br("origin/main", false, true), br("main", true, false), br("origin/fix", false, true)], String::new());
        a.resize(150, 30);
        a
    }

    #[test]
    fn rows_and_selection() {
        let mut a = app();
        let names: Vec<String> = a
            .rows()
            .iter()
            .map(|r| match r {
                Row::Header(h) => h.to_string(),
                Row::Branch(i) => a.branches[*i].name.clone(),
            })
            .collect();
        assert_eq!(names, ["Local", "main", "feat", "Remote", "origin/main", "origin/fix"]);
        assert_eq!(a.current, "main");
        a.key(ch('j'));
        a.key(ch('j'));
        assert_eq!(a.selected().unwrap().name, "origin/main", "headers are skipped");
        a.key(ch('G'));
        a.key(ch('j'));
        assert_eq!(a.selected().unwrap().name, "origin/fix");
    }

    #[test]
    fn files_requested_once_per_branch() {
        let mut a = app();
        let b = a.files_wanted().unwrap();
        assert!(b.current);
        a.set_files(&b, vec![item("b.txt")]);
        assert!(a.files_wanted().is_none());
        assert_eq!(a.diff_wanted().unwrap().1.entry.path, "b.txt");
        a.key(ch('j'));
        let feat = a.files_wanted().unwrap();
        assert_eq!(feat.name, "feat");
        assert!(a.diff_wanted().is_none(), "no diff until files are loaded");
        a.set_files(&feat, vec![item("a.rs")]);
        a.key(ch('k'));
        assert!(a.files_wanted().is_none(), "cached");
        assert_eq!(a.files[0].entry.path, "b.txt");
        a.replace_branches(a.branches.clone(), None);
        assert!(a.files_wanted().is_some(), "reload drops the cache");
    }

    #[test]
    fn focus_cycle_and_empty() {
        let mut a = app();
        a.key(key(KeyCode::Tab));
        assert_eq!(a.focus, Focus::Files);
        a.key(key(KeyCode::Tab));
        assert_eq!(a.focus, Focus::Diff);
        a.key(key(KeyCode::BackTab));
        assert_eq!(a.focus, Focus::Files);
        let mut e = App::new(vec![], String::new());
        assert!(e.selected().is_none() && e.files_wanted().is_none());
        for c in ['n', 'd', 'R', 'm'] {
            assert_eq!(e.key(ch(c)), Effect::None);
        }
        assert_eq!(e.key(key(KeyCode::Enter)), Effect::None);
        assert_eq!(e.key(ch('q')), Effect::Quit);
    }

    #[test]
    fn checkout_create_rename() {
        let mut a = app();
        a.key(ch('j'));
        assert_eq!(a.key(key(KeyCode::Enter)), Effect::Checkout(br("feat", false, false)));
        a.key(ch('n'));
        assert_eq!(a.prompt, Some(Prompt::Create));
        for c in "feat-2x".chars() {
            a.key(ch(c));
        }
        a.key(key(KeyCode::Backspace));
        assert_eq!(a.key(key(KeyCode::Enter)), Effect::Create { name: "feat-2".into(), from: br("feat", false, false) });
        a.key(ch('n'));
        assert_eq!(a.key(key(KeyCode::Enter)), Effect::None, "empty name does nothing");
        a.key(ch('n'));
        a.key(ch('x'));
        assert_eq!(a.key(key(KeyCode::Esc)), Effect::None);
        assert!(a.prompt.is_none());
        a.key(ch('R'));
        assert_eq!(a.input, "feat");
        for c in "ure".chars() {
            a.key(ch(c));
        }
        assert_eq!(a.key(key(KeyCode::Enter)), Effect::Rename { from: br("feat", false, false), to: "feature".into() });
        a.key(ch('j'));
        a.key(ch('R'));
        assert!(a.prompt.is_none() && a.status.as_ref().unwrap().0.contains("local branches"));
    }

    #[test]
    fn delete_force_remote_merge() {
        let mut a = app();
        a.key(ch('d'));
        assert!(a.prompt.is_none() && a.status.as_ref().unwrap().0.contains("current branch"));
        a.key(ch('m'));
        assert!(a.status.as_ref().unwrap().0.contains("into itself"));
        a.key(ch('j'));
        a.key(ch('d'));
        assert_eq!(a.prompt, Some(Prompt::Delete));
        assert_eq!(a.key(ch('n')), Effect::None, "cancel");
        a.key(ch('d'));
        let feat = br("feat", false, false);
        assert_eq!(a.key(ch('y')), Effect::Delete { branch: feat.clone(), force: false });
        a.ask_force(feat.clone());
        assert_eq!(a.hotkeys()[0], ("y", "Yes"));
        assert_eq!(a.key(ch('y')), Effect::Delete { branch: feat.clone(), force: true });
        a.ask_force(feat);
        assert_eq!(a.key(key(KeyCode::Esc)), Effect::None);
        a.key(ch('m'));
        assert_eq!(a.key(ch('y')), Effect::Merge(br("feat", false, false)));
        a.key(ch('j'));
        a.key(ch('d'));
        assert_eq!(a.prompt, Some(Prompt::DeleteRemote));
        assert_eq!(a.key(ch('y')), Effect::DeleteRemote(br("origin/main", false, true)));
        assert_eq!(a.key(KeyEvent::new(KeyCode::Char('r'), KeyModifiers::CONTROL)), Effect::Reload);
        assert_eq!(a.key(key(KeyCode::Esc)), Effect::Quit);
    }

    #[test]
    fn picker_keys() {
        let mut a = app();
        a.pick = true;
        a.key(ch('j'));
        for c in ['n', 'd', 'R', 'm'] {
            assert_eq!(a.key(ch(c)), Effect::None);
            assert!(a.prompt.is_none() && a.status.is_none());
        }
        assert_eq!(a.hotkeys().iter().map(|k| k.0).collect::<Vec<_>>(), ["Enter", "Tab", "^R", "q"]);
        assert_eq!(a.key(key(KeyCode::Enter)), Effect::Checkout(br("feat", false, false)));
        assert_eq!(a.key(ch('q')), Effect::Quit);
    }

    #[test]
    fn reload_selects_by_name() {
        let mut a = app();
        a.key(ch('j'));
        let mut l = a.branches.clone();
        l.retain(|b| b.name != "main");
        l.push(br("feature", true, false));
        a.replace_branches(l.clone(), None);
        assert_eq!(a.selected().unwrap().name, "feat");
        assert_eq!(a.current, "feature");
        a.replace_branches(l, Some("feature"));
        assert_eq!(a.sel, 0);
    }
}
