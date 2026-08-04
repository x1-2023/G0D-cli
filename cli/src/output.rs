//! Agent UI events — console or Grok-style TUI can render these.

use crate::terminal::TerminalState;
use anyhow::Result;
use std::io::{IsTerminal, Write};
use std::sync::mpsc;

#[derive(Debug)]
pub enum AgentEvent {
    /// Transient status (Thinking / step n/m).
    Status(String),
    /// Clear transient status line.
    ClearStatus,
    /// User-visible notice (compact, checkpoint hint).
    Notice(String),
    /// Warning / yellow notice.
    Warn(String),
    /// Tool invocation.
    Tool { name: String, preview: String },
    /// One-line tool observation.
    ToolResult(String),
    /// Final or intermediate assistant text (complete block).
    Assistant(String),
    /// Streaming token/chunk of assistant text (no trailing newline).
    AssistantDelta(String),
    /// Token / context footer line.
    Usage(String),
    /// Agent aborted because the user cancelled.
    Cancelled,
    /// Request interactive approval; respond on the channel.
    NeedApproval {
        description: String,
        response: mpsc::Sender<bool>,
    },
}

pub trait EventSink: Send {
    fn emit(&mut self, event: AgentEvent);
}

/// Classic terminal adapter (println + spinner-compatible).
pub struct ConsoleSink<'a> {
    pub term: &'a TerminalState,
}

impl EventSink for ConsoleSink<'_> {
    fn emit(&mut self, event: AgentEvent) {
        match event {
            AgentEvent::Status(text) => {
                // StatusIndicator in agent still handles animation for console;
                // this is a fallback one-shot line.
                eprintln!("{}...", self.term.dim(&text));
            }
            AgentEvent::ClearStatus => {}
            AgentEvent::Notice(text) => println!("{}", self.term.dim(&text)),
            AgentEvent::Warn(text) => println!("{}", self.term.yellow(&text)),
            AgentEvent::Tool { name, preview } => {
                println!("{}", self.term.dim(&format!("→ {name} {preview}")));
            }
            AgentEvent::ToolResult(summary) => {
                println!("{}", self.term.dim(&format!("  {summary}")));
            }
            AgentEvent::Assistant(text) => println!("{text}"),
            AgentEvent::AssistantDelta(text) => {
                print!("{text}");
                let _ = std::io::stdout().flush();
            }
            AgentEvent::Usage(text) => println!("{}", self.term.dim(&text)),
            AgentEvent::Cancelled => {
                println!("{}", self.term.yellow("Cancelled."));
            }
            AgentEvent::NeedApproval {
                description,
                response,
            } => {
                let ok = console_approve(&description, self.term).unwrap_or(false);
                let _ = response.send(ok);
            }
        }
    }
}

pub fn console_approve(description: &str, term: &TerminalState) -> Result<bool> {
    if !term.is_tty || !std::io::stdin().is_terminal() {
        return Ok(false);
    }
    print!(
        "{} [y/N]: ",
        term.yellow(&format!("Approve: {description}"))
    );
    std::io::stdout().flush()?;
    let mut answer = String::new();
    std::io::stdin().read_line(&mut answer)?;
    Ok(matches!(
        answer.trim().to_ascii_lowercase().as_str(),
        "y" | "yes" | "c" | "co" | "có"
    ))
}

/// Sink that forwards events to the TUI thread.
pub struct ChannelSink {
    pub tx: mpsc::Sender<AgentEvent>,
}

impl EventSink for ChannelSink {
    fn emit(&mut self, event: AgentEvent) {
        let _ = self.tx.send(event);
    }
}
