//! Renders the `gw status -i` [`App`] into a ratatui frame. Pure: no IO.

use ratatui::{
    Frame,
    layout::{Constraint, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Paragraph, Wrap},
};

use super::{App, Check, Focus, Item, NARROW, Overlay, Row};
use crate::browser::view::{DIM, SEL_BG, diff_line, draw_keys, draw_modal, fg, pane_block};
use crate::status::{Group, relative};
use crate::style::{sanitize, truncate};

pub(crate) fn group_color(g: Group) -> Color {
    match g {
        Group::Conflicts => Color::Red,
        Group::Staged => Color::Green,
        Group::Changed => Color::Yellow,
        Group::Submodules => Color::Cyan,
        Group::Untracked => Color::Indexed(244),
    }
}

/// Keeps the end of `s` (the file name) within `max` columns: `…ckout/money.ts`.
pub(crate) fn truncate_left(s: &str, max: usize) -> String {
    use unicode_width::{UnicodeWidthChar, UnicodeWidthStr};
    if s.width() <= max {
        return s.to_string();
    }
    let mut tail: Vec<char> = Vec::new();
    let mut used = 1; // the `…`
    for c in s.chars().rev() {
        let cw = c.width().unwrap_or(0);
        if used + cw > max {
            break;
        }
        used += cw;
        tail.push(c);
    }
    std::iter::once('…').chain(tail.into_iter().rev()).collect()
}

/// List pane width: 20% of the screen, at least 24 columns.
pub fn list_width(width: u16) -> u16 {
    (width / 5).max(24).min(width)
}

pub(crate) fn stat_text(stat: Option<Option<(u64, u64)>>) -> Vec<Span<'static>> {
    match stat {
        Some(Some((a, d))) => vec![Span::styled(format!("+{a}"), fg(Color::Green)), Span::raw(" "), Span::styled(format!("-{d}"), fg(Color::Red))],
        Some(None) => vec![Span::styled("bin", DIM)],
        None => vec![],
    }
}

fn display_path(app: &App, path: &str) -> String {
    sanitize(&relative(path, &app.prefix))
}

/// `old -> new` for renames, cwd-relative and sanitized.
pub(crate) fn item_name(it: &Item, prefix: &str) -> String {
    let rel = |p: &str| sanitize(&relative(p, prefix));
    match (&it.entry.orig, it.group.shows_orig() || it.check.is_some()) {
        (Some(o), true) => format!("{} -> {}", rel(o), rel(&it.entry.path)),
        _ => rel(&it.entry.path),
    }
}

pub(crate) fn sel_style(color: bool) -> Style {
    if color { Style::new().bg(SEL_BG).add_modifier(Modifier::BOLD) } else { Style::new().add_modifier(Modifier::REVERSED) }
}

/// One list row: optional checkbox, change code, path (left-truncated to `w`); padded when selected.
pub(crate) fn item_line(it: &Item, check: Option<Check>, prefix: &str, w: usize, selected: bool, color: bool) -> Line<'static> {
    let mut spans = Vec::new();
    if let Some(c) = check {
        let st = if c == Check::Off { DIM } else { fg(Color::Green) };
        spans.push(Span::styled(format!(" {}", c.mark()), st));
    }
    let lead = spans.iter().map(|s| s.width()).sum::<usize>() + 3;
    spans.push(Span::styled(format!(" {} ", it.code()), fg(group_color(it.group))));
    spans.push(Span::raw(truncate_left(&item_name(it, prefix), w.saturating_sub(lead))));
    let mut l = Line::from(spans);
    if selected {
        l = l.style(sel_style(color));
        // Pad so the highlight spans the pane.
        let pad = w.saturating_sub(l.width());
        l.spans.push(Span::raw(" ".repeat(pad)));
    }
    l
}

fn draw_list(f: &mut Frame, app: &App, area: Rect) {
    let block = pane_block(format!(" {} ", sanitize(&app.head)), app.focus == Focus::List);
    let inner = block.inner(area);
    f.render_widget(block, area);
    if app.items.is_empty() {
        f.render_widget(Paragraph::new(Line::styled("nothing to commit, working tree clean", DIM)).wrap(Wrap { trim: true }), inner);
        return;
    }
    let w = inner.width as usize;
    let lines: Vec<Line> = app
        .rows()
        .into_iter()
        .skip(app.scroll)
        .take(inner.height as usize)
        .map(|r| match r {
            Row::Header(g, n) => Line::styled(truncate(&format!("{} ({n})", g.title()), w), fg(group_color(g)).add_modifier(Modifier::BOLD)),
            Row::Item(i) => {
                let it = &app.items[i];
                item_line(it, it.check, &app.prefix, w, i == app.sel, app.color)
            }
        })
        .collect();
    f.render_widget(Paragraph::new(lines), inner);
}

