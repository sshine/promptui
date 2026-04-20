use std::io;

use crossterm::event::{
    DisableMouseCapture, EnableMouseCapture, Event, KeyEventKind, KeyboardEnhancementFlags,
    MouseEventKind, PopKeyboardEnhancementFlags, PushKeyboardEnhancementFlags,
};
use crossterm::execute;
use futures::StreamExt;
use ratatui::DefaultTerminal;
use ratatui::layout::{Constraint, Layout};
use ratatui::style::Style;
use ratatui::widgets::Widget;
use tokio::sync::mpsc;

use rand::Rng;

use promptui_core::input::Input;
use promptui_core::theme::Theme;
use promptui_core::widgets::message_list::{Message, MessageList, Role};
use promptui_core::widgets::padding::Padding;
use promptui_core::widgets::scroll_box::ScrollBox;
use promptui_core::widgets::status_bar::StatusBar;
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
    scroll_box: ScrollBox,
    status_bar: StatusBar,
    should_quit: bool,
    theme: Theme,
}

impl App {
    fn new() -> Self {
        let theme = Theme::default();

        let mut text_field = TextField::new(theme);
        text_field.set_padding(Padding::uniform(1));
        text_field.set_max_height(8);

        let mut message_list = MessageList::new(theme);
        message_list.set_padding(Padding::uniform(1));

        let mut status_bar = StatusBar::new(theme, 200_000);
        status_bar.set_padding(Padding::uniform(1));
        text_field.set_border_color(status_bar.mode().color(&theme));
        // Set cwd, replacing home dir with ~
        if let Ok(cwd) = std::env::current_dir() {
            let mut display = cwd.display().to_string();
            if let Ok(home) = std::env::var("HOME") {
                if display.starts_with(&home) {
                    display = format!("~{}", &display[home.len()..]);
                }
            }
            status_bar.set_cwd(display);
        }

        Self {
            text_field,
            message_list,
            scroll_box: ScrollBox::new(),
            status_bar,
            should_quit: false,
            theme,
        }
    }

    async fn run(&mut self, mut terminal: DefaultTerminal) -> io::Result<()> {
        let _ = execute!(
            io::stdout(),
            PushKeyboardEnhancementFlags(KeyboardEnhancementFlags::DISAMBIGUATE_ESCAPE_CODES)
        );
        let _ = execute!(io::stdout(), EnableMouseCapture);

        let mut events = EventHandler::new();

        loop {
            // Update context state from message list
            self.status_bar
                .context_mut()
                .set_word_count(self.message_list.total_word_count());

            terminal.draw(|frame| {
                let area = frame.area();

                // Fill entire background
                let bg =
                    ratatui::widgets::Block::default().style(Style::default().bg(self.theme.bg));
                bg.render(area, frame.buffer_mut());

                let input_height = self.text_field.required_height(area.width);

                let status_height = self.status_bar.height();
                let chunks = Layout::vertical([
                    Constraint::Min(3),
                    Constraint::Length(status_height),
                    Constraint::Length(input_height),
                ])
                .split(area);

                self.scroll_box
                    .render(chunks[0], frame.buffer_mut(), &self.message_list);
                (&self.status_bar).render(chunks[1], frame.buffer_mut());
                (&self.text_field).render(chunks[2], frame.buffer_mut());

                if let Some(pos) = self.text_field.cursor_position(chunks[2]) {
                    frame.set_cursor_position(pos);
                }
            })?;

            match events.next().await {
                Some(AppEvent::Terminal(Event::Key(key))) => {
                    if key.kind != KeyEventKind::Press {
                        continue;
                    }
                    let input = Input::from(key);
                    match input {
                        Input::Esc => self.should_quit = true,
                        Input::Tab => {
                            let color = self.status_bar.cycle_mode();
                            self.text_field.set_border_color(color);
                        }
                        Input::PageUp => self.scroll_box.page_up(),
                        Input::PageDown => self.scroll_box.page_down(),
                        _ => {
                            if let TextFieldAction::Submit(content) = self.text_field.input(input) {
                                self.message_list.push(Message {
                                    role: Role::User,
                                    content,
                                });
                                self.message_list.push(Message {
                                    role: Role::Assistant,
                                    content: mock_reply(),
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
                Some(AppEvent::Terminal(Event::Resize(_, _))) => {}
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

const MOCK_REPLIES: [&str; 10] = [
    "That's a great idea! Let me think about how to approach this.",
    "Interesting thought. I'll need a moment to consider the best way forward.",
    "Love it. Give me a second to work through the details.",
    "Great suggestion! Let me mull that over.",
    "Ooh, that's a fun one. Let me think on it.",
    "Solid plan. I'll chew on that for a bit.",
    "Nice, I like where this is heading. Let me reflect on it.",
    "That's worth exploring. Give me a moment to think it through.",
    "Brilliant. Let me sit with that idea for a moment.",
    "Fascinating. I'll ponder that and get back to you.",
];

fn mock_reply() -> String {
    let idx = rand::rng().random_range(0..MOCK_REPLIES.len());
    MOCK_REPLIES[idx].to_string()
}

#[tokio::main]
async fn main() -> io::Result<()> {
    let terminal = ratatui::init();
    let mut app = App::new();
    let result = app.run(terminal).await;
    ratatui::restore();
    result
}
