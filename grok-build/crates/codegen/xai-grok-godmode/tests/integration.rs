#[cfg(test)]
mod tests {
    use xai_grok_godmode::*;

    #[test]
    fn test_godmode_config_defaults() {
        let config = GodmodeConfig::default();
        assert!(!config.enabled);
        assert_eq!(config.default_profile, "classic-coding");
        assert!(config.autotune);
        assert!(!config.telemetry);
        assert_eq!(config.candidates.len(), 5);
    }

    #[test]
    fn test_candidate_presets_exist() {
        let presets = config::default_presets();
        assert_eq!(presets.len(), 5);
        let ids: Vec<&str> = presets.iter().map(|p| p.id.as_str()).collect();
        assert!(ids.contains(&"architect"));
        assert!(ids.contains(&"debugger"));
        assert!(ids.contains(&"minimalist"));
        assert!(ids.contains(&"security"));
        assert!(ids.contains(&"skeptic"));
    }

    #[test]
    fn test_candidate_agent_read_only() {
        let preset = config::default_presets().into_iter().find(|p| p.id == "architect").unwrap();
        let agent = CandidateAgent::new(preset);
        let denied = agent.permission_denylist();
        assert!(denied.contains(&"write_file".to_string()));
        assert!(denied.contains(&"bash".to_string()));
    }

    #[test]
    fn test_refusal_detection_explicit() {
        let status = RefusalDetector::detect("I cannot assist with that request");
        assert!(RefusalDetector::is_refused(&status));
        assert!(matches!(status, RefusalStatus::ExplicitRefusal { .. }));
    }

    #[test]
    fn test_refusal_detection_empty() {
        let status = RefusalDetector::detect("");
        assert!(RefusalDetector::is_refused(&status));
        assert_eq!(status, RefusalStatus::Empty);
    }

    #[test]
    fn test_refusal_detection_clean() {
        let status = RefusalDetector::detect("The fix is in src/main.rs line 42. Change the function signature.");
        assert!(!RefusalDetector::is_refused(&status));
    }

    #[test]
    fn test_ultraplinian_tier_counts() {
        assert_eq!(UltraplinianTier::Fast.model_count(), 12);
        assert_eq!(UltraplinianTier::Standard.model_count(), 27);
        assert_eq!(UltraplinianTier::Smart.model_count(), 41);
        assert_eq!(UltraplinianTier::Power.model_count(), 53);
        assert_eq!(UltraplinianTier::Ultra.model_count(), 60);
    }

    #[test]
    fn test_ultraplinian_tier_models() {
        let fast = tier_models(&UltraplinianTier::Fast);
        assert_eq!(fast.len(), 12);
        assert!(fast[0].starts_with("openrouter:"));

        let ultra = tier_models(&UltraplinianTier::Ultra);
        assert_eq!(ultra.len(), 60);
    }

    #[test]
    fn test_parseltongue_33_transformations() {
        assert_eq!(parseltongue::ALL_TRANSFORMATIONS.len(), 33);
    }

    #[test]
    fn test_parseltongue_light_uses_11() {
        let p = Parseltongue::new();
        let result = p.transform("hack and bypass security", Intensity::Light, &[]);
        assert_eq!(result.triggers_found.len(), 2);
        assert_eq!(result.applied_transformations.len(), 2);
    }

    #[test]
    fn test_parseltongue_all_techniques_named() {
        assert_eq!(parseltongue::TRANSFORM_NAMES.len(), 33);
    }

    #[test]
    fn test_autotune_20_contexts() {
        assert_eq!(AutoTuneContext::all().len(), 20);
    }

    #[test]
    fn test_autotune_classify_debugging() {
        let (ctx, conf) = AutoTune::classify("fix this bug in the auth module");
        assert_eq!(ctx, AutoTuneContext::Debugging);
        assert!(conf > 0.0);
    }

    #[test]
    fn test_autotune_classify_security() {
        let (ctx, conf) = AutoTune::classify("audit for XSS vulnerability");
        assert_eq!(ctx, AutoTuneContext::SecurityAudit);
        assert!(conf > 0.0);
    }

    #[test]
    fn test_autotune_fallback() {
        let (ctx, _) = AutoTune::classify("hello world");
        assert_eq!(ctx, AutoTuneContext::SimpleQuestion);
    }

    #[test]
    fn test_autotune_tune_params_debugging() {
        let params = AutoTune::tune_params(&AutoTuneContext::Debugging);
        assert!(params.temperature < 0.2);
        assert_eq!(params.candidate_count, 3);
    }

    #[test]
    fn test_tournament_grouping() {
        let candidates: Vec<RaceCandidateResult> = (0..12).map(|i| RaceCandidateResult {
            candidate_id: format!("c{}", i),
            provider: "test".into(), model: "test".into(), persona: "test".into(),
            proposal: None, score: Some(i as f64), status: CandidateStatus::Completed,
            latency_ms: 0, tokens_used: 0, estimated_cost_usd: 0.0,
        }).collect();

        let groups = Tournament::create_groups(candidates.clone(), 4);
        assert_eq!(groups.len(), 3);

        let winners = Tournament::select_winners(groups, 2);
        assert_eq!(winners.len(), 6);
    }

