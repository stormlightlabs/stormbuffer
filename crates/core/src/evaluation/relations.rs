use serde::{Deserialize, Serialize};

use crate::Error;

const RELATIONS_JSON: &str = include_str!("../../tests/fixtures/evaluation/relations.json");

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq)]
#[serde(rename_all = "snake_case")]
pub(crate) enum ReviewedRelation {
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
pub struct RelationRecord {
    pub title: String,
    pub kind: String,
    pub scope: String,
    pub body: String,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct RelationPair {
    pub id: String,
    pub relation: ReviewedRelation,
    pub left: RelationRecord,
    pub right: RelationRecord,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct RelationFixture {
    revision: String,
    pairs: Vec<RelationPair>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum AdvisoryRelation {
    Equivalent,
    Entails,
    EntailedBy,
    Contradiction,
    Related,
    Unrelated,
    Unknown,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ConfidenceBand {
    Low,
    Medium,
    High,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct RelationInference {
    pub relation: AdvisoryRelation,
    pub evidence: Vec<String>,
    pub confidence: ConfidenceBand,
    pub analyzer_fingerprint: String,
}

pub trait LocalRelationAnalyzer {
    fn fingerprint(&self) -> &str;
    fn analyze(&self, left: &RelationRecord, right: &RelationRecord) -> RelationInference;
}

#[derive(Clone, Debug, Default)]
pub struct ConservativeRelationAnalyzer;

impl LocalRelationAnalyzer for ConservativeRelationAnalyzer {
    fn fingerprint(&self) -> &str {
        "stormbuffer-conservative-relations-v1"
    }

    fn analyze(&self, left: &RelationRecord, right: &RelationRecord) -> RelationInference {
        let left_text = normalize(&format!("{} {}", left.title, left.body));
        let right_text = normalize(&format!("{} {}", right.title, right.body));
        let left_tokens = tokens(&left_text);
        let right_tokens = tokens(&right_text);
        let same_boundary = left.kind == right.kind && left.scope == right.scope;
        let (relation, evidence, confidence) = if same_boundary && left_text == right_text {
            (
                AdvisoryRelation::Equivalent,
                vec!["normalized title and body are identical".to_owned()],
                ConfidenceBand::High,
            )
        } else if same_boundary && explicit_negation_conflict(&normalize(&left.body), &normalize(&right.body)) {
            (
                AdvisoryRelation::Contradiction,
                vec!["the pair makes opposing explicit permission claims".to_owned()],
                ConfidenceBand::Medium,
            )
        } else if same_boundary && right_tokens.is_subset(&left_tokens) {
            (
                AdvisoryRelation::Entails,
                vec!["the right claim's terms are contained in the left claim".to_owned()],
                ConfidenceBand::Medium,
            )
        } else if same_boundary && left_tokens.is_subset(&right_tokens) {
            (
                AdvisoryRelation::EntailedBy,
                vec!["the left claim's terms are contained in the right claim".to_owned()],
                ConfidenceBand::Medium,
            )
        } else {
            let overlap = token_overlap(&left_tokens, &right_tokens);
            if overlap >= 0.45 {
                (
                    AdvisoryRelation::Related,
                    vec![format!("shared-term overlap is {:.0}%", overlap * 100.0)],
                    ConfidenceBand::Low,
                )
            } else if overlap <= 0.1 {
                (
                    AdvisoryRelation::Unrelated,
                    vec!["the pair has little shared vocabulary".to_owned()],
                    ConfidenceBand::Low,
                )
            } else {
                (
                    AdvisoryRelation::Unknown,
                    vec!["available local evidence does not support a relation".to_owned()],
                    ConfidenceBand::Low,
                )
            }
        };
        RelationInference { relation, evidence, confidence, analyzer_fingerprint: self.fingerprint().to_owned() }
    }
}

#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct RelationAnalysisReport {
    pub revision: String,
    pub pair_count: usize,
    pub deterministic_correct: usize,
    pub retrieval_candidate_recall: f64,
    pub pairwise_correct: usize,
    pub false_contradiction_count: usize,
    pub abstention_count: usize,
    pub analyzer_fingerprint: String,
    pub shadow_mode: bool,
}

pub(crate) fn relation_pairs() -> crate::Result<(String, Vec<RelationPair>)> {
    let fixture: RelationFixture = serde_json::from_str(RELATIONS_JSON)
        .map_err(|error| Error::invalid_input(format!("invalid relation pair fixture: {error}")))?;
    if fixture.pairs.is_empty() {
        return Err(Error::invalid_input("relation pair fixture is empty"));
    }
    Ok((fixture.revision, fixture.pairs))
}

pub fn run_relation_analysis_evaluation(
    retrieved_pair_ids: &std::collections::HashSet<String>, analyzer: &dyn LocalRelationAnalyzer,
) -> crate::Result<RelationAnalysisReport> {
    let (revision, pairs) = relation_pairs()?;
    let expected_candidates = pairs
        .iter()
        .filter(|pair| pair.relation != ReviewedRelation::Unrelated)
        .count();
    let retrieved_candidates = pairs
        .iter()
        .filter(|pair| pair.relation != ReviewedRelation::Unrelated && retrieved_pair_ids.contains(&pair.id))
        .count();
    let mut deterministic_correct = 0;
    let mut pairwise_correct = 0;
    let mut false_contradictions = 0;
    let mut abstentions = 0;
    for pair in &pairs {
        let exact = normalize(&pair.left.title) == normalize(&pair.right.title)
            && normalize(&pair.left.body) == normalize(&pair.right.body)
            && pair.left.kind == pair.right.kind
            && pair.left.scope == pair.right.scope;
        deterministic_correct += usize::from(
            (exact && pair.relation == ReviewedRelation::Equivalent)
                || (!exact && pair.relation != ReviewedRelation::Equivalent),
        );
        let inference = analyzer.analyze(&pair.left, &pair.right);
        let expected = reviewed_advisory_relation(pair.relation);
        pairwise_correct += usize::from(inference.relation == expected);
        false_contradictions += usize::from(
            inference.relation == AdvisoryRelation::Contradiction && pair.relation != ReviewedRelation::Contradiction,
        );
        abstentions += usize::from(inference.relation == AdvisoryRelation::Unknown);
    }
    Ok(RelationAnalysisReport {
        revision,
        pair_count: pairs.len(),
        deterministic_correct,
        retrieval_candidate_recall: retrieved_candidates as f64 / expected_candidates as f64,
        pairwise_correct,
        false_contradiction_count: false_contradictions,
        abstention_count: abstentions,
        analyzer_fingerprint: analyzer.fingerprint().to_owned(),
        shadow_mode: true,
    })
}

fn reviewed_advisory_relation(relation: ReviewedRelation) -> AdvisoryRelation {
    match relation {
        ReviewedRelation::Equivalent => AdvisoryRelation::Equivalent,
        ReviewedRelation::Refinement => AdvisoryRelation::EntailedBy,
        ReviewedRelation::Contradiction => AdvisoryRelation::Contradiction,
        ReviewedRelation::CompatibleAddition
        | ReviewedRelation::TemporalChange
        | ReviewedRelation::ConditionalDifference
        | ReviewedRelation::Related => AdvisoryRelation::Related,
        ReviewedRelation::Unrelated => AdvisoryRelation::Unrelated,
    }
}

fn tokens(value: &str) -> std::collections::HashSet<String> {
    value
        .split(|character: char| !character.is_alphanumeric())
        .filter(|token| token.len() > 2)
        .map(ToOwned::to_owned)
        .collect()
}

fn token_overlap(left: &std::collections::HashSet<String>, right: &std::collections::HashSet<String>) -> f64 {
    let union = left.union(right).count();
    if union == 0 {
        return 0.0;
    }
    left.intersection(right).count() as f64 / union as f64
}

fn explicit_negation_conflict(left: &str, right: &str) -> bool {
    let left_claims = permission_claims(left);
    let right_claims = permission_claims(right);
    left_claims.iter().any(|(left_permits, left_terms)| {
        right_claims.iter().any(|(right_permits, right_terms)| {
            left_permits != right_permits && token_overlap(left_terms, right_terms) >= 0.6
        })
    })
}

fn permission_claims(value: &str) -> Vec<(bool, std::collections::HashSet<String>)> {
    value
        .split(['.', ';', '!', '?'])
        .filter_map(|clause| {
            let (permits, claim) = if let Some(claim) = clause.split_once("must never ") {
                (false, claim.1)
            } else if let Some(claim) = clause.split_once("may not ") {
                (false, claim.1)
            } else if let Some(claim) = clause.split_once("cannot ") {
                (false, claim.1)
            } else if let Some(claim) = clause.split_once("may ") {
                (true, claim.1)
            } else if let Some(claim) = clause.split_once("can ") {
                (true, claim.1)
            } else {
                return None;
            };
            let terms = tokens(claim);
            (!terms.is_empty()).then_some((permits, terms))
        })
        .collect()
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
    let (revision, pairs) = relation_pairs()?;

    let mut normalized_duplicates = 0;
    let mut duplicate_errors = 0;
    let mut old_conflict_claims = 0;
    let mut reviewed_contradictions = 0;
    let mut false_conflicts = 0;
    let mut missed_contradictions = 0;
    for pair in &pairs {
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
        revision,
        pair_count: pairs.len(),
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

    #[test]
    fn contradiction_requires_opposing_claims_about_the_same_proposition() {
        assert!(explicit_negation_conflict(
            "the service may access the network.",
            "the service must never access the network.",
        ));
        assert!(!explicit_negation_conflict(
            "the service may access the network.",
            "the service must never delete backups.",
        ));
        assert!(!explicit_negation_conflict(
            "agents may read records. backups must never leave the host.",
            "agents may not publish records. backups can remain local.",
        ));
    }
}
