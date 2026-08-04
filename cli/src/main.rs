mod agent;
mod api;
mod commands;
mod config;
mod context;
mod meter;
mod output;
mod session;
mod status;
mod terminal;
mod tui;

use anyhow::{Context, Result};
use futures::StreamExt;
use nu_ansi_term::{Color, Style};
use reedline::{
    default_emacs_keybindings, ColumnarMenu, DefaultHinter, DefaultPrompt, DefaultPromptSegment,
    EditCommand, Emacs, FileBackedHistory, KeyCode, KeyModifiers, ListMenu, MenuBuilder, Reedline,
    ReedlineEvent, ReedlineMenu, Signal,
};
use serde_json::{json, Value};
use std::io::{Read, Write};

const VERSION: &str = env!("CARGO_PKG_VERSION");
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum RunMode {
    Chat,
    Godmode,
    Parseltongue,
    Ultra,
}

impl RunMode {
    fn label(self) -> &'static str {
        match self {
            Self::Chat => "chat",
            Self::Godmode => "godmode",
            Self::Parseltongue => "snake",
            Self::Ultra => "ultra",
        }
    }
}

#[derive(Debug, PartialEq, Eq)]
enum CliAction {
    Interactive,
    Help,
    Version,
    ConfigPath,
    Models,
    SaveKey(String),
    SetLanguage(String),
    Query {
        mode: RunMode,
        query: String,
        tier: String,
    },
}

#[derive(Debug, PartialEq, Eq)]
struct CliOptions {
    action: CliAction,
    headless: bool,
    no_color: bool,
    /// Use the classic reedline REPL instead of the Grok-style TUI.
    classic: bool,
    provider: Option<String>,
    model: Option<String>,
    /// Process-only base URL override for third-party gateways (not persisted).
    endpoint: Option<String>,
    approval: Option<config::ApprovalMode>,
    resume: Option<String>,
    /// Per-process agent step budget override (not persisted).
    steps: Option<usize>,
}

#[tokio::main]
async fn main() -> Result<()> {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let options = parse_cli_args(&args)?;
    let term = terminal::TerminalState::detect(options.headless, options.no_color);

    match &options.action {
        CliAction::Help => return print_cli_help(&term),
        CliAction::Version => {
            println!("g0d {VERSION}");
            return Ok(());
        }
        CliAction::ConfigPath => {
            println!("{}", config::config_path().display());
            return Ok(());
        }
        CliAction::Models => {
            list_models(&term);
            return Ok(());
        }
        _ => {}
    }

    let mut config = config::Config::load()?;
    let persist_selection =
        options.provider.is_some() || options.model.is_some() || options.approval.is_some();
    if let Some(provider) = options.provider.as_deref() {
        config.set_default_provider(provider)?;
    }
    if let Some(model) = options.model {
        if model.trim().is_empty() {
            anyhow::bail!("Model cannot be empty");
        }
        config.default_model = model;
    }
    if let Some(approval) = options.approval {
        config.approval_mode = approval;
    }
    if let Some(endpoint) = options.endpoint.clone() {
        config.endpoint_override = Some(endpoint);
    }
    if persist_selection {
        config.save()?;
    }

    match options.action {
        CliAction::SaveKey(key) => {
            let provider = config.default_provider.clone();
            config.set_provider_key(&provider, &key)?;
            config.save()?;
            println!(
                "Saved key for {provider}. Prefer an environment variable on shared machines."
            );
        }
        CliAction::SetLanguage(language) => {
            validate_language(&language)?;
            config.lang = language;
            config.save()?;
            println!("Language: {}", config.lang);
        }
        CliAction::Query { mode, query, tier } => {
            let mut session = if let Some(id) = options.resume.as_deref() {
                session::Session::load(id)?
            } else {
                session::Session::new()?
            };
            let run_opts = agent::RunOptions { max_steps: options.steps, approval: options.approval, cancel: None, };
            run_query(&config, mode, &query, &tier, &term, &mut session, run_opts).await?;
        }
        CliAction::Interactive if term.is_tty => {
            let session = if let Some(id) = options.resume.as_deref() {
                session::Session::load(id)?
            } else {
                session::Session::new()?
            };
            let use_classic = options.classic
                || std::env::var_os("G0D_CLASSIC").is_some()
                || std::env::var_os("G0D_REPL").is_some();
            if use_classic {
                run_repl(
                    &mut config,
                    &term,
                    Some(session),
                    options.steps,
                    options.approval,
                )
                .await?
            } else {
                tui::run(
                    config,
                    session,
                    term,
                    tui::TuiOptions {
                        steps: options.steps,
                        approval: options.approval,
                    },
                )
                .await?
            }
        }
        CliAction::Interactive => {
            let mut query = String::new();
            std::io::stdin()
                .read_to_string(&mut query)
                .context("Could not read stdin")?;
            if query.trim().is_empty() {
                print_cli_help(&term)?;
            } else {
                let mut session = if let Some(id) = options.resume.as_deref() {
                    session::Session::load(id)?
                } else {
                    session::Session::new()?
                };
                let run_opts = agent::RunOptions { max_steps: options.steps, approval: options.approval, cancel: None, };
                run_query(
                    &config,
                    RunMode::Chat,
                    query.trim(),
                    "fast",
                    &term,
                    &mut session,
                    run_opts,
                )
                .await?;
            }
        }
        CliAction::Help | CliAction::Version | CliAction::ConfigPath | CliAction::Models => {
            unreachable!()
        }
    }
    Ok(())
}

