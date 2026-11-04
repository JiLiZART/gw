//! Renders [`App`] into a ratatui frame. Pure: no IO.

use ratatui::{
    Frame,
    layout::{Constraint, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Clear, Paragraph},
};
use unicode_width::UnicodeWidthStr;

use super::{App, Focus, NameKind, Overlay, Picker};
use crate::style::{sanitize, truncate};

const LANES: [Color; 6] = [
    Color::Indexed(33),
    Color::Indexed(70),
    Color::Indexed(172),
    Color::Indexed(135),
    Color::Indexed(37),
    Color::Indexed(167),
];
const SEL_BG: Color = Color::Indexed(237);
const SEL_BG_DIM: Color = Color::Indexed(235);
const FOCUS: Color = Color::Indexed(75);
const DIM: Style = Style::new().fg(Color::Indexed(244));

fn fg(c: Color) -> Style {
    Style::new().fg(c)
}

/// `2h`, `3d`, `5mo`, `2y` from a unix time.
pub fn age(now: i64, t: i64) -> String {
    let s = (now - t).max(0);
    match s {
        0..60 => "now".into(),
        60..3600 => format!("{}m", s / 60),
        3600..86_400 => format!("{}h", s / 3600),
        86_400..2_592_000 => format!("{}d", s / 86_400),
        2_592_000..31_536_000 => format!("{}mo", s / 2_592_000),
        _ => format!("{}y", s / 31_536_000),
    }
}

fn graph_spans(graph: &str) -> Vec<Span<'static>> {
    graph
        .chars()
        .enumerate()
        .map(|(i, c)| {
            let (sym, bold) = match c {
                '*' => ('●', true),
                '|' => ('│', false),
                '/' => ('╱', false),
                '\\' => ('╲', false),
                '-' => ('─', false),
                '.' => ('╮', false),
                c => (c, false),
            };
            let mut st = fg(LANES[(i / 2) % LANES.len()]);
            if bold {
                st = st.add_modifier(Modifier::BOLD);
            }
            Span::styled(sym.to_string(), st)
        })
        .collect()
}

/// Text with search-matched char positions highlighted.
fn marked(text: &str, marks: &[usize], base: Style) -> Vec<Span<'static>> {
    if marks.is_empty() {
        return vec![Span::styled(text.to_string(), base)];
    }
    let hl = base.fg(Color::Indexed(220)).add_modifier(Modifier::BOLD | Modifier::UNDERLINED);
    text.chars().enumerate().map(|(i, c)| Span::styled(c.to_string(), if marks.binary_search(&i).is_ok() { hl } else { base })).collect()
}

fn ref_chip(r: &str, remotes: &[String]) -> Span<'static> {
    let r = sanitize(r);
    let (text, style) = if let Some(b) = r.strip_prefix("HEAD -> ") {
        (format!("@{b}"), fg(Color::Indexed(77)).add_modifier(Modifier::BOLD))
    } else if r == "HEAD" {
        ("HEAD".into(), fg(Color::Indexed(77)).add_modifier(Modifier::BOLD))
    } else if let Some(t) = r.strip_prefix("tag: ") {
        (format!("#{t}"), fg(Color::Indexed(178)))
    } else if remotes.iter().any(|rm| r.starts_with(&format!("{rm}/"))) {
        (r.clone(), fg(Color::Indexed(167)))
    } else {
        (r.clone(), fg(Color::Indexed(77)))
    };
    Span::styled(format!("({text})"), style)
}

fn commit_line(app: &App, pos: usize, width: usize) -> Line<'static> {
    let row = &app.rows[app.view[pos]];
    let marks = app.marks.get(pos).cloned().unwrap_or_default();
    let field = |i: usize| marks.get(i).cloned().unwrap_or_default();
    // While searching, the graph has no meaning (rows are reordered): drop it.
    let mut spans = if app.search.is_empty() { graph_spans(&row.graph) } else { Vec::new() };
    if let Some(c) = &row.commit {
        if !spans.is_empty() {
            spans.push(Span::raw(" "));
        }
        let short_marks: Vec<usize> = field(0).into_iter().filter(|&i| i < c.short.len()).collect();
        spans.extend(marked(&c.short, &short_marks, fg(Color::Indexed(178))));
        spans.push(Span::styled(format!(" {:>4} ", age(app.now, c.time)), DIM));
        let author = truncate(&sanitize(&c.author), 14);
        let pad = 14usize.saturating_sub(author.width());
        spans.extend(marked(&author, &field(1), fg(Color::Indexed(110))));
        spans.push(Span::raw(" ".repeat(pad + 1)));
        for r in &c.refs {
            spans.push(ref_chip(r, &app.remotes));
            spans.push(Span::raw(" "));
        }
        spans.extend(marked(&sanitize(&c.subject), &field(2), Style::new()));
    }
    let _ = width;
    Line::from(spans)
}

