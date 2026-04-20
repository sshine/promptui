use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::Style;
use ratatui::text::Text;
use ratatui::widgets::{Paragraph, Widget, Wrap};

use crate::theme::Theme;
use crate::widgets::scroll_box::Scrollable;
use crate::widgets::word_wrap::count_wrapped_lines;

/// Left border character matching OpenCode's style.
const LEFT_BORDER: &str = "\u{2503}"; // ┃

/// Left border width (1) + padding (1).
const LEFT_INDENT: u16 = 2;

/// A chat message with a role and text content.
#[derive(Debug, Clone)]
pub struct Message {
    pub role: Role,
    pub content: String,
}

/// Who authored a message.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Role {
    User,
    Assistant,
}

/// A list of chat messages rendered with left-border indicators.
///
/// Messages are displayed bottom-aligned (most recent at the bottom).
/// Scrolling is managed externally by a [`ScrollBox`](crate::widgets::scroll_box::ScrollBox).
pub struct MessageList {
    messages: Vec<Message>,
    theme: Theme,
}

impl MessageList {
    pub fn new(theme: Theme) -> Self {
        Self {
            messages: Vec::new(),
            theme,
        }
    }

    /// Append a message.
    pub fn push(&mut self, message: Message) {
        self.messages.push(message);
    }

    pub fn is_empty(&self) -> bool {
        self.messages.is_empty()
    }

    /// Border color for a given role.
    fn border_color(&self, role: Role) -> Style {
        let fg = match role {
            Role::User => self.theme.primary,
            Role::Assistant => self.theme.secondary,
        };
        Style::default().fg(fg).bg(self.theme.panel_bg)
    }

    /// Compute the visual height of a single message (wrapped lines).
    fn message_height(&self, content: &str, content_width: u16) -> u16 {
        if content.is_empty() {
            return 1;
        }
        let mut total: u16 = 0;
        for line in content.split('\n') {
            total = total.saturating_add(count_wrapped_lines(line, content_width));
        }
        total
    }

    /// Total height of all messages including separators.
    fn total_height(&self, area_width: u16) -> u16 {
        let content_width = area_width.saturating_sub(LEFT_INDENT);
        if content_width == 0 {
            return 0;
        }
        let mut total: u16 = 0;
        for (i, msg) in self.messages.iter().enumerate() {
            total = total.saturating_add(self.message_height(&msg.content, content_width));
            if i + 1 < self.messages.len() {
                total = total.saturating_add(1); // separator
            }
        }
        total
    }

    fn render_with_scroll(&self, area: Rect, buf: &mut Buffer, scroll_offset: u16) {
        if area.width == 0 || area.height == 0 {
            return;
        }

        // Fill background
        let bg_style = Style::default().bg(self.theme.panel_bg);
        for y in area.y..area.y + area.height {
            for x in area.x..area.x + area.width {
                if let Some(cell) = buf.cell_mut((x, y)) {
                    cell.set_style(bg_style);
                    cell.set_char(' ');
                }
            }
        }

        if self.messages.is_empty() {
            let hint = "Type a message and press Enter";
            let hint_style = self.theme.muted_style();
            let x = area.x + LEFT_INDENT;
            let y = area.y + area.height / 2;
            for (i, ch) in hint.chars().enumerate() {
                let px = x + i as u16;
                if px >= area.x + area.width {
                    break;
                }
                if let Some(cell) = buf.cell_mut((px, y)) {
                    cell.set_char(ch);
                    cell.set_style(hint_style);
                }
            }
            return;
        }

        let content_width = area.width.saturating_sub(LEFT_INDENT);
        if content_width == 0 {
            return;
        }

        // Compute heights for each message (+ 1 line separator between messages)
        let mut msg_heights: Vec<u16> = Vec::with_capacity(self.messages.len());
        let mut total_height: u16 = 0;
        for (i, msg) in self.messages.iter().enumerate() {
            let h = self.message_height(&msg.content, content_width);
            msg_heights.push(h);
            total_height = total_height.saturating_add(h);
            if i + 1 < self.messages.len() {
                total_height = total_height.saturating_add(1); // separator
            }
        }

        let visible_height = area.height;
        let scroll = scroll_offset.min(total_height.saturating_sub(visible_height));

        let content_bottom = total_height.saturating_sub(scroll);
        let content_top = content_bottom.saturating_sub(visible_height);

        let mut y_offset: u16 = 0;
        let text_style = self.theme.text_style();

        for (i, msg) in self.messages.iter().enumerate() {
            let msg_h = msg_heights[i];
            let msg_top = y_offset;
            let msg_bottom = y_offset + msg_h;

            if msg_bottom > content_top && msg_top < content_bottom {
                let screen_top = if msg_top >= content_top {
                    area.y + (msg_top - content_top)
                } else {
                    area.y
                };
                let screen_bottom = if msg_bottom <= content_bottom {
                    area.y + (msg_bottom - content_top)
                } else {
                    area.y + visible_height
                };
                let render_height = screen_bottom.saturating_sub(screen_top);

                if render_height > 0 {
                    let border_style = self.border_color(msg.role);
                    for y in screen_top..screen_bottom {
                        if let Some(cell) = buf.cell_mut((area.x, y)) {
                            cell.set_symbol(LEFT_BORDER);
                            cell.set_style(border_style);
                        }
                    }

                    let text_area = Rect {
                        x: area.x + LEFT_INDENT,
                        y: screen_top,
                        width: content_width,
                        height: render_height,
                    };

                    let skip_lines = content_top.saturating_sub(msg_top);

                    let paragraph = Paragraph::new(Text::styled(&msg.content, text_style))
                        .wrap(Wrap { trim: false })
                        .scroll((skip_lines, 0));
                    paragraph.render(text_area, buf);
                }
            }

            y_offset += msg_h;
            if i + 1 < self.messages.len() {
                y_offset += 1;
            }
        }
    }
}

impl Scrollable for MessageList {
    fn content_height(&self, width: u16) -> u16 {
        self.total_height(width)
    }

    fn render_scrolled(&self, area: Rect, buf: &mut Buffer, scroll_offset: u16) {
        self.render_with_scroll(area, buf, scroll_offset);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn empty_list() {
        let ml = MessageList::new(Theme::default());
        assert!(ml.is_empty());
    }

    #[test]
    fn push_adds_message() {
        let mut ml = MessageList::new(Theme::default());
        ml.push(Message {
            role: Role::User,
            content: "hello".to_string(),
        });
        assert!(!ml.is_empty());
    }

    #[test]
    fn content_height_single_message() {
        let mut ml = MessageList::new(Theme::default());
        ml.push(Message {
            role: Role::User,
            content: "hello".to_string(),
        });
        // At width 40: LEFT_INDENT=2, content_width=38, "hello" = 1 line
        assert_eq!(ml.content_height(40), 1);
    }

    #[test]
    fn content_height_with_separator() {
        let mut ml = MessageList::new(Theme::default());
        ml.push(Message {
            role: Role::User,
            content: "hello".to_string(),
        });
        ml.push(Message {
            role: Role::Assistant,
            content: "world".to_string(),
        });
        // 1 + 1 separator + 1 = 3
        assert_eq!(ml.content_height(40), 3);
    }
}
