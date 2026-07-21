use std::time::Duration;

use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::{Color, Modifier, Style};
use ratatui::widgets::Widget;

use crate::focus::{FocusAction, Focusable};
use crate::input::Input;
use crate::theme::Theme;
use crate::widgets::agent_scope::{AgentStatus, format_tokens};
use crate::widgets::border_box::{BorderBox, BorderStyle};
use crate::widgets::message_list::{Message, MessageList};
use crate::widgets::padding::Padding;
use crate::widgets::scroll_box::ScrollBox;

/// Checkbox characters.
const CHECKED: &str = "[\u{00D7}]"; // [×]
const UNCHECKED: &str = "[ ]";

/// Tree-drawing characters.
const TREE_BRANCH: &str = "\u{251C}\u{2500}"; // ├─
const TREE_LAST: &str = "\u{2514}\u{2500}"; // └─

/// The three display modes for the agent overview panel.
/// Cycled by Ctrl+A: Collapsed → Summary → Full → Collapsed.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OverviewMode {
    /// Single status line: dots, count, tokens, hint.
    Collapsed,
    /// Bordered panel with max rows (current compact behavior).
    Summary,
    /// Full tree view replacing the message area.
    Full,
}

/// What the user is doing inside Full mode.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum InternalFocus {
    /// Navigating the agent tree (Up/Down moves between agents, Enter views messages).
    TreeNav,
    /// Viewing a specific agent's message list.
    AgentMessages(usize),
}

/// A single agent entry in the overview panel.
#[derive(Debug, Clone)]
pub struct AgentEntry {
    pub id: u64,
    pub label: String,
    pub color: Color,
    pub status: AgentStatus,
    pub duration: Option<Duration>,
    pub input_tokens: usize,
    pub output_tokens: usize,
    pub activity: String,
    pub checked: bool,
    pub depth: u16,
}

impl AgentEntry {
    pub fn new(id: u64, label: String) -> Self {
        Self {
            id,
            label,
            color: Color::default(),
            status: AgentStatus::Active,
            duration: None,
            input_tokens: 0,
            output_tokens: 0,
            activity: String::new(),
            checked: true,
            depth: 0,
        }
    }

    pub fn color(mut self, color: Color) -> Self {
        self.color = color;
        self
    }

    pub fn status(mut self, status: AgentStatus) -> Self {
        self.status = status;
        self
    }

    pub fn duration(mut self, duration: Duration) -> Self {
        self.duration = Some(duration);
        self
    }

    pub fn input_tokens(mut self, count: usize) -> Self {
        self.input_tokens = count;
        self
    }

    pub fn output_tokens(mut self, count: usize) -> Self {
        self.output_tokens = count;
        self
    }

    pub fn activity(mut self, activity: String) -> Self {
        self.activity = activity;
        self
    }

    pub fn checked(mut self, checked: bool) -> Self {
        self.checked = checked;
        self
    }

    pub fn depth(mut self, depth: u16) -> Self {
        self.depth = depth;
        self
    }
}

/// A checkbox-driven agent panel with three display modes and per-agent message lists.
///
/// Ctrl+A cycles: Collapsed → Summary → Full → Collapsed.
/// In Full mode, the panel replaces the message area and shows a navigable tree.
/// Enter on an agent in Full mode shows that agent's message list.
pub struct AgentOverview {
    theme: Theme,
    padding: Padding,
    agents: Vec<AgentEntry>,
    focused: usize,
    mode: OverviewMode,
    internal_focus: InternalFocus,
    max_compact_rows: u16,
    /// Per-agent message lists.
    agent_messages: Vec<MessageList>,
    /// Per-agent scroll state.
    agent_scrolls: Vec<ScrollBox>,
}

impl AgentOverview {
    pub fn new(theme: Theme) -> Self {
        Self {
            theme,
            padding: Padding::uniform(0),
            agents: Vec::new(),
            focused: 0,
            mode: OverviewMode::Collapsed,
            internal_focus: InternalFocus::TreeNav,
            max_compact_rows: 3,
            agent_messages: Vec::new(),
            agent_scrolls: Vec::new(),
        }
    }

    pub fn set_padding(&mut self, padding: Padding) {
        self.padding = padding;
    }

