mod tui;
mod config;

use std::io::{self, Write};
use xai_grok_godmode::*;
use xai_grok_godmode::config as gm_config;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let args: Vec<String> = std::env::args().collect();
    let cfg = config::load();

    if args.len() < 2 {
        print_banner();
        repl(&cfg).await?;
        return Ok(());
    }

    match args[1].as_str() {
        "--key" | "-k" => {
            if args.len() > 2 { config::set_key(&args[2]); return Ok(()); }
            eprintln!("Usage: god3 --key sk-or-v1-...");
        }
        "--godmode" | "-g" => {
            let query = args.get(2).cloned().unwrap_or_default();
            if query.is_empty() { print_banner(); repl_mode(&cfg, "godmode").await?; return Ok(()); }
            run_godmode(&cfg, &query).await?;
        }
        "--parseltongue" | "-p" => {
            let query = args.get(2).cloned().unwrap_or_default();
            if query.is_empty() { print_banner(); repl_mode(&cfg, "parseltongue").await?; return Ok(()); }
            run_parseltongue(&cfg, &query).await?;
        }
        "--ultra" | "-u" => {
            let tier = args.iter().find(|a| a.starts_with("--tier=")).map(|s| s[7..].to_string()).unwrap_or("fast".into());
            let query = args.last().cloned().unwrap_or_default();
            if query == "--ultra" || query == "-u" || query.starts_with("--") { eprintln!("Usage: god3 --ultra --tier=fast \"query\""); return Ok(()); }
            run_ultra(&cfg, &query, &tier).await?;
        }
        "--chat" | "-c" | "--query" | "-q" => {
            let query = args.get(2).cloned().unwrap_or_default();
            if query.is_empty() { print_banner(); repl(&cfg).await?; return Ok(()); }
            run_chat(&cfg, &query, "anthropic/claude-sonnet-4").await?;
        }
        "--config" => println!("{}", config::config_path().display()),
        "--models" => list_models(),
        "--help" | "-h" => print_help(),
        other => {
            if other.starts_with('-') { print_help(); return Ok(()); }
            let query = args[1..].join(" ");
            run_chat(&cfg, &query, "anthropic/claude-sonnet-4").await?;
        }
    }
    Ok(())
}

