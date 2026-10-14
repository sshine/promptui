use std::io;
use std::time::Duration;

use crossterm::event::{
    DisableMouseCapture, EnableMouseCapture, Event, KeyEventKind, KeyboardEnhancementFlags,
    MouseEventKind, PopKeyboardEnhancementFlags, PushKeyboardEnhancementFlags,
};
use crossterm::execute;
use futures::StreamExt;
use rand::Rng;
use ratatui::DefaultTerminal;
use ratatui::layout::{Constraint, Layout};
use ratatui::style::{Color, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::Widget;
use tokio::sync::mpsc;

use promptui_core::focus::{FocusAction, FocusId, FocusStack, Focusable};
use promptui_core::input::Input;
use promptui_core::theme::Theme;
use promptui_core::widgets::agent_overview::{AgentEntry, AgentOverview, OverviewMode};
use promptui_core::widgets::agent_scope::AgentStatus;
use promptui_core::widgets::message_list::{ContentBlock, Message, MessageList, Role};
use promptui_core::widgets::notice::{Notice, NoticeLevel};
use promptui_core::widgets::padding::Padding;
use promptui_core::widgets::questionnaire::{Question, QuestionOption, Questionnaire};
use promptui_core::widgets::scroll_box::ScrollBox;
use promptui_core::widgets::spinner::{Spinner, SpinnerStyle};
use promptui_core::widgets::status_bar::{Mode, StatusBar};
use promptui_core::widgets::streaming_text::StreamingText;
use promptui_core::widgets::text_field::{TextField, TextFieldAction};

enum AppEvent {
    Terminal(Event),
    Tick,
    StreamStart(String),
    ShellOutput {
        output: String,
    },
    AgentStep {
        agent_index: usize,
        step_text: String,
        is_final: bool,
    },
}

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

const AGENT_COLORS: [Color; 5] = [
    Color::Rgb(92, 156, 245),
    Color::Rgb(127, 216, 143),
    Color::Rgb(245, 167, 66),
    Color::Rgb(157, 124, 216),
    Color::Rgb(86, 182, 194),
];

struct App {
    text_field: TextField,
    message_list: MessageList,
    scroll_box: ScrollBox,
    status_bar: StatusBar,
    spinner: Spinner,
    notice: Notice,
    streaming: Option<StreamingText>,
    thinking_verb: String,
    should_quit: bool,
    theme: Theme,
    focus_stack: FocusStack,
    agent_overview: AgentOverview,
    questionnaire: Option<Questionnaire>,
    next_agent_id: u64,
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

        let mut agent_overview = AgentOverview::new(theme);
        agent_overview.set_padding(Padding::symmetric(1, 0));
        agent_overview.push(
            AgentEntry::new(0, "main".into())
                .color(AGENT_COLORS[0])
                .checked(true)
                .status(AgentStatus::Active),
        );

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
            focus_stack: FocusStack::new(),
            agent_overview,
            questionnaire: None,
            next_agent_id: 1,
        }
    }

    fn spawn_mock_agents(&mut self, tx: &mpsc::UnboundedSender<AppEvent>) {
        let agent_count = rand::rng().random_range(3..=5usize);
        let agent_tasks = [
            (
                "research",
                vec![
                    "Searching codebase...",
                    "Found 12 relevant files",
                    "Analyzing patterns...",
                    "Research complete",
                ],
            ),
            (
                "architect",
                vec![
                    "Evaluating approaches...",
                    "Designing module structure",
                    "Checking dependencies...",
                    "Architecture plan ready",
                ],
            ),
            (
                "implementer",
                vec![
                    "Writing core logic...",
                    "Adding error handling",
                    "Refactoring helpers...",
                    "Implementation done",
                ],
            ),
            (
                "tester",
                vec![
                    "Generating test cases...",
                    "Running unit tests",
                    "Checking edge cases...",
                    "All tests pass",
                ],
            ),
            (
                "reviewer",
                vec![
                    "Scanning for issues...",
                    "Checking style guide",
                    "Verifying correctness...",
                    "Review complete",
                ],
            ),
        ];

        self.message_list.push(Message::plain(
            Role::Assistant,
            format!("Spawning {} agents to work on this...", agent_count),
        ));

        for i in 0..agent_count {
            let id = self.next_agent_id;
            self.next_agent_id += 1;
            let (name, _) = &agent_tasks[i];
            let color = AGENT_COLORS[i % AGENT_COLORS.len()];

            self.agent_overview.push(
                AgentEntry::new(id, name.to_string())
                    .color(color)
                    .checked(true)
                    .depth(1)
                    .status(AgentStatus::Active)
                    .activity("Starting...".into()),
            );

            let tx = tx.clone();
            let steps: Vec<String> = agent_tasks[i].1.iter().map(|s| s.to_string()).collect();
            let agent_index = self.agent_overview.agent_count() - 1;

            tokio::spawn(async move {
                for (step_i, step_text) in steps.iter().enumerate() {
                    let delay = rand::rng().random_range(800..=2500);
                    tokio::time::sleep(Duration::from_millis(delay)).await;
                    let is_final = step_i + 1 == steps.len();
                    let _ = tx.send(AppEvent::AgentStep {
                        agent_index,
                        step_text: step_text.clone(),
                        is_final,
                    });
                }
            });
        }

        self.scroll_box.scroll_to_bottom();
    }

    fn make_plan_questionnaire(&self) -> Questionnaire {
        Questionnaire::new(
            self.theme,
            vec![
                Question::single_select(
                    "What implementation approach do you prefer?",
                    vec![
                        QuestionOption::new("Incremental refactor — change as little as possible"),
                        QuestionOption::new("Clean rewrite — start fresh with new patterns"),
                        QuestionOption::new("Hybrid — rewrite core, adapt edges"),
                        QuestionOption::new("Other").with_inline_text(),
                    ],
                ),
                Question::multi_select(
                    "Which concerns should be prioritized?",
                    vec![
                        QuestionOption::new("Backward compatibility"),
                        QuestionOption::new("Performance"),
                        QuestionOption::new("Code readability"),
                        QuestionOption::new("Test coverage"),
                    ],
                ),
                Question::single_select(
                    "How should sub-tasks be parallelized?",
                    vec![
                        QuestionOption::new("Sequential — one agent at a time"),
                        QuestionOption::new("Parallel — all agents simultaneously"),
                        QuestionOption::new("Adaptive — start sequential, fan out if safe"),
                    ],
                ),
            ],
        )
    }

    fn make_ralph_questionnaire(&self) -> Questionnaire {
        Questionnaire::new(
            self.theme,
            vec![
                Question::single_select(
                    "What should Ralph optimize for?",
                    vec![
                        QuestionOption::new("Raw speed — damn the torpedoes"),
                        QuestionOption::new("Minimal diff — fewest lines changed"),
                        QuestionOption::new("Maximum chaos — rewrite everything in a new paradigm"),
                        QuestionOption::new("Custom target").with_inline_text(),
                    ],
                ),
                Question::multi_select(
                    "Which safety rails should Ralph ignore?",
                    vec![
                        QuestionOption::new("Type safety"),
                        QuestionOption::new("Error handling"),
                        QuestionOption::new("Backwards compatibility"),
                        QuestionOption::new("Code review norms"),
                        QuestionOption::new("All of the above"),
                    ],
                ),
            ],
        )
    }

    async fn run(&mut self, mut terminal: DefaultTerminal) -> io::Result<()> {
        let _ = execute!(
            io::stdout(),
            PushKeyboardEnhancementFlags(KeyboardEnhancementFlags::DISAMBIGUATE_ESCAPE_CODES)
        );
        let _ = execute!(io::stdout(), EnableMouseCapture);

        let mut events = EventHandler::new();

        loop {
            self.spinner.tick();
            self.notice.tick();
            if let Some(ref mut streaming) = self.streaming {
                if streaming.tick() {
                    self.message_list
                        .set_last_content(streaming.revealed_text().to_string());
                    self.scroll_box.scroll_to_bottom();
                }
                if streaming.is_finished() {
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

                let agent_mode = self.agent_overview.mode();
                let overview_height = self.agent_overview.required_height();
                let is_full_mode = agent_mode == OverviewMode::Full;
                let is_questionnaire = self.focus_stack.top() == FocusId::Questionnaire
                    && self.questionnaire.is_some();

                // TextField summary mode when another component dominates
                self.text_field
                    .set_summary_mode(is_full_mode || is_questionnaire);
                let input_height = self.text_field.required_height(area.width);
                let status_height = self.status_bar.height();

                let chunks = Layout::vertical([
                    Constraint::Length(overview_height),
                    Constraint::Min(3),
                    Constraint::Length(status_height),
                    Constraint::Length(input_height),
                ])
                .split(area);

                // Agent overview header (Collapsed or Summary)
                if overview_height > 0 {
                    (&self.agent_overview).render(chunks[0], frame.buffer_mut());
                }

                // Main content area
                if is_full_mode {
                    self.agent_overview
                        .render_full_view(chunks[1], frame.buffer_mut());
                } else if is_questionnaire {
                    if let Some(ref q) = self.questionnaire {
                        (q).render(chunks[1], frame.buffer_mut());
                    }
                } else {
                    self.scroll_box
                        .render(chunks[1], frame.buffer_mut(), &self.message_list);
                    (&self.notice).render(chunks[1], frame.buffer_mut());
                }

                (&self.status_bar).render(chunks[2], frame.buffer_mut());
                (&self.text_field).render(chunks[3], frame.buffer_mut());

                // Cursor
                let focused = self.focus_stack.top();
                if is_questionnaire {
                    if let Some(ref q) = self.questionnaire
                        && let Some(pos) = q.cursor_position(chunks[1])
                    {
                        frame.set_cursor_position(pos);
                    }
                } else if focused == FocusId::TextField
                    && let Some(pos) = self.text_field.cursor_position(chunks[3])
                {
                    frame.set_cursor_position(pos);
                }
            })?;

            match events.next().await {
                Some(AppEvent::Terminal(Event::Key(key))) => {
                    if key.kind != KeyEventKind::Press {
                        continue;
                    }
                    let input = Input::from(key);

                    // Global shortcuts
                    match input {
                        Input::CtrlA => {
                            if self.agent_overview.agent_count() > 1 {
                                let new_mode = self.agent_overview.cycle_mode();
                                match new_mode {
                                    OverviewMode::Full => {
                                        if self.focus_stack.top() != FocusId::AgentOverview {
                                            self.text_field.set_focused(false);
                                            self.focus_stack.push(FocusId::AgentOverview);
                                            self.agent_overview.on_focus();
                                        }
                                    }
                                    OverviewMode::Collapsed | OverviewMode::Summary => {
                                        if self.focus_stack.top() == FocusId::AgentOverview {
                                            self.focus_stack.pop();
                                            self.agent_overview.on_blur();
                                            self.text_field.set_focused(true);
                                        }
                                    }
                                }
                            }
                            continue;
                        }
                        Input::Esc if self.focus_stack.depth() == 1 => {
                            self.should_quit = true;
                            continue;
                        }
                        _ => {}
                    }

                    // Dispatch to focused component
                    match self.focus_stack.top() {
                        FocusId::TextField => match input {
                            Input::Tab => {
                                let color = self.status_bar.cycle_mode();
                                self.text_field.set_border_color(color);
                            }
                            Input::PageUp => self.scroll_box.page_up(),
                            Input::PageDown => self.scroll_box.page_down(),
                            _ => {
                                if let TextFieldAction::Submit(content) =
                                    self.text_field.input(input)
                                {
                                    self.handle_submit(content, &events.sender());
                                }
                            }
                        },
                        FocusId::AgentOverview => {
                            let action = self.agent_overview.handle_input(input);
                            if action == FocusAction::Pop {
                                self.focus_stack.pop();
                                self.agent_overview.on_blur();
                                self.text_field.set_focused(true);
                            }
                        }
                        FocusId::Questionnaire => {
                            if let Some(ref mut q) = self.questionnaire {
                                let action = q.handle_input(input);
                                if action == FocusAction::Pop {
                                    let completed = q.is_completed();
                                    let summary_action = q.summary_action();
                                    let summary = q.summary_text();
                                    let answered = q.answered_count();
                                    let total = q.question_count();
                                    q.on_blur();

                                    self.focus_stack.pop();
                                    self.text_field.set_focused(true);

                                    self.message_list.push_block(
                                        ContentBlock::QuestionnaireSummary {
                                            summary_text: summary,
                                            progress: (answered, total),
                                        },
                                    );
                                    self.scroll_box.scroll_to_bottom();

                                    if completed {
                                        // Switch to chosen mode
                                        let target_mode = if summary_action == 0 {
                                            Mode::Accept
                                        } else {
                                            Mode::Auto
                                        };
                                        // Cycle status bar to the target mode
                                        while self.status_bar.mode() != target_mode {
                                            self.status_bar.cycle_mode();
                                        }
                                        self.text_field
                                            .set_border_color(target_mode.color(&self.theme));

                                        let tx = events.sender();
                                        self.spawn_mock_agents(&tx);
                                        self.notice.show(
                                            NoticeLevel::Info,
                                            format!(
                                                "Plan accepted in {:?} mode — agents spawned",
                                                target_mode
                                            ),
                                        );
                                    } else {
                                        self.notice.show(NoticeLevel::Warn, "Plan cancelled");
                                    }

                                    self.questionnaire = None;
                                }
                            }
                        }
                    }
                }
                Some(AppEvent::StreamStart(reply)) => {
                    self.spinner.stop();
                    self.status_bar.set_activity(String::new());
                    self.message_list.push(Message::plain(Role::Assistant, ""));
                    let mut streaming = StreamingText::new(self.theme);
                    streaming.push_str(&reply);
                    streaming.finish();
                    self.streaming = Some(streaming);
                    self.scroll_box.scroll_to_bottom();
                }
                Some(AppEvent::ShellOutput { output }) => {
                    self.spinner.stop();
                    self.status_bar.set_activity(String::new());
                    let output = output.trim_end().to_string();
                    self.message_list.push(Message::plain(Role::System, output));
                    self.scroll_box.scroll_to_bottom();
                }
                Some(AppEvent::AgentStep {
                    agent_index,
                    step_text,
                    is_final,
                }) => {
                    // Update agent entry
                    if let Some(agent) = self.agent_overview.agent_mut(agent_index) {
                        agent.activity = step_text.clone();
                        if is_final {
                            agent.status = AgentStatus::Done;
                        }
                        let current = agent.duration.unwrap_or(Duration::ZERO);
                        agent.duration =
                            Some(current + Duration::from_secs(rand::rng().random_range(2..=5)));
                        agent.input_tokens += rand::rng().random_range(200..=600);
                        agent.output_tokens += rand::rng().random_range(100..=400);
                    }

                    // Push to per-agent message list
                    let agent_label = self
                        .agent_overview
                        .agents()
                        .get(agent_index)
                        .map(|a| a.label.clone())
                        .unwrap_or_default();
                    let agent_color = self
                        .agent_overview
                        .agents()
                        .get(agent_index)
                        .map(|a| a.color)
                        .unwrap_or(self.theme.text_muted);

                    let prefix = if is_final { "\u{2713}" } else { "\u{25CF}" };
                    let styled_line = Line::from(vec![Span::styled(
                        format!("{} {}", prefix, step_text),
                        Style::default().fg(if is_final {
                            self.theme.success
                        } else {
                            self.theme.text
                        }),
                    )]);

                    self.agent_overview.push_agent_message(
                        agent_index,
                        Message::styled(Role::System, styled_line.clone().into()),
                    );

                    // Also push compact summary to global message list
                    let global_line = Line::from(vec![
                        Span::styled(
                            format!("[{}] ", agent_label),
                            Style::default().fg(agent_color),
                        ),
                        Span::styled(
                            format!("{} {}", prefix, step_text),
                            Style::default().fg(if is_final {
                                self.theme.success
                            } else {
                                self.theme.text
                            }),
                        ),
                    ]);
                    self.message_list
                        .push(Message::styled(Role::System, global_line.into()));
                    self.scroll_box.scroll_to_bottom();

                    // Check all done
                    let all_done = self
                        .agent_overview
                        .agents()
                        .iter()
                        .skip(1)
                        .all(|a| a.status == AgentStatus::Done);
                    if all_done && self.agent_overview.agent_count() > 1 {
                        self.notice.show(NoticeLevel::Info, "All agents completed");
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

    fn handle_submit(&mut self, content: String, tx: &mpsc::UnboundedSender<AppEvent>) {
        let mode = self.status_bar.mode();

        match mode {
            Mode::Plan => {
                self.message_list.push(Message::plain(Role::User, &content));
                self.scroll_box.scroll_to_bottom();
                self.message_list.push(Message::plain(
                    Role::Assistant,
                    "I have some planning questions before proceeding:",
                ));
                self.scroll_box.scroll_to_bottom();

                let mut q = self.make_plan_questionnaire();
                self.text_field.set_focused(false);
                q.on_focus();
                self.questionnaire = Some(q);
                self.focus_stack.push(FocusId::Questionnaire);
            }
            Mode::Ralph => {
                self.message_list.push(Message::plain(Role::User, &content));
                self.scroll_box.scroll_to_bottom();
                self.message_list.push(Message::plain(
                    Role::Assistant,
                    "RALPH MODE ENGAGED. Before I go full send, some calibration:",
                ));
                self.scroll_box.scroll_to_bottom();

                let mut q = self.make_ralph_questionnaire();
                self.text_field.set_focused(false);
                q.on_focus();
                self.questionnaire = Some(q);
                self.focus_stack.push(FocusId::Questionnaire);
            }
            Mode::Talk => {
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

                    let tx = tx.clone();
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
                } else {
                    self.message_list.push(Message::plain(Role::User, content));
                    self.scroll_box.scroll_to_bottom();
                    self.notice
                        .show(NoticeLevel::Error, "Could not connect. Are you online?");

                    self.status_bar
                        .set_latency_ms(rand::rng().random_range(10..=150));

                    self.thinking_verb = thinking_verb();
                    self.spinner.start();

                    let tx = tx.clone();
                    let reply = mock_reply();
                    let delay_ms = rand::rng().random_range(1000..=3000);
                    tokio::spawn(async move {
                        tokio::time::sleep(Duration::from_millis(delay_ms)).await;
                        let _ = tx.send(AppEvent::StreamStart(reply));
                    });
                }
            }
            Mode::Accept | Mode::Auto => {
                self.message_list.push(Message::plain(Role::User, &content));
                self.scroll_box.scroll_to_bottom();
                self.notice.show(
                    NoticeLevel::Warn,
                    format!("{:?} mode: tool UI not yet implemented", mode),
                );
            }
        }
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

#[tokio::main]
async fn main() -> io::Result<()> {
    let terminal = ratatui::init();
    let mut app = App::new();
    let result = app.run(terminal).await;
    ratatui::restore();
    result
}
