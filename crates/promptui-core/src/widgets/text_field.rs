use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::{Color, Modifier, Style};
use ratatui::widgets::Widget;
use unicode_width::UnicodeWidthChar;

use crate::input::Input;
use crate::theme::Theme;
use crate::widgets::scrollbar::Scrollbar;
use crate::widgets::word_wrap::{count_wrapped_lines, wrap_lines};

/// Left border character matching OpenCode's style.
const LEFT_BORDER: &str = "\u{2503}"; // ┃

/// Border occupies 1 column.
const BORDER_WIDTH: u16 = 1;

/// Scrollbar occupies 1 column on the right when visible.
const SCROLLBAR_WIDTH: u16 = 1;

/// Reserve 1 column for the cursor so wrapping treats the cursor like a character.
const CURSOR_RESERVE: u16 = 1;

/// Padding configuration for the text field.
///
/// Each value is a number of cells (columns for horizontal, rows for vertical).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Padding {
    pub left: u16,
    pub right: u16,
    pub top: u16,
    pub bottom: u16,
}

impl Padding {
    /// Uniform padding on all sides.
    pub fn uniform(n: u16) -> Self {
        Self {
            left: n,
            right: n,
            top: n,
            bottom: n,
        }
    }

    /// Separate horizontal and vertical padding.
    pub fn symmetric(horizontal: u16, vertical: u16) -> Self {
        Self {
            left: horizontal,
            right: horizontal,
            top: vertical,
            bottom: vertical,
        }
    }

    /// Total horizontal padding (left + right).
    fn horizontal(&self) -> u16 {
        self.left + self.right
    }

    /// Total vertical padding (top + bottom).
    fn vertical(&self) -> u16 {
        self.top + self.bottom
    }
}

impl Default for Padding {
    fn default() -> Self {
        Self::uniform(1)
    }
}

/// Result of processing an input event.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TextFieldAction {
    /// The input was consumed but no special action occurred.
    Edited,
    /// The user pressed Enter; contains the submitted text.
    Submit(String),
    /// The input was not recognized or not consumed.
    Ignored,
}

/// A word-wrapping text input widget with dynamic height.
///
/// Owns its state (text content, cursor position) and renders by reference.
/// Inspired by ratatui-textarea but with built-in word wrapping, theme support,
/// and Enter/Shift+Enter distinction.
pub struct TextField {
    lines: Vec<String>,
    cursor: (usize, usize), // (logical_row, char_offset)
    theme: Theme,
    padding: Padding,
    border_color: Color,
    min_height: u16,
    max_height: u16,
    focused: bool,
    scroll_offset: u16, // visual lines scrolled from top
    last_content_width: u16,
}

impl TextField {
    pub fn new(theme: Theme) -> Self {
        Self {
            lines: vec![String::new()],
            cursor: (0, 0),
            border_color: theme.primary,
            theme,
            padding: Padding::default(),
            min_height: 3,
            max_height: 8,
            focused: true,
            scroll_offset: 0,
            last_content_width: 0,
        }
    }

    pub fn set_focused(&mut self, focused: bool) {
        self.focused = focused;
    }

    pub fn set_padding(&mut self, padding: Padding) {
        self.padding = padding;
    }

    pub fn set_border_color(&mut self, color: Color) {
        self.border_color = color;
    }

    pub fn set_min_height(&mut self, min: u16) {
        self.min_height = min;
    }

    pub fn set_max_height(&mut self, max: u16) {
        self.max_height = max;
    }

    /// Returns the text content, joining logical lines with newlines.
    pub fn content(&self) -> String {
        self.lines.join("\n")
    }

    /// Clears the text field and resets the cursor.
    pub fn clear(&mut self) {
        self.lines = vec![String::new()];
        self.cursor = (0, 0);
        self.scroll_offset = 0;
    }

    /// Total horizontal overhead: border + padding left + padding right + scrollbar.
    fn h_overhead(&self) -> u16 {
        BORDER_WIDTH + self.padding.horizontal() + SCROLLBAR_WIDTH
    }

