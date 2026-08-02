use crate::orchestrator::RaceCandidateResult;

pub struct Tournament;

#[derive(Debug, Clone)]
pub struct TournamentGroup {
    pub candidates: Vec<RaceCandidateResult>,
}

impl Tournament {
    pub fn create_groups(
        candidates: Vec<RaceCandidateResult>,
        group_size: usize,
    ) -> Vec<TournamentGroup> {
        candidates.chunks(group_size).map(|chunk| TournamentGroup {
            candidates: chunk.to_vec(),
        }).collect()
    }

    pub fn select_winners(
        groups: Vec<TournamentGroup>,
        winners_per_group: usize,
    ) -> Vec<RaceCandidateResult> {
        let mut winners = Vec::new();
        for group in groups {
            let mut sorted = group.candidates.clone();
            sorted.sort_by(|a, b| {
                b.score.unwrap_or(0.0).partial_cmp(&a.score.unwrap_or(0.0)).unwrap_or(std::cmp::Ordering::Equal)
            });
            winners.extend(sorted.into_iter().take(winners_per_group));
        }
        winners
    }

    pub fn run_tournament_rounds(
        mut candidates: Vec<RaceCandidateResult>,
        group_size: usize,
        winners_per_group: usize,
        min_remaining: usize,
    ) -> Vec<RaceCandidateResult> {
        while candidates.len() > min_remaining && candidates.len() > group_size {
            let groups = Self::create_groups(candidates, group_size);
            candidates = Self::select_winners(groups, winners_per_group);
        }
        candidates
    }
}
