//! Renders the `gw stash -i` [`App`] into a ratatui frame. Pure: no IO.

use ratatui::{
    Frame,
    layout::{Constraint, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::Paragraph,
};

use super::{App, Focus, Screen};
use crate::browser::view::{DIM, FOCUS, diff_line, draw_keys, draw_modal, fg, pane_block};
use crate::stage::{
    Check, NARROW,
    view::{item_line, list_width, sel_style, stat_text},
};
use crate::status::relative;
use crate::style::{sanitize, truncate};

fn stash_line(app: &App, i: usize, w: usize) -> Line<'static> {
    let s = &app.stashes[i];
    // Message first: the list pane is narrow and the paragraph clips the rest.
    let mut l = Line::from(vec![
        Span::styled(format!(" {} ", s.index), fg(Color::Yellow)),
        Span::raw(sanitize(&s.message)),
        Span::styled(s.branch.as_deref().map(|b| format!("  {}", sanitize(b))).unwrap_or_default(), fg(Color::Cyan)),
        Span::styled(format!("  {}", sanitize(&s.age)), DIM),
    ]);
    if i == app.sel {
        l = l.style(sel_style(app.color));
        let pad = w.saturating_sub(l.width());
        l.spans.push(Span::raw(" ".repeat(pad)));
    }
    l
}

fn draw_stashes(f: &mut Frame, app: &App, area: Rect) {
    let block = pane_block(" Stashes ".into(), app.focus == Focus::Stashes);
    let inner = block.inner(area);
    f.render_widget(block, area);
    if app.stashes.is_empty() {
        f.render_widget(Paragraph::new(Line::styled("no stashes", DIM)), inner);
        return;
    }
    let w = inner.width as usize;
    let lines: Vec<Line> = (app.scroll..app.stashes.len()).take(inner.height as usize).map(|i| stash_line(app, i, w)).collect();
    f.render_widget(Paragraph::new(lines), inner);
}

fn draw_files(f: &mut Frame, app: &App, area: Rect) {
    let title = app.selected().map(|s| format!(" {} ", s.reference)).unwrap_or_default();
    let block = pane_block(title, app.focus == Focus::Files);
    let inner = block.inner(area);
    f.render_widget(block, area);
    let w = inner.width as usize;
    let lines: Vec<Line> = app
        .files
        .iter()
        .enumerate()
        .skip(app.file_scroll)
        .take(inner.height as usize)
        .map(|(i, it)| item_line(it, None, &app.prefix, w, i == app.file_sel && app.selected().is_some(), app.color))
        .collect();
    f.render_widget(Paragraph::new(lines), inner);
}

fn draw_pick(f: &mut Frame, app: &App, area: Rect) {
    let n = app.pick.iter().filter(|r| r.1).count();
    let block = pane_block(format!(" New stash: {n}/{} ", app.pick.len()), app.focus != Focus::Diff);
    let inner = block.inner(area);
    f.render_widget(block, area);
    let w = inner.width as usize;
    let lines: Vec<Line> = app
        .pick
        .iter()
        .enumerate()
        .skip(app.pick_scroll)
        .take(inner.height as usize)
        .map(|(i, (it, on))| item_line(it, Some(if *on { Check::On } else { Check::Off }), &app.prefix, w, i == app.pick_sel, app.color))
        .collect();
    f.render_widget(Paragraph::new(lines), inner);
}

fn draw_diff(f: &mut Frame, app: &App, area: Rect) {
    let Some(it) = app.diff_item() else {
        f.render_widget(pane_block(String::new(), app.focus == Focus::Diff), area);
        return;
    };
    let mut title = vec![Span::raw(format!(" {} ", sanitize(&relative(&it.entry.path, &app.prefix))))];
    let stat = stat_text(it.stat);
    if !stat.is_empty() {
        title.extend(stat);
        title.push(Span::raw(" "));
    }
    let block = pane_block(String::new(), app.focus == Focus::Diff).title(Line::from(title));
    let inner = block.inner(area);
    f.render_widget(block, area);
    let lines: Vec<Line> = app.diff.iter().skip(app.diff_scroll).take(inner.height as usize).map(|l| diff_line(l)).collect();
    f.render_widget(Paragraph::new(lines), inner);
}

