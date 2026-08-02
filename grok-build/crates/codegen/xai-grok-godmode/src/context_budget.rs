pub struct ContextBudget {
    pub max_input_tokens: u64,
    pub max_output_tokens: u64,
    pub used_input_tokens: u64,
    pub used_output_tokens: u64,
}

impl ContextBudget {
    pub fn new(max_input: u64, max_output: u64) -> Self {
        Self { max_input_tokens: max_input, max_output_tokens: max_output, used_input_tokens: 0, used_output_tokens: 0 }
    }

    pub fn remaining_input(&self) -> u64 { self.max_input_tokens.saturating_sub(self.used_input_tokens) }
    pub fn remaining_output(&self) -> u64 { self.max_output_tokens.saturating_sub(self.used_output_tokens) }
    pub fn is_exhausted(&self) -> bool { self.remaining_input() == 0 || self.remaining_output() == 0 }
}
