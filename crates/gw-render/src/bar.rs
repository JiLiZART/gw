//! Live progress line: `{spinner} [<label> … <cur>/<total> <pct>%]`.

use std::{
    collections::VecDeque,
    time::{Duration, Instant},
};

use gw_parse::Progress;
use ratatui::{
    buffer::Buffer,
    layout::Rect,
    style::{Color, Modifier, Style},
    widgets::Widget,
};

/// #008700: white text on it passes WCAG AA contrast.
const FILL_BG: Color = Color::Indexed(28);
/// #3a3a3a
const EMPTY_BG: Color = Color::Indexed(237);
use throbber_widgets_tui::{BRAILLE_SIX, Throbber, ThrobberState};

/// Fill speed in percent per second (0 to 100% in 250 ms).
const RATE: f32 = 400.0;
/// Minimum time a stage stays on screen.
const HOLD: Duration = Duration::from_millis(120);

/// Eases the displayed fill toward git's percentage and plays stages in order,
/// so stages that git finishes in milliseconds stay visible.
pub struct Animator {
    shown: Option<Progress>,
    fill: f32,
    since: Instant,
    last: Instant,
    queue: VecDeque<Progress>,
}

fn same_stage(a: &Progress, b: &Progress) -> bool {
    a.label == b.label && a.remote == b.remote
}

impl Animator {
    pub fn new(now: Instant) -> Self {
        Self { shown: None, fill: 0.0, since: now, last: now, queue: VecDeque::new() }
    }

    /// New progress from git. Updates of the latest stage replace it in place.
    pub fn push(&mut self, p: Progress, now: Instant) {
        if let Some(b) = self.queue.back_mut().filter(|b| same_stage(b, &p)) {
            *b = p;
            return;
        }
        match &mut self.shown {
            Some(s) if self.queue.is_empty() && same_stage(s, &p) => *s = p,
            Some(_) => self.queue.push_back(p),
            None => {
                self.shown = Some(p);
                self.fill = 0.0;
                self.since = now;
            }
        }
    }

    fn target(&self) -> f32 {
        self.shown.as_ref().and_then(|s| s.pct).map_or(0.0, f32::from)
    }

    /// Advances the fill and moves to the next stage once the current one caught up and was held.
    pub fn tick(&mut self, now: Instant) {
        let dt = now.saturating_duration_since(self.last).as_secs_f32();
        self.last = now;
        self.fill = (self.fill + RATE * dt).min(self.target());
        if self.fill >= self.target()
            && now.saturating_duration_since(self.since) >= HOLD
            && let Some(next) = self.queue.pop_front()
        {
            self.shown = Some(next);
            self.fill = 0.0;
            self.since = now;
        }
    }

    /// Displayed stage and its eased fill percentage.
    pub fn current(&self) -> Option<(&Progress, u8)> {
        self.shown.as_ref().map(|s| (s, self.fill as u8))
    }

    /// Git's latest reported state, regardless of easing.
    pub fn latest(&self) -> Option<&Progress> {
        self.queue.back().or(self.shown.as_ref())
    }

    /// Nothing left to play: last stage shown and filled to its percentage.
    pub fn settled(&self) -> bool {
        self.queue.is_empty() && self.fill >= self.target()
    }
}

pub struct ProgressLine<'a> {
    pub progress: &'a Progress,
    /// Bar fill percentage; `None` uses `progress.pct`.
    pub fill: Option<u8>,
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
        let fill = self.fill.or(p.pct).map_or(0, |pct| w * pct.min(100) as usize / 100);
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
                    // Fixed 256-color shades: ANSI green/black vary by theme and can be unreadable.
                    let style = if filled {
                        Style::default().bg(FILL_BG).fg(Color::Indexed(231)).add_modifier(Modifier::BOLD)
                    } else {
                        Style::default().bg(EMPTY_BG).fg(Color::Indexed(252))
                    };
                    (t.unwrap_or(' '), style)
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
        t.draw(|f| f.render_widget(ProgressLine { progress: &p, fill: None, throbber: &th, done, color }, f.area())).unwrap();
        t.backend().buffer().clone()
    }

    fn line(b: &Buffer) -> String {
        b.content.iter().map(|c| c.symbol()).collect()
    }

    fn bg(b: &Buffer, x: u16) -> Option<Color> {
        b[(x, 0)].style().bg
    }

    fn stage(label: &str, pct: u8) -> Progress {
        Progress { label: label.into(), pct: Some(pct), ..Default::default() }
    }

    #[test]
    fn animator_plays_instant_stages_in_order() {
        let t0 = Instant::now();
        let ms = |n| t0 + Duration::from_millis(n);
        let mut a = Animator::new(t0);
        for (i, p) in [stage("Compressing objects", 50), stage("Compressing objects", 100), stage("Writing objects", 100)].into_iter().enumerate() {
            a.push(p, ms(i as u64));
        }
        a.push(stage("Resolving deltas", 100), ms(3));
        assert_eq!(a.latest().unwrap().label, "Resolving deltas");
        a.tick(ms(100));
        let (p, fill) = a.current().unwrap();
        assert_eq!((p.label.as_str(), fill), ("Compressing objects", 40), "merged to 100%, eased 400%/s");
        a.tick(ms(260));
        assert_eq!(a.current().unwrap().0.label, "Writing objects", "caught up and held -> next stage");
        assert_eq!(a.current().unwrap().1, 0);
        a.tick(ms(520));
        assert_eq!(a.current().unwrap().0.label, "Resolving deltas");
        assert!(!a.settled());
        a.tick(ms(800));
        assert!(a.settled());
    }

    #[test]
    fn fill_override() {
        let p = stage("Writing objects", 100);
        let mut t = Terminal::new(TestBackend::new(60, 1)).unwrap();
        let th = ThrobberState::default();
        t.draw(|f| f.render_widget(ProgressLine { progress: &p, fill: Some(50), throbber: &th, done: None, color: true }, f.area())).unwrap();
        let b = t.backend().buffer();
        assert_eq!(bg(b, 30), Some(FILL_BG));
        assert_eq!(bg(b, 31), Some(EMPTY_BG));
        assert!(line(b).contains("100%"), "counters show git's real value");
    }

    #[test]
    fn zero_percent() {
        let b = draw(60, Some(0), 0, None, true);
        assert!(line(&b).contains("[ Writing objects"));
        assert!(line(&b).ends_with("0/9 0% ]"));
        assert_eq!(bg(&b, 3), Some(EMPTY_BG));
    }

    #[test]
    fn forty_four_percent() {
        let b = draw(60, Some(44), 4, None, true);
        // inner width 56 -> 24 filled cells: x 3..27 green, 27.. dark
        assert_eq!(bg(&b, 26), Some(FILL_BG));
        assert_eq!(bg(&b, 27), Some(EMPTY_BG));
        assert_eq!(b[(5, 0)].style().fg, Some(Color::Indexed(231)), "label readable on green");
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
        assert_eq!(bg(&b, 58), Some(FILL_BG));
    }

    #[test]
    fn failed_keeps_progress() {
        let b = draw(60, Some(44), 4, Some(false), true);
        assert!(line(&b).starts_with("✘"));
        assert_eq!(b[(0, 0)].style().fg, Some(Color::Red));
        assert_eq!(bg(&b, 26), Some(FILL_BG));
        assert_eq!(bg(&b, 27), Some(EMPTY_BG));
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
