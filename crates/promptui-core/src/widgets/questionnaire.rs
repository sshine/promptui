use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::{Modifier, Style};
use ratatui::widgets::Widget;

use crate::focus::{FocusAction, Focusable};
use crate::input::Input;
use crate::theme::Theme;
use crate::widgets::padding::Padding;
use crate::widgets::text_field::TextField;

/// Left border character.
const LEFT_BORDER: &str = "\u{2503}"; // ┃

/// Border occupies 1 column.
const BORDER_WIDTH: u16 = 1;

/// Progress dots.
const DOT_FILLED: &str = "\u{25CF}"; // ●
const DOT_EMPTY: &str = "\u{25CB}"; // ○

/// Option indicators.
const RADIO_SELECTED: &str = "\u{25CF}"; // ●
const RADIO_UNSELECTED: &str = "\u{25CB}"; // ○
const CHECK_SELECTED: &str = "[\u{00D7}]"; // [×]
const CHECK_UNSELECTED: &str = "[ ]";

/// Cursor indicator for focused option.
const CURSOR: &str = "\u{203A}"; // ›

/// The type of a question.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum QuestionKind {
    SingleSelect,
    MultiSelect,
    FreeText,
}

/// A single option within a question.
#[derive(Debug, Clone)]
pub struct QuestionOption {
    pub label: String,
    pub selected: bool,
    pub inline_text: Option<String>,
}

impl QuestionOption {
    pub fn new(label: impl Into<String>) -> Self {
        Self {
            label: label.into(),
            selected: false,
            inline_text: None,
        }
    }

    pub fn with_inline_text(mut self) -> Self {
        self.inline_text = Some(String::new());
        self
    }

    pub fn selected(mut self, selected: bool) -> Self {
        self.selected = selected;
        self
    }
}

/// A question in the questionnaire.
#[derive(Debug, Clone)]
pub struct Question {
    pub prompt: String,
    pub kind: QuestionKind,
    pub options: Vec<QuestionOption>,
    pub text_content: String,
}

impl Question {
    pub fn single_select(prompt: impl Into<String>, options: Vec<QuestionOption>) -> Self {
        Self {
            prompt: prompt.into(),
            kind: QuestionKind::SingleSelect,
            options,
            text_content: String::new(),
        }
    }

    pub fn multi_select(prompt: impl Into<String>, options: Vec<QuestionOption>) -> Self {
        Self {
            prompt: prompt.into(),
            kind: QuestionKind::MultiSelect,
            options,
            text_content: String::new(),
        }
    }

    pub fn free_text(prompt: impl Into<String>) -> Self {
        Self {
            prompt: prompt.into(),
            kind: QuestionKind::FreeText,
            options: Vec::new(),
            text_content: String::new(),
        }
    }

    pub fn is_answered(&self) -> bool {
        match self.kind {
            QuestionKind::SingleSelect | QuestionKind::MultiSelect => {
                self.options.iter().any(|o| o.selected)
            }
            QuestionKind::FreeText => !self.text_content.trim().is_empty(),
        }
    }

    pub fn answer_summary(&self) -> String {
        match self.kind {
            QuestionKind::SingleSelect => self
                .options
                .iter()
                .find(|o| o.selected)
                .map(|o| {
                    if let Some(ref text) = o.inline_text {
                        if !text.is_empty() {
                            return format!("{}: {}", o.label, text);
                        }
                    }
                    o.label.clone()
                })
                .unwrap_or_default(),
            QuestionKind::MultiSelect => {
                let selected: Vec<&str> = self
                    .options
                    .iter()
                    .filter(|o| o.selected)
                    .map(|o| o.label.as_str())
                    .collect();
                selected.join(", ")
            }
            QuestionKind::FreeText => self.text_content.clone(),
        }
    }
}

/// Internal focus level within the questionnaire.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum InternalFocus {
    OptionList,
    InlineText,
    FreeTextField,
    /// Summary page shown after the last question.
    SummaryPage,
}

/// A multi-part planning questionnaire with tabbed questions and a summary page.
///
/// Enter selects and auto-advances to the next tab. On the last tab, Enter
/// goes to a summary page where Tab toggles between Accept/Auto mode and
/// Enter executes the plan. Esc cancels without executing.
pub struct Questionnaire {
    theme: Theme,
    padding: Padding,
    questions: Vec<Question>,
    current_tab: usize,
    focused_option: usize,
    internal_focus: InternalFocus,
    focused: bool,
    inline_field: TextField,
    /// On the summary page: 0 = Accept, 1 = Auto.
    summary_action: usize,
    /// Set to true when Enter is pressed on the summary page.
    completed: bool,
    /// Set to true when Esc is pressed from OptionList (cancel).
    cancelled: bool,
}

