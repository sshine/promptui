use std::time::{Duration, Instant};

use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::Style;
use ratatui::text::Text;
use ratatui::widgets::{Paragraph, Widget, Wrap};

use crate::theme::Theme;
use crate::widgets::padding::Padding;
use crate::widgets::word_wrap::count_wrapped_lines;

/// Left border character matching OpenCode's style.
const LEFT_BORDER: &str = "\u{2503}"; // ┃

/// Border occupies 1 column.
const BORDER_WIDTH: u16 = 1;

/// Trailing cursor shown while streaming is in progress.
const STREAM_CURSOR: &str = "▌";

/// Character-by-character text reveal that simulates streaming output.
///
/// Text is pushed externally via [`push_str`](StreamingText::push_str) and a
/// reveal cursor advances on each [`tick`](StreamingText::tick), making
/// characters visible at a configurable rate.
pub struct StreamingText {
    buffer: String,
    /// Number of *characters* (not bytes) currently revealed.
    revealed: usize,
    chars_per_tick: usize,
    interval: Duration,
    last_tick: Instant,
    done: bool,
    theme: Theme,
    padding: Padding,
}

impl StreamingText {
    pub fn new(theme: Theme) -> Self {
        Self {
            buffer: String::new(),
            revealed: 0,
            chars_per_tick: 2,
            interval: Duration::from_millis(30),
            last_tick: Instant::now(),
            done: false,
            theme,
            padding: Padding::uniform(0),
        }
    }

    /// Append text to the internal buffer.
    ///
    /// If the stream is already [`finish`](StreamingText::finish)ed, the new
    /// text is revealed immediately.
    pub fn push_str(&mut self, text: &str) {
        self.buffer.push_str(text);
        if self.done {
            self.revealed = self.buffer.chars().count();
        }
    }

    /// Advance the reveal cursor. Returns `true` if the visible text changed.
    pub fn tick(&mut self) -> bool {
        let total = self.buffer.chars().count();
        if self.revealed >= total {
            return false;
        }
        let now = Instant::now();
        if now.duration_since(self.last_tick) < self.interval {
            return false;
        }
        self.last_tick = now;
        let new_revealed = (self.revealed + self.chars_per_tick).min(total);
        if new_revealed != self.revealed {
            self.revealed = new_revealed;
            true
        } else {
            false
        }
    }

    /// Mark that no more text will arrive. The reveal animation continues
    /// until all buffered text has been shown.
    pub fn finish(&mut self) {
        self.done = true;
    }

    /// Instantly reveal all buffered text, skipping the animation.
    pub fn reveal_all(&mut self) {
        self.revealed = self.buffer.chars().count();
    }

    /// Whether the stream is done and all text has been revealed.
    pub fn is_finished(&self) -> bool {
        self.done && self.revealed >= self.buffer.chars().count()
    }

    /// The currently visible portion of the text.
    pub fn revealed_text(&self) -> &str {
        let byte_end = self
            .buffer
            .char_indices()
            .nth(self.revealed)
            .map(|(i, _)| i)
            .unwrap_or(self.buffer.len());
        &self.buffer[..byte_end]
    }

    /// Set how many characters are revealed per tick.
    pub fn set_chars_per_tick(&mut self, n: usize) {
        self.chars_per_tick = n.max(1);
    }

    /// Set the interval between reveal ticks.
    pub fn set_interval(&mut self, d: Duration) {
        self.interval = d;
    }

    pub fn set_padding(&mut self, padding: Padding) {
        self.padding = padding;
    }

    /// Compute the visual height needed for the currently revealed text.
    pub fn content_height(&self, width: u16) -> u16 {
        let text_width = width.saturating_sub(BORDER_WIDTH + self.padding.horizontal());
        if text_width == 0 {
            return self.padding.vertical();
        }
        let text = self.display_text();
        let mut lines: u16 = 0;
        for line in text.split('\n') {
            lines = lines.saturating_add(count_wrapped_lines(line, text_width));
        }
        lines.max(1) + self.padding.vertical()
    }

