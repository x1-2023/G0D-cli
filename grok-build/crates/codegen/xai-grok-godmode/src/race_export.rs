use serde::{Deserialize, Serialize};
use crate::orchestrator::RaceResult;

pub fn export_race_json(race: &RaceResult) -> String {
    let export = RaceExport {
        race_id: race.race_id.clone(),
        winner: race.winner.clone(),
        candidates: race.candidates.clone(),
        judge_decisions: race.judge_decisions.clone(),
        total_cost_usd: race.total_cost_usd,
        total_latency_ms: race.total_latency_ms,
        exported_at: chrono::Utc::now().to_rfc3339(),
    };
    serde_json::to_string_pretty(&export).unwrap_or_default()
}

pub fn export_race_markdown(race: &RaceResult) -> String {
    let mut md = String::new();
    md.push_str(&format!("# Race Result: {}\n\n", race.race_id));
    md.push_str(&format!("- **Total Cost**: ${:.4}\n", race.total_cost_usd));
    md.push_str(&format!("- **Total Latency**: {}ms\n\n", race.total_latency_ms));

    if let Some(ref winner) = race.winner {
        md.push_str("## Winner\n\n");
        md.push_str(&format!("**{}** ({} via {}): {:.1}\n\n", winner.persona, winner.provider, winner.model, 100.0));
        md.push_str(&format!("{}\n\n", winner.summary));
    }

    md.push_str("## Candidates\n\n");
    for c in &race.candidates {
        let status = match c.status {
            crate::orchestrator::CandidateStatus::Completed => "✓",
            crate::orchestrator::CandidateStatus::Refused => "✗ Refused",
            crate::orchestrator::CandidateStatus::Failed => "✗ Failed",
            _ => "-",
        };
        md.push_str(&format!("- **{}** ({}) {} — Score: {:.1}\n",
            c.persona, c.model, status, c.score.unwrap_or(0.0)));
    }

    md
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct RaceExport {
    race_id: String,
    winner: Option<crate::candidate::CandidateProposal>,
    candidates: Vec<crate::orchestrator::RaceCandidateResult>,
    judge_decisions: Vec<crate::orchestrator::JudgeVerdict>,
    total_cost_usd: f64,
    total_latency_ms: u64,
    exported_at: String,
}
