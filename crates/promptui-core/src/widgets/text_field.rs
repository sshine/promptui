use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::Modifier;
use ratatui::widgets::Widget;
use unicode_width::UnicodeWidthChar;

use crate::input::Input;
use crate::theme::Theme;
use crate::widgets::word_wrap::{count_wrapped_lines, wrap_lines};

/// Left border character matching OpenCode's style.
const LEFT_BORDER: &str = "\u{2503}"; // ┃

/// Left border width (1) + padding (1).
const LEFT_INDENT: u16 = 2;

/// Reserve 1 column for the cursor so wrapping treats the cursor like a character.
const CURSOR_RESERVE: u16 = 1;

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
    cursor: (usize, usize), // (logical_row, col_byte_offset... no, char offset)
    theme: Theme,
    min_height: u16,
    max_height: u16,
    focused: bool,
    last_width: u16,
}

impl TextField {
    pub fn new(theme: Theme) -> Self {
        Self {
            lines: vec![String::new()],
            cursor: (0, 0),
            theme,
            min_height: 3,
            max_height: 12,
            focused: true,
            last_width: 0,
        }
    }

    pub fn set_focused(&mut self, focused: bool) {
        self.focused = focused;
    }

    /// Returns the text content, joining logical lines with newlines.
    pub fn content(&self) -> String {
        self.lines.join("\n")
    }

    /// Clears the text field and resets the cursor.
    pub fn clear(&mut self) {
        self.lines = vec![String::new()];
        self.cursor = (0, 0);
    }

    /// Computes the required visual height for the current content at the given width.
    /// The width should be the total available width for the widget (border+padding will be subtracted).
    /// Caches the content width for use in cursor navigation.
    pub fn required_height(&mut self, width: u16) -> u16 {
        let content_width = width.saturating_sub(LEFT_INDENT);
        self.last_width = content_width;

        // Wrap width reserves 1 column for the cursor so the cursor
        // behaves like a character when deciding where to break lines.
        let wrap_width = content_width.saturating_sub(CURSOR_RESERVE);
        if wrap_width == 0 {
            return self.min_height;
        }

        let mut total_visual: u16 = 0;
        for line in &self.lines {
            total_visual = total_visual.saturating_add(count_wrapped_lines(line, wrap_width));
        }

        total_visual.clamp(self.min_height, self.max_height)
    }

