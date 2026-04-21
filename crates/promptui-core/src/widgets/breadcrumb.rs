use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::{Color, Modifier, Style};
use ratatui::widgets::Widget;

use crate::theme::Theme;
use crate::widgets::padding::Padding;

/// Separator between breadcrumb segments.
const SEPARATOR: &str = " \u{203A} "; // " › "

/// A segment in the breadcrumb trail.
#[derive(Debug, Clone)]
pub struct BreadcrumbSegment {
    /// Display label.
    pub label: String,
    /// Color for this segment.
    pub color: Color,
    /// Whether this is the currently active (focused) scope.
    pub active: bool,
}

impl BreadcrumbSegment {
    pub fn new(label: String, color: Color) -> Self {
        Self {
            label,
            color,
            active: false,
        }
    }

    pub fn active(mut self, active: bool) -> Self {
        self.active = active;
        self
    }
}

/// A navigation trail showing the path from root to focused agent scope.
///
/// Renders as a single row: `main › research › deep-analysis`
///
/// Hidden when depth <= 1 (returns height 0 for single-agent scenarios).
pub struct Breadcrumb {
    theme: Theme,
    padding: Padding,
    segments: Vec<BreadcrumbSegment>,
}

impl Breadcrumb {
    pub fn new(theme: Theme) -> Self {
        Self {
            theme,
            padding: Padding::uniform(0),
            segments: Vec::new(),
        }
    }

    pub fn set_padding(&mut self, padding: Padding) {
        self.padding = padding;
    }

    /// Push a new segment onto the trail.
    pub fn push(&mut self, segment: BreadcrumbSegment) {
        // Deactivate all existing segments
        for s in &mut self.segments {
            s.active = false;
        }
        self.segments.push(segment);
        // The last segment is always active
        if let Some(last) = self.segments.last_mut() {
            last.active = true;
        }
    }

    /// Pop the last segment from the trail.
    pub fn pop(&mut self) -> Option<BreadcrumbSegment> {
        let popped = self.segments.pop();
        // Mark the new last segment as active
        if let Some(last) = self.segments.last_mut() {
            last.active = true;
        }
        popped
    }

    /// Replace all segments.
    pub fn set_segments(&mut self, segments: Vec<BreadcrumbSegment>) {
        self.segments = segments;
    }

    /// Current depth (number of segments).
    pub fn depth(&self) -> usize {
        self.segments.len()
    }

    /// Total height needed. Returns 0 when depth <= 1 (invisible).
    pub fn height(&self) -> u16 {
        if self.segments.len() <= 1 {
            0
        } else {
            1 + self.padding.vertical()
        }
    }

    /// Get the segments.
    pub fn segments(&self) -> &[BreadcrumbSegment] {
        &self.segments
    }
}

impl Widget for &Breadcrumb {
    fn render(self, area: Rect, buf: &mut Buffer) {
        if area.width == 0 || area.height == 0 || self.segments.len() <= 1 {
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

        let separator_style = self.theme.muted_style();

        for (i, segment) in self.segments.iter().enumerate() {
            // Separator before non-first segments
            if i > 0 {
                for ch in SEPARATOR.chars() {
                    if x >= x_max {
                        return;
                    }
                    if let Some(cell) = buf.cell_mut((x, y)) {
                        cell.set_char(ch);
                        cell.set_style(separator_style);
                    }
                    x += 1;
                }
            }

            // Segment label
            let label_style = if segment.active {
                Style::default()
                    .fg(segment.color)
                    .bg(self.theme.panel_bg)
                    .add_modifier(Modifier::BOLD)
            } else {
                Style::default().fg(segment.color).bg(self.theme.panel_bg)
            };

            for ch in segment.label.chars() {
                if x >= x_max {
                    return;
                }
                if let Some(cell) = buf.cell_mut((x, y)) {
                    cell.set_char(ch);
                    cell.set_style(label_style);
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
    fn empty_breadcrumb_height_zero() {
        let bc = Breadcrumb::new(test_theme());
        assert_eq!(bc.height(), 0);
        assert_eq!(bc.depth(), 0);
    }

    #[test]
    fn single_segment_height_zero() {
        let mut bc = Breadcrumb::new(test_theme());
        bc.push(BreadcrumbSegment::new("main".into(), Color::White));
        assert_eq!(bc.height(), 0);
        assert_eq!(bc.depth(), 1);
    }

    #[test]
    fn two_segments_has_height() {
        let mut bc = Breadcrumb::new(test_theme());
        bc.push(BreadcrumbSegment::new("main".into(), Color::White));
        bc.push(BreadcrumbSegment::new("research".into(), Color::Blue));
        assert_eq!(bc.height(), 1);
        assert_eq!(bc.depth(), 2);
    }

    #[test]
    fn push_sets_last_active() {
        let mut bc = Breadcrumb::new(test_theme());
        bc.push(BreadcrumbSegment::new("main".into(), Color::White));
        assert!(bc.segments()[0].active);

        bc.push(BreadcrumbSegment::new("research".into(), Color::Blue));
        assert!(!bc.segments()[0].active);
        assert!(bc.segments()[1].active);
    }

    #[test]
    fn pop_updates_active() {
        let mut bc = Breadcrumb::new(test_theme());
        bc.push(BreadcrumbSegment::new("main".into(), Color::White));
        bc.push(BreadcrumbSegment::new("research".into(), Color::Blue));

        let popped = bc.pop();
        assert!(popped.is_some());
        assert_eq!(popped.unwrap().label, "research");
        assert_eq!(bc.depth(), 1);
        assert!(bc.segments()[0].active);
    }

    #[test]
    fn render_does_not_panic_single_segment() {
        let mut bc = Breadcrumb::new(test_theme());
        bc.push(BreadcrumbSegment::new("main".into(), Color::White));
        let area = Rect::new(0, 0, 40, 1);
        let mut buf = Buffer::empty(area);
        (&bc).render(area, &mut buf);
    }

    #[test]
    fn render_two_segments() {
        let mut bc = Breadcrumb::new(test_theme());
        bc.push(BreadcrumbSegment::new("main".into(), Color::White));
        bc.push(BreadcrumbSegment::new("research".into(), Color::Blue));
        let area = Rect::new(0, 0, 40, 1);
        let mut buf = Buffer::empty(area);
        (&bc).render(area, &mut buf);

        // "main › research" should be rendered
        assert_eq!(buf.cell((0, 0)).unwrap().symbol(), "m");
        assert_eq!(buf.cell((1, 0)).unwrap().symbol(), "a");
        assert_eq!(buf.cell((2, 0)).unwrap().symbol(), "i");
        assert_eq!(buf.cell((3, 0)).unwrap().symbol(), "n");
    }

    #[test]
    fn height_with_padding() {
        let mut bc = Breadcrumb::new(test_theme());
        bc.set_padding(Padding::symmetric(0, 1));
        bc.push(BreadcrumbSegment::new("main".into(), Color::White));
        bc.push(BreadcrumbSegment::new("research".into(), Color::Blue));
        assert_eq!(bc.height(), 3); // 1 + 1 top + 1 bottom
    }
}