fn draw_diff(f: &mut Frame, app: &App, area: Rect) {
    let Some(it) = app.selected() else {
        f.render_widget(pane_block(String::new(), false), area);
        return;
    };
    let mut title = vec![Span::raw(format!(" {} ", display_path(app, &it.entry.path)))];
    let stat = stat_text(it.stat);
    if !stat.is_empty() {
        title.extend(stat);
        title.push(Span::raw(" "));
    }
    let focused = app.focus == Focus::Diff;
    let block = pane_block(String::new(), focused).title(Line::from(title));
    let inner = block.inner(area);
    f.render_widget(block, area);
    let lines: Vec<Line> = app.diff.iter().skip(app.diff_scroll).take(inner.height as usize).map(|l| diff_line(l)).collect();
    f.render_widget(Paragraph::new(lines), inner);
}

fn confirm_lines(app: &App, files: &[String], total: usize) -> Vec<Line<'static>> {
    let Overlay::Confirm { item, .. } = &app.overlay else { return vec![] };
    let path = display_path(app, &item.entry.path);
    let warn = fg(Color::Red).add_modifier(Modifier::BOLD);
    let mut lines = vec![];
    match item.group {
        Group::Untracked => {
            lines.push(Line::raw(format!("Delete {total} untracked file{}:", if total == 1 { "" } else { "s" })));
            for p in files {
                lines.push(Line::raw(format!("  {}", display_path(app, p))));
            }
            if total > files.len() {
                lines.push(Line::styled(format!("  … and {} more", total - files.len()), DIM));
            }
        }
        Group::Staged if matches!(item.entry.x, 'A' | 'C') => lines.push(Line::raw(format!("Unstage and delete new file {path}"))),
        Group::Staged => lines.push(Line::raw(format!("Discard staged and unstaged changes in {path}"))),
        _ => lines.push(Line::raw(format!("Discard unstaged changes in {path}"))),
    }
    lines.push(Line::default());
    lines.push(Line::styled("This can't be undone.", warn));
    lines.push(Line::from(vec![Span::styled(" y ", Style::new().add_modifier(Modifier::REVERSED)), Span::raw(" revert   any other key: cancel")]));
    lines
}

