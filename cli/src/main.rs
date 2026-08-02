mod config;
mod context;
mod terminal;
mod commands;

use crossterm::terminal::{disable_raw_mode, enable_raw_mode};
use std::io::Write;
use reedline::{DefaultPrompt, DefaultPromptSegment, Reedline, Signal};

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let mut cfg = config::Config::load();
    let args: Vec<String> = std::env::args().collect();
    let headless = args.iter().any(|a| a == "--headless");
    let term = terminal::TerminalState::detect(headless, None);

    // CLI flags mode (non-interactive) — exit early
    if args.len() > 1 {
        return handle_cli_flags(&cfg, &args, &term).await;
    }

    // Interactive: enable raw mode for reedline
    if term.is_tty {
        let _ = enable_raw_mode();
    }

    let result = run_repl(&mut cfg, &term).await;

    // Restore terminal
    if term.is_tty {
        let _ = disable_raw_mode();
    }
    println!();
    result
}

async fn handle_cli_flags(cfg: &config::Config, args: &[String], term: &terminal::TerminalState) -> anyhow::Result<()> {
    match args[1].as_str() {
        "-g"|"--godmode" => { let q = args.get(2).cloned().unwrap_or_default(); if !q.is_empty() { cmd_godmode(cfg, &q, term).await?; } }
        "-u"|"--ultra" => { let q = args.last().cloned().unwrap_or_default(); let tier = args.iter().find(|a| a.starts_with("--tier=")).map(|s| s[7..].to_string()).unwrap_or("fast".into()); cmd_ultra(cfg, &q, &tier, term).await?; }
        "-p"|"--snake" => { let q = args.get(2).cloned().unwrap_or_default(); if !q.is_empty() { cmd_parseltongue(cfg, &q, term).await?; } }
        "--config" => println!("{}", config::config_path().display()),
        "--models" => list_models(term),
        "-h"|"--help" => print_help(term),
        "--language" => { if let Some(v) = args.get(2) { let mut c = cfg.clone(); c.set_lang(v); c.save(); println!("Language: {}", v); } }
        _ => { let q = args[1..].join(" "); cmd_chat(cfg, &q, term).await?; }
    }
    Ok(())
}

async fn run_repl(cfg: &mut config::Config, term: &terminal::TerminalState) -> anyhow::Result<()> {
    banner(cfg, term);

    let history_file = dirs::data_dir().unwrap_or_default().join("g0d").join("history.txt");
    let _ = std::fs::create_dir_all(history_file.parent().unwrap());

    let completer = commands::SlashCompleter::new();
    let mut editor = Reedline::create()
        .with_completer(Box::new(completer))
        .with_history(Box::new(
            reedline::FileBackedHistory::with_file(1000, history_file).unwrap_or_default()
        ))
        .with_quick_completions(true)
        .with_partial_completions(true);

    let mut mode = "chat".to_string();

    loop {
        let prompt_text = match mode.as_str() {
            "godmode" => format!("godmode › "),
            "parseltongue" => format!("snake › "),
            "ultra" => format!("ultra › "),
            _ => format!("{} › ", current_dir_name()),
        };
        let prompt_text = if term.colors { format!("\x1b[90m{}\x1b[0m", prompt_text) } else { prompt_text };

        let sig = editor.read_line(&DefaultPrompt::new(
            DefaultPromptSegment::Basic(prompt_text),
            DefaultPromptSegment::Empty,
        ));

        match sig {
            Ok(Signal::Success(line)) => {
                let input = line.trim().to_string();
                if input.is_empty() { continue; }

                if input.starts_with('/') {
                    let parts: Vec<&str> = input.splitn(4, ' ').collect();
                    let handled = handle_slash(cfg, &mut mode, &parts, term);
                    if !handled { break; }
                    continue;
                }

                let key = match cfg.get_api_key() {
                    Ok(k) => k,
                    Err(e) => { println!("\n{}", term.y(&e.to_string())); println!("{}", term.d("Get key: https://openrouter.ai/keys /key sk-or-...")); continue; }
                };
                let endpoint = cfg.get_endpoint();

                println!();
                match mode.as_str() {
                    "godmode" => { let _ = run_godmode(&key, &endpoint, &input, term).await; }
                    "parseltongue" => { let _ = run_parseltongue_repl(&key, &endpoint, &input, term).await; }
                    "ultra" => { let _ = run_ultra(&key, &endpoint, &input, "fast", term).await; }
                    _ => { let _ = run_chat(&key, &endpoint, &input, &cfg.default_model(), term).await; }
                }
                println!();
            }
            Ok(Signal::CtrlD) | Ok(Signal::CtrlC) => { println!("bye"); break; }
            Err(e) => { eprintln!("Editor error: {}", e); break; }
        }
    }
    Ok(())
}

