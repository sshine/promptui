use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::Style;
use ratatui::text::Text;
use ratatui::widgets::{Paragraph, Widget, Wrap};

use crate::theme::Theme;
use crate::widgets::padding::Padding;
use crate::widgets::scroll_box::Scrollable;
use crate::widgets::word_wrap::count_wrapped_lines;

/// Left border character matching the existing identity pattern.
const LEFT_BORDER: &str = "\u{2503}"; // ┃

/// Border occupies 1 column.
const BORDER_WIDTH: u16 = 1;

/// Overflow indicator shown when content is truncated.
const OVERFLOW_HINT: &str = "\u{25B8}"; // ▸

/// A content block that shows a limited number of lines when collapsed.
///
/// Expansion state is driven externally — the widget just respects
/// `expanded: bool`. When collapsed, an optional overflow hint shows
/// how many lines are hidden (e.g., "▸ 12 more lines").
#[derive(Clone)]
pub struct TruncatedBlock {
    theme: Theme,
    padding: Padding,
    content: Text<'static>,
    /// Maximum visible lines when collapsed. `None` means show all.
    max_lines: Option<u16>,
    /// Whether the block is currently expanded. Driven externally.
    expanded: bool,
    /// Whether to show the "▸ N more lines" hint when truncated.
    show_overflow_hint: bool,
}

impl TruncatedBlock {
    pub fn new(theme: Theme, content: Text<'static>) -> Self {
        Self {
            theme,
            padding: Padding::uniform(0),
            content,
            max_lines: Some(5),
            expanded: false,
            show_overflow_hint: true,
        }
    }

    pub fn max_lines(mut self, n: u16) -> Self {
        self.max_lines = Some(n);
        self
    }

    pub fn no_max_lines(mut self) -> Self {
        self.max_lines = None;
        self
    }

    pub fn expanded(mut self, expanded: bool) -> Self {
        self.expanded = expanded;
        self
    }

    pub fn show_overflow_hint(mut self, show: bool) -> Self {
        self.show_overflow_hint = show;
        self
    }

    pub fn padding(mut self, padding: Padding) -> Self {
        self.padding = padding;
        self
    }

    pub fn set_padding(&mut self, padding: Padding) {
        self.padding = padding;
    }

    pub fn set_expanded(&mut self, expanded: bool) {
        self.expanded = expanded;
    }

    pub fn set_max_lines(&mut self, n: Option<u16>) {
        self.max_lines = n;
    }

    pub fn set_content(&mut self, content: Text<'static>) {
        self.content = content;
    }

    /// Horizontal overhead: border + padding.
    fn h_overhead(&self) -> u16 {
        BORDER_WIDTH + self.padding.horizontal()
    }

    /// Extract plain text from the content, joining lines with newlines.
    fn plain_text(&self) -> String {
        let mut s = String::new();
        for (i, line) in self.content.lines.iter().enumerate() {
            if i > 0 {
                s.push('\n');
            }
            for span in &line.spans {
                s.push_str(&span.content);
            }
        }
        s
    }

    /// Total content height in wrapped lines for a given text width.
    fn full_content_height(&self, text_width: u16) -> u16 {
        if text_width == 0 {
            return 0;
        }
        let plain = self.plain_text();
        if plain.is_empty() {
            return 1;
        }
        let mut total: u16 = 0;
        for line in plain.split('\n') {
            total = total.saturating_add(count_wrapped_lines(line, text_width));
        }
        total
    }

    /// The number of visible content lines (respecting max_lines and expanded state).
    fn visible_content_lines(&self, text_width: u16) -> u16 {
        let full = self.full_content_height(text_width);
        if self.expanded || self.max_lines.is_none() {
            return full;
        }
        let max = self.max_lines.unwrap();
        if full <= max {
            return full;
        }
        // When truncated with hint, the hint takes 1 row from the budget
        if self.show_overflow_hint {
            max.saturating_sub(1)
        } else {
            max
        }
    }

