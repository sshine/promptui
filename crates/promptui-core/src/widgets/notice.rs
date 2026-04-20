use std::time::{Duration, Instant};

use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::{Color, Modifier, Style};
use ratatui::widgets::Widget;

use crate::theme::Theme;
use crate::widgets::padding::Padding;

/// Severity level for a notice notification.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NoticeLevel {
    Debug,
    Info,
    Warn,
    Error,
}

impl NoticeLevel {
    /// Map level to the matching theme color.
    pub fn color(&self, theme: &Theme) -> Color {
        match self {
            NoticeLevel::Debug => theme.text_muted,
            NoticeLevel::Info => theme.success,
            NoticeLevel::Warn => theme.warning,
            NoticeLevel::Error => theme.error,
        }
    }

    /// Short prefix tag rendered before the message.
    pub fn label(&self) -> &'static str {
        match self {
            NoticeLevel::Debug => "DBG",
            NoticeLevel::Info => "INF",
            NoticeLevel::Warn => "WRN",
            NoticeLevel::Error => "ERR",
        }
    }
}

/// A single-line notification popup anchored to the top-right corner,
/// with a countdown progress bar and level-colored borders.
///
/// Auto-dismisses after a configurable duration (default **5 seconds**).
pub struct Notice {
    theme: Theme,
    /// Currently visible notification, if any.
    active: Option<ActiveNotice>,
    /// How long a notice stays visible.
    duration: Duration,
    /// Inset from the edges of the render area.
    padding: Padding,
}

struct ActiveNotice {
    message: String,
    level: NoticeLevel,
    shown_at: Instant,
}

/// Default display duration: 5 seconds.
const DEFAULT_DURATION: Duration = Duration::from_secs(5);

impl Notice {
    pub fn new(theme: Theme) -> Self {
        Self {
            theme,
            active: None,
            duration: DEFAULT_DURATION,
            padding: Padding::uniform(0),
        }
    }

    pub fn set_padding(&mut self, padding: Padding) {
        self.padding = padding;
    }

    /// Override the auto-dismiss duration.
    pub fn set_duration(&mut self, duration: Duration) {
        self.duration = duration;
    }

    /// The configured duration (used by external progress bars, etc.).
    pub fn duration(&self) -> Duration {
        self.duration
    }

    /// Show a notice notification. Replaces any currently visible notice.
    pub fn show(&mut self, level: NoticeLevel, message: impl Into<String>) {
        self.active = Some(ActiveNotice {
            message: message.into(),
            level,
            shown_at: Instant::now(),
        });
    }

    /// Dismiss the notice if its duration has elapsed. Call this every tick.
    pub fn tick(&mut self) {
        if let Some(ref active) = self.active {
            if active.shown_at.elapsed() >= self.duration {
                self.active = None;
            }
        }
    }

    /// Whether a notice is currently visible.
    pub fn is_visible(&self) -> bool {
        self.active.is_some()
    }

    /// Progress from 0.0 (just shown) to 1.0 (about to dismiss).
    /// Returns 0.0 if no notice is active.
    pub fn progress(&self) -> f64 {
        match &self.active {
            Some(active) => {
                let elapsed = active.shown_at.elapsed().as_secs_f64();
                let total = self.duration.as_secs_f64();
                if total <= 0.0 {
                    1.0
                } else {
                    (elapsed / total).min(1.0)
                }
            }
            None => 0.0,
        }
    }

    /// Manually dismiss any visible notice.
    pub fn dismiss(&mut self) {
        self.active = None;
    }

    /// Inner text width: border + pad + label + gap + message + pad + border.
    fn box_width(&self) -> u16 {
        if let Some(ref active) = self.active {
            let label_len = active.level.label().len(); // 3
            let msg_len = active.message.len();
            // border(1) + pad(1) + label + gap(2) + msg + pad(1) + border(1)
            (1 + 1 + label_len + 2 + msg_len + 1 + 1) as u16
        } else {
            0
        }
    }

    /// Total height: 1 blank + 1 text + 1 progress bar.
    fn box_height(&self) -> u16 {
        if self.active.is_some() { 3 } else { 0 }
    }
}

