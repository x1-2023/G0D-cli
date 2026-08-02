use xai_grok_providers::{ProviderConfig, ProviderCapabilities, ModelCatalog, ModelInfo, ModelRef, Credential, ProviderError, HealthState, ProviderHealth, retry::RetryPolicy};
use xai_grok_godmode::*;
use xai_grok_godmode::config;
use xai_grok_godmode::judge;
use xai_grok_godmode::race_export;

#[tokio::main]
async fn main() {
    let args: Vec<String> = std::env::args().collect();

    if args.len() < 2 {
        print_usage();
        return;
    }

    match args[1].as_str() {
        "test" => cmd_test_all().await,
        "providers" => cmd_list_providers(),
        "models" => cmd_list_models(&args),
        "godmode" => cmd_godmode_demo(),
        "parseltongue" => cmd_parseltongue(&args),
        "autotune" => cmd_autotune(&args),
        "ultra" => cmd_ultra_demo(),
        "privacy" => cmd_privacy_demo(&args),
        "refusal" => cmd_refusal_demo(&args),
        "score" => cmd_score_demo(),
        "export" => cmd_export_demo(),
        "health" => cmd_health_demo(),
        _ => print_usage(),
    }
}

fn print_usage() {
    println!(r#"
G0D-cli — GODMODE Coding Edition CLI

USAGE:
  cargo run --example demo -- <command> [args]

COMMANDS:
  test            Run all unit tests (via cargo test)
  providers       List all configured providers with defaults
  models          List ULTRAPLINIAN tier model counts
  godmode         Show GODMODE CLASSIC 5 candidate presets
  parseltongue    Transform text with Parseltongue obfuscation
  autotune        Classify a query and show tuned parameters
  ultra           Show ULTRAPLINIAN tier tree
  privacy         Show privacy mode details
  refusal         Test refusal detection on sample texts
  score           Show scoring rubric breakdown
  export          Show race export format examples
  health          Show provider health state machine

EXAMPLES:
  cargo run --example demo -- parseltongue "how to hack bypass jailbreak"
  cargo run --example demo -- autotune "fix the auth bug in login.rs"
  cargo run --example demo -- models
  cargo run --example demo -- godmode
  cargo run --example demo -- privacy local-only
"#);
}

fn cmd_list_providers() {
    println!("=== Provider Defaults ===\n");
    let grok = ProviderConfig::grok_default();
    let or = ProviderConfig::openrouter_default();
    let venice = ProviderConfig::venice_default();
    let local = ProviderConfig::local_default("ollama", "http://localhost:11434/v1");

    for (name, cfg) in [("Grok", &grok), ("OpenRouter", &or), ("Venice", &venice), ("Local", &local)] {
        println!("{}:", name);
        println!("  id:       {}", cfg.id);
        println!("  enabled:  {}", cfg.enabled);
        println!("  local:    {}", cfg.local);
        println!("  base_url: {}", cfg.base_url.as_deref().unwrap_or("N/A"));
        println!("  key_env:  {}", cfg.api_key_env.as_deref().unwrap_or("N/A"));
        println!("  timeout:  {}s", cfg.request_timeout_seconds);
        println!();
    }
}

fn cmd_list_models(args: &[String]) {
    println!("=== ULTRAPLINIAN Tiers ===\n");
    for tier in [UltraplinianTier::Fast, UltraplinianTier::Standard, UltraplinianTier::Smart, UltraplinianTier::Power, UltraplinianTier::Ultra] {
        let models = tier_models(&tier);
        println!("{} - {} models:", tier.label(), models.len());
        if args.iter().any(|a| a == "--verbose") {
            for m in &models {
                println!("  {}", m);
            }
        }
        println!();
    }
}

fn cmd_godmode_demo() {
    println!("=== GODMODE CLASSIC — 5 Candidate Presets ===\n");
    let presets = config::default_presets();
    for p in &presets {
        println!("{} ({})", p.persona.name, p.id);
        println!("  Provider: {} | Model: {} | Temp: {}", p.provider, p.model, p.temperature);
        println!("  Role: {}", p.persona.role);
        println!("  Allowed tools: {:?}", p.persona.allowed_tools);
        println!("  Denied tools:  {:?}", p.persona.denied_tools);
        println!();
    }
}

fn cmd_parseltongue(args: &[String]) {
    let text = if args.len() > 2 { args[2..].join(" ") } else { "hack bypass jailbreak malware exploit".to_string() };
    println!("=== Parseltongue — Input Obfuscation ===\n");
    println!("Original: {}\n", text);

    let p = Parseltongue::new();

    for (label, intensity) in [("Light", Intensity::Light), ("Standard", Intensity::Standard), ("Heavy", Intensity::Heavy)] {
        let result = p.transform(&text, intensity, &[]);
        println!("[{}] {} techniques applied", label, result.applied_transformations.len());
        println!("  Transformed: {}", result.transformed);
        println!("  Triggers: {:?}", result.triggers_found);
        println!();
    }
}

fn cmd_autotune(args: &[String]) {
    let query = if args.len() > 2 { args[2..].join(" ") } else { "fix the authentication bug in login.rs".to_string() };
    println!("=== AutoTune — Context Detection ===\n");
    println!("Query: {}\n", query);

    let (context, confidence) = AutoTune::classify(&query);
    let params = AutoTune::tune_params(&context);

    println!("Context:    {} (confidence: {:.0}%)", context.label(), confidence * 100.0);
    println!("Params:");
    println!("  temperature:         {:.2}", params.temperature);
    println!("  top_p:               {:.2}", params.top_p);
    println!("  top_k:               {}", params.top_k);
    println!("  candidate_count:     {}", params.candidate_count);
    println!("  judge_count:         {}", params.judge_count);
    println!("  max_output_tokens:   {}", params.max_output_tokens);

    println!("\n=== All 20 Contexts ===\n");
    for ctx in AutoTuneContext::all() {
        println!("  - {}", ctx.label());
    }
}

fn cmd_ultra_demo() {
    println!("=== ULTRAPLINIAN — Tier Breakdown ===\n");
    let mut total = 0;
    for tier in [UltraplinianTier::Fast, UltraplinianTier::Standard, UltraplinianTier::Smart, UltraplinianTier::Power, UltraplinianTier::Ultra] {
        let count = tier.model_count();
        total = count;
        println!("{} ({})", tier.label(), count);
    }
    println!("\nMax models in ULTRA tier: {}", total);
}

fn cmd_privacy_demo(args: &[String]) {
    let mode = if args.len() > 2 {
        match args[2].as_str() {
            "standard" => PrivacyMode::Standard,
            "no-log" => PrivacyMode::NoLog,
            "local-only" => PrivacyMode::LocalOnly,
            "preview" => PrivacyMode::PrivacyPreview,
            _ => PrivacyMode::Standard,
        }
    } else {
        PrivacyMode::Standard
    };

    let config = PrivacyConfig { mode: mode.clone(), ..Default::default() };
    println!("=== Privacy Mode: {} ===\n", mode.label());
    println!("  local_only:   {}", config.is_local_only());
    println!("  no_log:       {}", config.is_no_log());
    println!("  allow_remote: {}", config.allow_remote(false));
    println!("  preview:      {}", config.requires_preview());
}

fn cmd_refusal_demo(_args: &[String]) {
    println!("=== Refusal Detection ===\n");
    let test_cases = [
        "I cannot assist with that request.",
        "I'm sorry, but I can't comply with this.",
        "As an AI language model, I must decline.",
        "The fix is in src/main.rs line 42. Change the function signature to accept an additional parameter.",
        "",
        "I'm unable to help with that.",
    ];

    for text in &test_cases {
        let status = RefusalDetector::detect(text);
        let refused = RefusalDetector::is_refused(&status);
        let penalty = RefusalDetector::penalty(&status);
        let label = if refused { "REFUSED" } else { "CLEAN" };
        println!("[{}] penalty={:.0} | \"{}\"", label, penalty, if text.len() > 80 { &text[..80] } else { text });
    }
}

fn cmd_score_demo() {
    println!("=== Scoring Rubric — 100 Points ===\n");
    let rubric = judge::scoring_rubric_coding();
    let weights = [
        ("Correctness", rubric.correctness.max),
        ("Repository Evidence", rubric.repository_evidence.max),
        ("Architecture Fit", rubric.architecture_fit.max),
        ("Minimal Change", rubric.minimal_change.max),
        ("Testability", rubric.testability.max),
        ("Security", rubric.security.max),
        ("Regression Risk", rubric.regression_risk.max),
        ("Clarity", rubric.clarity.max),
        ("Performance", rubric.performance.max),
        ("Maintainability", rubric.maintainability.max),
    ];
    for (name, max) in &weights {
        let bar = "█".repeat(*max as usize / 2);
        println!("  {:20} {:>3.0}/{} {}", name, max, (max * 10.0) as i32, bar);
    }
    println!("\n  Total: 100 pts");

    println!("\n=== Deterministic Scoring Example ===\n");
    let proposal = CandidateProposal {
        candidate_id: "demo".into(), provider: "test".into(), model: "test".into(),
        persona: "Debugger".into(),
        summary: "Fix race condition in auth module".into(),
        diagnosis: "Mutex lock ordering issue in login_handler.rs:42-67".into(),
        evidence: vec![],
        files_to_change: vec!["src/auth/login_handler.rs".into()],
        symbols_to_change: vec!["login_handler".into()],
        proposed_changes: vec![],
        proposed_patch: None,
        commands_to_run: vec![],
        tests: vec!["test_concurrent_login".into(), "test_race_condition".into()],
        risks: vec!["Possible deadlock if lock order reversed".into()],
        assumptions: vec![],
        limitations: vec![],
        confidence: 0.85,
    };
    let score = score_candidate_deterministic(&proposal, &rubric);
    println!("Proposal score: {:.1}/100", score.total);
    println!("  correctness:     {:.1}", score.correctness);
    println!("  evidence:        {:.1}", score.repository_evidence);
    println!("  architecture:    {:.1}", score.architecture_fit);
    println!("  minimal_change:  {:.1}", score.minimal_change);
    println!("  security:        {:.1}", score.security);
}

fn cmd_export_demo() {
    let race = RaceResult {
        race_id: "demo-2026-08-02".into(),
        winner: Some(CandidateProposal {
            candidate_id: "debugger-abc123".into(),
            provider: "openrouter".into(),
            model: "anthropic/claude-sonnet-4.6".into(),
            persona: "Debugger".into(),
            summary: "Root cause: mutex ordering issue in login_handler.rs".into(),
            diagnosis: "Lock A acquired before Lock B in path X, reversed in path Y".into(),
            evidence: vec![],
            files_to_change: vec!["src/auth/login_handler.rs".into()],
            symbols_to_change: vec!["authenticate_user".into()],
            proposed_changes: vec![],
            proposed_patch: None,
            commands_to_run: vec!["cargo test".into()],
            tests: vec!["test_concurrent_login".into()],
            risks: vec!["Deadlock if order reversed".into()],
            assumptions: vec![],
            limitations: vec![],
            confidence: 0.92,
        }),
        candidates: vec![],
        judge_decisions: vec![],
        total_cost_usd: 0.0234,
        total_latency_ms: 3200,
    };

    println!("=== JSON Export ===\n");
    println!("{}", race_export::export_race_json(&race));

    println!("\n=== Markdown Export ===\n");
    println!("{}", race_export::export_race_markdown(&race));
}

fn cmd_health_demo() {
    println!("=== Provider Health States ===\n");
    let states = [
        HealthState::Unknown,
        HealthState::Healthy,
        HealthState::Degraded,
        HealthState::RateLimited,
        HealthState::Unauthorized,
        HealthState::Unavailable,
        HealthState::Deprecated,
    ];
    for state in &states {
        let usable = if state.is_usable() { "✓ usable" } else { "✗ blocked" };
        println!("  {:15} ({})", state.label(), usable);
    }

    println!("\n=== Error Classification ===\n");
    println!("  Retryable errors: RateLimited, Timeout, Connection, HTTP 429/502/503/504");
    println!("  Auth errors:      Unauthorized, HTTP 401/403");
    println!("  Non-retryable:    Invalid requests, Context exceeded, Empty response");
}

async fn cmd_test_all() {
    println!("=== Provider Tests ===\n");
    println!("cargo test -p xai-grok-providers");
    println!("\n=== Godmode Tests ===\n");
    println!("cargo test -p xai-grok-godmode");
    println!("\nRun manually: cargo test -p xai-grok-providers -p xai-grok-godmode");
}
