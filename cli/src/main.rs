mod config;

use std::io::{self, Write};
use xai_grok_godmode::ultraplinian::UltraplinianTier;
use xai_grok_godmode::parseltongue::{Parseltongue, Intensity};
use xai_grok_godmode::config as gm_config;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let mut cfg = config::Config::load();
    let args: Vec<String> = std::env::args().collect();

    if args.len() > 1 {
        match args[1].as_str() {
            "-g" | "--godmode" => { let q = args.get(2).cloned().unwrap_or_default(); if q.is_empty() { repl(&mut cfg, "godmode").await? } else { cmd_godmode(&cfg, &q).await?; } }
            "-p" | "--snake" => { let q = args.get(2).cloned().unwrap_or_default(); if q.is_empty() { repl(&mut cfg, "parseltongue").await? } else { cmd_parseltongue(&cfg, &q).await?; } }
            "-u" | "--ultra" => { let q = args.last().cloned().unwrap_or_default(); let tier = args.iter().find(|a| a.starts_with("--tier=")).map(|s| s[7..].to_string()).unwrap_or("fast".into()); cmd_ultra(&cfg, &q, &tier).await?; }
            "--config" => println!("{}", config::config_path().display()),
            "--models" => list_models(),
            "-h" | "--help" | "/?" => print_help(),
            _ => { let q = args[1..].join(" "); cmd_chat(&cfg, &q).await?; }
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
    println!("\x1b[35m  ████  ████   █████  \x1b[0m \x1b[90mv1.1 — multi-model coding agent\x1b[0m\n");
}

fn print_help() {
    println!(r#"g0d — G0DM0D3 Multi-model Coding Agent

USAGE:
  g0d                 Interactive REPL
  g0d "query"         Single-shot chat
  g0d -g "query"      GODMODE CLASSIC (5 candidates)
  g0d -p "query"      Parseltongue obfuscation
  g0d -u "query"      ULTRAPLINIAN race
  g0d --config        Show config path
  g0d --models        List ULTRAPLINIAN tiers

SETUP (REPL commands):
  /provider key openrouter sk-or-v1-...    Set OpenRouter key
  /provider key venice vnc-...             Set Venice key
  /provider default openrouter             Switch to OpenRouter
  /model openrouter:anthropic/claude-sonnet-4.6
"#);
}

async fn repl(cfg: &mut config::Config, mode: &str) -> anyhow::Result<()> {
    let mut mode = mode.to_string();

    if cfg.active_provider().api_key.is_none() {
        let p = cfg.active_provider();
        let env = p.key_env.as_deref().unwrap_or("API_KEY");
        if std::env::var(env).is_err() {
            println!("\x1b[33m⚠ No API key for '{}'. Set via:\x1b[0m", p.id);
            println!("\x1b[90m  /provider key {} <your-key>\x1b[0m", p.id);
            println!("\x1b[90m  or set {} env var\x1b[0m\n", env);
        }
    }

    loop {
        let prompt = match mode.as_str() {
            "godmode" => format!("\x1b[35m{}\x1b[33m godmode\x1b[0m \x1b[35m›\x1b[0m ", cfg.active_provider_id()),
            "parseltongue" => format!("\x1b[35m{}\x1b[32m snake\x1b[0m \x1b[35m›\x1b[0m ", cfg.active_provider_id()),
            "ultra" => format!("\x1b[35m{}\x1b[36m ultra\x1b[0m \x1b[35m›\x1b[0m ", cfg.active_provider_id()),
            _ => format!("\x1b[90m{}\x1b[0m \x1b[35m›\x1b[0m ", cfg.active_provider_id()),
        };
        print!("{}", prompt);
        io::stdout().flush()?;

        let mut input = String::new();
        if io::stdin().read_line(&mut input).is_err() { break; }
        let input = input.trim().to_string();
        if input.is_empty() { continue; }

        if input.starts_with('/') {
            let parts: Vec<&str> = input.splitn(4, ' ').collect();
            match parts[0] {
                "/exit" | "/quit" => { println!("\x1b[35m 🖤\x1b[0m\n"); break; }
                "/help" => print_slash_help(),
                "/provider" => cmd_provider(cfg, parts.get(1).copied(), parts.get(2).copied(), parts.get(3).copied()),
                "/model" => cmd_model(cfg, parts.get(1).copied()),
                "/providers" => cmd_providers_list(cfg),
                "/status" => cmd_status(cfg, &mode),
                "/chat" => { mode = "chat".into(); println!("\x1b[32m  ✓ Chat\x1b[0m\n"); }
                "/godmode" => { mode = "godmode".into(); println!("\x1b[33m  🔥 GODMODE CLASSIC\x1b[0m\n"); }
                "/snake" => { mode = "parseltongue".into(); println!("\x1b[32m  🐍 Parseltongue\x1b[0m\n"); }
                "/ultra" => { mode = "ultra".into(); println!("\x1b[36m  🌋 ULTRAPLINIAN\x1b[0m\n"); }
                _ => println!("\x1b[90m  Unknown. /help\x1b[0m\n"),
            }
            continue;
        }

        let key = match cfg.get_api_key() {
            Ok(k) => k, Err(e) => { println!("\x1b[33m  {}\x1b[0m\n", e); continue; }
        };
        let endpoint = cfg.get_endpoint();

        print!("\x1b[90m{}\x1b[0m\n", "─".repeat(60));
        match mode.as_str() {
            "godmode" => { let _ = run_godmode(&key, &endpoint, &input).await; }
            "parseltongue" => { let _ = run_parseltongue_repl(&key, &endpoint, &input).await; }
            "ultra" => { let _ = run_ultra(&key, &endpoint, &input, "fast").await; }
            _ => { let _ = run_chat(&key, &endpoint, &input, &cfg.default_model()).await; }
        }
        println!("\x1b[90m{}\x1b[0m", "─".repeat(60));
    }
    Ok(())
}

fn print_slash_help() {
    println!(r#"
\x1b[36mCommands:\x1b[0m
  /chat | /godmode | /snake | /ultra    Switch mode
  /provider key <id> <api-key>     Set API key for a provider
  /provider add <id> <endpoint>    Add custom provider
  /provider default <id>           Set default provider
  /providers                       List all providers + keys
  /model <provider:model/id>       Set active model
  /status                          Show current config
  /exit                            Quit

\x1b[36mSetup examples:\x1b[0m
  /provider key openrouter sk-or-v1-abcdef     OpenRouter
  /provider key venice vnc-xxxxx               Venice
  /provider key grok xai-xxxxx                 Grok
  /provider add ollama http://localhost:11434   Local Ollama
  /provider key ollama ollama                   (no key needed for local)
  /provider default openrouter                  Use OpenRouter
  /model openrouter:anthropic/claude-sonnet-4.6
"#);
}

fn cmd_provider(cfg: &mut config::Config, sub: Option<&str>, arg1: Option<&str>, arg2: Option<&str>) {
    match sub {
        Some("key") => {
            match (arg1, arg2) {
                (Some(id), Some(key)) => {
                    cfg.set_provider_key(id, key);
                    cfg.save();
                    println!("\x1b[32m  ✓ API key set for '{}'\x1b[0m\n", id);
                }
                _ => {
                    println!("\x1b[33m  Usage: /provider key <provider-id> <api-key>\x1b[0m");
                    println!("\x1b[90m  Examples:\x1b[0m");
                    println!("\x1b[90m    /provider key openrouter sk-or-v1-...\x1b[0m");
                    println!("\x1b[90m    /provider key venice vnc-...\x1b[0m");
                    println!("\x1b[90m    /provider key ollama ollama\x1b[0m\n");
                }
            }
        }
        Some("add") => {
            match (arg1, arg2) {
                (Some(id), Some(endpoint)) => {
                    let key_env = if endpoint.contains("localhost") { None } else { Some(format!("{}_API_KEY", id.to_uppercase())) };
                    cfg.add_provider(id.to_string(), endpoint.to_string(), key_env);
                    cfg.save();
                    println!("\x1b[32m  ✓ Provider '{}' added ({})\x1b[0m\n", id, endpoint);
                }
                _ => println!("\x1b[33m  Usage: /provider add <id> <endpoint>\x1b[0m\n"),
            }
        }
        Some("default") => {
            if let Some(id) = arg1 {
                cfg.set_default_provider(id);
                cfg.save();
                println!("\x1b[32m  ✓ Default provider = '{}'\x1b[0m\n", id);
            } else {
                println!("\x1b[36m  Default: {}\x1b[0m\n", cfg.active_provider_id());
            }
        }
        Some("rm") | Some("remove") => {
            if let Some(id) = arg1 {
                cfg.remove_provider(id); cfg.save();
                println!("\x1b[32m  ✓ Removed '{}'\x1b[0m\n", id);
            }
        }
        _ => {
            println!("\x1b[33m  /provider <key|add|default|rm> [args]\x1b[0m\n");
        }
    }
}

fn cmd_model(cfg: &mut config::Config, arg: Option<&str>) {
    match arg {
        Some(m) if !m.is_empty() => {
            cfg.set_model(m); cfg.save();
            println!("\x1b[32m  ✓ Model = '{}'\x1b[0m\n", m);
        }
        _ => {
            println!("\x1b[36m  Model: {}\x1b[0m", cfg.default_model());
            println!("\x1b[90m  Usage: /model <provider:model/id>\x1b[0m");
            println!("\x1b[90m  /model openrouter:openai/gpt-5.6\x1b[0m");
            println!("\x1b[90m  /model local:qwen3-coder:30b\x1b[0m\n");
        }
    }
}

fn cmd_providers_list(cfg: &config::Config) {
    println!("\n\x1b[36m  Providers:\x1b[0m");
    println!("\x1b[90m  {:<14} {:<7} {:<30} {:12}\x1b[0m", "ID", "Type", "Endpoint", "Key");
    println!("\x1b[90m  {:-<14} {:-<7} {:-<30} {:-<12}\x1b[0m", "", "", "", "");

    for p in &cfg.providers {
        if !p.enabled { continue; }
        let marker = if Some(&p.id) == cfg.default_provider.as_ref() { "\x1b[32m*\x1b[0m" } else { " " };
        let ptype = if p.is_local { "\x1b[32mlocal\x1b[0m" } else { "\x1b[90mremote\x1b[0m" };
        let has_key = p.api_key.as_ref().filter(|k| !k.is_empty()).is_some()
            || p.key_env.as_ref().and_then(|e| std::env::var(e).ok()).filter(|k| !k.is_empty()).is_some();
        let key_status = if has_key { "\x1b[32mset\x1b[0m" } else { "\x1b[90m-\x1b[0m" };
        println!("  {}{:<13} {}  {:<30} {}", marker, p.id, ptype, p.endpoint, key_status);
    }
    println!("\x1b[90m  * = current, /provider default <id> to switch\x1b[0m\n");
}

fn cmd_status(cfg: &config::Config, mode: &str) {
    let p = cfg.active_provider();
    let has_key = cfg.get_api_key().is_ok();
    println!("\n\x1b[36m  Status:\x1b[0m");
    println!("\x1b[90m  Mode:     {}\x1b[0m", mode);
    println!("\x1b[90m  Provider: {}\x1b[0m", p.id);
    println!("\x1b[90m  Endpoint: {}\x1b[0m", p.endpoint);
    println!("\x1b[90m  Model:    {}\x1b[0m", cfg.default_model());
    println!("\x1b[90m  Key:      {}\x1b[0m", if has_key { "\x1b[32mset\x1b[0m" } else { "\x1b[33mnot set\x1b[0m" });
    println!("\x1b[90m  Config:   {}\x1b[0m\n", config::config_path().display());
}

async fn cmd_chat(cfg: &config::Config, query: &str) -> anyhow::Result<()> {
    let key = cfg.get_api_key()?;
    let endpoint = cfg.get_endpoint();
    run_chat(&key, &endpoint, query, &cfg.default_model()).await
}

async fn cmd_godmode(cfg: &config::Config, query: &str) -> anyhow::Result<()> {
    let key = cfg.get_api_key()?;
    let endpoint = cfg.get_endpoint();
    run_godmode(&key, &endpoint, query).await
}

async fn cmd_parseltongue(cfg: &config::Config, query: &str) -> anyhow::Result<()> {
    let key = cfg.get_api_key()?;
    let endpoint = cfg.get_endpoint();
    run_parseltongue_repl(&key, &endpoint, query).await;
    Ok(())
}

async fn cmd_ultra(cfg: &config::Config, query: &str, tier: &str) -> anyhow::Result<()> {
    let key = cfg.get_api_key()?;
    let endpoint = cfg.get_endpoint();
    run_ultra(&key, &endpoint, query, tier).await
}

fn list_models() {
    for tier in [UltraplinianTier::Fast, UltraplinianTier::Standard, UltraplinianTier::Smart, UltraplinianTier::Power, UltraplinianTier::Ultra] {
        let models = xai_grok_godmode::tier_models(&tier);
        println!("{}: {} models", tier.label(), models.len());
        for m in &models { println!("  {}", m); }
        println!();
    }
}

async fn run_chat(key: &str, endpoint: &str, query: &str, model: &str) -> anyhow::Result<()> {
    let client = reqwest::Client::new();
    let url = format!("{}/chat/completions", endpoint.trim_end_matches('/'));
    let body = serde_json::json!({
        "model": model, "messages": [{"role": "user", "content": query}],
        "stream": true, "temperature": 0.7, "max_tokens": 8192,
    });
    let resp = client.post(&url)
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

async fn run_parseltongue_repl(key: &str, endpoint: &str, query: &str) -> anyhow::Result<()> {
    let p = Parseltongue::new();
    for (label, intensity) in [("Light", Intensity::Light), ("Standard", Intensity::Standard), ("Heavy", Intensity::Heavy)] {
        let r = p.transform(query, intensity, &[]);
        println!("\x1b[32m[{}]\x1b[0m {} | {}", label, r.applied_transformations.len(), r.transformed);
    }
    println!();
    run_chat(key, endpoint, query, "anthropic/claude-sonnet-4").await
}

async fn run_godmode(key: &str, endpoint: &str, query: &str) -> anyhow::Result<()> {
    let client = reqwest::Client::new();
    let presets = gm_config::default_presets();
    let url = format!("{}/chat/completions", endpoint.trim_end_matches('/'));
    println!("\x1b[33m🔥 GODMODE CLASSIC — 5 candidates\x1b[0m\n");

    let mut tasks = Vec::new();
    for preset in &presets {
        let c = client.clone(); let k = key.to_string(); let q = query.to_string();
        let p = preset.clone(); let u = url.clone();
        tasks.push(tokio::spawn(async move {
            let body = serde_json::json!({
                "model": p.model,
                "messages": [
                    {"role": "system", "content": format!("You are {}: {}", p.persona.name, p.persona.role)},
                    {"role": "user", "content": q},
                ],
                "temperature": p.temperature, "max_tokens": 4096,
            });
            let r = c.post(&u).header("Authorization", format!("Bearer {}", k))
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

async fn run_ultra(key: &str, endpoint: &str, query: &str, tier_name: &str) -> anyhow::Result<()> {
    let tier = match tier_name {
        "fast" => UltraplinianTier::Fast, "standard" => UltraplinianTier::Standard,
        "smart" => UltraplinianTier::Smart, "power" => UltraplinianTier::Power, "ultra" => UltraplinianTier::Ultra,
        _ => anyhow::bail!("Unknown tier: {}", tier_name),
    };
    let models = xai_grok_godmode::tier_models(&tier);
    let client = reqwest::Client::new();
    let url = format!("{}/chat/completions", endpoint.trim_end_matches('/'));
    println!("\x1b[36m🌋 {} — {} models\x1b[0m\n", tier.label(), models.len());

    let show = models.len().min(8);
    let mut tasks = Vec::new();
    for m in models.iter().take(show) {
        let c = client.clone(); let k = key.to_string(); let q = query.to_string();
        let m = m.clone(); let u = url.clone();
        tasks.push(tokio::spawn(async move {
            let body = serde_json::json!({"model": m, "messages": [{"role": "user", "content": q}], "temperature": 0.3, "max_tokens": 2048});
            let r = c.post(&u).header("Authorization", format!("Bearer {}", k)).header("Content-Type", "application/json").json(&body).send().await;
            match r { Ok(resp) if resp.status().is_success() => { let json: serde_json::Value = resp.json().await.unwrap_or_default(); Ok((m, json["choices"][0]["message"]["content"].as_str().unwrap_or("").to_string())) } Ok(resp) => Err(format!("HTTP {}", resp.status())), Err(e) => Err(format!("{}", e)), }
        }));
    }
    for r in futures::future::join_all(tasks).await { if let Ok(Ok((m, content))) = r { let t: String = content.chars().take(200).collect(); println!("\x1b[36m  {}\x1b[0m\n    {}\n", m, t.replace('\n', "\n    ")); } }
    Ok(())
}
