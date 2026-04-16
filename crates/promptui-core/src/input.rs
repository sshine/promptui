use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

/// Abstraction over terminal key events.
///
/// Converts crossterm key events into a simpler enum that widgets can
/// pattern-match against. Shift+Enter and Alt+Enter both map to `Newline`,
/// while bare Enter maps to `Enter`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Input {
    /// A printable character.
    Char(char),
    /// Bare Enter key (submit action).
    Enter,
    /// Shift+Enter or Alt+Enter (insert literal newline).
    Newline,
    Backspace,
    Delete,
    Left,
    Right,
    Up,
    Down,
    Home,
    End,
    PageUp,
    PageDown,
    Esc,
    /// An unrecognized key event.
    Unknown,
}

impl From<KeyEvent> for Input {
    fn from(key: KeyEvent) -> Self {
        let has_shift = key.modifiers.contains(KeyModifiers::SHIFT);
        let has_alt = key.modifiers.contains(KeyModifiers::ALT);

        match key.code {
            KeyCode::Enter if has_shift || has_alt => Input::Newline,
            KeyCode::Enter => Input::Enter,
            KeyCode::Char(c) if !has_alt => Input::Char(c),
            KeyCode::Backspace => Input::Backspace,
            KeyCode::Delete => Input::Delete,
            KeyCode::Left => Input::Left,
            KeyCode::Right => Input::Right,
            KeyCode::Up => Input::Up,
            KeyCode::Down => Input::Down,
            KeyCode::Home => Input::Home,
            KeyCode::End => Input::End,
            KeyCode::PageUp => Input::PageUp,
            KeyCode::PageDown => Input::PageDown,
            KeyCode::Esc => Input::Esc,
            _ => Input::Unknown,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bare_enter_maps_to_enter() {
        let key = KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE);
        assert_eq!(Input::from(key), Input::Enter);
    }

    #[test]
    fn shift_enter_maps_to_newline() {
        let key = KeyEvent::new(KeyCode::Enter, KeyModifiers::SHIFT);
        assert_eq!(Input::from(key), Input::Newline);
    }

    #[test]
    fn alt_enter_maps_to_newline() {
        let key = KeyEvent::new(KeyCode::Enter, KeyModifiers::ALT);
        assert_eq!(Input::from(key), Input::Newline);
    }

    #[test]
    fn char_maps_to_char() {
        let key = KeyEvent::new(KeyCode::Char('a'), KeyModifiers::NONE);
        assert_eq!(Input::from(key), Input::Char('a'));
    }
}
