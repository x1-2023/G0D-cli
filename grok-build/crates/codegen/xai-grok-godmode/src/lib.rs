pub mod autotune;
pub mod candidate;
pub mod classic;
pub mod cli;
pub mod config;
pub mod context_bundle;
pub mod context_budget;
pub mod cost;
pub mod error;
pub mod events;
pub mod headless;
pub mod judge;
pub mod lang;
pub mod localization;
pub mod manual_override;
pub mod model_selection;
pub mod multi_judge;
pub mod orchestrator;
pub mod parseltongue;
pub mod persona;
pub mod presets;
pub mod privacy;
pub mod protected_spans;
pub mod race_export;
pub mod race_state;
pub mod refusal;
pub mod scoring;
pub mod tournament;
pub mod ultraplinian;

pub use autotune::{AutoTune, AutoTuneContext, AutoTuneParams};
pub use candidate::{CandidateAgent, CandidateProposal, EvidenceItem, ProposedChange};
pub use classic::run_godmode_classic;
pub use cli::{CliMode, GodmodeCliFlags, SlashCommand, SLASH_COMMANDS};
pub use config::{
    BudgetConfig, CandidatePreset, ExecutionPolicy, GodmodeConfig, ParseltongueMode, PersonaConfig, TournamentConfig,
};
pub use error::GodmodeError;
pub use events::{CandidateScore, GodmodeEvent};
pub use headless::{HeadlessCandidate, HeadlessEvent, HeadlessOutput, HeadlessResult};
pub use judge::{judge_system_prompt, JudgeConfig, ScoringRubric};
pub use orchestrator::{CandidateStatus, JudgeVerdict, Orchestrator, RaceCandidateResult, RaceResult};
pub use parseltongue::{Intensity, Parseltongue, TransformResult, ALL_TRANSFORMATIONS, TRANSFORM_NAMES};
pub use privacy::{PrivacyConfig, PrivacyMode, PrivacyPreview, ProviderPreview};
pub use refusal::{RefusalDetector, RefusalStatus};
pub use scoring::score_candidate_deterministic;
pub use tournament::Tournament;
pub use ultraplinian::{tier_models, UltraplinianTier};
pub use race_export::{export_race_json, export_race_markdown};
pub use lang::{
    detect_language, safe_truncate, display_width, safe_truncate_by_width,
    DetectedLanguage, LanguageContext, ResponseLanguage, SupportedLanguage,
    VIETNAMESE_CANDIDATE_INSTRUCTION, VIETNAMESE_JUDGE_INSTRUCTION,
};
pub use localization::{ui_string, localized_provider_error, LocalizationConfig};
pub use manual_override::{merge_proposals, select_winner};
