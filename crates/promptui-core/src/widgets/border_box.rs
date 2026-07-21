use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::{Color, Style};

use crate::theme::Theme;
use crate::widgets::padding::Padding;

/// Left border character matching the existing identity pattern.
const LEFT_BAR: &str = "\u{2503}"; // ┃

// Full box-drawing characters
const FULL_TOP_LEFT: &str = "\u{250C}"; // ┌
const FULL_TOP_RIGHT: &str = "\u{2510}"; // ┐
const FULL_BOTTOM_LEFT: &str = "\u{2514}"; // └
const FULL_BOTTOM_RIGHT: &str = "\u{2518}"; // ┘
const FULL_HORIZONTAL: &str = "\u{2500}"; // ─
const FULL_VERTICAL: &str = "\u{2502}"; // │

// Rounded box-drawing characters
const ROUND_TOP_LEFT: &str = "\u{256D}"; // ╭
const ROUND_TOP_RIGHT: &str = "\u{256E}"; // ╮
const ROUND_BOTTOM_LEFT: &str = "\u{2570}"; // ╰
const ROUND_BOTTOM_RIGHT: &str = "\u{256F}"; // ╯

/// Border style for a [`BorderBox`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BorderStyle {
    /// Just a left ┃ bar (matches existing MessageList/TextField identity pattern).
    LeftBar,
    /// Full box: ┌─┐│ │└─┘
    Full,
    /// Rounded box: ╭─╮│ │╰─╯
    Rounded,
}

/// A generic bordered container that renders chrome around an inner area.
///
/// `BorderBox` does **not** own its content. Instead it provides
/// [`inner_area`](Self::inner_area) so the caller can render content
/// into the remaining space after borders and padding are accounted for.
///
/// # Usage
///
/// ```ignore
/// let bbox = BorderBox::new(theme, BorderStyle::Rounded)
///     .border_color(theme.accent)
///     .title("Tool: bash".into());
///
/// bbox.render_chrome(area, buf);
/// my_content.render(bbox.inner_area(area), buf);
/// ```
pub struct BorderBox {
    theme: Theme,
    padding: Padding,
    border_style: BorderStyle,
    border_color: Color,
    title: Option<String>,
}

impl BorderBox {
    pub fn new(theme: Theme, border_style: BorderStyle) -> Self {
        Self {
            border_color: theme.border,
            theme,
            padding: Padding::uniform(0),
            border_style,
            title: None,
        }
    }

    pub fn border_color(mut self, color: Color) -> Self {
        self.border_color = color;
        self
    }

    pub fn title(mut self, title: String) -> Self {
        self.title = Some(title);
        self
    }

    pub fn padding(mut self, padding: Padding) -> Self {
        self.padding = padding;
        self
    }

    pub fn set_padding(&mut self, padding: Padding) {
        self.padding = padding;
    }

    pub fn set_border_color(&mut self, color: Color) {
        self.border_color = color;
    }

    pub fn set_title(&mut self, title: Option<String>) {
        self.title = title;
    }

    /// Horizontal overhead in columns (border + padding on each side).
    pub fn h_overhead(&self) -> u16 {
        match self.border_style {
            BorderStyle::LeftBar => 1 + self.padding.horizontal(),
            BorderStyle::Full | BorderStyle::Rounded => 2 + self.padding.horizontal(),
        }
    }

    /// Vertical overhead in rows (border + padding on top and bottom).
    pub fn v_overhead(&self) -> u16 {
        match self.border_style {
            BorderStyle::LeftBar => self.padding.vertical(),
            BorderStyle::Full | BorderStyle::Rounded => 2 + self.padding.vertical(),
        }
    }

    /// Compute the inner area available for content after borders and padding.
    pub fn inner_area(&self, outer: Rect) -> Rect {
        let (left_border, right_border, top_border, bottom_border) = match self.border_style {
            BorderStyle::LeftBar => (1u16, 0u16, 0u16, 0u16),
            BorderStyle::Full | BorderStyle::Rounded => (1, 1, 1, 1),
        };

        let x = outer.x + left_border + self.padding.left;
        let y = outer.y + top_border + self.padding.top;
        let width = outer
            .width
            .saturating_sub(left_border + right_border + self.padding.horizontal());
        let height = outer
            .height
            .saturating_sub(top_border + bottom_border + self.padding.vertical());

        Rect::new(x, y, width, height)
    }

    /// Render just the border chrome (borders, title, background fill).
    ///
    /// Call this first, then render your content into [`inner_area`](Self::inner_area).
    pub fn render_chrome(&self, area: Rect, buf: &mut Buffer) {
        if area.width == 0 || area.height == 0 {
            return;
        }

        match self.border_style {
            BorderStyle::LeftBar => self.render_left_bar(area, buf),
            BorderStyle::Full => self.render_box(area, buf, false),
            BorderStyle::Rounded => self.render_box(area, buf, true),
        }
    }