fn parse_cli_args(args: &[String]) -> Result<CliOptions> {
    let mut headless = false;
    let mut no_color = false;
    let mut classic = false;
    let mut provider = None;
    let mut model = None;
    let mut endpoint = None;
    let mut approval = None;
    let mut resume = None;
    let mut steps = None;
    let mut mode = RunMode::Chat;
    let mut tier = "fast".to_string();
    let mut query = Vec::new();
    let mut index = 0usize;

    while index < args.len() {
        let argument = &args[index];
        match argument.as_str() {
            "-h" | "--help" => {
                return Ok(options(
                    CliAction::Help,
                    headless,
                    no_color,
                    classic,
                    provider,
                    model,
                    endpoint,
                    approval,
                    resume,
                    steps,
                ))
            }
            "-V" | "--version" => {
                return Ok(options(
                    CliAction::Version,
                    headless,
                    no_color,
                    classic,
                    provider,
                    model,
                    endpoint,
                    approval,
                    resume,
                    steps,
                ))
            }
            "--config" => {
                return Ok(options(
                    CliAction::ConfigPath,
                    headless,
                    no_color,
                    classic,
                    provider,
                    model,
                    endpoint,
                    approval,
                    resume,
                    steps,
                ))
            }
            "--models" => {
                return Ok(options(
                    CliAction::Models,
                    headless,
                    no_color,
                    classic,
                    provider,
                    model,
                    endpoint,
                    approval,
                    resume,
                    steps,
                ))
            }
            "--headless" => headless = true,
            "--no-color" => no_color = true,
            "--classic" => classic = true,
            "--resume" => resume = Some("latest".into()),
            "--approval" => {
                index += 1;
                approval = Some(config::ApprovalMode::parse(required_value(
                    args,
                    index,
                    "--approval",
                )?)?);
            }
            "--steps" => {
                index += 1;
                let value = required_value(args, index, "--steps")?;
                let parsed: usize = value
                    .parse()
                    .context("--steps must be an integer between 1 and 50")?;
                if !(1..=50).contains(&parsed) {
                    anyhow::bail!("--steps must be between 1 and 50");
                }
                steps = Some(parsed);
            }
            "-g" | "--godmode" => mode = RunMode::Godmode,
            "-p" | "--snake" => mode = RunMode::Parseltongue,
            "-u" | "--ultra" => mode = RunMode::Ultra,
            "--provider" => {
                index += 1;
                provider = Some(required_value(args, index, "--provider")?.to_string());
            }
            "--model" => {
                index += 1;
                model = Some(required_value(args, index, "--model")?.to_string());
            }
            "--endpoint" => {
                index += 1;
                endpoint = Some(required_value(args, index, "--endpoint")?.to_string());
            }
            "-k" | "--key" => {
                index += 1;
                let key = required_value(args, index, argument)?.to_string();
                return Ok(options(
                    CliAction::SaveKey(key),
                    headless,
                    no_color,
                    classic,
                    provider,
                    model,
                    endpoint,
                    approval,
                    resume,
                    steps,
                ));
            }
            "--language" => {
                index += 1;
                let language = required_value(args, index, "--language")?.to_string();
                return Ok(options(
                    CliAction::SetLanguage(language),
                    headless,
                    no_color,
                    classic,
                    provider,
                    model,
                    endpoint,
                    approval,
                    resume,
                    steps,
                ));
            }
            "--tier" => {
                index += 1;
                tier = required_value(args, index, "--tier")?.to_string();
            }
            "--" => {
                query.extend(args[index + 1..].iter().cloned());
                break;
            }
            value if value.starts_with("--tier=") => tier = value[7..].to_string(),
            value if value.starts_with("--resume=") => resume = Some(value[9..].to_string()),
            value if value.starts_with("--approval=") => {
                approval = Some(config::ApprovalMode::parse(&value[11..])?)
            }
            value if value.starts_with("--steps=") => {
                let parsed: usize = value[8..]
                    .parse()
                    .context("--steps must be an integer between 1 and 50")?;
                if !(1..=50).contains(&parsed) {
                    anyhow::bail!("--steps must be between 1 and 50");
                }
                steps = Some(parsed);
            }
            value if value.starts_with('-') => {
                anyhow::bail!("Unknown option: {value}. Run g0d --help.")
            }
            value => query.push(value.to_string()),
        }
        index += 1;
    }

    let action = if query.is_empty() {
        if mode == RunMode::Chat {
            CliAction::Interactive
        } else {
            anyhow::bail!("A query is required for {} mode", mode.label())
        }
    } else {
        CliAction::Query {
            mode,
            query: query.join(" "),
            tier,
        }
    };
    Ok(options(
        action, headless, no_color, classic, provider, model, endpoint, approval, resume, steps,
    ))
}

fn options(
    action: CliAction,
    headless: bool,
    no_color: bool,
    classic: bool,
    provider: Option<String>,
    model: Option<String>,
    endpoint: Option<String>,
    approval: Option<config::ApprovalMode>,
    resume: Option<String>,
    steps: Option<usize>,
) -> CliOptions {
    CliOptions {
        action,
        headless,
        no_color,
        classic,
        provider,
        model,
        endpoint,
        approval,
        resume,
        steps,
    }
}

fn required_value<'a>(args: &'a [String], index: usize, option: &str) -> Result<&'a str> {
    args.get(index)
        .map(String::as_str)
        .with_context(|| format!("Missing value for {option}"))
}

