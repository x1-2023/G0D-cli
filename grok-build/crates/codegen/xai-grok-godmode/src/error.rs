
#[derive(Debug, thiserror::Error)]
pub enum GodmodeError {
    #[error("provider error: {0}")]
    Provider(#[from] xai_grok_providers::ProviderError),

    #[error("no candidates configured")]
    NoCandidates,

    #[error("all candidates failed: {details}")]
    AllCandidatesFailed { details: String },

    #[error("all judges failed")]
    AllJudgesFailed,

    #[error("budget exceeded: {limit}: {current}")]
    BudgetExceeded { limit: String, current: String },

    #[error("context too large: {requested} > {limit}")]
    ContextTooLarge { requested: usize, limit: usize },

    #[error("cost limit exceeded: ${estimated} > ${limit}")]
    CostLimit { estimated: f64, limit: f64 },

    #[error("timeout: {elapsed:?}")]
    Timeout { elapsed: std::time::Duration },

    #[error("invalid candidate output: {detail}")]
    InvalidCandidate { candidate_id: String, detail: String },

    #[error("invalid judge output: {detail}")]
    InvalidJudge { detail: String },

    #[error("race cancelled")]
    RaceCancelled,

    #[error("unsupported capability for candidate {candidate}: {capability}")]
    UnsupportedCapability { candidate: String, capability: String },

    #[error("configuration error: {detail}")]
    Config { detail: String },

    #[error("credential error: {detail}")]
    Credential { detail: String },

    #[error("internal error: {detail}")]
    Internal { detail: String },
}
