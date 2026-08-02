use serde::{Deserialize, Serialize};
use tokio::sync::mpsc;
use crate::config::GodmodeConfig;
use crate::events::GodmodeEvent;
use crate::candidate::CandidateProposal;

pub struct Orchestrator {
    config: GodmodeConfig,
    event_tx: mpsc::UnboundedSender<GodmodeEvent>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RaceResult {
    pub race_id: String,
    pub winner: Option<CandidateProposal>,
    pub candidates: Vec<RaceCandidateResult>,
    pub judge_decisions: Vec<JudgeVerdict>,
    pub total_cost_usd: f64,
    pub total_latency_ms: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RaceCandidateResult {
    pub candidate_id: String,
    pub provider: String,
    pub model: String,
    pub persona: String,
    pub proposal: Option<CandidateProposal>,
    pub score: Option<f64>,
    pub status: CandidateStatus,
    pub latency_ms: u64,
    pub tokens_used: u64,
    pub estimated_cost_usd: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "kebab-case")]
pub enum CandidateStatus {
    Queued,
    Running,
    Completed,
    Refused,
    Failed,
    Disqualified,
    Cancelled,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JudgeVerdict {
    pub judge_id: String,
    pub winner_id: String,
    pub scores: Vec<(String, f64)>,
    pub reason: String,
    pub combined_recommendations: Vec<String>,
    pub disqualified_candidates: Vec<String>,
    pub confidence: f32,
}

impl Orchestrator {
    pub fn new(config: GodmodeConfig, event_tx: mpsc::UnboundedSender<GodmodeEvent>) -> Self {
        Self { config, event_tx }
    }

    pub fn event_tx(&self) -> &mpsc::UnboundedSender<GodmodeEvent> {
        &self.event_tx
    }

    pub fn config(&self) -> &GodmodeConfig {
        &self.config
    }

    pub fn emit_event(&self, event: GodmodeEvent) {
        let _ = self.event_tx.send(event);
    }
}
