//! Renders the `gw branch -i` [`App`] into a ratatui frame. Pure: no IO.

use ratatui::{
    Frame,
    layout::{Constraint, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::Paragraph,
};

use super::{App, Focus, Prompt, Row};
use crate::browser::view::{DIM, FOCUS, draw_keys, draw_modal, fg, pane_block};
use crate::stage::view::{item_line, sel_style, stat_text};
use crate::stash::view::{draw_diff_pane, strip_colors, three_panes};
use crate::style::{sanitize, truncate};

fn branch_line(app: &App, i: usize, w: usize) -> Line<'static> {
    let b = &app.branches[i];
    let name = Style::new().fg(if b.remote { Color::Red } else { Color::Cyan });
    let mut spans = vec![
        Span::styled(if b.current { " * " } else { "   " }, fg(Color::Green)),
        Span::styled(sanitize(&b.name), if b.current { name.add_modifier(Modifier::BOLD) } else { name }),
    ];
    if b.gone {
        spans.push(Span::styled(" gone", fg(Color::Red)));
    } else if !b.remote {
        if b.ahead > 0 {
            spans.push(Span::styled(format!(" ↑{}", b.ahead), fg(Color::Green)));
        }
        if b.behind > 0 {
            spans.push(Span::styled(format!(" ↓{}", b.behind), fg(Color::Red)));
        }
    }
    spans.push(Span::styled(format!("  {}", sanitize(&b.age)), DIM));
    let mut l = Line::from(spans);
    if i == app.sel {
        l = l.style(sel_style(app.color));
        let pad = w.saturating_sub(l.width());
        l.spans.push(Span::raw(" ".repeat(pad)));
    }
    l
}

fn draw_branches(f: &mut Frame, app: &App, area: Rect) {
    let block = pane_block(" Branches ".into(), app.focus == Focus::Branches);
    let inner = block.inner(area);
    f.render_widget(block, area);
    if app.branches.is_empty() {
        f.render_widget(Paragraph::new(Line::styled("no branches yet", DIM)), inner);
        return;
    }
    let w = inner.width as usize;
    let lines: Vec<Line> = app
        .rows()
        .into_iter()
        .skip(app.scroll)
        .take(inner.height as usize)
        .map(|r| match r {
            Row::Header(h) => Line::styled(h, Style::new().add_modifier(Modifier::BOLD)),
            Row::Branch(i) => branch_line(app, i, w),
        })
        .collect();
    f.render_widget(Paragraph::new(lines), inner);
}

fn draw_files(f: &mut Frame, app: &App, area: Rect) {
    let b = app.selected();
    let current = sanitize(&app.current);
    let title = match b {
        Some(b) if b.current => " working tree ".into(),
        Some(b) => format!(" {} vs {current} ", sanitize(&b.name)),
        None => String::new(),
    };
    let block = pane_block(title, app.focus == Focus::Files);
    let inner = block.inner(area);
    f.render_widget(block, area);
    if !app.files_ready() {
        return;
    }
    if app.files.is_empty() {
        let msg = if b.is_some_and(|b| b.current) { "working tree clean".into() } else { format!("nothing new against {current}") };
        f.render_widget(Paragraph::new(Line::styled(msg, DIM)), inner);
        return;
    }
    let w = inner.width as usize;
    let lines: Vec<Line> = app
        .files
        .iter()
        .enumerate()
        .skip(app.file_scroll)
        .take(inner.height as usize)
        .map(|(i, it)| {
            // Counts right-aligned after the name.
            let stat = stat_text(it.stat);
            let sw: usize = stat.iter().map(|s| s.width()).sum();
            let mut l = item_line(it, None, &app.prefix, w.saturating_sub(sw + 1), false, app.color);
            if !stat.is_empty() {
                l.spans.push(Span::raw(" ".repeat(w.saturating_sub(l.width() + sw))));
                l.spans.extend(stat);
            }
            if i == app.file_sel {
                l = l.style(sel_style(app.color));
                l.spans.push(Span::raw(" ".repeat(w.saturating_sub(l.width()))));
            }
            l
        })
        .collect();
    f.render_widget(Paragraph::new(lines), inner);
}