    /// Add an agent entry with its own message list.
    pub fn push(&mut self, entry: AgentEntry) {
        self.agents.push(entry);
        let mut ml = MessageList::new(self.theme);
        ml.set_padding(Padding::uniform(1));
        self.agent_messages.push(ml);
        self.agent_scrolls.push(ScrollBox::new());
    }

    /// Push a message to a specific agent's message list.
    pub fn push_agent_message(&mut self, agent_index: usize, message: Message) {
        if let Some(ml) = self.agent_messages.get_mut(agent_index) {
            ml.push(message);
        }
        if let Some(sb) = self.agent_scrolls.get_mut(agent_index) {
            sb.scroll_to_bottom();
        }
    }

    pub fn agent_count(&self) -> usize {
        self.agents.len()
    }

    pub fn agents(&self) -> &[AgentEntry] {
        &self.agents
    }

    pub fn agent_mut(&mut self, index: usize) -> Option<&mut AgentEntry> {
        self.agents.get_mut(index)
    }

    pub fn focus_up(&mut self) {
        if self.focused > 0 {
            self.focused -= 1;
        }
    }

    pub fn focus_down(&mut self) {
        if !self.agents.is_empty() && self.focused < self.agents.len() - 1 {
            self.focused += 1;
        }
    }

    pub fn toggle_focused(&mut self) {
        if let Some(agent) = self.agents.get_mut(self.focused) {
            agent.checked = !agent.checked;
        }
    }

    pub fn checked_agents(&self) -> Vec<u64> {
        self.agents
            .iter()
            .filter(|a| a.checked)
            .map(|a| a.id)
            .collect()
    }

    pub fn checked_count(&self) -> usize {
        self.agents.iter().filter(|a| a.checked).count()
    }

    pub fn focused(&self) -> usize {
        self.focused
    }

    pub fn mode(&self) -> OverviewMode {
        self.mode
    }

    pub fn set_max_compact_rows(&mut self, n: u16) {
        self.max_compact_rows = n;
    }

    /// Cycle to the next display mode. Returns the new mode.
    pub fn cycle_mode(&mut self) -> OverviewMode {
        self.mode = match self.mode {
            OverviewMode::Collapsed => OverviewMode::Summary,
            OverviewMode::Summary => {
                self.internal_focus = InternalFocus::TreeNav;
                OverviewMode::Full
            }
            OverviewMode::Full => OverviewMode::Collapsed,
        };
        self.mode
    }

    /// Whether Full mode is showing a specific agent's messages.
    pub fn viewing_agent_messages(&self) -> Option<usize> {
        if self.mode != OverviewMode::Full {
            return None;
        }
        match self.internal_focus {
            InternalFocus::AgentMessages(i) => Some(i),
            _ => None,
        }
    }

    /// Aggregate input tokens across all agents.
    pub fn total_input_tokens(&self) -> usize {
        self.agents.iter().map(|a| a.input_tokens).sum()
    }

    /// Aggregate output tokens across all agents.
    pub fn total_output_tokens(&self) -> usize {
        self.agents.iter().map(|a| a.output_tokens).sum()
    }

    /// Height for header rendering (Collapsed or Summary). Full returns 0 (uses message area).
    pub fn required_height(&self) -> u16 {
        if self.agents.len() <= 1 {
            return 0;
        }
        match self.mode {
            OverviewMode::Collapsed => 1,
            OverviewMode::Summary => {
                let border_overhead = 2u16;
                let agent_count = self.agents.len() as u16;
                let content_rows = if agent_count > self.max_compact_rows {
                    self.max_compact_rows
                } else {
                    agent_count
                };
                border_overhead + content_rows + self.padding.vertical()
            }
            OverviewMode::Full => 0, // Full mode uses the message area
        }
    }

    /// Render the Full mode content into the given area (message area replacement).
    pub fn render_full_view(&mut self, area: Rect, buf: &mut Buffer) {
        if area.width == 0 || area.height == 0 {
            return;
        }

        match self.internal_focus {
            InternalFocus::TreeNav => self.render_tree(area, buf),
            InternalFocus::AgentMessages(idx) => self.render_agent_messages(area, buf, idx),
        }
    }