/// Returns false if should exit
fn handle_slash(cfg: &mut config::Config, mode: &mut String, parts: &[&str], term: &terminal::TerminalState) -> bool {
    match parts[0] {
        "/exit"|"/quit" => { println!("bye"); return false; }
        "/help" => print_slash_help(term),
        "/key" => cmd_key(cfg, parts.get(1).copied(), term),
        "/provider" => cmd_provider(cfg, parts.get(1).copied(), parts.get(2).copied(), parts.get(3).copied(), term),
        "/model" => cmd_model(cfg, parts.get(1).copied(), term),
        "/providers" => cmd_providers_list(cfg, term),
        "/status" => cmd_status(cfg, mode, term),
        "/chat" => { *mode = "chat".into(); println!("{}", term.g("Chat mode")); }
        "/godmode" => { *mode = "godmode".into(); println!("{}", term.y("GODMODE mode")); }
        "/snake" => { *mode = "parseltongue".into(); println!("{}", term.g("Parseltongue mode")); }
        "/ultra" => { *mode = "ultra".into(); println!("{}", term.c("ULTRAPLINIAN mode")); }
        "/language" => cmd_language(cfg, parts.get(1).copied(), term),
        invalid => {
            let suggestions = commands::registry().iter()
                .filter(|c| fuzzy_match(c.name, invalid))
                .map(|c| c.name).take(3).collect::<Vec<_>>();
            if suggestions.is_empty() {
                println!("{}", term.d("Unknown command. /help"));
            } else {
                println!("{}", term.d(&format!("Did you mean: {}?", suggestions.join(", "))));
            }
        }
    }
    true
}

fn fuzzy_match(a: &str, b: &str) -> bool {
    let a: Vec<char> = a.to_lowercase().chars().collect();
    let b: Vec<char> = b.to_lowercase().chars().collect();
    let mut bi = 0;
    for ac in &a {
        while bi < b.len() && b[bi] != *ac { bi += 1; }
        if bi >= b.len() { return false; }
        bi += 1;
    }
    true
}

fn banner(cfg: &config::Config, term: &terminal::TerminalState) {
    let ctx = context::read_context();
    println!("{}", term.b("g0d — AI coding agent"));
    println!("  {}  |  {}", ctx.cwd, ctx.project_type);
    if let Some(ref branch) = ctx.git_branch {
        let status = ctx.git_status.as_deref().unwrap_or("");
        if !status.is_empty() {
            println!("  {}  [{}]", branch, status);
        } else {
            println!("  {}", branch);
        }
    }
    let has_key = cfg.get_api_key().is_ok();
    if !has_key {
        println!("\n{}", term.y("No API key configured."));
        println!("{}  1. Get key: https://openrouter.ai/keys", term.d(""));
        println!("{}  2. /key sk-or-v1-...", term.d(""));
    }
    println!();
}

fn print_help(term: &terminal::TerminalState) {
    for c in commands::registry() {
        println!("  {:<20} {}", term.c(c.name), c.desc);
    }
}

fn print_slash_help(term: &terminal::TerminalState) {
    for c in commands::registry() {
        println!("  {:<22} {} — {}", term.c(c.name), term.d(&c.usage), c.desc_vi);
    }
}

fn current_dir_name() -> String {
    std::env::current_dir().ok()
        .and_then(|p| p.file_name().map(|n| n.to_string_lossy().to_string()))
        .unwrap_or_else(|| ".".into())
}

