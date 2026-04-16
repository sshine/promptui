use unicode_width::UnicodeWidthChar;

/// Count the number of visual lines needed to display `text` within `width` columns.
///
/// Uses word-boundary wrapping: lines break at whitespace when possible,
/// falling back to character-level breaks for words wider than the available width.
/// Returns at least 1 for any input (including empty strings).
pub fn count_wrapped_lines(text: &str, width: u16) -> u16 {
    if width == 0 {
        return 1;
    }
    let width = width as usize;
    let mut lines: u16 = 1;
    let mut col: usize = 0;
    let mut last_break: Option<usize> = None;
    let mut since_break: usize = 0;

    for ch in text.chars() {
        let ch_width = ch.width().unwrap_or(0);

        if col + ch_width > width {
            lines = lines.saturating_add(1);
            if last_break.is_some() {
                col = since_break;
                last_break = None;
                since_break = 0;
            } else {
                col = ch_width;
            }
        } else {
            col += ch_width;
        }

        if ch.is_whitespace() {
            last_break = Some(col);
            since_break = 0;
        } else {
            since_break += ch_width;
        }
    }

    lines
}

/// Break `text` into visual lines that fit within `width` columns.
///
/// Uses the same word-boundary wrapping algorithm as [`count_wrapped_lines`].
/// Returns byte-offset ranges into `text` for each visual line.
pub fn wrap_lines(text: &str, width: u16) -> Vec<(usize, usize)> {
    if width == 0 || text.is_empty() {
        return vec![(0, text.len())];
    }
    let width = width as usize;
    let mut result = Vec::new();
    let mut line_start: usize = 0;
    let mut col: usize = 0;
    // Byte offset of the position right after the last whitespace character
    let mut last_break_byte: Option<usize> = None;
    let mut col_at_break: usize = 0;

    for (byte_idx, ch) in text.char_indices() {
        let ch_width = ch.width().unwrap_or(0);

        if col + ch_width > width && col > 0 {
            if let Some(brk) = last_break_byte {
                result.push((line_start, brk));
                line_start = brk;
                col = col - col_at_break + ch_width;
                last_break_byte = None;
                col_at_break = 0;
            } else {
                result.push((line_start, byte_idx));
                line_start = byte_idx;
                col = ch_width;
            }
        } else {
            col += ch_width;
        }

        if ch.is_whitespace() {
            last_break_byte = Some(byte_idx + ch.len_utf8());
            col_at_break = col;
        }
    }

    result.push((line_start, text.len()));
    result
}

/// Convenience: get the visual line slices from a string.
pub fn wrap_line_strs(text: &str, width: u16) -> Vec<&str> {
    wrap_lines(text, width)
        .into_iter()
        .map(|(start, end)| &text[start..end])
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn empty_string_is_one_line() {
        assert_eq!(count_wrapped_lines("", 10), 1);
    }

    #[test]
    fn short_string_is_one_line() {
        assert_eq!(count_wrapped_lines("hello", 10), 1);
    }

    #[test]
    fn wraps_at_word_boundary() {
        // "hello world" in width 7: "hello " (6) then "world" (5)
        assert_eq!(count_wrapped_lines("hello world", 7), 2);
    }

    #[test]
    fn long_word_breaks_at_width() {
        assert_eq!(count_wrapped_lines("abcdefghij", 5), 2);
    }

    #[test]
    fn multiple_wraps() {
        assert_eq!(count_wrapped_lines("aa bb cc dd", 5), 3);
    }

    #[test]
    fn zero_width_returns_one() {
        assert_eq!(count_wrapped_lines("hello", 0), 1);
    }

    #[test]
    fn wrap_lines_basic() {
        let lines = wrap_line_strs("hello world", 7);
        assert_eq!(lines, vec!["hello ", "world"]);
    }

    #[test]
    fn wrap_lines_no_break_needed() {
        let lines = wrap_line_strs("hello", 10);
        assert_eq!(lines, vec!["hello"]);
    }

    #[test]
    fn wrap_lines_long_word() {
        let lines = wrap_line_strs("abcdefghij", 5);
        assert_eq!(lines, vec!["abcde", "fghij"]);
    }

    #[test]
    fn wrap_lines_multiple() {
        let lines = wrap_line_strs("aa bb cc dd", 5);
        assert_eq!(lines, vec!["aa ", "bb ", "cc dd"]);
    }

    #[test]
    fn wrap_lines_empty() {
        let lines = wrap_line_strs("", 10);
        assert_eq!(lines, vec![""]);
    }
}