impl Questionnaire {
    pub fn new(theme: Theme, questions: Vec<Question>) -> Self {
        let mut inline_field = TextField::new(theme);
        inline_field.set_min_height(1);
        inline_field.set_max_height(1);
        inline_field.set_padding(Padding::uniform(0));
        inline_field.set_focused(false);

        Self {
            theme,
            padding: Padding::uniform(0),
            questions,
            current_tab: 0,
            focused_option: 0,
            internal_focus: InternalFocus::OptionList,
            focused: false,
            inline_field,
            summary_action: 0,
            completed: false,
            cancelled: false,
        }
    }

    pub fn set_padding(&mut self, padding: Padding) {
        self.padding = padding;
    }

    pub fn current_question(&self) -> Option<&Question> {
        self.questions.get(self.current_tab)
    }

    pub fn question_count(&self) -> usize {
        self.questions.len()
    }

    pub fn answered_count(&self) -> usize {
        self.questions.iter().filter(|q| q.is_answered()).count()
    }

    pub fn current_tab(&self) -> usize {
        self.current_tab
    }

    pub fn is_focused(&self) -> bool {
        self.focused
    }

    pub fn is_completed(&self) -> bool {
        self.completed
    }

    pub fn is_cancelled(&self) -> bool {
        self.cancelled
    }

    /// 0 = Accept, 1 = Auto
    pub fn summary_action(&self) -> usize {
        self.summary_action
    }

    pub fn questions(&self) -> &[Question] {
        &self.questions
    }

    pub fn summary_text(&self) -> String {
        let answered = self.answered_count();
        let total = self.question_count();
        let current_answer = self
            .current_question()
            .map(|q| q.answer_summary())
            .unwrap_or_default();
        if current_answer.is_empty() {
            format!("Survey: {}/{} answered", answered, total)
        } else {
            format!(
                "Survey: {} \u{00B7} {}/{} answered",
                current_answer, answered, total
            )
        }
    }

    pub fn required_height(&self, _width: u16) -> u16 {
        if self.internal_focus == InternalFocus::SummaryPage {
            // Header(1) + gap(1) + questions + gap(1) + mode toggle(1) + hints(1) = questions + 5
            return self.questions.len() as u16 + 5 + self.padding.vertical();
        }
        let q = match self.current_question() {
            Some(q) => q,
            None => return 0,
        };
        match q.kind {
            QuestionKind::SingleSelect | QuestionKind::MultiSelect => {
                q.options.len() as u16 + 5 + self.padding.vertical()
            }
            QuestionKind::FreeText => 8 + self.padding.vertical(),
        }
    }

    pub fn cursor_position(&self, area: Rect) -> Option<(u16, u16)> {
        if !self.focused {
            return None;
        }
        match self.internal_focus {
            InternalFocus::InlineText => {
                let q = self.current_question()?;
                let text_x = area.x + BORDER_WIDTH + self.padding.left;
                let y_start = area.y + self.padding.top;
                let option_y = y_start + 3 + self.focused_option as u16;
                if option_y >= area.y + area.height {
                    return None;
                }
                let option = &q.options[self.focused_option];
                let indicator_width = match q.kind {
                    QuestionKind::SingleSelect => 2,
                    QuestionKind::MultiSelect => 4,
                    QuestionKind::FreeText => 0,
                };
                let prefix_width = 2 + indicator_width + option.label.len() as u16 + 2;
                let cursor_x = text_x + prefix_width;
                let content = self.inline_field.content();
                let char_offset = content.len() as u16;
                let final_x = cursor_x + char_offset;
                if final_x >= area.x + area.width {
                    None
                } else {
                    Some((final_x, option_y))
                }
            }
            InternalFocus::FreeTextField => {
                let text_x = area.x + BORDER_WIDTH + self.padding.left;
                let y_start = area.y + self.padding.top;
                let field_y = y_start + 3;
                let content = self.inline_field.content();
                let char_offset = content.len() as u16;
                let final_x = text_x + char_offset;
                if final_x >= area.x + area.width || field_y >= area.y + area.height {
                    None
                } else {
                    Some((final_x, field_y))
                }
            }
            InternalFocus::OptionList | InternalFocus::SummaryPage => None,
        }
    }

    // --- Navigation ---

