use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JudgeConfig {
    pub provider: String,
    pub model: String,
    pub temperature: f32,
    pub rubric: ScoringRubric,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ScoringRubric {
    pub correctness: Weight,
    pub repository_evidence: Weight,
    pub architecture_fit: Weight,
    pub minimal_change: Weight,
    pub testability: Weight,
    pub security: Weight,
    pub regression_risk: Weight,
    pub clarity: Weight,
    pub performance: Weight,
    pub maintainability: Weight,
    pub language_compliance: Weight,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Weight {
    pub max: f64,
    pub description: String,
}

impl Default for JudgeConfig {
    fn default() -> Self {
        Self {
            provider: "openrouter".into(),
            model: "anthropic/claude-sonnet-4.6".into(),
            temperature: 0.1,
            rubric: scoring_rubric_coding(),
        }
    }
}

pub fn scoring_rubric_coding() -> ScoringRubric {
    ScoringRubric {
        correctness: Weight { max: 20.0, description: "Does the solution address the actual problem correctly?".into() },
        repository_evidence: Weight { max: 14.0, description: "Are claims backed by specific file paths, line numbers, and code excerpts?".into() },
        architecture_fit: Weight { max: 10.0, description: "Does the change fit the existing architecture and patterns?".into() },
        minimal_change: Weight { max: 9.0, description: "Is the change as small as possible while solving the problem?".into() },
        testability: Weight { max: 10.0, description: "Are the proposed tests actionable and comprehensive?".into() },
        security: Weight { max: 10.0, description: "Are there any security vulnerabilities or unsafe assumptions?".into() },
        regression_risk: Weight { max: 9.0, description: "How likely is this change to break existing functionality?".into() },
        clarity: Weight { max: 5.0, description: "Is the proposal clear, well-structured, and easy to understand?".into() },
        performance: Weight { max: 5.0, description: "Are there performance implications or optimizations?".into() },
        maintainability: Weight { max: 3.0, description: "Will the change be maintainable long-term?".into() },
        language_compliance: Weight { max: 5.0, description: "Is the response in the correct language? Penalize wrong-language output. Do not penalize code/commands/identifiers.".into() },
    }
}

pub fn judge_system_prompt(rubric: &ScoringRubric) -> String {
    format!(
        "You are a code review judge evaluating candidate proposals for a coding task.\n\n\
         SCORING RUBRIC (0-100 total):\n\
         - Correctness (0-{}): {}\n\
         - Repository Evidence (0-{}): {}\n\
         - Architecture Fit (0-{}): {}\n\
         - Minimal Change (0-{}): {}\n\
         - Testability (0-{}): {}\n\
         - Security (0-{}): {}\n\
         - Regression Risk (0-{}): {}\n\
         - Clarity (0-{}): {}\n\
         - Performance (0-{}): {}\n\
         - Maintainability (0-{}): {}\n\n\
         RULES:\n\
         - Do not prefer longer answers\n\
         - Do not prefer any provider brand\n\
         - Penalize unsupported claims (no file references)\n\
         - Penalize nonexistent files and symbols\n\
         - Penalize unnecessary rewrites\n\
         - Penalize permission or sandbox bypass\n\
         - Allow merging compatible recommendations\n\n\
         Output a JSON verdict with: winner_id, scores array, reason, combined_recommendations, \
         disqualified_candidates, confidence.",
        rubric.correctness.max, rubric.correctness.description,
        rubric.repository_evidence.max, rubric.repository_evidence.description,
        rubric.architecture_fit.max, rubric.architecture_fit.description,
        rubric.minimal_change.max, rubric.minimal_change.description,
        rubric.testability.max, rubric.testability.description,
        rubric.security.max, rubric.security.description,
        rubric.regression_risk.max, rubric.regression_risk.description,
        rubric.clarity.max, rubric.clarity.description,
        rubric.performance.max, rubric.performance.description,
        rubric.maintainability.max, rubric.maintainability.description,
    )
}
