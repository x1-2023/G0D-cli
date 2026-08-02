#[derive(Debug, Clone, Default)]
pub struct CostTracker {
    pub total_input_tokens: u64,
    pub total_output_tokens: u64,
    pub total_cost_usd: f64,
    pub per_provider: std::collections::HashMap<String, ProviderCost>,
}

#[derive(Debug, Clone, Default)]
pub struct ProviderCost {
    pub input_tokens: u64,
    pub output_tokens: u64,
    pub cost_usd: f64,
}

impl CostTracker {
    pub fn new() -> Self { Self::default() }

    pub fn add_usage(&mut self, provider: &str, input: u64, output: u64, cost_per_1m_input: f64, cost_per_1m_output: f64) {
        let cost = (input as f64 / 1_000_000.0) * cost_per_1m_input + (output as f64 / 1_000_000.0) * cost_per_1m_output;
        self.total_input_tokens += input;
        self.total_output_tokens += output;
        self.total_cost_usd += cost;
        let entry = self.per_provider.entry(provider.to_string()).or_default();
        entry.input_tokens += input;
        entry.output_tokens += output;
        entry.cost_usd += cost;
    }

    pub fn estimate_cost(input: u64, output: u64, input_price: f64, output_price: f64) -> f64 {
        (input as f64 * input_price + output as f64 * output_price) / 1_000_000.0
    }
}