    fn move_option_up(&mut self) {
        if self.focused_option > 0 {
            self.save_inline_text();
            self.focused_option -= 1;
            self.try_activate_inline();
        }
    }

    fn move_option_down(&mut self) {
        let count = self
            .current_question()
            .map(|q| q.options.len())
            .unwrap_or(0);
        if self.focused_option + 1 < count {
            self.save_inline_text();
            self.focused_option += 1;
            self.try_activate_inline();
        }
    }

    fn prev_tab(&mut self) {
        if self.current_tab > 0 {
            self.save_inline_text();
            self.current_tab -= 1;
            self.focused_option = 0;
            self.internal_focus = InternalFocus::OptionList;
            self.try_activate_inline();
        }
    }

    fn next_tab(&mut self) {
        if self.current_tab + 1 < self.questions.len() {
            self.save_inline_text();
            self.current_tab += 1;
            self.focused_option = 0;
            self.internal_focus = InternalFocus::OptionList;
            self.try_activate_inline();
        }
    }

    fn select_current(&mut self) {
        let tab = self.current_tab;
        let opt = self.focused_option;
        if let Some(q) = self.questions.get_mut(tab) {
            match q.kind {
                QuestionKind::SingleSelect => {
                    for (i, o) in q.options.iter_mut().enumerate() {
                        o.selected = i == opt;
                    }
                }
                QuestionKind::MultiSelect => {
                    if let Some(o) = q.options.get_mut(opt) {
                        o.selected = !o.selected;
                    }
                }
                QuestionKind::FreeText => {}
            }
        }
    }

    /// Select current AND auto-advance to next tab or summary page.
    fn select_and_advance(&mut self) {
        self.select_current();
        if self.current_tab + 1 < self.questions.len() {
            self.next_tab();
        } else {
            // Last tab: go to summary page
            self.save_inline_text();
            self.internal_focus = InternalFocus::SummaryPage;
        }
    }

    /// Clear all selections for the current question.
    fn clear_current_question(&mut self) {
        let tab = self.current_tab;
        if let Some(q) = self.questions.get_mut(tab) {
            for opt in &mut q.options {
                opt.selected = false;
                if let Some(ref mut text) = opt.inline_text {
                    text.clear();
                }
            }
            q.text_content.clear();
        }
    }

    fn try_activate_inline(&mut self) {
        let activate_info = self.current_question().map(|q| {
            if q.kind == QuestionKind::FreeText {
                Some((InternalFocus::FreeTextField, q.text_content.clone()))
            } else if let Some(opt) = q.options.get(self.focused_option) {
                if opt.inline_text.is_some() {
                    Some((
                        InternalFocus::InlineText,
                        opt.inline_text.clone().unwrap_or_default(),
                    ))
                } else {
                    None
                }
            } else {
                None
            }
        });

        if let Some(Some((focus_mode, existing_text))) = activate_info {
            self.inline_field.clear();
            for ch in existing_text.chars() {
                self.inline_field.input(Input::Char(ch));
            }
            self.inline_field.set_focused(true);
            self.internal_focus = focus_mode;
        } else {
            self.inline_field.set_focused(false);
            self.internal_focus = InternalFocus::OptionList;
        }
    }

    fn save_inline_text(&mut self) {
        match self.internal_focus {
            InternalFocus::InlineText => {
                let content = self.inline_field.content();
                let tab = self.current_tab;
                let opt = self.focused_option;
                if let Some(q) = self.questions.get_mut(tab) {
                    if let Some(o) = q.options.get_mut(opt) {
                        if o.inline_text.is_some() {
                            o.inline_text = Some(content);
                        }
                    }
                }
                self.inline_field.set_focused(false);
            }
            InternalFocus::FreeTextField => {
                let content = self.inline_field.content();
                let tab = self.current_tab;
                if let Some(q) = self.questions.get_mut(tab) {
                    q.text_content = content;
                }
                self.inline_field.set_focused(false);
            }
            InternalFocus::OptionList | InternalFocus::SummaryPage => {}
        }
    }

    fn hint_text(&self) -> &'static str {
        match self.internal_focus {
            InternalFocus::OptionList => {
                "\u{2190}/\u{2192} tabs  \u{2191}/\u{2193} options  Enter select+next  Backspace clear  Esc cancel"
            }
            InternalFocus::InlineText | InternalFocus::FreeTextField => {
                "Type to edit  Esc/\u{2191}/\u{2193} exit field  \u{2190}/\u{2192} tabs"
            }
            InternalFocus::SummaryPage => "Tab toggle mode  Enter execute  Esc back",
        }
    }
}