async fn run_repl(
    config: &mut config::Config,
    term: &terminal::TerminalState,
    existing_session: Option<session::Session>,
    steps_override: Option<usize>,
    approval_override: Option<config::ApprovalMode>,
) -> Result<()> {
    print_banner(config, term);
    let history_path = config::history_path();
    if let Some(parent) = history_path.parent() {
        std::fs::create_dir_all(parent)
            .with_context(|| format!("Could not create history directory: {}", parent.display()))?;
    }

    let completion_menu = ColumnarMenu::default()
        .with_name("completion_menu")
        .with_columns(1)
        .with_column_padding(2);
    let history_menu = ListMenu::default().with_name("history_menu");
    let mut keybindings = default_emacs_keybindings();
    keybindings.add_binding(
        KeyModifiers::NONE,
        KeyCode::Tab,
        ReedlineEvent::UntilFound(vec![
            ReedlineEvent::Menu("completion_menu".into()),
            ReedlineEvent::Edit(vec![EditCommand::Complete]),
        ]),
    );
    keybindings.add_binding(
        KeyModifiers::SHIFT,
        KeyCode::BackTab,
        ReedlineEvent::MenuPrevious,
    );
    keybindings.add_binding(
        KeyModifiers::CONTROL,
        KeyCode::Char('r'),
        ReedlineEvent::Menu("history_menu".into()),
    );
    keybindings.add_binding(
        KeyModifiers::ALT,
        KeyCode::Enter,
        ReedlineEvent::Edit(vec![EditCommand::InsertNewline]),
    );

    let history = FileBackedHistory::with_file(1000, history_path).unwrap_or_default();
    let hinter_style = if term.colors {
        Style::new().italic().fg(Color::DarkGray)
    } else {
        Style::new()
    };
    let mut editor = Reedline::create()
        .with_completer(Box::new(commands::SlashCompleter::new()))
        .with_hinter(Box::new(DefaultHinter::default().with_style(hinter_style)))
        .with_history(Box::new(history))
        .with_history_exclusion_prefix(Some("/".into()))
        .with_menu(ReedlineMenu::EngineCompleter(Box::new(completion_menu)))
        .with_menu(ReedlineMenu::HistoryMenu(Box::new(history_menu)))
        .with_edit_mode(Box::new(Emacs::new(keybindings)))
        .with_quick_completions(true)
        .with_partial_completions(true);

    let mut mode = RunMode::Chat;
    let mut ultra_tier = "fast".to_string();
    let mut steps_override = steps_override;
    let mut session_approval = approval_override;
    let mut session = match existing_session {
        Some(session) => session,
        None => session::Session::new()?,
    };
    println!("{}", term.dim(&format!("session {}", session.id)));
    if let Some(instructions) = &context::read_context().instructions {
        println!(
            "{}",
            term.dim(&format!("project instructions · {}", instructions.source))
        );
    }

    loop {
        let context = context::read_context();
        let branch = context
            .git_branch
            .map(|branch| format!(" [{branch}]"))
            .unwrap_or_default();
        let meter = session.meter(config);
        let left = term.dim(&format!(
            "{}{} {} › ",
            current_dir_name(),
            branch,
            mode.label()
        ));
        let right = term.dim(&format!(
            "{} · {} · {}",
            config.default_provider,
            compact_model(&config.default_model),
            meter.short_label()
        ));
        let prompt = DefaultPrompt::new(
            DefaultPromptSegment::Basic(left),
            DefaultPromptSegment::Basic(right),
        );

        match editor.read_line(&prompt) {
            Ok(Signal::Success(line)) => {
                let input = line.trim();
                if input.is_empty() {
                    continue;
                }

                if input.starts_with('/') {
                    let parts: Vec<&str> = input.split_whitespace().collect();
                    let command = commands::canonical_name(parts[0]);
                    match command {
                        Some("/exit") => {
                            println!("bye");
                            break;
                        }
                        Some("/history") => {
                            editor.print_history()?;
                        }
                        Some("/clear") => {
                            editor.clear_scrollback()?;
                        }
                        Some("/new") => {
                            session = session::Session::new()?;
                            println!(
                                "{}",
                                term.green(&format!("Started session {}.", session.id))
                            );
                        }
                        Some("/session") => {
                            let title = session
                                .title
                                .as_deref()
                                .unwrap_or("(untitled)");
                            println!(
                                "Session: {} · {} · {} messages",
                                session.id,
                                title,
                                session.messages.len()
                            );
                        }
                        Some("/sessions") => {
                            let sessions = session::Session::list()?;
                            if sessions.is_empty() {
                                println!("No saved sessions for this workspace.");
                            } else {
                                for saved in sessions.into_iter().take(20) {
                                    let title = saved
                                        .title
                                        .as_deref()
                                        .unwrap_or("(untitled)");
                                    println!(
                                        "  {} · {} · {} messages · {}",
                                        saved.id, title, saved.messages, saved.updated_at
                                    );
                                }
                            }
                        }
                        Some("/resume") => {
                            let id = parts.get(1).copied().unwrap_or("latest");
                            session = session::Session::load(id)?;
                            let title = session
                                .title
                                .as_deref()
                                .map(|value| format!(" · {value}"))
                                .unwrap_or_default();
                            println!(
                                "{}",
                                term.green(&format!("Resumed session {}{title}.", session.id))
                            );
                        }
                        Some("/export") => {
                            let path = parts
                                .get(1)
                                .map(std::path::PathBuf::from)
                                .unwrap_or_else(|| session.default_export_path());
                            let written = session.export_markdown(&path)?;
                            println!(
                                "{}",
                                term.green(&format!("Exported session to {}.", written.display()))
                            );
                        }
                        Some("/compact") => {
                            let force = parts.get(1).is_none_or(|v| *v != "auto");
                            match session.compact(config, force) {
                                Some(note) => {
                                    session.save()?;
                                    println!("{}", term.green(&note));
                                    let meter = session.meter(config);
                                    println!(
                                        "{}",
                                        term.dim(&format!(
                                            "{} {}",
                                            meter.bar(16),
                                            meter.short_label()
                                        ))
                                    );
                                }
                                None => println!(
                                    "{}",
                                    term.dim("Context is within budget; nothing compacted.")
                                ),
                            }
                        }
                        _ => {
                            if !handle_slash(
                                config,
                                &mut mode,
                                &mut ultra_tier,
                                &mut steps_override,
                                &mut session_approval,
                                &session,
                                &parts,
                                term,
                            )
                            .await?
                            {
                                break;
                            }
                        }
                    }
                    continue;
                }

                println!();
                let started = std::time::Instant::now();
                let run_opts = agent::RunOptions { max_steps: steps_override, approval: session_approval, cancel: None, };
                match run_query(config, mode, input, &ultra_tier, term, &mut session, run_opts)
                    .await
                {
                    Ok(()) => {
                        // Successful agent turns save themselves; reload so titles/progress match disk.
                        if let Ok(latest) = session::Session::load(&session.id) {
                            session = latest;
                        }
                        println!(
                            "{}",
                            term.dim(&format!("done in {:.1}s", started.elapsed().as_secs_f32()))
                        );
                    }
                    Err(error) => {
                        // Do not overwrite crash-safe progress snapshots written mid-loop.
                        if let Ok(latest) = session::Session::load(&session.id) {
                            session = latest;
                        }
                        eprintln!("{}", term.red(&format!("Error: {error:#}")));
                    }
                }
                println!();
            }
            Ok(Signal::CtrlC) => println!("^C"),
            Ok(Signal::CtrlD) => {
                println!("bye");
                break;
            }
            Err(error) => return Err(error.into()),
        }
    }
    Ok(())
}

