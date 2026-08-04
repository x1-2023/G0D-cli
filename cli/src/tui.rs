//! Grok-style fullscreen TUI for g0d.
//!
//! Layout (matches the Grok Build shell):
//!   header  — branch/path · token budget · step meter
//!   chat    — user bubbles, ◆ tool lines, assistant text, thinking
//!   input   — rounded `>` box · model · approval
//!   footer  — keybinding hints

use crate::{
    agent::{self, RunOptions},
    commands,
    config::{ApprovalMode, Config},
    context,
    meter,
    output::{AgentEvent, ChannelSink, EventSink},
    session::Session,
    terminal::TerminalState,
};
use anyhow::{Context, Result};
use crossterm::{
    event::{self, Event, KeyCode, KeyEvent, KeyEventKind, KeyModifiers},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use ratatui::{
    backend::CrosstermBackend,
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, BorderType, Clear, Paragraph, Wrap},
    Frame, Terminal,
};
use std::collections::VecDeque;
use std::io::{self, Stdout};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{mpsc, Arc};
use std::time::{Duration, Instant};

const MAX_CHAT_LINES: usize = 2_000;
const MAX_QUEUE: usize = 32;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum LineKind {
    User,
    Assistant,
    Tool,
    ToolResult,
    Notice,
    Warn,
    Usage,
    System,
    Queued,
}

/// Interactive run modes (Shift+Tab cycles, Grok-style).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum TuiMode {
    Chat,
    Godmode,
    Snake,
    Ultra,
}

impl TuiMode {
    fn label(self) -> &'static str {
        match self {
            Self::Chat => "chat",
            Self::Godmode => "godmode",
            Self::Snake => "snake",
            Self::Ultra => "ultra",
        }
    }

}

#[derive(Clone, Debug)]
struct ChatLine {
    kind: LineKind,
    text: String,
    time: Option<String>,
}

struct PendingApproval {
    description: String,
    response: mpsc::Sender<bool>,
}

pub struct TuiOptions {
    pub steps: Option<usize>,
    pub approval: Option<ApprovalMode>,
}

struct App {
    config: Config,
    session: Session,
    term_state: TerminalState,
    lines: VecDeque<ChatLine>,
    scroll: u16,
    input: String,
    cursor: usize,
    status: Option<String>,
    thinking_since: Option<Instant>,
    busy: bool,
    exit: bool,
    steps_override: Option<usize>,
    session_approval: Option<ApprovalMode>,
    agent_rx: Option<mpsc::Receiver<AgentEvent>>,
    pending_approval: Option<PendingApproval>,
    turn_started: Option<Instant>,
    last_error: Option<String>,
    runtime: tokio::runtime::Handle,
    /// Live slash-command suggestions (Tab / auto when input starts with `/`).
    completions: Vec<commands::CompletionItem>,
    completion_index: usize,
    show_completions: bool,
    /// Shift+Tab mode (chat / godmode / snake / ultra).
    mode: TuiMode,
    /// Messages waiting while the agent is busy (Grok-style queue).
    queue: VecDeque<String>,
    /// Set by Esc while busy — agent checks between steps / mid-stream.
    cancel_flag: Option<Arc<AtomicBool>>,
    /// Last chat line is receiving AssistantDelta tokens.
    streaming_assistant: bool,
}

pub async fn run(
    config: Config,
    session: Session,
    term_state: TerminalState,
    options: TuiOptions,
) -> Result<()> {
    let runtime = tokio::runtime::Handle::current();
    enable_raw_mode().context("Could not enable raw mode for TUI")?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen).context("Could not enter alternate screen")?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend).context("Could not create TUI terminal")?;

    let seed: Vec<(LineKind, String)> = session
        .messages
        .iter()
        .filter_map(|message| {
            let role = message.get("role").and_then(|v| v.as_str()).unwrap_or("unknown");
            let content = message
                .get("content")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .trim();
            if content.is_empty() {
                return None;
            }
            let kind = match role {
                "user" => LineKind::User,
                "assistant" => LineKind::Assistant,
                _ => LineKind::System,
            };
            Some((kind, content.to_string()))
        })
        .collect();

    let mut app = App {
        config,
        session,
        term_state,
        lines: VecDeque::new(),
        scroll: 0,
        input: String::new(),
        cursor: 0,
        status: None,
        thinking_since: None,
        busy: false,
        exit: false,
        steps_override: options.steps,
        session_approval: options.approval,
        agent_rx: None,
        pending_approval: None,
        turn_started: None,
        last_error: None,
        runtime,
        completions: Vec::new(),
        completion_index: 0,
        show_completions: false,
        mode: TuiMode::Chat,
        queue: VecDeque::new(),
        cancel_flag: None,
        streaming_assistant: false,
    };

    for (kind, content) in seed {
        app.push_line(kind, content, false);
    }

    if app.lines.is_empty() {
        app.push_line(
            LineKind::System,
            format!(
                "g0d {} · Shift+Tab ask/always-approve · Enter queue/send · Ctrl+Enter now · / for commands.",
                env!("CARGO_PKG_VERSION")
            ),
            false,
        );
        if let Some(inst) = &context::read_context().instructions {
            app.push_line(
                LineKind::Notice,
                format!("Project instructions · {}", inst.source),
                false,
            );
        }
    }

    let result = app_loop(&mut terminal, &mut app).await;

    disable_raw_mode().ok();
    execute!(terminal.backend_mut(), LeaveAlternateScreen).ok();
    terminal.show_cursor().ok();
    result
}

async fn app_loop(
    terminal: &mut Terminal<CrosstermBackend<Stdout>>,
    app: &mut App,
) -> Result<()> {
    while !app.exit {
        app.drain_agent_events();
        // When the agent finishes, automatically start the next queued message.
        if !app.busy && !app.queue.is_empty() && app.pending_approval.is_none() {
            if let Some(next) = app.queue.pop_front() {
                app.push_line(
                    LineKind::Notice,
                    format!("▶ running queued ({} left)", app.queue.len()),
                    false,
                );
                start_agent_turn(app, next).await?;
            }
        }
        terminal.draw(|frame| draw(frame, app))?;

        // Poll UI events; keep loop responsive while agent runs.
        if event::poll(Duration::from_millis(80)).context("TUI event poll failed")? {
            if let Event::Key(key) = event::read().context("TUI event read failed")? {
                if key.kind == KeyEventKind::Press || key.kind == KeyEventKind::Repeat {
                    handle_key(app, key).await?;
                }
            }
        }
    }
    // Persist session on clean exit.
    if !app.session.messages.is_empty() {
        let _ = app.session.save();
    }
    Ok(())
}

