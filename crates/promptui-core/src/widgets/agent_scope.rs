use std::time::Duration;

use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::Style;
use ratatui::widgets::Widget;

use crate::theme::Theme;
use crate::widgets::padding::Padding;

/// Tree-drawing characters for agent hierarchy.
const TREE_BRANCH: &str = "\u{251C}\u{2500}"; // ├─
const TREE_LAST: &str = "\u{2514}\u{2500}"; // └─
const TREE_PIPE: &str = "\u{2502}"; // │
const HORIZONTAL: &str = "\u{2501}"; // ━

/// How content blocks should be rendered.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DisplayMode {
    /// Full rendering: complete message text, full tool output.
    Full,
    /// Compact rendering: 1-2 line summary, tool calls as name+status.
    Compact,
}

/// Status of an agent scope.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AgentStatus {
    /// Agent is actively processing.
    Active,
    /// Agent completed successfully.
    Done,
    /// Agent encountered an error.
    Error,
    /// Agent was cancelled.
    Cancelled,
}

impl AgentStatus {
    /// Status icon character.
    pub fn icon(&self) -> &'static str {
        match self {
            AgentStatus::Active => "\u{25CF}",    // ●
            AgentStatus::Done => "\u{2713}",      // ✓
            AgentStatus::Error => "\u{2717}",     // ✗
            AgentStatus::Cancelled => "\u{2014}", // —
        }
    }
}

/// Format a duration as a compact string (e.g., "12s", "2m", "1h").
fn format_duration(d: Duration) -> String {
    let secs = d.as_secs();
    if secs < 60 {
        format!("{}s", secs)
    } else if secs < 3600 {
        format!("{}m", secs / 60)
    } else {
        format!("{}h", secs / 3600)
    }
}

/// Format a token count compactly (e.g., "2.1k", "14.5k").
pub fn format_tokens(count: usize) -> String {
    if count < 1000 {
        format!("{}", count)
    } else if count < 10_000 {
        format!("{:.1}k", count as f64 / 1000.0)
    } else if count < 1_000_000 {
        format!("{:.0}k", count as f64 / 1000.0)
    } else {
        format!("{:.1}M", count as f64 / 1_000_000.0)
    }
}

/// A section header in the compact multi-agent tree view.
///
/// Renders a single decorated line identifying an agent scope with
/// tree-drawing characters to show parent/child relationships.
///
/// # Example output
///
/// ```text
/// ├─ research (12s · 2.1k)
/// └─ bugfix (8s · 0.8k)
/// ```
pub struct AgentScopeHeader {
    theme: Theme,
    padding: Padding,
    /// Agent label (e.g., "research", "code-review").
    label: String,
    /// Color for this agent's identity.
    agent_color: ratatui::style::Color,
    /// Current status.
    status: AgentStatus,
    /// How long the agent has been running.
    duration: Option<Duration>,
    /// Token count for this agent.
    token_count: usize,
    /// Tree depth (0 = root agent, 1 = direct child, etc.).
    depth: u16,
    /// Whether this is the last sibling at its depth (└─ vs ├─).
    is_last_sibling: bool,
}

impl AgentScopeHeader {
    pub fn new(theme: Theme, label: String) -> Self {
        Self {
            agent_color: theme.secondary,
            theme,
            padding: Padding::uniform(0),
            label,
            status: AgentStatus::Active,
            duration: None,
            token_count: 0,
            depth: 0,
            is_last_sibling: false,
        }
    }

