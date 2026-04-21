use std::collections::HashMap;

use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::Style;
use ratatui::text::Text;
use ratatui::widgets::{Paragraph, Widget, Wrap};

use crate::theme::Theme;
use crate::widgets::agent_scope::DisplayMode;
use crate::widgets::padding::Padding;
use crate::widgets::scroll_box::Scrollable;
use crate::widgets::truncated_block::TruncatedBlock;
use crate::widgets::word_wrap::count_wrapped_lines;

/// Left border character matching OpenCode's style.
const LEFT_BORDER: &str = "\u{2503}"; // ┃

/// Border occupies 1 column.
const BORDER_WIDTH: u16 = 1;

/// A chat message with a role and styled text content.
///
/// Content is stored as [`Text`], supporting arbitrary inline styling
/// (colored spans, bold, etc.). Use [`Message::plain`] for unstyled text
/// or construct `Text` directly for rich content.
#[derive(Debug, Clone)]
pub struct Message {
    pub role: Role,
    pub content: Text<'static>,
}

impl Message {
    /// Create a message with plain, unstyled text.
    pub fn plain(role: Role, content: impl Into<String>) -> Self {
        Self {
            role,
            content: Text::from(content.into()),
        }
    }

    /// Create a message with pre-styled rich text.
    pub fn styled(role: Role, content: Text<'static>) -> Self {
        Self { role, content }
    }
}

/// Who authored a message.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Role {
    User,
    Assistant,
    System,
}

/// A content block in the conversation stream.
///
/// The message list holds a sequence of these blocks, which can be
/// chat messages, tool call results, or other content types.
/// Each block supports [`DisplayMode::Full`] and [`DisplayMode::Compact`]
/// rendering modes.
#[derive(Clone)]
pub enum ContentBlock {
    /// A chat message (user, assistant, system).
    Message { role: Role, content: Text<'static> },
    /// A tool call with collapsible output.
    ToolCall {
        name: String,
        content: TruncatedBlock,
    },
    /// A questionnaire summary (rendered when questionnaire is not focused).
    QuestionnaireSummary {
        summary_text: String,
        progress: (usize, usize), // (answered, total)
    },
    /// A todo/task list with checkboxes.
    TodoList {
        title: String,
        items: Vec<(String, bool)>, // (label, done)
    },
}

/// Extract the plain text from a [`Text`], joining lines with newlines.
fn plain_text(text: &Text) -> String {
    let mut s = String::new();
    for (i, line) in text.lines.iter().enumerate() {
        if i > 0 {
            s.push('\n');
        }
        for span in &line.spans {
            s.push_str(&span.content);
        }
    }
    s
}

/// A list of content blocks rendered with left-border indicators.
///
/// Blocks are displayed bottom-aligned (most recent at the bottom).
/// Scrolling is managed externally by a [`ScrollBox`](crate::widgets::scroll_box::ScrollBox).
///
/// Supports [`DisplayMode::Full`] (default) for complete rendering and
/// [`DisplayMode::Compact`] for summarized single-line rendering.
/// Per-block overrides allow toggling individual blocks between modes.
pub struct MessageList {
    blocks: Vec<ContentBlock>,
    display_mode: DisplayMode,
    block_overrides: HashMap<usize, DisplayMode>,
    padding: Padding,
    theme: Theme,
}

impl MessageList {
    pub fn new(theme: Theme) -> Self {
        Self {
            blocks: Vec::new(),
            display_mode: DisplayMode::Full,
            block_overrides: HashMap::new(),
            padding: Padding::uniform(0),
            theme,
        }
    }

    pub fn set_padding(&mut self, padding: Padding) {
        self.padding = padding;
    }

    /// Set the global display mode (Full or Compact).
    pub fn set_display_mode(&mut self, mode: DisplayMode) {
        self.display_mode = mode;
    }

    /// Get the current global display mode.
    pub fn display_mode(&self) -> DisplayMode {
        self.display_mode
    }

    /// Set a per-block display mode override.
    pub fn set_block_override(&mut self, index: usize, mode: DisplayMode) {
        self.block_overrides.insert(index, mode);
    }

    /// Remove a per-block display mode override, reverting to global.
    pub fn clear_block_override(&mut self, index: usize) {
        self.block_overrides.remove(&index);
    }

    /// Clear all per-block overrides.
    pub fn clear_all_overrides(&mut self) {
        self.block_overrides.clear();
    }