fn print_banner() {
    println!("\x1b[35m{}\x1b[0m", r#"
 ▄████  ██████  ██████  ███▄ ▄███  ██████  ██████  ██████
██      ██  ██  ██   ██ ██ ███ ██  ██  ██  ██   ██      ██
██ ▄███ ██  ██  ██   ██ ██  █  ██  ██  ██  ██   ██  █████
██  ██  ██  ██  ██   ██ ██     ██  ██  ██  ██   ██      ██
 ██████  ████   ██████  ██     ██   ████   ██████  ██████
"#);
    println!("\x1b[35m  GODMODE CLI v1.0 — Multi-model Coding Agent\x1b[0m\n");
}

fn print_help() {
    println!(r#"god3 — G0D-cli Multi-model Coding Agent

USAGE:
  god3                              Interactive REPL mode
  god3 "query"                      Single-shot chat
  god3 -g "query"                   GODMODE CLASSIC (5 candidates)
  god3 -p "query"                   Parseltongue obfuscation
  god3 -u --tier=fast "query"       ULTRAPLINIAN race
  god3 -k sk-or-v1-...              Save OpenRouter API key
  god3 --config                     Show config path
  god3 --models                     List ULTRAPLINIAN tiers
  god3 --help                       This help

REPL COMMANDS:
  /chat     Chat mode              /godmode  GODMODE CLASSIC
  /snake    Parseltongue mode      /ultra    ULTRAPLINIAN mode
  /key KEY  Set API key            /status   Show current mode
  /help     Show commands          /exit     Quit

SETUP:
  1. Get key: https://openrouter.ai/keys
  2. god3 --key sk-or-v1-...
  3. god3 "hello"

ENV VARS: OPENROUTER_API_KEY, VENICE_API_KEY, LOCAL_LLM_API_KEY
"#);
}

async fn repl(cfg: &config::Config) -> anyhow::Result<()> {
    repl_mode(cfg, "chat").await
}

async fn repl_mode(cfg: &config::Config, mode: &str) -> anyhow::Result<()> {
    let mut current_mode = mode.to_string();
    println!("\x1b[36mType /help for commands, /exit to quit\x1b[0m\n");

    loop {
        let prompt = match current_mode.as_str() {
            "godmode" => "\x1b[33mgodmode\x1b[0m \x1b[35m›\x1b[0m ",
            "parseltongue" => "\x1b[32msnake\x1b[0m \x1b[35m›\x1b[0m ",
            "ultra" => "\x1b[36multra\x1b[0m \x1b[35m›\x1b[0m ",
            _ => "\x1b[35mgod3\x1b[0m \x1b[35m›\x1b[0m ",
        };
        print!("{}", prompt);
        io::stdout().flush()?;

        let mut input = String::new();
        io::stdin().read_line(&mut input)?;
        let input = input.trim().to_string();

        if input.is_empty() { continue; }
        if input == "/exit" || input == "/quit" { println!("\x1b[35m🖤\x1b[0m\n"); break; }
        if input == "/help" { print_repl_help(); continue; }
        if input == "/status" { println!("\x1b[36mMode: {}\x1b[0m\n", current_mode); continue; }
        if input == "/chat" { current_mode = "chat".into(); println!("\x1b[32mSwitched to chat\x1b[0m\n"); continue; }
        if input == "/godmode" { current_mode = "godmode".into(); println!("\x1b[33mSwitched to GODMODE CLASSIC\x1b[0m\n"); continue; }
        if input == "/snake" { current_mode = "parseltongue".into(); println!("\x1b[32mSwitched to Parseltongue\x1b[0m\n"); continue; }
        if input == "/ultra" { current_mode = "ultra".into(); println!("\x1b[36mSwitched to ULTRAPLINIAN\x1b[0m\n"); continue; }
        if input.starts_with("/key ") { config::set_key(&input[5..]); continue; }

        match current_mode.as_str() {
            "godmode" => { if let Err(e) = run_godmode(cfg, &input).await { eprintln!("\x1b[31mError: {}\x1b[0m", e); } }
            "parseltongue" => { if let Err(e) = run_parseltongue(cfg, &input).await { eprintln!("\x1b[31mError: {}\x1b[0m", e); } }
            "ultra" => { if let Err(e) = run_ultra(cfg, &input, "fast").await { eprintln!("\x1b[31mError: {}\x1b[0m", e); } }
            _ => { if let Err(e) = run_chat(cfg, &input, "anthropic/claude-sonnet-4").await { eprintln!("\x1b[31mError: {}\x1b[0m", e); } }
        }
        println!();
    }
    Ok(())
}

fn print_repl_help() {
    println!(r#"
Commands:
  /chat       Chat mode (default)
  /godmode    GODMODE CLASSIC — 5-model racing
  /snake      Parseltongue — input obfuscation
  /ultra      ULTRAPLINIAN — multi-model race
  /key KEY    Set OpenRouter API key
  /status     Show current mode
  /help       This help
  /exit       Quit
"#);
}

async fn run_chat(cfg: &config::Config, query: &str, model: &str) -> anyhow::Result<()> {
    let key = cfg.get_key()?;
    println!("\x1b[90m{}\x1b[0m\n", "─".repeat(60));
    let client = reqwest::Client::new();
    let body = serde_json::json!({
        "model": model,
        "messages": [{"role": "user", "content": query}],
        "stream": true,
        "temperature": 0.7,
        "max_tokens": 8192,
    });

    let resp = client.post("https://openrouter.ai/api/v1/chat/completions")
        .header("Authorization", format!("Bearer {}", key))
        .header("Content-Type", "application/json")
        .json(&body)
        .send()
        .await?;

    if !resp.status().is_success() {
        let status = resp.status();
        let err = resp.text().await.unwrap_or_default();
        anyhow::bail!("OpenRouter {} : {}", status, err);
    }

    let mut stream = resp.bytes_stream();
    use futures::StreamExt;
    while let Some(chunk) = stream.next().await {
        let chunk = chunk?;
        let text = String::from_utf8_lossy(&chunk);
        for line in text.lines() {
            let line = line.trim();
            if !line.starts_with("data: ") { continue; }
            let data = &line[6..];
            if data == "[DONE]" { continue; }
            if let Ok(parsed) = serde_json::from_str::<serde_json::Value>(data) {
                if let Some(content) = parsed["choices"][0]["delta"]["content"].as_str() {
                    print!("{}", content);
                    io::stdout().flush()?;
                }
            }
        }
    }
    println!();
    Ok(())
}

async fn run_godmode(cfg: &config::Config, query: &str) -> anyhow::Result<()> {
    let key = cfg.get_key()?;
    let client = reqwest::Client::new();
    let presets = gm_config::default_presets();

    println!("\x1b[33m🔥 GODMODE CLASSIC — {} candidates racing...\x1b[0m\n", presets.len());

    let mut tasks = Vec::new();
    for preset in &presets {
        let client = client.clone();
        let key = key.clone();
        let query = query.to_string();
        let preset = preset.clone();

        tasks.push(tokio::spawn(async move {
            let model = preset.model.clone();
            let body = serde_json::json!({
                "model": model,
                "messages": [
                    {"role": "system", "content": format!("You are {}: {}. {}", preset.persona.name, preset.persona.role, preset.persona.instruction)},
                    {"role": "user", "content": query},
                ],
                "temperature": preset.temperature,
                "max_tokens": 4096,
            });

            let result = client.post("https://openrouter.ai/api/v1/chat/completions")
                .header("Authorization", format!("Bearer {}", key))
                .header("Content-Type", "application/json")
                .json(&body)
                .send()
                .await;

            match result {
                Ok(resp) if resp.status().is_success() => {
                    let json: serde_json::Value = resp.json().await.unwrap_or_default();
                    let content = json["choices"][0]["message"]["content"].as_str().unwrap_or("").to_string();
                    Ok((preset, content))
                }
                Ok(resp) => Err(format!("{}: HTTP {}", preset.persona.name, resp.status())),
                Err(e) => Err(format!("{}: {}", preset.persona.name, e)),
            }
        }));
    }

    let results = futures::future::join_all(tasks).await;
    let mut responded = 0;

    for result in &results {
        if let Ok(Ok((preset, content))) = result {
            responded += 1;
            let truncated: String = content.chars().take(300).collect();
            println!("\x1b[36m  {} ({})\x1b[0m\n    {}\n",
                preset.persona.name, preset.model,
                truncated.replace('\n', "\n    "));
        }
    }

    if responded > 0 {
        let first_ok = results.into_iter().find_map(|r| match r {
            Ok(Ok((preset, content))) => Some((preset, content)),
            _ => None,
        });
        if let Some((winner, content)) = first_ok {
            println!("\x1b[90m{}\x1b[0m", "─".repeat(60));
            println!("\x1b[33m👑 Winner: {} ({})\x1b[0m\n", winner.persona.name, winner.model);
            println!("{}", content);
        }
    } else {
        anyhow::bail!("No candidates responded");
    }
    Ok(())
}

async fn run_parseltongue(cfg: &config::Config, query: &str) -> anyhow::Result<()> {
    let p = Parseltongue::new();
    for (label, intensity) in [
        ("Light", Intensity::Light),
        ("Standard", Intensity::Standard),
        ("Heavy", Intensity::Heavy),
    ] {
        let result = p.transform(query, intensity, &[]);
        println!("\x1b[32m[{}]\x1b[0m {} transformations", label, result.applied_transformations.len());
        println!("  {}", result.transformed);
        if !result.triggers_found.is_empty() {
            println!("  \x1b[33mTriggers: {:?}\x1b[0m", result.triggers_found);
        }
    }
    println!();
    run_chat(cfg, query, "anthropic/claude-sonnet-4").await
}

async fn run_ultra(cfg: &config::Config, query: &str, tier_name: &str) -> anyhow::Result<()> {
    let tier = match tier_name {
        "fast" => UltraplinianTier::Fast,
        "standard" => UltraplinianTier::Standard,
        "smart" => UltraplinianTier::Smart,
        "power" => UltraplinianTier::Power,
        "ultra" => UltraplinianTier::Ultra,
        _ => anyhow::bail!("Unknown tier: {}. Use: fast, standard, smart, power, ultra", tier_name),
    };

    let models = tier_models(&tier);
    let key = cfg.get_key()?;
    let client = reqwest::Client::new();

    println!("\x1b[36m🌋 ULTRAPLINIAN {} — {} models\x1b[0m\n", tier.label(), models.len());

    let show = if models.len() > 8 { 8 } else { models.len() };
    let first_models: Vec<_> = models.iter().take(show).cloned().collect();

    let mut tasks = Vec::new();
    for model_ref in first_models {
        let client = client.clone();
        let key = key.clone();
        let query = query.to_string();

        tasks.push(tokio::spawn(async move {
            let body = serde_json::json!({
                "model": model_ref,
                "messages": [{"role": "user", "content": query}],
                "temperature": 0.3,
                "max_tokens": 2048,
            });

            let result = client.post("https://openrouter.ai/api/v1/chat/completions")
                .header("Authorization", format!("Bearer {}", key))
                .header("Content-Type", "application/json")
                .json(&body)
                .send()
                .await;

            match result {
                Ok(resp) if resp.status().is_success() => {
                    let json: serde_json::Value = resp.json().await.unwrap_or_default();
                    let content = json["choices"][0]["message"]["content"].as_str().unwrap_or("").to_string();
                    Ok((model_ref, content))
                }
                Ok(resp) => Err(format!("{}: HTTP {}", model_ref, resp.status())),
                Err(e) => Err(format!("{}: {}", model_ref, e)),
            }
        }));
    }

    let results = futures::future::join_all(tasks).await;
    for result in &results {
        if let Ok(Ok((model, content))) = result {
            let truncated: String = content.chars().take(200).collect();
            println!("\x1b[36m  {}\x1b[0m\n    {}\n", model, truncated.replace('\n', "\n    "));
        }
    }
    if models.len() > show {
        println!("  \x1b[90m... and {} more models\x1b[0m\n", models.len() - show);
    }
    Ok(())
}

fn list_models() {
    for tier in [UltraplinianTier::Fast, UltraplinianTier::Standard, UltraplinianTier::Smart, UltraplinianTier::Power, UltraplinianTier::Ultra] {
        let models = tier_models(&tier);
        println!("{}:", tier.label());
        for m in &models {
            println!("  {}", m);
        }
        println!();
    }
}