async fn handle_slash(
    config: &mut config::Config,
    mode: &mut RunMode,
    ultra_tier: &mut String,
    steps_override: &mut Option<usize>,
    session_approval: &mut Option<config::ApprovalMode>,
    session: &session::Session,
    parts: &[&str],
    term: &terminal::TerminalState,
) -> Result<bool> {
    let Some(command) = commands::canonical_name(parts[0]) else {
        let suggestions = commands::suggestions(parts[0], 3);
        if suggestions.is_empty() {
            println!("{}", term.dim("Unknown command. Type /help."));
        } else {
            println!(
                "{}",
                term.dim(&format!("Did you mean {}?", suggestions.join(", ")))
            );
        }
        return Ok(true);
    };

    match command {
        "/help" => print_repl_help(term),
        "/status" => print_status(
            config,
            session,
            *mode,
            ultra_tier,
            *steps_override,
            *session_approval,
            term,
        ),
        "/model" => {
            if let Some(model) = parts.get(1) {
                config.default_model = (*model).into();
                config.save()?;
                println!("{}", term.green(&format!("Model: {model}")));
            } else {
                println!("Current model: {}", term.cyan(&config.default_model));
            }
        }
        "/key" => {
            let Some(key) = parts.get(1) else {
                anyhow::bail!("Usage: /key <api-key>");
            };
            let provider = config.default_provider.clone();
            config.set_provider_key(&provider, key)?;
            config.save()?;
            println!("{}", term.green(&format!("Saved key for {provider}.")));
        }
        "/provider" => handle_provider_async(config, &parts[1..], term).await?,
        "/providers" => print_providers(config, term),
        "/config" => print_config(config, parts.get(1).copied(), term),
        "/context" => print_context(config, session, term),
        "/instructions" => print_instructions(term),
        "/language" => {
            let Some(language) = parts.get(1) else {
                anyhow::bail!("Usage: /language <auto|vi|en>");
            };
            validate_language(language)?;
            config.lang = (*language).into();
            config.save()?;
            println!("{}", term.green(&format!("Language: {language}")));
        }
        "/approval" => {
            match parts.get(1).copied() {
                None => {}
                Some("session") => match parts.get(2).copied() {
                    Some("clear" | "reset" | "default") => {
                        *session_approval = None;
                    }
                    Some(value) => {
                        *session_approval = Some(config::ApprovalMode::parse(value)?);
                    }
                    None => anyhow::bail!("Usage: /approval session <on|off|clear>"),
                },
                Some(value) => {
                    config.approval_mode = config::ApprovalMode::parse(value)?;
                    config.save()?;
                    *session_approval = None;
                }
            }
            let effective = session_approval
                .unwrap_or(config.approval_mode)
                .label();
            if let Some(session_mode) = session_approval {
                println!(
                    "Approval: {effective} (session override; config={})",
                    config.approval_mode.label()
                );
                let _ = session_mode;
            } else {
                println!("Approval: {effective}");
            }
        }
        "/steps" => {
            if let Some(value) = parts.get(1) {
                if matches!(*value, "clear" | "reset" | "default") {
                    *steps_override = None;
                } else {
                    let parsed: usize = value
                        .parse()
                        .context("Usage: /steps <1-50|clear>")?;
                    if !(1..=50).contains(&parsed) {
                        anyhow::bail!("Steps must be between 1 and 50");
                    }
                    *steps_override = Some(parsed);
                }
            }
            let effective = steps_override.unwrap_or(config.max_agent_steps);
            if steps_override.is_some() {
                println!(
                    "Agent steps: {effective} (session override; config={})",
                    config.max_agent_steps
                );
            } else {
                println!("Agent steps: {effective}");
            }
        }
        "/chat" => {
            *mode = RunMode::Chat;
            println!("{}", term.green("Chat mode"));
        }
        "/godmode" => {
            *mode = RunMode::Godmode;
            println!("{}", term.yellow("GODMODE"));
        }
        "/snake" => {
            *mode = RunMode::Parseltongue;
            println!("{}", term.green("Parseltongue mode"));
        }
        "/ultra" => {
            if let Some(tier) = parts.get(1) {
                validate_tier(tier)?;
                *ultra_tier = (*tier).into();
            }
            *mode = RunMode::Ultra;
            println!("{}", term.cyan(&format!("ULTRAPLINIAN ({ultra_tier})")));
        }
        "/exit" => return Ok(false),
        "/new"
        | "/history"
        | "/clear"
        | "/session"
        | "/sessions"
        | "/resume"
        | "/export"
        | "/compact" => {}
        _ => unreachable!("all registry commands must be handled"),
    }
    Ok(true)
}

fn handle_provider(
    config: &mut config::Config,
    args: &[&str],
    term: &terminal::TerminalState,
) -> Result<()> {
    match args.first().copied() {
        None | Some("list") => print_providers(config, term),
        Some("default") | Some("use") => {
            let id = args
                .get(1)
                .context("Usage: /provider use <id>  (alias: default)")?;
            config.set_default_provider(id)?;
            config.save()?;
            println!(
                "{}",
                term.green(&format!(
                    "Provider: {id} · model {}",
                    config.default_model
                ))
            );
        }
        Some("key") => {
            let id = args.get(1).context("Usage: /provider key <id> <key>")?;
            let key = args.get(2).context("Usage: /provider key <id> <key>")?;
            config.set_provider_key(id, key)?;
            config.save()?;
            println!("{}", term.green(&format!("Saved key for {id}.")));
        }
        Some("add") => {
            let id = args
                .get(1)
                .context("Usage: /provider add <id> <endpoint> [key-env]")?;
            let endpoint = args
                .get(2)
                .context("Usage: /provider add <id> <endpoint> [key-env]")?;
            config.add_provider(id, endpoint, args.get(3).copied())?;
            config.save()?;
            println!("{}", term.green(&format!("Added provider {id} → {endpoint}")));
        }
        Some("setup") => {
            // /provider setup <id> <endpoint> <api-key> [model]
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
            config.setup_provider(id, endpoint, Some(key), model, None)?;
            config.save()?;
            println!(
                "{}",
                term.green(&format!(
                    "Ready: provider={id} endpoint={endpoint} model={}",
                    config.default_model
                ))
            );
        }
        Some("endpoint") => {
            let id = args
                .get(1)
                .context("Usage: /provider endpoint <id> <url>")?;
            let endpoint = args
                .get(2)
                .context("Usage: /provider endpoint <id> <url>")?;
            config.set_provider_endpoint(id, endpoint)?;
            config.save()?;
            println!("{}", term.green(&format!("Endpoint {id} → {endpoint}")));
        }
        Some("auth") => {
            let id = args
                .get(1)
                .context("Usage: /provider auth <id> <bearer|x-api-key|raw|none>")?;
            let style = args
                .get(2)
                .context("Usage: /provider auth <id> <bearer|x-api-key|raw|none>")?;
            config.set_provider_auth_style(id, style)?;
            config.save()?;
            println!("{}", term.green(&format!("Auth style {id} → {style}")));
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
            config.set_provider_header(id, name, value)?;
            config.save()?;
            println!("{}", term.green(&format!("Header {id}: {name}")));
        }
        Some("remove") => {
            let id = args.get(1).context("Usage: /provider remove <id>")?;
            config.remove_provider(id)?;
            config.save()?;
            println!("{}", term.green(&format!("Removed provider {id}.")));
        }
        Some(action) => anyhow::bail!(
            "Unknown provider action: {action}. Try list|use|add|setup|key|endpoint|auth|header|test|models|remove"
        ),
    }
    Ok(())
}

