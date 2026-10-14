use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::{Color, Modifier, Style};
use ratatui::widgets::Widget;

use crate::theme::Theme;
use crate::widgets::draw::draw_text;
use crate::widgets::padding::Padding;

/// Agent operating mode, cycled by Tab.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Mode {
    /// Talk-only: conversation without changes.
    Talk,
    /// Planning: reason about approach, don't execute.
    Plan,
    /// Accept: act with explicit permission per action.
    Accept,
    /// Auto: act autonomously within guardrails.
    Auto,
    /// Ralph: no limits, full send.
    Ralph,
}

impl Mode {
    pub fn label(&self) -> &'static str {
        match self {
            Mode::Talk => "Talk",
            Mode::Plan => "Plan",
            Mode::Accept => "Accept",
            Mode::Auto => "Auto",
            Mode::Ralph => "Ralph",
        }
    }

    pub fn color(&self, theme: &Theme) -> Color {
        match self {
            Mode::Talk => theme.info,        // cyan
            Mode::Plan => theme.primary,     // orange
            Mode::Accept => theme.secondary, // blue
            Mode::Auto => theme.success,     // green
            Mode::Ralph => theme.accent,     // purple
        }
    }

    pub fn next(&self) -> Self {
        match self {
            Mode::Talk => Mode::Plan,
            Mode::Plan => Mode::Accept,
            Mode::Accept => Mode::Auto,
            Mode::Auto => Mode::Ralph,
            Mode::Ralph => Mode::Talk,
        }
    }
}

/// Cached context-window usage derived from message content.
#[derive(Debug, Clone)]
pub struct ContextState {
    word_count: usize,
    context_window: usize,
}

impl ContextState {
    pub fn new(context_window: usize) -> Self {
        Self {
            word_count: 0,
            context_window,
        }
    }

    /// Replace the cached word count.
    pub fn set_word_count(&mut self, count: usize) {
        self.word_count = count;
    }

    pub fn word_count(&self) -> usize {
        self.word_count
    }

    pub fn usage_percent(&self) -> f64 {
        if self.context_window == 0 {
            return 0.0;
        }
        (self.word_count as f64 / self.context_window as f64) * 100.0
    }
}

/// A single-row status bar with Mode, Subscription, CWD, and Context segments.
pub struct StatusBar {
    mode: Mode,
    context: ContextState,
    cwd: String,
    padding: Padding,
    /// Optional spinner frame + verb shown after Mode (e.g. "🌓 Thinking...").
    /// Set to empty string when inactive.
    activity: String,
    /// Latest request latency in milliseconds, if known.
    latency_ms: Option<u32>,
    theme: Theme,
}

impl StatusBar {
    pub fn new(theme: Theme, context_window: usize) -> Self {
        Self {
            mode: Mode::Talk,
            context: ContextState::new(context_window),
            cwd: String::new(),
            padding: Padding::uniform(0),
            activity: String::new(),
            latency_ms: None,
            theme,
        }
    }

    /// Set the activity text shown after Mode (e.g. "🌓 Pondering...").
    /// Pass an empty string to clear.
    pub fn set_activity(&mut self, activity: String) {
        self.activity = activity;
    }

    pub fn set_padding(&mut self, padding: Padding) {
        self.padding = padding;
    }

    /// Total height including padding.
    pub fn height(&self) -> u16 {
        1 + self.padding.vertical()
    }

    pub fn mode(&self) -> Mode {
        self.mode
    }

    /// Advance to the next mode. Returns the new mode's color.
    pub fn cycle_mode(&mut self) -> Color {
        self.mode = self.mode.next();
        self.mode.color(&self.theme)
    }

    pub fn context_mut(&mut self) -> &mut ContextState {
        &mut self.context
    }

    pub fn set_cwd(&mut self, cwd: String) {
        self.cwd = cwd;
    }

    pub fn set_latency_ms(&mut self, ms: u32) {
        self.latency_ms = Some(ms);
    }

    /// Shorten the cwd for display: replace home dir with ~, keep last N path components.
    fn display_cwd(&self) -> &str {
        if self.cwd.is_empty() {
            return "";
        }
        // Show the path starting from ~ or the last 3 components
        &self.cwd
    }
}

