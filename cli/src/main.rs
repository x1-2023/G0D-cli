mod config;
mod context;

use std::io::{self, Write};
use xai_grok_godmode::ultraplinian::UltraplinianTier;
use xai_grok_godmode::parseltongue::{Parseltongue, Intensity};
use xai_grok_godmode::config as gm_config;

const C: &str = "\x1b[36m"; // cyan
const G: &str = "\x1b[32m"; // green
const Y: &str = "\x1b[33m"; // yellow
const M: &str = "\x1b[35m"; // magenta
const D: &str = "\x1b[90m"; // dim
const R: &str = "\x1b[0m";  // reset

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
            "--language" => { let v = args.get(2).cloned().unwrap_or("auto".into()); cfg.set_lang(&v); cfg.save(); println!("Language: {}", v); }
            "--ui-language" => { let v = args.get(2).cloned().unwrap_or("en".into()); cfg.set_ui_lang(&v); cfg.save(); println!("UI: {}", v); }
            _ => { let q = args[1..].join(" "); cmd_chat(&cfg, &q).await?; }
        }
        return Ok(());
    }

    banner();
    repl(&mut cfg, "chat").await
}

fn banner() {
    let ctx = context::read_context();
    println!("{M}  g0d{D} — AI coding agent for your project{R}");
    println!("{D}  {}  |  {}{R}", ctx.cwd, ctx.project_type);
    if let Some(ref branch) = ctx.git_branch {
        let status = ctx.git_status.as_deref().unwrap_or("");
        print!("{D}  {}{R}", branch);
        if !status.is_empty() { print!("{D}  [{}]{R}", status); }
        println!();
    }
    println!();
}

