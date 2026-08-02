#[cfg(test)]
mod tests {
    use xai_grok_providers::*;
    use xai_grok_providers::retry::RetryPolicy;

    #[test]
    fn test_model_ref_parse() {
        let ref_: ModelRef = "openrouter:anthropic/claude-sonnet-4.6".parse().unwrap();
        assert_eq!(ref_.provider, "openrouter");
        assert_eq!(ref_.model_id, "anthropic/claude-sonnet-4.6");
        assert_eq!(ref_.canonical(), "openrouter:anthropic/claude-sonnet-4.6");
    }

    #[test]
    fn test_model_ref_parse_no_separator() {
        let result: Result<ModelRef, _> = "invalid".parse();
        assert!(result.is_err());
    }

    #[test]
    fn test_model_ref_display() {
        let ref_ = ModelRef::new("grok", "grok-code-fast");
        assert_eq!(format!("{}", ref_), "grok:grok-code-fast");
    }

    #[test]
    fn test_catalog_add_and_find() {
        let mut catalog = ModelCatalog::new();
        catalog.add(ModelInfo {
            provider: "openrouter".into(),
            model_id: "test-model".into(),
            display_name: Some("Test Model".into()),
            context_window: Some(128000),
            max_output_tokens: Some(4096),
            capabilities: ProviderCapabilities::all(),
            pricing: None,
            aliases: vec!["test".into()],
            categories: vec!["coding".into()],
            deprecated: false,
            replacement: None,
        });

        let found = catalog.find("openrouter", "test-model");
        assert!(found.is_some());

        let by_alias = catalog.find_by_alias("openrouter", "test");
        assert!(by_alias.is_some());

        let not_found = catalog.find("openrouter", "nonexistent");
        assert!(not_found.is_none());
    }

    #[test]
    fn test_catalog_dedup() {
        let mut catalog = ModelCatalog::new();
        let model = ModelInfo {
            provider: "openrouter".into(),
            model_id: "dup".into(),
            display_name: None,
            context_window: None,
            max_output_tokens: None,
            capabilities: Default::default(),
            pricing: None,
            aliases: vec![],
            categories: vec![],
            deprecated: false,
            replacement: None,
        };
        catalog.add(model.clone());
        catalog.add(model.clone());
        assert_eq!(catalog.models.len(), 1);
    }

    #[test]
    fn test_catalog_all_providers() {
        let mut catalog = ModelCatalog::new();
        catalog.add(ModelInfo {
            provider: "a".into(), model_id: "1".into(),
            display_name: None, context_window: None, max_output_tokens: None,
            capabilities: Default::default(), pricing: None,
            aliases: vec![], categories: vec![], deprecated: false, replacement: None,
        });
        catalog.add(ModelInfo {
            provider: "b".into(), model_id: "2".into(),
            display_name: None, context_window: None, max_output_tokens: None,
            capabilities: Default::default(), pricing: None,
            aliases: vec![], categories: vec![], deprecated: false, replacement: None,
        });
        let providers = catalog.all_providers();
        assert_eq!(providers, vec!["a", "b"]);
    }

    #[test]
    fn test_error_is_retryable() {
        assert!(ProviderError::RateLimited { provider: "test".into(), retry_after: None }.is_retryable());
        assert!(ProviderError::Timeout { provider: "test".into(), elapsed: std::time::Duration::from_secs(30) }.is_retryable());
        assert!(ProviderError::Http { provider: "test".into(), status: 429, body: "rate limited".into() }.is_retryable());
        assert!(!ProviderError::Auth { provider: "test".into(), detail: "bad key".into() }.is_retryable());
    }

    #[test]
    fn test_error_is_auth_error() {
        assert!(ProviderError::Auth { provider: "test".into(), detail: "bad".into() }.is_auth_error());
        assert!(ProviderError::Http { provider: "test".into(), status: 401, body: "unauthorized".into() }.is_auth_error());
        assert!(!ProviderError::RateLimited { provider: "test".into(), retry_after: None }.is_auth_error());
    }

    #[test]
    fn test_credential_redaction() {
        let cred = Credential::BearerToken("sk-or-v1-abcdef1234567890".into());
        let redacted = cred.redacted();
        assert!(redacted.contains("..."));
        assert!(!redacted.contains("sk-or-v1-abcdef1234567890"));

        let none = Credential::None;
        assert!(!none.is_set());
    }

    #[test]
    fn test_provider_config_resolve_key() {
        let config = ProviderConfig {
            api_key_env: Some("NONEXISTENT_VAR_12345".into()),
            ..Default::default()
        };
        assert!(config.resolve_api_key().is_none());

        let config2 = ProviderConfig {
            api_key: Some("my-key".into()),
            ..Default::default()
        };
        assert_eq!(config2.resolve_api_key(), Some("my-key".into()));
    }

    #[test]
    fn test_health_state_is_usable() {
        assert!(HealthState::Healthy.is_usable());
        assert!(HealthState::Degraded.is_usable());
        assert!(HealthState::Unknown.is_usable());
        assert!(!HealthState::Unauthorized.is_usable());
        assert!(!HealthState::Unavailable.is_usable());
        assert!(!HealthState::Deprecated.is_usable());
    }

    #[test]
    fn test_retry_policy_delay() {
        let policy = RetryPolicy::default();
        let d1 = policy.delay_for_attempt(0);
        assert!(d1 >= std::time::Duration::from_secs(1));
        let d10 = policy.delay_for_attempt(10);
        assert!(d10.as_secs() <= policy.max_delay.as_secs() + 60);
    }

    #[test]
    fn test_provider_config_defaults() {
        let grok = ProviderConfig::grok_default();
        assert_eq!(grok.id, "grok");
        assert!(grok.enabled);

        let or = ProviderConfig::openrouter_default();
        assert_eq!(or.id, "openrouter");
        assert!(!or.enabled);
        assert_eq!(or.base_url, Some("https://openrouter.ai/api/v1".into()));

        let local = ProviderConfig::local_default("ollama", "http://localhost:11434/v1");
        assert!(local.local);
        assert_eq!(local.base_url, Some("http://localhost:11434/v1".into()));
    }
}