async fn handle_provider_async(
    config: &mut config::Config,
    args: &[&str],
    term: &terminal::TerminalState,
) -> Result<()> {
    match args.first().copied() {
        Some("test") => {
            if let Some(id) = args.get(1) {
                config.set_default_provider(id)?;
                config.save()?;
            }
            let key = config.get_api_key()?;
            let result = api::test_provider(config, &key).await?;
            println!("{}", term.green(&result));
        }
        Some("models") => {
            if let Some(id) = args.get(1) {
                config.set_default_provider(id)?;
                config.save()?;
            }
            let key = config.get_api_key()?;
            let models = api::list_models(config, &key).await?;
            println!(
                "{}",
                term.bold(&format!(
                    "{} models from {}",
                    models.len(),
                    api::models_url(config)
                ))
            );
            for model in models.iter().take(80) {
                let mark = if *model == config.default_model {
                    "*"
                } else {
                    " "
                };
                println!(" {mark} {model}");
            }
            if models.len() > 80 {
                println!("  … {} more", models.len() - 80);
            }
        }
        _ => handle_provider(config, args, term)?,
    }
    Ok(())
}

async fn run_query(
    config: &config::Config,
    mode: RunMode,
    query: &str,
    tier: &str,
    term: &terminal::TerminalState,
    session: &mut session::Session,
    run_opts: agent::RunOptions,
) -> Result<()> {
    let key = config.get_api_key()?;
    match mode {
        RunMode::Chat => {
            let mut sink = output::ConsoleSink { term };
            agent::run(config, &key, query, term, session, run_opts, &mut sink).await
        }
        RunMode::Godmode => run_godmode(config, &key, query, term).await,
        RunMode::Parseltongue => {
            run_parseltongue(config, &key, query, term, &mut session.messages).await?;
            session.ensure_title(query);
            session.save()?;
            Ok(())
        }
        RunMode::Ultra => run_ultra(config, &key, query, tier, term).await,
    }
}

fn api_client() -> Result<reqwest::Client> {
    api::http_client()
}

async fn run_chat(
    config: &config::Config,
    key: &str,
    query: &str,
    _term: &terminal::TerminalState,
    session: &mut Vec<Value>,
) -> Result<()> {
    let client = api_client()?;
    let url = api::chat_completions_url(config);
    let project = context::read_context();
    let language = match config.lang.as_str() {
        "vi" => "Reply in Vietnamese.",
        "en" => "Reply in English.",
        _ => "Reply in the user's language.",
    };
    let system = format!(
        "You are g0d, a concise coding assistant. {language}\n\n{}",
        context::context_summary(&project)
    );
    let mut messages = vec![json!({"role": "system", "content": system})];
    messages.extend(session.iter().cloned());
    messages.push(json!({"role": "user", "content": query}));
    let body = json!({
        "model": &config.default_model,
        "messages": messages,
        "stream": true,
        "temperature": 0.4,
        "max_tokens": 8192
    });

    let indicator = status::StatusIndicator::start("Thinking");
    let builder = client
        .post(&url)
        .header("Content-Type", "application/json")
        .json(&body);
    let builder = api::apply_provider_auth(builder, config.active_provider(), key)?;
    let response = builder
        .send()
        .await
        .with_context(|| format!("API request failed ({url})"))?;
    indicator.stop();
    let response = ensure_success(response, &url).await?;
    let is_event_stream = response
        .headers()
        .get(reqwest::header::CONTENT_TYPE)
        .and_then(|value| value.to_str().ok())
        .is_some_and(|value| value.to_ascii_lowercase().contains("text/event-stream"));
    if !is_event_stream {
        let payload: Value = response
            .json()
            .await
            .context("Provider returned invalid JSON")?;
        let answer = payload
            .pointer("/choices/0/message/content")
            .and_then(Value::as_str)
            .context("Provider returned an unsupported response")?
            .to_string();
        println!("{answer}");
        session.push(json!({"role": "user", "content": query}));
        session.push(json!({"role": "assistant", "content": answer}));
        trim_session(session, config.max_context_messages);
        return Ok(());
    }

    let mut stream = response.bytes_stream();
    let mut pending = Vec::<u8>::new();
    let mut answer = String::new();
    while let Some(chunk) = stream.next().await {
        let chunk = chunk.context("Failed while reading response stream")?;
        for content in push_sse_chunk(&mut pending, &chunk)? {
            print!("{content}");
            std::io::stdout().flush()?;
            answer.push_str(&content);
        }
    }
    if !pending.is_empty() {
        let line = std::str::from_utf8(&pending).context("Provider returned invalid UTF-8")?;
        if let Some(content) = parse_sse_line(line.trim()) {
            print!("{content}");
            answer.push_str(&content);
        }
    }
    println!();

    if answer.is_empty() {
        anyhow::bail!("Provider returned an empty response stream");
    }
    session.push(json!({"role": "user", "content": query}));
    session.push(json!({"role": "assistant", "content": answer}));
    trim_session(session, config.max_context_messages);
    Ok(())
}

