use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ContextBundle {
    pub task: String,
    pub repository_summary: String,
    pub selected_files: Vec<String>,
    pub selected_symbols: Vec<String>,
    pub git_status: Option<String>,
    pub git_diff: Option<String>,
    pub diagnostics: Vec<String>,
    pub test_failures: Vec<String>,
    pub constraints: Vec<String>,
    pub tool_catalog: Vec<String>,
    pub privacy_metadata: PrivacyMetadata,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PrivacyMetadata {
    pub contains_secrets: bool,
    pub remote_providers: Vec<String>,
    pub estimated_tokens: u64,
    pub excluded_files: Vec<String>,
}

impl Default for ContextBundle {
    fn default() -> Self {
        Self {
            task: String::new(), repository_summary: String::new(),
            selected_files: vec![], selected_symbols: vec![],
            git_status: None, git_diff: None,
            diagnostics: vec![], test_failures: vec![],
            constraints: vec![], tool_catalog: vec![],
            privacy_metadata: PrivacyMetadata {
                contains_secrets: false, remote_providers: vec![],
                estimated_tokens: 0, excluded_files: vec![],
            },
        }
    }
}
