use crate::judge::JudgeConfig;

pub struct MultiJudge {
    pub judges: Vec<JudgeConfig>,
    pub mode: JudgeMode,
}

pub enum JudgeMode {
    MajorityVote,
    WeightedVote,
    BestOf,
    Consensus,
}

impl Default for MultiJudge {
    fn default() -> Self {
        Self {
            judges: vec![JudgeConfig::default()],
            mode: JudgeMode::MajorityVote,
        }
    }
}