impl App {
    fn push_line(&mut self, kind: LineKind, text: String, with_time: bool) {
        let time = if with_time {
            Some(chrono_like_now())
        } else {
            None
        };
        self.lines.push_back(ChatLine { kind, text, time });
        while self.lines.len() > MAX_CHAT_LINES {
            self.lines.pop_front();
        }
        self.scroll = 0;
    }

    fn refresh_completions(&mut self) {
        if !self.input.starts_with('/') {
            self.completions.clear();
            self.completion_index = 0;
            self.show_completions = false;
            return;
        }
        let previous = self
            .completions
            .get(self.completion_index)
            .map(|item| item.value.clone());
        self.completions = commands::complete_line(&self.input, self.cursor);
        if self.completions.is_empty() {
            self.show_completions = false;
            self.completion_index = 0;
            return;
        }
        // Auto-open as soon as the user types `/` (Grok-style), no Tab required.
        self.show_completions = true;
        self.completion_index = previous
            .and_then(|value| {
                self.completions
                    .iter()
                    .position(|item| item.value == value)
            })
            .unwrap_or(0);
    }

    fn accept_completion(&mut self) {
        if self.completions.is_empty() {
            return;
        }
        let item = self.completions[self.completion_index].clone();
        let from = item.replace_from.min(self.input.len());
        let mut next = String::new();
        next.push_str(&self.input[..from]);
        next.push_str(&item.value);
        // Always leave a trailing space so the next arg menu can open.
        if !next.ends_with(' ') {
            next.push(' ');
        }
        self.input = next;
        self.cursor = self.input.len();
        self.refresh_completions();
    }

    fn effective_approval(&self) -> ApprovalMode {
        self.session_approval.unwrap_or(self.config.approval_mode)
    }

    /// Shift+Tab: toggle approval policy (Grok-style ask / always-approve).
    fn cycle_approval(&mut self) {
        let next = match self.effective_approval() {
            ApprovalMode::On => ApprovalMode::Off,
            ApprovalMode::Off => ApprovalMode::On,
        };
        // Process-local override — does not rewrite config.toml unless user uses /approval.
        self.session_approval = Some(next);
        let label = match next {
            ApprovalMode::On => "ask",
            ApprovalMode::Off => "always-approve",
        };
        self.push_line(
            LineKind::Notice,
            format!("Approval → {label}  (Shift+Tab to toggle · /approval to persist)"),
            false,
        );
    }

    fn enqueue(&mut self, text: String, front: bool) {
        if self.queue.len() >= MAX_QUEUE {
            self.push_line(
                LineKind::Warn,
                format!("Queue full ({MAX_QUEUE}). Wait for the agent to finish."),
                false,
            );
            return;
        }
        if front {
            self.queue.push_front(text.clone());
        } else {
            self.queue.push_back(text.clone());
        }
        let preview: String = text.chars().take(80).collect();
        self.push_line(
            LineKind::Queued,
            format!(
                "queued #{}{} · {preview}",
                self.queue.len(),
                if front { " (next)" } else { "" }
            ),
            true,
        );
    }

    fn meter_label(&self) -> String {
        let meter = self.session.meter(&self.config);
        let q = if self.queue.is_empty() {
            String::new()
        } else {
            format!(" · q{}", self.queue.len())
        };
        format!(
            "{} / {} | {}/{} ✓{q}",
            meter::format_token_count(meter.estimated_tokens).to_uppercase(),
            meter::format_token_count(meter.budget).to_uppercase(),
            (self.session.messages.len() / 2).min(99),
            self.steps_override
                .unwrap_or(self.config.max_agent_steps)
        )
    }

    fn drain_agent_events(&mut self) {
        let Some(rx) = self.agent_rx.take() else {
            return;
        };
        loop {
            match rx.try_recv() {
                Ok(event) => match event {
                    AgentEvent::Status(text) => {
                        if self.thinking_since.is_none() {
                            self.thinking_since = Some(Instant::now());
                        }
                        self.status = Some(text);
                    }
                    AgentEvent::ClearStatus => {
                        self.status = None;
                    }
                    AgentEvent::Notice(text) => {
                        if !text.is_empty() {
                            self.push_line(LineKind::Notice, text, false);
                        }
                    }
                    AgentEvent::Warn(text) => {
                        self.streaming_assistant = false;
                        self.push_line(LineKind::Warn, text, false);
                    }
                    AgentEvent::Tool { name, preview } => {
                        self.streaming_assistant = false;
                        let label = human_tool_line(&name, &preview);
                        self.push_line(LineKind::Tool, label, false);
                    }
                    AgentEvent::ToolResult(summary) => {
                        self.streaming_assistant = false;
                        self.push_line(LineKind::ToolResult, summary, false);
                    }
                    AgentEvent::Assistant(text) => {
                        self.streaming_assistant = false;
                        self.push_line(LineKind::Assistant, text, true);
                    }
                    AgentEvent::AssistantDelta(delta) => {
                        if delta == "\n" {
                            self.streaming_assistant = false;
                        } else if self.streaming_assistant {
                            if let Some(last) = self.lines.back_mut() {
                                if last.kind == LineKind::Assistant {
                                    last.text.push_str(&delta);
                                } else {
                                    self.streaming_assistant = true;
                                    self.push_line(LineKind::Assistant, delta, true);
                                }
                            }
                        } else {
                            self.streaming_assistant = true;
                            self.push_line(LineKind::Assistant, delta, true);
                        }
                    }
                    AgentEvent::Usage(text) => {
                        self.streaming_assistant = false;
                        self.push_line(LineKind::Usage, text, false);
                    }
                    AgentEvent::Cancelled => {
                        self.streaming_assistant = false;
                        self.push_line(LineKind::Warn, "Cancelled.".into(), false);
                    }
                    AgentEvent::NeedApproval {
                        description,
                        response,
                    } => {
                        self.pending_approval = Some(PendingApproval {
                            description,
                            response,
                        });
                    }
                },
                Err(mpsc::TryRecvError::Empty) => {
                    self.agent_rx = Some(rx);
                    break;
                }
                Err(mpsc::TryRecvError::Disconnected) => {
                    self.busy = false;
                    self.status = None;
                    self.thinking_since = None;
                    self.cancel_flag = None;
                    self.streaming_assistant = false;
                    if let Ok(latest) = Session::load(&self.session.id) {
                        self.session = latest;
                    }
                    break;
                }
            }
        }
    }
}

