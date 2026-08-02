use serde::{Deserialize, Serialize};
use chrono::{DateTime, Utc};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "kebab-case")]
pub enum GodmodeEvent {
    RaceCreated { race_id: String, timestamp: DateTime<Utc> },
    RaceStarted { race_id: String, tier: String, candidate_count: usize },
    ContextBundleCreated { race_id: String, file_count: usize, estimated_tokens: u64 },
    PrivacyPreviewRequired { race_id: String, providers: Vec<String>, models: Vec<String>, token_count: u64 },
    BudgetPreviewCreated { race_id: String, estimated_cost_usd: f64, estimated_tokens: u64 },
    ProviderStatusChanged { provider: String, status: String },
    CandidateQueued { race_id: String, candidate_id: String },
    CandidateStarted { race_id: String, candidate_id: String, provider: String, model: String, persona: String },
    CandidateCompleted { race_id: String, candidate_id: String, provider: String, model: String, latency_ms: u64, tokens: u64, score: Option<f64> },
    CandidateFailed { race_id: String, candidate_id: String, provider: String, model: String, error: String },
    CandidateRefused { race_id: String, candidate_id: String, reason: String },
    CandidateDisqualified { race_id: String, candidate_id: String, reason: String },
    TournamentRoundStarted { race_id: String, round: usize, group_count: usize },
    TournamentRoundCompleted { race_id: String, round: usize, promoted_count: usize },
    JudgingStarted { race_id: String, judge_id: String, candidate_count: usize },
    JudgeCompleted { race_id: String, judge_id: String, winner_id: String, reason: String },
    CandidateScored { race_id: String, candidate_id: String, score: CandidateScore, judge_id: String },
    WinnerSelected { race_id: String, candidate_id: String, provider: String, model: String, score: f64 },
    WinnerOverridden { race_id: String, from_candidate_id: String, to_candidate_id: String },
    ProposalsMerged { race_id: String, source_candidates: Vec<String> },
    ExecutionStarted { race_id: String, candidate_id: String },
    ExecutionCompleted { race_id: String, candidate_id: String, build_success: bool, test_success: bool },
    BudgetWarning { race_id: String, limit: String, current: String },
    RaceCancelled { race_id: String, reason: String },
    RaceCompleted { race_id: String, winner_id: String, total_latency_ms: u64, total_cost_usd: f64 },
    RaceExported { race_id: String, format: String, path: String },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CandidateScore {
    pub correctness: f64,
    pub repository_evidence: f64,
    pub architecture_fit: f64,
    pub minimal_change: f64,
    pub testability: f64,
    pub security: f64,
    pub regression_risk: f64,
    pub clarity: f64,
    pub performance: f64,
    pub maintainability: f64,
    pub language_compliance: f64,
    pub total: f64,
}

impl CandidateScore {
    pub fn compute_total(&mut self) {
        self.total = self.correctness + self.repository_evidence + self.architecture_fit
            + self.minimal_change + self.testability + self.security
            + self.regression_risk + self.clarity + self.performance + self.maintainability
            + self.language_compliance;
    }
}