    /// Computes the required visual height for the current content at the given width.
    ///
    /// The width should be the total available width for the widget.
    /// Returns the outer height including padding.
    pub fn required_height(&mut self, width: u16) -> u16 {
        let content_width = width.saturating_sub(self.h_overhead());
        self.last_content_width = content_width;

        let wrap_width = content_width.saturating_sub(CURSOR_RESERVE);
        if wrap_width == 0 {
            return self.min_height;
        }

        let total_visual = self.total_visual_lines(wrap_width);

        let content_height = total_visual.clamp(self.min_height, self.max_height);

        // Ensure scroll keeps cursor visible
        self.ensure_cursor_visible(wrap_width, content_height);

        content_height + self.padding.vertical()
    }

    /// The effective width used for word wrapping (content width minus cursor reserve).
    fn wrap_width(&self) -> u16 {
        self.last_content_width.saturating_sub(CURSOR_RESERVE)
    }

    /// Total number of visual (wrapped) lines across all logical lines.
    fn total_visual_lines(&self, wrap_width: u16) -> u16 {
        let mut total: u16 = 0;
        for line in &self.lines {
            total = total.saturating_add(count_wrapped_lines(line, wrap_width));
        }
        total
    }

    /// Returns (visual_line_index, display_column) for the current cursor.
    ///
    /// At a wrap boundary the cursor belongs to the start of the next visual line.
    /// At the end of a logical line the cursor belongs to the last visual line.
    fn cursor_visual_position(&self, wrap_width: u16) -> (u16, u16) {
        let mut visual_row: u16 = 0;
        for (i, line) in self.lines.iter().enumerate() {
            if i == self.cursor.0 {
                let ranges = wrap_lines(line, wrap_width);
                let mut chars_counted: usize = 0;
                for (range_idx, &(start, end)) in ranges.iter().enumerate() {
                    let seg_chars = line[start..end].chars().count();
                    let is_last = range_idx == ranges.len() - 1;
                    if self.cursor.1 < chars_counted + seg_chars || is_last {
                        let col_in_seg = self.cursor.1.saturating_sub(chars_counted);
                        let display_col: u16 = line[start..]
                            .chars()
                            .take(col_in_seg)
                            .map(|c| c.width().unwrap_or(0) as u16)
                            .sum();
                        return (visual_row + range_idx as u16, display_col);
                    }
                    chars_counted += seg_chars;
                }
                return (visual_row, 0);
            }
            visual_row += count_wrapped_lines(line, wrap_width);
        }
        (visual_row, 0)
    }

    /// Visual line index where the cursor sits.
    fn cursor_visual_line(&self, wrap_width: u16) -> u16 {
        self.cursor_visual_position(wrap_width).0
    }

    /// Map a target visual line and display column to a logical cursor (row, char_offset).
    fn visual_to_cursor(
        &self,
        target_visual: u16,
        target_display_col: u16,
        wrap_width: u16,
    ) -> (usize, usize) {
        let mut visual_line: u16 = 0;
        for (line_idx, line) in self.lines.iter().enumerate() {
            let ranges = wrap_lines(line, wrap_width);
            for &(start, end) in &ranges {
                if visual_line == target_visual {
                    let segment = &line[start..end];
                    let chars_before_segment = line[..start].chars().count();
                    let mut col: u16 = 0;
                    let mut char_count: usize = 0;
                    for ch in segment.chars() {
                        let w = ch.width().unwrap_or(0) as u16;
                        if col + w > target_display_col {
                            break;
                        }
                        col += w;
                        char_count += 1;
                    }
                    return (line_idx, chars_before_segment + char_count);
                }
                visual_line += 1;
            }
        }
        let last_row = self.lines.len().saturating_sub(1);
        let last_col = self.lines.last().map_or(0, |l| l.chars().count());
        (last_row, last_col)
    }

    /// Adjust scroll_offset so the cursor is visible within the given height.
    fn ensure_cursor_visible(&mut self, wrap_width: u16, visible_height: u16) {
        if visible_height == 0 {
            return;
        }
        let cursor_line = self.cursor_visual_line(wrap_width);

        // Scroll up if cursor is above viewport
        if cursor_line < self.scroll_offset {
            self.scroll_offset = cursor_line;
        }
        // Scroll down if cursor is below viewport
        if cursor_line >= self.scroll_offset + visible_height {
            self.scroll_offset = cursor_line + 1 - visible_height;
        }
    }

