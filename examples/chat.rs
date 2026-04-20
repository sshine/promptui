use std::io;

use crossterm::event::{
    DisableMouseCapture, EnableMouseCapture, Event, KeyEventKind, KeyboardEnhancementFlags,
    MouseEventKind, PopKeyboardEnhancementFlags, PushKeyboardEnhancementFlags,
};
use crossterm::execute;
use futures::StreamExt;
use ratatui::DefaultTerminal;
use ratatui::layout::{Constraint, Layout};
use ratatui::style::{Color, Style};
use ratatui::widgets::Widget;
use tokio::sync::mpsc;

use promptui_core::input::Input;
use promptui_core::theme::Theme;
use promptui_core::widgets::message_list::{Message, MessageList, Role};
use promptui_core::widgets::padding::Padding;
use promptui_core::widgets::scroll_box::ScrollBox;
use promptui_core::widgets::text_field::{TextField, TextFieldAction};

/// The 4 border colors cycled by Tab.
const BORDER_COLORS: [Color; 4] = [
    Color::Rgb(250, 178, 131), // primary / orange
    Color::Rgb(92, 156, 245),  // secondary / blue
    Color::Rgb(157, 124, 216), // accent / purple
    Color::Rgb(127, 216, 143), // success / green
];

/// Application events.
enum AppEvent {
    /// A crossterm terminal event.
    Terminal(Event),
    /// A periodic tick for rendering.
    Tick,
}

/// Reads crossterm events and ticks, sending them to the app.
struct EventHandler {
    rx: mpsc::UnboundedReceiver<AppEvent>,
}

impl EventHandler {
    fn new() -> Self {
        let (tx, rx) = mpsc::unbounded_channel();

        tokio::spawn(async move {
            let mut reader = crossterm::event::EventStream::new();
            let mut tick = tokio::time::interval(tokio::time::Duration::from_millis(33));

            loop {
                tokio::select! {
                    _ = tx.closed() => break,
                    _ = tick.tick() => {
                        if tx.send(AppEvent::Tick).is_err() {
                            break;
                        }
                    }
                    Some(Ok(evt)) = reader.next() => {
                        if tx.send(AppEvent::Terminal(evt)).is_err() {
                            break;
                        }
                    }
                }
            }
        });

        Self { rx }
    }

    async fn next(&mut self) -> Option<AppEvent> {
        self.rx.recv().await
    }
}

struct App {
    text_field: TextField,
    message_list: MessageList,
    scroll_box: ScrollBox,
    should_quit: bool,
    theme: Theme,
    border_color_idx: usize,
}

impl App {
    fn new() -> Self {
        let theme = Theme::default();
        let mut text_field = TextField::new(theme);
        text_field.set_padding(Padding::uniform(1));
        text_field.set_max_height(8);
        text_field.set_border_color(BORDER_COLORS[0]);

        let mut message_list = MessageList::new(theme);
        message_list.set_padding(Padding::symmetric(1, 0));

        Self {
            text_field,
            message_list,
            scroll_box: ScrollBox::new(),
            should_quit: false,
            theme,
            border_color_idx: 0,
        }
    }

    fn cycle_border_color(&mut self) {
        self.border_color_idx = (self.border_color_idx + 1) % BORDER_COLORS.len();
        self.text_field
            .set_border_color(BORDER_COLORS[self.border_color_idx]);
    }

    async fn run(&mut self, mut terminal: DefaultTerminal) -> io::Result<()> {
        // Enable enhanced keyboard protocol for Shift+Enter detection.
        // Falls back gracefully on terminals that don't support it.
        let _ = execute!(
            io::stdout(),
            PushKeyboardEnhancementFlags(KeyboardEnhancementFlags::DISAMBIGUATE_ESCAPE_CODES)
        );
        let _ = execute!(io::stdout(), EnableMouseCapture);

        let mut events = EventHandler::new();

        loop {
            terminal.draw(|frame| {
                let area = frame.area();

                // Fill entire background
                let bg =
                    ratatui::widgets::Block::default().style(Style::default().bg(self.theme.bg));
                bg.render(area, frame.buffer_mut());

                // Compute dynamic text field height
                let input_height = self.text_field.required_height(area.width);

                // Split: scroll box on top, input at bottom
                let chunks =
                    Layout::vertical([Constraint::Min(3), Constraint::Length(input_height)])
                        .split(area);

                // Render scroll box with message list
                self.scroll_box
                    .render(chunks[0], frame.buffer_mut(), &self.message_list);

                // Render text field
                (&self.text_field).render(chunks[1], frame.buffer_mut());

                // Set cursor
                if let Some(pos) = self.text_field.cursor_position(chunks[1]) {
                    frame.set_cursor_position(pos);
                }
            })?;

            match events.next().await {
                Some(AppEvent::Terminal(Event::Key(key))) => {
                    // Only handle key press events (not release/repeat)
                    if key.kind != KeyEventKind::Press {
                        continue;
                    }
                    let input = Input::from(key);
                    match input {
                        Input::Esc => self.should_quit = true,
                        Input::Tab => self.cycle_border_color(),
                        Input::PageUp => self.scroll_box.page_up(),
                        Input::PageDown => self.scroll_box.page_down(),
                        _ => {
                            if let TextFieldAction::Submit(content) = self.text_field.input(input) {
                                self.message_list.push(Message {
                                    role: Role::User,
                                    content,
                                });
                                self.scroll_box.scroll_to_bottom();
                            }
                        }
                    }
                }
                Some(AppEvent::Terminal(Event::Mouse(mouse))) => match mouse.kind {
                    MouseEventKind::ScrollUp => self.scroll_box.scroll_up(3),
                    MouseEventKind::ScrollDown => self.scroll_box.scroll_down(3),
                    _ => {}
                },
                Some(AppEvent::Terminal(Event::Resize(_, _))) => {
                    // Layout recomputes automatically on next draw
                }
                Some(AppEvent::Tick) | Some(AppEvent::Terminal(_)) => {}
                None => break,
            }

            if self.should_quit {
                break;
            }
        }

        let _ = execute!(io::stdout(), PopKeyboardEnhancementFlags);
        let _ = execute!(io::stdout(), DisableMouseCapture);

        Ok(())
    }
}

#[tokio::main]
async fn main() -> io::Result<()> {
    let terminal = ratatui::init();
    let mut app = App::new();
    let result = app.run(terminal).await;
    ratatui::restore();
    result
}