fn push_sse_chunk(pending: &mut Vec<u8>, chunk: &[u8]) -> Result<Vec<String>> {
    pending.extend_from_slice(chunk);
    let mut contents = Vec::new();
    while let Some(newline) = pending.iter().position(|byte| *byte == b'\n') {
        let line: Vec<u8> = pending.drain(..=newline).collect();
        let line = std::str::from_utf8(&line).context("Provider returned invalid UTF-8")?;
        if let Some(content) = parse_sse_line(line.trim_end_matches(['\r', '\n'])) {
            contents.push(content);
        }
    }
    Ok(contents)
}

fn trim_session(session: &mut Vec<Value>, limit: usize) {
    if session.len() > limit {
        session.drain(..session.len() - limit);
    }
}

fn parse_sse_line(line: &str) -> Option<String> {
    let data = line.strip_prefix("data:")?.trim();
    if data.is_empty() || data == "[DONE]" {
        return None;
    }
    let payload: Value = serde_json::from_str(data).ok()?;
    payload
        .pointer("/choices/0/delta/content")?
        .as_str()
        .map(str::to_string)
}

async fn ensure_success(response: reqwest::Response, url: &str) -> Result<reqwest::Response> {
    if response.status().is_success() {
        return Ok(response);
    }
    let status = response.status();
    let body = response.text().await.unwrap_or_default();
    anyhow::bail!(api::format_http_error(status, &body, url))
}

async fn run_parseltongue(
    config: &config::Config,
    key: &str,
    query: &str,
    term: &terminal::TerminalState,
    session: &mut Vec<Value>,
) -> Result<()> {
    use xai_grok_godmode::parseltongue::{Intensity, Parseltongue};
    let parser = Parseltongue::new();
    let transformed = parser.transform(query, Intensity::Standard, &[]);
    if !transformed.applied_transformations.is_empty() {
        println!("{} {}", term.green("Transformed:"), transformed.transformed);
    }
    run_chat(config, key, query, term, session).await
}

async fn run_godmode(
    config: &config::Config,
    key: &str,
    query: &str,
    term: &terminal::TerminalState,
) -> Result<()> {
    let client = api_client()?;
    let url = api::chat_completions_url(config);
    let provider = config.active_provider().clone();
    let presets = xai_grok_godmode::config::default_presets();
    let indicator = status::StatusIndicator::start("Racing 5 candidates");
    let mut tasks = Vec::new();

    for preset in presets {
        let client = client.clone();
        let key = key.to_string();
        let query = query.to_string();
        let url = url.clone();
        let provider = provider.clone();
        tasks.push(tokio::spawn(async move {
            let body = json!({
                "model": preset.model,
                "messages": [
                    {"role": "system", "content": format!("You are {}: {}", preset.persona.name, preset.persona.role)},
                    {"role": "user", "content": query}
                ],
                "temperature": preset.temperature,
                "max_tokens": 4096
            });
            let builder = client.post(&url).json(&body);
            let builder = api::apply_provider_auth(builder, &provider, &key).map_err(|e| e.to_string())?;
            let response = builder.send().await.map_err(|error| error.to_string())?;
            if !response.status().is_success() { return Err(format!("HTTP {}", response.status())); }
            let payload: Value = response.json().await.map_err(|error| error.to_string())?;
            let content = payload.pointer("/choices/0/message/content").and_then(Value::as_str).unwrap_or_default().to_string();
            Ok::<_, String>((preset, content))
        }));
    }

    let results = futures::future::join_all(tasks).await;
    indicator.stop();
    let mut successes = 0usize;
    for result in results {
        match result {
            Ok(Ok((preset, content))) => {
                successes += 1;
                println!(
                    "\n{} {}\n{}",
                    term.cyan(&preset.persona.name),
                    term.dim(&preset.model),
                    content
                );
            }
            Ok(Err(error)) => eprintln!("{}", term.red(&format!("Candidate failed: {error}"))),
            Err(error) => eprintln!("{}", term.red(&format!("Candidate task failed: {error}"))),
        }
    }
    if successes == 0 {
        anyhow::bail!("All GODMODE candidates failed");
    }
    Ok(())
}

async fn run_ultra(
    config: &config::Config,
    key: &str,
    query: &str,
    tier_name: &str,
    term: &terminal::TerminalState,
) -> Result<()> {
    use xai_grok_godmode::ultraplinian::UltraplinianTier;
    validate_tier(tier_name)?;
    let tier = match tier_name {
        "fast" => UltraplinianTier::Fast,
        "standard" => UltraplinianTier::Standard,
        "smart" => UltraplinianTier::Smart,
        "power" => UltraplinianTier::Power,
        "ultra" => UltraplinianTier::Ultra,
        _ => unreachable!(),
    };
    let models = xai_grok_godmode::tier_models(&tier);
    let selected: Vec<_> = models.into_iter().take(8).collect();
    let client = api_client()?;
    let url = api::chat_completions_url(config);
    let provider = config.active_provider().clone();
    let indicator =
        status::StatusIndicator::start(&format!("{} · {} models", tier.label(), selected.len()));
    let mut tasks = Vec::new();

    for model in selected {
        let client = client.clone();
        let key = key.to_string();
        let query = query.to_string();
        let url = url.clone();
        let provider = provider.clone();
        tasks.push(tokio::spawn(async move {
            let body = json!({"model": model, "messages": [{"role": "user", "content": query}], "temperature": 0.3, "max_tokens": 2048});
            let builder = client.post(&url).json(&body);
            let builder = api::apply_provider_auth(builder, &provider, &key).map_err(|e| e.to_string())?;
            let response = builder.send().await.map_err(|error| error.to_string())?;
            if !response.status().is_success() { return Err(format!("{model}: HTTP {}", response.status())); }
            let payload: Value = response.json().await.map_err(|error| error.to_string())?;
            let content = payload.pointer("/choices/0/message/content").and_then(Value::as_str).unwrap_or_default().to_string();
            Ok::<_, String>((model, content))
        }));
    }

    let results = futures::future::join_all(tasks).await;
    indicator.stop();
    let mut successes = 0usize;
    for result in results {
        match result {
            Ok(Ok((model, content))) => {
                successes += 1;
                println!("\n{}\n{}", term.cyan(&model), content);
            }
            Ok(Err(error)) => eprintln!("{}", term.red(&error)),
            Err(error) => eprintln!("{}", term.red(&format!("Model task failed: {error}"))),
        }
    }
    if successes == 0 {
        anyhow::bail!("All ULTRAPLINIAN model requests failed");
    }
    Ok(())
}

