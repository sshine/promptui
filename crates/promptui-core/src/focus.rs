use crate::input::Input;

/// Result of a focusable component handling input.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FocusAction {
    /// Input was consumed, no navigation change.
    Consumed,
    /// Component wants to be popped from the focus stack (e.g., Esc pressed).
    Pop,
    /// Input was not consumed; the app can handle it (e.g., global shortcuts).
    Ignored,
}

/// A component that can receive exclusive keyboard focus.
///
/// The app-level focus stack stores [`FocusId`] tags, not trait objects.
/// The app dispatches input to the right concrete widget based on `FocusId`,
/// calling this trait's methods. This avoids borrow-checker conflicts where
/// the stack and the widget would need simultaneous mutable access.
pub trait Focusable {
    /// Handle an input event while this component has focus.
    /// Called only when this component is the top of the focus stack.
    fn handle_input(&mut self, input: Input) -> FocusAction;

    /// Called when this component gains focus (pushed onto stack or
    /// revealed when something above it is popped).
    fn on_focus(&mut self);

    /// Called when this component loses focus (something pushed on top,
    /// or this component is popped).
    fn on_blur(&mut self);
}

/// Identifies which focusable component is on the stack.
///
/// The app matches on this to dispatch input to the correct widget field.
/// This is an enum rather than trait objects because the app owns all
/// widget state as concrete struct fields.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FocusId {
    /// The text input field (always at the bottom of the stack).
    TextField,
    /// The agent overview/selector panel.
    AgentOverview,
    /// A planning questionnaire.
    Questionnaire,
}

/// Manages the focus stack for layered input capture.
///
/// The stack always has [`FocusId::TextField`] at the bottom. Components
/// push onto the stack to capture input; [`FocusStack::pop`] removes
/// the top component (but never removes TextField).
///
/// The app uses [`FocusStack::top`] to decide which widget receives
/// keyboard input each tick.
pub struct FocusStack {
    stack: Vec<FocusId>,
}

impl FocusStack {
    pub fn new() -> Self {
        Self {
            stack: vec![FocusId::TextField],
        }
    }

    /// The currently focused component (top of stack).
    /// Always returns a value because TextField is permanently at the bottom.
    pub fn top(&self) -> FocusId {
        self.stack.last().copied().unwrap_or(FocusId::TextField)
    }

    /// Push a component onto the focus stack.
    /// Returns `false` if it's already on top (no-op).
    pub fn push(&mut self, id: FocusId) -> bool {
        if self.stack.last() == Some(&id) {
            return false;
        }
        self.stack.push(id);
        true
    }

    /// Pop the top component. Never pops below TextField.
    /// Returns the popped `FocusId`, or `None` if only TextField remains.
    pub fn pop(&mut self) -> Option<FocusId> {
        if self.stack.len() <= 1 {
            return None;
        }
        self.stack.pop()
    }

    /// Whether a given component is anywhere in the stack.
    pub fn contains(&self, id: FocusId) -> bool {
        self.stack.contains(&id)
    }

    /// Current stack depth (always >= 1 because of TextField).
    pub fn depth(&self) -> usize {
        self.stack.len()
    }
}

impl Default for FocusStack {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn new_stack_has_text_field() {
        let stack = FocusStack::new();
        assert_eq!(stack.top(), FocusId::TextField);
        assert_eq!(stack.depth(), 1);
    }

    #[test]
    fn push_changes_top() {
        let mut stack = FocusStack::new();
        assert!(stack.push(FocusId::AgentOverview));
        assert_eq!(stack.top(), FocusId::AgentOverview);
        assert_eq!(stack.depth(), 2);
    }

    #[test]
    fn push_same_is_noop() {
        let mut stack = FocusStack::new();
        stack.push(FocusId::AgentOverview);
        assert!(!stack.push(FocusId::AgentOverview));
        assert_eq!(stack.depth(), 2);
    }

    #[test]
    fn pop_returns_top() {
        let mut stack = FocusStack::new();
        stack.push(FocusId::Questionnaire);
        let popped = stack.pop();
        assert_eq!(popped, Some(FocusId::Questionnaire));
        assert_eq!(stack.top(), FocusId::TextField);
    }

    #[test]
    fn pop_never_removes_text_field() {
        let mut stack = FocusStack::new();
        assert_eq!(stack.pop(), None);
        assert_eq!(stack.top(), FocusId::TextField);
        assert_eq!(stack.depth(), 1);
    }

    #[test]
    fn contains_checks_whole_stack() {
        let mut stack = FocusStack::new();
        stack.push(FocusId::AgentOverview);
        stack.push(FocusId::Questionnaire);
        assert!(stack.contains(FocusId::TextField));
        assert!(stack.contains(FocusId::AgentOverview));
        assert!(stack.contains(FocusId::Questionnaire));
    }

    #[test]
    fn multi_level_push_pop() {
        let mut stack = FocusStack::new();
        stack.push(FocusId::AgentOverview);
        stack.push(FocusId::Questionnaire);
        assert_eq!(stack.depth(), 3);
        assert_eq!(stack.top(), FocusId::Questionnaire);

        stack.pop();
        assert_eq!(stack.top(), FocusId::AgentOverview);

        stack.pop();
        assert_eq!(stack.top(), FocusId::TextField);

        // Can't pop further
        assert_eq!(stack.pop(), None);
    }
}