impl Focusable for Questionnaire {
    fn handle_input(&mut self, input: Input) -> FocusAction {
        match self.internal_focus {
            InternalFocus::InlineText | InternalFocus::FreeTextField => match input {
                Input::Esc => {
                    self.save_inline_text();
                    self.internal_focus = InternalFocus::OptionList;
                    self.inline_field.set_focused(false);
                    FocusAction::Consumed
                }
                Input::Up => {
                    let was_free = self.internal_focus == InternalFocus::FreeTextField;
                    self.save_inline_text();
                    self.internal_focus = InternalFocus::OptionList;
                    self.inline_field.set_focused(false);
                    if !was_free {
                        self.move_option_up();
                    }
                    FocusAction::Consumed
                }
                Input::Down => {
                    let was_free = self.internal_focus == InternalFocus::FreeTextField;
                    self.save_inline_text();
                    self.internal_focus = InternalFocus::OptionList;
                    self.inline_field.set_focused(false);
                    if !was_free {
                        self.move_option_down();
                    }
                    FocusAction::Consumed
                }
                Input::Enter => {
                    self.save_inline_text();
                    self.internal_focus = InternalFocus::OptionList;
                    self.inline_field.set_focused(false);
                    self.select_and_advance();
                    FocusAction::Consumed
                }
                _ => {
                    self.inline_field.input(input);
                    FocusAction::Consumed
                }
            },
            InternalFocus::OptionList => match input {
                Input::Esc => {
                    self.cancelled = true;
                    FocusAction::Pop
                }
                Input::Up => {
                    self.move_option_up();
                    FocusAction::Consumed
                }
                Input::Down => {
                    self.move_option_down();
                    FocusAction::Consumed
                }
                Input::Left => {
                    self.prev_tab();
                    FocusAction::Consumed
                }
                Input::Right => {
                    self.next_tab();
                    FocusAction::Consumed
                }
                Input::Enter => {
                    self.select_and_advance();
                    FocusAction::Consumed
                }
                Input::Char(' ') => {
                    self.select_current();
                    FocusAction::Consumed
                }
                Input::Backspace => {
                    self.clear_current_question();
                    FocusAction::Consumed
                }
                _ => FocusAction::Ignored,
            },
            InternalFocus::SummaryPage => match input {
                Input::Tab => {
                    self.summary_action = (self.summary_action + 1) % 2;
                    FocusAction::Consumed
                }
                Input::Enter => {
                    self.completed = true;
                    FocusAction::Pop
                }
                Input::Esc => {
                    // Go back to last question tab
                    self.current_tab = self.questions.len().saturating_sub(1);
                    self.focused_option = 0;
                    self.internal_focus = InternalFocus::OptionList;
                    self.try_activate_inline();
                    FocusAction::Consumed
                }
                _ => FocusAction::Ignored,
            },
        }
    }

    fn on_focus(&mut self) {
        self.focused = true;
        self.completed = false;
        self.cancelled = false;
        self.try_activate_inline();
    }

    fn on_blur(&mut self) {
        self.focused = false;
        self.save_inline_text();
        self.internal_focus = InternalFocus::OptionList;
        self.inline_field.set_focused(false);
    }
}

impl Widget for &Questionnaire {
    fn render(self, area: Rect, buf: &mut Buffer) {
        if area.width == 0 || area.height == 0 {
            return;
        }

        if self.focused {
            if self.internal_focus == InternalFocus::SummaryPage {
                self.render_summary_page(area, buf);
            } else {
                self.render_full(area, buf);
            }
        } else {
            self.render_compact(area, buf);
        }
    }
}

