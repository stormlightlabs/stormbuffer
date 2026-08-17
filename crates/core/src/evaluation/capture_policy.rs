use std::collections::{HashMap, HashSet};

use serde::{Deserialize, Serialize};

use crate::{
    Error, ProposalOutcomeRates, ReceiptFeedbackFile, ReceiptId, RecordId, RecordKind, parse_receipt_feedback_file,
};

const CAPTURE_POLICY_JSON: &str = include_str!("../../tests/fixtures/evaluation/capture-policy.json");
const FEEDBACK_JSON: &str = include_str!("../../tests/fixtures/evaluation/receipt-feedback.json");

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum CaptureEvent {
    DurableCorrection,
    AcceptedDecision,
    TentativeDiscussion,
    RoutineCompletion,
    RepositoryAuthoritativeKnowledge,
    ConfirmedRootCause,
    NecessaryHandoff,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum CaptureDisposition {
    Abstain,
    Propose,
    Update,
    Checkpoint,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum CaptureReason {
    ExistingMemoryIsStale,
    DurableAcceptedDecision,
    TentativeOrUnsettled,
    NoCaptureEvent,
    RepositoryAlreadyPreservesKnowledge,
    DurableConfirmedRootCause,
    CrossSessionStateIsNotRecoverable,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct CaptureCandidate {
    #[serde(deserialize_with = "record_id")]
    record_id: RecordId,
    kind: String,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct CaptureAssessment {
    scenario_id: String,
    event: CaptureEvent,
    disposition: CaptureDisposition,
    reason: CaptureReason,
    candidate: Option<CaptureCandidate>,
    receipt_id: Option<ReceiptId>,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct CaptureScenario {
    id: String,
    event: CaptureEvent,
    expected_disposition: CaptureDisposition,
    expected_reason: CaptureReason,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct CapturePolicyFile {
    revision: String,
    policy_revision: String,
    scenarios: Vec<CaptureScenario>,
    assessments: Vec<CaptureAssessment>,
}

#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct CapturePolicyReport {
    pub revision: String,
    pub policy_revision: String,
    pub scenario_count: usize,
    pub correct_assessment_count: usize,
    pub correct_abstention_count: usize,
    pub actionable_assessment_count: usize,
    pub proposal_precision: f64,
    pub missed_memory_judgments: usize,
    pub proposals: ProposalOutcomeRates,
    pub passed: bool,
}

/// Scores assessments already made by a host. This evaluator does not infer a
/// disposition from an event and is not used by capture or mutation paths.
pub fn run_synthetic_capture_policy_evaluation() -> crate::Result<CapturePolicyReport> {
    let fixture: CapturePolicyFile = serde_json::from_str(CAPTURE_POLICY_JSON)
        .map_err(|error| Error::invalid_input(format!("invalid capture policy fixture: {error}")))?;
    let feedback = parse_receipt_feedback_file(FEEDBACK_JSON)?;
    evaluate(&fixture, &feedback)
}

fn evaluate(fixture: &CapturePolicyFile, feedback: &ReceiptFeedbackFile) -> crate::Result<CapturePolicyReport> {
    validate(fixture, feedback)?;
    let assessments = fixture
        .assessments
        .iter()
        .map(|assessment| (assessment.scenario_id.as_str(), assessment))
        .collect::<HashMap<_, _>>();
    let mut correct = 0usize;
    let mut correct_abstentions = 0usize;
    let mut actionable = 0usize;
    let mut correct_actionable = 0usize;
    let mut missed = 0usize;
    let mut joined_receipts = HashSet::new();

    for scenario in &fixture.scenarios {
        let assessment = assessments[scenario.id.as_str()];
        let matches = assessment.event == scenario.event
            && assessment.disposition == scenario.expected_disposition
            && assessment.reason == scenario.expected_reason;
        if matches {
            correct += 1;
        }
        if scenario.expected_disposition != CaptureDisposition::Abstain && !matches {
            missed += 1;
        }
        if assessment.disposition == CaptureDisposition::Abstain {
            if scenario.expected_disposition == CaptureDisposition::Abstain && matches {
                correct_abstentions += 1;
            }
        } else {
            actionable += 1;
            if matches && scenario.expected_disposition != CaptureDisposition::Abstain {
                correct_actionable += 1;
            }
            if let Some(receipt_id) = assessment.receipt_id {
                joined_receipts.insert(receipt_id);
            }
        }
    }

    let joined_feedback = ReceiptFeedbackFile {
        revision: feedback.revision.clone(),
        judgments: feedback
            .judgments
            .iter()
            .filter(|judgment| joined_receipts.contains(&judgment.receipt_id))
            .cloned()
            .collect(),
    };
    let passed = correct == fixture.scenarios.len() && missed == 0;
    Ok(CapturePolicyReport {
        revision: fixture.revision.clone(),
        policy_revision: fixture.policy_revision.clone(),
        scenario_count: fixture.scenarios.len(),
        correct_assessment_count: correct,
        correct_abstention_count: correct_abstentions,
        actionable_assessment_count: actionable,
        proposal_precision: ratio(correct_actionable, actionable),
        missed_memory_judgments: missed,
        proposals: super::usefulness::proposal_rates(Some(&joined_feedback)),
        passed,
    })
}

fn validate(fixture: &CapturePolicyFile, feedback: &ReceiptFeedbackFile) -> crate::Result<()> {
    if fixture.policy_revision != "stormbuffer-capture-v1" {
        return Err(Error::invalid_input("unknown capture policy revision"));
    }
    if fixture.revision.trim() != fixture.revision || fixture.revision.is_empty() {
        return Err(Error::invalid_input(
            "capture evaluation revision must be non-empty without surrounding whitespace",
        ));
    }
    let feedback_by_receipt = feedback
        .judgments
        .iter()
        .map(|judgment| (judgment.receipt_id, judgment))
        .collect::<HashMap<_, _>>();
    let scenario_ids = fixture
        .scenarios
        .iter()
        .map(|scenario| {
            if scenario.id.trim() != scenario.id || scenario.id.is_empty() {
                return Err(Error::invalid_input(
                    "capture scenario IDs must be non-empty without surrounding whitespace",
                ));
            }
            Ok(scenario.id.as_str())
        })
        .collect::<crate::Result<Vec<_>>>()?
        .into_iter()
        .collect::<HashSet<_>>();
    if scenario_ids.len() != fixture.scenarios.len() || scenario_ids.len() != fixture.assessments.len() {
        return Err(Error::invalid_input(
            "capture scenarios and assessments must have one unique matching entry",
        ));
    }
    let mut assessment_ids = HashSet::new();
    for assessment in &fixture.assessments {
        if !scenario_ids.contains(assessment.scenario_id.as_str())
            || !assessment_ids.insert(assessment.scenario_id.as_str())
        {
            return Err(Error::invalid_input(
                "capture assessments must match one unique scenario",
            ));
        }
        let actionable = assessment.disposition != CaptureDisposition::Abstain;
        if actionable != assessment.candidate.is_some() || actionable != assessment.receipt_id.is_some() {
            return Err(Error::invalid_input(format!(
                "capture assessment {} must attach one candidate and receipt exactly when actionable",
                assessment.scenario_id
            )));
        }
        if let Some(candidate) = &assessment.candidate {
            candidate.kind.parse::<RecordKind>().map_err(|error| {
                Error::invalid_input(format!(
                    "capture assessment {} has invalid candidate kind: {error}",
                    assessment.scenario_id
                ))
            })?;
        }
        if let Some(receipt) = assessment.receipt_id {
            let judgment = feedback_by_receipt.get(&receipt).ok_or_else(|| {
                Error::invalid_input(format!(
                    "capture assessment {} references unknown receipt feedback",
                    assessment.scenario_id
                ))
            })?;
            if judgment.proposal.as_ref().map(|proposal| proposal.record_id)
                != assessment.candidate.as_ref().map(|candidate| candidate.record_id)
            {
                return Err(Error::invalid_input(format!(
                    "capture assessment {} candidate does not match receipt feedback",
                    assessment.scenario_id
                )));
            }
        }
    }
    Ok(())
}

fn ratio(numerator: usize, denominator: usize) -> f64 {
    if denominator == 0 { 0.0 } else { numerator as f64 / denominator as f64 }
}

fn record_id<'de, D>(deserializer: D) -> Result<RecordId, D::Error>
where
    D: serde::Deserializer<'de>,
{
    let value = String::deserialize(deserializer)?;
    RecordId::parse(&value).map_err(serde::de::Error::custom)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn host_assessments_cover_events_abstentions_and_review_outcomes() {
        let report = run_synthetic_capture_policy_evaluation().expect("capture policy evaluation");
        assert!(report.passed);
        assert_eq!(report.scenario_count, 8);
        assert_eq!(report.correct_abstention_count, 3);
        assert_eq!(report.actionable_assessment_count, 5);
        assert_eq!(report.proposal_precision, 1.0);
        assert_eq!(report.missed_memory_judgments, 0);
        assert_eq!(report.proposals.proposal_count, 5);
        assert_eq!(report.proposals.approval_rate, 0.2);
        assert_eq!(report.proposals.edit_rate, 0.2);
        assert_eq!(report.proposals.rejection_rate, 0.2);
        assert_eq!(report.proposals.superseding_rate, 0.2);
        assert_eq!(report.proposals.duplicate_rate, 0.2);
    }

    #[test]
    fn capture_report_contains_no_conversation_content() {
        let report = run_synthetic_capture_policy_evaluation().expect("capture policy evaluation");
        let output = serde_json::to_string(&report).expect("serialize capture report");
        for field in ["query", "prompt", "answer", "transcript", "body"] {
            assert!(!output.contains(field), "unexpected content field: {field}");
        }
    }
}