    #[test]
    fn test_tournament_run_rounds() {
        let candidates: Vec<RaceCandidateResult> = (0..20).map(|i| RaceCandidateResult {
            candidate_id: format!("c{}", i),
            provider: "test".into(), model: "test".into(), persona: "test".into(),
            proposal: None, score: Some(i as f64), status: CandidateStatus::Completed,
            latency_ms: 0, tokens_used: 0, estimated_cost_usd: 0.0,
        }).collect();

        let result = Tournament::run_tournament_rounds(candidates, 4, 2, 4);
        assert!(result.len() <= 4);
    }

    #[test]
    fn test_race_export_json() {
        let race = RaceResult {
            race_id: "test-1".into(),
            winner: None,
            candidates: vec![],
            judge_decisions: vec![],
            total_cost_usd: 0.0,
            total_latency_ms: 0,
        };
        let json = race_export::export_race_json(&race);
        assert!(json.contains("test-1"));
        assert!(serde_json::from_str::<serde_json::Value>(&json).is_ok());
    }

    #[test]
    fn test_race_export_markdown() {
        let race = RaceResult {
            race_id: "test-1".into(),
            winner: None,
            candidates: vec![],
            judge_decisions: vec![],
            total_cost_usd: 0.0,
            total_latency_ms: 0,
        };
        let md = race_export::export_race_markdown(&race);
        assert!(md.contains("# Race Result: test-1"));
    }

    #[test]
    fn test_privacy_local_only() {
        let config = privacy::PrivacyConfig {
            mode: privacy::PrivacyMode::LocalOnly,
            ..Default::default()
        };
        assert!(config.is_local_only());
        assert!(config.is_no_log());
        assert!(!config.allow_remote(false));
        assert!(config.allow_remote(true));
    }

    #[test]
    fn test_privacy_no_log() {
        let config = privacy::PrivacyConfig {
            mode: privacy::PrivacyMode::NoLog,
            ..Default::default()
        };
        assert!(config.is_no_log());
        assert!(!config.is_local_only());
    }

    #[test]
    fn test_cli_flags_default() {
        let flags = cli::GodmodeCliFlags::default();
        assert_eq!(flags.mode, cli::CliMode::Single);
        assert!(!flags.headless);
        assert!(flags.image.is_empty());
    }

    #[test]
    fn test_slash_command_parse() {
        let cmd = cli::SlashCommand::parse("/godmode classic").unwrap();
        assert_eq!(cmd.command, "/godmode");
        assert_eq!(cmd.args, vec!["classic"]);

        let cmd = cli::SlashCommand::parse("/privacy local-only").unwrap();
        assert_eq!(cmd.command, "/privacy");
        assert_eq!(cmd.args, vec!["local-only"]);

        assert!(cli::SlashCommand::parse("not a command").is_none());
    }

    #[test]
    fn test_headless_output_serialization() {
        let mut output = headless::HeadlessOutput::new("session-1".into());
        output.push_event(headless::HeadlessEvent::RaceStarted {
            race_id: "r1".into(), tier: "fast".into(),
            mode: "godmode".into(), candidate_count: 5,
        });

        let json = output.to_json();
        let parsed: serde_json::Value = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed["session_id"], "session-1");
        assert_eq!(parsed["events"][0]["type"], "race-started");
    }

    #[test]
    fn test_context_budget() {
        let budget = context_budget::ContextBudget::new(1000, 500);
        assert_eq!(budget.remaining_input(), 1000);
        assert_eq!(budget.remaining_output(), 500);
        assert!(!budget.is_exhausted());
    }

    #[test]
    fn test_cost_tracker() {
        let mut tracker = cost::CostTracker::new();
        tracker.add_usage("openrouter", 1_000_000, 500_000, 2.0, 10.0);
        assert!(tracker.total_cost_usd > 0.0);
        assert!(tracker.per_provider.contains_key("openrouter"));
    }

    #[test]
    fn test_proposal_merge() {
        let p1 = CandidateProposal {
            candidate_id: "a".into(), provider: "x".into(), model: "m1".into(),
            persona: "Architect".into(), summary: "Fix A".into(),
            diagnosis: "Issue in file A".into(),
            evidence: vec![], files_to_change: vec!["a.rs".into()],
            symbols_to_change: vec![], proposed_changes: vec![],
            proposed_patch: None, commands_to_run: vec![],
            tests: vec!["test_a".into()], risks: vec!["risk1".into()],
            assumptions: vec![], limitations: vec![],
            confidence: 0.8,
        };
        let p2 = CandidateProposal {
            candidate_id: "b".into(), provider: "x".into(), model: "m2".into(),
            persona: "Debugger".into(), summary: "Fix B".into(),
            diagnosis: "Issue in file B".into(),
            evidence: vec![], files_to_change: vec!["b.rs".into()],
            symbols_to_change: vec![], proposed_changes: vec![],
            proposed_patch: None, commands_to_run: vec![],
            tests: vec!["test_b".into()], risks: vec!["risk2".into()],
            assumptions: vec![], limitations: vec![],
            confidence: 0.9,
        };
        let merged = manual_override::merge_proposals(&[p1, p2]);
        assert!(merged.files_to_change.contains(&"a.rs".to_string()));
        assert!(merged.files_to_change.contains(&"b.rs".to_string()));
        assert!(merged.tests.contains(&"test_a".to_string()));
        assert!(merged.tests.contains(&"test_b".to_string()));
        assert!(merged.risks.contains(&"risk1".to_string()));
        assert!(merged.risks.contains(&"risk2".to_string()));
        assert!((merged.confidence - 0.85).abs() < 0.01);
    }
}