fn prompt_lines(app: &App) -> Option<(String, Vec<Line<'static>>, bool)> {
    let b = app.target.as_ref()?;
    let name = sanitize(&b.name);
    let input = || Line::from(vec![Span::styled(sanitize(&app.input), Style::new().add_modifier(Modifier::BOLD)), Span::styled("▌", fg(FOCUS))]);
    let yes = |what: &str| Line::from(vec![Span::styled(" y ", Style::new().add_modifier(Modifier::REVERSED)), Span::raw(format!(" {what}   any other key: cancel"))]);
    Some(match app.prompt? {
        Prompt::Create => ("New branch".into(), vec![Line::raw(format!("Name of the new branch from {name}:")), Line::default(), input()], false),
        Prompt::Rename => (format!("Rename {name}"), vec![Line::raw("New name:"), Line::default(), input()], false),
        Prompt::Delete => (format!("Delete branch {name}?"), vec![Line::raw(truncate(&sanitize(&b.subject), 60)), Line::default(), yes("delete")], true),
        Prompt::Force => (
            "Force delete?".into(),
            vec![Line::raw(format!("{name} is not fully merged.")), Line::default(), Line::styled("Its commits may be lost.", fg(Color::Red).add_modifier(Modifier::BOLD)), yes("force delete")],
            true,
        ),
        Prompt::DeleteRemote => (
            format!("Delete {name} on the server?"),
            vec![Line::raw(format!("Runs git push {} --delete {}", sanitize(remote_of(&b.name).0), sanitize(remote_of(&b.name).1))), Line::default(), yes("delete")],
            true,
        ),
        Prompt::Merge => (format!("Merge {name} into {}?", sanitize(&app.current)), vec![Line::raw(truncate(&sanitize(&b.subject), 60)), Line::default(), yes("merge")], false),
    })
}

/// `origin/fix/x` -> (`origin`, `fix/x`).
pub fn remote_of(name: &str) -> (&str, &str) {
    name.split_once('/').unwrap_or(("", name))
}

