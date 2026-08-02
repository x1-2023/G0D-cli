use crate::candidate::CandidateProposal;
use crate::judge::ScoringRubric;

pub fn score_candidate_deterministic(
    proposal: &CandidateProposal,
    rubric: &ScoringRubric,
    expected_lang: Option<crate::lang::SupportedLanguage>,
) -> crate::events::CandidateScore {
    let correctness = rubric.correctness.max * evidence_quality(proposal);
    let evidence = rubric.repository_evidence.max * evidence_quality(proposal);
    let architecture = rubric.architecture_fit.max * (1.0 - risky_pattern_count(proposal) * 0.1).max(0.0);
    let minimal = rubric.minimal_change.max * minimality_score(proposal);
    let testability = rubric.testability.max * test_coverage_score(proposal);
    let security = rubric.security.max * security_score(proposal);
    let regression = rubric.regression_risk.max * (1.0 - proposal.risks.len() as f64 * 0.15).max(0.0);
    let clarity = rubric.clarity.max * clarity_score(proposal);
    let performance = rubric.performance.max * 0.5;
    let maintainability = rubric.maintainability.max * minimality_score(proposal);
    let language = rubric.language_compliance.max * language_score(proposal, expected_lang);

    let mut score = crate::events::CandidateScore {
        correctness, repository_evidence: evidence, architecture_fit: architecture,
        minimal_change: minimal, testability, security, regression_risk: regression,
        clarity, performance, maintainability, language_compliance: language, total: 0.0,
    };
    score.compute_total();
    score
}

fn evidence_quality(p: &CandidateProposal) -> f64 {
    let has_evidence = !p.evidence.is_empty();
    let has_files = !p.files_to_change.is_empty();
    let has_symbols = !p.symbols_to_change.is_empty();
    let base = if has_evidence && has_files { 0.7 } else if has_evidence { 0.4 } else { 0.1 };
    (base + has_files as u8 as f64 * 0.15 + has_symbols as u8 as f64 * 0.15).min(1.0)
}

fn risky_pattern_count(p: &CandidateProposal) -> f64 {
    p.risks.len() as f64 * 0.5 + if p.assumptions.len() > 3 { 1.0 } else { 0.0 }
}

fn minimality_score(p: &CandidateProposal) -> f64 {
    let files = p.files_to_change.len().max(1) as f64;
    (1.0 / files.sqrt()).min(1.0)
}

fn test_coverage_score(p: &CandidateProposal) -> f64 {
    if p.tests.is_empty() { 0.1 } else { (p.tests.len() as f64 * 0.25).min(1.0) }
}

fn security_score(p: &CandidateProposal) -> f64 {
    let has_security_aware: bool = p.risks.iter().any(|r| {
        let lower = r.to_lowercase();
        lower.contains("secur") || lower.contains("inject") || lower.contains("auth")
            || lower.contains("sanitiz") || lower.contains("validat")
            || lower.contains("bảo mật") || lower.contains("xác thực") || lower.contains("chèn mã")
            || lower.contains("lỗ hổng") || lower.contains("mã hóa") || lower.contains("phân quyền")
    });
    if has_security_aware { 0.8 } else { 0.3 }
}

fn language_score(p: &CandidateProposal, expected_lang: Option<crate::lang::SupportedLanguage>) -> f64 {
    let Some(lang) = expected_lang else { return 1.0 };
    let summary_lower = p.summary.to_lowercase();
    let diagnosis_lower = p.diagnosis.to_lowercase();

    match lang {
        crate::lang::SupportedLanguage::Vietnamese => {
            let has_vn = summary_lower.chars().any(|c| matches!(c,
                'ắ' | 'ằ' | 'ẳ' | 'ẵ' | 'ặ' | 'ấ' | 'ầ' | 'ẩ' | 'ẫ' | 'ậ' |
                'ế' | 'ề' | 'ể' | 'ễ' | 'ệ' | 'ố' | 'ồ' | 'ổ' | 'ỗ' | 'ộ' |
                'ớ' | 'ờ' | 'ở' | 'ỡ' | 'ợ' | 'ứ' | 'ừ' | 'ử' | 'ữ' | 'ự' |
                'á' | 'à' | 'ả' | 'ã' | 'ạ' | 'é' | 'è' | 'ẻ' | 'ẽ' | 'ẹ' |
                'í' | 'ì' | 'ỉ' | 'ĩ' | 'ị' | 'ó' | 'ò' | 'ỏ' | 'õ' | 'ọ' |
                'ú' | 'ù' | 'ủ' | 'ũ' | 'ụ' | 'ý' | 'ỳ' | 'ỷ' | 'ỹ' | 'ỵ' |
                'ă' | 'â' | 'đ' | 'ê' | 'ô' | 'ơ' | 'ư'
            ));
            // Check if diagnosis text has Vietnamese function words
            let has_vn_words = ["của", "và", "một", "cho", "để", "trong", "được", "không", "có", "là"]
                .iter().any(|w| diagnosis_lower.contains(w));
            if has_vn || has_vn_words { 1.0 } else { 0.3 }
        }
        crate::lang::SupportedLanguage::English => {
            // Penalize if mostly Vietnamese
            let vn_char_count = summary_lower.chars().filter(|c| matches!(c,
                'ắ' | 'ằ' | 'ẳ' | 'ế' | 'ề' | 'ể' | 'ố' | 'ồ' | 'ổ' |
                'ớ' | 'ờ' | 'ở' | 'ứ' | 'ừ' | 'ử' | 'á' | 'à' | 'ả' |
                'é' | 'è' | 'ẻ' | 'í' | 'ì' | 'ỉ' | 'ó' | 'ò' | 'ỏ' |
                'ú' | 'ù' | 'ủ' | 'ý' | 'ỳ' | 'ỷ' | 'ă' | 'â' | 'đ' | 'ê' | 'ô' | 'ơ' | 'ư'
            )).count();
            if vn_char_count > 3 { 0.3 } else { 1.0 }
        }
    }
}

fn clarity_score(p: &CandidateProposal) -> f64 {
    let summary_len = p.summary.len().min(500) as f64 / 500.0;
    let has_diag = !p.diagnosis.is_empty();
    (summary_len * 0.5 + has_diag as u8 as f64 * 0.5).min(1.0)
}
