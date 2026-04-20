use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

/// Abstraction over terminal key events.
///
/// Converts crossterm key events into a simpler enum that widgets can
/// pattern-match against.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Input {
    /// A printable character.
    Char(char),
    /// Bare Enter key (submit action in chat context).
    Enter,
    /// Shift+Enter or Alt+Enter (insert literal newline).
    Newline,
    Backspace,
    Delete,
    /// Tab key.
    Tab,

    Left,
    Right,
    Up,
    Down,
    Home,
    End,
    PageUp,
    PageDown,

    /// Ctrl+U: clear the entire buffer.
    ClearAll,

    Esc,
    /// An unrecognized key event.
    Unknown,
}

impl From<KeyEvent> for Input {
    fn from(key: KeyEvent) -> Self {
        let shift = key.modifiers.contains(KeyModifiers::SHIFT);
        let alt = key.modifiers.contains(KeyModifiers::ALT);
        let ctrl = key.modifiers.contains(KeyModifiers::CONTROL);

        match key.code {
            KeyCode::Enter if shift || alt => Input::Newline,
            KeyCode::Enter => Input::Enter,

            KeyCode::Tab => Input::Tab,

            KeyCode::Char('c') if ctrl => Input::Esc,
            KeyCode::Char('u') if ctrl => Input::ClearAll,

            // Only accept plain chars (no Ctrl/Alt modifiers) to avoid
            // interpreting escape sequences as printable text.
            KeyCode::Char(c) if !alt && !ctrl => Input::Char(c),

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

    #[test]
    fn ctrl_u_maps_to_clear_all() {
        let key = KeyEvent::new(KeyCode::Char('u'), KeyModifiers::CONTROL);
        assert_eq!(Input::from(key), Input::ClearAll);
    }

    #[test]
    fn ctrl_c_maps_to_esc() {
        let key = KeyEvent::new(KeyCode::Char('c'), KeyModifiers::CONTROL);
        assert_eq!(Input::from(key), Input::Esc);
    }

    #[test]
    fn ctrl_char_does_not_map_to_char() {
        let key = KeyEvent::new(KeyCode::Char('x'), KeyModifiers::CONTROL);
        assert_eq!(Input::from(key), Input::Unknown);
    }

    #[test]
    fn alt_char_does_not_map_to_char() {
        let key = KeyEvent::new(KeyCode::Char('x'), KeyModifiers::ALT);
        assert_eq!(Input::from(key), Input::Unknown);
    }
}