fn validate_language(language: &str) -> Result<()> {
    if matches!(language, "auto" | "vi" | "en") {
        Ok(())
    } else {
        anyhow::bail!("Language must be auto, vi, or en")
    }
}

fn validate_tier(tier: &str) -> Result<()> {
    if matches!(tier, "fast" | "standard" | "smart" | "power" | "ultra") {
        Ok(())
    } else {
        anyhow::bail!("Tier must be fast, standard, smart, power, or ultra")
    }
}

fn print_banner(config: &config::Config, term: &terminal::TerminalState) {
    let project = context::read_context();
    println!(
        "{}",
        term.bold(&format!("g0d {VERSION} · AI coding assistant"))
    );
    println!(
        "{}",
        term.dim(&format!(
            "{} · {} · {} · auto_compact={}",
            project.cwd, project.project_type, config.default_model, config.auto_compact
        ))
    );
    if config.get_api_key().is_err() {
        println!(
            "\n{}",
            term.yellow("No API key found. Set an environment variable or run /key <api-key>.")
        );
    }
    println!(
        "\n{}",
        term.dim(
            "Tab complete · Ctrl-R history · Ctrl-L clear · Alt-Enter newline · /help commands"
        )
    );
}

fn print_cli_help(term: &terminal::TerminalState) -> Result<()> {
    println!(
        "{}",
        term.bold(&format!("g0d {VERSION} — multi-provider coding assistant"))
    );
    println!("\nUSAGE\n  g0d [OPTIONS] [QUERY]\n");
    println!("MODES\n  -g, --godmode        Race five candidates\n  -p, --snake          Parseltongue mode\n  -u, --ultra          ULTRAPLINIAN mode\n      --tier <TIER>     fast|standard|smart|power|ultra\n");
    println!("OPTIONS\n      --provider <ID>  Select and persist provider\n      --model <ID>     Select and persist model\n      --endpoint <URL> One-shot base URL override (third-party, not saved)\n  -k, --key <KEY>      Save key for active provider\n      --language <LANG> auto|vi|en\n      --models         List model tiers\n      --config         Print config path\n      --headless       Disable interactive terminal behavior\n      --classic        Classic reedline REPL instead of Grok-style TUI\n      --no-color       Disable ANSI color\n  -h, --help           Show help\n  -V, --version        Show version\n");
    println!("AGENT\n      --approval MODE  Persist on (ask) or off (automatic)\n      --steps N        Override max agent steps for this process (1-50)\n      --resume[=ID]    Resume latest or a workspace session\n");
    println!("TOOLS\n  list/glob/search/read · replace/create/write/delete/rename · apply_patch · run_command · git status/diff/log/add/commit\n");
    println!(
        "With no query, g0d opens the Grok-style TUI (use --classic for the old REPL). Piped stdin is accepted in non-TTY mode."
    );
    Ok(())
}

fn print_repl_help(term: &terminal::TerminalState) {
    for command in commands::registry() {
        println!("  {:<34} {}", term.cyan(command.usage), command.desc);
    }
    println!("\n{}", term.dim("Keys: Tab complete · Shift-Tab previous · Ctrl-R history · Ctrl-L clear · Alt-Enter newline · Ctrl-D exit"));
}

fn print_status(
    config: &config::Config,
    session: &session::Session,
    mode: RunMode,
    ultra_tier: &str,
    steps_override: Option<usize>,
    session_approval: Option<config::ApprovalMode>,
    term: &terminal::TerminalState,
) {
    let provider = config.active_provider();
    let key_source = if provider
        .api_key
        .as_deref()
        .is_some_and(|key| !key.is_empty())
    {
        "config"
    } else if provider
        .key_env
        .as_deref()
        .is_some_and(|name| std::env::var_os(name).is_some())
    {
        "environment"
    } else if provider.is_local {
        "not required"
    } else {
        "missing"
    };
    let approval = session_approval.unwrap_or(config.approval_mode);
    let steps = steps_override.unwrap_or(config.max_agent_steps);
    let meter = session.meter(config);
    println!(
        "  Mode: {}{}",
        mode.label(),
        if mode == RunMode::Ultra {
            format!(" ({ultra_tier})")
        } else {
            String::new()
        }
    );
    println!(
        "  Provider: {}  {}",
        term.cyan(&provider.id),
        term.dim(&provider.endpoint)
    );
    println!("  Model: {}", config.default_model);
    if session_approval.is_some() {
        println!(
            "  Approval: {} (session; config={})",
            approval.label(),
            config.approval_mode.label()
        );
    } else {
        println!("  Approval: {}", approval.label());
    }
    if steps_override.is_some() {
        println!(
            "  Agent steps: {steps} (session; config={})",
            config.max_agent_steps
        );
    } else {
        println!("  Agent steps: {steps}");
    }
    println!(
        "  Context: {} {} · est ~{} / {} ({}%) · cap {} msgs · auto_compact={}",
        meter.bar(16),
        meter.messages,
        meter::format_token_count(meter.estimated_tokens),
        meter::format_token_count(meter.budget),
        meter.usage_pct().min(100),
        config.max_context_messages,
        config.auto_compact
    );
    println!(
        "  Tokens: last p{}/c{} · lifetime p{}/c{} · compact×{}",
        meter.last_prompt,
        meter.last_completion,
        meter.lifetime_prompt,
        meter.lifetime_completion,
        meter.compact_count
    );
    println!("  API key: {key_source} · Language: {}", config.lang);
    if let Some(instructions) = &context::read_context().instructions {
        println!("  Instructions: {}", instructions.source);
    } else {
        println!("  Instructions: (none)");
    }
    println!("  Config: {}", config::config_path().display());
    println!("  History: {}", config::history_path().display());
    println!("  Sessions: {}", config::sessions_dir().display());
}

