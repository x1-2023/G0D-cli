pub fn select_models_for_race(
    tier: &crate::ultraplinian::UltraplinianTier,
    _available_providers: &[String],
) -> Vec<(String, String)> {
    crate::ultraplinian::tier_models(tier)
        .iter()
        .filter_map(|m| m.split_once(':').map(|(p, m)| (p.to_string(), m.to_string())))
        .collect()
}

pub fn filter_by_capability(
    models: Vec<(String, String)>,
    _required_caps: &[&str],
) -> Vec<(String, String)> {
    models
}