    /// The effective width used for word wrapping (content width minus cursor reserve).
    fn wrap_width(&self) -> u16 {
        self.last_width.saturating_sub(CURSOR_RESERVE)
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

        // Count visual lines before the cursor's logical row
        let mut visual_row: u16 = 0;
        for (i, line) in self.lines.iter().enumerate() {
            if i == self.cursor.0 {
                break;
            }
            visual_row = visual_row.saturating_add(count_wrapped_lines(line, wrap_w));
        }

        // Find cursor position within the current logical line's wrapped output
        let current_line = self.lines.get(self.cursor.0).map_or("", |s| s.as_str());
        let cursor_col = self.cursor.1;
        let ranges = wrap_lines(current_line, wrap_w);

        let mut chars_counted: usize = 0;
        for (range_idx, &(start, end)) in ranges.iter().enumerate() {
            let segment = &current_line[start..end];
            let segment_chars: usize = segment.chars().count();

            if cursor_col < chars_counted + segment_chars || range_idx == ranges.len() - 1 {
                let col_in_segment = cursor_col.saturating_sub(chars_counted);
                let display_col: u16 = segment
                    .chars()
                    .take(col_in_segment)
                    .map(|c| c.width().unwrap_or(0) as u16)
                    .sum();

                let x = area.x + LEFT_INDENT + display_col;
                let y = area.y + visual_row + range_idx as u16;

                if y < area.y + area.height {
                    return Some((x, y));
                }
                return None;
            }

            chars_counted += segment_chars;
        }

        Some((area.x + LEFT_INDENT, area.y + visual_row))
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
                let len = self.current_line_char_len();
                self.cursor.1 = len;
                TextFieldAction::Edited
            }
            Input::ClearAll => {
                if self.lines.len() == 1 && self.lines[0].is_empty() {
                    return TextFieldAction::Ignored;
                }
                self.clear();
                TextFieldAction::Edited
            }
            _ => TextFieldAction::Ignored,
        }
    }

    // --- Editing operations ---

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
            // Join with previous line
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
            // Join with next line
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
        for y in area.y..area.y + area.height {
            for x in area.x..area.x + area.width {
                if let Some(cell) = buf.cell_mut((x, y)) {
                    cell.set_style(self.theme.input_style());
                    cell.set_char(' ');
                }
            }
        }

        // Draw left border
        let border_style = if self.focused {
            self.theme.border_focused_style()
        } else {
            self.theme.border_unfocused_style()
        };
        for y in area.y..area.y + area.height {
            if let Some(cell) = buf.cell_mut((area.x, y)) {
                cell.set_style(border_style);
                cell.set_symbol(LEFT_BORDER);
            }
        }

        // Render wrapped text
        let text_style = self.theme.input_style();
        let cursor_style = text_style.add_modifier(Modifier::REVERSED);
        let text_x = area.x + LEFT_INDENT;

        let mut visual_y: u16 = 0;

        for (line_idx, line) in self.lines.iter().enumerate() {
            let ranges = wrap_lines(line, wrap_w);

            let mut chars_counted: usize = 0;
            for &(start, end) in &ranges {
                if visual_y >= area.height {
                    break;
                }

                let segment = &line[start..end];
                let y = area.y + visual_y;
                let mut x = text_x;

                for (char_idx_in_line, ch) in segment.chars().enumerate() {
                    let absolute_char_idx = chars_counted + char_idx_in_line;
                    let ch_width = ch.width().unwrap_or(0) as u16;

                    if x + ch_width > area.x + area.width {
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

                // Draw cursor at end of line if needed
                let segment_char_count = segment.chars().count();
                let is_cursor_at_end = self.focused
                    && line_idx == self.cursor.0
                    && self.cursor.1 == chars_counted + segment_char_count
                    && x < area.x + area.width;

                if is_cursor_at_end && let Some(cell) = buf.cell_mut((x, y)) {
                    cell.set_char(' ');
                    cell.set_style(cursor_style);
                }

                chars_counted += segment_char_count;
                visual_y += 1;
            }
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
        assert_eq!(tf.required_height(80), 3); // min_height = 3
    }

    #[test]
    fn required_height_grows_with_content() {
        let mut tf = TextField::new(Theme::default());
        // Add 5 logical lines
        for _ in 0..4 {
            tf.input(Input::Char('x'));
            tf.input(Input::Newline);
        }
        tf.input(Input::Char('x'));
        // 5 lines, each 1 visual line at width 80 → height = max(5, min_height=3)
        assert_eq!(tf.required_height(80), 5);
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

    // --- Cursor-aware wrapping regression tests ---

    #[test]
    fn height_grows_before_cursor_hits_edge() {
        // With total widget width 12, LEFT_INDENT=2, CURSOR_RESERVE=1,
        // the effective wrap width is 12-2-1=9.
        // Typing 9 characters should cause wrapping to a second line,
        // so required_height should grow beyond min_height=3 when we have
        // enough lines. But more importantly, 9 chars should wrap to 2 visual lines.
        let mut tf = TextField::new(Theme::default());
        // Type exactly 9 chars (fills wrap_width=9 exactly)
        for c in "abcdefghi".chars() {
            tf.input(Input::Char(c));
        }
        // At width 12: content_width=10, wrap_width=9
        // "abcdefghi" is 9 chars in 9 cols → fits in 1 line
        assert_eq!(tf.required_height(12), 3); // still min_height

        // One more char → 10 chars in 9 cols → wraps to 2 lines
        tf.input(Input::Char('j'));
        assert_eq!(tf.required_height(12), 3); // 2 visual lines, but min is 3

        // Now make it actually exceed min_height
        // With wrap_width=9, we need >3 visual lines → >27 chars
        let mut tf2 = TextField::new(Theme::default());
        for c in "aaa bbb ccc ddd eee fff ggg".chars() {
            tf2.input(Input::Char(c));
        }
        // 27 chars. At wrap_width=9: "aaa bbb " (8) | "ccc ddd " (8) | "eee fff " (8) | "ggg" (3) → 4 lines
        let h = tf2.required_height(12);
        assert!(h >= 4, "expected height >= 4, got {h}");
    }

    #[test]
    fn wrapping_with_cursor_reserve_at_exact_boundary() {
        // Widget width 7, LEFT_INDENT=2, CURSOR_RESERVE=1 → wrap_width=4
        let mut tf = TextField::new(Theme::default());
        // Type "abcd" (exactly 4 chars = wrap_width)
        for c in "abcd".chars() {
            tf.input(Input::Char(c));
        }
        // Should be 1 visual line: "abcd" fits in 4 cols
        // Adding one more char should wrap
        tf.input(Input::Char('e'));
        // "abcde" in width 4 → 2 visual lines
        // required_height returns max(2, min_height=3) = 3
        assert_eq!(tf.required_height(7), 3);

        // Verify the wrapping actually happened by checking content renders on 2 visual lines
        let wrap_w = tf.wrap_width();
        assert_eq!(wrap_w, 4);
        let lines = crate::widgets::word_wrap::count_wrapped_lines("abcde", wrap_w);
        assert_eq!(lines, 2);
    }
}
