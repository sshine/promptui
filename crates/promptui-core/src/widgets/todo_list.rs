use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::Style;
use ratatui::widgets::Widget;

use crate::theme::Theme;
use crate::widgets::padding::Padding;
use crate::widgets::scroll_box::Scrollable;

/// Left border character.
const LEFT_BORDER: &str = "\u{2503}"; // ┃

/// Border occupies 1 column.
const BORDER_WIDTH: u16 = 1;

/// Checkbox characters.
const CHECK_DONE: &str = "[\u{2713}]"; // [✓]
const CHECK_PENDING: &str = "[ ]";

/// A single item in a todo list.
#[derive(Debug, Clone)]
pub struct TodoItem {
    pub label: String,
    pub done: bool,
}

impl TodoItem {
    pub fn new(label: impl Into<String>, done: bool) -> Self {
        Self {
            label: label.into(),
            done,
        }
    }
}

/// A view-only checkbox list for displaying task progress.
///
/// Renders as a titled list with checkboxes. Not interactive —
/// intended for the message view to show planning task status.
///
/// ```text
/// ┃ Tasks (2/4)
/// ┃ [✓] Set up database schema
/// ┃ [✓] Write migration script
/// ┃ [ ] Run integration tests
/// ┃ [ ] Deploy to staging
/// ```
pub struct TodoList {
    theme: Theme,
    padding: Padding,
    title: String,
    items: Vec<TodoItem>,
}

impl TodoList {
    pub fn new(theme: Theme, title: impl Into<String>) -> Self {
        Self {
            theme,
            padding: Padding::uniform(0),
            title: title.into(),
            items: Vec::new(),
        }
    }

    pub fn set_padding(&mut self, padding: Padding) {
        self.padding = padding;
    }

    pub fn push(&mut self, item: TodoItem) {
        self.items.push(item);
    }

    pub fn set_items(&mut self, items: Vec<TodoItem>) {
        self.items = items;
    }

    pub fn items(&self) -> &[TodoItem] {
        &self.items
    }

    pub fn done_count(&self) -> usize {
        self.items.iter().filter(|i| i.done).count()
    }

    pub fn total_count(&self) -> usize {
        self.items.len()
    }

    pub fn title(&self) -> &str {
        &self.title
    }

    /// Height: title line + one line per item + vertical padding.
    pub fn required_height(&self) -> u16 {
        let content = 1 + self.items.len() as u16; // title + items
        content + self.padding.vertical()
    }

    /// Horizontal overhead: border + padding.
    fn h_overhead(&self) -> u16 {
        BORDER_WIDTH + self.padding.horizontal()
    }

    /// Format the title line with progress count.
    fn title_line(&self) -> String {
        format!(
            "{} ({}/{})",
            self.title,
            self.done_count(),
            self.total_count()
        )
    }
}

impl Widget for &TodoList {
    fn render(self, area: Rect, buf: &mut Buffer) {
        if area.width == 0 || area.height == 0 {
            return;
        }

        let bg_style = Style::default().bg(self.theme.panel_bg);
        let border_style = Style::default()
            .fg(self.theme.secondary)
            .bg(self.theme.panel_bg);
        let title_style = self.theme.text_style();
        let done_check_style = Style::default()
            .fg(self.theme.success)
            .bg(self.theme.panel_bg);
        let pending_check_style = self.theme.muted_style();
        let done_label_style = self.theme.muted_style();
        let pending_label_style = self.theme.text_style();

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
                cell.set_symbol(LEFT_BORDER);
                cell.set_style(border_style);
            }
        }

        let text_x = area.x + BORDER_WIDTH + self.padding.left;
        let text_width = area.width.saturating_sub(self.h_overhead());
        if text_width == 0 {
            return;
        }

        let mut y = area.y + self.padding.top;
        let y_max = area.y + area.height.saturating_sub(self.padding.bottom);

        // Title line
        if y < y_max {
            let title = self.title_line();
            for (i, ch) in title.chars().enumerate() {
                let x = text_x + i as u16;
                if x >= text_x + text_width {
                    break;
                }
                if let Some(cell) = buf.cell_mut((x, y)) {
                    cell.set_char(ch);
                    cell.set_style(title_style);
                }
            }
            y += 1;
        }

        // Items
        for item in &self.items {
            if y >= y_max {
                break;
            }

            let (check, check_style, label_style) = if item.done {
                (CHECK_DONE, done_check_style, done_label_style)
            } else {
                (CHECK_PENDING, pending_check_style, pending_label_style)
            };

            let mut x = text_x;

            // Checkbox
            for ch in check.chars() {
                if x >= text_x + text_width {
                    break;
                }
                if let Some(cell) = buf.cell_mut((x, y)) {
                    cell.set_char(ch);
                    cell.set_style(check_style);
                }
                x += 1;
            }

            // Space after checkbox
            if x < text_x + text_width {
                x += 1;
            }

            // Label
            for ch in item.label.chars() {
                if x >= text_x + text_width {
                    break;
                }
                if let Some(cell) = buf.cell_mut((x, y)) {
                    cell.set_char(ch);
                    cell.set_style(label_style);
                }
                x += 1;
            }

            y += 1;
        }
    }
}