fn cmd_key(cfg: &mut config::Config, arg: Option<&str>, term: &terminal::TerminalState) {
    if let Some(key) = arg {
        cfg.set_provider_key(&cfg.active_provider_id(), key); cfg.save();
        println!("{}", term.g("Key saved."));
    } else {
        println!("{}", term.d("/key sk-or-v1-..."));
    }
}

fn cmd_provider(cfg: &mut config::Config, sub: Option<&str>, arg1: Option<&str>, arg2: Option<&str>, term: &terminal::TerminalState) {
    match sub {
        Some("key") => match (arg1, arg2) {
            (Some(id), Some(key)) => { cfg.set_provider_key(id, key); cfg.save(); println!("{}", term.g(&format!("Key set for {id}"))); }
            _ => println!("{}", term.d("/provider key <provider> <key>")),
        }
        Some("add") => match (arg1, arg2) {
            (Some(id), Some(endpoint)) => { cfg.add_provider(id.to_string(), endpoint.to_string(), None); cfg.save(); println!("{}", term.g(&format!("Added {id}"))); }
            _ => println!("{}", term.d("/provider add <name> <endpoint>")),
        }
        Some("default") => if let Some(id) = arg1 { cfg.set_default_provider(id); cfg.save(); println!("{}", term.g(&format!("Using {id}"))); }
        else { println!("{}", term.d(&format!("Current: {}", cfg.active_provider_id()))); }
        _ => {
            for c in commands::registry() { if c.name == "/provider" { println!("{} — {}", term.c(&c.usage), c.desc_vi); } }
            cmd_providers_mini(cfg, term);
        }
    }
}

fn cmd_providers_mini(cfg: &config::Config, term: &terminal::TerminalState) {
    for p in &cfg.providers {
        if !p.enabled { continue; }
        let star = if Some(&p.id) == cfg.default_provider.as_ref() { "*" } else { " " };
        let has_key = p.api_key.as_ref().map_or(false, |k| !k.is_empty())
            || p.key_env.as_ref().and_then(|e| std::env::var(e).ok()).map_or(false, |k| !k.is_empty());
        let dot = if has_key { term.g("●") } else { term.d("○") };
        println!("  {dot} {star} {}", p.id);
    }
}

fn cmd_model(cfg: &mut config::Config, arg: Option<&str>, term: &terminal::TerminalState) {
    match arg {
        Some(m) => { cfg.set_model(m); cfg.save(); println!("{}", term.g(&format!("Model: {m}"))); }
        _ => {
            println!("{}  Current: {}", term.d(""), term.c(&cfg.default_model()));
            println!("{}  Popular:", term.d(""));
            for m in ["anthropic/claude-sonnet-4.6", "openai/gpt-5.6", "google/gemini-2.5-pro", "deepseek/deepseek-chat"] {
                println!("{}    {}", term.d(""), m);
            }
        }
    }
}

fn cmd_providers_list(cfg: &config::Config, term: &terminal::TerminalState) {
    for p in &cfg.providers {
        if !p.enabled { continue; }
        let star = if Some(&p.id) == cfg.default_provider.as_ref() { "*" } else { " " };
        let has_key = p.api_key.as_ref().map_or(false, |k| !k.is_empty())
            || p.key_env.as_ref().and_then(|e| std::env::var(e).ok()).map_or(false, |k| !k.is_empty());
        let key_status = if has_key { term.g("set") } else { term.d("no key") };
        println!("  {star} {:<14} {}  {}", p.id, key_status, term.d(&p.endpoint));
    }
}

fn cmd_status(cfg: &config::Config, mode: &str, term: &terminal::TerminalState) {
    let p = cfg.active_provider();
    let has_key = cfg.get_api_key().is_ok();
    let lang = match cfg.get_lang() { "vi" => "VN", "en" => "EN", _ => "auto", };
    println!("  Mode: {} | Provider: {} | Model: {}", mode, p.id, cfg.default_model());
    println!("  Key: {} | Language: {} | Config: {}", if has_key { "set" } else { "not set" }, lang, config::config_path().display());
}