    /// Whether content is currently truncated at the given width.
    pub fn is_truncated(&self, width: u16) -> bool {
        if self.expanded || self.max_lines.is_none() {
            return false;
        }
        let text_width = width.saturating_sub(self.h_overhead());
        let full = self.full_content_height(text_width);
        let max = self.max_lines.unwrap();
        full > max
    }

    /// Number of lines hidden when collapsed.
    pub fn hidden_line_count(&self, width: u16) -> u16 {
        let text_width = width.saturating_sub(self.h_overhead());
        let full = self.full_content_height(text_width);
        let visible = self.visible_content_lines(text_width);
        full.saturating_sub(visible)
    }

    /// Total height this block needs to render (content + padding + optional hint).
    pub fn required_height(&self, width: u16) -> u16 {
        let text_width = width.saturating_sub(self.h_overhead());
        if text_width == 0 {
            return 0;
        }
        let full = self.full_content_height(text_width);

        let content_lines = if self.expanded || self.max_lines.is_none() {
            full
        } else {
            let max = self.max_lines.unwrap();
            if full <= max {
                full
            } else if self.show_overflow_hint {
                // hint row is included in the max budget
                max
            } else {
                max
            }
        };

        content_lines + self.padding.vertical()
    }
}

impl Scrollable for TruncatedBlock {
    fn content_height(&self, width: u16) -> u16 {
        self.required_height(width)
    }

    fn render_scrolled(&self, area: Rect, buf: &mut Buffer, scroll_offset: u16) {
        // For now, ignore scroll_offset — TruncatedBlock handles its own truncation.
        // A future version could support scrolling within an expanded block.
        let _ = scroll_offset;
        (&*self).render(area, buf);
    }
}