    fn render_left_bar(&self, area: Rect, buf: &mut Buffer) {
        let border_style = Style::default()
            .fg(self.border_color)
            .bg(self.theme.panel_bg);
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

        // Draw left border
        for y in area.y..area.y + area.height {
            if let Some(cell) = buf.cell_mut((area.x, y)) {
                cell.set_symbol(LEFT_BAR);
                cell.set_style(border_style);
            }
        }
    }

    fn render_box(&self, area: Rect, buf: &mut Buffer, rounded: bool) {
        if area.height < 2 || area.width < 2 {
            return;
        }

        let border_style = Style::default()
            .fg(self.border_color)
            .bg(self.theme.panel_bg);
        let bg_style = Style::default().bg(self.theme.panel_bg);

        let (tl, tr, bl, br) = if rounded {
            (
                ROUND_TOP_LEFT,
                ROUND_TOP_RIGHT,
                ROUND_BOTTOM_LEFT,
                ROUND_BOTTOM_RIGHT,
            )
        } else {
            (
                FULL_TOP_LEFT,
                FULL_TOP_RIGHT,
                FULL_BOTTOM_LEFT,
                FULL_BOTTOM_RIGHT,
            )
        };

        // Fill background
        for y in area.y..area.y + area.height {
            for x in area.x..area.x + area.width {
                if let Some(cell) = buf.cell_mut((x, y)) {
                    cell.set_char(' ');
                    cell.set_style(bg_style);
                }
            }
        }

        let top = area.y;
        let bottom = area.y + area.height - 1;
        let left = area.x;
        let right = area.x + area.width - 1;

        // Corners
        if let Some(cell) = buf.cell_mut((left, top)) {
            cell.set_symbol(tl);
            cell.set_style(border_style);
        }
        if let Some(cell) = buf.cell_mut((right, top)) {
            cell.set_symbol(tr);
            cell.set_style(border_style);
        }
        if let Some(cell) = buf.cell_mut((left, bottom)) {
            cell.set_symbol(bl);
            cell.set_style(border_style);
        }
        if let Some(cell) = buf.cell_mut((right, bottom)) {
            cell.set_symbol(br);
            cell.set_style(border_style);
        }

        // Top and bottom horizontal lines
        for x in (left + 1)..right {
            if let Some(cell) = buf.cell_mut((x, top)) {
                cell.set_symbol(FULL_HORIZONTAL);
                cell.set_style(border_style);
            }
            if let Some(cell) = buf.cell_mut((x, bottom)) {
                cell.set_symbol(FULL_HORIZONTAL);
                cell.set_style(border_style);
            }
        }

        // Left and right vertical lines
        for y in (top + 1)..bottom {
            if let Some(cell) = buf.cell_mut((left, y)) {
                cell.set_symbol(FULL_VERTICAL);
                cell.set_style(border_style);
            }
            if let Some(cell) = buf.cell_mut((right, y)) {
                cell.set_symbol(FULL_VERTICAL);
                cell.set_style(border_style);
            }
        }

        // Title on top row (after corner)
        if let Some(ref title) = self.title {
            let title_style = Style::default().fg(self.theme.text).bg(self.theme.panel_bg);
            let sep_style = Style::default()
                .fg(self.border_color)
                .bg(self.theme.panel_bg);

            // " Title " with ─ separator before
            let max_title_width = (area.width as usize).saturating_sub(4); // corner + space + space + corner
            let display_title: String = if title.len() > max_title_width {
                title.chars().take(max_title_width).collect()
            } else {
                title.clone()
            };

            let mut x = left + 1;

            // ─ before title
            if let Some(cell) = buf.cell_mut((x, top)) {
                cell.set_symbol(FULL_HORIZONTAL);
                cell.set_style(sep_style);
            }
            x += 1;

            // space before title
            if x < right {
                if let Some(cell) = buf.cell_mut((x, top)) {
                    cell.set_char(' ');
                    cell.set_style(title_style);
                }
                x += 1;
            }

            // title text
            for ch in display_title.chars() {
                if x >= right {
                    break;
                }
                if let Some(cell) = buf.cell_mut((x, top)) {
                    cell.set_char(ch);
                    cell.set_style(title_style);
                }
                x += 1;
            }

            // space after title
            if x < right
                && let Some(cell) = buf.cell_mut((x, top))
            {
                cell.set_char(' ');
                cell.set_style(title_style);
                // remaining ─ already rendered in the top line loop
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
    fn left_bar_overhead() {
        let bbox = BorderBox::new(test_theme(), BorderStyle::LeftBar);
        assert_eq!(bbox.h_overhead(), 1);
        assert_eq!(bbox.v_overhead(), 0);
    }

    #[test]
    fn left_bar_with_padding_overhead() {
        let bbox =
            BorderBox::new(test_theme(), BorderStyle::LeftBar).padding(Padding::symmetric(1, 1));
        assert_eq!(bbox.h_overhead(), 3); // 1 border + 1 left pad + 1 right pad
        assert_eq!(bbox.v_overhead(), 2); // 1 top pad + 1 bottom pad
    }

    #[test]
    fn full_box_overhead() {
        let bbox = BorderBox::new(test_theme(), BorderStyle::Full);
        assert_eq!(bbox.h_overhead(), 2);
        assert_eq!(bbox.v_overhead(), 2);
    }

    #[test]
    fn rounded_box_overhead() {
        let bbox = BorderBox::new(test_theme(), BorderStyle::Rounded);
        assert_eq!(bbox.h_overhead(), 2);
        assert_eq!(bbox.v_overhead(), 2);
    }

    #[test]
    fn inner_area_left_bar() {
        let bbox = BorderBox::new(test_theme(), BorderStyle::LeftBar);
        let outer = Rect::new(0, 0, 40, 10);
        let inner = bbox.inner_area(outer);
        assert_eq!(inner, Rect::new(1, 0, 39, 10));
    }

    #[test]
    fn inner_area_full_box() {
        let bbox = BorderBox::new(test_theme(), BorderStyle::Full);
        let outer = Rect::new(0, 0, 40, 10);
        let inner = bbox.inner_area(outer);
        assert_eq!(inner, Rect::new(1, 1, 38, 8));
    }

    #[test]
    fn inner_area_with_padding() {
        let bbox =
            BorderBox::new(test_theme(), BorderStyle::Rounded).padding(Padding::symmetric(1, 1));
        let outer = Rect::new(0, 0, 40, 10);
        let inner = bbox.inner_area(outer);
        assert_eq!(inner, Rect::new(2, 2, 36, 6));
    }

    #[test]
    fn inner_area_too_small() {
        let bbox = BorderBox::new(test_theme(), BorderStyle::Full);
        let outer = Rect::new(0, 0, 2, 2);
        let inner = bbox.inner_area(outer);
        assert_eq!(inner.width, 0);
        assert_eq!(inner.height, 0);
    }

    #[test]
    fn render_chrome_zero_area() {
        let bbox = BorderBox::new(test_theme(), BorderStyle::Full);
        let area = Rect::new(0, 0, 0, 0);
        let mut buf = Buffer::empty(Rect::new(0, 0, 1, 1));
        bbox.render_chrome(area, &mut buf); // should not panic
    }

    #[test]
    fn render_left_bar_draws_border() {
        let bbox = BorderBox::new(test_theme(), BorderStyle::LeftBar);
        let area = Rect::new(0, 0, 10, 3);
        let mut buf = Buffer::empty(area);
        bbox.render_chrome(area, &mut buf);
        // First column should be ┃ on every row
        for y in 0..3 {
            let cell = &buf[(0, y)];
            assert_eq!(cell.symbol(), LEFT_BAR);
        }
    }

    #[test]
    fn render_full_box_draws_corners() {
        let bbox = BorderBox::new(test_theme(), BorderStyle::Full);
        let area = Rect::new(0, 0, 10, 4);
        let mut buf = Buffer::empty(area);
        bbox.render_chrome(area, &mut buf);
        assert_eq!(buf[(0, 0)].symbol(), FULL_TOP_LEFT);
        assert_eq!(buf[(9, 0)].symbol(), FULL_TOP_RIGHT);
        assert_eq!(buf[(0, 3)].symbol(), FULL_BOTTOM_LEFT);
        assert_eq!(buf[(9, 3)].symbol(), FULL_BOTTOM_RIGHT);
    }

    #[test]
    fn render_rounded_box_draws_corners() {
        let bbox = BorderBox::new(test_theme(), BorderStyle::Rounded);
        let area = Rect::new(0, 0, 10, 4);
        let mut buf = Buffer::empty(area);
        bbox.render_chrome(area, &mut buf);
        assert_eq!(buf[(0, 0)].symbol(), ROUND_TOP_LEFT);
        assert_eq!(buf[(9, 0)].symbol(), ROUND_TOP_RIGHT);
        assert_eq!(buf[(0, 3)].symbol(), ROUND_BOTTOM_LEFT);
        assert_eq!(buf[(9, 3)].symbol(), ROUND_BOTTOM_RIGHT);
    }

    #[test]
    fn render_box_with_title() {
        let bbox = BorderBox::new(test_theme(), BorderStyle::Full).title("Test".into());
        let area = Rect::new(0, 0, 20, 4);
        let mut buf = Buffer::empty(area);
        bbox.render_chrome(area, &mut buf);
        // Title should appear on the top row
        // Position: corner(0) + ─(1) + space(2) + T(3) + e(4) + s(5) + t(6)
        assert_eq!(buf[(3, 0)].symbol(), "T");
        assert_eq!(buf[(4, 0)].symbol(), "e");
        assert_eq!(buf[(5, 0)].symbol(), "s");
        assert_eq!(buf[(6, 0)].symbol(), "t");
    }
}