pub fn draw(f: &mut Frame, app: &App) {
    let area = f.area();
    let [body, status, keys] = Layout::vertical([Constraint::Min(1), Constraint::Length(1), Constraint::Length(1)]).areas(area);
    let narrow = body.width < NARROW;
    if app.screen == Screen::Browse {
        if narrow {
            match app.focus {
                Focus::Stashes => draw_stashes(f, app, body),
                Focus::Files => draw_files(f, app, body),
                Focus::Diff => draw_diff(f, app, body),
            }
        } else {
            let lw = list_width(body.width);
            let [a, b, c] = Layout::horizontal([Constraint::Length(lw), Constraint::Length(lw), Constraint::Min(1)]).areas(body);
            draw_stashes(f, app, a);
            draw_files(f, app, b);
            draw_diff(f, app, c);
        }
    } else if narrow {
        if app.focus == Focus::Diff { draw_diff(f, app, body) } else { draw_pick(f, app, body) }
    } else {
        let [a, b] = Layout::horizontal([Constraint::Length(list_width(body.width)), Constraint::Min(1)]).areas(body);
        draw_pick(f, app, a);
        draw_diff(f, app, b);
    }
    if let Some((msg, err)) = &app.status {
        f.render_widget(Paragraph::new(Line::styled(sanitize(msg), fg(if *err { Color::Red } else { Color::Green }))), status);
    } else if let (Screen::Browse, Some(s)) = (app.screen, app.selected()) {
        // The whole stash on one line: the list pane is narrow and clips branch and age.
        let mut spans = vec![Span::styled(format!("{} ", s.reference), fg(Color::Yellow))];
        if let Some(b) = &s.branch {
            spans.push(Span::raw("on "));
            spans.push(Span::styled(format!("{} ", sanitize(b)), fg(Color::Cyan)));
        }
        spans.push(Span::styled(format!("{}: ", sanitize(&s.age)), DIM));
        spans.push(Span::raw(sanitize(&s.message)));
        f.render_widget(Paragraph::new(Line::from(spans)), status);
    }
    draw_keys(f, &app.hotkeys(), keys);
    if app.screen == Screen::Message {
        let n = app.pick.iter().filter(|r| r.1).count();
        let lines = vec![
            Line::raw(format!("Stash {n} file{}. Message (empty: git's default):", if n == 1 { "" } else { "s" })),
            Line::default(),
            Line::from(vec![Span::styled(sanitize(&app.input), Style::new().add_modifier(Modifier::BOLD)), Span::styled("▌", fg(FOCUS))]),
        ];
        draw_modal(f, "New stash", lines, body, false);
    }
    if let Some(s) = app.confirm_drop.and_then(|i| app.stashes.get(i)) {
        let lines = vec![
            Line::raw(format!("Delete stash {}: {}", s.index, truncate(&sanitize(&s.message), 60))),
            Line::default(),
            Line::styled("This can't be undone.", fg(Color::Red).add_modifier(Modifier::BOLD)),
            Line::from(vec![Span::styled(" y ", Style::new().add_modifier(Modifier::REVERSED)), Span::raw(" drop   any other key: cancel")]),
        ];
        draw_modal(f, &format!("Drop {}?", s.reference), lines, body, true);
    }
    if !app.color {
        // NO_COLOR: keep layout and modifiers (bold, reverse), drop every color.
        for c in f.buffer_mut().content.iter_mut() {
            c.set_fg(Color::Reset).set_bg(Color::Reset);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::stage::Item;
    use crate::status::Group;
    use gw_parse::{Entry, Stash};
    use ratatui::{
        Terminal,
        backend::TestBackend,
        buffer::Buffer,
        crossterm::event::{KeyCode, KeyEvent},
    };

    fn stash(i: usize, msg: &str) -> Stash {
        Stash { reference: format!("stash@{{{i}}}"), index: i, oid: format!("oid{i}"), age: "2 hours ago".into(), branch: Some("main".into()), message: msg.into() }
    }

    fn item(group: Group, path: &str) -> Item {
        let (x, y) = match group {
            Group::Untracked => ('?', '?'),
            Group::Changed => ('.', 'M'),
            _ => ('M', '.'),
        };
        Item { group, entry: Entry { x, y, unmerged: false, path: path.into(), orig: None, submodule: None }, stat: Some(Some((2, 1))), check: None }
    }

    fn app() -> App {
        let mut a = App::new(vec![stash(0, "wip parser"), stash(1, "try forms")], String::new());
        a.set_files("oid0", vec![item(Group::Staged, "a.rs"), item(Group::Untracked, "n.txt")]);
        let src = a.diff_wanted().unwrap();
        a.set_diff(&src, vec!["@@ -1 +1 @@".into(), "-old".into(), "+new".into()]);
        a
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
    fn wide_three_panes() {
        let mut a = app();
        let t = text(&render(&mut a, 150, 10));
        let row = t.lines().nth(1).unwrap();
        assert!(row.contains("0 wip parser  main") && row.contains("M a.rs") && row.contains("@@ -1 +1 @@"), "{row}");
        insta::assert_snapshot!(t);
    }

    #[test]
    fn narrow_one_pane() {
        let mut a = app();
        let t = text(&render(&mut a, 60, 8));
        assert!(t.contains("wip parser") && !t.contains("a.rs"), "{t}");
        a.key(KeyEvent::from(KeyCode::Tab));
        let t = text(&render(&mut a, 60, 8));
        let panes: String = t.lines().take(6).collect();
        assert!(!panes.contains("wip parser") && panes.contains("a.rs"), "{t}");
        assert!(t.contains("stash@{0} on main 2 hours ago: wip parser"), "status line names the stash: {t}");
    }

    #[test]
    fn picker_and_prompt() {
        let mut a = app();
        a.open_picker(vec![item(Group::Changed, "a.rs"), item(Group::Untracked, "n.txt")]);
        a.key(KeyEvent::from(KeyCode::Down));
        a.key(KeyEvent::from(KeyCode::Char(' ')));
        let t = text(&render(&mut a, 120, 10));
        assert!(t.contains("New stash: 1/2") && t.contains("[x] M a.rs") && t.contains("[ ] ? n.txt"), "{t}");
        insta::assert_snapshot!("picker", t);
        a.key(KeyEvent::from(KeyCode::Enter));
        for c in "wip".chars() {
            a.key(KeyEvent::from(KeyCode::Char(c)));
        }
        let t = text(&render(&mut a, 120, 12));
        assert!(t.contains("Stash 1 file.") && t.contains("wip▌"), "{t}");
        insta::assert_snapshot!("prompt", t);
    }

    #[test]
    fn drop_modal() {
        let mut a = app();
        a.key(KeyEvent::from(KeyCode::Char('d')));
        let t = text(&render(&mut a, 120, 12));
        assert!(t.contains("Drop stash@{0}?") && t.contains("Delete stash 0: wip parser"), "{t}");
        insta::assert_snapshot!(t);
    }

    #[test]
    fn escape_in_message_and_no_color() {
        let mut a = App::new(vec![stash(0, "evil\x1b[2J")], String::new());
        a.color = false;
        let b = render(&mut a, 120, 6);
        let t = text(&b);
        assert!(t.contains("evil\\x1b") && !t.contains('\x1b'), "{t}");
        assert!(b.content.iter().all(|c| c.fg == Color::Reset && c.bg == Color::Reset));
        assert!(b[(3, 1)].modifier.contains(Modifier::REVERSED), "selection in reverse video");
    }
}
