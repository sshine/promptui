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
    let mut last_break_col: Option<usize> = None;

    for ch in text.chars() {
        let ch_width = ch.width().unwrap_or(0);

        if col + ch_width > width && col > 0 {
            // Need to wrap
            lines = lines.saturating_add(1);
            if let Some(brk_col) = last_break_col {
                // Break at word boundary: new line starts with the current word
                col -= brk_col;
                last_break_col = None;
            } else {
                // No word boundary: hard break at this character
                col = 0;
            }
        }

        // Always add the current character to the line
        col += ch_width;

        if ch.is_whitespace() {
            last_break_col = Some(col);
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
    let mut last_break_byte: Option<usize> = None;
    let mut last_break_col: Option<usize> = None;

    for (byte_idx, ch) in text.char_indices() {
        let ch_width = ch.width().unwrap_or(0);

        if col + ch_width > width && col > 0 {
            // Need to wrap
            if let Some(brk_byte) = last_break_byte {
                let brk_col = last_break_col.unwrap_or(0);
                result.push((line_start, brk_byte));
                line_start = brk_byte;
                col -= brk_col;
                last_break_byte = None;
                last_break_col = None;
            } else {
                result.push((line_start, byte_idx));
                line_start = byte_idx;
                col = 0;
            }
        }

        // Always add the current character
        col += ch_width;

        if ch.is_whitespace() {
            last_break_byte = Some(byte_idx + ch.len_utf8());
            last_break_col = Some(col);
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

    // --- count_wrapped_lines ---

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
    fn exact_width_no_wrap() {
        assert_eq!(count_wrapped_lines("abcde", 5), 1);
    }

    #[test]
    fn one_over_exact_width_wraps() {
        assert_eq!(count_wrapped_lines("abcdef", 5), 2);
    }

    #[test]
    fn count_and_wrap_agree() {
        // Regression: count_wrapped_lines and wrap_lines must always agree.
        let cases = [
            ("hello world", 7),
            ("aaa bbb ccc ddd", 9),
            ("abcdefghijklmno", 5),
            ("a b c d e f g h", 3),
            ("the quick brown fox jumps", 10),
            ("xxxx yyyy zzzz", 6),
        ];
        for (text, w) in cases {
            let count = count_wrapped_lines(text, w);
            let lines = wrap_lines(text, w);
            assert_eq!(
                count as usize,
                lines.len(),
                "mismatch for {:?} at width {}: count={}, wrap_lines={}",
                text,
                w,
                count,
                lines.len()
            );
        }
    }

    #[test]
    fn three_line_wrap_count() {
        // "aaa bbb ccc ddd" in width 9:
        // Line 1: "aaa bbb " (8 cols)
        // Line 2: "ccc ddd" (7 cols)
        // = 2 lines
        assert_eq!(count_wrapped_lines("aaa bbb ccc ddd", 9), 2);
    }

    #[test]
    fn cascading_wraps_correct_count() {
        // Regression: after a word-break, the new line's column tracking must be
        // accurate so subsequent breaks happen at the right position.
        // "aaa bbb ccc ddd eee" in width 8:
        // Line 1: "aaa bbb " (8)
        // Line 2: "ccc ddd " (8)
        // Line 3: "eee" (3)
        // = 3 lines
        assert_eq!(count_wrapped_lines("aaa bbb ccc ddd eee", 8), 3);
    }

    #[test]
    fn four_wraps_from_continuous_typing() {
        // Simulates typing into a width-9 field with cursor reserve already applied.
        // Each word is 4 chars + space = 5.
        // "aaaa bbbb cccc dddd eeee" (24 chars)
        // Width 9:
        // Line 1: "aaaa " (5) + "bbbb" would be 9, but "bbbb " is 10 > 9
        //   Actually: "aaaa bbbb" is 9 chars/cols, fits exactly.
        //   Then ' ' would make it 10 > 9? No, "aaaa bbbb " is 10.
        //   Wait: "aaaa " = 5 cols. "aaaa b" = 6. ... "aaaa bbbb" = 9. Fits.
        //   "aaaa bbbb " = 10 > 9 → break. Line 1: "aaaa bbbb " but break at space.
        //   Hmm, the space is at col 5 and col 10. Break at col 10? No, col 10 > width.
        //   When we reach the space after "bbbb": col would be 10 > 9, so we break.
        //   last_break is at col 5 (after "aaaa "). New line starts with "bbbb".
        //   col = 10 - 5 = 5. Then add space: col = 5 + 1 = 6.
        // That gets complex. Let's just verify the count.
        let text = "aaaa bbbb cccc dddd eeee";
        let count = count_wrapped_lines(text, 9);
        let lines = wrap_line_strs(text, 9);
        assert_eq!(count as usize, lines.len());
        // Verify the actual splits make visual sense
        for line in &lines {
            let display_width: usize = line.chars().map(|c| c.width().unwrap_or(0)).sum();
            assert!(
                display_width <= 9,
                "line {:?} has display width {} > 9",
                line,
                display_width
            );
        }
    }

    // --- wrap_lines / wrap_line_strs ---

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

    #[test]
    fn wrap_lines_no_line_exceeds_width() {
        let cases = [
            ("the quick brown fox jumps over the lazy dog", 10),
            ("abcdef ghijkl mnopqr stuvwx", 8),
            ("a b c d e f g h i j k l m", 3),
        ];
        for (text, w) in cases {
            let lines = wrap_line_strs(text, w);
            for line in &lines {
                let display_width: usize = line.chars().map(|c| c.width().unwrap_or(0)).sum();
                assert!(
                    display_width <= w as usize,
                    "at width {w}, line {line:?} has display width {display_width}"
                );
            }
        }
    }

    #[test]
    fn wrap_lines_concatenation_equals_original() {
        let cases = [
            ("hello world foo bar", 7),
            ("aaa bbb ccc", 5),
            ("abcdefghij", 3),
            ("x y z", 2),
        ];
        for (text, w) in cases {
            let lines = wrap_line_strs(text, w);
            let joined: String = lines.concat();
            assert_eq!(
                joined, text,
                "concatenated wrap lines don't match original for {:?} at width {}",
                text, w
            );
        }
    }

    #[test]
    fn wrap_cascading_word_breaks() {
        // Regression: after a word break, column tracking must be correct.
        let lines = wrap_line_strs("aaa bbb ccc ddd eee", 8);
        assert_eq!(lines, vec!["aaa bbb ", "ccc ddd ", "eee"]);
    }
}