    /// Get the effective display mode for a block (override or global).
    fn effective_mode(&self, index: usize) -> DisplayMode {
        self.block_overrides
            .get(&index)
            .copied()
            .unwrap_or(self.display_mode)
    }

    /// Append a message (convenience method wrapping into ContentBlock::Message).
    pub fn push(&mut self, message: Message) {
        self.blocks.push(ContentBlock::Message {
            role: message.role,
            content: message.content,
        });
    }

    /// Append a content block.
    pub fn push_block(&mut self, block: ContentBlock) {
        self.blocks.push(block);
    }

    pub fn is_empty(&self) -> bool {
        self.blocks.is_empty()
    }

    /// Number of content blocks.
    pub fn len(&self) -> usize {
        self.blocks.len()
    }

    /// Get a reference to the blocks.
    pub fn blocks(&self) -> &[ContentBlock] {
        &self.blocks
    }

    /// Get a mutable reference to a block by index.
    pub fn block_mut(&mut self, index: usize) -> Option<&mut ContentBlock> {
        self.blocks.get_mut(index)
    }

    /// Replace the content of the last message block with plain text
    /// (e.g. for streaming updates). Only affects ContentBlock::Message blocks.
    pub fn set_last_content(&mut self, content: String) {
        for block in self.blocks.iter_mut().rev() {
            if let ContentBlock::Message { content: c, .. } = block {
                *c = Text::from(content);
                return;
            }
        }
    }

    /// Total word count across all message blocks (proxy for token count).
    pub fn total_word_count(&self) -> usize {
        self.blocks
            .iter()
            .map(|block| match block {
                ContentBlock::Message { content, .. } => {
                    plain_text(content).split_whitespace().count()
                }
                ContentBlock::ToolCall { .. }
                | ContentBlock::QuestionnaireSummary { .. }
                | ContentBlock::TodoList { .. } => 0,
            })
            .sum()
    }

    /// Horizontal overhead per block: border + left padding + right padding.
    fn h_overhead(&self) -> u16 {
        BORDER_WIDTH + self.padding.horizontal()
    }

    /// Border color for a given role.
    fn border_color(&self, role: Role) -> Style {
        let fg = match role {
            Role::User => self.theme.primary,
            Role::Assistant => self.theme.secondary,
            Role::System => self.theme.info,
        };
        Style::default().fg(fg).bg(self.theme.panel_bg)
    }

    /// Compute height of a content block at a given text width and display mode.
    fn block_height(&self, block: &ContentBlock, text_width: u16, mode: DisplayMode) -> u16 {
        match (block, mode) {
            (ContentBlock::Message { content, .. }, DisplayMode::Full) => {
                self.text_content_height(content, text_width)
            }
            (ContentBlock::Message { .. }, DisplayMode::Compact) => {
                // Compact: always 1 line + vertical padding
                1 + self.padding.vertical()
            }
            (ContentBlock::ToolCall { content, .. }, DisplayMode::Full) => {
                // Tool label (1 line) + truncated block content
                let tool_label_height = 1;
                let block_width = text_width; // TruncatedBlock uses full text width
                tool_label_height + content.required_height(block_width + content_h_overhead())
            }
            (ContentBlock::ToolCall { .. }, DisplayMode::Compact) => {
                // Compact: "Tool: name" — 1 line + vertical padding
                1 + self.padding.vertical()
            }
            (ContentBlock::QuestionnaireSummary { .. }, _) => {
                // Always 1 line (same in full and compact)
                1 + self.padding.vertical()
            }
            (ContentBlock::TodoList { items, .. }, DisplayMode::Full) => {
                // Title line + one line per item
                let content_lines = 1 + items.len() as u16;
                content_lines + self.padding.vertical()
            }
            (ContentBlock::TodoList { .. }, DisplayMode::Compact) => {
                // Just title line: "Tasks (2/4)"
                1 + self.padding.vertical()
            }
        }
    }

    /// Height of a text message (wrapped lines + vertical padding).
    fn text_content_height(&self, content: &Text, text_width: u16) -> u16 {
        let plain = plain_text(content);
        let text_lines = if plain.is_empty() {
            1
        } else {
            let mut total: u16 = 0;
            for line in plain.split('\n') {
                total = total.saturating_add(count_wrapped_lines(line, text_width));
            }
            total
        };
        text_lines + self.padding.vertical()
    }

