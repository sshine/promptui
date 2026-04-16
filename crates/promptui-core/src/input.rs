use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

/// Abstraction over terminal key events.
///
/// Converts crossterm key events into a simpler enum that widgets can
/// pattern-match against. Provides emacs-style keybindings for text editing.
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

    // --- Cursor movement ---
    Left,
    Right,
    Up,
    Down,
    Home,
    End,

    /// Alt+F / Ctrl+Right: move cursor to next word boundary.
    WordForward,
    /// Alt+B / Ctrl+Left: move cursor to previous word boundary.
    WordBackward,

    // --- Deletion ---
    /// Ctrl+W / Alt+Backspace / Ctrl+Backspace: delete word backward.
    DeleteWordBackward,
    /// Alt+D: delete word forward.
    DeleteWordForward,
    /// Ctrl+K: delete from cursor to end of line.
    KillToEnd,
    /// Ctrl+J: delete from cursor to start of line.
    KillToStart,
    /// Ctrl+U: clear the entire buffer.
    ClearAll,

    // --- Scrolling ---
    PageUp,
    PageDown,

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
            // --- Enter ---
            KeyCode::Enter if shift || alt => Input::Newline,
            KeyCode::Enter => Input::Enter,

            // --- Tab ---
            KeyCode::Tab => Input::Tab,

            // --- Ctrl+key bindings ---
            KeyCode::Char('c') if ctrl => Input::Esc,
            KeyCode::Char('u') if ctrl => Input::ClearAll,
            KeyCode::Char('k') if ctrl => Input::KillToEnd,
            KeyCode::Char('j') if ctrl => Input::KillToStart,
            KeyCode::Char('w') if ctrl => Input::DeleteWordBackward,
            KeyCode::Char('a') if ctrl => Input::Home,
            KeyCode::Char('e') if ctrl => Input::End,
            KeyCode::Char('f') if ctrl => Input::Right,
            KeyCode::Char('b') if ctrl => Input::Left,
            KeyCode::Char('n') if ctrl => Input::Down,
            KeyCode::Char('p') if ctrl => Input::Up,

            // --- Alt+key bindings ---
            KeyCode::Char('f') if alt => Input::WordForward,
            KeyCode::Char('b') if alt => Input::WordBackward,
            KeyCode::Char('d') if alt => Input::DeleteWordForward,
            KeyCode::Backspace if alt => Input::DeleteWordBackward,

            // --- Ctrl+arrow ---
            KeyCode::Right if ctrl => Input::WordForward,
            KeyCode::Left if ctrl => Input::WordBackward,

            // --- Ctrl+Backspace (delete word) ---
            KeyCode::Backspace if ctrl => Input::DeleteWordBackward,

            // --- Plain characters (no Ctrl/Alt) ---
            KeyCode::Char(c) if !alt && !ctrl => Input::Char(c),

            // --- Navigation ---
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

    #[test]
    fn ctrl_k_maps_to_kill_to_end() {
        let key = KeyEvent::new(KeyCode::Char('k'), KeyModifiers::CONTROL);
        assert_eq!(Input::from(key), Input::KillToEnd);
    }

    #[test]
    fn ctrl_w_maps_to_delete_word_backward() {
        let key = KeyEvent::new(KeyCode::Char('w'), KeyModifiers::CONTROL);
        assert_eq!(Input::from(key), Input::DeleteWordBackward);
    }

    #[test]
    fn alt_f_maps_to_word_forward() {
        let key = KeyEvent::new(KeyCode::Char('f'), KeyModifiers::ALT);
        assert_eq!(Input::from(key), Input::WordForward);
    }

    #[test]
    fn alt_b_maps_to_word_backward() {
        let key = KeyEvent::new(KeyCode::Char('b'), KeyModifiers::ALT);
        assert_eq!(Input::from(key), Input::WordBackward);
    }

    #[test]
    fn ctrl_a_maps_to_home() {
        let key = KeyEvent::new(KeyCode::Char('a'), KeyModifiers::CONTROL);
        assert_eq!(Input::from(key), Input::Home);
    }

    #[test]
    fn ctrl_e_maps_to_end() {
        let key = KeyEvent::new(KeyCode::Char('e'), KeyModifiers::CONTROL);
        assert_eq!(Input::from(key), Input::End);
    }

    #[test]
    fn ctrl_right_maps_to_word_forward() {
        let key = KeyEvent::new(KeyCode::Right, KeyModifiers::CONTROL);
        assert_eq!(Input::from(key), Input::WordForward);
    }

    #[test]
    fn alt_backspace_maps_to_delete_word_backward() {
        let key = KeyEvent::new(KeyCode::Backspace, KeyModifiers::ALT);
        assert_eq!(Input::from(key), Input::DeleteWordBackward);
    }

    #[test]
    fn alt_d_maps_to_delete_word_forward() {
        let key = KeyEvent::new(KeyCode::Char('d'), KeyModifiers::ALT);
        assert_eq!(Input::from(key), Input::DeleteWordForward);
    }
}
