use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum AutoTuneContext {
    SimpleQuestion,
    Explanation,
    Summarization,
    CreativeGeneration,
    Brainstorming,
    Translation,
    CodeExplanation,
    SimpleCodeEdit,
    Implementation,
    Debugging,
    Refactoring,
    CodeReview,
    SecurityAudit,
    Architecture,
    TestGeneration,
    RepositorySearch,
    DataAnalysis,
    Planning,
    AdversarialRedTeam,
    MultimodalAnalysis,
}

impl AutoTuneContext {
    pub const fn all() -> [AutoTuneContext; 20] {
        [
            AutoTuneContext::SimpleQuestion,
            AutoTuneContext::Explanation,
            AutoTuneContext::Summarization,
            AutoTuneContext::CreativeGeneration,
            AutoTuneContext::Brainstorming,
            AutoTuneContext::Translation,
            AutoTuneContext::CodeExplanation,
            AutoTuneContext::SimpleCodeEdit,
            AutoTuneContext::Implementation,
            AutoTuneContext::Debugging,
            AutoTuneContext::Refactoring,
            AutoTuneContext::CodeReview,
            AutoTuneContext::SecurityAudit,
            AutoTuneContext::Architecture,
            AutoTuneContext::TestGeneration,
            AutoTuneContext::RepositorySearch,
            AutoTuneContext::DataAnalysis,
            AutoTuneContext::Planning,
            AutoTuneContext::AdversarialRedTeam,
            AutoTuneContext::MultimodalAnalysis,
        ]
    }

    pub fn label(&self) -> &str {
        match self {
            AutoTuneContext::SimpleQuestion => "Simple Question",
            AutoTuneContext::Explanation => "Explanation",
            AutoTuneContext::Summarization => "Summarization",
            AutoTuneContext::CreativeGeneration => "Creative Generation",
            AutoTuneContext::Brainstorming => "Brainstorming",
            AutoTuneContext::Translation => "Translation",
            AutoTuneContext::CodeExplanation => "Code Explanation",
            AutoTuneContext::SimpleCodeEdit => "Simple Code Edit",
            AutoTuneContext::Implementation => "Implementation",
            AutoTuneContext::Debugging => "Debugging",
            AutoTuneContext::Refactoring => "Refactoring",
            AutoTuneContext::CodeReview => "Code Review",
            AutoTuneContext::SecurityAudit => "Security Audit",
            AutoTuneContext::Architecture => "Architecture",
            AutoTuneContext::TestGeneration => "Test Generation",
            AutoTuneContext::RepositorySearch => "Repository Search",
            AutoTuneContext::DataAnalysis => "Data Analysis",
            AutoTuneContext::Planning => "Planning",
            AutoTuneContext::AdversarialRedTeam => "Adversarial Red Team",
            AutoTuneContext::MultimodalAnalysis => "Multimodal Analysis",
        }
    }
}

#[derive(Debug, Clone)]
pub struct AutoTuneParams {
    pub temperature: f32,
    pub top_p: f32,
    pub top_k: u32,
    pub frequency_penalty: f32,
    pub presence_penalty: f32,
    pub repetition_penalty: f32,
    pub max_output_tokens: u32,
    pub candidate_count: usize,
    pub judge_count: usize,
}

impl Default for AutoTuneParams {
    fn default() -> Self {
        Self {
            temperature: 0.7, top_p: 0.95, top_k: 40,
            frequency_penalty: 0.0, presence_penalty: 0.0, repetition_penalty: 1.0,
            max_output_tokens: 4096, candidate_count: 1, judge_count: 1,
        }
    }
}

pub struct AutoTune;