impl Widget for &Notice {
    fn render(self, area: Rect, buf: &mut Buffer) {
        let active = match &self.active {
            Some(a) => a,
            None => return,
        };

        let inner_width = area.width.saturating_sub(self.padding.horizontal());
        let inner_height = area.height.saturating_sub(self.padding.vertical());
        let width = self.box_width().min(inner_width);
        let height = self.box_height().min(inner_height);
        if width < 4 || height == 0 {
            return;
        }

        // Anchor to top-right corner, inset by padding.
        let x = area.x + area.width.saturating_sub(self.padding.right + width);
        let y = area.y + self.padding.top;

        let level_color = active.level.color(&self.theme);
        let bg = self.theme.element_bg;

        let bg_style = Style::default().bg(bg);
        let border_style = Style::default().fg(level_color).bg(bg);
        let label_style = Style::default()
            .fg(level_color)
            .bg(bg)
            .add_modifier(Modifier::BOLD);
        let msg_style = Style::default().fg(self.theme.text).bg(bg);

        // Helper: fill a row with bg + left/right borders
        let fill_bordered_row = |row_y: u16, buf: &mut Buffer| {
            for cx in x..x + width {
                if let Some(cell) = buf.cell_mut((cx, row_y)) {
                    cell.set_char(' ');
                    cell.set_style(bg_style);
                }
            }
            if let Some(cell) = buf.cell_mut((x, row_y)) {
                cell.set_char('▎');
                cell.set_style(border_style);
            }
            if let Some(cell) = buf.cell_mut((x + width - 1, row_y)) {
                cell.set_char('▕');
                cell.set_style(border_style);
            }
        };

        // --- Row 0: blank spacer ---
        fill_bordered_row(y, buf);

        // --- Row 1: text content ---
        if height >= 2 {
            let text_y = y + 1;
            fill_bordered_row(text_y, buf);

            let mut cx = x + 2; // border + 1 pad
            let x_max = x + width - 1; // stop before right border

            // Label
            for ch in active.level.label().chars() {
                if cx >= x_max {
                    break;
                }
                if let Some(cell) = buf.cell_mut((cx, text_y)) {
                    cell.set_char(ch);
                    cell.set_style(label_style);
                }
                cx += 1;
            }

            // Gap
            cx += 2;

            // Message
            for ch in active.message.chars() {
                if cx >= x_max.saturating_sub(1) {
                    break;
                }
                if let Some(cell) = buf.cell_mut((cx, text_y)) {
                    cell.set_char(ch);
                    cell.set_style(msg_style);
                }
                cx += 1;
            }
        }

        // --- Row 2: progress bar ---
        if height >= 3 {
            let bar_y = y + 2;
            fill_bordered_row(bar_y, buf);

            let progress = self.progress();
            let bar_inner = width.saturating_sub(2) as usize;
            let filled = ((bar_inner as f64) * progress) as usize;

            let bar_style = Style::default().fg(level_color).bg(bg);
            for i in 0..filled.min(bar_inner) {
                let bx = x + 1 + i as u16;
                if let Some(cell) = buf.cell_mut((bx, bar_y)) {
                    cell.set_char('▁');
                    cell.set_style(bar_style);
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn show_and_dismiss() {
        let theme = Theme::default();
        let mut notice = Notice::new(theme);
        assert!(!notice.is_visible());

        notice.show(NoticeLevel::Error, "oops");
        assert!(notice.is_visible());

        notice.dismiss();
        assert!(!notice.is_visible());
    }

    #[test]
    fn auto_dismiss_after_duration() {
        let theme = Theme::default();
        let mut notice = Notice::new(theme);
        notice.set_duration(Duration::from_millis(0));
        notice.show(NoticeLevel::Info, "hello");
        notice.tick();
        assert!(!notice.is_visible());
    }

    #[test]
    fn progress_advances() {
        let theme = Theme::default();
        let mut notice = Notice::new(theme);
        notice.show(NoticeLevel::Warn, "test");
        let p = notice.progress();
        assert!(p >= 0.0 && p <= 1.0);
    }

    #[test]
    fn progress_zero_when_inactive() {
        let theme = Theme::default();
        let notice = Notice::new(theme);
        assert_eq!(notice.progress(), 0.0);
    }

    #[test]
    fn renders_without_panic() {
        let theme = Theme::default();
        let mut notice = Notice::new(theme);
        notice.show(NoticeLevel::Warn, "watch out");
        let area = Rect::new(0, 0, 40, 3);
        let mut buf = Buffer::empty(area);
        (&notice).render(area, &mut buf);
    }

    #[test]
    fn renders_nothing_when_inactive() {
        let theme = Theme::default();
        let notice = Notice::new(theme);
        let area = Rect::new(0, 0, 40, 3);
        let mut buf = Buffer::empty(area);
        (&notice).render(area, &mut buf);
    }

    #[test]
    fn replaces_previous_notice() {
        let theme = Theme::default();
        let mut notice = Notice::new(theme);
        notice.show(NoticeLevel::Info, "first");
        notice.show(NoticeLevel::Error, "second");
        assert!(notice.is_visible());
    }

    #[test]
    fn level_labels() {
        assert_eq!(NoticeLevel::Debug.label(), "DBG");
        assert_eq!(NoticeLevel::Info.label(), "INF");
        assert_eq!(NoticeLevel::Warn.label(), "WRN");
        assert_eq!(NoticeLevel::Error.label(), "ERR");
    }

    #[test]
    fn level_colors() {
        let theme = Theme::default();
        assert_eq!(NoticeLevel::Debug.color(&theme), theme.text_muted);
        assert_eq!(NoticeLevel::Info.color(&theme), theme.success);
        assert_eq!(NoticeLevel::Warn.color(&theme), theme.warning);
        assert_eq!(NoticeLevel::Error.color(&theme), theme.error);
    }

    #[test]
    fn default_duration_is_five_seconds() {
        let theme = Theme::default();
        let notice = Notice::new(theme);
        assert_eq!(notice.duration(), Duration::from_secs(5));
    }
}
