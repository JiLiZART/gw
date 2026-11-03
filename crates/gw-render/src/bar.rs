//! Live progress line: `{spinner} [<label> … <cur>/<total> <pct>%]`.

use gw_parse::Progress;
use ratatui::{
    buffer::Buffer,
    layout::Rect,
    style::{Color, Style},
    widgets::Widget,
};
use throbber_widgets_tui::{BRAILLE_SIX, Throbber, ThrobberState};

pub struct ProgressLine<'a> {
    pub progress: &'a Progress,
    pub throbber: &'a ThrobberState,
    /// `Some(true)` success, `Some(false)` failure, `None` while running.
    pub done: Option<bool>,
    pub color: bool,
}

impl Widget for ProgressLine<'_> {
    fn render(self, area: Rect, buf: &mut Buffer) {
        if area.width < 6 || area.height == 0 {
            return;
        }
        let y = area.y;
        let st = |c: Color| if self.color { Style::default().fg(c) } else { Style::default() };
        let (mark, mark_style) = match self.done {
            Some(true) => ("✔".to_string(), st(Color::Green)),
            Some(false) => ("✘".to_string(), st(Color::Red)),
            None => {
                let span = Throbber::default().throbber_set(BRAILLE_SIX).to_symbol_span(self.throbber);
                (span.content.trim().to_string(), Style::default())
            }
        };
        buf.set_string(area.x, y, &mark, mark_style);
        buf.set_string(area.x + 2, y, "[", Style::default());
        buf.set_string(area.x + area.width - 1, y, "]", Style::default());

        let x0 = area.x + 3;
        let w = (area.width - 4) as usize;
        let p = self.progress;
        let fill = p.pct.map_or(0, |pct| w * pct.min(100) as usize / 100);
        let right = match (p.cur, p.total, p.pct) {
            (Some(c), Some(t), Some(pct)) if area.width >= 40 => format!("{c}/{t} {pct}%"),
            (_, _, Some(pct)) => format!("{pct}%"),
            (Some(c), _, None) if area.width >= 40 => c.to_string(),
            _ => String::new(),
        };
        let label = if area.width >= 40 { p.label.as_str() } else { "" };

        // Text cells over the bar: label left, counters right, one space padding each side.
        let mut text: Vec<Option<char>> = vec![None; w];
        let right_start = w.saturating_sub(right.chars().count() + 1);
        for (i, c) in label.chars().enumerate() {
            if 1 + i + 1 < right_start {
                text[1 + i] = Some(c);
            }
        }
        for (i, c) in right.chars().enumerate() {
            if right_start + i < w {
                text[right_start + i] = Some(c);
            }
        }
        for (i, t) in text.iter().enumerate() {
            let filled = i < fill;
            let (sym, style) = match (self.color, t) {
                (true, _) => {
                    let (bg, fg) = if filled { (Color::Green, Color::Black) } else { (Color::DarkGray, Color::White) };
                    (t.unwrap_or(' '), Style::default().bg(bg).fg(fg))
                }
                (false, Some(c)) => (*c, Style::default()),
                (false, None) => (if filled { '#' } else { '.' }, Style::default()),
            };
            buf[(x0 + i as u16, y)].set_char(sym).set_style(style);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use ratatui::{Terminal, backend::TestBackend};

    fn draw(width: u16, pct: Option<u8>, cur: u64, done: Option<bool>, color: bool) -> Buffer {
        let p = Progress { label: "Writing objects".into(), cur: Some(cur), total: Some(9), pct, ..Default::default() };
        let mut t = Terminal::new(TestBackend::new(width, 1)).unwrap();
        let th = ThrobberState::default();
        t.draw(|f| f.render_widget(ProgressLine { progress: &p, throbber: &th, done, color }, f.area())).unwrap();
        t.backend().buffer().clone()
    }

    fn line(b: &Buffer) -> String {
        b.content.iter().map(|c| c.symbol()).collect()
    }

    fn bg(b: &Buffer, x: u16) -> Option<Color> {
        b[(x, 0)].style().bg
    }

    #[test]
    fn zero_percent() {
        let b = draw(60, Some(0), 0, None, true);
        assert!(line(&b).contains("[ Writing objects"));
        assert!(line(&b).ends_with("0/9 0% ]"));
        assert_eq!(bg(&b, 3), Some(Color::DarkGray));
    }

    #[test]
    fn forty_four_percent() {
        let b = draw(60, Some(44), 4, None, true);
        // inner width 56 -> 24 filled cells: x 3..27 green, 27.. dark
        assert_eq!(bg(&b, 26), Some(Color::Green));
        assert_eq!(bg(&b, 27), Some(Color::DarkGray));
        assert_eq!(b[(5, 0)].style().fg, Some(Color::Black), "label readable on green");
        assert!(line(&b).contains("4/9 44%"));
        let plain = line(&draw(60, Some(44), 4, None, false));
        assert!(plain.contains("[#Writing objects########."), "{plain}");
        assert!(plain.contains("....4/9 44%.]"), "{plain}");
    }

    #[test]
    fn full_success() {
        let b = draw(60, Some(100), 9, Some(true), true);
        assert!(line(&b).starts_with("✔ [ Writing objects"));
        assert!(line(&b).ends_with("9/9 100% ]"));
        assert_eq!(b[(0, 0)].style().fg, Some(Color::Green));
        assert_eq!(bg(&b, 58), Some(Color::Green));
    }

    #[test]
    fn failed_keeps_progress() {
        let b = draw(60, Some(44), 4, Some(false), true);
        assert!(line(&b).starts_with("✘"));
        assert_eq!(b[(0, 0)].style().fg, Some(Color::Red));
        assert_eq!(bg(&b, 26), Some(Color::Green));
        assert_eq!(bg(&b, 27), Some(Color::DarkGray));
    }

    #[test]
    fn narrow_drops_label() {
        let s = line(&draw(30, Some(44), 4, None, false));
        assert_eq!(s.chars().count(), 30);
        assert!(!s.contains("Writing"));
        assert!(s.ends_with("44%.]"), "{s}");
        assert!(s.contains("[###"), "{s}");
    }
}
