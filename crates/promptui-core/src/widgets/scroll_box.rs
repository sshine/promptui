use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::widgets::Widget;

use crate::widgets::scrollbar::Scrollbar;

const SCROLLBAR_WIDTH: u16 = 1;

/// Content that can be rendered inside a [`ScrollBox`].
pub trait Scrollable {
    /// Total content height in rows for the given width.
    fn content_height(&self, width: u16) -> u16;

    /// Render content into `area` with bottom-aligned scrolling.
    /// `scroll_offset` is lines scrolled up from the bottom (0 = at bottom).
    fn render_scrolled(&self, area: Rect, buf: &mut Buffer, scroll_offset: u16);
}

/// A scrollable container with a scrollbar.
///
/// Uses bottom-aligned scrolling: offset 0 shows the most recent content.
/// Renders any [`Scrollable`] content and adds a scrollbar when content overflows.
pub struct ScrollBox {
    scroll_offset: u16,
    last_content_height: u16,
    last_viewport_height: u16,
}

impl ScrollBox {
    pub fn new() -> Self {
        Self {
            scroll_offset: 0,
            last_content_height: 0,
            last_viewport_height: 0,
        }
    }

    /// Scroll up (toward older content) by n lines.
    pub fn scroll_up(&mut self, n: u16) {
        self.scroll_offset = self.scroll_offset.saturating_add(n);
        self.clamp_scroll();
    }

    /// Scroll down (toward newer content) by n lines.
    pub fn scroll_down(&mut self, n: u16) {
        self.scroll_offset = self.scroll_offset.saturating_sub(n);
    }

    /// Scroll up by half a viewport.
    pub fn page_up(&mut self) {
        let amount = (self.last_viewport_height / 2).max(1);
        self.scroll_up(amount);
    }

    /// Scroll down by half a viewport.
    pub fn page_down(&mut self) {
        let amount = (self.last_viewport_height / 2).max(1);
        self.scroll_down(amount);
    }

    /// Scroll to the bottom (most recent content).
    pub fn scroll_to_bottom(&mut self) {
        self.scroll_offset = 0;
    }

    fn clamp_scroll(&mut self) {
        let max = self
            .last_content_height
            .saturating_sub(self.last_viewport_height);
        self.scroll_offset = self.scroll_offset.min(max);
    }

    /// Render the scroll box with the given scrollable content.
    pub fn render(&mut self, area: Rect, buf: &mut Buffer, content: &dyn Scrollable) {
        if area.width == 0 || area.height == 0 {
            return;
        }

        // Check if scrollbar is needed at full width first
        let full_height = content.content_height(area.width);
        let needs_scrollbar = full_height > area.height;

        // Only reserve the scrollbar column when content actually overflows
        let content_width = if needs_scrollbar {
            area.width.saturating_sub(SCROLLBAR_WIDTH)
        } else {
            area.width
        };
        let content_area = Rect::new(area.x, area.y, content_width, area.height);

        // Recompute height at actual content width (may differ when scrollbar steals a column)
        let content_height = if needs_scrollbar {
            content.content_height(content_width)
        } else {
            full_height
        };
        self.last_content_height = content_height;
        self.last_viewport_height = area.height;
        self.clamp_scroll();

        // Render content
        content.render_scrolled(content_area, buf, self.scroll_offset);

        // Render scrollbar
        if needs_scrollbar {
            let max_scroll = content_height.saturating_sub(area.height);
            let position = max_scroll.saturating_sub(self.scroll_offset);
            let scrollbar = Scrollbar::new(content_height, area.height, position);
            let sb_area = Rect::new(area.x + content_width, area.y, SCROLLBAR_WIDTH, area.height);
            scrollbar.render(sb_area, buf);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    struct DummyContent {
        height: u16,
    }

    impl Scrollable for DummyContent {
        fn content_height(&self, _width: u16) -> u16 {
            self.height
        }

        fn render_scrolled(&self, _area: Rect, _buf: &mut Buffer, _scroll_offset: u16) {}
    }

    #[test]
    fn scroll_clamps_to_max() {
        let mut sb = ScrollBox::new();
        sb.last_content_height = 100;
        sb.last_viewport_height = 20;
        sb.scroll_up(200);
        assert_eq!(sb.scroll_offset, 80);
    }

    #[test]
    fn scroll_down_clamps_to_zero() {
        let mut sb = ScrollBox::new();
        sb.scroll_down(10);
        assert_eq!(sb.scroll_offset, 0);
    }

    #[test]
    fn page_up_scrolls_half_viewport() {
        let mut sb = ScrollBox::new();
        sb.last_content_height = 100;
        sb.last_viewport_height = 20;
        sb.page_up();
        assert_eq!(sb.scroll_offset, 10);
    }

    #[test]
    fn page_down_scrolls_half_viewport() {
        let mut sb = ScrollBox::new();
        sb.last_content_height = 100;
        sb.last_viewport_height = 20;
        sb.scroll_offset = 30;
        sb.page_down();
        assert_eq!(sb.scroll_offset, 20);
    }

    #[test]
    fn scroll_to_bottom() {
        let mut sb = ScrollBox::new();
        sb.scroll_offset = 50;
        sb.scroll_to_bottom();
        assert_eq!(sb.scroll_offset, 0);
    }

    #[test]
    fn render_updates_state() {
        let mut sb = ScrollBox::new();
        let content = DummyContent { height: 50 };
        let area = Rect::new(0, 0, 20, 10);
        let mut buf = Buffer::empty(area);
        sb.render(area, &mut buf, &content);
        assert_eq!(sb.last_content_height, 50);
        assert_eq!(sb.last_viewport_height, 10);
    }
}