fn pane_block(title: String, focused: bool) -> Block<'static> {
    let st = if focused { fg(FOCUS).add_modifier(Modifier::BOLD) } else { DIM };
    Block::new().borders(Borders::ALL).border_style(st).title(Span::styled(title, st))
}

fn draw_list(f: &mut Frame, app: &App, area: Rect) {
    let focused = app.focus == Focus::List;
    let block = pane_block(" Commits ".into(), focused);
    let inner = block.inner(area);
    f.render_widget(block, area);
    let h = inner.height as usize;
    let lines: Vec<Line> = (app.scroll..(app.scroll + h).min(app.view.len()))
        .map(|pos| {
            let mut l = commit_line(app, pos, inner.width as usize);
            if pos == app.sel {
                l = l.style(Style::new().bg(if focused { SEL_BG } else { SEL_BG_DIM }));
            }
            l
        })
        .collect();
    let empty = app.view.is_empty();
    let mut p = Paragraph::new(lines);
    if empty {
        p = Paragraph::new(Line::styled(if app.loading { "loading…" } else { "no commits match" }, DIM));
    }
    f.render_widget(p, inner);
}

fn stat_spans(stat: Option<Option<(u64, u64)>>) -> Vec<Span<'static>> {
    match stat {
        Some(Some((a, d))) => vec![
            Span::styled(format!("+{a}"), if a > 0 { fg(Color::Green) } else { DIM }),
            Span::raw(" "),
            Span::styled(format!("-{d}"), if d > 0 { fg(Color::Red) } else { DIM }),
        ],
        Some(None) => vec![Span::styled("bin", DIM)],
        None => vec![],
    }
}

fn draw_sidebar(f: &mut Frame, app: &App, area: Rect) {
    let focused = app.focus == Focus::Sidebar;
    let files = app.files.as_ref().filter(|(s, _)| Some(s.as_str()) == app.selected_hash()).map(|f| &f.1);
    let title = match files {
        Some(fl) => format!(" Files ({}) ", fl.len()),
        None => " Files ".into(),
    };
    let block = pane_block(title, focused);
    let inner = block.inner(area);
    f.render_widget(block, area);
    let Some(files) = files else {
        let msg = if app.selected_hash().is_some() { "loading…" } else { "no commit selected" };
        f.render_widget(Paragraph::new(Line::styled(msg, DIM)), inner);
        return;
    };
    let w = inner.width as usize;
    let rows = (inner.height as usize).saturating_sub(1);
    let mut lines: Vec<Line> = Vec::new();
    for (i, fe) in files.iter().enumerate().skip(app.file_scroll).take(rows) {
        let code_st = match fe.code {
            'A' => fg(Color::Green),
            'D' => fg(Color::Red),
            'R' | 'C' => fg(Color::Cyan),
            _ => fg(Color::Yellow),
        };
        let stat = stat_spans(fe.stat);
        let stat_w: usize = stat.iter().map(|s| s.content.width()).sum();
        let name = match &fe.orig {
            Some(o) => format!("{} -> {}", sanitize(o), sanitize(&fe.path)),
            None => sanitize(&fe.path),
        };
        let room = w.saturating_sub(2 + stat_w + 1);
        // Keep the end of long paths: the file name matters most.
        let name = if name.width() > room {
            let tail: String = name.chars().rev().take(room.saturating_sub(1)).collect::<Vec<_>>().into_iter().rev().collect();
            format!("…{tail}")
        } else {
            name
        };
        let pad = room.saturating_sub(name.width());
        let mut spans = vec![Span::styled(format!("{} ", fe.code), code_st), Span::raw(name), Span::raw(" ".repeat(pad + 1))];
        spans.extend(stat);
        let mut l = Line::from(spans);
        if i == app.file_sel && focused {
            l = l.style(Style::new().bg(SEL_BG));
        }
        lines.push(l);
    }
    if files.is_empty() {
        lines.push(Line::styled("no file changes", DIM));
    }
    f.render_widget(Paragraph::new(lines), inner);
    let (a, d) = files.iter().filter_map(|fe| fe.stat.flatten()).fold((0, 0), |t, (a, d)| (t.0 + a, t.1 + d));
    let mut total = vec![Span::styled("Total ", DIM)];
    total.extend(stat_spans(Some(Some((a, d)))));
    let last = Rect { y: inner.y + inner.height.saturating_sub(1), height: 1, ..inner };
    f.render_widget(Paragraph::new(Line::from(total)), last);
}