/// Humanize tool lines like the Grok UI ("Read 2 files, Searched 2 patterns").
fn human_tool_line(name: &str, preview: &str) -> String {
    let preview = preview.trim().trim_matches(|c| c == '(' || c == ')');
    match name {
        "read_file" => format!("Read {preview}"),
        "list_files" => format!("Listed {preview}"),
        "glob_files" => format!("Glob {preview}"),
        "search_files" => format!("Searched {preview}"),
        "replace_in_file" => format!("Edited {preview}"),
        "create_file" | "write_file" => format!("Wrote {preview}"),
        "delete_file" => format!("Deleted {preview}"),
        "rename_file" => format!("Renamed {preview}"),
        "run_command" => format!("Ran `{preview}`"),
        "git_status" => "Git status".into(),
        "git_diff" => format!("Git diff ({preview})"),
        "git_log" => "Git log".into(),
        "git_add" => format!("Git add {preview}"),
        "git_commit" => format!("Git commit {preview}"),
        "apply_patch" => "Applied patch".into(),
        other => format!("{other} {preview}"),
    }
}

fn chrono_like_now() -> String {
    // Local HH:MM without extra deps.
    use std::time::{SystemTime, UNIX_EPOCH};
    let secs = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);
    // Approximate local by using UTC — good enough for UI chrome; Windows local offset is non-trivial without chrono.
    let mins = (secs / 60) % (24 * 60);
    let h = mins / 60;
    let m = mins % 60;
    format!("{h:02}:{m:02}")
}

async fn handle_key(app: &mut App, key: KeyEvent) -> Result<()> {
    // Approval modal takes priority.
    if app.pending_approval.is_some() {
        match key.code {
            KeyCode::Char('y') | KeyCode::Char('Y') | KeyCode::Enter => {
                if let Some(pending) = app.pending_approval.take() {
                    let _ = pending.response.send(true);
                }
            }
            KeyCode::Char('n') | KeyCode::Char('N') | KeyCode::Esc => {
                if let Some(pending) = app.pending_approval.take() {
                    let _ = pending.response.send(false);
                }
            }
            _ => {}
        }
        return Ok(());
    }

    // Global quit always works.
    if key.modifiers.contains(KeyModifiers::CONTROL)
        && matches!(key.code, KeyCode::Char('c') | KeyCode::Char('d'))
    {
        app.exit = true;
        return Ok(());
    }

    // Shift+Tab: toggle ask ↔ always-approve (Grok footer "mode").
    // When the / suggestion popup is open, Shift+Tab walks that list instead.
    if matches!(key.code, KeyCode::BackTab) {
        if app.show_completions && !app.completions.is_empty() {
            if app.completion_index == 0 {
                app.completion_index = app.completions.len() - 1;
            } else {
                app.completion_index -= 1;
            }
        } else {
            app.cycle_approval();
        }
        return Ok(());
    }

    // Ctrl+Enter = send now (priority if busy).
    if key.code == KeyCode::Enter && key.modifiers.contains(KeyModifiers::CONTROL) {
        return submit_input(app, /*priority*/ true).await;
    }

    // Alt+Enter = newline in the composer.
    if key.code == KeyCode::Enter && key.modifiers.contains(KeyModifiers::ALT) {
        app.input.insert(app.cursor, '\n');
        app.cursor += 1;
        app.refresh_completions();
        return Ok(());
    }

    match key.code {
        KeyCode::Esc => {
            if app.show_completions {
                app.show_completions = false;
            } else if app.busy {
                // Cancel the in-flight agent turn (checked between steps / mid-stream).
                if let Some(flag) = &app.cancel_flag {
                    flag.store(true, Ordering::Relaxed);
                }
                app.status = Some("Cancelling…".into());
                app.push_line(
                    LineKind::Notice,
                    "Cancel requested — stopping after current model/tool step…".into(),
                    false,
                );
            } else if !app.input.is_empty() {
                app.input.clear();
                app.cursor = 0;
                app.refresh_completions();
            } else if !app.queue.is_empty() {
                let n = app.queue.len();
                app.queue.clear();
                app.push_line(LineKind::Notice, format!("Cleared {n} queued message(s)."), false);
            }
        }
        KeyCode::Tab => {
            if app.input.starts_with('/') {
                app.refresh_completions();
                if app.completions.is_empty() {
                    // nothing
                } else if app.completions.len() == 1 {
                    app.completion_index = 0;
                    app.accept_completion();
                } else if !app.show_completions {
                    app.show_completions = true;
                    app.completion_index = 0;
                } else {
                    app.completion_index = (app.completion_index + 1) % app.completions.len();
                }
            }
        }
        KeyCode::Up if app.show_completions && !app.completions.is_empty() => {
            if app.completion_index == 0 {
                app.completion_index = app.completions.len() - 1;
            } else {
                app.completion_index -= 1;
            }
        }
        KeyCode::Down if app.show_completions && !app.completions.is_empty() => {
            app.completion_index = (app.completion_index + 1) % app.completions.len();
        }
        KeyCode::Enter => {
            if app.show_completions && !app.completions.is_empty() {
                app.accept_completion();
                return Ok(());
            }
            // Enter = queue if busy, otherwise send (Grok-style queue/send).
            return submit_input(app, /*priority*/ false).await;
        }
        KeyCode::Backspace => {
            if app.cursor > 0 {
                let idx = prev_char_boundary(&app.input, app.cursor);
                app.input.replace_range(idx..app.cursor, "");
                app.cursor = idx;
                app.refresh_completions();
            }
        }
        KeyCode::Left => {
            if app.cursor > 0 {
                app.cursor = prev_char_boundary(&app.input, app.cursor);
                app.refresh_completions();
            }
        }
        KeyCode::Right => {
            if app.cursor < app.input.len() {
                app.cursor = next_char_boundary(&app.input, app.cursor);
                app.refresh_completions();
            }
        }
        KeyCode::Home => {
            app.cursor = 0;
            app.refresh_completions();
        }
        KeyCode::End => {
            app.cursor = app.input.len();
            app.refresh_completions();
        }
        KeyCode::PageUp => app.scroll = app.scroll.saturating_add(5),
        KeyCode::PageDown => app.scroll = app.scroll.saturating_sub(5),
        KeyCode::Char(c)
            if !key.modifiers.contains(KeyModifiers::CONTROL)
                && !key.modifiers.contains(KeyModifiers::ALT) =>
        {
            // Allow composing the next message while the agent is still working.
            app.input.insert(app.cursor, c);
            app.cursor += c.len_utf8();
            app.refresh_completions();
        }
        _ => {}
    }
    Ok(())
}

