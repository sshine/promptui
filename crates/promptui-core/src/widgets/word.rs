/// Character classification for word boundary detection.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum CharKind {
    Space,
    Punct,
    Other,
}

fn char_kind(c: char) -> CharKind {
    if c.is_whitespace() {
        CharKind::Space
    } else if c.is_ascii_punctuation() {
        CharKind::Punct
    } else {
        CharKind::Other
    }
}

/// Find the char offset of the start of the previous word from `col` in `line`.
/// Returns 0 if already at the beginning.
pub fn find_word_start_backward(line: &str, col: usize) -> usize {
    let chars: Vec<char> = line.chars().collect();
    if col == 0 {
        return 0;
    }
    let mut pos = col.min(chars.len());

    // Skip whitespace backward
    while pos > 0 && char_kind(chars[pos - 1]) == CharKind::Space {
        pos -= 1;
    }
    if pos == 0 {
        return 0;
    }

    // Skip the word backward (same kind)
    let kind = char_kind(chars[pos - 1]);
    while pos > 0 && char_kind(chars[pos - 1]) == kind {
        pos -= 1;
    }

    pos
}

/// Find the char offset of the end of the next word from `col` in `line`.
/// Returns the line length if already at the end.
pub fn find_word_end_forward(line: &str, col: usize) -> usize {
    let chars: Vec<char> = line.chars().collect();
    let len = chars.len();
    if col >= len {
        return len;
    }
    let mut pos = col;

    // Skip current word (same kind)
    let kind = char_kind(chars[pos]);
    while pos < len && char_kind(chars[pos]) == kind {
        pos += 1;
    }

    // Skip whitespace forward
    while pos < len && char_kind(chars[pos]) == CharKind::Space {
        pos += 1;
    }

    pos
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn word_back_from_end() {
        assert_eq!(find_word_start_backward("hello world", 11), 6);
    }

    #[test]
    fn word_back_from_middle_of_word() {
        assert_eq!(find_word_start_backward("hello world", 8), 6);
    }

    #[test]
    fn word_back_at_start() {
        assert_eq!(find_word_start_backward("hello world", 0), 0);
    }

    #[test]
    fn word_back_from_space() {
        assert_eq!(find_word_start_backward("hello world", 6), 0);
    }

    #[test]
    fn word_back_with_punct() {
        // "foo.bar" → from end of "bar" (7) should stop at "." boundary
        assert_eq!(find_word_start_backward("foo.bar", 7), 4);
    }

    #[test]
    fn word_forward_from_start() {
        assert_eq!(find_word_end_forward("hello world", 0), 6);
    }

    #[test]
    fn word_forward_from_space() {
        assert_eq!(find_word_end_forward("hello world", 5), 6);
    }

    #[test]
    fn word_forward_at_end() {
        assert_eq!(find_word_end_forward("hello world", 11), 11);
    }

    #[test]
    fn word_forward_with_punct() {
        assert_eq!(find_word_end_forward("foo.bar", 0), 3);
    }

    #[test]
    fn word_back_multiple_spaces() {
        assert_eq!(find_word_start_backward("hello   world", 13), 8);
    }

    #[test]
    fn word_forward_multiple_spaces() {
        assert_eq!(find_word_end_forward("hello   world", 0), 8);
    }
}