    /// Returns the absolute cursor position within the given render area,
    /// suitable for `frame.set_cursor_position()`.
    pub fn cursor_position(&self, area: Rect) -> Option<(u16, u16)> {
        if !self.focused {
            return None;
        }

        let wrap_w = self.wrap_width();
        if wrap_w == 0 {
            return None;
        }

        let cursor_visual = self.cursor_visual_line(wrap_w);
        let visual_in_viewport = cursor_visual.saturating_sub(self.scroll_offset);

        // Find the display column
        let current_line = self.lines.get(self.cursor.0).map_or("", |s| s.as_str());
        let ranges = wrap_lines(current_line, wrap_w);

        let mut chars_counted: usize = 0;
        for (range_idx, &(start, end)) in ranges.iter().enumerate() {
            let segment = &current_line[start..end];
            let segment_chars = segment.chars().count();

            if self.cursor.1 < chars_counted + segment_chars || range_idx == ranges.len() - 1 {
                let col_in_segment = self.cursor.1.saturating_sub(chars_counted);
                let display_col: u16 = segment
                    .chars()
                    .take(col_in_segment)
                    .map(|c| c.width().unwrap_or(0) as u16)
                    .sum();

                let x = area.x + BORDER_WIDTH + self.padding.left + display_col;
                let y = area.y + self.padding.top + visual_in_viewport;

                if y < area.y + area.height && x < area.x + area.width {
                    return Some((x, y));
                }
                return None;
            }

            chars_counted += segment_chars;
        }

        let x = area.x + BORDER_WIDTH + self.padding.left;
        let y = area.y + self.padding.top + visual_in_viewport;
        Some((x, y))
    }

    /// Process an input event. Returns what action resulted.
    pub fn input(&mut self, input: Input) -> TextFieldAction {
        match input {
            Input::Char(c) => {
                self.insert_char(c);
                TextFieldAction::Edited
            }
            Input::Enter => {
                let content = self.content();
                if content.trim().is_empty() {
                    return TextFieldAction::Ignored;
                }
                self.clear();
                TextFieldAction::Submit(content)
            }
            Input::Newline => {
                self.insert_newline();
                TextFieldAction::Edited
            }

            Input::Backspace => {
                if self.delete_backward() {
                    TextFieldAction::Edited
                } else {
                    TextFieldAction::Ignored
                }
            }
            Input::Delete => {
                if self.delete_forward() {
                    TextFieldAction::Edited
                } else {
                    TextFieldAction::Ignored
                }
            }
            Input::ClearAll => {
                if self.lines.len() == 1 && self.lines[0].is_empty() {
                    return TextFieldAction::Ignored;
                }
                self.clear();
                TextFieldAction::Edited
            }

            Input::Left => {
                self.move_left();
                TextFieldAction::Edited
            }
            Input::Right => {
                self.move_right();
                TextFieldAction::Edited
            }
            Input::Up => {
                self.move_up();
                TextFieldAction::Edited
            }
            Input::Down => {
                self.move_down();
                TextFieldAction::Edited
            }
            Input::Home => {
                self.cursor.1 = 0;
                TextFieldAction::Edited
            }
            Input::End => {
                self.cursor.1 = self.current_line_char_len();
                TextFieldAction::Edited
            }

            _ => TextFieldAction::Ignored,
        }
    }

    // --- Text insertion ---

    fn insert_char(&mut self, c: char) {
        let (row, col) = self.cursor;
        if let Some(line) = self.lines.get_mut(row) {
            let byte_idx = char_to_byte_index(line, col);
            line.insert(byte_idx, c);
            self.cursor.1 += 1;
        }
    }

    fn insert_newline(&mut self) {
        let (row, col) = self.cursor;
        if let Some(line) = self.lines.get_mut(row) {
            let byte_idx = char_to_byte_index(line, col);
            let remainder = line[byte_idx..].to_string();
            line.truncate(byte_idx);
            self.lines.insert(row + 1, remainder);
            self.cursor = (row + 1, 0);
        }
    }

