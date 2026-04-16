use ratatui::style::{Color, Style};

/// Semantic color palette inspired by OpenCode's terminal TUI.
///
/// All colors default to the OpenCode dark theme values.
#[derive(Debug, Clone, Copy)]
pub struct Theme {
    /// Main window background (#0a0a0a)
    pub bg: Color,
    /// Panel/message area background (#141414)
    pub panel_bg: Color,
    /// Input field / element background (#1e1e1e)
    pub element_bg: Color,
    /// Primary text (#eeeeee)
    pub text: Color,
    /// Muted/secondary text (#808080)
    pub text_muted: Color,
    /// Primary accent - orange (#fab283)
    pub primary: Color,
    /// Secondary accent - blue (#5c9cf5)
    pub secondary: Color,
    /// Tertiary accent - purple (#9d7cd8)
    pub accent: Color,
    /// Error color - red (#e06c75)
    pub error: Color,
    /// Warning color - orange (#f5a742)
    pub warning: Color,
    /// Success color - green (#7fd88f)
    pub success: Color,
    /// Info color - cyan (#56b6c2)
    pub info: Color,
    /// Default border color (#303030)
    pub border: Color,
}

impl Default for Theme {
    fn default() -> Self {
        Self {
            bg: Color::Rgb(10, 10, 10),
            panel_bg: Color::Rgb(20, 20, 20),
            element_bg: Color::Rgb(30, 30, 30),
            text: Color::Rgb(238, 238, 238),
            text_muted: Color::Rgb(128, 128, 128),
            primary: Color::Rgb(250, 178, 131),
            secondary: Color::Rgb(92, 156, 245),
            accent: Color::Rgb(157, 124, 216),
            error: Color::Rgb(224, 108, 117),
            warning: Color::Rgb(245, 167, 66),
            success: Color::Rgb(127, 216, 143),
            info: Color::Rgb(86, 182, 194),
            border: Color::Rgb(48, 48, 48),
        }
    }
}

impl Theme {
    /// Style for primary text on panel background.
    pub fn text_style(&self) -> Style {
        Style::default().fg(self.text).bg(self.panel_bg)
    }

    /// Style for muted text on panel background.
    pub fn muted_style(&self) -> Style {
        Style::default().fg(self.text_muted).bg(self.panel_bg)
    }

    /// Style for text inside an input field.
    pub fn input_style(&self) -> Style {
        Style::default().fg(self.text).bg(self.element_bg)
    }

    /// Style for the focused left border.
    pub fn border_focused_style(&self) -> Style {
        Style::default().fg(self.primary).bg(self.element_bg)
    }

    /// Style for the unfocused left border.
    pub fn border_unfocused_style(&self) -> Style {
        Style::default().fg(self.border).bg(self.element_bg)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_theme_has_opencode_colors() {
        let theme = Theme::default();
        assert_eq!(theme.bg, Color::Rgb(10, 10, 10));
        assert_eq!(theme.primary, Color::Rgb(250, 178, 131));
        assert_eq!(theme.text, Color::Rgb(238, 238, 238));
    }
}