/// Submit the composer: slash commands run immediately; prompts go to the agent or queue.
async fn submit_input(app: &mut App, priority: bool) -> Result<()> {
    let text = app.input.trim().to_string();
    if text.is_empty() {
        return Ok(());
    }
    app.input.clear();
    app.cursor = 0;
    app.show_completions = false;
    app.completions.clear();

    if text.starts_with('/') {
        handle_slash_command(app, &text)?;
        return Ok(());
    }

    if app.busy {
        app.enqueue(text, priority);
        return Ok(());
    }

    // Idle: send immediately (queue is drained first by the main loop).
    if priority && !app.queue.is_empty() {
        app.enqueue(text, true);
        if let Some(next) = app.queue.pop_front() {
            start_agent_turn(app, next).await?;
        }
    } else {
        start_agent_turn(app, text).await?;
    }
    Ok(())
}

fn handle_slash_command(app: &mut App, text: &str) -> Result<()> {
    let parts: Vec<&str> = text.split_whitespace().collect();
    let raw = parts[0];
    let cmd = commands::canonical_name(raw).unwrap_or(raw);
    match cmd {
        "/exit" => app.exit = true,
        "/help" => {
            let mut lines = vec![
                "Slash commands (Tab = gợi ý · Enter = chọn · Enter lần nữa = chạy):".to_string(),
            ];
            for command in commands::registry() {
                lines.push(format!("  {:<28} {}", command.usage, command.desc));
            }
            lines.push(
                "Keys: Shift+Tab ask/always-approve · Enter queue/send · Ctrl+Enter now · Alt+Enter newline · Tab / · Esc clear"
                    .into(),
            );
            app.push_line(LineKind::System, lines.join("\n"), false);
        }
        "/status" => {
            let meter = app.session.meter(&app.config);
            app.push_line(
                LineKind::System,
                format!(
                    "provider={} model={} approval={} steps={} context={} est~{}/{} auto_compact={}",
                    app.config.default_provider,
                    app.config.default_model,
                    app.effective_approval().label(),
                    app.steps_override.unwrap_or(app.config.max_agent_steps),
                    meter.messages,
                    meter::format_token_count(meter.estimated_tokens),
                    meter::format_token_count(meter.budget),
                    app.config.auto_compact
                ),
                false,
            );
        }
        "/key" => {
            let Some(key) = parts.get(1) else {
                app.push_line(LineKind::Warn, "Usage: /key <api-key>".into(), false);
                return Ok(());
            };
            let provider = app.config.default_provider.clone();
            app.config.set_provider_key(&provider, key)?;
            app.config.save()?;
            app.push_line(
                LineKind::Notice,
                format!("Saved key for {provider}."),
                false,
            );
        }
        "/model" => {
            if let Some(model) = parts.get(1) {
                app.config.default_model = (*model).into();
                app.config.save()?;
                app.push_line(LineKind::Notice, format!("Model: {model}"), false);
            } else {
                app.push_line(
                    LineKind::System,
                    format!("Current model: {}", app.config.default_model),
                    false,
                );
            }
        }
        "/provider" => {
            handle_tui_provider(app, &parts[1..])?;
        }
        "/providers" => {
            let mut lines = Vec::new();
            for provider in &app.config.providers {
                let mark = if provider.id == app.config.default_provider {
                    "*"
                } else {
                    " "
                };
                lines.push(format!(
                    "{mark} {:<12} {}",
                    provider.id, provider.endpoint
                ));
            }
            app.push_line(LineKind::System, lines.join("\n"), false);
        }
        "/config" => {
            match parts.get(1).copied() {
                Some("path") => app.push_line(
                    LineKind::System,
                    crate::config::config_path().display().to_string(),
                    false,
                ),
                _ => app.push_line(
                    LineKind::System,
                    format!(
                        "provider={} model={} lang={} approval={} auto_compact={} budget={}",
                        app.config.default_provider,
                        app.config.default_model,
                        app.config.lang,
                        app.config.approval_mode.label(),
                        app.config.auto_compact,
                        app.config.context_token_budget
                    ),
                    false,
                ),
            }
        }
        "/context" => {
            let meter = app.session.meter(&app.config);
            app.push_line(
                LineKind::System,
                format!(
                    "{} {} · est ~{} / {} ({}%) · compact×{}",
                    meter.bar(16),
                    meter.messages,
                    meter::format_token_count(meter.estimated_tokens),
                    meter::format_token_count(meter.budget),
                    meter.usage_pct().min(100),
                    meter.compact_count
                ),
                false,
            );
        }
        "/instructions" => match context::read_context().instructions {
            Some(inst) => app.push_line(
                LineKind::System,
                format!("From {}:\n{}", inst.source, inst.body),
                false,
            ),
            None => app.push_line(
                LineKind::Notice,
                "No AGENTS.md / G0D.md / .g0d/instructions.md found.".into(),
                false,
            ),
        },
        "/language" => {
            let Some(language) = parts.get(1) else {
                app.push_line(
                    LineKind::Warn,
                    "Usage: /language <auto|vi|en>".into(),
                    false,
                );
                return Ok(());
            };
            if !matches!(*language, "auto" | "vi" | "en") {
                app.push_line(LineKind::Warn, "Language must be auto, vi, or en".into(), false);
                return Ok(());
            }
            app.config.lang = (*language).into();
            app.config.save()?;
            app.push_line(LineKind::Notice, format!("Language: {language}"), false);
        }
        "/compact" => {
            let force = parts.get(1).is_none_or(|v| *v != "auto");
            match app.session.compact(&app.config, force) {
                Some(note) => {
                    let _ = app.session.save();
                    app.push_line(LineKind::Notice, note, false);
                }
                None => app.push_line(
                    LineKind::Notice,
                    "Context is within budget; nothing compacted.".into(),
                    false,
                ),
            }
        }
        "/new" => {
            app.session = Session::new()?;
            app.lines.clear();
            app.push_line(LineKind::System, "Started a fresh session.".into(), false);
        }
        "/session" => {
            app.push_line(
                LineKind::System,
                format!(
                    "session {} · {} messages · {}",
                    app.session.id,
                    app.session.messages.len(),
                    app.session.title.as_deref().unwrap_or("(untitled)")
                ),
                false,
            );
        }
        "/sessions" => {
            let list = Session::list()?;
            if list.is_empty() {
                app.push_line(LineKind::Notice, "No sessions for this workspace.".into(), false);
            } else {
                let mut lines = Vec::new();
                for item in list.into_iter().take(20) {
                    lines.push(format!(
                        "{} · {} msgs · {}",
                        item.id,
                        item.messages,
                        item.title.as_deref().unwrap_or("(untitled)")
                    ));
                }
                app.push_line(LineKind::System, lines.join("\n"), false);
            }
        }
        "/resume" => {
            let id = parts.get(1).copied().unwrap_or("latest");
            match Session::load(id) {
                Ok(session) => {
                    app.session = session;
                    app.lines.clear();
                    app.push_line(
                        LineKind::Notice,
                        format!(
                            "Resumed {} · {}",
                            app.session.id,
                            app.session.title.as_deref().unwrap_or("(untitled)")
                        ),
                        false,
                    );
                }
                Err(err) => app.push_line(LineKind::Warn, format!("{err:#}"), false),
            }
        }
        "/export" => {
            let path = parts
                .get(1)
                .map(std::path::PathBuf::from)
                .unwrap_or_else(|| app.session.default_export_path());
            match app.session.export_markdown(&path) {
                Ok(written) => app.push_line(
                    LineKind::Notice,
                    format!("Exported to {}", written.display()),
                    false,
                ),
                Err(err) => app.push_line(LineKind::Warn, format!("{err:#}"), false),
            }
        }
        "/approval" => {
            if let Some(value) = parts.get(1) {
                if *value == "session" {
                    if let Some(mode) = parts.get(2) {
                        if matches!(*mode, "clear" | "reset" | "default") {
                            app.session_approval = None;
                        } else {
                            app.session_approval = Some(ApprovalMode::parse(mode)?);
                        }
                    } else {
                        app.push_line(
                            LineKind::Warn,
                            "Usage: /approval session <on|off|clear>".into(),
                            false,
                        );
                        return Ok(());
                    }
                } else {
                    app.config.approval_mode = ApprovalMode::parse(value)?;
                    app.config.save()?;
                    app.session_approval = None;
                }
            }
            app.push_line(
                LineKind::System,
                format!("Approval: {}", app.effective_approval().label()),
                false,
            );
        }
        "/steps" => {
            if let Some(value) = parts.get(1) {
                if matches!(*value, "clear" | "reset" | "default") {
                    app.steps_override = None;
                } else {
                    let n: usize = value.parse().context("Usage: /steps <1-50|clear>")?;
                    if !(1..=50).contains(&n) {
                        anyhow::bail!("Steps must be 1-50");
                    }
                    app.steps_override = Some(n);
                }
            }
            app.push_line(
                LineKind::System,
                format!(
                    "Agent steps: {}",
                    app.steps_override.unwrap_or(app.config.max_agent_steps)
                ),
                false,
            );
        }
        "/clear" => {
            app.lines.clear();
        }
        "/chat" => {
            app.mode = TuiMode::Chat;
            app.push_line(LineKind::Notice, "Agent mode → chat".into(), false);
        }
        "/godmode" => {
            app.mode = TuiMode::Godmode;
            app.push_line(LineKind::Notice, "Agent mode → godmode".into(), false);
        }
        "/snake" => {
            app.mode = TuiMode::Snake;
            app.push_line(LineKind::Notice, "Agent mode → snake".into(), false);
        }
        "/ultra" => {
            app.mode = TuiMode::Ultra;
            app.push_line(LineKind::Notice, "Agent mode → ultra".into(), false);
        }
        "/history" => {
            app.push_line(
                LineKind::Notice,
                "Input history lives in the classic REPL (`g0d --classic`).".into(),
                false,
            );
        }
        other => {
            let near = commands::suggestions(other, 3);
            if near.is_empty() {
                app.push_line(
                    LineKind::Warn,
                    format!("Unknown command: {other}. Type /help or Tab for suggestions."),
                    false,
                );
            } else {
                app.push_line(
                    LineKind::Warn,
                    format!("Unknown command: {other}. Did you mean {}?", near.join(", ")),
                    false,
                );
            }
        }
    }
    Ok(())
}

