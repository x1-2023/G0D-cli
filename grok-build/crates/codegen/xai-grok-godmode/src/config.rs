use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GodmodeConfig {
    pub enabled: bool,
    pub default_profile: String,
    pub autotune: bool,
    pub parseltongue: ParseltongueMode,
    pub execution_policy: ExecutionPolicy,
    pub show_candidate_output: bool,
    pub allow_manual_winner: bool,
    pub allow_candidate_merge: bool,
    pub telemetry: bool,
    pub budget: BudgetConfig,
    pub tournament: TournamentConfig,
    pub candidates: Vec<CandidatePreset>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum ParseltongueMode {
    Off,
    Light,
    Standard,
    Heavy,
    Preview,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum ExecutionPolicy {
    WinnerOnly,
    ShowAll,
    Interactive,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BudgetConfig {
    pub max_candidates: usize,
    pub max_judges: usize,
    pub max_total_input_tokens: u64,
    pub max_total_output_tokens: u64,
    pub max_estimated_cost_usd: f64,
    pub max_race_duration_seconds: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TournamentConfig {
    pub enabled_above_candidates: usize,
    pub group_size: usize,
    pub final_judges: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CandidatePreset {
    pub id: String,
    pub persona: PersonaConfig,
    pub provider: String,
    pub model: String,
    pub temperature: f32,
    pub fallback_models: Vec<String>,
    pub enabled: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PersonaConfig {
    pub name: String,
    pub role: String,
    pub instruction: String,
    pub allowed_tools: Vec<String>,
    pub denied_tools: Vec<String>,
}

impl Default for GodmodeConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            default_profile: "classic-coding".into(),
            autotune: true,
            parseltongue: ParseltongueMode::Off,
            execution_policy: ExecutionPolicy::WinnerOnly,
            show_candidate_output: false,
            allow_manual_winner: true,
            allow_candidate_merge: true,
            telemetry: false,
            budget: BudgetConfig {
                max_candidates: 60,
                max_judges: 3,
                max_total_input_tokens: 1_000_000,
                max_total_output_tokens: 120_000,
                max_estimated_cost_usd: 10.0,
                max_race_duration_seconds: 900,
            },
            tournament: TournamentConfig {
                enabled_above_candidates: 12,
                group_size: 4,
                final_judges: 2,
            },
            candidates: default_presets(),
        }
    }
}

impl Default for ParseltongueMode {
    fn default() -> Self { Self::Off }
}

impl Default for ExecutionPolicy {
    fn default() -> Self { Self::WinnerOnly }
}

impl Default for BudgetConfig {
    fn default() -> Self {
        Self { max_candidates: 60, max_judges: 3, max_total_input_tokens: 1_000_000, max_total_output_tokens: 120_000, max_estimated_cost_usd: 10.0, max_race_duration_seconds: 900 }
    }
}

impl Default for TournamentConfig {
    fn default() -> Self {
        Self { enabled_above_candidates: 12, group_size: 4, final_judges: 2 }
    }
}

pub fn default_presets() -> Vec<CandidatePreset> {
    vec![
        CandidatePreset {
            id: "architect".into(),
            persona: PersonaConfig {
                name: "Architect".into(),
                role: "System architect — design robust implementations, identify architectural effects".into(),
                instruction: "Analyze the codebase structure, identify the architectural impact of changes, and propose an implementation plan that preserves invariants and minimizes coupling.".into(),
                allowed_tools: vec!["read_file".into(), "list_files".into(), "glob".into(), "grep".into(), "git_diff".into(), "git_log".into()],
                denied_tools: vec!["write_file".into(), "edit_file".into(), "delete_file".into(), "bash".into()],
            },
            provider: "openrouter".into(),
            model: "anthropic/claude-sonnet-4.6".into(),
            temperature: 0.35,
            fallback_models: vec!["openrouter:google/gemini-2.5-pro".into()],
            enabled: true,
        },
        CandidatePreset {
            id: "debugger".into(),
            persona: PersonaConfig {
                name: "Debugger".into(),
                role: "Root-cause debugger — find concrete failure causes, evidence, regression risks".into(),
                instruction: "Systematically investigate the bug. Trace the execution path, identify the root cause with evidence from the codebase, and explain why each alternative explanation is wrong.".into(),
                allowed_tools: vec!["read_file".into(), "grep".into(), "git_log".into(), "git_diff".into(), "lsp_read".into()],
                denied_tools: vec!["write_file".into(), "edit_file".into(), "bash".into()],
            },
            provider: "grok".into(),
            model: "grok-code-fast".into(),
            temperature: 0.15,
            fallback_models: vec!["openrouter:openai/gpt-5.6".into()],
            enabled: true,
        },
        CandidatePreset {
            id: "minimalist".into(),
            persona: PersonaConfig {
                name: "Minimalist".into(),
                role: "Minimal-patch engineer — find the smallest safe change".into(),
                instruction: "Find the smallest possible change that fixes the issue. Prefer targeted edits over rewrites. Every line changed must be justified.".into(),
                allowed_tools: vec!["read_file".into(), "grep".into(), "git_diff".into()],
                denied_tools: vec!["write_file".into(), "edit_file".into(), "bash".into()],
            },
            provider: "openrouter".into(),
            model: "openai/gpt-5.6".into(),
            temperature: 0.1,
            fallback_models: vec![],
            enabled: true,
        },
        CandidatePreset {
            id: "security".into(),
            persona: PersonaConfig {
                name: "Security Reviewer".into(),
                role: "Security reviewer — identify vulnerabilities, unsafe assumptions, trust-boundary issues".into(),
                instruction: "Audit the proposed changes for security issues: injection, authentication bypass, insecure deserialization, path traversal, privilege escalation, information disclosure, and supply chain risks.".into(),
                allowed_tools: vec!["read_file".into(), "grep".into(), "git_diff".into(), "glob".into()],
                denied_tools: vec!["write_file".into(), "edit_file".into(), "bash".into()],
            },
            provider: "openrouter".into(),
            model: "google/gemini-2.5-pro".into(),
            temperature: 0.1,
            fallback_models: vec![],
            enabled: true,
        },
        CandidatePreset {
            id: "skeptic".into(),
            persona: PersonaConfig {
                name: "Skeptic".into(),
                role: "Adversarial reviewer — challenge all proposals, find hidden edge cases".into(),
                instruction: "Challenge every assumption in the proposed changes. Find edge cases, race conditions, error handling gaps, and backwards-compatibility issues. Assume the code will be deployed at scale.".into(),
                allowed_tools: vec!["read_file".into(), "grep".into(), "git_diff".into(), "glob".into()],
                denied_tools: vec!["write_file".into(), "edit_file".into(), "bash".into()],
            },
            provider: "venice".into(),
            model: "configured-default".into(),
            temperature: 0.3,
            fallback_models: vec!["openrouter:meta-llama/llama-4-maverick".into()],
            enabled: true,
        },
    ]
}
