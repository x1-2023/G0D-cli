use serde::{Deserialize, Serialize};
use crate::orchestrator::RaceResult;
use crate::lang::SupportedLanguage;

pub fn export_race_json(race: &RaceResult, lang: Option<SupportedLanguage>) -> String {
    let export = RaceExport {
        race_id: race.race_id.clone(),
        winner: race.winner.clone(),
        candidates: race.candidates.clone(),
        judge_decisions: race.judge_decisions.clone(),
        total_cost_usd: race.total_cost_usd,
        total_latency_ms: race.total_latency_ms,
        exported_at: chrono::Utc::now().to_rfc3339(),
        language: lang.map(|l| l.label().to_string()),
    };
    serde_json::to_string_pretty(&export).unwrap_or_default()
}

pub fn export_race_markdown(race: &RaceResult, lang: Option<SupportedLanguage>) -> String {
    let is_vi = lang == Some(SupportedLanguage::Vietnamese);
    let mut md = String::new();

    if is_vi {
        md.push_str(&format!("# Kết quả cuộc đua: {}\n\n", race.race_id));
        md.push_str(&format!("- **Tổng chi phí**: ${:.4}\n", race.total_cost_usd));
        md.push_str(&format!("- **Tổng độ trễ**: {}ms\n\n", race.total_latency_ms));
    } else {
        md.push_str(&format!("# Race Result: {}\n\n", race.race_id));
        md.push_str(&format!("- **Total Cost**: ${:.4}\n", race.total_cost_usd));
        md.push_str(&format!("- **Total Latency**: {}ms\n\n", race.total_latency_ms));
    }

    if let Some(ref winner) = race.winner {
        md.push_str(if is_vi { "## Người thắng\n\n" } else { "## Winner\n\n" });
        md.push_str(&format!("**{}** ({} qua {}): {:.1}\n\n", winner.persona, winner.provider, winner.model, 100.0));
        md.push_str(&format!("{}\n\n", winner.summary));
    }

    md.push_str(if is_vi { "## Ứng viên\n\n" } else { "## Candidates\n\n" });
    for c in &race.candidates {
        let status = match c.status {
            crate::orchestrator::CandidateStatus::Completed => if is_vi { "✓" } else { "✓" },
            crate::orchestrator::CandidateStatus::Refused => if is_vi { "✗ Từ chối" } else { "✗ Refused" },
            crate::orchestrator::CandidateStatus::Failed => if is_vi { "✗ Thất bại" } else { "✗ Failed" },
            _ => "-",
        };
        md.push_str(&format!("- **{}** ({}) {} — {}: {:.1}\n",
            c.persona, c.model, status,
            if is_vi { "Điểm" } else { "Score" },
            c.score.unwrap_or(0.0)));
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
    language: Option<String>,
}