impl Widget for &StatusBar {
    fn render(self, area: Rect, buf: &mut Buffer) {
        if area.width == 0 || area.height == 0 {
            return;
        }

        let bg_style = Style::default().bg(self.theme.bg);
        let sep = " · ";
        let sep_style = Style::default().fg(self.theme.text_muted).bg(self.theme.bg);
        let muted_style = Style::default().fg(self.theme.text_muted).bg(self.theme.bg);

        // Fill entire background
        for y in area.y..area.y + area.height {
            for x in area.x..area.x + area.width {
                if let Some(cell) = buf.cell_mut((x, y)) {
                    cell.set_char(' ');
                    cell.set_style(bg_style);
                }
            }
        }

        // The text row sits inside the padding
        let text_y = area.y + self.padding.top;
        if text_y >= area.y + area.height {
            return;
        }

        // --- Left segments ---
        let mode_label = self.mode.label();
        let mode_style = Style::default()
            .fg(self.mode.color(&self.theme))
            .bg(self.theme.bg)
            .add_modifier(Modifier::BOLD);

        let model = "GLM-5.1";
        let model_style = Style::default()
            .fg(self.theme.text)
            .bg(self.theme.bg)
            .add_modifier(Modifier::BOLD);

        // let plan = "Z.ai Coding Plan";

        let activity_style = Style::default().fg(self.theme.text).bg(self.theme.bg);

        let mut left_parts: Vec<(&str, Style)> = vec![(mode_label, mode_style)];

        if !self.activity.is_empty() {
            left_parts.push((sep, sep_style));
            left_parts.push((&self.activity, activity_style));
        }

        left_parts.push((sep, sep_style));
        left_parts.push((model, model_style));
        // left_parts.push((sep, sep_style));
        // left_parts.push((plan, muted_style));

        let mut x = area.x + self.padding.left;
        let x_max = area.x + area.width.saturating_sub(self.padding.right);

        for (text, style) in &left_parts {
            for ch in text.chars() {
                if x >= x_max {
                    return;
                }
                if let Some(cell) = buf.cell_mut((x, text_y)) {
                    cell.set_char(ch);
                    cell.set_style(*style);
                }
                x += 1;
            }
        }

        // --- Right segments: "cwd · 100ms · Nk tokens (N.N%)" ---
        // Graceful degradation: try full text, then drop cwd, then drop latency, then nothing.
        let tokens = format_token_count(self.context.word_count());
        let pct = self.context.usage_percent();
        let context_text = format!("{tokens} tokens ({pct:.1}%)");
        let latency_text = self.latency_ms.map(|ms| format!("{ms}ms"));
        let display_cwd = self.display_cwd();

        // Build right-side segments from most to least expendable
        let mut parts: Vec<&str> = Vec::new();
        if !display_cwd.is_empty() {
            parts.push(display_cwd);
        }
        if let Some(ref lt) = latency_text {
            parts.push(lt);
        }
        parts.push(&context_text);

        let right_text = loop {
            let candidate = parts.join(sep);
            if x_max.saturating_sub(x) > candidate.len() as u16 {
                break candidate;
            }
            // Drop the first (leftmost) segment to fit
            if parts.len() > 1 {
                parts.remove(0);
            } else {
                return; // nothing fits
            }
        };

        let right_len = right_text.len() as u16;
        let right_start = x_max.saturating_sub(right_len);

        // Draw right side in muted style with separators
        draw_text(buf, right_start, text_y, x_max, &right_text, muted_style);
    }
}

fn format_token_count(count: usize) -> String {
    if count >= 1_000_000 {
        format!("{:.1}m", count as f64 / 1_000_000.0)
    } else if count >= 1_000 {
        format!("{:.1}k", count as f64 / 1_000.0)
    } else {
        count.to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn mode_cycles_through_all_five() {
        let mut mode = Mode::Talk;
        mode = mode.next();
        assert_eq!(mode, Mode::Plan);
        mode = mode.next();
        assert_eq!(mode, Mode::Accept);
        mode = mode.next();
        assert_eq!(mode, Mode::Auto);
        mode = mode.next();
        assert_eq!(mode, Mode::Ralph);
        mode = mode.next();
        assert_eq!(mode, Mode::Talk);
    }

    #[test]
    fn context_state_percentage() {
        let mut ctx = ContextState::new(200_000);
        ctx.set_word_count(1_000);
        assert!((ctx.usage_percent() - 0.5).abs() < 0.01);
    }

    #[test]
    fn context_state_zero_window() {
        let ctx = ContextState::new(0);
        assert_eq!(ctx.usage_percent(), 0.0);
    }

    #[test]
    fn format_tokens() {
        assert_eq!(format_token_count(0), "0");
        assert_eq!(format_token_count(500), "500");
        assert_eq!(format_token_count(1_234), "1.2k");
        assert_eq!(format_token_count(12_345), "12.3k");
        assert_eq!(format_token_count(1_234_567), "1.2m");
    }

    #[test]
    fn cycle_mode_returns_color() {
        let theme = Theme::default();
        let mut sb = StatusBar::new(theme, 200_000);
        assert_eq!(sb.mode(), Mode::Talk);
        let color = sb.cycle_mode();
        assert_eq!(sb.mode(), Mode::Plan);
        assert_eq!(color, theme.primary);
    }

    #[test]
    fn renders_without_panic() {
        let theme = Theme::default();
        let sb = StatusBar::new(theme, 200_000);
        let area = Rect::new(0, 0, 80, 1);
        let mut buf = Buffer::empty(area);
        (&sb).render(area, &mut buf);
    }

    #[test]
    fn renders_with_cwd() {
        let theme = Theme::default();
        let mut sb = StatusBar::new(theme, 200_000);
        sb.set_cwd("~/projects/foo".to_string());
        let area = Rect::new(0, 0, 80, 1);
        let mut buf = Buffer::empty(area);
        (&sb).render(area, &mut buf);
    }

    #[test]
    fn renders_narrow_without_panic() {
        let theme = Theme::default();
        let mut sb = StatusBar::new(theme, 200_000);
        sb.set_cwd("~/projects/foo".to_string());
        let area = Rect::new(0, 0, 20, 1);
        let mut buf = Buffer::empty(area);
        (&sb).render(area, &mut buf);
    }
}