fn print_help() {
    println!(r#"g0d — AI coding agent

  g0d                Chat with your codebase
  g0d "query"        Quick question  
  g0d -g "query"     5 models racing (GODMODE)
  g0d -u "query"     Up to 60 models racing

Start here:
  1. Get key: https://openrouter.ai/keys
  2. g0d
  3. /key sk-or-v1-...
  4. Ask anything
"#);
}

async fn repl(cfg: &mut config::Config, mode: &str) -> anyhow::Result<()> {
    let mut mode = mode.to_string();
    let has_key = cfg.get_api_key().is_ok();

    if !has_key {
        println!("{Y}  First time? Let's set up your API key.{R}");
        println!("{D}  1. Get free key at {C}https://openrouter.ai/keys{R}");
        println!("{D}  2. Paste it here: {C}/key sk-or-v1-...{R}");
        println!("{D}  3. Then just type your question!{R}\n");
    } else {
        println!("{D}  Ready. Type a question or /help to see commands.{R}\n");
    }

    loop {
        let cwd_name = current_dir_name();
        let label = match mode.as_str() {
            "godmode" => format!("{C}godmode{R}"),
            "parseltongue" => format!("{G}snake{R}"),
            "ultra" => format!("{C}ultra{R}"),
            _ => cwd_name,
        };
        print!("{D}{}{R} {M}>{R} ", label);
        io::stdout().flush()?;

        let mut input = String::new();
        if io::stdin().read_line(&mut input).is_err() { break; }
        let input = input.trim().to_string();
        if input.is_empty() { continue; }

        if input.starts_with('/') {
            let parts: Vec<&str> = input.splitn(4, ' ').collect();
            match parts[0] {
                "/exit" | "/quit" => { println!("{M}bye{R}\n"); break; }
                "/help" => print_slash_help(),
                "/key" => cmd_key(cfg, parts.get(1).copied()),
                "/provider" => cmd_provider(cfg, parts.get(1).copied(), parts.get(2).copied(), parts.get(3).copied()),
                "/model" => cmd_model(cfg, parts.get(1).copied()),
                "/providers" => cmd_providers_list(cfg),
                "/status" => cmd_status(cfg, &mode),
                "/chat" => { mode = "chat".into(); println!("{G}  Chat mode{R}\n"); }
                "/godmode" => { mode = "godmode".into(); println!("{Y}  GODMODE — 5 models racing{R}\n"); }
                "/snake" => { mode = "parseltongue".into(); println!("{G}  Parseltongue{R}\n"); }
                "/ultra" => { mode = "ultra".into(); println!("{C}  ULTRAPLINIAN{R}\n"); }
                "/language" => cmd_language(cfg, parts.get(1).copied()),
                cmd => {
                    println!("{Y}  Unknown: {cmd}{R}");
                    suggest_command(cmd);
                    println!();
                }
            }
            continue;
        }

        let key = match cfg.get_api_key() {
            Ok(k) => k,
            Err(e) => {
                println!("{Y}  No API key. {D}/key sk-or-v1-...{R}\n");
                println!("{D}  Get one: {C}https://openrouter.ai/keys{R}\n");
                continue;
            }
        };
        let endpoint = cfg.get_endpoint();

        println!("{D}──────────────────────────────────────────────────{R}");
        match mode.as_str() {
            "godmode" => { let _ = run_godmode(&key, &endpoint, &input).await; }
            "parseltongue" => { let _ = run_parseltongue_repl(&key, &endpoint, &input).await; }
            "ultra" => { let _ = run_ultra(&key, &endpoint, &input, "fast").await; }
            _ => { let _ = run_chat(&key, &endpoint, &input, &cfg.default_model()).await; }
        }
        println!("{D}──────────────────────────────────────────────────{R}");
    }
    Ok(())
}

fn suggest_command(input: &str) {
    let suggestions: Vec<&str> = [
        "/key", "/provider", "/model", "/providers", "/status",
        "/chat", "/godmode", "/snake", "/ultra", "/language",
        "/help", "/exit",
    ].iter().filter(|c| {
        let d = str_distance(c, input);
        d < 4 || c.contains(input) || input.contains(*c)
    }).copied().take(3).collect();

    if suggestions.is_empty() {
        println!("{D}  Try {C}/help{D} to see all commands{R}");
    } else {
        println!("{D}  Did you mean: {C}{}?{R}", suggestions.join(", "));
    }
}

fn str_distance(a: &str, b: &str) -> usize {
    let a = a.as_bytes(); let b = b.as_bytes();
    let n = a.len(); let m = b.len();
    if n == 0 { return m; } if m == 0 { return n; }
    let mut prev: Vec<usize> = (0..=m).collect();
    let mut curr = vec![0; m + 1];
    for i in 1..=n {
        curr[0] = i;
        for j in 1..=m {
            let cost = if a[i-1] == b[j-1] { 0 } else { 1 };
            curr[j] = (prev[j] + 1).min(curr[j-1] + 1).min(prev[j-1] + cost);
        }
        std::mem::swap(&mut prev, &mut curr);
    }
    prev[m]
}

fn cmd_key(cfg: &mut config::Config, arg: Option<&str>) {
    if let Some(key) = arg {
        cfg.set_provider_key(&cfg.active_provider_id(), key);
        cfg.save();
        println!("{G}  Key saved! Type your question.{R}\n");
    } else {
        println!("{D}  Paste your OpenRouter API key:{R}");
        println!("{D}  {C}/key sk-or-v1-...{R}");
        println!("{D}  Get one at {C}https://openrouter.ai/keys{R}\n");
    }
}

fn print_slash_help() {
    println!();
    println!("{C}  Essentials{R}");
    println!("{D}  {C}/key <key>                {D}Set API key{R}");
    println!("{D}  {C}/model <provider:model>   {D}Change AI model{R}");
    println!("{D}  {C}/providers                {D}Show all providers{R}");
    println!("{D}  {C}/status                   {D}Current settings{R}");
    println!();
    println!("{C}  Modes{R}");
    println!("{D}  {C}/chat                    {D}Normal chat (default){R}");
    println!("{D}  {C}/godmode                 {D}5 models race, pick best answer{R}");
    println!("{D}  {C}/snake                   {D}Parseltongue text obfuscation{R}");
    println!("{D}  {C}/ultra                   {D}Up to 60 models race{R}");
    println!();
    println!("{C}  Language{R}");
    println!("{D}  {C}/language vi             {D}Reply in Vietnamese{R}");
    println!("{D}  {C}/language en             {D}Reply in English{R}");
    println!("{D}  {C}/language auto           {D}Auto-detect{R}");
    println!();
    println!("{D}  {C}/exit                    {D}Quit{R}");
    println!();
}

fn cmd_provider(cfg: &mut config::Config, sub: Option<&str>, arg1: Option<&str>, arg2: Option<&str>) {
    match sub {
        Some("key") => match (arg1, arg2) {
            (Some(id), Some(key)) => { cfg.set_provider_key(id, key); cfg.save(); println!("{G}  Key saved for '{id}'{R}\n"); }
            _ => {
                println!("{D}  Set API key for a provider:{R}");
                println!("{D}  {C}/provider key openrouter sk-or-v1-...{R}");
                println!("{D}  {C}/provider key venice vnc-...{R}\n");
            }
        }
        Some("add") => match (arg1, arg2) {
            (Some(id), Some(endpoint)) => {
                cfg.add_provider(id.to_string(), endpoint.to_string(), None); cfg.save();
                println!("{G}  Added provider '{id}' → {endpoint}{R}\n");
            }
            _ => println!("{D}  {C}/provider add <name> <url>{D}  e.g. /provider add ollama http://localhost:11434/v1{R}\n"),
        }
        Some("default") => if let Some(id) = arg1 {
            cfg.set_default_provider(id); cfg.save(); println!("{G}  Now using '{id}'{R}\n");
        } else {
            println!("{D}  Current: {C}{}{R}\n", cfg.active_provider_id());
        }
        Some("rm") | Some("remove") => if let Some(id) = arg1 {
            cfg.remove_provider(id); cfg.save(); println!("{G}  Removed '{id}'{R}\n");
        } else { println!("{D}  {C}/provider rm <name>{R}\n"); }
        _ => {
            println!("{D}  Manage providers:{R}\n");
            println!("{D}  {C}/provider key <name> <key>     {D}Add API key{R}");
            println!("{D}  {C}/provider add <name> <url>     {D}Add custom provider{R}");
            println!("{D}  {C}/provider default <name>        {D}Switch provider{R}");
            println!("{D}  {C}/providers                     {D}List all{R}\n");
            cmd_providers_mini(cfg);
        }
    }
}

fn cmd_providers_mini(cfg: &config::Config) {
    println!("{D}  Available:{R}");
    for p in &cfg.providers {
        if !p.enabled { continue; }
        let star = if Some(&p.id) == cfg.default_provider.as_ref() { "*" } else { " " };
        let has_key = p.api_key.as_ref().map_or(false, |k| !k.is_empty())
            || p.key_env.as_ref().and_then(|e| std::env::var(e).ok()).map_or(false, |k| !k.is_empty());
        let dot = if has_key { "{G}●{R}" } else { "{D}○{R}" };
        println!("{D}    {dot} {star} {}{R}", p.id);
    }
    println!("{D}  * = current, {G}●{D} = key set{R}\n");
}

fn cmd_model(cfg: &mut config::Config, arg: Option<&str>) {
    match arg {
        Some(m) if !m.is_empty() => { cfg.set_model(m); cfg.save(); println!("{G}  Model: {m}{R}\n"); }
        _ => {
            println!("{D}  Current: {C}{}{R}", cfg.default_model());
            println!("{D}  Change: {C}/model openrouter:anthropic/claude-sonnet-4.6{R}");
            println!("{D}  Popular:{R}");
            println!("{D}    {C}anthropic/claude-sonnet-4.6{D} — best all-round{R}");
            println!("{D}    {C}openai/gpt-5.6{D}            — strong coding{R}");
            println!("{D}    {C}google/gemini-2.5-pro{D}      — reasoning{R}");
            println!("{D}    {C}deepseek/deepseek-chat{D}     — fast & cheap{R}");
            println!();
        }
    }
}

fn cmd_providers_list(cfg: &config::Config) {
    println!();
    for p in &cfg.providers {
        if !p.enabled { continue; }
        let star = if Some(&p.id) == cfg.default_provider.as_ref() { "*" } else { " " };
        let has_key = p.api_key.as_ref().map_or(false, |k| !k.is_empty())
            || p.key_env.as_ref().and_then(|e| std::env::var(e).ok()).map_or(false, |k| !k.is_empty());
        let key_status = if has_key { format!("{G}set{R}") } else { format!("{D}no key{R}") };
        let ptype = if p.is_local { format!("{G}local{R}") } else { format!("{D}remote{R}") };
        println!("{D}  {star} {C}{:<14}{R} {ptype}  {key_status}  {D}{}{R}", p.id, p.endpoint);
    }
    println!("{D}  * = current, {C}/provider default <name>{D} to switch{R}\n");
}

fn cmd_status(cfg: &config::Config, mode: &str) {
    let p = cfg.active_provider();
    let has_key = cfg.get_api_key().is_ok();
    let lang_label = match cfg.get_lang() { "vi" => "VN", "en" => "EN", _ => "auto", };
    println!();
    println!("{D}  Mode:     {C}{mode}{R}");
    println!("{D}  Provider: {C}{}{D}  →  {}{R}", p.id, p.endpoint);
    println!("{D}  Model:    {C}{}{R}", cfg.default_model());
    println!("{D}  Key:      {}{R}", if has_key { format!("{G}set{R}") } else { format!("{Y}not set{R}") });
    println!("{D}  Language: {C}{}{R}", lang_label);
    println!("{D}  Config:   {C}{}{R}", config::config_path().display());
    println!();
}

fn cmd_language(cfg: &mut config::Config, arg: Option<&str>) {
    match arg {
        Some("vi") => { cfg.set_lang("vi"); cfg.save(); println!("{G}  Reply in Vietnamese{R}\n"); }
        Some("en") => { cfg.set_lang("en"); cfg.save(); println!("{G}  Reply in English{R}\n"); }
        Some("auto") => { cfg.set_lang("auto"); cfg.save(); println!("{G}  Auto-detect language{R}\n"); }
        None => {
            let lang = match cfg.get_lang() { "vi" => "Vietnamese", "en" => "English", _ => "Auto-detect", };
            println!("{D}  Language: {C}{}{R}", lang);
            println!("{D}  {C}/language vi{D} | {C}en{D} | {C}auto{R}\n");
        }
        _ => println!("{D}  {C}/language vi{D} | {C}en{D} | {C}auto{R}\n"),
    }
}

fn current_dir_name() -> String {
    std::env::current_dir().ok()
        .and_then(|p| p.file_name().map(|n| n.to_string_lossy().to_string()))
        .unwrap_or_else(|| ".".into())
}

fn list_models() {
    for tier in [UltraplinianTier::Fast, UltraplinianTier::Standard, UltraplinianTier::Smart, UltraplinianTier::Power, UltraplinianTier::Ultra] {
        let models = xai_grok_godmode::tier_models(&tier);
        println!("{}: {} models", tier.label(), models.len());
        for m in &models { println!("  {}", m); }
        println!();
    }
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
    let _ = run_parseltongue_repl(&key, &endpoint, query).await;
    Ok(())
}

async fn cmd_ultra(cfg: &config::Config, query: &str, tier: &str) -> anyhow::Result<()> {
    let key = cfg.get_api_key()?; let endpoint = cfg.get_endpoint();
    run_ultra(&key, &endpoint, query, tier).await
}

async fn run_chat(key: &str, endpoint: &str, query: &str, model: &str) -> anyhow::Result<()> {
    let client = reqwest::Client::new();
    let url = format!("{}/chat/completions", endpoint.trim_end_matches('/'));
    let ctx = context::read_context();
    let system = format!(
        "You are g0d, a coding assistant. You help with code in the current project.\n\n{}\nBe concise. Use file paths when relevant.",
        context::context_summary(&ctx),
    );
    let body = serde_json::json!({
        "model": model, "messages": [{"role":"system","content":system},{"role":"user","content":query}],
        "stream": true, "temperature": 0.7, "max_tokens": 8192,
    });
    let resp = client.post(&url).header("Authorization", format!("Bearer {}", key))
        .header("Content-Type", "application/json").json(&body).send().await?;

    if !resp.status().is_success() {
        let s = resp.status(); let e = resp.text().await.unwrap_or_default();
        anyhow::bail!("HTTP {}: {}", s, e);
    }
    let mut stream = resp.bytes_stream();
    use futures::StreamExt;
    while let Some(chunk) = stream.next().await {
        let chunk = chunk?;
        for line in String::from_utf8_lossy(&chunk).lines() {
            let line = line.trim();
            if !line.starts_with("data: ") { continue; }
            if &line[6..] == "[DONE]" { continue; }
            if let Ok(p) = serde_json::from_str::<serde_json::Value>(&line[6..]) {
                if let Some(c) = p["choices"][0]["delta"]["content"].as_str() { print!("{}", c); io::stdout().flush()?; }
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
        if r.applied_transformations.len() > 0 {
            println!("{G}[{label}]{R} {}", r.transformed);
        }
    }
    run_chat(key, endpoint, query, "anthropic/claude-sonnet-4").await
}

async fn run_godmode(key: &str, endpoint: &str, query: &str) -> anyhow::Result<()> {
    let client = reqwest::Client::new();
    let presets = gm_config::default_presets();
    let url = format!("{}/chat/completions", endpoint.trim_end_matches('/'));
    println!("{Y}  GODMODE — 5 models{R}\n");

    let mut tasks = Vec::new();
    for preset in &presets {
        let c = client.clone(); let k = key.to_string(); let q = query.to_string();
        let p = preset.clone(); let u = url.clone();
        tasks.push(tokio::spawn(async move {
            let body = serde_json::json!({
                "model": p.model,
                "messages": [
                    {"role":"system","content": format!("You are {}: {}", p.persona.name, p.persona.role)},
                    {"role":"user","content": q},
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
            println!("{C}  {}{R} ({D}{}{R})\n    {}\n", p.persona.name, p.model, t.replace('\n', "\n    "));
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
    println!("{C}  ULTRAPLINIAN {}{R} — {} models\n", tier.label(), models.len());

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
                    Ok((m, json["choices"][0]["message"]["content"].as_str().unwrap_or("").to_string()))
                }
                Ok(resp) => Err(format!("HTTP {}", resp.status())),
                Err(e) => Err(format!("{}", e)),
            }
        }));
    }
    for r in futures::future::join_all(tasks).await {
        if let Ok(Ok((m, content))) = r {
            let t: String = content.chars().take(200).collect();
            println!("{C}  {}{R}\n    {}\n", m, t.replace('\n', "\n    "));
        }
    }
    Ok(())
}