    pub fn agent_color(mut self, color: ratatui::style::Color) -> Self {
        self.agent_color = color;
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

    pub fn token_count(mut self, count: usize) -> Self {
        self.token_count = count;
        self
    }

    pub fn depth(mut self, depth: u16) -> Self {
        self.depth = depth;
        self
    }

    pub fn is_last_sibling(mut self, is_last: bool) -> Self {
        self.is_last_sibling = is_last;
        self
    }

    pub fn padding(mut self, padding: Padding) -> Self {
        self.padding = padding;
        self
    }

    pub fn set_padding(&mut self, padding: Padding) {
        self.padding = padding;
    }

    pub fn set_status(&mut self, status: AgentStatus) {
        self.status = status;
    }

    pub fn set_duration(&mut self, duration: Option<Duration>) {
        self.duration = duration;
    }

    pub fn set_token_count(&mut self, count: usize) {
        self.token_count = count;
    }

    /// The height this header needs (always 1 row + vertical padding).
    pub fn required_height(&self) -> u16 {
        1 + self.padding.vertical()
    }

    /// Build the formatted header text (e.g., "● research (12s · 2.1k)").
    pub fn header_text(&self) -> String {
        let icon = self.status.icon();
        let mut parts = vec![format!("{} {}", icon, self.label)];

        if let Some(d) = self.duration {
            parts.push(format_duration(d));
        }
        if self.token_count > 0 {
            parts.push(format_tokens(self.token_count));
        }

        let label_part = parts[0].clone();
        let stats: Vec<&str> = parts[1..].iter().map(|s| s.as_str()).collect();
        if stats.is_empty() {
            label_part
        } else {
            format!("{} ({})", label_part, stats.join(" \u{00B7} "))
        }
    }
}

impl Widget for &AgentScopeHeader {
    fn render(self, area: Rect, buf: &mut Buffer) {
        if area.width == 0 || area.height == 0 {
            return;
        }

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

        let y = area.y + self.padding.top;
        if y >= area.y + area.height {
            return;
        }

        let mut x = area.x + self.padding.left;
        let x_max = area.x + area.width.saturating_sub(self.padding.right);

        let tree_style = Style::default()
            .fg(self.theme.border)
            .bg(self.theme.panel_bg);
        let label_style = Style::default()
            .fg(self.agent_color)
            .bg(self.theme.panel_bg);
        let status_color = match self.status {
            AgentStatus::Active => self.theme.success,
            AgentStatus::Done => self.theme.success,
            AgentStatus::Error => self.theme.error,
            AgentStatus::Cancelled => self.theme.text_muted,
        };
        let icon_style = Style::default().fg(status_color).bg(self.theme.panel_bg);
        let stats_style = self.theme.muted_style();
        let rule_style = Style::default()
            .fg(self.theme.border)
            .bg(self.theme.panel_bg);

        // Tree indentation for non-root agents
        if self.depth > 0 {
            // Add indentation: 2 chars per depth level for ancestor pipes
            for _level in 0..(self.depth - 1) {
                if x + 2 > x_max {
                    return;
                }
                if let Some(cell) = buf.cell_mut((x, y)) {
                    cell.set_symbol(TREE_PIPE);
                    cell.set_style(tree_style);
                }
                x += 1;
                if let Some(cell) = buf.cell_mut((x, y)) {
                    cell.set_char(' ');
                    cell.set_style(tree_style);
                }
                x += 1;
            }

            // Branch connector
            let connector = if self.is_last_sibling {
                TREE_LAST
            } else {
                TREE_BRANCH
            };
            for ch in connector.chars() {
                if x >= x_max {
                    return;
                }
                if let Some(cell) = buf.cell_mut((x, y)) {
                    cell.set_char(ch);
                    cell.set_style(tree_style);
                }
                x += 1;
            }

            // Space after connector
            if x < x_max {
                if let Some(cell) = buf.cell_mut((x, y)) {
                    cell.set_char(' ');
                    cell.set_style(bg_style);
                }
                x += 1;
            }
        } else {
            // Root agent: draw ━━ prefix
            for _ in 0..2 {
                if x >= x_max {
                    return;
                }
                if let Some(cell) = buf.cell_mut((x, y)) {
                    cell.set_symbol(HORIZONTAL);
                    cell.set_style(rule_style);
                }
                x += 1;
            }
            if x < x_max {
                if let Some(cell) = buf.cell_mut((x, y)) {
                    cell.set_char(' ');
                    cell.set_style(bg_style);
                }
                x += 1;
            }
        }

        // Status icon
        let icon = self.status.icon();
        for ch in icon.chars() {
            if x >= x_max {
                return;
            }
            if let Some(cell) = buf.cell_mut((x, y)) {
                cell.set_char(ch);
                cell.set_style(icon_style);
            }
            x += 1;
        }

        // Space
        if x < x_max {
            if let Some(cell) = buf.cell_mut((x, y)) {
                cell.set_char(' ');
                cell.set_style(bg_style);
            }
            x += 1;
        }

        // Label
        for ch in self.label.chars() {
            if x >= x_max {
                return;
            }
            if let Some(cell) = buf.cell_mut((x, y)) {
                cell.set_char(ch);
                cell.set_style(label_style);
            }
            x += 1;
        }

        // Stats in parentheses
        let mut stats_parts: Vec<String> = Vec::new();
        if let Some(d) = self.duration {
            stats_parts.push(format_duration(d));
        }
        if self.token_count > 0 {
            stats_parts.push(format_tokens(self.token_count));
        }

        if !stats_parts.is_empty() {
            let stats_str = format!(" ({})", stats_parts.join(" \u{00B7} "));
            for ch in stats_str.chars() {
                if x >= x_max {
                    return;
                }
                if let Some(cell) = buf.cell_mut((x, y)) {
                    cell.set_char(ch);
                    cell.set_style(stats_style);
                }
                x += 1;
            }
        }

        // Space
        if x < x_max {
            if let Some(cell) = buf.cell_mut((x, y)) {
                cell.set_char(' ');
                cell.set_style(bg_style);
            }
            x += 1;
        }

        // Fill remainder with ━ (for root) or space (for children)
        if self.depth == 0 {
            while x < x_max {
                if let Some(cell) = buf.cell_mut((x, y)) {
                    cell.set_symbol(HORIZONTAL);
                    cell.set_style(rule_style);
                }
                x += 1;
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
    fn agent_status_icons() {
        assert_eq!(AgentStatus::Active.icon(), "●");
        assert_eq!(AgentStatus::Done.icon(), "✓");
        assert_eq!(AgentStatus::Error.icon(), "✗");
        assert_eq!(AgentStatus::Cancelled.icon(), "—");
    }

    #[test]
    fn format_duration_seconds() {
        assert_eq!(format_duration(Duration::from_secs(12)), "12s");
    }

    #[test]
    fn format_duration_minutes() {
        assert_eq!(format_duration(Duration::from_secs(120)), "2m");
    }

    #[test]
    fn format_duration_hours() {
        assert_eq!(format_duration(Duration::from_secs(7200)), "2h");
    }

    #[test]
    fn format_tokens_small() {
        assert_eq!(format_tokens(500), "500");
    }

    #[test]
    fn format_tokens_thousands() {
        assert_eq!(format_tokens(2100), "2.1k");
    }

    #[test]
    fn format_tokens_large() {
        assert_eq!(format_tokens(14500), "14k");
    }

    #[test]
    fn format_tokens_millions() {
        assert_eq!(format_tokens(1_500_000), "1.5M");
    }

    #[test]
    fn header_required_height() {
        let header = AgentScopeHeader::new(test_theme(), "test".into());
        assert_eq!(header.required_height(), 1);
    }

    #[test]
    fn header_with_padding_height() {
        let header =
            AgentScopeHeader::new(test_theme(), "test".into()).padding(Padding::symmetric(0, 1));
        assert_eq!(header.required_height(), 3);
    }

    #[test]
    fn header_text_basic() {
        let header = AgentScopeHeader::new(test_theme(), "research".into());
        assert_eq!(header.header_text(), "● research");
    }

    #[test]
    fn header_text_with_stats() {
        let header = AgentScopeHeader::new(test_theme(), "research".into())
            .duration(Duration::from_secs(12))
            .token_count(2100);
        assert_eq!(header.header_text(), "● research (12s · 2.1k)");
    }

    #[test]
    fn header_text_done() {
        let header = AgentScopeHeader::new(test_theme(), "bugfix".into())
            .status(AgentStatus::Done)
            .duration(Duration::from_secs(8))
            .token_count(800);
        assert_eq!(header.header_text(), "✓ bugfix (8s · 800)");
    }

    #[test]
    fn render_does_not_panic_zero_area() {
        let header = AgentScopeHeader::new(test_theme(), "test".into());
        let area = Rect::new(0, 0, 0, 0);
        let mut buf = Buffer::empty(Rect::new(0, 0, 1, 1));
        (&header).render(area, &mut buf);
    }

    #[test]
    fn render_root_header() {
        let header = AgentScopeHeader::new(test_theme(), "main".into()).status(AgentStatus::Active);
        let area = Rect::new(0, 0, 30, 1);
        let mut buf = Buffer::empty(area);
        (&header).render(area, &mut buf);
        // First two chars should be ━━
        assert_eq!(buf[(0, 0)].symbol(), HORIZONTAL);
        assert_eq!(buf[(1, 0)].symbol(), HORIZONTAL);
    }

    #[test]
    fn render_child_header() {
        let header = AgentScopeHeader::new(test_theme(), "research".into())
            .depth(1)
            .is_last_sibling(false);
        let area = Rect::new(0, 0, 30, 1);
        let mut buf = Buffer::empty(area);
        (&header).render(area, &mut buf);
        // Should start with ├─
        assert_eq!(buf[(0, 0)].symbol(), "├");
    }

    #[test]
    fn render_last_child_header() {
        let header = AgentScopeHeader::new(test_theme(), "bugfix".into())
            .depth(1)
            .is_last_sibling(true);
        let area = Rect::new(0, 0, 30, 1);
        let mut buf = Buffer::empty(area);
        (&header).render(area, &mut buf);
        // Should start with └─
        assert_eq!(buf[(0, 0)].symbol(), "└");
    }

    #[test]
    fn display_mode_equality() {
        assert_eq!(DisplayMode::Full, DisplayMode::Full);
        assert_ne!(DisplayMode::Full, DisplayMode::Compact);
    }
}