fn cmd_language(cfg: &mut config::Config, arg: Option<&str>, term: &terminal::TerminalState) {
    match arg {
        Some("vi") => { cfg.set_lang("vi"); cfg.save(); println!("{}", term.g("Vietnamese")); }
        Some("en") => { cfg.set_lang("en"); cfg.save(); println!("{}", term.g("English")); }
        Some("auto") => { cfg.set_lang("auto"); cfg.save(); println!("{}", term.g("Auto-detect")); }
        _ => println!("{}", term.d("/language auto | vi | en")),
    }
}

// ── API functions ─────────────────────────────────────────────

async fn cmd_chat(cfg: &config::Config, q: &str, term: &terminal::TerminalState) -> anyhow::Result<()> {
    let key = cfg.get_api_key()?; run_chat(&key, &cfg.get_endpoint(), q, &cfg.default_model(), term).await
}

async fn cmd_godmode(cfg: &config::Config, q: &str, term: &terminal::TerminalState) -> anyhow::Result<()> {
    let key = cfg.get_api_key()?; run_godmode(&key, &cfg.get_endpoint(), q, term).await
}

async fn cmd_parseltongue(cfg: &config::Config, q: &str, term: &terminal::TerminalState) -> anyhow::Result<()> {
    let key = cfg.get_api_key()?; let _ = run_parseltongue_repl(&key, &cfg.get_endpoint(), q, term).await; Ok(())
}

async fn cmd_ultra(cfg: &config::Config, q: &str, tier: &str, term: &terminal::TerminalState) -> anyhow::Result<()> {
    let key = cfg.get_api_key()?; run_ultra(&key, &cfg.get_endpoint(), q, tier, term).await
}

fn list_models(term: &terminal::TerminalState) {
    use xai_grok_godmode::ultraplinian::UltraplinianTier;
    for tier in [UltraplinianTier::Fast, UltraplinianTier::Standard, UltraplinianTier::Smart, UltraplinianTier::Power, UltraplinianTier::Ultra] {
        let models = xai_grok_godmode::tier_models(&tier);
        println!("{} {} models", term.b(&tier.label()), models.len());
        for m in &models { println!("  {}", m); }
    }
}

async fn run_chat(key: &str, endpoint: &str, query: &str, model: &str, term: &terminal::TerminalState) -> anyhow::Result<()> {
    let client = reqwest::Client::new();
    let url = format!("{}/chat/completions", endpoint.trim_end_matches('/'));
    let ctx = context::read_context();
    let system = format!("You are g0d. Help with code.\n\n{}Be concise.", context::context_summary(&ctx));
    let body = serde_json::json!({
        "model": model, "messages": [{"role":"system","content":system},{"role":"user","content":query}],
        "stream": true, "temperature": 0.7, "max_tokens": 8192,
    });
    let resp = client.post(&url).header("Authorization", format!("Bearer {}", key))
        .header("Content-Type", "application/json").json(&body).send().await?;
    if !resp.status().is_success() { let s = resp.status(); anyhow::bail!("HTTP {}: {}", s, resp.text().await.unwrap_or_default()); }
    let mut stream = resp.bytes_stream();
    use futures::StreamExt;
    while let Some(chunk) = stream.next().await {
        for line in String::from_utf8_lossy(&chunk?).lines() {
            let line = line.trim(); if !line.starts_with("data: ") { continue; } if &line[6..] == "[DONE]" { continue; }
            if let Ok(p) = serde_json::from_str::<serde_json::Value>(&line[6..]) {
                if let Some(c) = p["choices"][0]["delta"]["content"].as_str() { print!("{}", c); std::io::stdout().flush()?; }
            }
        }
    }
    println!();
    Ok(())
}

async fn run_parseltongue_repl(key: &str, endpoint: &str, query: &str, term: &terminal::TerminalState) -> anyhow::Result<()> {
    use xai_grok_godmode::parseltongue::{Parseltongue, Intensity};
    let p = Parseltongue::new();
    for (label, intensity) in [("Light", Intensity::Light), ("Standard", Intensity::Standard), ("Heavy", Intensity::Heavy)] {
        let r = p.transform(query, intensity, &[]);
        if !r.applied_transformations.is_empty() {
            println!("{} {}", term.g(&format!("[{label}]")), r.transformed);
        }
    }
    run_chat(key, endpoint, query, "anthropic/claude-sonnet-4", term).await
}