    /// Total height of all blocks including separators.
    fn total_height(&self, area_width: u16) -> u16 {
        let text_width = area_width.saturating_sub(self.h_overhead());
        if text_width == 0 {
            return 0;
        }
        let mut total: u16 = 0;
        for (i, block) in self.blocks.iter().enumerate() {
            let mode = self.effective_mode(i);
            total = total.saturating_add(self.block_height(block, text_width, mode));
            if i + 1 < self.blocks.len() {
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

        if self.blocks.is_empty() {
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

        // Compute heights for each block
        let mut block_heights: Vec<u16> = Vec::with_capacity(self.blocks.len());
        let mut total_height: u16 = 0;
        for (i, block) in self.blocks.iter().enumerate() {
            let mode = self.effective_mode(i);
            let h = self.block_height(block, text_width, mode);
            block_heights.push(h);
            total_height = total_height.saturating_add(h);
            if i + 1 < self.blocks.len() {
                total_height = total_height.saturating_add(1); // separator
            }
        }

        let visible_height = area.height;
        let scroll = scroll_offset.min(total_height.saturating_sub(visible_height));

        let content_bottom = total_height.saturating_sub(scroll);
        let content_top = content_bottom.saturating_sub(visible_height);

        let mut y_offset: u16 = 0;

        for (i, block) in self.blocks.iter().enumerate() {
            let block_h = block_heights[i];
            let block_top = y_offset;
            let block_bottom = y_offset + block_h;
            let mode = self.effective_mode(i);

            if block_bottom > content_top && block_top < content_bottom {
                let screen_top = if block_top >= content_top {
                    area.y + (block_top - content_top)
                } else {
                    area.y
                };
                let screen_bottom = if block_bottom <= content_bottom {
                    area.y + (block_bottom - content_top)
                } else {
                    area.y + visible_height
                };
                let render_height = screen_bottom.saturating_sub(screen_top);

                if render_height > 0 {
                    let skip_lines = content_top.saturating_sub(block_top + self.padding.top);

                    match (block, mode) {
                        (ContentBlock::Message { role, content }, DisplayMode::Full) => {
                            self.render_message_full(
                                area,
                                buf,
                                *role,
                                content,
                                screen_top,
                                screen_bottom,
                                text_x,
                                text_width,
                                skip_lines,
                            );
                        }
                        (ContentBlock::Message { role, content }, DisplayMode::Compact) => {
                            self.render_message_compact(
                                area,
                                buf,
                                *role,
                                content,
                                screen_top,
                                screen_bottom,
                                text_x,
                                text_width,
                            );
                        }
                        (ContentBlock::ToolCall { name, content }, DisplayMode::Full) => {
                            self.render_tool_call_full(
                                area,
                                buf,
                                name,
                                content,
                                screen_top,
                                screen_bottom,
                                text_x,
                                text_width,
                            );
                        }
                        (ContentBlock::ToolCall { name, .. }, DisplayMode::Compact) => {
                            self.render_tool_call_compact(
                                area,
                                buf,
                                name,
                                screen_top,
                                screen_bottom,
                                text_x,
                                text_width,
                            );
                        }
                        (
                            ContentBlock::QuestionnaireSummary {
                                summary_text,
                                progress,
                            },
                            _,
                        ) => {
                            self.render_questionnaire_summary(
                                area,
                                buf,
                                summary_text,
                                *progress,
                                screen_top,
                                screen_bottom,
                                text_x,
                                text_width,
                            );
                        }
                        (ContentBlock::TodoList { title, items }, DisplayMode::Full) => {
                            self.render_todo_list_full(
                                area,
                                buf,
                                title,
                                items,
                                screen_top,
                                screen_bottom,
                                text_x,
                                text_width,
                            );
                        }
                        (ContentBlock::TodoList { title, items }, DisplayMode::Compact) => {
                            self.render_todo_list_compact(
                                area,
                                buf,
                                title,
                                items,
                                screen_top,
                                screen_bottom,
                                text_x,
                                text_width,
                            );
                        }
                    }
                }
            }

            y_offset += block_h;
            if i + 1 < self.blocks.len() {
                y_offset += 1;
            }
        }
    }

    /// Render a message in Full mode (current behavior).
    #[allow(clippy::too_many_arguments)]
    fn render_message_full(
        &self,
        area: Rect,
        buf: &mut Buffer,
        role: Role,
        content: &Text,
        screen_top: u16,
        screen_bottom: u16,
        text_x: u16,
        text_width: u16,
        skip_lines: u16,
    ) {
        let render_height = screen_bottom.saturating_sub(screen_top);
        let border_style = self.border_color(role);
        let text_style = self.theme.text_style();

        // Draw left border
        for y in screen_top..screen_bottom {
            if let Some(cell) = buf.cell_mut((area.x, y)) {
                cell.set_symbol(LEFT_BORDER);
                cell.set_style(border_style);
            }
        }

        // Text area inside padding
        let text_area = Rect {
            x: text_x,
            y: screen_top + self.padding.top,
            width: text_width,
            height: render_height.saturating_sub(self.padding.vertical()),
        };

        if text_area.height > 0 {
            let paragraph = Paragraph::new(content.clone())
                .style(text_style)
                .wrap(Wrap { trim: false })
                .scroll((skip_lines, 0));
            paragraph.render(text_area, buf);
        }
    }

    /// Render a message in Compact mode (single truncated line).
    #[allow(clippy::too_many_arguments)]
    fn render_message_compact(
        &self,
        area: Rect,
        buf: &mut Buffer,
        role: Role,
        content: &Text,
        screen_top: u16,
        screen_bottom: u16,
        text_x: u16,
        text_width: u16,
    ) {
        let border_style = self.border_color(role);
        let text_style = self.theme.text_style();
        let ellipsis_style = self.theme.muted_style();

        // Draw left border
        for y in screen_top..screen_bottom {
            if let Some(cell) = buf.cell_mut((area.x, y)) {
                cell.set_symbol(LEFT_BORDER);
                cell.set_style(border_style);
            }
        }

        // Extract first line of content
        let plain = plain_text(content);
        let first_line = plain.lines().next().unwrap_or("");
        let max_chars = text_width.saturating_sub(3) as usize; // room for "..."
        let (display, needs_ellipsis) = if first_line.len() > max_chars && text_width > 3 {
            (&first_line[..max_chars], true)
        } else {
            (
                first_line,
                plain.contains('\n') || first_line.len() > text_width as usize,
            )
        };

        let y = screen_top + self.padding.top;
        if y >= screen_bottom {
            return;
        }

        let mut x = text_x;
        for ch in display.chars() {
            if x >= area.x + area.width {
                break;
            }
            if let Some(cell) = buf.cell_mut((x, y)) {
                cell.set_char(ch);
                cell.set_style(text_style);
            }
            x += 1;
        }

        if needs_ellipsis {
            for ch in "...".chars() {
                if x >= area.x + area.width {
                    break;
                }
                if let Some(cell) = buf.cell_mut((x, y)) {
                    cell.set_char(ch);
                    cell.set_style(ellipsis_style);
                }
                x += 1;
            }
        }
    }

    /// Render a tool call in Full mode (label + truncated block output).
    #[allow(clippy::too_many_arguments)]
    fn render_tool_call_full(
        &self,
        _area: Rect,
        buf: &mut Buffer,
        name: &str,
        content: &TruncatedBlock,
        screen_top: u16,
        screen_bottom: u16,
        text_x: u16,
        text_width: u16,
    ) {
        let label_style = self.theme.muted_style();

        // Tool label line
        let y = screen_top;
        if y < screen_bottom {
            let label = format!("Tool: {}", name);
            let mut x = text_x;
            for ch in label.chars() {
                if x >= text_x + text_width {
                    break;
                }
                if let Some(cell) = buf.cell_mut((x, y)) {
                    cell.set_char(ch);
                    cell.set_style(label_style);
                }
                x += 1;
            }
        }

        // TruncatedBlock content below the label
        let content_top = screen_top + 1;
        if content_top < screen_bottom {
            let content_area = Rect {
                x: text_x.saturating_sub(BORDER_WIDTH), // TruncatedBlock draws its own border
                y: content_top,
                width: text_width + BORDER_WIDTH,
                height: screen_bottom - content_top,
            };
            (content).render(content_area, buf);
        }
    }

    /// Render a tool call in Compact mode (single line summary).
    #[allow(clippy::too_many_arguments)]
    fn render_tool_call_compact(
        &self,
        area: Rect,
        buf: &mut Buffer,
        name: &str,
        screen_top: u16,
        screen_bottom: u16,
        text_x: u16,
        text_width: u16,
    ) {
        let label_style = self.theme.muted_style();

        // Draw a subtle left border
        let border_style = Style::default()
            .fg(self.theme.border)
            .bg(self.theme.panel_bg);
        for y in screen_top..screen_bottom {
            if let Some(cell) = buf.cell_mut((area.x, y)) {
                cell.set_symbol(LEFT_BORDER);
                cell.set_style(border_style);
            }
        }

        let y = screen_top + self.padding.top;
        if y >= screen_bottom {
            return;
        }

        let label = format!("Tool: {}", name);
        let mut x = text_x;
        for ch in label.chars() {
            if x >= text_x + text_width {
                break;
            }
            if let Some(cell) = buf.cell_mut((x, y)) {
                cell.set_char(ch);
                cell.set_style(label_style);
            }
            x += 1;
        }
    }

    /// Render a questionnaire summary (same in full and compact).
    #[allow(clippy::too_many_arguments)]
    fn render_questionnaire_summary(
        &self,
        area: Rect,
        buf: &mut Buffer,
        summary_text: &str,
        progress: (usize, usize),
        screen_top: u16,
        screen_bottom: u16,
        text_x: u16,
        text_width: u16,
    ) {
        let border_style = Style::default()
            .fg(self.theme.accent)
            .bg(self.theme.panel_bg);
        let text_style = self.theme.muted_style();

        for y in screen_top..screen_bottom {
            if let Some(cell) = buf.cell_mut((area.x, y)) {
                cell.set_symbol(LEFT_BORDER);
                cell.set_style(border_style);
            }
        }

        let y = screen_top + self.padding.top;
        if y >= screen_bottom {
            return;
        }

        let label = format!(
            "Survey: {} \u{00B7} {}/{}",
            summary_text, progress.0, progress.1
        );
        let mut x = text_x;
        for ch in label.chars() {
            if x >= text_x + text_width {
                break;
            }
            if let Some(cell) = buf.cell_mut((x, y)) {
                cell.set_char(ch);
                cell.set_style(text_style);
            }
            x += 1;
        }
    }

    /// Render a todo list in full mode (title + items).
    #[allow(clippy::too_many_arguments)]
    fn render_todo_list_full(
        &self,
        area: Rect,
        buf: &mut Buffer,
        title: &str,
        items: &[(String, bool)],
        screen_top: u16,
        screen_bottom: u16,
        text_x: u16,
        text_width: u16,
    ) {
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

        for y in screen_top..screen_bottom {
            if let Some(cell) = buf.cell_mut((area.x, y)) {
                cell.set_symbol(LEFT_BORDER);
                cell.set_style(border_style);
            }
        }

        let mut y = screen_top + self.padding.top;

        // Title line
        if y < screen_bottom {
            let done_count = items.iter().filter(|(_, d)| *d).count();
            let title_text = format!("{} ({}/{})", title, done_count, items.len());
            let mut x = text_x;
            for ch in title_text.chars() {
                if x >= text_x + text_width {
                    break;
                }
                if let Some(cell) = buf.cell_mut((x, y)) {
                    cell.set_char(ch);
                    cell.set_style(title_style);
                }
                x += 1;
            }
            y += 1;
        }

        // Items
        for (label, done) in items {
            if y >= screen_bottom {
                break;
            }
            let (check, check_style, label_style) = if *done {
                ("[\u{2713}]", done_check_style, done_label_style)
            } else {
                ("[ ]", pending_check_style, pending_label_style)
            };

            let mut x = text_x;
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
            x += 1; // space
            for ch in label.chars() {
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

    /// Render a todo list in compact mode (title only).
    #[allow(clippy::too_many_arguments)]
    fn render_todo_list_compact(
        &self,
        area: Rect,
        buf: &mut Buffer,
        title: &str,
        items: &[(String, bool)],
        screen_top: u16,
        screen_bottom: u16,
        text_x: u16,
        text_width: u16,
    ) {
        let border_style = Style::default()
            .fg(self.theme.secondary)
            .bg(self.theme.panel_bg);
        let text_style = self.theme.muted_style();

        for y in screen_top..screen_bottom {
            if let Some(cell) = buf.cell_mut((area.x, y)) {
                cell.set_symbol(LEFT_BORDER);
                cell.set_style(border_style);
            }
        }

        let y = screen_top + self.padding.top;
        if y >= screen_bottom {
            return;
        }

        let done_count = items.iter().filter(|(_, d)| *d).count();
        let label = format!("{} ({}/{})", title, done_count, items.len());
        let mut x = text_x;
        for ch in label.chars() {
            if x >= text_x + text_width {
                break;
            }
            if let Some(cell) = buf.cell_mut((x, y)) {
                cell.set_char(ch);
                cell.set_style(text_style);
            }
            x += 1;
        }
    }
}

/// TruncatedBlock's horizontal overhead (border + its own padding).
fn content_h_overhead() -> u16 {
    BORDER_WIDTH // TruncatedBlock has its own border
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
        ml.push(Message::plain(Role::User, "hello"));
        assert!(!ml.is_empty());
        assert_eq!(ml.len(), 1);
    }

    #[test]
    fn content_height_single_message() {
        let mut ml = MessageList::new(Theme::default());
        ml.push(Message::plain(Role::User, "hello"));
        // At width 40: border=1, content_width=39, "hello" = 1 line
        assert_eq!(ml.content_height(40), 1);
    }

    #[test]
    fn content_height_with_separator() {
        let mut ml = MessageList::new(Theme::default());
        ml.push(Message::plain(Role::User, "hello"));
        ml.push(Message::plain(Role::Assistant, "world"));
        // 1 + 1 separator + 1 = 3
        assert_eq!(ml.content_height(40), 3);
    }

    #[test]
    fn compact_mode_single_line_per_message() {
        let mut ml = MessageList::new(Theme::default());
        ml.set_display_mode(DisplayMode::Compact);
        ml.push(Message::plain(
            Role::User,
            "this is a long message that would normally wrap",
        ));
        // Compact = 1 line regardless of content length
        assert_eq!(ml.content_height(40), 1);
    }

    #[test]
    fn block_override_changes_height() {
        let mut ml = MessageList::new(Theme::default());
        ml.push(Message::plain(Role::User, "line1\nline2\nline3"));
        let full_height = ml.content_height(40);

        ml.set_display_mode(DisplayMode::Compact);
        let compact_height = ml.content_height(40);

        assert!(full_height > compact_height);

        // Override block 0 back to Full
        ml.set_block_override(0, DisplayMode::Full);
        let override_height = ml.content_height(40);
        assert_eq!(override_height, full_height);
    }

    #[test]
    fn push_block_tool_call() {
        let mut ml = MessageList::new(Theme::default());
        let tb = TruncatedBlock::new(Theme::default(), Text::from("output line 1\noutput line 2"));
        ml.push_block(ContentBlock::ToolCall {
            name: "read_file".to_string(),
            content: tb,
        });
        assert_eq!(ml.len(), 1);
        assert!(!ml.is_empty());
    }

    #[test]
    fn set_last_content_updates_last_message() {
        let mut ml = MessageList::new(Theme::default());
        ml.push(Message::plain(Role::User, "hello"));
        ml.push_block(ContentBlock::ToolCall {
            name: "test".to_string(),
            content: TruncatedBlock::new(Theme::default(), Text::from("out")),
        });
        ml.push(Message::plain(Role::Assistant, ""));

        ml.set_last_content("updated".to_string());

        // The last Message block should be updated
        if let Some(ContentBlock::Message { content, .. }) = ml.blocks.last() {
            assert_eq!(plain_text(content), "updated");
        } else {
            panic!("Expected last block to be a Message");
        }
    }

    #[test]
    fn total_word_count() {
        let mut ml = MessageList::new(Theme::default());
        ml.push(Message::plain(Role::User, "hello world"));
        ml.push(Message::plain(Role::Assistant, "hi there friend"));
        assert_eq!(ml.total_word_count(), 5);
    }

    #[test]
    fn clear_overrides() {
        let mut ml = MessageList::new(Theme::default());
        ml.push(Message::plain(Role::User, "test"));
        ml.set_block_override(0, DisplayMode::Compact);
        assert_eq!(ml.effective_mode(0), DisplayMode::Compact);

        ml.clear_block_override(0);
        assert_eq!(ml.effective_mode(0), DisplayMode::Full);
    }

    #[test]
    fn tool_call_compact_height() {
        let mut ml = MessageList::new(Theme::default());
        ml.set_display_mode(DisplayMode::Compact);
        let tb = TruncatedBlock::new(
            Theme::default(),
            Text::from("line1\nline2\nline3\nline4\nline5"),
        );
        ml.push_block(ContentBlock::ToolCall {
            name: "read_file".to_string(),
            content: tb,
        });
        // Compact tool call = 1 line
        assert_eq!(ml.content_height(40), 1);
    }
}
