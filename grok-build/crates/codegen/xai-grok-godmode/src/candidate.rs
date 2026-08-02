use serde::{Deserialize, Serialize};
use crate::config::CandidatePreset;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CandidateAgent {
    pub id: String,
    pub preset: CandidatePreset,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CandidateProposal {
    pub candidate_id: String,
    pub provider: String,
    pub model: String,
    pub persona: String,
    pub summary: String,
    pub diagnosis: String,
    pub evidence: Vec<EvidenceItem>,
    pub files_to_change: Vec<String>,
    pub symbols_to_change: Vec<String>,
    pub proposed_changes: Vec<ProposedChange>,
    pub proposed_patch: Option<String>,
    pub commands_to_run: Vec<String>,
    pub tests: Vec<String>,
    pub risks: Vec<String>,
    pub assumptions: Vec<String>,
    pub limitations: Vec<String>,
    pub confidence: f32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EvidenceItem {
    pub file_path: String,
    pub line_start: Option<usize>,
    pub line_end: Option<usize>,
    pub excerpt: String,
    pub relevance: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProposedChange {
    pub file_path: String,
    pub description: String,
    pub diff_hunk: Option<String>,
    pub rationale: String,
}

impl CandidateAgent {
    pub fn new(preset: CandidatePreset) -> Self {
        let id = format!("{}-{}", preset.id, uuid::Uuid::new_v4().to_string().split('-').next().unwrap_or("0"));
        Self { id, preset }
    }

    pub fn permission_allowlist(&self) -> Vec<String> {
        self.preset.persona.allowed_tools.clone()
    }

    pub fn permission_denylist(&self) -> Vec<String> {
        self.preset.persona.denied_tools.clone()
    }

    pub fn system_instruction(&self) -> String {
        format!(
            "You are the {} persona: {}. {}\n\nYou are a candidate agent in a multi-model coding race. \
             Analyze the task and repository, then produce a structured proposal with evidence, \
             diagnosis, and recommended changes. Be specific and cite exact file paths and line numbers.\n\n\
             FORMAT: Output a JSON proposal with fields: summary, diagnosis, evidence, files_to_change, \
             symbols_to_change, proposed_changes, tests, risks, assumptions, limitations, confidence.",
            self.preset.persona.name,
            self.preset.persona.role,
            self.preset.persona.instruction,
        )
    }
}
