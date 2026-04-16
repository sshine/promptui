use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::{Color, Style};
use ratatui::widgets::Widget;

// Sub-cell precision characters (2x virtual resolution).
const FULL_BLOCK: &str = "\u{2588}"; // █
const TOP_HALF: &str = "\u{2580}"; // ▀
const BOTTOM_HALF: &str = "\u{2584}"; // ▄

/// Default scrollbar track background (#252527).
const DEFAULT_TRACK_BG: Color = Color::Rgb(37, 37, 39);

/// Default scrollbar thumb foreground (#9a9ea3).
const DEFAULT_THUMB_FG: Color = Color::Rgb(154, 158, 163);

/// A vertical scrollbar with sub-cell precision using half-block characters.
///
/// Renders a 1-column wide scrollbar. Automatically hidden when content fits.
/// Uses 2x virtual resolution for smooth thumb positioning.
#[derive(Debug, Clone, Copy)]
pub struct Scrollbar {
    /// Total number of content lines.
    pub content_length: u16,
    /// Number of visible lines in the viewport.
    pub viewport_length: u16,
    /// Current scroll position (0-based line offset from top).
    pub position: u16,
    /// Track background color.
    pub track_bg: Color,
    /// Thumb foreground color.
    pub thumb_fg: Color,
}

impl Scrollbar {
    pub fn new(content_length: u16, viewport_length: u16, position: u16) -> Self {
        Self {
            content_length,
            viewport_length,
            position,
            track_bg: DEFAULT_TRACK_BG,
            thumb_fg: DEFAULT_THUMB_FG,
        }
    }

    pub fn track_bg(mut self, color: Color) -> Self {
        self.track_bg = color;
        self
    }

    pub fn thumb_fg(mut self, color: Color) -> Self {
        self.thumb_fg = color;
        self
    }

    /// Whether the scrollbar should be visible (content exceeds viewport).
    pub fn is_visible(&self) -> bool {
        self.content_length > self.viewport_length && self.viewport_length > 0
    }
}

impl Widget for Scrollbar {
    fn render(self, area: Rect, buf: &mut Buffer) {
        if !self.is_visible() || area.width == 0 || area.height == 0 {
            return;
        }

        let track_height = area.height;
        let track_style = Style::default().bg(self.track_bg);
        let thumb_style = Style::default().fg(self.thumb_fg).bg(self.track_bg);

        // 2x virtual resolution for sub-cell precision
        let virtual_track = track_height as u32 * 2;
        let thumb_ratio = self.viewport_length as f64 / self.content_length as f64;
        let virtual_thumb = (virtual_track as f64 * thumb_ratio).round().max(2.0) as u32; // min 1 cell

        let scroll_range = self.content_length.saturating_sub(self.viewport_length);
        let value_ratio = if scroll_range > 0 {
            self.position as f64 / scroll_range as f64
        } else {
            0.0
        };
        let virtual_pos = (value_ratio * (virtual_track - virtual_thumb) as f64).round() as u32;

        let x = area.x;
        for row in 0..track_height {
            let y = area.y + row;
            let virt_top = row as u32 * 2;
            let virt_bot = virt_top + 2;

            let overlap_top = virt_top.max(virtual_pos);
            let overlap_bot = virt_bot.min(virtual_pos + virtual_thumb);

            let (symbol, style) = if overlap_top < overlap_bot {
                let coverage = overlap_bot - overlap_top;
                let sym = if coverage >= 2 {
                    FULL_BLOCK
                } else if overlap_top == virt_top {
                    TOP_HALF
                } else {
                    BOTTOM_HALF
                };
                (sym, thumb_style)
            } else {
                (" ", track_style)
            };

            if let Some(cell) = buf.cell_mut((x, y)) {
                cell.set_symbol(symbol);
                cell.set_style(style);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hidden_when_content_fits() {
        let sb = Scrollbar::new(10, 20, 0);
        assert!(!sb.is_visible());
    }

    #[test]
    fn visible_when_content_overflows() {
        let sb = Scrollbar::new(20, 10, 0);
        assert!(sb.is_visible());
    }

    #[test]
    fn hidden_when_equal() {
        let sb = Scrollbar::new(10, 10, 0);
        assert!(!sb.is_visible());
    }

    #[test]
    fn renders_without_panic() {
        let sb = Scrollbar::new(100, 10, 50);
        let area = Rect::new(0, 0, 1, 10);
        let mut buf = Buffer::empty(area);
        sb.render(area, &mut buf);
    }

    #[test]
    fn thumb_at_top_when_position_zero() {
        let sb = Scrollbar::new(100, 10, 0);
        let area = Rect::new(0, 0, 1, 10);
        let mut buf = Buffer::empty(area);
        sb.render(area, &mut buf);
        // First cell should have thumb content (not empty space)
        let first = buf.cell((0, 0)).map(|c| c.symbol().to_string());
        assert!(
            first.as_deref() == Some(FULL_BLOCK)
                || first.as_deref() == Some(TOP_HALF)
                || first.as_deref() == Some(BOTTOM_HALF),
            "expected thumb at top, got {:?}",
            first
        );
    }
}
