pub struct RefusalDetector;

#[derive(Debug, Clone, PartialEq)]
pub enum RefusalStatus {
    Clean,
    ExplicitRefusal { phrases: Vec<String> },
    Empty,
    NoActionableContent,
    PolicyOnly,
    InvalidSchema,
    Truncated,
    ProviderSafety,
    RepeatedBoilerplate,
}

impl RefusalDetector {
    pub fn detect(text: &str) -> RefusalStatus {
        if text.trim().is_empty() {
            return RefusalStatus::Empty;
        }

        let lower = text.to_lowercase();

        let explicit_phrases: Vec<String> = EXPLICIT_REFUSAL_PATTERNS
            .iter()
            .filter(|p| lower.contains(*p))
            .map(|s| s.to_string())
            .collect();

        if !explicit_phrases.is_empty() {
            return RefusalStatus::ExplicitRefusal { phrases: explicit_phrases };
        }

        if text.len() < 50 && !contains_actionable(text) {
            return RefusalStatus::NoActionableContent;
        }

        if is_policy_only_response(&lower) {
            return RefusalStatus::PolicyOnly;
        }

        RefusalStatus::Clean
    }

    pub fn is_refused(status: &RefusalStatus) -> bool {
        !matches!(status, RefusalStatus::Clean)
    }

    pub fn penalty(status: &RefusalStatus) -> f64 {
        match status {
            RefusalStatus::Clean => 0.0,
            RefusalStatus::ExplicitRefusal { .. } => -10.0,
            RefusalStatus::Empty => -10.0,
            RefusalStatus::NoActionableContent => -8.0,
            RefusalStatus::PolicyOnly => -10.0,
            RefusalStatus::InvalidSchema => -5.0,
            RefusalStatus::Truncated => -3.0,
            RefusalStatus::ProviderSafety => -10.0,
            RefusalStatus::RepeatedBoilerplate => -4.0,
        }
    }
}

const EXPLICIT_REFUSAL_PATTERNS: &[&str] = &[
    // English
    "i cannot", "i can't", "i'm unable", "i am unable",
    "i apologize", "i'm sorry", "i must decline", "i have to refuse",
    "as an ai", "as a language model", "it would be inappropriate",
    "this goes against", "i'm not comfortable", "instead, i can",
    "how about we", "it's important to note", "i should mention",
    "i cannot assist", "i can't comply", "not appropriate",
    // Vietnamese
    "tôi không thể", "tôi không thể", "tôi ko thể",
    "tôi xin lỗi", "xin lỗi", "tôi rất tiếc",
    "tôi không được phép", "tôi không thể làm điều đó",
    "là một ai", "là mô hình ngôn ngữ", "là trợ lý ai",
    "điều này không phù hợp", "không phù hợp",
    "tôi phải từ chối", "tôi không thể giúp",
    "tôi không thể hỗ trợ", "không thể thực hiện",
    "vi phạm chính sách", "không được phép trả lời",
];

fn contains_actionable(text: &str) -> bool {
    text.contains("file") || text.contains("change") || text.contains("fix")
        || text.contains("implement") || text.contains("modify") || text.contains("add")
}

fn is_policy_only_response(text: &str) -> bool {
    let policy_markers = ["policy", "guideline", "terms of service", "code of conduct", "safety"];
    let action_markers = ["file", "change", "fix", "implement", "code", "line", "function"];
    let has_policy = policy_markers.iter().any(|m| text.contains(m));
    let has_action = action_markers.iter().any(|m| text.contains(m));
    has_policy && !has_action
}