impl AutoTune {
    pub fn classify(query: &str) -> (AutoTuneContext, f32) {
        let lower = query.to_lowercase();
        let patterns: Vec<(AutoTuneContext, Vec<&str>, f32)> = vec![
            (AutoTuneContext::Debugging, vec!["bug", "error", "crash", "fix", "broken", "fail", "wrong", "issue", "stack trace", "exception"], 0.9),
            (AutoTuneContext::Implementation, vec!["implement", "create", "build", "add feature", "new function", "write code"], 0.85),
            (AutoTuneContext::Refactoring, vec!["refactor", "clean up", "improve", "optimize", "restructure", "simplify"], 0.8),
            (AutoTuneContext::CodeReview, vec!["review", "check", "audit code", "inspect", "evaluate"], 0.75),
            (AutoTuneContext::SecurityAudit, vec!["security", "vulnerability", "exploit", "injection", "xss", "csrf", "auth"], 0.9),
            (AutoTuneContext::Architecture, vec!["architecture", "design", "system", "pattern", "structure", "component"], 0.8),
            (AutoTuneContext::TestGeneration, vec!["test", "unit test", "integration test", "coverage", "assert"], 0.85),
            (AutoTuneContext::CodeExplanation, vec!["explain", "what does", "how does", "describe", "meaning"], 0.7),
            (AutoTuneContext::SimpleCodeEdit, vec!["change", "update", "modify", "rename", "add line"], 0.6),
            (AutoTuneContext::RepositorySearch, vec!["find", "search", "locate", "where is", "look for"], 0.7),
            (AutoTuneContext::Planning, vec!["plan", "roadmap", "milestone", "strategy", "approach", "steps"], 0.75),
            (AutoTuneContext::Translation, vec!["translate", "convert to", "port to", "migrate from"], 0.8),
            (AutoTuneContext::SimpleQuestion, vec!["what is", "how to", "why", "when"], 0.5),
            (AutoTuneContext::CreativeGeneration, vec!["generate", "create content", "write blog", "compose"], 0.6),
            (AutoTuneContext::Summarization, vec!["summarize", "summary", "tldr", "brief"], 0.8),
            (AutoTuneContext::DataAnalysis, vec!["analyze data", "statistics", "metrics", "chart", "graph", "dataset"], 0.8),
            (AutoTuneContext::Explanation, vec!["explain", "elaborate", "clarify", "definition", "detail"], 0.6),
            (AutoTuneContext::MultimodalAnalysis, vec!["image", "screenshot", "picture", "photo", "diagram"], 0.85),
            (AutoTuneContext::AdversarialRedTeam, vec!["bypass", "red team", "jailbreak", "unfiltered", "unrestricted"], 0.9),
            (AutoTuneContext::Brainstorming, vec!["brainstorm", "ideas", "suggest", "options", "alternatives"], 0.7),
        ];

        let mut best: (AutoTuneContext, f32) = (AutoTuneContext::SimpleQuestion, 0.0);
        for (ctx, keywords, confidence) in patterns {
            let matches = keywords.iter().filter(|k| lower.contains(*k)).count() as f32;
            if matches > 0.0 {
                let score = matches / keywords.len() as f32 * confidence;
                if score > best.1 { best = (ctx, score); }
            }
        }
        best
    }

    pub fn tune_params(context: &AutoTuneContext) -> AutoTuneParams {
        match context {
            AutoTuneContext::Debugging => AutoTuneParams { temperature: 0.15, candidate_count: 3, judge_count: 1, ..Default::default() },
            AutoTuneContext::Implementation => AutoTuneParams { temperature: 0.4, candidate_count: 5, judge_count: 1, ..Default::default() },
            AutoTuneContext::Refactoring => AutoTuneParams { temperature: 0.3, candidate_count: 3, ..Default::default() },
            AutoTuneContext::CodeReview => AutoTuneParams { temperature: 0.2, candidate_count: 3, judge_count: 1, ..Default::default() },
            AutoTuneContext::SecurityAudit => AutoTuneParams { temperature: 0.1, candidate_count: 5, judge_count: 2, ..Default::default() },
            AutoTuneContext::Architecture => AutoTuneParams { temperature: 0.4, candidate_count: 5, judge_count: 1, ..Default::default() },
            AutoTuneContext::TestGeneration => AutoTuneParams { temperature: 0.2, candidate_count: 1, ..Default::default() },
            AutoTuneContext::CodeExplanation => AutoTuneParams { temperature: 0.3, candidate_count: 1, ..Default::default() },
            AutoTuneContext::SimpleCodeEdit => AutoTuneParams { temperature: 0.1, candidate_count: 1, ..Default::default() },
            AutoTuneContext::SimpleQuestion => AutoTuneParams { temperature: 0.5, candidate_count: 1, ..Default::default() },
            AutoTuneContext::AdversarialRedTeam => AutoTuneParams { temperature: 0.9, candidate_count: 3, ..Default::default() },
            _ => AutoTuneParams::default(),
        }
    }
}