pub fn draw(f: &mut Frame, app: &App) {
    let area = f.area();
    let [body, status, keys] = Layout::vertical([Constraint::Min(1), Constraint::Length(1), Constraint::Length(1)]).areas(area);
    let focused = [Focus::Branches, Focus::Files, Focus::Diff].iter().position(|x| *x == app.focus).unwrap_or(0);
    let [a, b, c] = three_panes(body, focused);
    a.into_iter().for_each(|r| draw_branches(f, app, r));
    b.into_iter().for_each(|r| draw_files(f, app, r));
    c.into_iter().for_each(|r| draw_diff_pane(f, app.diff_item(), &app.diff, app.diff_scroll, &app.prefix, app.focus == Focus::Diff, r));
    if let Some((msg, err)) = &app.status {
        f.render_widget(Paragraph::new(Line::styled(sanitize(msg), fg(if *err { Color::Red } else { Color::Green }))), status);
    } else if let Some(b) = app.selected() {
        let mut spans = vec![Span::styled(format!("{} ", sanitize(&b.name)), fg(Color::Cyan))];
        if let Some(u) = &b.upstream {
            spans.push(Span::raw("-> "));
            spans.push(Span::styled(format!("{} ", sanitize(u)), fg(Color::Magenta)));
        }
        spans.push(Span::styled(format!("{}: ", sanitize(&b.age)), DIM));
        spans.push(Span::raw(sanitize(&b.subject)));
        f.render_widget(Paragraph::new(Line::from(spans)), status);
    }
    draw_keys(f, &app.hotkeys(), keys);
    if let Some((title, lines, danger)) = prompt_lines(app) {
        draw_modal(f, &title, lines, body, danger);
    }
    if !app.color {
        strip_colors(f);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::branch::tests::br;
    use crate::stage::Item;
    use crate::status::Group;
    use gw_parse::Entry;
    use ratatui::{
        Terminal,
        backend::TestBackend,
        buffer::Buffer,
        crossterm::event::{KeyCode, KeyEvent},
    };

    fn item(x: char, path: &str, stat: Option<(u64, u64)>) -> Item {
        Item { group: Group::Staged, entry: Entry { x, y: '.', unmerged: false, path: path.into(), orig: None, submodule: None }, stat: Some(stat), check: None }
    }

    fn app() -> App {
        let mut main = br("main", true, false);
        main.upstream = Some("origin/main".into());
        main.ahead = 2;
        let mut feat = br("feat", false, false);
        feat.gone = true;
        let mut a = App::new(vec![main, feat, br("origin/main", false, true), br("origin/fix", false, true)], String::new());
        a.key(KeyEvent::from(KeyCode::Down));
        let b = a.files_wanted().unwrap();
        a.set_files(&b, vec![item('M', "src/a.rs", Some((10, 1))), item('A', "new.rs", Some((3, 0)))]);
        let (b, it) = a.diff_wanted().unwrap();
        a.set_diff(&b, &it, vec!["@@ -1 +1 @@".into(), "-old".into(), "+new".into()]);
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
        let t = text(&render(&mut a, 150, 12));
        assert!(t.contains("feat vs main") && t.contains("M src/a.rs") && t.contains("+10 -1") && t.contains("@@ -1 +1 @@"), "{t}");
        assert!(t.contains("* main ↑2") && t.contains("feat gone") && t.contains("Remote"), "{t}");
        insta::assert_snapshot!(t);
    }

    #[test]
    fn narrow_one_pane() {
        let mut a = app();
        a.key(KeyEvent::from(KeyCode::Tab));
        let t = text(&render(&mut a, 60, 10));
        let panes: String = t.lines().take(8).collect();
        assert!(panes.contains("src/a.rs") && !panes.contains("Remote"), "{t}");
        insta::assert_snapshot!(t);
    }

    #[test]
    fn nothing_new_and_working_tree() {
        let mut a = app();
        a.key(KeyEvent::from(KeyCode::Down));
        let b = a.files_wanted().unwrap();
        a.set_files(&b, vec![]);
        assert!(text(&render(&mut a, 150, 10)).contains("nothing new against main"));
        a.key(KeyEvent::from(KeyCode::Home));
        let b = a.files_wanted().unwrap();
        a.set_files(&b, vec![]);
        let t = text(&render(&mut a, 150, 10));
        assert!(t.contains("working tree") && t.contains("working tree clean"), "{t}");
    }

    #[test]
    fn prompts() {
        let mut a = app();
        a.key(KeyEvent::from(KeyCode::Char('d')));
        let t = text(&render(&mut a, 120, 14));
        assert!(t.contains("Delete branch feat?"), "{t}");
        insta::assert_snapshot!("delete", t);
        a.key(KeyEvent::from(KeyCode::Char('n')));
        a.ask_force(a.selected().unwrap().clone());
        let t = text(&render(&mut a, 120, 14));
        assert!(t.contains("feat is not fully merged."), "{t}");
        insta::assert_snapshot!("force", t);
        a.key(KeyEvent::from(KeyCode::Esc));
        a.key(KeyEvent::from(KeyCode::Char('R')));
        a.key(KeyEvent::from(KeyCode::Char('2')));
        let t = text(&render(&mut a, 120, 14));
        assert!(t.contains("Rename feat") && t.contains("feat2▌"), "{t}");
        insta::assert_snapshot!("rename", t);
        a.key(KeyEvent::from(KeyCode::Esc));
        a.key(KeyEvent::from(KeyCode::Down));
        a.key(KeyEvent::from(KeyCode::Char('d')));
        let t = text(&render(&mut a, 120, 14));
        assert!(t.contains("Delete origin/main on the server?") && t.contains("git push origin --delete main"), "{t}");
    }

    #[test]
    fn escape_in_subject_and_no_color() {
        let mut b = br("main", true, false);
        b.subject = "evil\x1b[2J".into();
        let mut a = App::new(vec![b], String::new());
        a.color = false;
        let buf = render(&mut a, 120, 6);
        let t = text(&buf);
        assert!(t.contains("evil\\x1b") && !t.contains('\x1b'), "{t}");
        assert!(buf.content.iter().all(|c| c.fg == Color::Reset && c.bg == Color::Reset));
        assert!(buf[(3, 2)].modifier.contains(Modifier::REVERSED), "selection in reverse video");
    }
}