fn handle_tui_provider(app: &mut App, args: &[&str]) -> Result<()> {
    match args.first().copied() {
        None | Some("list") => {
            let mut lines = Vec::new();
            for provider in &app.config.providers {
                let mark = if provider.id == app.config.default_provider {
                    "*"
                } else {
                    " "
                };
                lines.push(format!(
                    "{mark} {:<12} {}",
                    provider.id, provider.endpoint
                ));
            }
            app.push_line(LineKind::System, lines.join("\n"), false);
        }
        Some("default") | Some("use") => {
            let id = args.get(1).context("Usage: /provider use <id>")?;
            app.config.set_default_provider(id)?;
            app.config.save()?;
            app.push_line(
                LineKind::Notice,
                format!("Provider: {id} · model {}", app.config.default_model),
                false,
            );
        }
        Some("key") => {
            let id = args.get(1).context("Usage: /provider key <id> <key>")?;
            let key = args.get(2).context("Usage: /provider key <id> <key>")?;
            app.config.set_provider_key(id, key)?;
            app.config.save()?;
            app.push_line(LineKind::Notice, format!("Saved key for {id}."), false);
        }
        Some("add") => {
            let id = args
                .get(1)
                .context("Usage: /provider add <id> <endpoint> [key-env]")?;
            let endpoint = args
                .get(2)
                .context("Usage: /provider add <id> <endpoint> [key-env]")?;
            app.config
                .add_provider(id, endpoint, args.get(3).copied())?;
            app.config.save()?;
            app.push_line(
                LineKind::Notice,
                format!("Added provider {id} → {endpoint}"),
                false,
            );
        }
        Some("setup") => {
            let id = args
                .get(1)
                .context("Usage: /provider setup <id> <endpoint> <api-key> [model]")?;
            let endpoint = args
                .get(2)
                .context("Usage: /provider setup <id> <endpoint> <api-key> [model]")?;
            let key = args
                .get(3)
                .context("Usage: /provider setup <id> <endpoint> <api-key> [model]")?;
            let model = args.get(4).copied();
            app.config
                .setup_provider(id, endpoint, Some(key), model, None)?;
            app.config.save()?;
            app.push_line(
                LineKind::Notice,
                format!(
                    "Ready: {id} · {endpoint} · model {}",
                    app.config.default_model
                ),
                false,
            );
        }
        Some("endpoint") => {
            let id = args
                .get(1)
                .context("Usage: /provider endpoint <id> <url>")?;
            let endpoint = args
                .get(2)
                .context("Usage: /provider endpoint <id> <url>")?;
            app.config.set_provider_endpoint(id, endpoint)?;
            app.config.save()?;
            app.push_line(
                LineKind::Notice,
                format!("Endpoint {id} → {endpoint}"),
                false,
            );
        }
        Some("auth") => {
            let id = args.get(1).context("Usage: /provider auth <id> <style>")?;
            let style = args.get(2).context("Usage: /provider auth <id> <style>")?;
            app.config.set_provider_auth_style(id, style)?;
            app.config.save()?;
            app.push_line(LineKind::Notice, format!("Auth {id} → {style}"), false);
        }
        Some("header") => {
            let id = args
                .get(1)
                .context("Usage: /provider header <id> <Name> <Value>")?;
            let name = args
                .get(2)
                .context("Usage: /provider header <id> <Name> <Value>")?;
            let value = args
                .get(3)
                .context("Usage: /provider header <id> <Name> <Value>")?;
            app.config.set_provider_header(id, name, value)?;
            app.config.save()?;
            app.push_line(LineKind::Notice, format!("Header {id}: {name}"), false);
        }
        Some("test") | Some("models") => {
            // Run async probe on a worker; report via chat lines.
            let action = args[0].to_string();
            let switch_id = args.get(1).map(|s| (*s).to_string());
            let mut cfg = app.config.clone();
            if let Some(id) = switch_id.as_deref() {
                cfg.set_default_provider(id)?;
            }
            let handle = app.runtime.clone();
            let result = std::thread::spawn(move || {
                handle.block_on(async move {
                    let key = cfg.get_api_key()?;
                    if action == "test" {
                        crate::api::test_provider(&cfg, &key).await
                    } else {
                        let models = crate::api::list_models(&cfg, &key).await?;
                        Ok(format!(
                            "{} models · {}",
                            models.len(),
                            models.iter().take(20).cloned().collect::<Vec<_>>().join(", ")
                        ))
                    }
                })
            })
            .join()
            .map_err(|_| anyhow::anyhow!("provider probe thread failed"))?;
            match result {
                Ok(msg) => {
                    if let Some(id) = switch_id.as_deref() {
                        app.config.set_default_provider(id)?;
                        app.config.save()?;
                    }
                    app.push_line(LineKind::Notice, msg, false);
                }
                Err(err) => app.push_line(LineKind::Warn, format!("{err:#}"), false),
            }
        }
        Some("remove") => {
            let id = args.get(1).context("Usage: /provider remove <id>")?;
            app.config.remove_provider(id)?;
            app.config.save()?;
            app.push_line(LineKind::Notice, format!("Removed provider {id}."), false);
        }
        Some(action) => app.push_line(
            LineKind::Warn,
            format!(
                "Unknown action: {action}. Try list|use|setup|add|key|endpoint|auth|header|test|models|remove"
            ),
            false,
        ),
    }
    Ok(())
}

