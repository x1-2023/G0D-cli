use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub struct ModelRef {
    pub provider: String,
    pub model_id: String,
}

impl ModelRef {
    pub fn new(provider: impl Into<String>, model_id: impl Into<String>) -> Self {
        Self { provider: provider.into(), model_id: model_id.into() }
    }

    pub fn canonical(&self) -> String {
        format!("{}:{}", self.provider, self.model_id)
    }
}

impl std::str::FromStr for ModelRef {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.split_once(':') {
            Some((provider, model_id)) => {
                if provider.is_empty() || model_id.is_empty() {
                    Err("model ref must be in provider:model/id format".into())
                } else {
                    Ok(Self::new(provider, model_id))
                }
            }
            None => Err("model ref must contain ':' separator (e.g. openrouter:anthropic/claude-sonnet-4.6)".into()),
        }
    }
}

impl std::fmt::Display for ModelRef {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}:{}", self.provider, self.model_id)
    }
}