    fn render_tree(&self, area: Rect, buf: &mut Buffer) {
        let bg_style = Style::default().bg(self.theme.panel_bg);

        // Fill background
        for y in area.y..area.y + area.height {
            for x in area.x..area.x + area.width {
                if let Some(cell) = buf.cell_mut((x, y)) {
                    cell.set_char(' ');
                    cell.set_style(bg_style);
                }
            }
        }

        for (i, agent) in self.agents.iter().enumerate() {
            let row_y = area.y + i as u16;
            if row_y >= area.y + area.height {
                break;
            }

            let is_focused = i == self.focused;
            let status_color = match agent.status {
                AgentStatus::Active => self.theme.success,
                AgentStatus::Done => self.theme.success,
                AgentStatus::Error => self.theme.error,
                AgentStatus::Cancelled => self.theme.text_muted,
            };
            let row_bg = if is_focused {
                self.theme.element_bg
            } else {
                self.theme.panel_bg
            };
            let label_style =
                Style::default()
                    .fg(agent.color)
                    .bg(row_bg)
                    .add_modifier(if is_focused {
                        Modifier::BOLD
                    } else {
                        Modifier::empty()
                    });
            let icon_style = Style::default().fg(status_color).bg(row_bg);
            let muted_style = Style::default().fg(self.theme.text_muted).bg(row_bg);
            let tree_style = Style::default().fg(self.theme.border).bg(row_bg);

            // Fill row background if focused
            if is_focused {
                let row_style = Style::default().bg(self.theme.element_bg);
                for x in area.x..area.x + area.width {
                    if let Some(cell) = buf.cell_mut((x, row_y)) {
                        cell.set_char(' ');
                        cell.set_style(row_style);
                    }
                }
            }

            let mut x = area.x + self.padding.left;
            let x_max = area.x + area.width.saturating_sub(self.padding.right);

            // Tree indentation
            if agent.depth > 0 {
                // Indent for ancestor levels
                for _ in 0..(agent.depth - 1) {
                    if x + 2 > x_max {
                        break;
                    }
                    x += 2;
                }

                // Branch connector
                let is_last = self
                    .agents
                    .get(i + 1)
                    .map(|next| next.depth < agent.depth)
                    .unwrap_or(true);
                let connector = if is_last { TREE_LAST } else { TREE_BRANCH };
                for ch in connector.chars() {
                    if x >= x_max {
                        break;
                    }
                    if let Some(cell) = buf.cell_mut((x, row_y)) {
                        cell.set_char(ch);
                        cell.set_style(tree_style);
                    }
                    x += 1;
                }
                if x < x_max {
                    x += 1; // space after connector
                }
            }

            // Status icon
            let icon = agent.status.icon();
            for ch in icon.chars() {
                if x >= x_max {
                    break;
                }
                if let Some(cell) = buf.cell_mut((x, row_y)) {
                    cell.set_char(ch);
                    cell.set_style(icon_style);
                }
                x += 1;
            }
            if x < x_max {
                x += 1;
            }

            // Label
            let label_width = 14usize.min(x_max.saturating_sub(x) as usize);
            let label_display: String = if agent.label.len() > label_width {
                agent.label.chars().take(label_width).collect()
            } else {
                format!("{:<width$}", agent.label, width = label_width)
            };
            for ch in label_display.chars() {
                if x >= x_max {
                    break;
                }
                if let Some(cell) = buf.cell_mut((x, row_y)) {
                    cell.set_char(ch);
                    cell.set_style(label_style);
                }
                x += 1;
            }

            // Activity
            if !agent.activity.is_empty() && x < x_max {
                let avail = x_max.saturating_sub(x) as usize;
                let activity_width = 20usize.min(avail);
                let activity_display: String = if agent.activity.len() > activity_width {
                    agent.activity.chars().take(activity_width).collect()
                } else {
                    format!("{:<width$}", agent.activity, width = activity_width)
                };
                for ch in activity_display.chars() {
                    if x >= x_max {
                        break;
                    }
                    if let Some(cell) = buf.cell_mut((x, row_y)) {
                        cell.set_char(ch);
                        cell.set_style(muted_style);
                    }
                    x += 1;
                }
            }

            // Right-aligned stats
            let mut stats_parts: Vec<String> = Vec::new();
            if let Some(d) = agent.duration {
                let secs = d.as_secs();
                if secs < 60 {
                    stats_parts.push(format!("{}s", secs));
                } else {
                    stats_parts.push(format!("{}m", secs / 60));
                }
            }
            if agent.input_tokens > 0 || agent.output_tokens > 0 {
                stats_parts.push(format!(
                    "\u{2191}{} \u{2193}{}",
                    format_tokens(agent.input_tokens),
                    format_tokens(agent.output_tokens)
                ));
            }
            if !stats_parts.is_empty() {
                let stats_str = format!(" {} ", stats_parts.join(" \u{00B7} "));
                let stats_start = x_max.saturating_sub(stats_str.len() as u16);
                if stats_start > x {
                    let mut sx = stats_start;
                    for ch in stats_str.chars() {
                        if sx >= x_max {
                            break;
                        }
                        if let Some(cell) = buf.cell_mut((sx, row_y)) {
                            cell.set_char(ch);
                            cell.set_style(muted_style);
                        }
                        sx += 1;
                    }
                }
            }
        }
    }