async fn start_agent_turn(app: &mut App, query: String) -> Result<()> {
    let mode_tag = app.mode.label();
    app.push_line(
        LineKind::User,
        if app.mode == TuiMode::Chat {
            query.clone()
        } else {
            format!("[{mode_tag}] {query}")
        },
        true,
    );
    app.busy = true;
    app.thinking_since = Some(Instant::now());
    app.turn_started = Some(Instant::now());
    app.status = Some(format!("Thinking · {mode_tag}"));
    app.last_error = None;
    app.streaming_assistant = false;
    let cancel = Arc::new(AtomicBool::new(false));
    app.cancel_flag = Some(Arc::clone(&cancel));

    let (tx, rx) = mpsc::channel::<AgentEvent>();
    app.agent_rx = Some(rx);

    let config = app.config.clone();
    let key = match app.config.get_api_key() {
        Ok(k) => k,
        Err(err) => {
            app.busy = false;
            app.status = None;
            app.thinking_since = None;
            app.agent_rx = None;
            app.cancel_flag = None;
            app.push_line(LineKind::Warn, format!("Error: {err:#}"), false);
            return Ok(());
        }
    };
    let term = app.term_state;
    let mut session = app.session.clone();
    let run_opts = RunOptions {
        max_steps: app.steps_override,
        approval: app.session_approval,
        cancel: Some(cancel),
    };
    let handle = app.runtime.clone();
    let mode = app.mode;

    // Non-chat modes currently still use the coding agent loop with a mode hint in the query.
    // Full GODMODE/Ultra multi-race can be expanded later; UX mode switch is live now.
    let query_for_agent = match mode {
        TuiMode::Chat => query,
        TuiMode::Godmode => format!(
            "[MODE:godmode — prefer multi-perspective, bold alternatives]\n{query}"
        ),
        TuiMode::Snake => format!(
            "[MODE:parseltongue — precise, structured rewrite of the request first]\n{query}"
        ),
        TuiMode::Ultra => format!(
            "[MODE:ultra — thorough, high-effort analysis]\n{query}"
        ),
    };

    // Run agent on a dedicated thread so the TUI keeps painting / handling y/n / queueing.
    std::thread::spawn(move || {
        let mut sink = ChannelSink { tx };
        let result = handle.block_on(async {
            agent::run(
                &config,
                &key,
                &query_for_agent,
                &term,
                &mut session,
                run_opts,
                &mut sink,
            )
            .await
        });
        let _ = session.save();
        if let Err(err) = result {
            sink.emit(AgentEvent::Warn(format!("Error: {err:#}")));
        }
        // Dropping sink/tx signals completion to the TUI.
    });

    Ok(())
}