fn draw_diff(f: &mut Frame, app: &App, area: Rect) {
    let Some(d) = &app.diff else { return };
    let short: String = d.sha.chars().take(7).collect();
    let block = pane_block(format!(" {} @ {short} ", sanitize(&d.path)), true);
    let inner = block.inner(area);
    f.render_widget(block, area);
    let lines: Vec<Line> = d
        .lines
        .iter()
        .skip(d.scroll)
        .take(inner.height as usize)
        .map(|l| {
            let text = sanitize(&l.replace('\t', "    "));
            let st = if l.starts_with("+++") || l.starts_with("---") || l.starts_with("diff ") || l.starts_with("index ") {
                DIM.add_modifier(Modifier::BOLD)
            } else if l.starts_with('+') {
                fg(Color::Green)
            } else if l.starts_with('-') {
                fg(Color::Red)
            } else if l.starts_with("@@") {
                fg(Color::Indexed(73))
            } else {
                Style::new()
            };
            Line::styled(text, st)
        })
        .collect();
    f.render_widget(Paragraph::new(lines), inner);
}

fn draw_hotkeys(f: &mut Frame, app: &App, area: Rect) {
    let mut spans = Vec::new();
    let mut used = 0;
    for (k, label) in app.hotkeys() {
        let w = k.width() + label.width() + 3;
        if used + w > area.width as usize {
            break; // drop the rest from the right
        }
        used += w;
        spans.push(Span::styled(format!(" {k} "), Style::new().add_modifier(Modifier::REVERSED)));
        spans.push(Span::raw(format!("{label} ")));
    }
    f.render_widget(Paragraph::new(Line::from(spans)), area);
}

fn draw_status(f: &mut Frame, app: &App, area: Rect) {
    let line = if app.overlay == Overlay::Search {
        Line::from(vec![Span::styled("/", fg(FOCUS).add_modifier(Modifier::BOLD)), Span::raw(sanitize(&app.search)), Span::styled("▌", fg(FOCUS))])
    } else if let Some((msg, err)) = &app.status {
        Line::styled(sanitize(msg), if *err { fg(Color::Red) } else { fg(Color::Green) })
    } else {
        Line::default()
    };
    f.render_widget(Paragraph::new(line), area);
}

fn draw_panel(f: &mut Frame, app: &App, area: Rect) {
    let short = app.selected_short().unwrap_or_default();
    let (title, items): (String, &[(&str, &str)]) = if app.overlay == Overlay::ResetMode {
        (
            format!(" Reset current branch to {short} "),
            &[("s", "Soft: keep index and files"), ("m", "Mixed: keep files"), ("h", "Hard: discard changes"), ("Esc", "Back")],
        )
    } else {
        (
            format!(" Actions on {short} "),
            &[("p", "Cherry-pick"), ("v", "Revert"), ("o", "Checkout"), ("n", "New branch"), ("r", "Reset"), ("y", "Copy hash"), ("t", "Tag")],
        )
    };
    let block = pane_block(title, true);
    let inner = block.inner(area);
    f.render_widget(Clear, area);
    f.render_widget(block, area);
    let mut spans = Vec::new();
    for (k, l) in items {
        spans.push(Span::styled(format!(" {k} "), Style::new().add_modifier(Modifier::REVERSED)));
        spans.push(Span::raw(format!(" {l}  ")));
    }
    f.render_widget(Paragraph::new(Line::from(spans)), inner);
}

fn centered(area: Rect, w: u16, h: u16) -> Rect {
    let w = w.min(area.width.saturating_sub(2));
    let h = h.min(area.height.saturating_sub(2));
    Rect { x: area.x + (area.width - w) / 2, y: area.y + (area.height - h) / 2, width: w, height: h }
}