impl Widget for &TruncatedBlock {
    fn render(self, area: Rect, buf: &mut Buffer) {
        if area.width == 0 || area.height == 0 {
            return;
        }

        let bg_style = Style::default().bg(self.theme.panel_bg);
        let border_style = Style::default()
            .fg(self.theme.secondary)
            .bg(self.theme.panel_bg);
        let text_style = self.theme.text_style();
        let hint_style = self.theme.muted_style();

        // Fill background
        for y in area.y..area.y + area.height {
            for x in area.x..area.x + area.width {
                if let Some(cell) = buf.cell_mut((x, y)) {
                    cell.set_char(' ');
                    cell.set_style(bg_style);
                }
            }
        }

        // Draw left border
        for y in area.y..area.y + area.height {
            if let Some(cell) = buf.cell_mut((area.x, y)) {
                cell.set_symbol(LEFT_BORDER);
                cell.set_style(border_style);
            }
        }

        let text_x = area.x + BORDER_WIDTH + self.padding.left;
        let text_width = area.width.saturating_sub(self.h_overhead());
        if text_width == 0 {
            return;
        }

        let full_height = self.full_content_height(text_width);
        let truncated =
            !self.expanded && self.max_lines.is_some() && full_height > self.max_lines.unwrap();

        let visible_lines = self.visible_content_lines(text_width);

        // Render content text
        let content_area = Rect {
            x: text_x,
            y: area.y + self.padding.top,
            width: text_width,
            height: visible_lines.min(area.height.saturating_sub(self.padding.vertical())),
        };

        if content_area.height > 0 {
            let paragraph = Paragraph::new(self.content.clone())
                .style(text_style)
                .wrap(Wrap { trim: false });
            paragraph.render(content_area, buf);
        }

        // Render overflow hint
        if truncated && self.show_overflow_hint {
            let hidden = full_height.saturating_sub(visible_lines);
            let hint = format!("{} {} more lines", OVERFLOW_HINT, hidden);
            let hint_y = content_area.y + content_area.height;

            if hint_y < area.y + area.height {
                for (i, ch) in hint.chars().enumerate() {
                    let x = text_x + i as u16;
                    if x >= area.x + area.width {
                        break;
                    }
                    if let Some(cell) = buf.cell_mut((x, hint_y)) {
                        cell.set_char(ch);
                        cell.set_style(hint_style);
                    }
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_theme() -> Theme {
        Theme::default()
    }

    #[test]
    fn default_max_lines_is_5() {
        let block = TruncatedBlock::new(test_theme(), Text::from("hello"));
        assert_eq!(block.max_lines, Some(5));
    }

    #[test]
    fn short_content_not_truncated() {
        let block = TruncatedBlock::new(test_theme(), Text::from("hello"));
        assert!(!block.is_truncated(40));
    }

    #[test]
    fn long_content_is_truncated() {
        let content = "line1\nline2\nline3\nline4\nline5\nline6\nline7\nline8";
        let block = TruncatedBlock::new(test_theme(), Text::from(content));
        // 8 lines > max_lines of 5
        assert!(block.is_truncated(40));
    }

    #[test]
    fn expanded_never_truncated() {
        let content = "line1\nline2\nline3\nline4\nline5\nline6\nline7\nline8";
        let block = TruncatedBlock::new(test_theme(), Text::from(content)).expanded(true);
        assert!(!block.is_truncated(40));
    }

    #[test]
    fn hidden_line_count() {
        let content = "line1\nline2\nline3\nline4\nline5\nline6\nline7\nline8";
        let block = TruncatedBlock::new(test_theme(), Text::from(content));
        // max_lines=5, hint takes 1 row, so 4 visible content lines, 4 hidden
        assert_eq!(block.hidden_line_count(40), 4);
    }

    #[test]
    fn required_height_collapsed() {
        let content = "line1\nline2\nline3\nline4\nline5\nline6\nline7\nline8";
        let block = TruncatedBlock::new(test_theme(), Text::from(content));
        // 8 lines, max_lines=5, so height = 5 (4 content + 1 hint)
        assert_eq!(block.required_height(40), 5);
    }

    #[test]
    fn required_height_expanded() {
        let content = "line1\nline2\nline3\nline4\nline5\nline6\nline7\nline8";
        let block = TruncatedBlock::new(test_theme(), Text::from(content)).expanded(true);
        assert_eq!(block.required_height(40), 8);
    }

    #[test]
    fn required_height_fits_within_max() {
        let content = "line1\nline2\nline3";
        let block = TruncatedBlock::new(test_theme(), Text::from(content));
        // 3 lines < max_lines of 5, so height = 3
        assert_eq!(block.required_height(40), 3);
    }

    #[test]
    fn no_max_lines_shows_all() {
        let content = "line1\nline2\nline3\nline4\nline5\nline6\nline7\nline8";
        let block = TruncatedBlock::new(test_theme(), Text::from(content)).no_max_lines();
        assert!(!block.is_truncated(40));
        assert_eq!(block.required_height(40), 8);
    }

    #[test]
    fn scrollable_content_height() {
        let content = "line1\nline2\nline3";
        let block = TruncatedBlock::new(test_theme(), Text::from(content));
        assert_eq!(block.content_height(40), 3);
    }

    #[test]
    fn render_does_not_panic_on_zero_area() {
        let block = TruncatedBlock::new(test_theme(), Text::from("hello"));
        let area = Rect::new(0, 0, 0, 0);
        let mut buf = Buffer::empty(Rect::new(0, 0, 1, 1));
        (&block).render(area, &mut buf);
    }

    #[test]
    fn render_draws_left_border() {
        let block = TruncatedBlock::new(test_theme(), Text::from("hello"));
        let area = Rect::new(0, 0, 20, 3);
        let mut buf = Buffer::empty(area);
        (&block).render(area, &mut buf);
        assert_eq!(buf.cell((0, 0)).unwrap().symbol(), LEFT_BORDER);
    }
}