async fn run_godmode(key: &str, endpoint: &str, query: &str, term: &terminal::TerminalState) -> anyhow::Result<()> {
    use xai_grok_godmode::config as gm_config;
    let client = reqwest::Client::new();
    let presets = gm_config::default_presets();
    let url = format!("{}/chat/completions", endpoint.trim_end_matches('/'));
    println!("{}", term.y("GODMODE — 5 models racing..."));

    let mut tasks = Vec::new();
    for preset in &presets {
        let c = client.clone(); let k = key.to_string(); let q = query.to_string();
        let p = preset.clone(); let u = url.clone();
        tasks.push(tokio::spawn(async move {
            let body = serde_json::json!({
                "model": p.model, "messages": [
                    {"role":"system","content": format!("You are {}: {}", p.persona.name, p.persona.role)},
                    {"role":"user","content": q},
                ], "temperature": p.temperature, "max_tokens": 4096,
            });
            let r = c.post(&u).header("Authorization", format!("Bearer {}", k))
                .header("Content-Type", "application/json").json(&body).send().await;
            match r {
                Ok(resp) if resp.status().is_success() => {
                    let json: serde_json::Value = resp.json().await.unwrap_or_default();
                    Ok((p, json["choices"][0]["message"]["content"].as_str().unwrap_or("").to_string()))
                }
                Ok(resp) => Err(format!("HTTP {}", resp.status())), Err(e) => Err(format!("{}", e)),
            }
        }));
    }
    let results = futures::future::join_all(tasks).await;
    for r in &results {
        if let Ok(Ok((p, content))) = r {
            let t: String = content.chars().take(200).collect();
            println!("{} {} ({})", term.c(&p.persona.name), p.model, &t.replace('\n', " "));
        }
    }
    Ok(())
}

async fn run_ultra(key: &str, endpoint: &str, query: &str, tier_name: &str, term: &terminal::TerminalState) -> anyhow::Result<()> {
    use xai_grok_godmode::ultraplinian::UltraplinianTier;
    let tier = match tier_name {
        "fast" => UltraplinianTier::Fast, "standard" => UltraplinianTier::Standard,
        "smart" => UltraplinianTier::Smart, "power" => UltraplinianTier::Power, "ultra" => UltraplinianTier::Ultra,
        _ => anyhow::bail!("Unknown tier: {}", tier_name),
    };
    let models = xai_grok_godmode::tier_models(&tier);
    let client = reqwest::Client::new();
    let url = format!("{}/chat/completions", endpoint.trim_end_matches('/'));
    println!("{} {} models", term.c(&tier.label()), models.len());

    let show = models.len().min(8);
    let mut tasks = Vec::new();
    for m in models.iter().take(show) {
        let c = client.clone(); let k = key.to_string(); let q = query.to_string();
        let m = m.clone(); let u = url.clone();
        tasks.push(tokio::spawn(async move {
            let body = serde_json::json!({"model":m,"messages":[{"role":"user","content":q}],"temperature":0.3,"max_tokens":2048});
            let r = c.post(&u).header("Authorization", format!("Bearer {}", k))
                .header("Content-Type", "application/json").json(&body).send().await;
            match r {
                Ok(resp) if resp.status().is_success() => {
                    let json: serde_json::Value = resp.json().await.unwrap_or_default();
                    let text = json["choices"][0]["message"]["content"].as_str().unwrap_or("").to_string();
                    Ok((m, text))
                }
                Ok(resp) => Err(format!("HTTP {}", resp.status())),
                Err(e) => Err(format!("{}", e)),
            }
        }));
    }
    for r in futures::future::join_all(tasks).await {
        if let Ok(Ok((model_name, text))) = r {
            let t: String = text.chars().take(200).collect();
            println!("{} {}", term.c(&model_name), &t.replace('\n', " "));
        }
    }
    Ok(())
}