impl Scrollable for TodoList {
    fn content_height(&self, _width: u16) -> u16 {
        self.required_height()
    }

    fn render_scrolled(&self, area: Rect, buf: &mut Buffer, _scroll_offset: u16) {
        (self).render(area, buf);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_theme() -> Theme {
        Theme::default()
    }

    #[test]
    fn empty_todo_list() {
        let list = TodoList::new(test_theme(), "Tasks");
        assert_eq!(list.total_count(), 0);
        assert_eq!(list.done_count(), 0);
        assert_eq!(list.required_height(), 1); // just the title
    }

    #[test]
    fn push_items() {
        let mut list = TodoList::new(test_theme(), "Tasks");
        list.push(TodoItem::new("First", false));
        list.push(TodoItem::new("Second", true));
        assert_eq!(list.total_count(), 2);
        assert_eq!(list.done_count(), 1);
    }

    #[test]
    fn required_height() {
        let mut list = TodoList::new(test_theme(), "Tasks");
        list.push(TodoItem::new("A", false));
        list.push(TodoItem::new("B", true));
        list.push(TodoItem::new("C", false));
        // 1 title + 3 items = 4
        assert_eq!(list.required_height(), 4);
    }

    #[test]
    fn title_line_format() {
        let mut list = TodoList::new(test_theme(), "Tasks");
        list.push(TodoItem::new("A", true));
        list.push(TodoItem::new("B", true));
        list.push(TodoItem::new("C", false));
        list.push(TodoItem::new("D", false));
        assert_eq!(list.title_line(), "Tasks (2/4)");
    }

    #[test]
    fn render_does_not_panic() {
        let mut list = TodoList::new(test_theme(), "Tasks");
        list.push(TodoItem::new("Item 1", true));
        list.push(TodoItem::new("Item 2", false));
        let area = Rect::new(0, 0, 40, 5);
        let mut buf = Buffer::empty(area);
        (&list).render(area, &mut buf);
    }

    #[test]
    fn render_draws_border() {
        let mut list = TodoList::new(test_theme(), "Tasks");
        list.push(TodoItem::new("Item", false));
        let area = Rect::new(0, 0, 20, 3);
        let mut buf = Buffer::empty(area);
        (&list).render(area, &mut buf);
        assert_eq!(buf.cell((0, 0)).unwrap().symbol(), LEFT_BORDER);
        assert_eq!(buf.cell((0, 1)).unwrap().symbol(), LEFT_BORDER);
    }

    #[test]
    fn render_zero_area() {
        let list = TodoList::new(test_theme(), "Tasks");
        let area = Rect::new(0, 0, 0, 0);
        let mut buf = Buffer::empty(Rect::new(0, 0, 1, 1));
        (&list).render(area, &mut buf);
    }

    #[test]
    fn scrollable_content_height() {
        let mut list = TodoList::new(test_theme(), "Tasks");
        list.push(TodoItem::new("A", false));
        list.push(TodoItem::new("B", false));
        assert_eq!(list.content_height(40), 3); // 1 title + 2 items
    }
}
