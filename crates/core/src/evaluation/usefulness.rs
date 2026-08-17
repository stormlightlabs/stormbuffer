use std::collections::{HashMap, HashSet};

use serde::{Deserialize, Serialize};

use crate::{
    Error, EvidenceOutcome, ProposalFeedbackOutcome, ReceiptFeedbackFile, ReceiptId, RecordId, Timestamp,
    parse_receipt_feedback_file,
};

const CORPUS_JSON: &str = include_str!("../../tests/fixtures/evaluation/corpus.json");
const FEEDBACK_JSON: &str = include_str!("../../tests/fixtures/evaluation/receipt-feedback.json");
const USEFULNESS_JSON: &str = include_str!("../../tests/fixtures/evaluation/usefulness.json");

#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct CorpusFile {
    revision: String,
    #[serde(rename = "fixed_seed")]
    _fixed_seed: u64,
    records: Vec<CorpusRecord>,
}

#[derive(Clone, Debug, Deserialize)]
struct CorpusRecord {
    id: String,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct UsefulnessFile {
    revision: String,
    corpus_revision: String,
    feedback_revision: String,
    observations: Vec<UsefulnessObservation>,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct UsefulnessObservation {
    id: String,
    #[serde(deserialize_with = "record_ids")]
    target_record_ids: Vec<RecordId>,
    captured_at: Option<String>,
    receipt: ReceiptObservation,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct ReceiptObservation {
    receipt_id: ReceiptId,
    retrieved_at: String,
    used_tokens: usize,
    #[serde(deserialize_with = "record_ids")]
    retrieved_record_ids: Vec<RecordId>,
}

#[derive(Clone, Debug, Default, Eq, PartialEq, Serialize)]
pub struct UsefulnessBreakdown {
    pub absent_memory: usize,
    pub retrieval_miss: usize,
    pub retrieved_ignored: usize,
    pub stale_or_incorrect: usize,
    pub retrieved_and_used: usize,
    pub retrieved_unjudged: usize,
}

#[derive(Clone, Debug, Default, PartialEq, Serialize)]
pub struct ProposalOutcomeRates {
    pub proposal_count: usize,
    pub approval_rate: f64,
    pub edit_rate: f64,
    pub rejection_rate: f64,
    pub superseding_rate: f64,
    pub duplicate_rate: f64,
}

#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct UsefulnessReport {
    pub observation_count: usize,
    pub feedback_receipt_count: usize,
    pub breakdown: UsefulnessBreakdown,
    pub retrieved_and_used_rate: f64,
    pub stale_correction_count: usize,
    pub context_tokens_per_used_memory: f64,
    pub mean_seconds_to_later_reuse: f64,
    pub proposals: ProposalOutcomeRates,
}

#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct UsefulnessComparisonReport {
    pub revision: String,
    pub without_receipt_feedback: UsefulnessReport,
    pub with_receipt_feedback: UsefulnessReport,
}

pub fn run_synthetic_usefulness_evaluation() -> crate::Result<UsefulnessComparisonReport> {
    let corpus: CorpusFile = parse_fixture(CORPUS_JSON, "evaluation corpus")?;
    let fixture: UsefulnessFile = parse_fixture(USEFULNESS_JSON, "usefulness observations")?;
    let feedback = parse_receipt_feedback_file(FEEDBACK_JSON)?;
    validate_fixture(&fixture, &corpus.revision, &feedback)?;
    let corpus_ids = corpus
        .records
        .into_iter()
        .map(|record| RecordId::parse(&record.id).map_err(Error::invalid_input))
        .collect::<crate::Result<HashSet<_>>>()?;

    Ok(UsefulnessComparisonReport {
        revision: fixture.revision.clone(),
        without_receipt_feedback: evaluate(&fixture, &corpus_ids, None),
        with_receipt_feedback: evaluate(&fixture, &corpus_ids, Some(&feedback)),
    })
}

fn parse_fixture<T>(contents: &str, name: &str) -> crate::Result<T>
where
    T: for<'de> Deserialize<'de>,
{
    serde_json::from_str(contents).map_err(|error| Error::invalid_input(format!("invalid {name}: {error}")))
}

fn validate_fixture(
    fixture: &UsefulnessFile, corpus_revision: &str, feedback: &ReceiptFeedbackFile,
) -> crate::Result<()> {
    if fixture.revision.trim() != fixture.revision || fixture.revision.is_empty() {
        return Err(Error::invalid_input(
            "usefulness revision must be non-empty without surrounding whitespace",
        ));
    }
    if fixture.corpus_revision != corpus_revision {
        return Err(Error::invalid_input(
            "usefulness observations and corpus revisions do not match",
        ));
    }
    if fixture.feedback_revision != feedback.revision {
        return Err(Error::invalid_input(
            "usefulness observations and receipt feedback revisions do not match",
        ));
    }
    let mut ids = HashSet::new();
    let mut receipts = HashSet::new();
    for observation in &fixture.observations {
        if observation.id.trim() != observation.id || observation.id.is_empty() || !ids.insert(&observation.id) {
            return Err(Error::invalid_input(
                "usefulness observation IDs must be unique and non-empty",
            ));
        }
        if observation.target_record_ids.is_empty() {
            return Err(Error::invalid_input(format!(
                "usefulness observation {} must name target records",
                observation.id
            )));
        }
        if !receipts.insert(observation.receipt.receipt_id) {
            return Err(Error::invalid_input(format!(
                "duplicate usefulness receipt {}",
                observation.receipt.receipt_id
            )));
        }
        let retrieved = Timestamp::parse(&observation.receipt.retrieved_at).map_err(Error::invalid_input)?;
        if let Some(captured_at) = &observation.captured_at {
            let captured = Timestamp::parse(captured_at).map_err(Error::invalid_input)?;
            if captured > retrieved {
                return Err(Error::invalid_input(format!(
                    "usefulness observation {} was retrieved before capture",
                    observation.id
                )));
            }
        }
    }
    Ok(())
}

fn evaluate(
    fixture: &UsefulnessFile, corpus_ids: &HashSet<RecordId>, feedback: Option<&ReceiptFeedbackFile>,
) -> UsefulnessReport {
    let feedback_by_receipt = feedback
        .map(|file| {
            file.judgments
                .iter()
                .map(|judgment| (judgment.receipt_id, judgment))
                .collect::<HashMap<_, _>>()
        })
        .unwrap_or_default();
    let mut breakdown = UsefulnessBreakdown::default();
    let mut used_tokens = 0usize;
    let mut reuse_seconds = 0f64;

    for observation in &fixture.observations {
        let captured_targets = observation
            .target_record_ids
            .iter()
            .copied()
            .filter(|id| corpus_ids.contains(id))
            .collect::<HashSet<_>>();
        if captured_targets.is_empty() {
            breakdown.absent_memory += 1;
            continue;
        }
        let retrieved_targets = observation
            .receipt
            .retrieved_record_ids
            .iter()
            .copied()
            .filter(|id| captured_targets.contains(id))
            .collect::<HashSet<_>>();
        if retrieved_targets.is_empty() {
            breakdown.retrieval_miss += 1;
            continue;
        }
        let outcomes = feedback_by_receipt
            .get(&observation.receipt.receipt_id)
            .into_iter()
            .flat_map(|judgment| judgment.evidence.iter())
            .filter(|evidence| retrieved_targets.contains(&evidence.record_id))
            .map(|evidence| evidence.outcome)
            .collect::<Vec<_>>();
        if outcomes.contains(&EvidenceOutcome::Corrected) {
            breakdown.stale_or_incorrect += 1;
        } else if outcomes
            .iter()
            .any(|outcome| matches!(outcome, EvidenceOutcome::Included | EvidenceOutcome::Cited))
        {
            breakdown.retrieved_and_used += 1;
            used_tokens += observation.receipt.used_tokens;
            if let Some(captured_at) = &observation.captured_at {
                let captured = Timestamp::parse(captured_at).expect("validated capture time");
                let retrieved = Timestamp::parse(&observation.receipt.retrieved_at).expect("validated retrieval time");
                reuse_seconds +=
                    (retrieved.as_offset_datetime() - captured.as_offset_datetime()).whole_seconds() as f64;
            }
        } else if outcomes.contains(&EvidenceOutcome::Ignored) {
            breakdown.retrieved_ignored += 1;
        } else {
            breakdown.retrieved_unjudged += 1;
        }
    }

    let relevant_retrievals = breakdown.retrieved_ignored
        + breakdown.stale_or_incorrect
        + breakdown.retrieved_and_used
        + breakdown.retrieved_unjudged;
    let used = breakdown.retrieved_and_used;
    UsefulnessReport {
        observation_count: fixture.observations.len(),
        feedback_receipt_count: feedback.map_or(0, |file| file.judgments.len()),
        retrieved_and_used_rate: ratio(used, relevant_retrievals),
        stale_correction_count: breakdown.stale_or_incorrect,
        context_tokens_per_used_memory: ratio(used_tokens, used),
        mean_seconds_to_later_reuse: if used == 0 { 0.0 } else { reuse_seconds / used as f64 },
        proposals: proposal_rates(feedback),
        breakdown,
    }
}

pub(crate) fn proposal_rates(feedback: Option<&ReceiptFeedbackFile>) -> ProposalOutcomeRates {
    let outcomes = feedback
        .into_iter()
        .flat_map(|file| file.judgments.iter())
        .filter_map(|judgment| judgment.proposal.as_ref())
        .map(|proposal| proposal.outcome)
        .collect::<Vec<_>>();
    let count = outcomes.len();
    let rate = |expected| ratio(outcomes.iter().filter(|outcome| **outcome == expected).count(), count);
    ProposalOutcomeRates {
        proposal_count: count,
        approval_rate: rate(ProposalFeedbackOutcome::Approved),
        edit_rate: rate(ProposalFeedbackOutcome::Edited),
        rejection_rate: rate(ProposalFeedbackOutcome::Rejected),
        superseding_rate: rate(ProposalFeedbackOutcome::Superseding),
        duplicate_rate: rate(ProposalFeedbackOutcome::Duplicate),
    }
}

fn ratio(numerator: usize, denominator: usize) -> f64 {
    if denominator == 0 { 0.0 } else { numerator as f64 / denominator as f64 }
}

fn record_ids<'de, D>(deserializer: D) -> Result<Vec<RecordId>, D::Error>
where
    D: serde::Deserializer<'de>,
{
    let values = Vec::<String>::deserialize(deserializer)?;
    values
        .into_iter()
        .map(|value| RecordId::parse(&value).map_err(serde::de::Error::custom))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn feedback_distinguishes_usefulness_failures_and_review_outcomes() {
        let comparison = run_synthetic_usefulness_evaluation().expect("usefulness evaluation");
        let without = comparison.without_receipt_feedback;
        assert_eq!(without.breakdown.absent_memory, 1);
        assert_eq!(without.breakdown.retrieval_miss, 1);
        assert_eq!(without.breakdown.retrieved_unjudged, 4);
        assert_eq!(without.breakdown.retrieved_and_used, 0);

        let with = comparison.with_receipt_feedback;
        assert_eq!(with.breakdown.absent_memory, 1);
        assert_eq!(with.breakdown.retrieval_miss, 1);
        assert_eq!(with.breakdown.retrieved_ignored, 1);
        assert_eq!(with.breakdown.stale_or_incorrect, 1);
        assert_eq!(with.breakdown.retrieved_and_used, 2);
        assert_eq!(with.breakdown.retrieved_unjudged, 0);
        assert_eq!(with.retrieved_and_used_rate, 0.5);
        assert_eq!(with.context_tokens_per_used_memory, 17.0);
        assert_eq!(with.mean_seconds_to_later_reuse, 47_070.0);
        assert_eq!(with.proposals.proposal_count, 5);
        assert_eq!(with.proposals.duplicate_rate, 0.2);
    }

    #[test]
    fn usefulness_output_contains_no_conversation_content() {
        let report = run_synthetic_usefulness_evaluation().expect("usefulness evaluation");
        let output = serde_json::to_string(&report).expect("serialize usefulness report");
        for field in ["query", "prompt", "answer", "transcript", "body"] {
            assert!(!output.contains(field), "unexpected content field: {field}");
        }
    }
}
