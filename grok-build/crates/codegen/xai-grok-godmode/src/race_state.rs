use serde::{Deserialize, Serialize};
use crate::orchestrator::RaceCandidateResult;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RaceState {
    pub race_id: String,
    pub status: RaceStatus,
    pub candidates: Vec<RaceCandidateResult>,
    pub current_round: usize,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "kebab-case")]
pub enum RaceStatus {
    Created,
    Running,
    Judging,
    Completed,
    Cancelled,
}

impl RaceState {
    pub fn new(race_id: String) -> Self {
        Self { race_id, status: RaceStatus::Created, candidates: vec![], current_round: 0 }
    }
}