    fn render_agent_messages(&mut self, area: Rect, buf: &mut Buffer, idx: usize) {
        let bg_style = Style::default().bg(self.theme.panel_bg);

        // Fill background
        for y in area.y..area.y + area.height {
            for x in area.x..area.x + area.width {
                if let Some(cell) = buf.cell_mut((x, y)) {
                    cell.set_char(' ');
                    cell.set_style(bg_style);
                }
            }
        }

        // Breadcrumb header: "Agents > {name}"
        let agent_label = self
            .agents
            .get(idx)
            .map(|a| a.label.clone())
            .unwrap_or_default();
        let agent_color = self
            .agents
            .get(idx)
            .map(|a| a.color)
            .unwrap_or(self.theme.text_muted);

        let header_y = area.y;
        let muted = self.theme.muted_style();
        let label_style = Style::default().fg(agent_color).bg(self.theme.panel_bg);

        let mut x = area.x + self.padding.left;
        let x_max = area.x + area.width;

        for ch in "Agents".chars() {
            if x >= x_max {
                break;
            }
            if let Some(cell) = buf.cell_mut((x, header_y)) {
                cell.set_char(ch);
                cell.set_style(muted);
            }
            x += 1;
        }
        for ch in " \u{203A} ".chars() {
            // " › "
            if x >= x_max {
                break;
            }
            if let Some(cell) = buf.cell_mut((x, header_y)) {
                cell.set_char(ch);
                cell.set_style(muted);
            }
            x += 1;
        }
        for ch in agent_label.chars() {
            if x >= x_max {
                break;
            }
            if let Some(cell) = buf.cell_mut((x, header_y)) {
                cell.set_char(ch);
                cell.set_style(label_style);
            }
            x += 1;
        }

        // Message list below header
        let msg_area = Rect {
            x: area.x,
            y: area.y + 1,
            width: area.width,
            height: area.height.saturating_sub(1),
        };

        if msg_area.height > 0
            && let (Some(ml), Some(sb)) = (
                self.agent_messages.get(idx),
                self.agent_scrolls.get_mut(idx),
            )
        {
            sb.render(msg_area, buf, ml);
        }
    }
}

impl Focusable for AgentOverview {
    fn handle_input(&mut self, input: Input) -> FocusAction {
        match self.internal_focus {
            InternalFocus::TreeNav => match input {
                Input::Esc => {
                    self.mode = OverviewMode::Summary;
                    FocusAction::Pop
                }
                Input::Up => {
                    self.focus_up();
                    FocusAction::Consumed
                }
                Input::Down => {
                    self.focus_down();
                    FocusAction::Consumed
                }
                Input::Enter => {
                    self.internal_focus = InternalFocus::AgentMessages(self.focused);
                    FocusAction::Consumed
                }
                Input::Char(' ') => {
                    self.toggle_focused();
                    FocusAction::Consumed
                }
                _ => FocusAction::Ignored,
            },
            InternalFocus::AgentMessages(idx) => match input {
                Input::Esc => {
                    self.internal_focus = InternalFocus::TreeNav;
                    FocusAction::Consumed
                }
                Input::PageUp => {
                    if let Some(sb) = self.agent_scrolls.get_mut(idx) {
                        sb.page_up();
                    }
                    FocusAction::Consumed
                }
                Input::PageDown => {
                    if let Some(sb) = self.agent_scrolls.get_mut(idx) {
                        sb.page_down();
                    }
                    FocusAction::Consumed
                }
                _ => FocusAction::Ignored,
            },
        }
    }