fn prev_char_boundary(s: &str, idx: usize) -> usize {
    if idx == 0 {
        return 0;
    }
    let mut i = idx - 1;
    while i > 0 && !s.is_char_boundary(i) {
        i -= 1;
    }
    i
}

fn next_char_boundary(s: &str, idx: usize) -> usize {
    if idx >= s.len() {
        return s.len();
    }
    let mut i = idx + 1;
    while i < s.len() && !s.is_char_boundary(i) {
        i += 1;
    }
    i
}

fn draw(frame: &mut Frame, app: &App) {
    let area = frame.area();
    let input_lines = app.input.matches('\n').count() as u16 + 1;
    let input_height = (input_lines + 2).clamp(3, 8);
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(1),
            Constraint::Min(5),
            Constraint::Length(input_height),
            Constraint::Length(1),
        ])
        .split(area);

    draw_header(frame, chunks[0], app);
    draw_chat(frame, chunks[1], app);
    draw_input(frame, chunks[2], app);
    draw_footer(frame, chunks[3], app);

    if app.show_completions && !app.completions.is_empty() {
        draw_completion_popup(frame, chunks[2], app);
    }
    if let Some(pending) = &app.pending_approval {
        draw_approval_modal(frame, area, pending);
    }
}

fn draw_completion_popup(frame: &mut Frame, input_area: Rect, app: &App) {
    // Grok-style: wide floating list just above the input, command left / description right.
    let max_rows = 12usize;
    let total = app.completions.len();
    let visible = total.min(max_rows) as u16;
    let height = visible.saturating_add(2).max(3);
    let width = input_area.width.max(40);
    let x = input_area.x;
    let y = input_area.y.saturating_sub(height);
    let rect = Rect::new(x, y, width, height);
    frame.render_widget(Clear, rect);

    // Keep the selected row in the visible window.
    let selected = app.completion_index.min(total.saturating_sub(1));
    let start = if selected >= max_rows {
        selected + 1 - max_rows
    } else {
        0
    };
    let end = (start + max_rows).min(total);

    let name_width = app.completions[start..end]
        .iter()
        .map(|item| item.value.chars().count())
        .max()
        .unwrap_or(12)
        .clamp(10, 22);

    let mut rows: Vec<Line> = Vec::new();
    for (offset, item) in app.completions[start..end].iter().enumerate() {
        let index = start + offset;
        let is_selected = index == selected;
        let row_style = if is_selected {
            Style::default().fg(Color::White).bg(Color::DarkGray)
        } else {
            Style::default().fg(Color::Gray).bg(Color::Rgb(30, 30, 30))
        };
        let name = format!(" {:<width$}", item.value, width = name_width);
        let mut desc = item.description.clone();
        let desc_budget = width.saturating_sub(name_width as u16 + 4) as usize;
        if desc.chars().count() > desc_budget {
            desc = desc.chars().take(desc_budget.saturating_sub(1)).collect();
            desc.push('…');
        }
        rows.push(Line::from(vec![
            Span::styled(name, row_style.add_modifier(Modifier::BOLD)),
            Span::styled(format!(" {desc}"), row_style),
        ]));
    }

    let block = Block::default()
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(Style::default().fg(Color::Rgb(70, 70, 70)))
        .style(Style::default().bg(Color::Rgb(30, 30, 30)))
        .title(Span::styled(
            format!(" / commands · {} · ↑↓ Tab · Enter ", total),
            Style::default().fg(Color::DarkGray),
        ));
    frame.render_widget(
        Paragraph::new(rows)
            .block(block)
            .style(Style::default().bg(Color::Rgb(30, 30, 30))),
        rect,
    );
}

fn draw_header(frame: &mut Frame, area: Rect, app: &App) {
    let project = context::read_context();
    let branch = project
        .git_branch
        .as_deref()
        .unwrap_or("main");
    let path = project.cwd;
    // Truncate path if needed.
    let path_short = if path.len() > 48 {
        format!("…{}", &path[path.len().saturating_sub(47)..])
    } else {
        path
    };
    let left = format!(" ≡ {branch} {path_short}");
    let right = app.meter_label();
    let spacer = area
        .width
        .saturating_sub((left.chars().count() + right.chars().count()) as u16)
        .max(1);
    let line = Line::from(vec![
        Span::styled(left, Style::default().fg(Color::Gray)),
        Span::raw(" ".repeat(spacer as usize)),
        Span::styled(right, Style::default().fg(Color::DarkGray)),
    ]);
    frame.render_widget(Paragraph::new(line), area);
}

