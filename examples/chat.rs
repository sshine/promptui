use std::io;

use crossterm::event::{
    DisableMouseCapture, EnableMouseCapture, Event, KeyEventKind, KeyboardEnhancementFlags,
    MouseEventKind, PopKeyboardEnhancementFlags, PushKeyboardEnhancementFlags,
};
use crossterm::execute;
use futures::StreamExt;
use rand::Rng;
use ratatui::DefaultTerminal;
use ratatui::layout::{Constraint, Layout};
use ratatui::style::Style;
use ratatui::text::{Line, Span};
use ratatui::widgets::Widget;
use tokio::sync::mpsc;

use promptui_core::input::Input;
use promptui_core::theme::Theme;
use promptui_core::widgets::message_list::{ContentBlock, Message, MessageList, Role};
use promptui_core::widgets::notice::{Notice, NoticeLevel};
use promptui_core::widgets::padding::Padding;
use promptui_core::widgets::scroll_box::ScrollBox;
use promptui_core::widgets::spinner::{Spinner, SpinnerStyle};
use promptui_core::widgets::status_bar::{Mode, StatusBar};
use promptui_core::widgets::streaming_text::StreamingText;
use promptui_core::widgets::text_field::{TextField, TextFieldAction};

/// Application events.
enum AppEvent {
    Terminal(Event),
    Tick,
    /// Start streaming a reply after the thinking delay.
    StreamStart(String),
    /// Shell command finished; carries the command string and its output.
    ShellOutput {
        output: String,
    },
    /// Check off the next todo item and post a comment.
    TodoProgress {
        /// Index of the TodoList block in the message list.
        block_index: usize,
        /// Which item to mark done.
        item_index: usize,
        /// Comment to post as an assistant message.
        comment: String,
    },
}

/// Reads crossterm events and ticks, sending them to the app.
struct EventHandler {
    tx: mpsc::UnboundedSender<AppEvent>,
    rx: mpsc::UnboundedReceiver<AppEvent>,
}

impl EventHandler {
    fn new() -> Self {
        let (tx, rx) = mpsc::unbounded_channel();

        let tx2 = tx.clone();
        tokio::spawn(async move {
            let mut reader = crossterm::event::EventStream::new();
            let mut tick = tokio::time::interval(tokio::time::Duration::from_millis(33));

            loop {
                tokio::select! {
                    _ = tx2.closed() => break,
                    _ = tick.tick() => {
                        if tx2.send(AppEvent::Tick).is_err() {
                            break;
                        }
                    }
                    Some(Ok(evt)) = reader.next() => {
                        if tx2.send(AppEvent::Terminal(evt)).is_err() {
                            break;
                        }
                    }
                }
            }
        });

        Self { tx, rx }
    }