    fn on_focus(&mut self) {
        self.internal_focus = InternalFocus::TreeNav;
    }

    fn on_blur(&mut self) {
        // Reset internal focus when losing focus stack position
    }
}

/// Render for Collapsed and Summary modes (header rendering).
impl Widget for &AgentOverview {
    fn render(self, area: Rect, buf: &mut Buffer) {
        if area.width == 0 || area.height == 0 || self.agents.len() <= 1 {
            return;
        }

        match self.mode {
            OverviewMode::Collapsed => self.render_collapsed(area, buf),
            OverviewMode::Summary => self.render_summary(area, buf),
            OverviewMode::Full => {} // Full mode uses render_full_view() separately
        }
    }
}

impl AgentOverview {
    fn render_collapsed(&self, area: Rect, buf: &mut Buffer) {
        let bg_style = Style::default().bg(self.theme.panel_bg);
        let muted_style = self.theme.muted_style();

        // Fill background
        for x in area.x..area.x + area.width {
            if let Some(cell) = buf.cell_mut((x, area.y)) {
                cell.set_char(' ');
                cell.set_style(bg_style);
            }
        }

        let mut x = area.x + self.padding.left;
        let x_max = area.x + area.width.saturating_sub(self.padding.right);

        // Status dots per agent
        for agent in &self.agents {
            if x >= x_max {
                break;
            }
            let (icon, color) = match agent.status {
                AgentStatus::Active => ("\u{25CF}", self.theme.success), // ●
                AgentStatus::Done => ("\u{2713}", self.theme.success),   // ✓
                AgentStatus::Error => ("\u{2717}", self.theme.error),    // ✗
                AgentStatus::Cancelled => ("\u{2014}", self.theme.text_muted), // —
            };
            let style = Style::default().fg(color).bg(self.theme.panel_bg);
            for ch in icon.chars() {
                if x >= x_max {
                    break;
                }
                if let Some(cell) = buf.cell_mut((x, area.y)) {
                    cell.set_char(ch);
                    cell.set_style(style);
                }
                x += 1;
            }
        }

        // "  N agents"
        let count_str = format!("  {} agents", self.agents.len());
        for ch in count_str.chars() {
            if x >= x_max {
                break;
            }
            if let Some(cell) = buf.cell_mut((x, area.y)) {
                cell.set_char(ch);
                cell.set_style(muted_style);
            }
            x += 1;
        }

        // "  ↑Nk ↓Nk"
        let input_total = self.total_input_tokens();
        let output_total = self.total_output_tokens();
        let token_str = format!(
            "  \u{2191}{} \u{2193}{}",
            format_tokens(input_total),
            format_tokens(output_total)
        );
        for ch in token_str.chars() {
            if x >= x_max {
                break;
            }
            if let Some(cell) = buf.cell_mut((x, area.y)) {
                cell.set_char(ch);
                cell.set_style(muted_style);
            }
            x += 1;
        }

        // "  Ctrl+A"
        let hint = "  Ctrl+A";
        for ch in hint.chars() {
            if x >= x_max {
                break;
            }
            if let Some(cell) = buf.cell_mut((x, area.y)) {
                cell.set_char(ch);
                cell.set_style(muted_style);
            }
            x += 1;
        }
    }