impl Questionnaire {
    fn render_full(&self, area: Rect, buf: &mut Buffer) {
        let bg_style = Style::default().bg(self.theme.panel_bg);
        let text_style = self.theme.text_style();
        let muted_style = self.theme.muted_style();
        let primary_style = Style::default()
            .fg(self.theme.primary)
            .bg(self.theme.panel_bg);
        let prompt_style = Style::default()
            .fg(self.theme.text)
            .bg(self.theme.panel_bg)
            .add_modifier(Modifier::BOLD);

        // Fill background
        for y in area.y..area.y + area.height {
            for x in area.x..area.x + area.width {
                if let Some(cell) = buf.cell_mut((x, y)) {
                    cell.set_char(' ');
                    cell.set_style(bg_style);
                }
            }
        }

        let q = match self.current_question() {
            Some(q) => q,
            None => return,
        };

        let text_x = area.x + BORDER_WIDTH + self.padding.left;
        let text_width = area
            .width
            .saturating_sub(BORDER_WIDTH + self.padding.horizontal());
        if text_width == 0 {
            return;
        }

        let mut y = area.y + self.padding.top;
        let y_max = area.y + area.height.saturating_sub(self.padding.bottom);

        // --- Tab dots ---
        if y < y_max {
            let mut x = text_x;
            for (i, question) in self.questions.iter().enumerate() {
                if x >= text_x + text_width {
                    break;
                }
                let dot = if i == self.current_tab || question.is_answered() {
                    DOT_FILLED
                } else {
                    DOT_EMPTY
                };
                let dot_style = if i == self.current_tab {
                    primary_style
                } else if question.is_answered() {
                    Style::default()
                        .fg(self.theme.success)
                        .bg(self.theme.panel_bg)
                } else {
                    muted_style
                };
                for ch in dot.chars() {
                    if x >= text_x + text_width {
                        break;
                    }
                    if let Some(cell) = buf.cell_mut((x, y)) {
                        cell.set_char(ch);
                        cell.set_style(dot_style);
                    }
                    x += 1;
                }
                if i + 1 < self.questions.len() && x < text_x + text_width {
                    x += 1;
                }
            }
            y += 1;
        }

        // --- Question prompt ---
        if y < y_max {
            let mut x = text_x;
            for ch in q.prompt.chars() {
                if x >= text_x + text_width {
                    break;
                }
                if let Some(cell) = buf.cell_mut((x, y)) {
                    cell.set_char(ch);
                    cell.set_style(prompt_style);
                }
                x += 1;
            }
            y += 1;
        }

        // --- Gap ---
        if y < y_max {
            y += 1;
        }

        // --- Options or free text ---
        match q.kind {
            QuestionKind::SingleSelect | QuestionKind::MultiSelect => {
                for (i, opt) in q.options.iter().enumerate() {
                    if y >= y_max {
                        break;
                    }
                    let is_focused = i == self.focused_option;

                    let mut x = text_x;

                    if is_focused {
                        if let Some(cell) = buf.cell_mut((x, y)) {
                            cell.set_symbol(CURSOR);
                            cell.set_style(primary_style);
                        }
                        x += 1;
                        if let Some(cell) = buf.cell_mut((x, y)) {
                            cell.set_char(' ');
                        }
                        x += 1;
                    } else {
                        x += 2;
                    }

                    let indicator = match q.kind {
                        QuestionKind::SingleSelect => {
                            if opt.selected {
                                RADIO_SELECTED
                            } else {
                                RADIO_UNSELECTED
                            }
                        }
                        QuestionKind::MultiSelect => {
                            if opt.selected {
                                CHECK_SELECTED
                            } else {
                                CHECK_UNSELECTED
                            }
                        }
                        QuestionKind::FreeText => "",
                    };
                    let indicator_style = if opt.selected {
                        primary_style
                    } else {
                        muted_style
                    };
                    for ch in indicator.chars() {
                        if x >= text_x + text_width {
                            break;
                        }
                        if let Some(cell) = buf.cell_mut((x, y)) {
                            cell.set_char(ch);
                            cell.set_style(indicator_style);
                        }
                        x += 1;
                    }

                    if x < text_x + text_width {
                        x += 1;
                    }

                    let label_style = if is_focused || opt.selected {
                        text_style
                    } else {
                        muted_style
                    };
                    for ch in opt.label.chars() {
                        if x >= text_x + text_width {
                            break;
                        }
                        if let Some(cell) = buf.cell_mut((x, y)) {
                            cell.set_char(ch);
                            cell.set_style(label_style);
                        }
                        x += 1;
                    }

                    if let Some(ref inline_text) = opt.inline_text {
                        let separator = ": ";
                        for ch in separator.chars() {
                            if x >= text_x + text_width {
                                break;
                            }
                            if let Some(cell) = buf.cell_mut((x, y)) {
                                cell.set_char(ch);
                                cell.set_style(muted_style);
                            }
                            x += 1;
                        }

                        let is_editing =
                            is_focused && self.internal_focus == InternalFocus::InlineText;
                        let field_style = if is_editing { text_style } else { muted_style };

                        let display_text = if is_editing {
                            self.inline_field.content()
                        } else {
                            inline_text.clone()
                        };

                        for ch in display_text.chars() {
                            if x >= text_x + text_width {
                                break;
                            }
                            if let Some(cell) = buf.cell_mut((x, y)) {
                                cell.set_char(ch);
                                cell.set_style(field_style);
                            }
                            x += 1;
                        }

                        if is_editing && x < text_x + text_width {
                            if let Some(cell) = buf.cell_mut((x, y)) {
                                cell.set_char('\u{2588}');
                                cell.set_style(primary_style);
                            }
                        }
                    }

                    y += 1;
                }
            }
            QuestionKind::FreeText => {
                if y < y_max {
                    let is_editing = self.internal_focus == InternalFocus::FreeTextField;
                    let field_style = if is_editing { text_style } else { muted_style };

                    let display_text = if is_editing {
                        self.inline_field.content()
                    } else {
                        q.text_content.clone()
                    };

                    let mut x = text_x;
                    for ch in display_text.chars() {
                        if x >= text_x + text_width {
                            break;
                        }
                        if let Some(cell) = buf.cell_mut((x, y)) {
                            cell.set_char(ch);
                            cell.set_style(field_style);
                        }
                        x += 1;
                    }

                    if is_editing && x < text_x + text_width {
                        if let Some(cell) = buf.cell_mut((x, y)) {
                            cell.set_char('\u{2588}');
                            cell.set_style(primary_style);
                        }
                    }

                    y += 1;
                }
            }
        }

        // --- Gap ---
        if y < y_max {
            y += 1;
        }

        // --- Hint bar ---
        if y < y_max {
            let hint = self.hint_text();
            let mut x = text_x;
            for ch in hint.chars() {
                if x >= text_x + text_width {
                    break;
                }
                if let Some(cell) = buf.cell_mut((x, y)) {
                    cell.set_char(ch);
                    cell.set_style(muted_style);
                }
                x += 1;
            }
        }
    }

