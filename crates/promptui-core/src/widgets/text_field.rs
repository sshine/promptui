use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::{Color, Modifier, Style};
use ratatui::widgets::Widget;
use unicode_width::UnicodeWidthChar;

use crate::input::Input;
use crate::theme::Theme;
use crate::widgets::scrollbar::Scrollbar;
use crate::widgets::word;
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

    /// Visual line index where the cursor sits.
    fn cursor_visual_line(&self, wrap_width: u16) -> u16 {
        let mut visual_row: u16 = 0;
        for (i, line) in self.lines.iter().enumerate() {
            if i == self.cursor.0 {
                // Find which visual line within this logical line
                let ranges = wrap_lines(line, wrap_width);
                let mut chars_counted: usize = 0;
                for (range_idx, &(start, end)) in ranges.iter().enumerate() {
                    let seg_chars = line[start..end].chars().count();
                    if self.cursor.1 < chars_counted + seg_chars || range_idx == ranges.len() - 1 {
                        return visual_row + range_idx as u16;
                    }
                    chars_counted += seg_chars;
                }
                return visual_row;
            }
            visual_row = visual_row.saturating_add(count_wrapped_lines(line, wrap_width));
        }
        visual_row
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
            // --- Text insertion ---
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

            // --- Character deletion ---
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

            // --- Word deletion ---
            Input::DeleteWordBackward => {
                if self.delete_word_backward() {
                    TextFieldAction::Edited
                } else {
                    TextFieldAction::Ignored
                }
            }
            Input::DeleteWordForward => {
                if self.delete_word_forward() {
                    TextFieldAction::Edited
                } else {
                    TextFieldAction::Ignored
                }
            }

            // --- Line deletion ---
            Input::KillToEnd => {
                if self.kill_to_end() {
                    TextFieldAction::Edited
                } else {
                    TextFieldAction::Ignored
                }
            }
            Input::KillToStart => {
                if self.kill_to_start() {
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

            // --- Cursor movement ---
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
            Input::WordForward => {
                self.move_word_forward();
                TextFieldAction::Edited
            }
            Input::WordBackward => {
                self.move_word_backward();
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

    // --- Word deletion ---

    fn delete_word_backward(&mut self) -> bool {
        let (row, col) = self.cursor;
        if col == 0 {
            // At line start: join with previous line (same as backspace)
            return self.delete_backward();
        }
        if let Some(line) = self.lines.get_mut(row) {
            let new_col = word::find_word_start_backward(line, col);
            let start_byte = char_to_byte_index(line, new_col);
            let end_byte = char_to_byte_index(line, col);
            line.drain(start_byte..end_byte);
            self.cursor.1 = new_col;
            return true;
        }
        false
    }

    fn delete_word_forward(&mut self) -> bool {
        let (row, col) = self.cursor;
        let line_len = self.current_line_char_len();
        if col >= line_len {
            // At line end: join with next line (same as delete)
            return self.delete_forward();
        }
        if let Some(line) = self.lines.get_mut(row) {
            let end_col = word::find_word_end_forward(line, col);
            let start_byte = char_to_byte_index(line, col);
            let end_byte = char_to_byte_index(line, end_col);
            line.drain(start_byte..end_byte);
            return true;
        }
        false
    }

    // --- Line deletion ---

    /// Delete from cursor to end of line (Ctrl+K).
    fn kill_to_end(&mut self) -> bool {
        let (row, col) = self.cursor;
        let line_len = self.current_line_char_len();
        if col < line_len {
            // Kill text after cursor on this line
            if let Some(line) = self.lines.get_mut(row) {
                let byte_idx = char_to_byte_index(line, col);
                line.truncate(byte_idx);
                return true;
            }
        } else if row + 1 < self.lines.len() {
            // At end of line: join with next line
            let next = self.lines.remove(row + 1);
            if let Some(current) = self.lines.get_mut(row) {
                current.push_str(&next);
                return true;
            }
        }
        false
    }

    /// Delete from cursor to start of line (Ctrl+J).
    fn kill_to_start(&mut self) -> bool {
        let (row, col) = self.cursor;
        if col > 0 {
            if let Some(line) = self.lines.get_mut(row) {
                let byte_idx = char_to_byte_index(line, col);
                line.drain(..byte_idx);
                self.cursor.1 = 0;
                return true;
            }
        } else if row > 0 {
            // At start of line: join with previous line
            return self.delete_backward();
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
        let (row, _col) = self.cursor;
        if row > 0 {
            self.cursor.0 -= 1;
            let max_col = self
                .lines
                .get(self.cursor.0)
                .map_or(0, |l| l.chars().count());
            self.cursor.1 = self.cursor.1.min(max_col);
        }
    }

    fn move_down(&mut self) {
        let (row, _col) = self.cursor;
        if row + 1 < self.lines.len() {
            self.cursor.0 += 1;
            let max_col = self
                .lines
                .get(self.cursor.0)
                .map_or(0, |l| l.chars().count());
            self.cursor.1 = self.cursor.1.min(max_col);
        }
    }

    fn move_word_forward(&mut self) {
        let (row, col) = self.cursor;
        let line_len = self.current_line_char_len();
        if col >= line_len {
            // At end of line: move to start of next line
            if row + 1 < self.lines.len() {
                self.cursor.0 += 1;
                self.cursor.1 = 0;
            }
            return;
        }
        if let Some(line) = self.lines.get(row) {
            self.cursor.1 = word::find_word_end_forward(line, col);
        }
    }

    fn move_word_backward(&mut self) {
        let (row, col) = self.cursor;
        if col == 0 {
            // At start of line: move to end of previous line
            if row > 0 {
                self.cursor.0 -= 1;
                self.cursor.1 = self
                    .lines
                    .get(self.cursor.0)
                    .map_or(0, |l| l.chars().count());
            }
            return;
        }
        if let Some(line) = self.lines.get(row) {
            self.cursor.1 = word::find_word_start_backward(line, col);
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

            let mut chars_counted: usize = 0;
            for &(start, end) in &ranges {
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

                    // Draw cursor at end of segment if needed
                    let segment_char_count = segment.chars().count();
                    let is_cursor_at_end = self.focused
                        && line_idx == self.cursor.0
                        && self.cursor.1 == chars_counted + segment_char_count
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
        // scroll_offset should be adjusted so cursor (visual line 4) is visible
        // viewport: [scroll_offset .. scroll_offset + 3)
        // cursor at visual line 4 → scroll_offset >= 2
        assert!(tf.scroll_offset >= 2, "scroll_offset={}", tf.scroll_offset);
    }

    // --- Cursor-aware wrapping regression tests ---

    #[test]
    fn height_grows_before_cursor_hits_edge() {
        let mut tf = TextField::new(Theme::default());
        tf.set_padding(Padding::uniform(0));
        // Widget width 12: border(1) + scrollbar(1) = 2 overhead
        // → content_width=10, wrap_width=9
        for c in "abcdefghi".chars() {
            tf.input(Input::Char(c));
        }
        // 9 chars in wrap_width=9 → fits in 1 line → min 3
        assert_eq!(tf.required_height(12), 3);

        tf.input(Input::Char('j'));
        // 10 chars in wrap_width=9 → 2 lines → still min 3
        assert_eq!(tf.required_height(12), 3);
    }

    #[test]
    fn wrapping_with_cursor_reserve_at_exact_boundary() {
        let mut tf = TextField::new(Theme::default());
        tf.set_padding(Padding::uniform(0));
        // Widget width 7: border(1) + scrollbar(1) = 2 overhead
        // → content_width=5, wrap_width=4
        for c in "abcd".chars() {
            tf.input(Input::Char(c));
        }
        tf.input(Input::Char('e'));
        // "abcde" in wrap_width 4 → 2 visual lines
        assert_eq!(tf.required_height(7), 3); // min

        let wrap_w = tf.wrap_width();
        assert_eq!(wrap_w, 4);
        let lines = crate::widgets::word_wrap::count_wrapped_lines("abcde", wrap_w);
        assert_eq!(lines, 2);
    }

    // --- Word operations ---

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

    #[test]
    fn delete_word_backward_simple() {
        let mut tf = tf_with("hello world");
        // Cursor at end (0, 11)
        tf.input(Input::DeleteWordBackward);
        assert_eq!(tf.content(), "hello ");
        assert_eq!(tf.cursor.1, 6);
    }

    #[test]
    fn delete_word_backward_at_start() {
        let mut tf = tf_with("hello");
        tf.cursor.1 = 0;
        // At start of only line: nothing to delete
        assert_eq!(
            tf.input(Input::DeleteWordBackward),
            TextFieldAction::Ignored
        );
    }

    #[test]
    fn delete_word_forward_simple() {
        let mut tf = tf_with("hello world");
        tf.cursor.1 = 0;
        tf.input(Input::DeleteWordForward);
        assert_eq!(tf.content(), "world");
    }

    #[test]
    fn delete_word_forward_at_end() {
        let mut tf = tf_with("hello");
        // At end: nothing to delete (no next line)
        assert_eq!(tf.input(Input::DeleteWordForward), TextFieldAction::Ignored);
    }

    #[test]
    fn kill_to_end_of_line() {
        let mut tf = tf_with("hello world");
        tf.cursor.1 = 5; // after "hello"
        tf.input(Input::KillToEnd);
        assert_eq!(tf.content(), "hello");
        assert_eq!(tf.cursor.1, 5);
    }

    #[test]
    fn kill_to_end_joins_next_line() {
        let mut tf = tf_with("hello\nworld");
        tf.cursor = (0, 5); // end of first line
        tf.input(Input::KillToEnd);
        assert_eq!(tf.content(), "helloworld");
    }

    #[test]
    fn kill_to_start_of_line() {
        let mut tf = tf_with("hello world");
        tf.cursor.1 = 6; // before "world"
        tf.input(Input::KillToStart);
        assert_eq!(tf.content(), "world");
        assert_eq!(tf.cursor.1, 0);
    }

    #[test]
    fn kill_to_start_joins_prev_line() {
        let mut tf = tf_with("hello\nworld");
        tf.cursor = (1, 0); // start of second line
        tf.input(Input::KillToStart);
        assert_eq!(tf.content(), "helloworld");
    }

    #[test]
    fn word_forward_navigation() {
        let mut tf = tf_with("hello world foo");
        tf.cursor.1 = 0;
        tf.input(Input::WordForward);
        assert_eq!(tf.cursor.1, 6); // after "hello "
        tf.input(Input::WordForward);
        assert_eq!(tf.cursor.1, 12); // after "world "
        tf.input(Input::WordForward);
        assert_eq!(tf.cursor.1, 15); // end
    }

    #[test]
    fn word_backward_navigation() {
        let mut tf = tf_with("hello world foo");
        // cursor at end (15)
        tf.input(Input::WordBackward);
        assert_eq!(tf.cursor.1, 12); // start of "foo"
        tf.input(Input::WordBackward);
        assert_eq!(tf.cursor.1, 6); // start of "world"
        tf.input(Input::WordBackward);
        assert_eq!(tf.cursor.1, 0); // start
    }

    #[test]
    fn word_forward_crosses_lines() {
        let mut tf = tf_with("hello\nworld");
        tf.cursor = (0, 5); // end of first line
        tf.input(Input::WordForward);
        assert_eq!(tf.cursor, (1, 0)); // start of next line
    }

    #[test]
    fn word_backward_crosses_lines() {
        let mut tf = tf_with("hello\nworld");
        tf.cursor = (1, 0); // start of second line
        tf.input(Input::WordBackward);
        assert_eq!(tf.cursor, (0, 5)); // end of first line
    }

    #[test]
    fn up_down_move_between_logical_lines() {
        let mut tf = tf_with("aaa\nbbb\nccc");
        tf.cursor = (2, 2);
        tf.input(Input::Up);
        assert_eq!(tf.cursor, (1, 2));
        tf.input(Input::Up);
        assert_eq!(tf.cursor, (0, 2));
        tf.input(Input::Down);
        assert_eq!(tf.cursor, (1, 2));
    }
}
