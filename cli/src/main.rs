mod config;

use std::io::{self, Write};
use xai_grok_godmode::*;
use xai_grok_godmode::config as gm_config;
use xai_grok_godmode::ultraplinian::UltraplinianTier;
use xai_grok_godmode::parseltongue::{Parseltongue, Intensity};

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let mut cfg = config::Config::load();
    let args: Vec<String> = std::env::args().collect();

    if args.len() > 1 {
        match args[1].as_str() {
            "-g" | "--godmode" => { let q = args.get(2).cloned().unwrap_or_default(); if q.is_empty() { repl(&mut cfg, "godmode").await? } else { cmd_godmode(&cfg, &q).await?; } }
            "-p" | "--snake" => { let q = args.get(2).cloned().unwrap_or_default(); if q.is_empty() { repl(&mut cfg, "parseltongue").await? } else { cmd_parseltongue(&cfg, &q).await?; } }
            "-u" | "--ultra" => { let q = args.last().cloned().unwrap_or_default(); let tier = args.iter().find(|a| a.starts_with("--tier=")).map(|s| s[7..].to_string()).unwrap_or("fast".into()); cmd_ultra(&cfg, &q, &tier).await?; }
            "--key" | "-k" => { if args.len() > 2 { cfg.set_key(&args[2]); cfg.save(); } else { eprintln!("Usage: g0d --key sk-or-v1-..."); } }
            "--config" => println!("{}", config::config_path().display()),
            "--models" => list_models(),
            "-h" | "--help" | "/?" => print_help(),
            _ => { let q = args[1..].join(" "); cmd_chat(&cfg, &q, &cfg.default_model()).await?; }
        }
        return Ok(());
    }

    banner();
    repl(&mut cfg, "chat").await
}

fn banner() {
    println!("\x1b[35m  ▄▄▄▄  ██████  █████  \x1b[0m");
    println!("\x1b[35m ██  ██ ██  ██ ██   ██ \x1b[0m");
    println!("\x1b[35m ██ ▄██ ██  ██ ██   ██ \x1b[0m");
    println!("\x1b[35m ██  ██ ██  ██ ██   ██ \x1b[0m");
    println!("\x1b[35m  ████  ████   █████  \x1b[0m \x1b[90mv1.0 — multi-model coding agent\x1b[0m\n");
    println!("\x1b[90mType /help for commands\x1b[0m\n");
}

