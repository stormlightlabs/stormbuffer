use serde::{Deserialize, Serialize};

use crate::Error;

const RELATIONS_JSON: &str = include_str!("../../tests/fixtures/evaluation/relations.json");

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq)]
#[serde(rename_all = "snake_case")]
enum ReviewedRelation {
    Equivalent,
    Refinement,
    Contradiction,
    CompatibleAddition,
    TemporalChange,
    ConditionalDifference,
    Related,
    Unrelated,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct RelationRecord {
    title: String,
    kind: String,
    scope: String,
    body: String,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct RelationPair {
    id: String,
    relation: ReviewedRelation,
    left: RelationRecord,
    right: RelationRecord,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct RelationFixture {
    revision: String,
    pairs: Vec<RelationPair>,
}

#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct RelationHeuristicReport {
    pub revision: String,
    pub pair_count: usize,
    pub normalized_duplicate_count: usize,
    pub normalized_duplicate_errors: usize,
    pub old_conflict_claim_count: usize,
    pub reviewed_contradiction_count: usize,
    pub false_conflict_count: usize,
    pub missed_contradiction_count: usize,
}

pub fn run_relation_heuristic_evaluation() -> crate::Result<RelationHeuristicReport> {
    let fixture: RelationFixture = serde_json::from_str(RELATIONS_JSON)
        .map_err(|error| Error::invalid_input(format!("invalid relation pair fixture: {error}")))?;
    if fixture.pairs.is_empty() {
        return Err(Error::invalid_input("relation pair fixture is empty"));
    }

    let mut normalized_duplicates = 0;
    let mut duplicate_errors = 0;
    let mut old_conflict_claims = 0;
    let mut reviewed_contradictions = 0;
    let mut false_conflicts = 0;
    let mut missed_contradictions = 0;
    for pair in &fixture.pairs {
        if pair.id.trim().is_empty() {
            return Err(Error::invalid_input("relation pair id is empty"));
        }
        let same_identity = normalize(&pair.left.title) == normalize(&pair.right.title)
            && pair.left.kind == pair.right.kind
            && pair.left.scope == pair.right.scope;
        let duplicate = same_identity && normalize(&pair.left.body) == normalize(&pair.right.body);
        let old_conflict = same_identity && !duplicate;
        let contradiction = pair.relation == ReviewedRelation::Contradiction;

        normalized_duplicates += usize::from(duplicate);
        duplicate_errors += usize::from(duplicate && pair.relation != ReviewedRelation::Equivalent);
        old_conflict_claims += usize::from(old_conflict);
        reviewed_contradictions += usize::from(contradiction);
        false_conflicts += usize::from(old_conflict && !contradiction);
        missed_contradictions += usize::from(contradiction && !old_conflict);
    }

    Ok(RelationHeuristicReport {
        revision: fixture.revision,
        pair_count: fixture.pairs.len(),
        normalized_duplicate_count: normalized_duplicates,
        normalized_duplicate_errors: duplicate_errors,
        old_conflict_claim_count: old_conflict_claims,
        reviewed_contradiction_count: reviewed_contradictions,
        false_conflict_count: false_conflicts,
        missed_contradiction_count: missed_contradictions,
    })
}

fn normalize(value: &str) -> String {
    value
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
        .to_ascii_lowercase()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reviewed_pairs_expose_the_old_conflict_heuristic() {
        let report = run_relation_heuristic_evaluation().expect("relation evaluation");
        assert_eq!(report.revision, "m6-relation-pairs-1");
        assert_eq!(report.pair_count, 9);
        assert_eq!(report.normalized_duplicate_count, 1);
        assert_eq!(report.normalized_duplicate_errors, 0);
        assert_eq!(report.old_conflict_claim_count, 7);
        assert_eq!(report.reviewed_contradiction_count, 1);
        assert_eq!(report.false_conflict_count, 6);
        assert_eq!(report.missed_contradiction_count, 0);
    }
}
