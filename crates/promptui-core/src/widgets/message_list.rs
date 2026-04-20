use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::Style;
use ratatui::text::Text;
use ratatui::widgets::{Paragraph, Widget, Wrap};

use crate::theme::Theme;
use crate::widgets::padding::Padding;
use crate::widgets::scroll_box::Scrollable;
use crate::widgets::word_wrap::count_wrapped_lines;

/// Left border character matching OpenCode's style.
const LEFT_BORDER: &str = "\u{2503}"; // ┃

/// Border occupies 1 column.
const BORDER_WIDTH: u16 = 1;

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
    padding: Padding,
    theme: Theme,
}

impl MessageList {
    pub fn new(theme: Theme) -> Self {
        Self {
            messages: Vec::new(),
            padding: Padding::uniform(0),
            theme,
        }
    }

    pub fn set_padding(&mut self, padding: Padding) {
        self.padding = padding;
    }

    /// Append a message.
    pub fn push(&mut self, message: Message) {
        self.messages.push(message);
    }

    pub fn is_empty(&self) -> bool {
        self.messages.is_empty()
    }

    /// Replace the content of the last message (e.g. for spinner → real reply).
    pub fn set_last_content(&mut self, content: String) {
        if let Some(msg) = self.messages.last_mut() {
            msg.content = content;
        }
    }

    /// Total word count across all messages (proxy for token count).
    pub fn total_word_count(&self) -> usize {
        self.messages
            .iter()
            .map(|m| m.content.split_whitespace().count())
            .sum()
    }

    /// Horizontal overhead per message: border + left padding + right padding.
    fn h_overhead(&self) -> u16 {
        BORDER_WIDTH + self.padding.horizontal()
    }

    /// Border color for a given role.
    fn border_color(&self, role: Role) -> Style {
        let fg = match role {
            Role::User => self.theme.primary,
            Role::Assistant => self.theme.secondary,
        };
        Style::default().fg(fg).bg(self.theme.panel_bg)
    }

    /// Compute the visual height of a single message (wrapped text + vertical padding).
    fn message_height(&self, content: &str, text_width: u16) -> u16 {
        let text_lines = if content.is_empty() {
            1
        } else {
            let mut total: u16 = 0;
            for line in content.split('\n') {
                total = total.saturating_add(count_wrapped_lines(line, text_width));
            }
            total
        };
        text_lines + self.padding.vertical()
    }

    /// Total height of all messages including separators.
    fn total_height(&self, area_width: u16) -> u16 {
        let text_width = area_width.saturating_sub(self.h_overhead());
        if text_width == 0 {
            return 0;
        }
        let mut total: u16 = 0;
        for (i, msg) in self.messages.iter().enumerate() {
            total = total.saturating_add(self.message_height(&msg.content, text_width));
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

        let text_x = area.x + BORDER_WIDTH + self.padding.left;
        let text_width = area.width.saturating_sub(self.h_overhead());

        if self.messages.is_empty() {
            let hint = "Type a message and press Enter";
            let hint_style = self.theme.muted_style();
            let y = area.y + area.height / 2;
            for (i, ch) in hint.chars().enumerate() {
                let px = text_x + i as u16;
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

        if text_width == 0 {
            return;
        }

        // Compute heights for each message (+ 1 line separator between messages)
        let mut msg_heights: Vec<u16> = Vec::with_capacity(self.messages.len());
        let mut total_height: u16 = 0;
        for (i, msg) in self.messages.iter().enumerate() {
            let h = self.message_height(&msg.content, text_width);
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
                    // Draw left border for all rows of this message
                    let border_style = self.border_color(msg.role);
                    for y in screen_top..screen_bottom {
                        if let Some(cell) = buf.cell_mut((area.x, y)) {
                            cell.set_symbol(LEFT_BORDER);
                            cell.set_style(border_style);
                        }
                    }

                    // Text area sits inside the padding
                    let text_area = Rect {
                        x: text_x,
                        y: screen_top + self.padding.top,
                        width: text_width,
                        height: render_height.saturating_sub(self.padding.vertical()),
                    };

                    // Skip lines accounts for both scroll and top padding
                    let skip_lines = content_top.saturating_sub(msg_top + self.padding.top);

                    if text_area.height > 0 {
                        let paragraph = Paragraph::new(Text::styled(&msg.content, text_style))
                            .wrap(Wrap { trim: false })
                            .scroll((skip_lines, 0));
                        paragraph.render(text_area, buf);
                    }
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