    fn render_summary_page(&self, area: Rect, buf: &mut Buffer) {
        let bg_style = Style::default().bg(self.theme.panel_bg);
        let text_style = self.theme.text_style();
        let muted_style = self.theme.muted_style();
        let primary_style = Style::default()
            .fg(self.theme.primary)
            .bg(self.theme.panel_bg);
        let header_style = Style::default()
            .fg(self.theme.text)
            .bg(self.theme.panel_bg)
            .add_modifier(Modifier::BOLD);
        let success_style = Style::default()
            .fg(self.theme.success)
            .bg(self.theme.panel_bg);

        // Fill background
        for y in area.y..area.y + area.height {
            for x in area.x..area.x + area.width {
                if let Some(cell) = buf.cell_mut((x, y)) {
                    cell.set_char(' ');
                    cell.set_style(bg_style);
                }
            }
        }

        let text_x = area.x + BORDER_WIDTH + self.padding.left;
        let text_width = area
            .width
            .saturating_sub(BORDER_WIDTH + self.padding.horizontal());
        if text_width == 0 {
            return;
        }

        let mut y = area.y + self.padding.top;
        let y_max = area.y + area.height.saturating_sub(self.padding.bottom);

        // --- Header ---
        if y < y_max {
            let header = "Summary";
            let mut x = text_x;
            for ch in header.chars() {
                if x >= text_x + text_width {
                    break;
                }
                if let Some(cell) = buf.cell_mut((x, y)) {
                    cell.set_char(ch);
                    cell.set_style(header_style);
                }
                x += 1;
            }
            y += 1;
        }

        // --- Gap ---
        if y < y_max {
            y += 1;
        }

        // --- Questions with answers ---
        for q in &self.questions {
            if y >= y_max {
                break;
            }

            let mut x = text_x;

            // Check mark or empty
            let (mark, mark_style) = if q.is_answered() {
                ("\u{2713} ", success_style) // ✓
            } else {
                ("  ", muted_style)
            };
            for ch in mark.chars() {
                if x >= text_x + text_width {
                    break;
                }
                if let Some(cell) = buf.cell_mut((x, y)) {
                    cell.set_char(ch);
                    cell.set_style(mark_style);
                }
                x += 1;
            }

            // Prompt (truncated)
            let max_prompt = 20usize.min((text_width / 2) as usize);
            let prompt_display: String = if q.prompt.len() > max_prompt {
                format!("{}...", &q.prompt[..max_prompt.saturating_sub(3)])
            } else {
                q.prompt.clone()
            };
            for ch in prompt_display.chars() {
                if x >= text_x + text_width {
                    break;
                }
                if let Some(cell) = buf.cell_mut((x, y)) {
                    cell.set_char(ch);
                    cell.set_style(text_style);
                }
                x += 1;
            }

            // " → " separator
            for ch in " \u{2192} ".chars() {
                if x >= text_x + text_width {
                    break;
                }
                if let Some(cell) = buf.cell_mut((x, y)) {
                    cell.set_char(ch);
                    cell.set_style(muted_style);
                }
                x += 1;
            }

            // Answer
            let answer = q.answer_summary();
            let answer_display = if answer.is_empty() {
                "(unanswered)".to_string()
            } else {
                answer
            };
            let answer_style = if q.is_answered() {
                primary_style
            } else {
                muted_style
            };
            for ch in answer_display.chars() {
                if x >= text_x + text_width {
                    break;
                }
                if let Some(cell) = buf.cell_mut((x, y)) {
                    cell.set_char(ch);
                    cell.set_style(answer_style);
                }
                x += 1;
            }

            y += 1;
        }

        // --- Gap ---
        if y < y_max {
            y += 1;
        }

        // --- Mode toggle ---
        if y < y_max {
            let mut x = text_x;
            let modes = ["Accept", "Auto"];
            for (i, mode) in modes.iter().enumerate() {
                let is_selected = i == self.summary_action;
                let style = if is_selected {
                    Style::default()
                        .fg(self.theme.panel_bg)
                        .bg(self.theme.primary)
                        .add_modifier(Modifier::BOLD)
                } else {
                    muted_style
                };

                let label = if is_selected {
                    format!(" [{}] ", mode)
                } else {
                    format!("  {}  ", mode)
                };

                for ch in label.chars() {
                    if x >= text_x + text_width {
                        break;
                    }
                    if let Some(cell) = buf.cell_mut((x, y)) {
                        cell.set_char(ch);
                        cell.set_style(style);
                    }
                    x += 1;
                }
            }
            y += 1;
        }

        // --- Hint bar ---
        if y < y_max {
            let hint = self.hint_text();
            let mut x = text_x;
            for ch in hint.chars() {
                if x >= text_x + text_width {
                    break;
                }
                if let Some(cell) = buf.cell_mut((x, y)) {
                    cell.set_char(ch);
                    cell.set_style(muted_style);
                }
                x += 1;
            }
        }
    }

