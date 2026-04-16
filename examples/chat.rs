use std::io;

use crossterm::event::{
    DisableMouseCapture, EnableMouseCapture, Event, KeyEventKind, MouseEventKind,
    KeyboardEnhancementFlags, PopKeyboardEnhancementFlags, PushKeyboardEnhancementFlags,
};
use crossterm::execute;
use futures::StreamExt;
use ratatui::layout::{Constraint, Layout};
use ratatui::style::Style;
use ratatui::widgets::Widget;
use ratatui::DefaultTerminal;
use tokio::sync::mpsc;

use promptui_core::input::Input;
use promptui_core::theme::Theme;
use promptui_core::widgets::message_list::{Message, MessageList, Role};
use promptui_core::widgets::text_field::{TextField, TextFieldAction};

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
    should_quit: bool,
    theme: Theme,
}

impl App {
    fn new() -> Self {
        let theme = Theme::default();
        Self {
            text_field: TextField::new(theme),
            message_list: MessageList::new(theme),
            should_quit: false,
            theme,
        }
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
                let bg = ratatui::widgets::Block::default()
                    .style(Style::default().bg(self.theme.bg));
                bg.render(area, frame.buffer_mut());

                // Compute dynamic text field height
                let input_height = self.text_field.required_height(area.width);

                // Split: messages on top, input at bottom
                let chunks = Layout::vertical([
                    Constraint::Min(3),
                    Constraint::Length(input_height),
                ])
                .split(area);

                // Render widgets
                (&self.message_list).render(chunks[0], frame.buffer_mut());
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
                        _ => {
                            if let TextFieldAction::Submit(content) = self.text_field.input(input) {
                                self.message_list.push(Message {
                                    role: Role::User,
                                    content,
                                });
                            }
                        }
                    }
                }
                Some(AppEvent::Terminal(Event::Mouse(mouse))) => {
                    match mouse.kind {
                        MouseEventKind::ScrollUp => self.message_list.scroll_up(3),
                        MouseEventKind::ScrollDown => self.message_list.scroll_down(3),
                        _ => {}
                    }
                }
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