fn draw_chat(frame: &mut Frame, area: Rect, app: &App) {
    let mut rows: Vec<Line> = Vec::new();
    for line in &app.lines {
        match line.kind {
            LineKind::User => {
                let mut spans = vec![
                    Span::styled("› ", Style::default().fg(Color::White)),
                    Span::styled(line.text.clone(), Style::default().fg(Color::White)),
                ];
                if let Some(t) = &line.time {
                    spans.push(Span::raw("  "));
                    spans.push(Span::styled(t.clone(), Style::default().fg(Color::DarkGray)));
                }
                rows.push(Line::from(spans));
                rows.push(Line::from(""));
            }
            LineKind::Assistant => {
                rows.push(Line::from(Span::styled(
                    line.text.clone(),
                    Style::default().fg(Color::Gray),
                )));
                rows.push(Line::from(""));
            }
            LineKind::Tool => {
                rows.push(Line::from(vec![
                    Span::styled("◆ ", Style::default().fg(Color::DarkGray)),
                    Span::styled(line.text.clone(), Style::default().fg(Color::DarkGray)),
                ]));
            }
            LineKind::ToolResult => {
                rows.push(Line::from(Span::styled(
                    format!("  {}", line.text),
                    Style::default().fg(Color::DarkGray),
                )));
            }
            LineKind::Notice => {
                rows.push(Line::from(Span::styled(
                    line.text.clone(),
                    Style::default().fg(Color::DarkGray).add_modifier(Modifier::ITALIC),
                )));
            }
            LineKind::Warn => {
                rows.push(Line::from(Span::styled(
                    line.text.clone(),
                    Style::default().fg(Color::Yellow),
                )));
            }
            LineKind::Usage => {
                rows.push(Line::from(Span::styled(
                    line.text.clone(),
                    Style::default().fg(Color::DarkGray),
                )));
            }
            LineKind::System => {
                rows.push(Line::from(Span::styled(
                    line.text.clone(),
                    Style::default().fg(Color::DarkGray),
                )));
                rows.push(Line::from(""));
            }
            LineKind::Queued => {
                rows.push(Line::from(vec![
                    Span::styled("⏳ ", Style::default().fg(Color::Yellow)),
                    Span::styled(
                        line.text.clone(),
                        Style::default()
                            .fg(Color::Yellow)
                            .add_modifier(Modifier::ITALIC),
                    ),
                ]));
            }
        }
    }

    if let Some(status) = &app.status {
        let elapsed = app
            .thinking_since
            .map(|t| t.elapsed().as_secs())
            .unwrap_or(0);
        rows.push(Line::from(""));
        rows.push(Line::from(vec![
            Span::styled("- ", Style::default().fg(Color::DarkGray)),
            Span::styled(
                format!("Thinking… {elapsed}s"),
                Style::default().fg(Color::DarkGray),
            ),
            Span::raw("  "),
            Span::styled(
                format!("{status}"),
                Style::default().fg(Color::DarkGray),
            ),
        ]));
    }

    // Scroll: show last N lines with offset.
    let height = area.height as usize;
    let total = rows.len();
    let max_scroll = total.saturating_sub(height);
    let scroll = (app.scroll as usize).min(max_scroll);
    let start = total.saturating_sub(height + scroll);
    let end = total.saturating_sub(scroll);
    let visible: Vec<Line> = rows
        .into_iter()
        .skip(start)
        .take(end.saturating_sub(start))
        .collect();

    let widget = Paragraph::new(visible).wrap(Wrap { trim: false });
    frame.render_widget(widget, area);
}

fn draw_input(frame: &mut Frame, area: Rect, app: &App) {
    let approval = match app.effective_approval() {
        ApprovalMode::Off => "always-approve",
        ApprovalMode::On => "ask",
    };
    let model = short_model(&app.config.default_model);
    let q = if app.queue.is_empty() {
        String::new()
    } else {
        format!(" · queue {}", app.queue.len())
    };
    let busy = if app.busy { " · running" } else { "" };
    let title_right = format!(
        " {} · {} · {approval}{q}{busy} ",
        app.mode.label(),
        model
    );
    let block = Block::default()
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(Style::default().fg(if app.busy {
            Color::Yellow
        } else {
            Color::DarkGray
        }))
        .title(Span::styled(
            title_right,
            Style::default().fg(Color::DarkGray),
        ))
        .title_alignment(ratatui::layout::Alignment::Right);

    let prompt = format!("› {}", app.input);
    let paragraph = Paragraph::new(prompt)
        .style(Style::default().fg(Color::White))
        .block(block)
        .wrap(Wrap { trim: false });
    frame.render_widget(paragraph, area);

    // Cursor: account for newlines in the multi-line composer.
    let before = &app.input[..app.cursor.min(app.input.len())];
    let line_idx = before.matches('\n').count() as u16;
    let col = before
        .rsplit('\n')
        .next()
        .map(|s| s.chars().count() as u16)
        .unwrap_or(0);
    let cursor_x = area.x + 1 + 2 + col;
    let cursor_y = area.y + 1 + line_idx;
    if app.pending_approval.is_none() {
        frame.set_cursor_position((
            cursor_x.min(area.x + area.width.saturating_sub(1)),
            cursor_y.min(area.y + area.height.saturating_sub(2)),
        ));
    }
}

fn draw_footer(frame: &mut Frame, area: Rect, app: &App) {
    let enter = if app.busy { ":queue" } else { ":send" };
    let approval = match app.effective_approval() {
        ApprovalMode::On => "ask",
        ApprovalMode::Off => "always-approve",
    };
    let q = if app.queue.is_empty() {
        String::new()
    } else {
        format!(" q:{}", app.queue.len())
    };
    let esc = if app.busy { "Esc:cancel" } else { "Esc:clear" };
    let text = format!(
        "Enter{enter}  Ctrl+Enter:now  Alt+Enter:newline  Shift+Tab:{approval}  Tab:/  {esc}{q}"
    );
    frame.render_widget(
        Paragraph::new(Span::styled(text, Style::default().fg(Color::DarkGray))),
        area,
    );
}

fn draw_approval_modal(frame: &mut Frame, area: Rect, pending: &PendingApproval) {
    let width = area.width.min(72).max(30);
    let height = 7u16;
    let x = area.x + (area.width.saturating_sub(width)) / 2;
    let y = area.y + (area.height.saturating_sub(height)) / 2;
    let rect = Rect::new(x, y, width, height);
    frame.render_widget(Clear, rect);
    let block = Block::default()
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(Style::default().fg(Color::Yellow))
        .title(" Approve ");
    let text = format!(
        "{}\n\n[y] approve   [n] deny   [Esc] deny",
        pending.description
    );
    let paragraph = Paragraph::new(text)
        .wrap(Wrap { trim: true })
        .style(Style::default().fg(Color::White))
        .block(block);
    frame.render_widget(paragraph, rect);
}

fn short_model(model: &str) -> &str {
    model
        .rsplit('/')
        .next()
        .filter(|s| !s.is_empty())
        .unwrap_or(model)
}