    fn render_compact(&self, area: Rect, buf: &mut Buffer) {
        if area.width == 0 || area.height == 0 {
            return;
        }

        let bg_style = Style::default().bg(self.theme.panel_bg);
        let border_style = Style::default()
            .fg(self.theme.secondary)
            .bg(self.theme.panel_bg);
        let text_style = self.theme.muted_style();

        for x in area.x..area.x + area.width {
            if let Some(cell) = buf.cell_mut((x, area.y)) {
                cell.set_char(' ');
                cell.set_style(bg_style);
            }
        }

        if let Some(cell) = buf.cell_mut((area.x, area.y)) {
            cell.set_symbol(LEFT_BORDER);
            cell.set_style(border_style);
        }

        let text_x = area.x + BORDER_WIDTH + self.padding.left;
        let text_width = area
            .width
            .saturating_sub(BORDER_WIDTH + self.padding.horizontal());
        let summary = self.summary_text();
        let mut x = text_x;
        for ch in summary.chars() {
            if x >= text_x + text_width {
                break;
            }
            if let Some(cell) = buf.cell_mut((x, area.y)) {
                cell.set_char(ch);
                cell.set_style(text_style);
            }
            x += 1;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_theme() -> Theme {
        Theme::default()
    }

    fn sample_questions() -> Vec<Question> {
        vec![
            Question::single_select(
                "Which approach?",
                vec![
                    QuestionOption::new("Option A"),
                    QuestionOption::new("Option B"),
                    QuestionOption::new("Other").with_inline_text(),
                ],
            ),
            Question::multi_select(
                "Which features?",
                vec![
                    QuestionOption::new("Logging"),
                    QuestionOption::new("Metrics"),
                    QuestionOption::new("Tracing"),
                ],
            ),
            Question::free_text("Additional notes?"),
        ]
    }

    #[test]
    fn new_questionnaire() {
        let q = Questionnaire::new(test_theme(), sample_questions());
        assert_eq!(q.question_count(), 3);
        assert_eq!(q.answered_count(), 0);
        assert!(!q.is_completed());
        assert!(!q.is_cancelled());
    }

    #[test]
    fn enter_selects_and_advances() {
        let mut q = Questionnaire::new(test_theme(), sample_questions());
        q.on_focus();
        q.internal_focus = InternalFocus::OptionList;
        // On first question, Enter selects option 0 and advances to tab 1
        q.handle_input(Input::Enter);
        assert!(q.questions[0].options[0].selected);
        assert_eq!(q.current_tab(), 1);
    }

    #[test]
    fn enter_on_last_tab_goes_to_summary() {
        let mut q = Questionnaire::new(test_theme(), sample_questions());
        q.on_focus();
        q.current_tab = 2; // last tab (free text)
        q.internal_focus = InternalFocus::OptionList;
        q.handle_input(Input::Enter);
        assert_eq!(q.internal_focus, InternalFocus::SummaryPage);
    }

    #[test]
    fn backspace_clears_selection() {
        let mut q = Questionnaire::new(test_theme(), sample_questions());
        q.on_focus();
        q.internal_focus = InternalFocus::OptionList;
        // Select option 1
        q.focused_option = 1;
        q.select_current();
        assert!(q.questions[0].options[1].selected);
        // Backspace clears
        q.handle_input(Input::Backspace);
        assert!(!q.questions[0].is_answered());
    }

    #[test]
    fn esc_cancels() {
        let mut q = Questionnaire::new(test_theme(), sample_questions());
        q.on_focus();
        q.internal_focus = InternalFocus::OptionList;
        let action = q.handle_input(Input::Esc);
        assert_eq!(action, FocusAction::Pop);
        assert!(q.is_cancelled());
        assert!(!q.is_completed());
    }

    #[test]
    fn summary_page_enter_completes() {
        let mut q = Questionnaire::new(test_theme(), sample_questions());
        q.on_focus();
        q.internal_focus = InternalFocus::SummaryPage;
        let action = q.handle_input(Input::Enter);
        assert_eq!(action, FocusAction::Pop);
        assert!(q.is_completed());
        assert!(!q.is_cancelled());
    }

    #[test]
    fn summary_page_tab_toggles_mode() {
        let mut q = Questionnaire::new(test_theme(), sample_questions());
        q.on_focus();
        q.internal_focus = InternalFocus::SummaryPage;
        assert_eq!(q.summary_action(), 0); // Accept
        q.handle_input(Input::Tab);
        assert_eq!(q.summary_action(), 1); // Auto
        q.handle_input(Input::Tab);
        assert_eq!(q.summary_action(), 0); // back to Accept
    }

    #[test]
    fn summary_page_esc_goes_back() {
        let mut q = Questionnaire::new(test_theme(), sample_questions());
        q.on_focus();
        q.internal_focus = InternalFocus::SummaryPage;
        let action = q.handle_input(Input::Esc);
        assert_eq!(action, FocusAction::Consumed);
        assert_eq!(q.current_tab(), 2); // last tab
        assert_ne!(q.internal_focus, InternalFocus::SummaryPage);
    }

    #[test]
    fn space_toggles_without_advancing() {
        let mut q = Questionnaire::new(test_theme(), sample_questions());
        q.on_focus();
        q.internal_focus = InternalFocus::OptionList;
        q.handle_input(Input::Char(' '));
        // Should select but stay on same tab
        assert!(q.questions[0].options[0].selected);
        assert_eq!(q.current_tab(), 0);
    }

    #[test]
    fn render_summary_page_does_not_panic() {
        let mut q = Questionnaire::new(test_theme(), sample_questions());
        q.focused = true;
        q.internal_focus = InternalFocus::SummaryPage;
        let area = Rect::new(0, 0, 60, 15);
        let mut buf = Buffer::empty(area);
        (&q).render(area, &mut buf);
    }

    #[test]
    fn render_full_does_not_panic() {
        let mut q = Questionnaire::new(test_theme(), sample_questions());
        q.focused = true;
        let area = Rect::new(0, 0, 50, 15);
        let mut buf = Buffer::empty(area);
        (&q).render(area, &mut buf);
    }

    #[test]
    fn render_compact_does_not_panic() {
        let q = Questionnaire::new(test_theme(), sample_questions());
        let area = Rect::new(0, 0, 50, 1);
        let mut buf = Buffer::empty(area);
        (&q).render(area, &mut buf);
    }

    #[test]
    fn on_blur_saves_state() {
        let mut q = Questionnaire::new(test_theme(), sample_questions());
        q.on_focus();
        q.focused_option = 2;
        q.try_activate_inline();
        assert_eq!(q.internal_focus, InternalFocus::InlineText);
        q.inline_field.input(Input::Char('h'));
        q.inline_field.input(Input::Char('i'));
        q.on_blur();
        assert_eq!(
            q.questions[0].options[2].inline_text,
            Some("hi".to_string())
        );
    }
}