    // --- Character deletion ---

    fn delete_backward(&mut self) -> bool {
        let (row, col) = self.cursor;
        if col > 0 {
            if let Some(line) = self.lines.get_mut(row) {
                let byte_idx = char_to_byte_index(line, col);
                let prev_byte_idx = char_to_byte_index(line, col - 1);
                line.drain(prev_byte_idx..byte_idx);
                self.cursor.1 -= 1;
                return true;
            }
        } else if row > 0 {
            let current = self.lines.remove(row);
            if let Some(prev) = self.lines.get_mut(row - 1) {
                let prev_len = prev.chars().count();
                prev.push_str(&current);
                self.cursor = (row - 1, prev_len);
                return true;
            }
        }
        false
    }

    fn delete_forward(&mut self) -> bool {
        let (row, col) = self.cursor;
        let line_len = self.current_line_char_len();
        if col < line_len {
            if let Some(line) = self.lines.get_mut(row) {
                let byte_idx = char_to_byte_index(line, col);
                let next_byte_idx = char_to_byte_index(line, col + 1);
                line.drain(byte_idx..next_byte_idx);
                return true;
            }
        } else if row + 1 < self.lines.len() {
            let next = self.lines.remove(row + 1);
            if let Some(current) = self.lines.get_mut(row) {
                current.push_str(&next);
                return true;
            }
        }
        false
    }

    // --- Cursor movement ---

    fn move_left(&mut self) {
        let (row, col) = self.cursor;
        if col > 0 {
            self.cursor.1 -= 1;
        } else if row > 0 {
            self.cursor.0 -= 1;
            self.cursor.1 = self.lines.get(row - 1).map_or(0, |l| l.chars().count());
        }
    }

    fn move_right(&mut self) {
        let (row, col) = self.cursor;
        let line_len = self.current_line_char_len();
        if col < line_len {
            self.cursor.1 += 1;
        } else if row + 1 < self.lines.len() {
            self.cursor.0 += 1;
            self.cursor.1 = 0;
        }
    }

    fn move_up(&mut self) {
        let wrap_w = self.wrap_width();
        if wrap_w == 0 {
            return;
        }
        let (vis_line, display_col) = self.cursor_visual_position(wrap_w);
        if vis_line > 0 {
            self.cursor = self.visual_to_cursor(vis_line - 1, display_col, wrap_w);
        }
    }

    fn move_down(&mut self) {
        let wrap_w = self.wrap_width();
        if wrap_w == 0 {
            return;
        }
        let (vis_line, display_col) = self.cursor_visual_position(wrap_w);
        let total = self.total_visual_lines(wrap_w);
        if vis_line + 1 < total {
            self.cursor = self.visual_to_cursor(vis_line + 1, display_col, wrap_w);
        }
    }

    fn current_line_char_len(&self) -> usize {
        self.lines
            .get(self.cursor.0)
            .map_or(0, |l| l.chars().count())
    }
}