    fn sender(&self) -> mpsc::UnboundedSender<AppEvent> {
        self.tx.clone()
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
    spinner: Spinner,
    notice: Notice,
    streaming: Option<StreamingText>,
    /// The verb shown while thinking (e.g. "Pondering...").
    thinking_verb: String,
    should_quit: bool,
    theme: Theme,
}

impl App {
    fn new() -> Self {
        let theme = Theme::default();

        let mut text_field = TextField::new(theme);
        text_field.set_padding(Padding::uniform(1));
        text_field.set_max_height(8);
        let golden = theme.warning;
        text_field.set_style_fn(move |line, col, ch| {
            if line == 0 && col == 0 && ch == '$' {
                Some(Style::default().fg(golden))
            } else {
                None
            }
        });

        let mut message_list = MessageList::new(theme);
        message_list.set_padding(Padding::uniform(1));

        let mut status_bar = StatusBar::new(theme, 200_000);
        status_bar.set_padding(Padding::uniform(1));
        text_field.set_border_color(status_bar.mode().color(&theme));
        if let Ok(cwd) = std::env::current_dir() {
            let mut display = cwd.display().to_string();
            if let Ok(home) = std::env::var("HOME")
                && display.starts_with(&home)
            {
                display = format!("~{}", &display[home.len()..]);
            }
            status_bar.set_cwd(display);
        }
        status_bar.set_latency_ms(rand::rng().random_range(10..=150));

        Self {
            text_field,
            message_list,
            scroll_box: ScrollBox::new(),
            status_bar,
            spinner: Spinner::new(SpinnerStyle::WAVE_RIPPLE),
            notice: {
                let mut n = Notice::new(theme);
                n.set_padding(Padding::uniform(1));
                n
            },
            streaming: None,
            thinking_verb: String::new(),
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
            // Advance spinner, toast timer, streaming reveal, and update status bar
            self.spinner.tick();
            self.notice.tick();
            if let Some(ref mut streaming) = self.streaming {
                if streaming.tick() {
                    self.message_list
                        .set_last_content(streaming.revealed_text().to_string());
                    self.scroll_box.scroll_to_bottom();
                }
                if streaming.is_finished() {
                    // Sync final text and clear streaming state
                    self.message_list
                        .set_last_content(streaming.revealed_text().to_string());
                    self.streaming = None;
                }
            }
            if self.spinner.is_active() {
                self.status_bar.set_activity(format!(
                    "{} {}",
                    self.spinner.frame(),
                    self.thinking_verb
                ));
            }

            self.status_bar
                .context_mut()
                .set_word_count(self.message_list.total_word_count());

            terminal.draw(|frame| {
                let area = frame.area();

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
                (&self.notice).render(chunks[0], frame.buffer_mut());
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
                                if let Some(cmd) = content.strip_prefix('$') {
                                    let cmd = cmd.trim().to_string();
                                    let styled_line = Line::from(vec![
                                        Span::styled("$ ", Style::default().fg(self.theme.warning)),
                                        Span::raw(cmd.clone()),
                                    ]);
                                    self.message_list
                                        .push(Message::styled(Role::User, styled_line.into()));
                                    self.scroll_box.scroll_to_bottom();

                                    self.thinking_verb = "Running...".to_string();
                                    self.spinner.start();

                                    let tx = events.sender();
                                    tokio::spawn(async move {
                                        let result = tokio::process::Command::new("sh")
                                            .arg("-c")
                                            .arg(&cmd)
                                            .output()
                                            .await;
                                        let output = match result {
                                            Ok(out) => {
                                                let stdout = String::from_utf8_lossy(&out.stdout);
                                                let stderr = String::from_utf8_lossy(&out.stderr);
                                                let mut s = stdout.into_owned();
                                                if !stderr.is_empty() {
                                                    if !s.is_empty() {
                                                        s.push('\n');
                                                    }
                                                    s.push_str(&stderr);
                                                }
                                                if s.is_empty() {
                                                    "(no output)".to_string()
                                                } else {
                                                    s
                                                }
                                            }
                                            Err(e) => format!("error: {e}"),
                                        };
                                        let _ = tx.send(AppEvent::ShellOutput { output });
                                    });
                                } else if self.status_bar.mode() == Mode::Accept {
                                    // Accept mode: create a todo list and gradually work through it
                                    self.message_list.push(Message::plain(Role::User, &content));
                                    self.scroll_box.scroll_to_bottom();

                                    let items = make_todo_items(&content);
                                    let block_index = self.message_list.len();
                                    self.message_list.push_block(ContentBlock::TodoList {
                                        title: "Plan".to_string(),
                                        items: items
                                            .iter()
                                            .map(|label| (label.clone(), false))
                                            .collect(),
                                    });
                                    self.scroll_box.scroll_to_bottom();

                                    // Schedule gradual progress
                                    self.thinking_verb = thinking_verb();
                                    self.spinner.start();
                                    let tx = events.sender();
                                    let item_count = items.len();
                                    // Pre-generate delays and comments (ThreadRng isn't Send)
                                    let schedule: Vec<(u64, String)> = (0..item_count)
                                        .map(|i| {
                                            let delay = rand::rng().random_range(1500..=3500);
                                            (delay, todo_comment(i, item_count))
                                        })
                                        .collect();
                                    tokio::spawn(async move {
                                        for (i, (delay, comment)) in
                                            schedule.into_iter().enumerate()
                                        {
                                            tokio::time::sleep(tokio::time::Duration::from_millis(
                                                delay,
                                            ))
                                            .await;
                                            if tx
                                                .send(AppEvent::TodoProgress {
                                                    block_index,
                                                    item_index: i,
                                                    comment,
                                                })
                                                .is_err()
                                            {
                                                break;
                                            }
                                        }
                                    });
                                } else {
                                    self.message_list.push(Message::plain(Role::User, content));
                                    self.scroll_box.scroll_to_bottom();
                                    self.notice.show(
                                        NoticeLevel::Error,
                                        "Could not connect. Are you online?",
                                    );

                                    self.status_bar
                                        .set_latency_ms(rand::rng().random_range(10..=150));

                                    // Start thinking
                                    self.thinking_verb = thinking_verb();
                                    self.spinner.start();

                                    // Schedule the reply after 1-3 seconds
                                    let tx = events.sender();
                                    let reply = mock_reply();
                                    let delay_ms = rand::rng().random_range(1000..=3000);
                                    tokio::spawn(async move {
                                        tokio::time::sleep(tokio::time::Duration::from_millis(
                                            delay_ms,
                                        ))
                                        .await;
                                        let _ = tx.send(AppEvent::StreamStart(reply));
                                    });
                                }
                            }
                        }
                    }
                }
                Some(AppEvent::StreamStart(reply)) => {
                    self.spinner.stop();
                    self.status_bar.set_activity(String::new());
                    // Push an empty assistant message; streaming will fill it
                    self.message_list.push(Message::plain(Role::Assistant, ""));
                    let mut streaming = StreamingText::new(self.theme);
                    streaming.push_str(&reply);
                    streaming.finish();
                    self.streaming = Some(streaming);
                    self.scroll_box.scroll_to_bottom();
                }
                Some(AppEvent::TodoProgress {
                    block_index,
                    item_index,
                    comment,
                }) => {
                    // Mark the item done in the todo list block
                    if let Some(ContentBlock::TodoList { items, .. }) =
                        self.message_list.block_mut(block_index)
                        && let Some(item) = items.get_mut(item_index)
                    {
                        item.1 = true;
                    }

                    // Post a streamed assistant comment
                    self.message_list.push(Message::plain(Role::Assistant, ""));
                    let mut streaming = StreamingText::new(self.theme);
                    streaming.push_str(&comment);
                    streaming.finish();
                    self.streaming = Some(streaming);
                    self.scroll_box.scroll_to_bottom();

                    // Check if all items are done
                    let all_done = if let Some(ContentBlock::TodoList { items, .. }) =
                        self.message_list.blocks().get(block_index)
                    {
                        items.iter().all(|(_, done)| *done)
                    } else {
                        false
                    };
                    if all_done {
                        self.spinner.stop();
                        self.status_bar.set_activity(String::new());
                        self.notice.show(NoticeLevel::Info, "All tasks completed!");
                    } else {
                        self.thinking_verb = thinking_verb();
                    }

                    self.status_bar
                        .set_latency_ms(rand::rng().random_range(10..=150));
                }
                Some(AppEvent::ShellOutput { output }) => {
                    self.spinner.stop();
                    self.status_bar.set_activity(String::new());
                    let output = output.trim_end().to_string();
                    self.message_list.push(Message::plain(Role::System, output));
                    self.scroll_box.scroll_to_bottom();
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

const THINKING_VERBS: [&str; 9] = [
    "Thinking...",
    "Pondering...",
    "Mulling...",
    "Brewing...",
    "Crafting...",
    "Dreaming...",
    "Plotting...",
    "Scheming...",
    "Conjuring...",
];

fn mock_reply() -> String {
    let idx = rand::rng().random_range(0..MOCK_REPLIES.len());
    MOCK_REPLIES[idx].to_string()
}

fn thinking_verb() -> String {
    let idx = rand::rng().random_range(0..THINKING_VERBS.len());
    THINKING_VERBS[idx].to_string()
}

/// Generate todo items based on the user's message.
fn make_todo_items(prompt: &str) -> Vec<String> {
    let word_count = prompt.split_whitespace().count();
    // Pick a set based on rough length so the demo feels responsive
    if word_count <= 3 {
        vec![
            "Parse request".into(),
            "Gather context".into(),
            "Draft response".into(),
        ]
    } else if word_count <= 8 {
        vec![
            "Analyze requirements".into(),
            "Search codebase".into(),
            "Plan changes".into(),
            "Apply edits".into(),
            "Verify result".into(),
        ]
    } else {
        vec![
            "Break down problem".into(),
            "Research approach".into(),
            "Draft implementation plan".into(),
            "Write code changes".into(),
            "Run tests".into(),
            "Review diff".into(),
            "Finalize".into(),
        ]
    }
}

const TODO_COMMENTS: [&str; 10] = [
    "Done. Moving on to the next step.",
    "That went smoothly. Continuing...",
    "Checked. Let me keep going.",
    "All good here. Next up...",
    "Finished that part. Onwards!",
    "Handled. Proceeding to the rest.",
    "Sorted. On to the next one.",
    "Wrapped that up. Let's continue.",
    "Complete. Making progress!",
    "Taken care of. Almost there...",
];

fn todo_comment(item_index: usize, total: usize) -> String {
    if item_index + 1 == total {
        "All done! Everything looks good.".to_string()
    } else {
        let idx = rand::rng().random_range(0..TODO_COMMENTS.len());
        TODO_COMMENTS[idx].to_string()
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