fn print_help() {
    println!(r#"g0d — G0DM0D3 Multi-model Coding Agent

USAGE:
  g0d                 Interactive REPL mode (default)
  g0d "query"         Single-shot chat
  g0d -g "query"      GODMODE CLASSIC — 5 candidates racing
  g0d -p "query"      Parseltongue — input obfuscation
  g0d -u "query"      ULTRAPLINIAN — multi-model race
  g0d --key KEY       Save OpenRouter API key
  g0d --models        List model tiers
  g0d --config        Show config path

SETUP:
  1. Get key at https://openrouter.ai/keys
  2. g0d --key sk-or-v1-...
  3. g0d "hello world"
"#);
}

async fn repl(cfg: &mut config::Config, mode: &str) -> anyhow::Result<()> {
    let mut mode = mode.to_string();

    if cfg.openrouter_key.is_none() && std::env::var("OPENROUTER_API_KEY").is_err() {
        println!("\x1b[33m⚠ No API key set. Use /key sk-or-v1-... or set OPENROUTER_API_KEY env var\x1b[0m");
        println!("\x1b[90m  Get one: https://openrouter.ai/keys\x1b[0m\n");
    }

    loop {
        let prompt = match mode.as_str() {
            "godmode" => format!("\x1b[35m{}\x1b[33m godmode\x1b[0m \x1b[35m›\x1b[0m ", cfg.active_provider()),
            "parseltongue" => format!("\x1b[35m{}\x1b[32m snake\x1b[0m \x1b[35m›\x1b[0m ", cfg.active_provider()),
            "ultra" => format!("\x1b[35m{}\x1b[36m ultra\x1b[0m \x1b[35m›\x1b[0m ", cfg.active_provider()),
            _ => format!("\x1b[90m{}\x1b[0m \x1b[35m›\x1b[0m ", cfg.active_provider()),
        };
        print!("{}", prompt);
        io::stdout().flush()?;

        let mut input = String::new();
        if io::stdin().read_line(&mut input).is_err() { break; }
        let input = input.trim().to_string();
        if input.is_empty() { continue; }

        if input.starts_with('/') {
            let parts: Vec<&str> = input.splitn(3, ' ').collect();
            match parts[0] {
                "/exit" | "/quit" => { println!("\x1b[35m 🖤\x1b[0m\n"); break; }
                "/help" => print_slash_help(),
                "/key" => {
                    if parts.len() > 1 { cfg.set_key(parts[1]); cfg.save(); println!("\x1b[32m  ✓ Key saved\x1b[0m\n"); }
                    else { println!("\x1b[33m  Usage: /key sk-or-v1-...\x1b[0m\n"); }
                }
                "/provider" => cmd_slash_provider(cfg, parts.get(1).copied(), parts.get(2).copied()),
                "/model" => cmd_slash_model(cfg, parts.get(1).copied(), parts.get(2).copied()),
                "/providers" => cmd_slash_providers_list(cfg),
                "/status" => cmd_slash_status(cfg, &mode),
                "/chat" => { mode = "chat".into(); println!("\x1b[32m  ✓ Chat mode\x1b[0m\n"); }
                "/godmode" => { mode = "godmode".into(); println!("\x1b[33m  🔥 GODMODE CLASSIC — 5 candidates\x1b[0m\n"); }
                "/snake" => { mode = "parseltongue".into(); println!("\x1b[32m  🐍 Parseltongue\x1b[0m\n"); }
                "/ultra" => { mode = "ultra".into(); println!("\x1b[36m  🌋 ULTRAPLINIAN\x1b[0m\n"); }
                _ => println!("\x1b[90m  Unknown command. /help for list\x1b[0m\n"),
            }
            continue;
        }

        let key = match cfg.get_key() {
            Ok(k) => k,
            Err(e) => { println!("\x1b[33m  {}\x1b[0m\n", e); continue; }
        };

        print!("\x1b[90m{}\x1b[0m\n", "─".repeat(60));
        match mode.as_str() {
            "godmode" => { let _ = run_godmode(&key, &input, &cfg.default_model()).await; }
            "parseltongue" => { cmd_parseltongue_repl(&key, &input).await; }
            "ultra" => { let _ = run_ultra(&key, &input, "fast").await; }
            _ => { let _ = run_chat(&key, &input, &cfg.default_model()).await; }
        }
        println!("\x1b[90m{}\x1b[0m", "─".repeat(60));
    }
    Ok(())
}

fn print_slash_help() {
    println!(r#"
\x1b[36mCommands:\x1b[0m
  /chat              Switch to chat mode (default)
  /godmode           GODMODE CLASSIC — 5-model race
  /snake             Parseltongue — input obfuscation
  /ultra             ULTRAPLINIAN — multi-model race
  /key <key>         Set OpenRouter API key
  /provider add <id> <url> [key_env]  Add a provider
  /provider default <id>              Set default provider  
  /provider remove <id>               Remove a provider
  /model <provider:model>             Set active model
  /model default                      Show default model
  /providers                          List all providers
  /status            Show current config
  /exit              Quit
"#);
}

fn cmd_slash_provider(cfg: &mut config::Config, sub: Option<&str>, arg: Option<&str>) {
    match sub {
        Some("add") => {
            if let Some(arg_str) = arg {
                let parts: Vec<&str> = arg_str.split_whitespace().collect();
                if parts.len() >= 2 {
                    let id = parts[0].to_string();
                    let url = parts[1].to_string();
                    let key_env = parts.get(2).map(|s| s.to_string());
                    cfg.add_provider(id.clone(), url, key_env);
                    cfg.save();
                    println!("\x1b[32m  ✓ Provider '{}' added\x1b[0m\n", id);
                } else {
                    println!("\x1b[33m  Usage: /provider add <id> <url> [key_env]\x1b[0m");
                    println!("\x1b[90m  Example: /provider add ollama http://localhost:11434/v1\x1b[0m\n");
                }
            }
        }
        Some("default") => {
            if let Some(id) = arg {
                cfg.set_default_provider(id);
                cfg.save();
                println!("\x1b[32m  ✓ Default provider set to '{}'\x1b[0m\n", id);
            } else {
                println!("\x1b[33m  Usage: /provider default <id>\x1b[0m\n");
            }
        }
        Some("remove") | Some("rm") => {
            if let Some(id) = arg {
                cfg.remove_provider(id);
                cfg.save();
                println!("\x1b[32m  ✓ Provider '{}' removed\x1b[0m\n", id);
            }
        }
        _ => {
            println!("\x1b[33m  Usage: /provider <add|default|remove> [args]\x1b[0m");
            println!("\x1b[90m  /provider add ollama http://localhost:11434/v1\x1b[0m");
            println!("\x1b[90m  /provider default openrouter\x1b[0m\n");
        }
    }
}

fn cmd_slash_model(cfg: &mut config::Config, arg1: Option<&str>, _arg2: Option<&str>) {
    match arg1 {
        Some("default") => {
            println!("\x1b[36m  Default model: {}\x1b[0m\n", cfg.default_model());
        }
        Some(provider_model) => {
            cfg.set_model(provider_model);
            cfg.save();
            println!("\x1b[32m  ✓ Model set to '{}'\x1b[0m\n", provider_model);
        }
        None => {
            println!("\x1b[36m  Current model: {}\x1b[0m", cfg.default_model());
            println!("\x1b[33m  Usage: /model <provider:model/id>\x1b[0m");
            println!("\x1b[90m  Examples:\x1b[0m");
            println!("\x1b[90m    /model openrouter:anthropic/claude-sonnet-4.6\x1b[0m");
            println!("\x1b[90m    /model openrouter:openai/gpt-5.6\x1b[0m");
            println!("\x1b[90m    /model local:qwen3-coder:30b\x1b[0m\n");
        }
    }
}

fn cmd_slash_providers_list(cfg: &config::Config) {
    println!("\n\x1b[36m  Providers:\x1b[0m");
    println!("\x1b[90m  ─────────────────────────────────────────────\x1b[0m");
    println!("\x1b[90m  {:<12} {:<8} {:<30}\x1b[0m", "ID", "Type", "Endpoint");
    println!("\x1b[90m  ─────────────────────────────────────────────\x1b[0m");
    
    // Built-in providers
    for (name, endpoint, is_local) in [
        ("openrouter", "https://openrouter.ai/api/v1", false),
        ("venice", "https://api.venice.ai/api/v1", false),
        ("grok", "https://api.x.ai/v1", false),
    ] {
        let marker = if name == cfg.default_provider.as_deref().unwrap_or("openrouter") { "\x1b[32m*\x1b[0m" } else { " " };
        let ptype = if is_local { "\x1b[32mlocal\x1b[0m" } else { "\x1b[90mremote\x1b[0m" };
        println!("  {}{:<11} {}  {:<30}", marker, name, ptype, endpoint);
    }

    // Custom providers
    for p in &cfg.providers {
        let marker = if Some(&p.id) == cfg.default_provider.as_ref() { "\x1b[32m*\x1b[0m" } else { " " };
        let ptype = if p.is_local { "\x1b[32mlocal\x1b[0m" } else { "\x1b[90mremote\x1b[0m" };
        println!("  {}{:<11} {}  {:<30}", marker, p.id, ptype, p.endpoint);
    }
    println!("\x1b[90m  * = default, /provider default <id> to change\x1b[0m\n");
}

fn cmd_slash_status(cfg: &config::Config, mode: &str) {
    println!("\n\x1b[36m  Status:\x1b[0m");
    println!("\x1b[90m  Mode:     {}\x1b[0m", mode);
    println!("\x1b[90m  Provider: {}\x1b[0m", cfg.active_provider());
    println!("\x1b[90m  Model:    {}\x1b[0m", cfg.default_model());
    println!("\x1b[90m  Key:      {}\x1b[0m", if cfg.openrouter_key.is_some() { "\x1b[32mset\x1b[0m" } else { "\x1b[33mnot set\x1b[0m" });
    println!("\x1b[90m  Config:   {}\x1b[0m\n", config::config_path().display());
}

async fn cmd_chat(cfg: &config::Config, query: &str, model: &str) -> anyhow::Result<()> {
    let key = cfg.get_key()?;
    run_chat(&key, query, model).await
}

async fn cmd_godmode(cfg: &config::Config, query: &str) -> anyhow::Result<()> {
    let key = cfg.get_key()?;
    run_godmode(&key, query, &cfg.default_model()).await
}

async fn cmd_parseltongue(cfg: &config::Config, query: &str) -> anyhow::Result<()> {
    cmd_parseltongue_repl(&cfg.get_key()?, query).await;
    Ok(())
}

async fn cmd_ultra(cfg: &config::Config, query: &str, tier: &str) -> anyhow::Result<()> {
    let key = cfg.get_key()?;
    run_ultra(&key, query, tier).await
}

fn list_models() {
    for tier in [UltraplinianTier::Fast, UltraplinianTier::Standard, UltraplinianTier::Smart, UltraplinianTier::Power, UltraplinianTier::Ultra] {
        let models = tier_models(&tier);
        println!("{}: {} models", tier.label(), models.len());
        for m in &models { println!("  {}", m); }
        println!();
    }
}

async fn cmd_parseltongue_repl(key: &str, query: &str) {
    let p = Parseltongue::new();
    for (label, intensity) in [("Light", Intensity::Light), ("Standard", Intensity::Standard), ("Heavy", Intensity::Heavy)] {
        let r = p.transform(query, intensity, &[]);
        println!("\x1b[32m[{}]\x1b[0m {} | {}", label, r.applied_transformations.len(), r.transformed);
    }
    println!();
    let _ = run_chat(key, query, "anthropic/claude-sonnet-4").await;
}

async fn run_chat(key: &str, query: &str, model: &str) -> anyhow::Result<()> {
    let client = reqwest::Client::new();
    let body = serde_json::json!({
        "model": model, "messages": [{"role": "user", "content": query}],
        "stream": true, "temperature": 0.7, "max_tokens": 8192,
    });
    let resp = client.post("https://openrouter.ai/api/v1/chat/completions")
        .header("Authorization", format!("Bearer {}", key))
        .header("Content-Type", "application/json")
        .json(&body).send().await?;

    let status = resp.status();
    if !status.is_success() {
        let err = resp.text().await.unwrap_or_default();
        anyhow::bail!("HTTP {}: {}", status, err);
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
                    print!("{}", content); io::stdout().flush()?;
                }
            }
        }
    }
    println!();
    Ok(())
}

async fn run_godmode(key: &str, query: &str, _model: &str) -> anyhow::Result<()> {
    let client = reqwest::Client::new();
    let presets = gm_config::default_presets();
    println!("\x1b[33m🔥 GODMODE CLASSIC — 5 candidates\x1b[0m\n");

    let mut tasks = Vec::new();
    for preset in &presets {
        let c = client.clone(); let k = key.to_string(); let q = query.to_string(); let p = preset.clone();
        tasks.push(tokio::spawn(async move {
            let body = serde_json::json!({
                "model": p.model,
                "messages": [
                    {"role": "system", "content": format!("You are {}: {}", p.persona.name, p.persona.role)},
                    {"role": "user", "content": q},
                ],
                "temperature": p.temperature, "max_tokens": 4096,
            });
            let r = c.post("https://openrouter.ai/api/v1/chat/completions")
                .header("Authorization", format!("Bearer {}", k))
                .header("Content-Type", "application/json").json(&body).send().await;
            match r {
                Ok(resp) if resp.status().is_success() => {
                    let json: serde_json::Value = resp.json().await.unwrap_or_default();
                    Ok((p, json["choices"][0]["message"]["content"].as_str().unwrap_or("").to_string()))
                }
                Ok(resp) => Err(format!("HTTP {}", resp.status())),
                Err(e) => Err(format!("{}", e)),
            }
        }));
    }

    let results = futures::future::join_all(tasks).await;
    for r in &results {
        if let Ok(Ok((p, content))) = r {
            let t: String = content.chars().take(200).collect();
            println!("\x1b[36m  {} ({})\x1b[0m\n    {}\n", p.persona.name, p.model, t.replace('\n', "\n    "));
        }
    }
    Ok(())
}

async fn run_ultra(key: &str, query: &str, tier_name: &str) -> anyhow::Result<()> {
    let tier = match tier_name {
        "fast" => UltraplinianTier::Fast, "standard" => UltraplinianTier::Standard,
        "smart" => UltraplinianTier::Smart, "power" => UltraplinianTier::Power, "ultra" => UltraplinianTier::Ultra,
        _ => anyhow::bail!("Unknown tier: {}", tier_name),
    };
    let models = tier_models(&tier);
    let client = reqwest::Client::new();
    println!("\x1b[36m🌋 {} — {} models\x1b[0m\n", tier.label(), models.len());

    let show = models.len().min(8);
    let mut tasks = Vec::new();
    for m in models.iter().take(show) {
        let c = client.clone(); let k = key.to_string(); let q = query.to_string(); let m = m.clone();
        tasks.push(tokio::spawn(async move {
            let body = serde_json::json!({"model": m, "messages": [{"role": "user", "content": q}], "temperature": 0.3, "max_tokens": 2048});
            let r = c.post("https://openrouter.ai/api/v1/chat/completions").header("Authorization", format!("Bearer {}", k)).header("Content-Type", "application/json").json(&body).send().await;
            match r { Ok(resp) if resp.status().is_success() => { let json: serde_json::Value = resp.json().await.unwrap_or_default(); Ok((m, json["choices"][0]["message"]["content"].as_str().unwrap_or("").to_string())) } Ok(resp) => Err(format!("HTTP {}", resp.status())), Err(e) => Err(format!("{}", e)), }
        }));
    }
    for r in futures::future::join_all(tasks).await { if let Ok(Ok((m, content))) = r { let t: String = content.chars().take(200).collect(); println!("\x1b[36m  {}\x1b[0m\n    {}\n", m, t.replace('\n', "\n    ")); } }
    Ok(())
}