    fn render_summary(&self, area: Rect, buf: &mut Buffer) {
        let title = format!("Agents ({})", self.agents.len());
        let border = BorderBox::new(self.theme, BorderStyle::Rounded)
            .border_color(self.theme.border)
            .title(title)
            .padding(Padding::uniform(0));

        border.render_chrome(area, buf);
        let inner = border.inner_area(area);

        if inner.width == 0 || inner.height == 0 {
            return;
        }

        let overflow = self.agents.len() as u16 > self.max_compact_rows;
        let max_agent_rows = if overflow {
            self.max_compact_rows.saturating_sub(1) as usize
        } else {
            self.agents.len()
        };

        for (i, agent) in self.agents.iter().enumerate() {
            if i >= max_agent_rows {
                break;
            }
            let row_y = inner.y + i as u16;
            if row_y >= inner.y + inner.height {
                break;
            }

            let is_focused = i == self.focused;
            let indent = agent.depth * 2;

            let status_color = match agent.status {
                AgentStatus::Active => self.theme.success,
                AgentStatus::Done => self.theme.success,
                AgentStatus::Error => self.theme.error,
                AgentStatus::Cancelled => self.theme.text_muted,
            };
            let row_bg = if is_focused {
                self.theme.element_bg
            } else {
                self.theme.panel_bg
            };
            let checkbox_style = Style::default()
                .fg(if is_focused {
                    self.theme.text
                } else {
                    self.theme.text_muted
                })
                .bg(row_bg)
                .add_modifier(if is_focused {
                    Modifier::BOLD
                } else {
                    Modifier::empty()
                });
            let label_style =
                Style::default()
                    .fg(agent.color)
                    .bg(row_bg)
                    .add_modifier(if is_focused {
                        Modifier::BOLD
                    } else {
                        Modifier::empty()
                    });
            let icon_style = Style::default().fg(status_color).bg(row_bg);
            let muted = Style::default().fg(self.theme.text_muted).bg(row_bg);

            if is_focused {
                let bg = Style::default().bg(self.theme.element_bg);
                for x in inner.x..inner.x + inner.width {
                    if let Some(cell) = buf.cell_mut((x, row_y)) {
                        cell.set_style(bg);
                        cell.set_char(' ');
                    }
                }
            }

            let mut x = inner.x + indent;
            let x_max = inner.x + inner.width;

            if x < x_max {
                x += 1;
            }

            let checkbox = if agent.checked { CHECKED } else { UNCHECKED };
            for ch in checkbox.chars() {
                if x >= x_max {
                    break;
                }
                if let Some(cell) = buf.cell_mut((x, row_y)) {
                    cell.set_char(ch);
                    cell.set_style(checkbox_style);
                }
                x += 1;
            }

            if x < x_max {
                x += 1;
            }

            let icon = agent.status.icon();
            for ch in icon.chars() {
                if x >= x_max {
                    break;
                }
                if let Some(cell) = buf.cell_mut((x, row_y)) {
                    cell.set_char(ch);
                    cell.set_style(icon_style);
                }
                x += 1;
            }

            if x < x_max {
                x += 1;
            }

            let label_width = 12usize.min(x_max.saturating_sub(x) as usize);
            let label_display: String = if agent.label.len() > label_width {
                agent.label.chars().take(label_width).collect()
            } else {
                format!("{:<width$}", agent.label, width = label_width)
            };
            for ch in label_display.chars() {
                if x >= x_max {
                    break;
                }
                if let Some(cell) = buf.cell_mut((x, row_y)) {
                    cell.set_char(ch);
                    cell.set_style(label_style);
                }
                x += 1;
            }

            if !agent.activity.is_empty() && x < x_max {
                let avail = x_max.saturating_sub(x) as usize;
                let activity_width = 16usize.min(avail);
                let activity_display: String = if agent.activity.len() > activity_width {
                    agent.activity.chars().take(activity_width).collect()
                } else {
                    format!("{:<width$}", agent.activity, width = activity_width)
                };
                for ch in activity_display.chars() {
                    if x >= x_max {
                        break;
                    }
                    if let Some(cell) = buf.cell_mut((x, row_y)) {
                        cell.set_char(ch);
                        cell.set_style(muted);
                    }
                    x += 1;
                }
            }

            let mut stats_parts: Vec<String> = Vec::new();
            if let Some(d) = agent.duration {
                let secs = d.as_secs();
                if secs < 60 {
                    stats_parts.push(format!("{}s", secs));
                } else {
                    stats_parts.push(format!("{}m", secs / 60));
                }
            }
            if agent.input_tokens > 0 || agent.output_tokens > 0 {
                stats_parts.push(format!(
                    "\u{2191}{} \u{2193}{}",
                    format_tokens(agent.input_tokens),
                    format_tokens(agent.output_tokens)
                ));
            }
            if !stats_parts.is_empty() {
                let stats_str = format!(" {} ", stats_parts.join(" \u{00B7} "));
                let stats_start = x_max.saturating_sub(stats_str.len() as u16);
                if stats_start > x {
                    let mut sx = stats_start;
                    for ch in stats_str.chars() {
                        if sx >= x_max {
                            break;
                        }
                        if let Some(cell) = buf.cell_mut((sx, row_y)) {
                            cell.set_char(ch);
                            cell.set_style(muted);
                        }
                        sx += 1;
                    }
                }
            }
        }

        if overflow {
            let hint_y = inner.y + max_agent_rows as u16;
            if hint_y < inner.y + inner.height {
                let remaining = self.agents.len() - max_agent_rows;
                let hint = format!("+{} more  Ctrl+A \u{25B8}", remaining);
                let hint_style = Style::default()
                    .fg(self.theme.text_muted)
                    .bg(self.theme.panel_bg);
                let mut x = inner.x + 1;
                for ch in hint.chars() {
                    if x >= inner.x + inner.width {
                        break;
                    }
                    if let Some(cell) = buf.cell_mut((x, hint_y)) {
                        cell.set_char(ch);
                        cell.set_style(hint_style);
                    }
                    x += 1;
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_theme() -> Theme {
        Theme::default()
    }

    #[test]
    fn empty_overview_height_zero() {
        let overview = AgentOverview::new(test_theme());
        assert_eq!(overview.required_height(), 0);
    }

    #[test]
    fn single_agent_height_zero() {
        let mut overview = AgentOverview::new(test_theme());
        overview.push(AgentEntry::new(1, "main".into()));
        assert_eq!(overview.required_height(), 0);
    }

    #[test]
    fn collapsed_mode_height_one() {
        let mut overview = AgentOverview::new(test_theme());
        overview.push(AgentEntry::new(1, "main".into()));
        overview.push(AgentEntry::new(2, "research".into()));
        assert_eq!(overview.mode(), OverviewMode::Collapsed);
        assert_eq!(overview.required_height(), 1);
    }

    #[test]
    fn summary_mode_height() {
        let mut overview = AgentOverview::new(test_theme());
        overview.push(AgentEntry::new(1, "main".into()));
        overview.push(AgentEntry::new(2, "research".into()));
        overview.cycle_mode(); // Collapsed → Summary
        assert_eq!(overview.mode(), OverviewMode::Summary);
        // 2 border + 2 agents = 4
        assert_eq!(overview.required_height(), 4);
    }

    #[test]
    fn full_mode_height_zero() {
        let mut overview = AgentOverview::new(test_theme());
        overview.push(AgentEntry::new(1, "main".into()));
        overview.push(AgentEntry::new(2, "research".into()));
        overview.cycle_mode(); // Collapsed → Summary
        overview.cycle_mode(); // Summary → Full
        assert_eq!(overview.mode(), OverviewMode::Full);
        assert_eq!(overview.required_height(), 0); // uses message area
    }

    #[test]
    fn cycle_mode_wraps() {
        let mut overview = AgentOverview::new(test_theme());
        assert_eq!(overview.mode(), OverviewMode::Collapsed);
        overview.cycle_mode();
        assert_eq!(overview.mode(), OverviewMode::Summary);
        overview.cycle_mode();
        assert_eq!(overview.mode(), OverviewMode::Full);
        overview.cycle_mode();
        assert_eq!(overview.mode(), OverviewMode::Collapsed);
    }

    #[test]
    fn focus_navigation() {
        let mut overview = AgentOverview::new(test_theme());
        overview.push(AgentEntry::new(1, "main".into()));
        overview.push(AgentEntry::new(2, "research".into()));
        overview.push(AgentEntry::new(3, "bugfix".into()));

        assert_eq!(overview.focused(), 0);
        overview.focus_down();
        assert_eq!(overview.focused(), 1);
        overview.focus_down();
        assert_eq!(overview.focused(), 2);
        overview.focus_down();
        assert_eq!(overview.focused(), 2);
        overview.focus_up();
        assert_eq!(overview.focused(), 1);
    }

    #[test]
    fn toggle_checked() {
        let mut overview = AgentOverview::new(test_theme());
        overview.push(AgentEntry::new(1, "main".into()).checked(true));
        overview.push(AgentEntry::new(2, "research".into()).checked(true));
        assert_eq!(overview.checked_count(), 2);
        overview.toggle_focused();
        assert_eq!(overview.checked_count(), 1);
    }

    #[test]
    fn per_agent_messages() {
        let mut overview = AgentOverview::new(test_theme());
        overview.push(AgentEntry::new(1, "main".into()));
        overview.push(AgentEntry::new(2, "research".into()));
        overview.push_agent_message(
            1,
            Message::plain(crate::widgets::message_list::Role::System, "step 1"),
        );
        // Should not panic
    }

    #[test]
    fn token_aggregation() {
        let mut overview = AgentOverview::new(test_theme());
        overview.push(
            AgentEntry::new(1, "main".into())
                .input_tokens(1000)
                .output_tokens(500),
        );
        overview.push(
            AgentEntry::new(2, "research".into())
                .input_tokens(2000)
                .output_tokens(800),
        );
        assert_eq!(overview.total_input_tokens(), 3000);
        assert_eq!(overview.total_output_tokens(), 1300);
    }

    #[test]
    fn tree_nav_enter_views_messages() {
        let mut overview = AgentOverview::new(test_theme());
        overview.push(AgentEntry::new(1, "main".into()));
        overview.push(AgentEntry::new(2, "research".into()));
        // Cycle to Full mode, then on_focus sets TreeNav
        overview.cycle_mode(); // Collapsed → Summary
        overview.cycle_mode(); // Summary → Full
        overview.on_focus();
        assert_eq!(overview.viewing_agent_messages(), None);
        let action = overview.handle_input(Input::Enter);
        assert_eq!(action, FocusAction::Consumed);
        assert_eq!(overview.viewing_agent_messages(), Some(0));
    }

    #[test]
    fn agent_messages_esc_returns_to_tree() {
        let mut overview = AgentOverview::new(test_theme());
        overview.push(AgentEntry::new(1, "main".into()));
        overview.cycle_mode(); // Collapsed → Summary
        overview.cycle_mode(); // Summary → Full
        overview.on_focus();
        overview.handle_input(Input::Enter); // enter messages
        assert_eq!(overview.viewing_agent_messages(), Some(0));
        let action = overview.handle_input(Input::Esc);
        assert_eq!(action, FocusAction::Consumed);
        assert_eq!(overview.viewing_agent_messages(), None);
    }

    #[test]
    fn tree_nav_esc_pops() {
        let mut overview = AgentOverview::new(test_theme());
        overview.push(AgentEntry::new(1, "main".into()));
        overview.on_focus();
        let action = overview.handle_input(Input::Esc);
        assert_eq!(action, FocusAction::Pop);
        assert_eq!(overview.mode(), OverviewMode::Summary);
    }

    #[test]
    fn render_collapsed_does_not_panic() {
        let mut overview = AgentOverview::new(test_theme());
        overview.push(
            AgentEntry::new(1, "main".into())
                .input_tokens(5000)
                .output_tokens(2000),
        );
        overview.push(AgentEntry::new(2, "research".into()));
        let area = Rect::new(0, 0, 60, 1);
        let mut buf = Buffer::empty(area);
        (&overview).render(area, &mut buf);
    }

    #[test]
    fn render_summary_does_not_panic() {
        let mut overview = AgentOverview::new(test_theme());
        overview.push(AgentEntry::new(1, "main".into()));
        overview.push(AgentEntry::new(2, "research".into()).depth(1));
        overview.cycle_mode(); // → Summary
        let area = Rect::new(0, 0, 50, 6);
        let mut buf = Buffer::empty(area);
        (&overview).render(area, &mut buf);
    }

    #[test]
    fn render_full_tree_does_not_panic() {
        let mut overview = AgentOverview::new(test_theme());
        overview.push(AgentEntry::new(1, "main".into()));
        overview.push(AgentEntry::new(2, "research".into()).depth(1));
        overview.cycle_mode(); // → Summary
        overview.cycle_mode(); // → Full
        let area = Rect::new(0, 0, 60, 10);
        let mut buf = Buffer::empty(area);
        overview.render_full_view(area, &mut buf);
    }
}
