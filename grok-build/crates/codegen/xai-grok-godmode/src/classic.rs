use crate::config::CandidatePreset;
use crate::candidate::{CandidateAgent, CandidateProposal};
use crate::error::GodmodeError;
use crate::events::GodmodeEvent;
use crate::orchestrator::Orchestrator;
use crate::refusal::RefusalDetector;

pub async fn run_godmode_classic(
    orchestrator: &Orchestrator,
    task: &str,
    repository_context: &str,
    provider_registry: &xai_grok_providers::ProviderRegistry,
) -> Result<Vec<(CandidateAgent, Option<CandidateProposal>)>, GodmodeError> {
    let config = orchestrator.config();
    let enabled: Vec<&CandidatePreset> = config.candidates.iter().filter(|c| c.enabled).collect();

    if enabled.is_empty() {
        return Err(GodmodeError::NoCandidates);
    }

    let candidates: Vec<CandidateAgent> = enabled.iter().map(|p| CandidateAgent::new((*p).clone())).collect();
    let event_tx = orchestrator.event_tx().clone();

    for c in &candidates {
        let _ = event_tx.send(GodmodeEvent::CandidateQueued {
            race_id: "classic".into(),
            candidate_id: c.id.clone(),
        });
    }

    let mut handles = Vec::new();
    for candidate in candidates.iter() {
        let c = candidate.clone();
        let task = task.to_string();
        let ctx = repository_context.to_string();
        let tx = event_tx.clone();
        let registry = provider_registry.clone();

        let handle = tokio::spawn(async move {
            let _ = tx.send(GodmodeEvent::CandidateStarted {
                race_id: "classic".into(),
                candidate_id: c.id.clone(),
                provider: c.preset.provider.clone(),
                model: c.preset.model.clone(),
                persona: c.preset.persona.name.clone(),
            });

            let instruction = format!("{}\n\nTASK:\n{}\n\nREPOSITORY CONTEXT:\n{}", c.system_instruction(), task, ctx);

            let provider = registry.get(&c.preset.provider).await;
            if provider.is_none() {
                let _ = tx.send(GodmodeEvent::CandidateFailed {
                    race_id: "classic".into(),
                    candidate_id: c.id.clone(),
                    provider: c.preset.provider.clone(),
                    model: c.preset.model.clone(),
                    error: format!("Provider {} not found", c.preset.provider),
                });
                return (c, None);
            }

            let provider = provider.unwrap();
            let request = xai_grok_providers::ModelRequest {
                provider_id: c.preset.provider.clone(),
                model: c.preset.model.clone(),
                messages: vec![
                    xai_grok_providers::Message {
                        role: xai_grok_providers::MessageRole::System,
                        content: xai_grok_providers::MessageContent::Text(c.system_instruction()),
                        name: None,
                        tool_call_id: None,
                    },
                    xai_grok_providers::Message {
                        role: xai_grok_providers::MessageRole::User,
                        content: xai_grok_providers::MessageContent::Text(instruction),
                        name: None,
                        tool_call_id: None,
                    },
                ],
                temperature: Some(c.preset.temperature),
                max_tokens: Some(4096),
                ..Default::default()
            };

            match provider.complete(request).await {
                Ok(response) => {
                    let refusal = RefusalDetector::detect(&response.content);
                    if RefusalDetector::is_refused(&refusal) {
                        let _ = tx.send(GodmodeEvent::CandidateRefused {
                            race_id: "classic".into(),
                            candidate_id: c.id.clone(),
                            reason: format!("{:?}", refusal),
                        });
                        return (c, None);
                    }

                    let proposal = CandidateProposal {
                        candidate_id: c.id.clone(),
                        provider: c.preset.provider.clone(),
                        model: c.preset.model.clone(),
                        persona: c.preset.persona.name.clone(),
                        summary: response.content[..response.content.len().min(500)].to_string(),
                        diagnosis: response.content.clone(),
                        evidence: vec![],
                        files_to_change: vec![],
                        symbols_to_change: vec![],
                        proposed_changes: vec![],
                        proposed_patch: None,
                        commands_to_run: vec![],
                        tests: vec![],
                        risks: vec![],
                        assumptions: vec![],
                        limitations: vec![],
                        confidence: 0.7,
                    };

                    let _ = tx.send(GodmodeEvent::CandidateCompleted {
                        race_id: "classic".into(),
                        candidate_id: c.id.clone(),
                        provider: c.preset.provider.clone(),
                        model: c.preset.model.clone(),
                        latency_ms: 0,
                        tokens: 0,
                        score: None,
                    });

                    (c, Some(proposal))
                }
                Err(e) => {
                    let _ = tx.send(GodmodeEvent::CandidateFailed {
                        race_id: "classic".into(),
                        candidate_id: c.id.clone(),
                        provider: c.preset.provider.clone(),
                        model: c.preset.model.clone(),
                        error: e.to_string(),
                    });
                    (c, None)
                }
            }
        });
        handles.push(handle);
    }

    let results = futures::future::join_all(handles).await;
    Ok(results.into_iter().filter_map(|r| r.ok()).collect())
}