impl Widget for &TextField {
    fn render(self, area: Rect, buf: &mut Buffer) {
        if area.width == 0 || area.height == 0 {
            return;
        }

        let wrap_w = self.wrap_width();

        // Fill background
        let bg_style = self.theme.input_style();
        for y in area.y..area.y + area.height {
            for x in area.x..area.x + area.width {
                if let Some(cell) = buf.cell_mut((x, y)) {
                    cell.set_style(bg_style);
                    cell.set_char(' ');
                }
            }
        }

        // Draw left border
        let border_fg = if self.focused {
            self.border_color
        } else {
            self.theme.border
        };
        let border_style = Style::default().fg(border_fg).bg(self.theme.element_bg);
        for y in area.y..area.y + area.height {
            if let Some(cell) = buf.cell_mut((area.x, y)) {
                cell.set_style(border_style);
                cell.set_symbol(LEFT_BORDER);
            }
        }

        // Content area
        let text_x = area.x + BORDER_WIDTH + self.padding.left;
        let text_y = area.y + self.padding.top;
        let visible_rows = area.height.saturating_sub(self.padding.vertical());

        // Render wrapped text with scroll offset
        let text_style = self.theme.input_style();
        let cursor_style = text_style.add_modifier(Modifier::REVERSED);

        let total_visual = self.total_visual_lines(wrap_w);

        let mut visual_line: u16 = 0; // absolute visual line index across all content

        for (line_idx, line) in self.lines.iter().enumerate() {
            let ranges = wrap_lines(line, wrap_w);
            let num_ranges = ranges.len();

            let mut chars_counted: usize = 0;
            for (range_idx, &(start, end)) in ranges.iter().enumerate() {
                let in_viewport = visual_line >= self.scroll_offset
                    && visual_line < self.scroll_offset + visible_rows;

                if in_viewport {
                    let row_in_view = visual_line - self.scroll_offset;
                    let y = text_y + row_in_view;
                    let segment = &line[start..end];
                    let mut x = text_x;

                    for (char_idx_in_seg, ch) in segment.chars().enumerate() {
                        let absolute_char_idx = chars_counted + char_idx_in_seg;
                        let ch_width = ch.width().unwrap_or(0) as u16;

                        if x + ch_width
                            > area.x
                                + area
                                    .width
                                    .saturating_sub(self.padding.right + SCROLLBAR_WIDTH)
                        {
                            break;
                        }

                        let is_cursor = self.focused
                            && line_idx == self.cursor.0
                            && absolute_char_idx == self.cursor.1;

                        let style = if is_cursor { cursor_style } else { text_style };

                        if let Some(cell) = buf.cell_mut((x, y)) {
                            cell.set_char(ch);
                            cell.set_style(style);
                        }
                        x += ch_width;
                    }

                    // Draw cursor at end of segment only for the LAST segment of
                    // a logical line. At wrap boundaries the cursor belongs to the
                    // start of the next visual line, not the end of this one.
                    let segment_char_count = segment.chars().count();
                    let is_last_segment = range_idx == num_ranges - 1;
                    let is_cursor_at_end = self.focused
                        && line_idx == self.cursor.0
                        && self.cursor.1 == chars_counted + segment_char_count
                        && is_last_segment
                        && x < area.x
                            + area
                                .width
                                .saturating_sub(self.padding.right + SCROLLBAR_WIDTH);

                    if is_cursor_at_end && let Some(cell) = buf.cell_mut((x, y)) {
                        cell.set_char(' ');
                        cell.set_style(cursor_style);
                    }
                }

                chars_counted += line[start..end].chars().count();
                visual_line += 1;
            }
        }

        // --- Scrollbar ---
        let scrollbar = Scrollbar::new(total_visual, visible_rows, self.scroll_offset);
        if scrollbar.is_visible() {
            let sb_x = area.x + area.width - self.padding.right - SCROLLBAR_WIDTH;
            let sb_area = Rect::new(sb_x, text_y, SCROLLBAR_WIDTH, visible_rows);
            scrollbar.render(sb_area, buf);
        }
    }
}