    /// Build the text to display: revealed portion + cursor if still streaming.
    fn display_text(&self) -> String {
        let revealed = self.revealed_text();
        if self.revealed >= self.buffer.chars().count() {
            revealed.to_string()
        } else {
            format!("{}{}", revealed, STREAM_CURSOR)
        }
    }
}

impl Widget for &StreamingText {
    fn render(self, area: Rect, buf: &mut Buffer) {
        if area.width == 0 || area.height == 0 {
            return;
        }

        // Fill background
        let bg_style = Style::default().bg(self.theme.panel_bg);
        for y in area.y..area.y + area.height {
            for x in area.x..area.x + area.width {
                if let Some(cell) = buf.cell_mut((x, y)) {
                    cell.set_style(bg_style);
                    cell.set_char(' ');
                }
            }
        }

        // Draw left border
        let border_style = Style::default()
            .fg(self.theme.secondary)
            .bg(self.theme.panel_bg);
        for y in area.y..area.y + area.height {
            if let Some(cell) = buf.cell_mut((area.x, y)) {
                cell.set_symbol(LEFT_BORDER);
                cell.set_style(border_style);
            }
        }

        let text_x = area.x + BORDER_WIDTH + self.padding.left;
        let text_width = area
            .width
            .saturating_sub(BORDER_WIDTH + self.padding.horizontal());
        if text_width == 0 {
            return;
        }

        let text_area = Rect {
            x: text_x,
            y: area.y + self.padding.top,
            width: text_width,
            height: area.height.saturating_sub(self.padding.vertical()),
        };
        if text_area.height == 0 {
            return;
        }

        let display = self.display_text();
        let text_style = self.theme.text_style();
        let paragraph =
            Paragraph::new(Text::styled(display, text_style)).wrap(Wrap { trim: false });
        paragraph.render(text_area, buf);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn st() -> StreamingText {
        StreamingText::new(Theme::default())
    }

    #[test]
    fn empty_initially() {
        let s = st();
        assert_eq!(s.revealed_text(), "");
        assert!(!s.is_finished());
    }

    #[test]
    fn push_does_not_reveal() {
        let mut s = st();
        s.push_str("hello");
        assert_eq!(s.revealed_text(), "");
    }

    #[test]
    fn tick_reveals_chars() {
        let mut s = st();
        s.push_str("hello world");
        // Force the interval to have elapsed
        s.last_tick = Instant::now() - Duration::from_millis(100);
        assert!(s.tick());
        // Default chars_per_tick is 2
        assert_eq!(s.revealed_text(), "he");
    }

    #[test]
    fn tick_clamps_to_buffer_length() {
        let mut s = st();
        s.push_str("hi");
        s.last_tick = Instant::now() - Duration::from_millis(100);
        s.tick();
        assert_eq!(s.revealed_text(), "hi");
    }

    #[test]
    fn finish_marks_done_without_revealing() {
        let mut s = st();
        s.push_str("hello world");
        s.finish();
        assert_eq!(s.revealed_text(), "");
        assert!(!s.is_finished());
    }

    #[test]
    fn reveal_all_shows_everything() {
        let mut s = st();
        s.push_str("hello world");
        s.reveal_all();
        assert_eq!(s.revealed_text(), "hello world");
    }

    #[test]
    fn finish_then_tick_to_completion() {
        let mut s = st();
        s.push_str("hi");
        s.finish();
        s.last_tick = Instant::now() - Duration::from_millis(100);
        assert!(s.tick()); // reveals "hi"
        assert!(s.is_finished());
    }

    #[test]
    fn push_after_finish_is_instantly_revealed() {
        let mut s = st();
        s.push_str("hello");
        s.finish();
        s.reveal_all();
        s.push_str(" world");
        assert_eq!(s.revealed_text(), "hello world");
    }

    #[test]
    fn tick_returns_false_when_fully_revealed() {
        let mut s = st();
        s.push_str("hi");
        s.reveal_all();
        assert!(!s.tick());
    }

    #[test]
    fn tick_returns_false_before_interval() {
        let mut s = st();
        s.push_str("hello");
        // last_tick is now, so interval hasn't elapsed
        assert!(!s.tick());
        assert_eq!(s.revealed_text(), "");
    }

    #[test]
    fn set_chars_per_tick() {
        let mut s = st();
        s.set_chars_per_tick(5);
        s.push_str("hello world");
        s.last_tick = Instant::now() - Duration::from_millis(100);
        s.tick();
        assert_eq!(s.revealed_text(), "hello");
    }

    #[test]
    fn unicode_reveal() {
        let mut s = st();
        s.push_str("héllo 🌍");
        s.set_chars_per_tick(3);
        s.last_tick = Instant::now() - Duration::from_millis(100);
        s.tick();
        assert_eq!(s.revealed_text(), "hél");
    }

    #[test]
    fn display_text_includes_cursor_when_partially_revealed() {
        let mut s = st();
        s.push_str("hello");
        s.last_tick = Instant::now() - Duration::from_millis(100);
        s.tick(); // reveals "he", 3 chars remain
        assert_eq!(s.display_text(), "he▌");
    }

    #[test]
    fn display_text_no_cursor_when_done() {
        let mut s = st();
        s.push_str("hi");
        s.reveal_all();
        s.finish();
        assert_eq!(s.display_text(), "hi");
    }

    #[test]
    fn content_height_basic() {
        let mut s = st();
        s.push_str("hello");
        s.reveal_all();
        s.finish();
        // width 40, border=1, no padding => text width 39, "hello" = 1 line
        assert_eq!(s.content_height(40), 1);
    }

    // Regression: finish() must not prevent tick() from revealing text.
    // Previously finish() instantly revealed all text AND tick() bailed
    // out when done==true, so the chat example never saw intermediate
    // updates and the message stayed empty.
    #[test]
    fn finish_then_tick_reveals_incrementally() {
        let mut s = st();
        s.push_str("abcdef");
        s.finish(); // marks done, but does NOT reveal
        assert_eq!(s.revealed_text(), ""); // nothing visible yet

        s.last_tick = Instant::now() - Duration::from_millis(100);
        assert!(s.tick()); // reveals 2 chars
        assert_eq!(s.revealed_text(), "ab");
        assert!(!s.is_finished()); // still more to go

        s.last_tick = Instant::now() - Duration::from_millis(100);
        s.tick();
        assert_eq!(s.revealed_text(), "abcd");

        s.last_tick = Instant::now() - Duration::from_millis(100);
        s.tick();
        assert_eq!(s.revealed_text(), "abcdef");
        assert!(s.is_finished()); // now fully done
    }

    // Regression: revealed_text must be available at the moment
    // is_finished() becomes true, so the caller can sync it.
    #[test]
    fn revealed_text_complete_when_is_finished() {
        let mut s = st();
        s.set_chars_per_tick(100);
        s.push_str("hello world");
        s.finish();
        s.last_tick = Instant::now() - Duration::from_millis(100);
        s.tick();
        assert!(s.is_finished());
        assert_eq!(s.revealed_text(), "hello world");
    }

    // Regression: cursor (▌) must show during streaming, not after.
    #[test]
    fn cursor_visible_during_streaming_hidden_after() {
        let mut s = st();
        s.push_str("hello");
        s.finish();

        // Partially revealed — cursor should show
        s.last_tick = Instant::now() - Duration::from_millis(100);
        s.tick();
        assert!(s.display_text().ends_with(STREAM_CURSOR));

        // Fully revealed + done — no cursor
        s.set_chars_per_tick(100);
        s.last_tick = Instant::now() - Duration::from_millis(100);
        s.tick();
        assert!(!s.display_text().ends_with(STREAM_CURSOR));
        assert_eq!(s.display_text(), "hello");
    }
}