fn print_instructions(term: &terminal::TerminalState) {
    match context::read_context().instructions {
        Some(instructions) => {
            println!(
                "{}",
                term.bold(&format!("Project instructions · {}", instructions.source))
            );
            println!("{}", instructions.body);
        }
        None => println!(
            "{}",
            term.dim(
                "No project instructions found. Create AGENTS.md, G0D.md, or .g0d/instructions.md."
            )
        ),
    }
}

fn print_config(config: &config::Config, action: Option<&str>, term: &terminal::TerminalState) {
    if action == Some("path") {
        println!("{}", config::config_path().display());
        return;
    }
    println!("default_provider = {:?}", config.default_provider);
    println!("default_model = {:?}", config.default_model);
    println!("language = {:?}", config.lang);
    println!("max_context_messages = {}", config.max_context_messages);
    println!("max_agent_steps = {}", config.max_agent_steps);
    println!("context_token_budget = {}", config.context_token_budget);
    println!("auto_compact = {}", config.auto_compact);
    println!("keep_recent_messages = {}", config.keep_recent_messages);
    println!("approval_mode = {:?}", config.approval_mode.label());
    println!("providers:");
    for provider in &config.providers {
        let secret = if provider.api_key.is_some() {
            "key=config"
        } else if let Some(env) = &provider.key_env {
            env
        } else {
            "no-key"
        };
        println!(
            "  {} {} [{}]",
            term.cyan(&provider.id),
            term.dim(&provider.endpoint),
            secret
        );
    }
}

fn print_context(
    config: &config::Config,
    session: &session::Session,
    term: &terminal::TerminalState,
) {
    let meter = session.meter(config);
    println!("{}", term.bold("Session context window"));
    println!(
        "  {} {} · est ~{} / {} tokens ({}%)",
        meter.bar(20),
        meter.messages,
        meter::format_token_count(meter.estimated_tokens),
        meter::format_token_count(meter.budget),
        meter.usage_pct().min(100)
    );
    println!(
        "  messages cap {} · keep_recent {} · auto_compact {} · compact×{}",
        config.max_context_messages,
        config.keep_recent_messages,
        config.auto_compact,
        meter.compact_count
    );
    if meter.last_prompt > 0 || meter.last_completion > 0 {
        println!(
            "  last API usage: prompt {} · completion {}",
            meter.last_prompt, meter.last_completion
        );
    }
    if meter.lifetime_prompt > 0 || meter.lifetime_completion > 0 {
        println!(
            "  lifetime API usage: prompt {} · completion {}",
            meter.lifetime_prompt, meter.lifetime_completion
        );
    }
    println!();
    let project = context::read_context();
    println!("{}", term.bold("Project context sent to the model"));
    print!("{}", context::context_summary(&project));
}



fn print_providers(config: &config::Config, term: &terminal::TerminalState) {
    for provider in &config.providers {
        let active = if provider.id == config.default_provider {
            "*"
        } else {
            " "
        };
        let key = if provider.is_local
            || provider.api_key.is_some()
            || provider
                .key_env
                .as_deref()
                .is_some_and(|name| std::env::var_os(name).is_some())
        {
            "ready"
        } else {
            "no key"
        };
        println!(
            " {active} {:<14} {:<8} {}",
            term.cyan(&provider.id),
            key,
            term.dim(&provider.endpoint)
        );
    }
}

fn list_models(term: &terminal::TerminalState) {
    use xai_grok_godmode::ultraplinian::UltraplinianTier;
    for tier in [
        UltraplinianTier::Fast,
        UltraplinianTier::Standard,
        UltraplinianTier::Smart,
        UltraplinianTier::Power,
        UltraplinianTier::Ultra,
    ] {
        let models = xai_grok_godmode::tier_models(&tier);
        println!("{} · {} models", term.bold(tier.label()), models.len());
        for model in models {
            println!("  {model}");
        }
    }
}

fn compact_model(model: &str) -> &str {
    model.rsplit('/').next().unwrap_or(model)
}

fn current_dir_name() -> String {
    std::env::current_dir()
        .ok()
        .and_then(|path| {
            path.file_name()
                .map(|name| name.to_string_lossy().to_string())
        })
        .unwrap_or_else(|| ".".into())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn strings(values: &[&str]) -> Vec<String> {
        values.iter().map(|value| (*value).to_string()).collect()
    }

    #[test]
    fn parses_multi_word_mode_query() {
        let parsed = parse_cli_args(&strings(&["-g", "fix", "the", "bug"])).unwrap();
        assert_eq!(
            parsed.action,
            CliAction::Query {
                mode: RunMode::Godmode,
                query: "fix the bug".into(),
                tier: "fast".into()
            }
        );
    }

    #[test]
    fn parses_global_overrides() {
        let parsed = parse_cli_args(&strings(&[
            "--provider",
            "grok",
            "--model",
            "grok-4",
            "hello",
        ]))
        .unwrap();
        assert_eq!(parsed.provider.as_deref(), Some("grok"));
        assert_eq!(parsed.model.as_deref(), Some("grok-4"));
    }

    #[test]
    fn rejects_missing_mode_query() {
        assert!(parse_cli_args(&strings(&["--ultra"])).is_err());
    }

    #[test]
    fn extracts_stream_content() {
        let line = r#"data: {"choices":[{"delta":{"content":"hello"}}]}"#;
        assert_eq!(parse_sse_line(line).as_deref(), Some("hello"));
        assert_eq!(parse_sse_line("data: [DONE]"), None);
    }

    #[test]
    fn preserves_utf8_split_across_network_chunks() {
        let event = "data: {\"choices\":[{\"delta\":{\"content\":\"chào\"}}]}\n".as_bytes();
        let split = event.iter().position(|byte| *byte >= 0x80).unwrap() + 1;
        let mut pending = Vec::new();
        assert!(push_sse_chunk(&mut pending, &event[..split])
            .unwrap()
            .is_empty());
        assert_eq!(
            push_sse_chunk(&mut pending, &event[split..]).unwrap(),
            ["chào"]
        );
    }
}