fn input_line(label: &str, value: &str, active: bool) -> Line<'static> {
    let st = if active { Style::new().bg(SEL_BG) } else { Style::new() };
    let mut spans = vec![Span::styled(label.to_string(), DIM), Span::styled(sanitize(value), st)];
    if active {
        spans.push(Span::styled("▌", fg(FOCUS)));
    }
    Line::from(spans)
}

fn draw_modal(f: &mut Frame, title: &str, lines: Vec<Line<'static>>, area: Rect, danger: bool) {
    let h = lines.len() as u16 + 2;
    let w = lines.iter().map(|l| l.width() as u16).max().unwrap_or(20).max(title.width() as u16 + 4).clamp(40, 80) + 2;
    let r = centered(area, w, h);
    let st = if danger { fg(Color::Red).add_modifier(Modifier::BOLD) } else { fg(FOCUS).add_modifier(Modifier::BOLD) };
    let block = Block::new().borders(Borders::ALL).border_style(st).title(Span::styled(format!(" {title} "), st));
    f.render_widget(Clear, r);
    let inner = block.inner(r);
    f.render_widget(block, r);
    f.render_widget(Paragraph::new(lines), inner);
}

fn picker_lines(p: &Picker, label: &str) -> Vec<Line<'static>> {
    let mut lines = vec![input_line(label, &p.input, true), Line::default()];
    let start = p.sel.saturating_sub(9);
    for (k, &i) in p.shown.iter().enumerate().skip(start).take(10) {
        let mut l = Line::raw(format!(" {}", sanitize(&p.items[i])));
        if k == p.sel {
            l = l.style(Style::new().bg(SEL_BG).add_modifier(Modifier::BOLD));
        }
        lines.push(l);
    }
    if p.shown.is_empty() {
        lines.push(Line::styled(" no match", DIM));
    }
    lines
}

pub fn draw(f: &mut Frame, app: &App) {
    let area = f.area();
    let panel = if matches!(app.overlay, Overlay::Actions | Overlay::ResetMode) { 3 } else { 0 };
    let [header, body, panel_area, status, keys] =
        Layout::vertical([Constraint::Length(1), Constraint::Min(1), Constraint::Length(panel), Constraint::Length(1), Constraint::Length(1)]).areas(area);

    f.render_widget(Paragraph::new(Line::styled(truncate(&app.header(), header.width as usize), Style::new().add_modifier(Modifier::BOLD))), header);

    if app.focus == Focus::Diff {
        draw_diff(f, app, body);
    } else if app.wide() {
        let side = (body.width * 35 / 100).clamp(30, 60);
        let [list, sidebar] = Layout::horizontal([Constraint::Min(1), Constraint::Length(side)]).areas(body);
        draw_list(f, app, list);
        draw_sidebar(f, app, sidebar);
    } else if app.focus == Focus::Sidebar {
        draw_sidebar(f, app, body);
    } else {
        draw_list(f, app, body);
    }
    if panel > 0 {
        draw_panel(f, app, panel_area);
    }
    draw_status(f, app, status);
    draw_hotkeys(f, app, keys);

    match &app.overlay {
        Overlay::Branch(p) => draw_modal(f, "Branch", picker_lines(p, "Branch: "), body, false),
        Overlay::Author(p) => draw_modal(f, "Author", picker_lines(p, "Author: "), body, false),
        Overlay::Date { since, until, field, error } => {
            let mut lines = vec![input_line("Since: ", since, *field == 0), input_line("Until: ", until, *field == 1), Line::default()];
            lines.push(match error {
                Some(e) => Line::styled(sanitize(e), fg(Color::Red)),
                None => Line::styled("e.g. 2 weeks ago, 2026-10-01, yesterday", DIM),
            });
            draw_modal(f, "Date range", lines, body, false);
        }
        Overlay::Name { kind, input, error } => {
            let (title, label) = match kind {
                NameKind::Branch => ("New branch", "Name: "),
                NameKind::Tag => ("New tag", "Name: "),
            };
            let short = app.selected_short().unwrap_or_default();
            let mut lines = vec![Line::styled(format!("at {short}"), DIM), input_line(label, input, true)];
            if let Some(e) = error {
                lines.push(Line::styled(sanitize(e), fg(Color::Red)));
            }
            draw_modal(f, title, lines, body, false);
        }
        Overlay::Confirm { lines, need_y, .. } => {
            let ls = lines.iter().map(|l| Line::raw(l.clone())).collect();
            draw_modal(f, "Confirm", ls, body, *need_y);
        }
        _ => {}
    }
}