/// Convert a char offset to a byte index within a string.
fn char_to_byte_index(s: &str, char_idx: usize) -> usize {
    s.char_indices()
        .nth(char_idx)
        .map_or(s.len(), |(byte_idx, _)| byte_idx)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tf_with(text: &str) -> TextField {
        let mut tf = TextField::new(Theme::default());
        for c in text.chars() {
            if c == '\n' {
                tf.input(Input::Newline);
            } else {
                tf.input(Input::Char(c));
            }
        }
        tf
    }

    /// Build a TextField with the given text, zero padding, and a specific widget width
    /// so that `last_content_width` (and therefore `wrap_width()`) is set.
    fn tf_with_width(text: &str, widget_width: u16) -> TextField {
        let mut tf = tf_with(text);
        tf.set_padding(Padding::uniform(0));
        tf.required_height(widget_width);
        tf
    }

    #[test]
    fn new_text_field_is_empty() {
        let tf = TextField::new(Theme::default());
        assert_eq!(tf.content(), "");
    }

    #[test]
    fn insert_and_submit() {
        let mut tf = TextField::new(Theme::default());
        tf.input(Input::Char('h'));
        tf.input(Input::Char('i'));
        assert_eq!(tf.content(), "hi");

        let result = tf.input(Input::Enter);
        assert_eq!(result, TextFieldAction::Submit("hi".to_string()));
        assert_eq!(tf.content(), "");
    }

    #[test]
    fn newline_inserts_logical_line() {
        let mut tf = TextField::new(Theme::default());
        tf.input(Input::Char('a'));
        tf.input(Input::Newline);
        tf.input(Input::Char('b'));
        assert_eq!(tf.content(), "a\nb");
    }

    #[test]
    fn backspace_deletes_char() {
        let mut tf = TextField::new(Theme::default());
        tf.input(Input::Char('a'));
        tf.input(Input::Char('b'));
        tf.input(Input::Backspace);
        assert_eq!(tf.content(), "a");
    }

    #[test]
    fn backspace_joins_lines() {
        let mut tf = TextField::new(Theme::default());
        tf.input(Input::Char('a'));
        tf.input(Input::Newline);
        tf.input(Input::Backspace);
        assert_eq!(tf.content(), "a");
    }

    #[test]
    fn required_height_respects_min() {
        let mut tf = TextField::new(Theme::default());
        // min_height=3 + padding vertical=2 = 5
        assert_eq!(tf.required_height(80), 3 + 2);
    }

    #[test]
    fn required_height_grows_with_content() {
        let mut tf = TextField::new(Theme::default());
        for _ in 0..4 {
            tf.input(Input::Char('x'));
            tf.input(Input::Newline);
        }
        tf.input(Input::Char('x'));
        // 5 lines at width 80 → 5 content + 2 padding = 7
        assert_eq!(tf.required_height(80), 5 + 2);
    }

    #[test]
    fn required_height_capped_at_max() {
        let mut tf = TextField::new(Theme::default());
        // max_height = 8 by default
        for _ in 0..10 {
            tf.input(Input::Char('x'));
            tf.input(Input::Newline);
        }
        // 11 lines, capped to max=8, + padding=2 = 10
        assert_eq!(tf.required_height(80), 8 + 2);
    }

    #[test]
    fn empty_enter_is_ignored() {
        let mut tf = TextField::new(Theme::default());
        assert_eq!(tf.input(Input::Enter), TextFieldAction::Ignored);
    }

    #[test]
    fn clear_all_empties_buffer() {
        let mut tf = TextField::new(Theme::default());
        tf.input(Input::Char('h'));
        tf.input(Input::Char('e'));
        tf.input(Input::Char('l'));
        tf.input(Input::Char('l'));
        tf.input(Input::Char('o'));
        assert_eq!(tf.input(Input::ClearAll), TextFieldAction::Edited);
        assert_eq!(tf.content(), "");
        assert_eq!(tf.cursor, (0, 0));
    }

    #[test]
    fn clear_all_on_empty_is_ignored() {
        let mut tf = TextField::new(Theme::default());
        assert_eq!(tf.input(Input::ClearAll), TextFieldAction::Ignored);
    }

    #[test]
    fn clear_all_with_multiline() {
        let mut tf = TextField::new(Theme::default());
        tf.input(Input::Char('a'));
        tf.input(Input::Newline);
        tf.input(Input::Char('b'));
        assert_eq!(tf.content(), "a\nb");
        tf.input(Input::ClearAll);
        assert_eq!(tf.content(), "");
    }

    #[test]
    fn padding_affects_height() {
        let mut tf = TextField::new(Theme::default());
        tf.set_padding(Padding::symmetric(2, 3));
        // min_height=3 + top=3 + bottom=3 = 9
        assert_eq!(tf.required_height(80), 3 + 6);
    }

    #[test]
    fn scroll_keeps_cursor_visible() {
        let mut tf = TextField::new(Theme::default());
        tf.set_max_height(3);
        tf.set_padding(Padding::uniform(0));
        // Type 5 logical lines → exceeds max 3
        for _ in 0..4 {
            tf.input(Input::Char('x'));
            tf.input(Input::Newline);
        }
        tf.input(Input::Char('x'));
        // Cursor is on line 5 (index 4), viewport is 3 lines
        let h = tf.required_height(80);
        assert_eq!(h, 3); // capped
        assert!(tf.scroll_offset >= 2, "scroll_offset={}", tf.scroll_offset);
    }

    #[test]
    fn height_grows_before_cursor_hits_edge() {
        let mut tf = TextField::new(Theme::default());
        tf.set_padding(Padding::uniform(0));
        for c in "abcdefghi".chars() {
            tf.input(Input::Char(c));
        }
        assert_eq!(tf.required_height(12), 3);
        tf.input(Input::Char('j'));
        assert_eq!(tf.required_height(12), 3);
    }

    #[test]
    fn wrapping_with_cursor_reserve_at_exact_boundary() {
        let mut tf = TextField::new(Theme::default());
        tf.set_padding(Padding::uniform(0));
        for c in "abcd".chars() {
            tf.input(Input::Char(c));
        }
        tf.input(Input::Char('e'));
        assert_eq!(tf.required_height(7), 3);
        let wrap_w = tf.wrap_width();
        assert_eq!(wrap_w, 4);
        let lines = crate::widgets::word_wrap::count_wrapped_lines("abcde", wrap_w);
        assert_eq!(lines, 2);
    }

    #[test]
    fn up_down_move_between_logical_lines() {
        let mut tf = tf_with("aaa\nbbb\nccc");
        tf.set_padding(Padding::uniform(0));
        tf.required_height(80);
        tf.cursor = (2, 2);
        tf.input(Input::Up);
        assert_eq!(tf.cursor, (1, 2));
        tf.input(Input::Up);
        assert_eq!(tf.cursor, (0, 2));
        tf.input(Input::Down);
        assert_eq!(tf.cursor, (1, 2));
    }

    // ====================================================================
    // Regression tests for bugs that should FAIL with current broken code
    // ====================================================================

    // --- Bug: up/down ignores visual wrapped lines ---

    #[test]
    fn down_moves_across_visual_wrapped_lines() {
        // Widget width 6: border(1)+scrollbar(1)=2 overhead → content=4, wrap=3
        // "abcdef" → wrap_lines at width 3: ["abc", "def"]
        let mut tf = tf_with_width("abcdef", 6);
        assert_eq!(tf.wrap_width(), 3);
        tf.cursor = (0, 1); // visual line 0, display col 1
        tf.input(Input::Down);
        // Should move to visual line 1, display col 1 → char 3+1=4
        assert_eq!(tf.cursor, (0, 4));
    }

    #[test]
    fn up_moves_across_visual_wrapped_lines() {
        // "abcdef" at wrap_width 3 → ["abc", "def"]
        let mut tf = tf_with_width("abcdef", 6);
        assert_eq!(tf.wrap_width(), 3);
        // Cursor at end → (0, 6) on visual line 1
        assert_eq!(tf.cursor, (0, 6));
        tf.input(Input::Up);
        // Should move to visual line 0, display col 3 (clamped to segment end)
        assert_eq!(tf.cursor.0, 0);
        assert!(tf.cursor.1 <= 3, "cursor.1={}", tf.cursor.1);
    }

    // --- Bug: double cursor rendered at wrap boundary ---

    #[test]
    fn no_double_cursor_at_wrap_boundary() {
        // "abcdef" at wrap_width 3 → ["abc", "def"]
        // Place cursor at position 3 (start of "def" = wrap boundary)
        let mut tf = tf_with_width("abcdef", 6);
        tf.cursor = (0, 3);

        let area = Rect::new(0, 0, 6, 3);
        let mut buf = Buffer::empty(area);
        (&tf).render(area, &mut buf);

        let mut reversed_count = 0;
        for y in area.y..area.y + area.height {
            for x in area.x..area.x + area.width {
                if let Some(cell) = buf.cell((x, y)) {
                    if cell.style().add_modifier.contains(Modifier::REVERSED) {
                        reversed_count += 1;
                    }
                }
            }
        }
        assert_eq!(
            reversed_count, 1,
            "cursor should appear exactly once, got {}",
            reversed_count
        );
    }
}