pub fn draw(f: &mut Frame, app: &App) {
    let area = f.area();
    let [body, status, keys] = Layout::vertical([Constraint::Min(1), Constraint::Length(1), Constraint::Length(1)]).areas(area);
    if body.width < NARROW {
        if app.focus == Focus::Diff {
            draw_diff(f, app, body);
        } else {
            draw_list(f, app, body);
        }
    } else {
        let [list, diff] = Layout::horizontal([Constraint::Length(list_width(body.width)), Constraint::Min(1)]).areas(body);
        draw_list(f, app, list);
        draw_diff(f, app, diff);
    }
    if let Some((msg, err)) = &app.status {
        f.render_widget(Paragraph::new(Line::styled(sanitize(msg), fg(if *err { Color::Red } else { Color::Green }))), status);
    }
    draw_keys(f, &app.hotkeys(), keys);
    if let Overlay::Confirm { item, files, total } = &app.overlay {
        let title = format!("Revert {}?", truncate(&display_path(app, &item.entry.path), 50));
        draw_modal(f, &title, confirm_lines(app, files, *total), body, true);
    }
    if !app.color {
        // NO_COLOR: keep layout and modifiers (bold, reverse), drop every color.
        let buf = f.buffer_mut();
        for c in buf.content.iter_mut() {
            c.set_fg(Color::Reset).set_bg(Color::Reset);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::stage::{App, items};
    use crate::status::LineStats;
    use gw_parse::{Entry, Status};
    use ratatui::{Terminal, backend::TestBackend, buffer::Buffer};

    fn e(x: char, y: char, path: &str) -> Entry {
        Entry { x, y, unmerged: false, path: path.into(), orig: None, submodule: None }
    }

    fn app(entries: Vec<Entry>) -> App {
        let st = Status { entries, ..Default::default() };
        let mut stats = LineStats::default();
        stats.staged.insert("a.rs".into(), Some((3, 1)));
        let mut a = App::new(items(&st, &stats), String::new(), "main -> origin/main".into());
        a.set_diff(&a.items[0].clone(), vec!["diff --git a/a.rs b/a.rs".into(), "@@ -1 +1 @@".into(), "-old".into(), "+new".into()]);
        a
    }

    fn mixed() -> App {
        let mut r = e('R', '.', "new.rs");
        r.orig = Some("old.rs".into());
        app(vec![e('M', 'M', "a.rs"), r, e('.', 'D', "gone.rs"), e('?', '?', "n.txt")])
    }

    fn render(a: &mut App, w: u16, h: u16) -> Buffer {
        a.resize(w, h);
        let mut t = Terminal::new(TestBackend::new(w, h)).unwrap();
        t.draw(|f| draw(f, a)).unwrap();
        t.backend().buffer().clone()
    }

    fn text(b: &Buffer) -> String {
        let w = b.area.width as usize;
        let cells: Vec<&str> = b.content.iter().map(|c| c.symbol()).collect();
        cells.chunks(w).map(|r| r.concat().trim_end().to_string()).collect::<Vec<_>>().join("\n")
    }

    #[test]
    fn wide_layout() {
        let mut a = mixed();
        let b = render(&mut a, 150, 14);
        assert_eq!(list_width(150), 30);
        let t = text(&b);
        let first_diff_line = t.lines().nth(1).unwrap();
        assert!(first_diff_line.starts_with("│Staged (2)"), "{first_diff_line}");
        assert_eq!(first_diff_line.chars().nth(30), Some('│'), "diff pane starts at column 30: {first_diff_line}");
        insta::assert_snapshot!(t);
    }

    #[test]
    fn narrow_shows_one_pane() {
        let mut a = mixed();
        let t = text(&render(&mut a, 60, 10));
        assert!(t.contains("Staged (2)") && !t.contains("+new"), "{t}");
        a.key(ratatui::crossterm::event::KeyEvent::from(ratatui::crossterm::event::KeyCode::Tab));
        let t = text(&render(&mut a, 60, 10));
        assert!(!t.contains("Staged (2)") && t.contains("+new"), "{t}");
    }

    #[test]
    fn colors_and_selection() {
        let mut a = mixed();
        let b = render(&mut a, 150, 14);
        // Row 1: "Staged" header in green; row 2: selected a.rs on the selection background.
        assert_eq!(b[(1, 1)].fg, Color::Green);
        assert_eq!(b[(10, 2)].bg, SEL_BG);
        // Diff: "+new" green, "-old" red.
        let row = |y: u16| (0..150).map(|x| b[(x, y)].symbol()).collect::<String>();
        let y_new = (0..14).find(|&y| row(y).contains("+new")).unwrap();
        let x_new = row(y_new).chars().position(|c| c == '+').unwrap() as u16;
        assert_eq!(b[(x_new, y_new)].fg, Color::Green);
    }

    #[test]
    fn no_color() {
        let mut a = mixed();
        a.color = false;
        let b = render(&mut a, 150, 14);
        assert!(b.content.iter().all(|c| c.fg == Color::Reset && c.bg == Color::Reset));
        assert!(b[(10, 2)].modifier.contains(Modifier::REVERSED), "selection in reverse video");
    }

    #[test]
    fn escape_in_name_is_inert() {
        let mut a = app(vec![e('?', '?', "evil\x1b[2J.txt")]);
        let t = text(&render(&mut a, 150, 8));
        assert!(t.contains("evil\\x1b[2J.txt"), "{t}");
        assert!(!t.contains('\x1b'));
    }

    #[test]
    fn long_paths_keep_file_name() {
        assert_eq!(truncate_left("src/checkout/money.ts", 12), "…ut/money.ts");
        assert_eq!(truncate_left("a.rs", 12), "a.rs");
        let mut a = app(vec![e('?', '?', "src/checkout/receipt.ts")]);
        let t = text(&render(&mut a, 100, 6));
        assert!(t.contains("…") && t.contains("/receipt.ts"), "{t}");
    }

    #[test]
    fn clean_tree() {
        let mut a = App::new(vec![], String::new(), "main".into());
        let t = text(&render(&mut a, 100, 6));
        assert!(t.contains("nothing to commit,") && t.contains("working tree clean"), "{t}");
    }

    #[test]
    fn add_checklist() {
        let st = Status { entries: vec![e('M', '.', "a.rs"), e('M', 'M', "b.rs"), e('.', 'M', "c.rs"), e('?', '?', "n.txt")], ..Default::default() };
        let mut a = App::new(crate::stage::add_items(&st, &LineStats::default()), String::new(), "main".into());
        a.mode = crate::stage::Mode::Add;
        let t = text(&render(&mut a, 120, 8));
        assert!(!t.contains("Staged ("), "no headers: {t}");
        insta::assert_snapshot!(t);
    }

    #[test]
    fn revert_modal() {
        let mut a = app(vec![e('?', '?', "tmp/")]);
        let it = a.items[0].clone();
        let files: Vec<String> = (0..10).map(|i| format!("tmp/f{i}")).collect();
        a.confirm_revert(it, files, 25);
        let t = text(&render(&mut a, 120, 24));
        assert!(t.contains("Revert tmp/?") && t.contains("Delete 25 untracked files:") && t.contains("… and 15 more"), "{t}");
        insta::assert_snapshot!(t);
    }
}
