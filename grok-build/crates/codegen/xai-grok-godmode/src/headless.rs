use serde::{Deserialize, Serialize};
use crate::orchestrator::RaceCandidateResult;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HeadlessOutput {
    pub session_id: String,
    pub events: Vec<HeadlessEvent>,
    pub result: Option<HeadlessResult>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "kebab-case")]
pub enum HeadlessEvent {
    RaceStarted { race_id: String, tier: String, mode: String, candidate_count: usize },
    CandidateUpdate { candidate_id: String, provider: String, model: String, status: String, elapsed_ms: u64 },
    CandidateComplete { candidate_id: String, provider: String, model: String, token_count: u64, score: Option<f64> },
    CandidateError { candidate_id: String, provider: String, model: String, error: String },
    JudgeVerdict { winner_id: String, reason: String, confidence: f32 },
    RaceComplete { winner_id: String, total_cost_usd: f64, total_latency_ms: u64 },
    RaceCancelled { reason: String },
    BudgetWarning { limit: String, current: String },
    PrivacyPreview { providers: Vec<String>, estimated_tokens: u64 },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HeadlessResult {
    pub winner: Option<HeadlessCandidate>,
    pub candidates: Vec<HeadlessCandidate>,
    pub total_cost_usd: f64,
    pub total_latency_ms: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HeadlessCandidate {
    pub id: String,
    pub provider: String,
    pub model: String,
    pub persona: String,
    pub summary: String,
    pub files_to_change: Vec<String>,
    pub score: Option<f64>,
    pub status: String,
    pub token_count: u64,
}

impl HeadlessCandidate {
    pub fn from_race_result(r: &RaceCandidateResult) -> Self {
        Self {
            id: r.candidate_id.clone(),
            provider: r.provider.clone(),
            model: r.model.clone(),
            persona: r.persona.clone(),
            summary: r.proposal.as_ref().map(|p| p.summary.clone()).unwrap_or_default(),
            files_to_change: r.proposal.as_ref().map(|p| p.files_to_change.clone()).unwrap_or_default(),
            score: r.score,
            status: format!("{:?}", r.status),
            token_count: r.tokens_used,
        }
    }
}

impl HeadlessOutput {
    pub fn new(session_id: String) -> Self {
        Self { session_id, events: vec![], result: None }
    }

    pub fn to_json(&self) -> String {
        serde_json::to_string_pretty(self).unwrap_or_default()
    }

    pub fn push_event(&mut self, event: HeadlessEvent) {
        self.events.push(event);
    }
}
