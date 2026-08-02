use crate::candidate::CandidateProposal;

pub fn merge_proposals(proposals: &[CandidateProposal]) -> CandidateProposal {
    if proposals.is_empty() {
        return CandidateProposal {
            candidate_id: "merged".into(), provider: "merged".into(), model: "merged".into(),
            persona: "Merged".into(), summary: String::new(), diagnosis: String::new(),
            evidence: vec![], files_to_change: vec![], symbols_to_change: vec![],
            proposed_changes: vec![], proposed_patch: None, commands_to_run: vec![],
            tests: vec![], risks: vec![], assumptions: vec![], limitations: vec![],
            confidence: 0.0,
        };
    }

    let mut merged = proposals[0].clone();
    merged.candidate_id = format!("merged-{}", proposals.iter().map(|p| &p.candidate_id).cloned().collect::<Vec<_>>().join("+"));
    merged.confidence = (proposals.iter().map(|p| p.confidence as f64).sum::<f64>() / proposals.len() as f64) as f32;

    for p in &proposals[1..] {
        for f in &p.files_to_change { if !merged.files_to_change.contains(f) { merged.files_to_change.push(f.clone()); } }
        for r in &p.risks { if !merged.risks.contains(r) { merged.risks.push(r.clone()); } }
        for t in &p.tests { if !merged.tests.contains(t) { merged.tests.push(t.clone()); } }
    }
    merged
}

pub fn select_winner(
    candidates: Vec<(crate::candidate::CandidateAgent, Option<crate::candidate::CandidateProposal>)>,
    override_id: Option<&str>,
) -> Option<(crate::candidate::CandidateAgent, crate::candidate::CandidateProposal)> {
    if let Some(id) = override_id {
        return candidates.iter().find(|(c, p)| c.id == id && p.is_some())
            .map(|(c, p)| (c.clone(), p.clone().unwrap()));
    }

    candidates.iter()
        .filter(|(_, p)| p.is_some())
        .max_by(|(a, _), (b, _)| {
            a.preset.provider.cmp(&b.preset.provider)
        })
        .map(|(c, p)| (c.clone(), p.clone().unwrap()))
}
