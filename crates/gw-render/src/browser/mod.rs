//! `gw log` browser: pure state + key handling. IO is requested through [`Effect`]s,
//! which the binary runs and feeds back through the `set_*` / `*_result` methods.

pub mod fuzzy;
pub mod view;

use gw_parse::LogRow;
use ratatui::crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

use crate::style::sanitize;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Scope {
    Head,
    All,
    Branch(String),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Filters {
    pub scope: Scope,
    pub since: Option<String>,
    pub until: Option<String>,
    pub author: Option<String>,
    /// Arguments the user passed to `gw log`, appended unchanged.
    pub user_args: Vec<String>,
}

impl Filters {
    pub fn new(user_args: Vec<String>) -> Self {
        Self { scope: Scope::Head, since: None, until: None, author: None, user_args }
    }

    /// git log arguments for the current filters (format and graph flags not included).
    pub fn git_args(&self) -> Vec<String> {
        let mut a = Vec::new();
        // A revision given by the user replaces the default HEAD scope.
        let user_rev = self.user_args.iter().take_while(|a| *a != "--").any(|a| !a.starts_with('-'));
        match &self.scope {
            Scope::Head if user_rev => {}
            Scope::Head => a.push("HEAD".into()),
            Scope::All => a.push("--all".into()),
            Scope::Branch(b) => a.push(b.clone()),
        }
        if let Some(s) = &self.since {
            a.push(format!("--since={s}"));
        }
        if let Some(u) = &self.until {
            a.push(format!("--until={u}"));
        }
        if let Some(au) = &self.author {
            a.push(format!("--author={au}"));
        }
        a.extend(self.user_args.iter().cloned());
        a
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ResetMode {
    Soft,
    Mixed,
    Hard,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Action {
    CherryPick(String),
    Revert(String),
    Checkout(String),
    NewBranch { name: String, sha: String },
    Tag { name: String, sha: String },
    Reset(ResetMode, String),
}

impl Action {
    pub fn git_args(&self) -> Vec<String> {
        let v = |a: &[&str]| a.iter().map(|s| s.to_string()).collect();
        match self {
            Action::CherryPick(s) => v(&["cherry-pick", s]),
            Action::Revert(s) => v(&["revert", s]),
            Action::Checkout(s) => v(&["checkout", "--detach", s]),
            Action::NewBranch { name, sha } => v(&["branch", "--", name, sha]),
            Action::Tag { name, sha } => v(&["tag", "--", name, sha]),
            Action::Reset(m, s) => {
                let flag = match m {
                    ResetMode::Soft => "--soft",
                    ResetMode::Mixed => "--mixed",
                    ResetMode::Hard => "--hard",
                };
                v(&["reset", flag, s])
            }
        }
    }

    /// Actions that may open an editor or print conflicts get the real terminal;
    /// branch/tag creation runs captured so errors can stay in their modal.
    pub fn needs_terminal(&self) -> bool {
        !matches!(self, Action::NewBranch { .. } | Action::Tag { .. })
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Effect {
    None,
    Quit,
    /// Restart `git log` with the current filters.
    Reload,
    LoadFiles(String),
    LoadDiff { sha: String, path: String },
    LoadBranches,
    LoadAuthors,
    ValidateDate { since: String, until: String },
    PrepareReset(ResetMode, String),
    Run(Action),
    Copy(String),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Focus {
    List,
    Sidebar,
    Diff,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FileEntry {
    pub code: char,
    pub path: String,
    pub orig: Option<String>,
    /// `None`: no counts known; `Some(None)`: binary.
    pub stat: Option<Option<(u64, u64)>>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Diff {
    pub sha: String,
    pub path: String,
    pub lines: Vec<String>,
    pub scroll: usize,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NameKind {
    Branch,
    Tag,
}

/// A filterable list modal (branches or authors).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Picker {
    pub input: String,
    pub items: Vec<String>,
    pub shown: Vec<usize>,
    pub sel: usize,
}

impl Picker {
    pub fn new(items: Vec<String>) -> Self {
        let mut p = Self { input: String::new(), shown: Vec::new(), items, sel: 0 };
        p.refilter();
        p
    }

    fn refilter(&mut self) {
        let mut scored: Vec<(i64, usize)> = self
            .items
            .iter()
            .enumerate()
            .filter_map(|(i, it)| if self.input.is_empty() { Some((0, i)) } else { fuzzy::score(&self.input, &[it]).map(|(s, _)| (s, i)) })
            .collect();
        scored.sort_by_key(|(s, i)| (std::cmp::Reverse(*s), *i));
        self.shown = scored.into_iter().map(|(_, i)| i).collect();
        self.sel = 0;
    }

    pub fn selected(&self) -> Option<&str> {
        self.shown.get(self.sel).map(|&i| self.items[i].as_str())
    }

    /// Text editing and selection keys; returns true if handled.
    fn key(&mut self, k: KeyEvent) -> bool {
        match k.code {
            KeyCode::Up => self.sel = self.sel.saturating_sub(1),
            KeyCode::Down => self.sel = (self.sel + 1).min(self.shown.len().saturating_sub(1)),
            KeyCode::Backspace => {
                self.input.pop();
                self.refilter();
            }
            KeyCode::Char(c) if !k.modifiers.contains(KeyModifiers::CONTROL) => {
                self.input.push(c);
                self.refilter();
            }
            _ => return false,
        }
        true
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Overlay {
    None,
    Search,
    Branch(Picker),
    /// `labels[i]` = "Name <email>" as shown; items carry the same text with counts.
    Author(Picker),
    Date { since: String, until: String, field: usize, error: Option<String> },
    Actions,
    ResetMode,
    Name { kind: NameKind, input: String, error: Option<String> },
    Confirm { action: Action, lines: Vec<String>, need_y: bool },
}

pub struct App {
    pub rows: Vec<LogRow>,
    /// Indices into `rows` currently shown.
    pub view: Vec<usize>,
    /// Search match positions per shown row: [short hash, author, subject, refs].
    pub marks: Vec<Vec<Vec<usize>>>,
    pub sel: usize,
    pub scroll: usize,
    pub loading: bool,
    pub filters: Filters,
    pub search: String,
    pub focus: Focus,
    pub files: Option<(String, Vec<FileEntry>)>,
    pub file_sel: usize,
    pub file_scroll: usize,
    pub diff: Option<Diff>,
    pub overlay: Overlay,
    /// (message, is_error)
    pub status: Option<(String, bool)>,
    /// Header label for HEAD: branch name or `detached at <sha>`.
    pub head: String,
    pub remotes: Vec<String>,
    /// Unix time used for relative ages.
    pub now: i64,
    pub width: u16,
    pub height: u16,
    reselect: Option<String>,
}

impl App {
    pub fn new(filters: Filters, head: String, remotes: Vec<String>, now: i64) -> Self {
        Self {
            rows: Vec::new(),
            view: Vec::new(),
            marks: Vec::new(),
            sel: 0,
            scroll: 0,
            loading: true,
            filters,
            search: String::new(),
            focus: Focus::List,
            files: None,
            file_sel: 0,
            file_scroll: 0,
            diff: None,
            overlay: Overlay::None,
            status: None,
            head,
            remotes,
            now,
            width: 80,
            height: 24,
            reselect: None,
        }
    }

    pub fn resize(&mut self, w: u16, h: u16) {
        self.width = w;
        self.height = h;
        self.ensure_visible();
    }

    /// Rows available for the commit list / sidebar / diff.
    pub fn body_height(&self) -> usize {
        let panel = if matches!(self.overlay, Overlay::Actions | Overlay::ResetMode) { 3 } else { 0 };
        // header + status + hotkeys, plus the pane borders
        (self.height as usize).saturating_sub(3 + panel + 2).max(1)
    }

    /// Wide enough to show the sidebar beside the list.
    pub fn wide(&self) -> bool {
        self.width >= 100
    }

    // ---- loading ----

    /// Starts a fresh load (after a filter change); keeps the selected commit if it reappears.
    pub fn begin_load(&mut self) {
        self.reselect = self.selected_hash().map(str::to_string);
        self.rows.clear();
        self.view.clear();
        self.marks.clear();
        self.sel = 0;
        self.scroll = 0;
        self.loading = true;
    }

    pub fn push_rows(&mut self, rows: Vec<LogRow>) {
        let start = self.rows.len();
        self.rows.extend(rows);
        if self.search.is_empty() {
            self.view.extend(start..self.rows.len());
            self.marks.resize(self.view.len(), Vec::new());
        } else {
            self.refilter();
        }
        if let Some(h) = self.reselect.clone() {
            if let Some(p) = self.view.iter().position(|&i| self.rows[i].commit.as_ref().is_some_and(|c| c.hash == h)) {
                self.sel = p;
                self.reselect = None;
                self.ensure_visible();
                return;
            }
        }
        if !self.is_commit(self.sel) {
            self.move_by(1);
        }
    }

    pub fn finish_load(&mut self) {
        self.loading = false;
        self.reselect = None;
    }

    // ---- selection ----

    fn is_commit(&self, pos: usize) -> bool {
        self.view.get(pos).is_some_and(|&i| self.rows[i].commit.is_some())
    }

    pub fn selected_hash(&self) -> Option<&str> {
        self.view.get(self.sel).and_then(|&i| self.rows[i].commit.as_ref()).map(|c| c.hash.as_str())
    }

    pub fn selected_short(&self) -> Option<&str> {
        self.view.get(self.sel).and_then(|&i| self.rows[i].commit.as_ref()).map(|c| c.short.as_str())
    }

    /// Moves by `delta` lines, landing on a commit row (searching forward, then backward).
    fn move_by(&mut self, delta: isize) {
        if self.view.is_empty() {
            return;
        }
        let last = self.view.len() - 1;
        let mut p = (self.sel as isize + delta).clamp(0, last as isize) as usize;
        if delta.abs() == 1 {
            // single steps skip connector lines in the direction of travel
            while !self.is_commit(p) && p > 0 && p < last {
                p = (p as isize + delta) as usize;
            }
        }
        if !self.is_commit(p) {
            let fwd = (p..=last).find(|&q| self.is_commit(q));
            let back = (0..p).rev().find(|&q| self.is_commit(q));
            p = if delta < 0 { back.or(fwd) } else { fwd.or(back) }.unwrap_or(self.sel);
        }
        self.sel = p;
        self.ensure_visible();
    }

    fn ensure_visible(&mut self) {
        let h = self.body_height();
        if self.sel < self.scroll {
            self.scroll = self.sel;
        } else if self.sel >= self.scroll + h {
            self.scroll = self.sel + 1 - h;
        }
    }

    // ---- search ----

    fn refilter(&mut self) {
        let keep = self.selected_hash().map(str::to_string);
        self.view.clear();
        self.marks.clear();
        if self.search.trim().is_empty() {
            self.view.extend(0..self.rows.len());
            self.marks.resize(self.view.len(), Vec::new());
        } else {
            let mut hits: Vec<(i64, usize, Vec<Vec<usize>>)> = self
                .rows
                .iter()
                .enumerate()
                .filter_map(|(i, r)| {
                    let c = r.commit.as_ref()?;
                    let (author, subject, refs) = (sanitize(&c.author), sanitize(&c.subject), sanitize(&c.refs.join(", ")));
                    // The full hash is matched, but highlights are shown on the short hash.
                    fuzzy::score(&self.search, &[&c.hash, &author, &subject, &refs]).map(|(s, m)| (s, i, m))
                })
                .collect();
            hits.sort_by_key(|(s, i, _)| (std::cmp::Reverse(*s), *i));
            for (_, i, m) in hits {
                self.view.push(i);
                self.marks.push(m);
            }
        }
        self.sel = keep.and_then(|h| self.view.iter().position(|&i| self.rows[i].commit.as_ref().is_some_and(|c| c.hash == h))).unwrap_or(0);
        self.scroll = 0;
        if !self.is_commit(self.sel) {
            self.move_by(1);
        }
        self.ensure_visible();
    }

    pub fn set_search(&mut self, q: &str) {
        self.search = q.to_string();
        self.refilter();
    }

    // ---- results fed back by the binary ----

    pub fn set_files(&mut self, sha: String, files: Vec<FileEntry>) {
        self.files = Some((sha, files));
        self.file_sel = 0;
        self.file_scroll = 0;
    }

    /// The selected commit's file list is missing or stale.
    pub fn files_wanted(&self) -> Option<&str> {
        let sha = self.selected_hash()?;
        match &self.files {
            Some((s, _)) if s == sha => None,
            _ => Some(sha),
        }
    }

    pub fn set_diff(&mut self, sha: String, path: String, lines: Vec<String>) {
        self.diff = Some(Diff { sha, path, lines, scroll: 0 });
        self.focus = Focus::Diff;
    }

    pub fn open_branches(&mut self, names: Vec<String>) {
        self.overlay = Overlay::Branch(Picker::new(names));
    }

    pub fn open_authors(&mut self, authors: Vec<(u32, String)>) {
        self.overlay = Overlay::Author(Picker::new(authors.into_iter().map(|(_, a)| a).collect()));
    }

    pub fn date_error(&mut self, msg: String) {
        if let Overlay::Date { error, .. } = &mut self.overlay {
            *error = Some(msg);
        }
    }

    /// Valid dates: apply them. Caller then reloads.
    pub fn apply_date(&mut self, since: String, until: String) {
        let opt = |s: String| (!s.trim().is_empty()).then(|| s.trim().to_string());
        self.filters.since = opt(since);
        self.filters.until = opt(until);
        self.overlay = Overlay::None;
    }

    pub fn confirm_reset(&mut self, mode: ResetMode, sha: String, branch: String, at_risk: Vec<String>) {
        let short = sha.chars().take(7).collect::<String>();
        let name = match mode {
            ResetMode::Soft => "soft",
            ResetMode::Mixed => "mixed",
            ResetMode::Hard => "hard",
        };
        let mut lines = vec![format!("Reset {} to {short} ({name})?", sanitize(&branch))];
        let hard = mode == ResetMode::Hard;
        if hard {
            if at_risk.is_empty() {
                lines.push("No uncommitted changes will be lost.".into());
            } else {
                lines.push("Uncommitted changes in these files will be LOST:".into());
                lines.extend(at_risk.iter().take(8).map(|f| format!("  {}", sanitize(f))));
                if at_risk.len() > 8 {
                    lines.push(format!("  … and {} more", at_risk.len() - 8));
                }
            }
        }
        lines.push(if hard { "Press y to reset, Esc to cancel".into() } else { "Press Enter or y to reset, Esc to cancel".into() });
        self.overlay = Overlay::Confirm { action: Action::Reset(mode, sha), lines, need_y: hard };
    }

    /// Result of a git action. Name modals stay open on error.
    pub fn action_result(&mut self, ok: bool, msg: String) {
        if let (false, Overlay::Name { error, .. }) = (ok, &mut self.overlay) {
            *error = Some(msg);
            return;
        }
        self.overlay = Overlay::None;
        self.status = Some((msg, !ok));
    }

    // ---- keys ----

    pub fn key(&mut self, k: KeyEvent) -> Effect {
        if k.code == KeyCode::Char('c') && k.modifiers.contains(KeyModifiers::CONTROL) {
            return Effect::Quit;
        }
        if self.overlay != Overlay::None {
            return self.overlay_key(k);
        }
        self.status = None;
        match self.focus {
            Focus::Diff => self.diff_key(k),
            Focus::List | Focus::Sidebar => self.main_key(k),
        }
    }

    fn page(&self) -> isize {
        self.body_height().max(1) as isize
    }

    fn main_key(&mut self, k: KeyEvent) -> Effect {
        let ctrl = k.modifiers.contains(KeyModifiers::CONTROL);
        if self.focus == Focus::Sidebar {
            let n = self.files.as_ref().map_or(0, |f| f.1.len());
            let page = self.page();
            let step = |s: &mut usize, d: isize| *s = (*s as isize + d).clamp(0, n.saturating_sub(1) as isize) as usize;
            let handled = match k.code {
                KeyCode::Up | KeyCode::Char('k') => Some(step(&mut self.file_sel, -1)),
                KeyCode::Down | KeyCode::Char('j') => Some(step(&mut self.file_sel, 1)),
                KeyCode::PageUp => Some(step(&mut self.file_sel, -page)),
                KeyCode::PageDown => Some(step(&mut self.file_sel, page)),
                KeyCode::Home | KeyCode::Char('g') => Some(self.file_sel = 0),
                KeyCode::End | KeyCode::Char('G') => Some(self.file_sel = n.saturating_sub(1)),
                _ => None,
            };
            if handled.is_some() {
                let h = self.body_height().saturating_sub(1).max(1);
                if self.file_sel < self.file_scroll {
                    self.file_scroll = self.file_sel;
                } else if self.file_sel >= self.file_scroll + h {
                    self.file_scroll = self.file_sel + 1 - h;
                }
                return Effect::None;
            }
            if k.code == KeyCode::Enter {
                if let (Some((sha, files)), true) = (&self.files, n > 0) {
                    return Effect::LoadDiff { sha: sha.clone(), path: files[self.file_sel].path.clone() };
                }
                return Effect::None;
            }
        }
        match k.code {
            KeyCode::Char('q') => return Effect::Quit,
            KeyCode::Tab | KeyCode::BackTab => {
                self.focus = if self.focus == Focus::List { Focus::Sidebar } else { Focus::List };
            }
            KeyCode::Up | KeyCode::Char('k') => self.move_by(-1),
            KeyCode::Down | KeyCode::Char('j') => self.move_by(1),
            KeyCode::PageUp => self.move_by(-self.page()),
            KeyCode::PageDown => self.move_by(self.page()),
            KeyCode::Home | KeyCode::Char('g') => self.move_by(-(self.view.len() as isize)),
            KeyCode::End | KeyCode::Char('G') => self.move_by(self.view.len() as isize),
            KeyCode::Enter => self.focus = Focus::Sidebar,
            KeyCode::Esc if !self.search.is_empty() => self.set_search(""),
            KeyCode::Char('/') => self.overlay = Overlay::Search,
            KeyCode::Char('f') if ctrl => self.overlay = Overlay::Search,
            KeyCode::Char('a') => {
                self.filters.scope = if self.filters.scope == Scope::All { Scope::Head } else { Scope::All };
                return Effect::Reload;
            }
            KeyCode::Char('b') => return Effect::LoadBranches,
            KeyCode::Char('u') => return Effect::LoadAuthors,
            KeyCode::Char('d') => {
                self.overlay = Overlay::Date {
                    since: self.filters.since.clone().unwrap_or_default(),
                    until: self.filters.until.clone().unwrap_or_default(),
                    field: 0,
                    error: None,
                }
            }
            KeyCode::Char('x') => {
                let f = Filters::new(std::mem::take(&mut self.filters.user_args));
                self.filters = f;
                self.search.clear();
                return Effect::Reload;
            }
            KeyCode::Char(' ') if self.selected_hash().is_some() => self.overlay = Overlay::Actions,
            _ => {}
        }
        Effect::None
    }

    fn diff_key(&mut self, k: KeyEvent) -> Effect {
        let page = self.page();
        let Some(d) = &mut self.diff else {
            self.focus = Focus::Sidebar;
            return Effect::None;
        };
        let max = d.lines.len().saturating_sub(1) as isize;
        let mut scroll = |delta: isize| d.scroll = (d.scroll as isize + delta).clamp(0, max) as usize;
        match k.code {
            KeyCode::Esc | KeyCode::Char('q') | KeyCode::Backspace => {
                self.diff = None;
                self.focus = Focus::Sidebar;
            }
            KeyCode::Up | KeyCode::Char('k') => scroll(-1),
            KeyCode::Down | KeyCode::Char('j') => scroll(1),
            KeyCode::PageUp => scroll(-page),
            KeyCode::PageDown | KeyCode::Char(' ') => scroll(page),
            KeyCode::Home | KeyCode::Char('g') => scroll(-max - 1),
            KeyCode::End | KeyCode::Char('G') => scroll(max),
            _ => {}
        }
        Effect::None
    }

    fn overlay_key(&mut self, k: KeyEvent) -> Effect {
        let esc = k.code == KeyCode::Esc;
        let enter = k.code == KeyCode::Enter;
        let sha = self.selected_hash().map(str::to_string).unwrap_or_default();
        match &mut self.overlay {
            Overlay::None => Effect::None,
            Overlay::Search => {
                match k.code {
                    KeyCode::Esc => {
                        self.overlay = Overlay::None;
                        self.set_search("");
                    }
                    KeyCode::Enter => self.overlay = Overlay::None,
                    KeyCode::Backspace => {
                        let mut q = self.search.clone();
                        q.pop();
                        self.set_search(&q);
                    }
                    KeyCode::Up => self.move_by(-1),
                    KeyCode::Down => self.move_by(1),
                    KeyCode::Char(c) if !k.modifiers.contains(KeyModifiers::CONTROL) => {
                        let q = format!("{}{c}", self.search);
                        self.set_search(&q);
                    }
                    _ => {}
                }
                Effect::None
            }
            Overlay::Branch(p) => {
                if esc {
                    self.overlay = Overlay::None;
                } else if enter {
                    if let Some(b) = p.selected().map(str::to_string) {
                        self.filters.scope = Scope::Branch(b);
                        self.overlay = Overlay::None;
                        return Effect::Reload;
                    }
                } else {
                    p.key(k);
                }
                Effect::None
            }
            Overlay::Author(p) => {
                if esc {
                    self.overlay = Overlay::None;
                } else if enter {
                    let pattern = match p.selected() {
                        Some(a) => regex_escape(a),
                        None if !p.input.trim().is_empty() => p.input.trim().to_string(),
                        None => return Effect::None,
                    };
                    self.filters.author = Some(pattern);
                    self.overlay = Overlay::None;
                    return Effect::Reload;
                } else {
                    p.key(k);
                }
                Effect::None
            }
            Overlay::Date { since, until, field, error } => {
                match k.code {
                    KeyCode::Esc => self.overlay = Overlay::None,
                    KeyCode::Enter => return Effect::ValidateDate { since: since.clone(), until: until.clone() },
                    KeyCode::Tab | KeyCode::BackTab | KeyCode::Up | KeyCode::Down => *field = 1 - *field,
                    KeyCode::Backspace => {
                        if *field == 0 { since.pop() } else { until.pop() };
                        *error = None;
                    }
                    KeyCode::Char(c) if !k.modifiers.contains(KeyModifiers::CONTROL) => {
                        if *field == 0 { since.push(c) } else { until.push(c) };
                        *error = None;
                    }
                    _ => {}
                }
                Effect::None
            }
            Overlay::Actions => {
                self.overlay = Overlay::None;
                let short = self.selected_short().unwrap_or_default().to_string();
                match k.code {
                    KeyCode::Char('p') => return Effect::Run(Action::CherryPick(sha)),
                    KeyCode::Char('v') => return Effect::Run(Action::Revert(sha)),
                    KeyCode::Char('o') => {
                        self.overlay = Overlay::Confirm {
                            action: Action::Checkout(sha),
                            lines: vec![format!("Check out {short}?"), "HEAD will be detached (not on any branch).".into(), "Press Enter or y to continue, Esc to cancel".into()],
                            need_y: false,
                        }
                    }
                    KeyCode::Char('n') => self.overlay = Overlay::Name { kind: NameKind::Branch, input: String::new(), error: None },
                    KeyCode::Char('t') => self.overlay = Overlay::Name { kind: NameKind::Tag, input: String::new(), error: None },
                    KeyCode::Char('r') => self.overlay = Overlay::ResetMode,
                    KeyCode::Char('y') => {
                        self.status = Some((format!("Copied {short}"), false));
                        return Effect::Copy(sha);
                    }
                    _ => {}
                }
                Effect::None
            }
            Overlay::ResetMode => {
                self.overlay = Overlay::None;
                let mode = match k.code {
                    KeyCode::Char('s') => ResetMode::Soft,
                    KeyCode::Char('m') => ResetMode::Mixed,
                    KeyCode::Char('h') => ResetMode::Hard,
                    _ => return Effect::None,
                };
                Effect::PrepareReset(mode, sha)
            }
            Overlay::Name { kind, input, error } => {
                match k.code {
                    KeyCode::Esc => self.overlay = Overlay::None,
                    KeyCode::Enter if !input.trim().is_empty() => {
                        let name = input.trim().to_string();
                        return Effect::Run(match kind {
                            NameKind::Branch => Action::NewBranch { name, sha },
                            NameKind::Tag => Action::Tag { name, sha },
                        });
                    }
                    KeyCode::Backspace => {
                        input.pop();
                        *error = None;
                    }
                    KeyCode::Char(c) if !k.modifiers.contains(KeyModifiers::CONTROL) => {
                        input.push(c);
                        *error = None;
                    }
                    _ => {}
                }
                Effect::None
            }
            Overlay::Confirm { action, need_y, .. } => {
                let go = k.code == KeyCode::Char('y') || (enter && !*need_y);
                let action = action.clone();
                if go || esc {
                    self.overlay = Overlay::None;
                }
                if go { Effect::Run(action) } else { Effect::None }
            }
        }
    }

    /// Hotkeys for the bottom bar in the current context: (key, label).
    pub fn hotkeys(&self) -> Vec<(&'static str, &'static str)> {
        match (&self.overlay, self.focus) {
            (Overlay::Search, _) => vec![("Enter", "Keep"), ("Esc", "Clear"), ("↑↓", "Move")],
            (Overlay::Branch(_) | Overlay::Author(_), _) => vec![("Enter", "Apply"), ("↑↓", "Select"), ("Esc", "Cancel")],
            (Overlay::Date { .. }, _) => vec![("Enter", "Apply"), ("Tab", "Field"), ("Esc", "Cancel")],
            (Overlay::Name { .. }, _) => vec![("Enter", "Create"), ("Esc", "Cancel")],
            (Overlay::Confirm { need_y: true, .. }, _) => vec![("y", "Confirm"), ("Esc", "Cancel")],
            (Overlay::Confirm { .. }, _) => vec![("Enter", "Confirm"), ("Esc", "Cancel")],
            (Overlay::Actions | Overlay::ResetMode, _) => vec![("Esc", "Close")],
            (Overlay::None, Focus::Diff) => vec![("j/k", "Scroll"), ("PgUp/PgDn", "Page"), ("g/G", "Top/End"), ("Esc", "Back")],
            (Overlay::None, Focus::Sidebar) => {
                vec![("j/k", "Move"), ("Enter", "Diff"), ("Tab", "Commits"), ("Space", "Actions"), ("q", "Quit")]
            }
            (Overlay::None, Focus::List) => vec![
                ("j/k", "Move"),
                ("Tab", "Files"),
                ("/", "Search"),
                ("a", "All"),
                ("b", "Branch"),
                ("d", "Date"),
                ("u", "Author"),
                ("x", "Clear"),
                ("Space", "Actions"),
                ("q", "Quit"),
            ],
        }
    }

    /// Header text: HEAD, scope, and active filters.
    pub fn header(&self) -> String {
        let scope = match &self.filters.scope {
            Scope::Head => "current branch".to_string(),
            Scope::All => "all branches".to_string(),
            Scope::Branch(b) => format!("branch {}", sanitize(b)),
        };
        let mut parts = vec![sanitize(&self.head), scope];
        if let Some(s) = &self.filters.since {
            parts.push(format!("since {}", sanitize(s)));
        }
        if let Some(u) = &self.filters.until {
            parts.push(format!("until {}", sanitize(u)));
        }
        if let Some(a) = &self.filters.author {
            parts.push(format!("author {}", sanitize(a)));
        }
        if !self.search.is_empty() {
            parts.push(format!("/{}", sanitize(&self.search)));
        }
        let commits = self.rows.iter().filter(|r| r.commit.is_some()).count();
        let noun = if commits == 1 { "commit" } else { "commits" };
        parts.push(if self.loading { format!("{commits} {noun}, loading…") } else { format!("{commits} {noun}") });
        parts.join(" │ ")
    }
}

/// Escapes regex metacharacters so a listed author matches literally in `--author`.
fn regex_escape(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for c in s.chars() {
        if ".[]()*+?^$|{}\\".contains(c) {
            out.push('\\');
        }
        out.push(c);
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use gw_parse::parse_graph_line;

    fn key(c: KeyCode) -> KeyEvent {
        KeyEvent::new(c, KeyModifiers::NONE)
    }

    fn app(fixture: &str) -> App {
        let p = format!("{}/../gw-parse/tests/fixtures/{fixture}", env!("CARGO_MANIFEST_DIR"));
        let text = std::fs::read_to_string(p).unwrap();
        let mut a = App::new(Filters::new(vec![]), "main".into(), vec!["origin".into()], 1_793_751_908);
        a.resize(120, 12);
        a.push_rows(text.lines().map(parse_graph_line).collect());
        a.finish_load();
        a
    }

    #[test]
    fn navigation_skips_connectors() {
        let mut a = app("log-merge.txt");
        assert_eq!(a.selected_short(), Some("3c5ecf2"));
        a.key(key(KeyCode::Char('j')));
        assert_eq!(a.selected_short(), Some("708b13e"), "connector line `|\\` skipped");
        a.key(key(KeyCode::Char('G')));
        assert_eq!(a.selected_short(), Some("389c790"));
        a.key(key(KeyCode::Char('k')));
        assert_eq!(a.selected_short(), Some("a1bc23a"));
        a.key(key(KeyCode::Char('g')));
        assert_eq!(a.selected_short(), Some("3c5ecf2"));
    }

    #[test]
    fn page_moves_and_scrolls() {
        let mut a = app("log-all.txt");
        let h = a.body_height();
        a.key(key(KeyCode::PageDown));
        assert!(a.sel >= h - 1 && a.is_commit(a.sel), "sel={} h={h}", a.sel);
        assert!(a.scroll > 0);
        a.key(key(KeyCode::PageUp));
        a.key(key(KeyCode::PageUp));
        assert_eq!(a.sel, 0);
    }

    #[test]
    fn tab_switches_focus_and_sidebar_navigation() {
        let mut a = app("log-linear.txt");
        a.set_files("x".into(), vec![FileEntry { code: 'M', path: "a".into(), orig: None, stat: None }; 3]);
        a.key(key(KeyCode::Tab));
        assert_eq!(a.focus, Focus::Sidebar);
        a.key(key(KeyCode::Char('j')));
        a.key(key(KeyCode::Char('j')));
        a.key(key(KeyCode::Char('j')));
        assert_eq!(a.file_sel, 2, "clamped to last file");
        let e = a.key(key(KeyCode::Enter));
        assert_eq!(e, Effect::LoadDiff { sha: "x".into(), path: "a".into() });
        a.set_diff("x".into(), "a".into(), vec!["+x".into()]);
        assert_eq!(a.focus, Focus::Diff);
        a.key(key(KeyCode::Esc));
        assert_eq!((a.focus, a.file_sel), (Focus::Sidebar, 2), "back to the same file");
        a.key(key(KeyCode::Tab));
        assert_eq!(a.focus, Focus::List);
    }

    #[test]
    fn scope_toggle_changes_git_args() {
        let mut a = app("log-linear.txt");
        assert_eq!(a.filters.git_args(), vec!["HEAD"]);
        assert_eq!(a.key(key(KeyCode::Char('a'))), Effect::Reload);
        assert_eq!(a.filters.git_args(), vec!["--all"]);
        assert!(a.header().contains("all branches"));
        a.key(key(KeyCode::Char('a')));
        assert_eq!(a.filters.git_args(), vec!["HEAD"]);
        let f = Filters { since: Some("2 weeks ago".into()), author: Some("x".into()), ..Filters::new(vec!["--".into(), "src/".into()]) };
        assert_eq!(f.git_args(), vec!["HEAD", "--since=2 weeks ago", "--author=x", "--", "src/"]);
        assert_eq!(Filters::new(vec!["main..feat".into()]).git_args(), vec!["main..feat"], "user revision replaces HEAD");
    }

    #[test]
    fn search_filters_and_esc_clears() {
        let mut a = app("log-merge.txt");
        a.key(key(KeyCode::Char('/')));
        for c in "alice forms".chars() {
            a.key(key(KeyCode::Char(c)));
        }
        assert_eq!(a.view.len(), 2);
        assert!(a.view.iter().all(|&i| a.rows[i].commit.as_ref().unwrap().author == "Alice"));
        a.key(key(KeyCode::Enter));
        assert_eq!(a.overlay, Overlay::None);
        assert_eq!(a.view.len(), 2, "Enter keeps the filter");
        a.key(key(KeyCode::Esc));
        assert_eq!(a.view.len(), a.rows.len());
    }

    #[test]
    fn modal_flows() {
        let mut a = app("log-linear.txt");
        assert_eq!(a.key(key(KeyCode::Char('b'))), Effect::LoadBranches);
        a.open_branches(vec!["main".into(), "release/1.2".into(), "origin/main".into()]);
        for c in "rel".chars() {
            a.key(key(KeyCode::Char(c)));
        }
        assert_eq!(a.key(key(KeyCode::Enter)), Effect::Reload);
        assert_eq!(a.filters.scope, Scope::Branch("release/1.2".into()));

        a.open_authors(vec![(3, "Alice <a+b@x.io>".into())]);
        assert_eq!(a.key(key(KeyCode::Enter)), Effect::Reload);
        assert_eq!(a.filters.author.as_deref(), Some(r"Alice <a\+b@x\.io>"));
        a.open_authors(vec![(3, "Alice <a@x.io>".into())]);
        for c in "zed@ex".chars() {
            a.key(key(KeyCode::Char(c)));
        }
        a.key(key(KeyCode::Enter));
        assert_eq!(a.filters.author.as_deref(), Some("zed@ex"), "unlisted author used as typed");

        a.key(key(KeyCode::Char('d')));
        for c in "2 weeks ago".chars() {
            a.key(key(KeyCode::Char(c)));
        }
        assert_eq!(a.key(key(KeyCode::Enter)), Effect::ValidateDate { since: "2 weeks ago".into(), until: String::new() });
        a.date_error("date not understood".into());
        assert!(matches!(a.overlay, Overlay::Date { error: Some(_), .. }), "stays open on error");
        a.apply_date("2 weeks ago".into(), String::new());
        assert_eq!(a.filters.since.as_deref(), Some("2 weeks ago"));
        assert_eq!(a.key(key(KeyCode::Char('x'))), Effect::Reload);
        assert_eq!(a.filters, Filters::new(vec![]));
    }

    #[test]
    fn actions_and_confirmations() {
        let mut a = app("log-linear.txt");
        let sha = a.selected_hash().unwrap().to_string();
        a.key(key(KeyCode::Char(' ')));
        assert_eq!(a.overlay, Overlay::Actions);
        assert_eq!(a.key(key(KeyCode::Char('v'))), Effect::Run(Action::Revert(sha.clone())));

        a.key(key(KeyCode::Char(' ')));
        a.key(key(KeyCode::Char('r')));
        assert_eq!(a.key(key(KeyCode::Char('h'))), Effect::PrepareReset(ResetMode::Hard, sha.clone()));
        a.confirm_reset(ResetMode::Hard, sha.clone(), "main".into(), vec!["a.rs".into()]);
        assert_eq!(a.key(key(KeyCode::Enter)), Effect::None, "hard reset needs y");
        assert_eq!(a.key(key(KeyCode::Esc)), Effect::None);
        assert_eq!(a.overlay, Overlay::None, "Esc cancels");
        a.confirm_reset(ResetMode::Hard, sha.clone(), "main".into(), vec![]);
        assert_eq!(a.key(key(KeyCode::Char('y'))), Effect::Run(Action::Reset(ResetMode::Hard, sha.clone())));

        a.key(key(KeyCode::Char(' ')));
        a.key(key(KeyCode::Char('t')));
        for c in "v0.2.0".chars() {
            a.key(key(KeyCode::Char(c)));
        }
        let e = a.key(key(KeyCode::Enter));
        assert_eq!(e, Effect::Run(Action::Tag { name: "v0.2.0".into(), sha: sha.clone() }));
        a.action_result(false, "fatal: tag exists".into());
        assert!(matches!(a.overlay, Overlay::Name { error: Some(_), .. }), "error kept in modal");

        a.overlay = Overlay::None;
        a.key(key(KeyCode::Char(' ')));
        assert_eq!(a.key(key(KeyCode::Char('y'))), Effect::Copy(sha.clone()));
        assert_eq!(Action::Checkout(sha.clone()).git_args(), vec!["checkout", "--detach", sha.as_str()]);
        assert_eq!(Action::Tag { name: "-x".into(), sha: "s".into() }.git_args(), vec!["tag", "--", "-x", "s"]);
    }
}
