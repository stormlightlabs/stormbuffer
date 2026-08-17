use std::collections::HashSet;
use std::fmt;
use std::fs;
use std::path::Path;
use std::str::FromStr;

use rusqlite::{Connection, params};
use serde::{Deserialize, Deserializer, Serialize, Serializer};
use uuid::Uuid;

use crate::repository::replace_file;
use crate::{Error, RecordId, Timestamp};

const FEEDBACK_SCHEMA_VERSION: u32 = 1;

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub struct ReceiptId(Uuid);

impl ReceiptId {
    pub fn new_v7() -> Self {
        Self(Uuid::now_v7())
    }

    pub fn parse(value: &str) -> Result<Self, String> {
        if value.trim() != value || value.is_empty() {
            return Err("must be a non-empty UUID without surrounding whitespace".to_owned());
        }
        let id = Uuid::parse_str(value).map_err(|error| format!("must be a valid UUID: {error}"))?;
        if id.is_nil() {
            return Err("must not be the nil UUID".to_owned());
        }
        Ok(Self(id))
    }
}

impl fmt::Display for ReceiptId {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.0.fmt(formatter)
    }
}

impl FromStr for ReceiptId {
    type Err = String;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        Self::parse(value)
    }
}

impl Serialize for ReceiptId {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.collect_str(self)
    }
}

impl<'de> Deserialize<'de> for ReceiptId {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let value = String::deserialize(deserializer)?;
        Self::parse(&value).map_err(serde::de::Error::custom)
    }
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum EvidenceOutcome {
    Included,
    Cited,
    Ignored,
    Corrected,
}

impl EvidenceOutcome {
    const fn as_str(self) -> &'static str {
        match self {
            Self::Included => "included",
            Self::Cited => "cited",
            Self::Ignored => "ignored",
            Self::Corrected => "corrected",
        }
    }
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ProposalFeedbackOutcome {
    Approved,
    Edited,
    Rejected,
    Superseding,
    Duplicate,
}

impl ProposalFeedbackOutcome {
    const fn as_str(self) -> &'static str {
        match self {
            Self::Approved => "approved",
            Self::Edited => "edited",
            Self::Rejected => "rejected",
            Self::Superseding => "superseding",
            Self::Duplicate => "duplicate",
        }
    }
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct EvidenceFeedback {
    #[serde(with = "record_id_serde")]
    pub record_id: RecordId,
    pub outcome: EvidenceOutcome,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ProposalFeedback {
    #[serde(with = "record_id_serde")]
    pub record_id: RecordId,
    pub outcome: ProposalFeedbackOutcome,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ReceiptFeedback {
    pub receipt_id: ReceiptId,
    pub recorded_at: String,
    pub evidence: Vec<EvidenceFeedback>,
    pub proposal: Option<ProposalFeedback>,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ReceiptFeedbackFile {
    pub revision: String,
    pub judgments: Vec<ReceiptFeedback>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct ReceiptFeedbackProjectionReport {
    pub revision: String,
    pub receipt_count: usize,
    pub evidence_count: usize,
    pub proposal_count: usize,
}

pub fn parse_receipt_feedback_file(contents: &str) -> crate::Result<ReceiptFeedbackFile> {
    let feedback: ReceiptFeedbackFile = serde_json::from_str(contents)
        .map_err(|error| Error::invalid_input(format!("invalid receipt feedback judgments: {error}")))?;
    validate_feedback(&feedback)?;
    Ok(feedback)
}

pub fn rebuild_receipt_feedback_projection(
    path: &Path, feedback: &ReceiptFeedbackFile,
) -> crate::Result<ReceiptFeedbackProjectionReport> {
    validate_feedback(feedback)?;
    let parent = path.parent().ok_or_else(|| {
        Error::io(
            "resolve the receipt feedback projection directory",
            std::io::Error::other("projection path has no parent"),
        )
    })?;
    fs::create_dir_all(parent)
        .map_err(|source| Error::io("create the receipt feedback projection directory", source))?;
    let temporary = parent.join(format!(".receipt-feedback-{}.sqlite3", Uuid::now_v7()));
    let result = build_projection(&temporary, feedback);
    if let Err(error) = result {
        let _ = fs::remove_file(&temporary);
        return Err(error);
    }
    if let Err(source) = replace_file(&temporary, path) {
        let _ = fs::remove_file(&temporary);
        return Err(Error::io("replace the receipt feedback projection", source));
    }

    Ok(ReceiptFeedbackProjectionReport {
        revision: feedback.revision.clone(),
        receipt_count: feedback.judgments.len(),
        evidence_count: feedback.judgments.iter().map(|judgment| judgment.evidence.len()).sum(),
        proposal_count: feedback
            .judgments
            .iter()
            .filter(|judgment| judgment.proposal.is_some())
            .count(),
    })
}

fn validate_feedback(feedback: &ReceiptFeedbackFile) -> crate::Result<()> {
    if feedback.revision.trim() != feedback.revision || feedback.revision.is_empty() {
        return Err(Error::invalid_input(
            "receipt feedback revision must be non-empty without surrounding whitespace",
        ));
    }
    let mut receipts = HashSet::new();
    for judgment in &feedback.judgments {
        if !receipts.insert(judgment.receipt_id) {
            return Err(Error::invalid_input(format!(
                "duplicate receipt feedback for {}",
                judgment.receipt_id
            )));
        }
        Timestamp::parse(&judgment.recorded_at).map_err(|message| {
            Error::invalid_input(format!(
                "receipt feedback {} recorded_at {message}",
                judgment.receipt_id
            ))
        })?;
        let mut records = HashSet::new();
        for evidence in &judgment.evidence {
            if !records.insert(evidence.record_id) {
                return Err(Error::invalid_input(format!(
                    "duplicate evidence feedback for record {} in receipt {}",
                    evidence.record_id, judgment.receipt_id
                )));
            }
        }
    }
    Ok(())
}

fn build_projection(path: &Path, feedback: &ReceiptFeedbackFile) -> crate::Result<()> {
    let mut connection =
        Connection::open(path).map_err(|source| db_error("open the receipt feedback projection", source))?;
    connection
        .execute_batch(
            "PRAGMA foreign_keys = ON;
             CREATE TABLE projection_metadata (
               key TEXT PRIMARY KEY,
               value TEXT NOT NULL
             );
             CREATE TABLE receipt_feedback (
               receipt_id TEXT PRIMARY KEY,
               recorded_at TEXT NOT NULL
             );
             CREATE TABLE evidence_feedback (
               receipt_id TEXT NOT NULL REFERENCES receipt_feedback(receipt_id) ON DELETE CASCADE,
               record_id TEXT NOT NULL,
               outcome TEXT NOT NULL CHECK (outcome IN ('included', 'cited', 'ignored', 'corrected')),
               PRIMARY KEY (receipt_id, record_id)
             );
             CREATE TABLE proposal_feedback (
               receipt_id TEXT PRIMARY KEY REFERENCES receipt_feedback(receipt_id) ON DELETE CASCADE,
               record_id TEXT NOT NULL,
               outcome TEXT NOT NULL CHECK (outcome IN ('approved', 'edited', 'rejected', 'superseding', 'duplicate'))
             );",
        )
        .map_err(|source| db_error("create the receipt feedback projection", source))?;
    let transaction = connection
        .transaction()
        .map_err(|source| db_error("begin receipt feedback projection rebuild", source))?;
    transaction
        .execute(
            "INSERT INTO projection_metadata(key, value) VALUES ('schema_version', ?1), ('revision', ?2)",
            params![FEEDBACK_SCHEMA_VERSION, feedback.revision],
        )
        .map_err(|source| db_error("write receipt feedback projection metadata", source))?;
    for judgment in &feedback.judgments {
        transaction
            .execute(
                "INSERT INTO receipt_feedback(receipt_id, recorded_at) VALUES (?1, ?2)",
                params![judgment.receipt_id.to_string(), judgment.recorded_at],
            )
            .map_err(|source| db_error("project receipt feedback", source))?;
        for evidence in &judgment.evidence {
            transaction
                .execute(
                    "INSERT INTO evidence_feedback(receipt_id, record_id, outcome) VALUES (?1, ?2, ?3)",
                    params![
                        judgment.receipt_id.to_string(),
                        evidence.record_id.to_string(),
                        evidence.outcome.as_str()
                    ],
                )
                .map_err(|source| db_error("project evidence feedback", source))?;
        }
        if let Some(proposal) = &judgment.proposal {
            transaction
                .execute(
                    "INSERT INTO proposal_feedback(receipt_id, record_id, outcome) VALUES (?1, ?2, ?3)",
                    params![
                        judgment.receipt_id.to_string(),
                        proposal.record_id.to_string(),
                        proposal.outcome.as_str()
                    ],
                )
                .map_err(|source| db_error("project proposal feedback", source))?;
        }
    }
    transaction
        .commit()
        .map_err(|source| db_error("commit the receipt feedback projection", source))?;
    connection
        .execute_batch("PRAGMA optimize;")
        .map_err(|source| db_error("optimize the receipt feedback projection", source))
}

fn db_error(operation: &'static str, source: rusqlite::Error) -> Error {
    Error::Index { operation, source }
}

mod record_id_serde {
    use serde::{Deserialize, Deserializer, Serializer};

    use crate::RecordId;

    pub(super) fn serialize<S>(record_id: &RecordId, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.collect_str(record_id)
    }

    pub(super) fn deserialize<'de, D>(deserializer: D) -> Result<RecordId, D::Error>
    where
        D: Deserializer<'de>,
    {
        let value = String::deserialize(deserializer)?;
        RecordId::parse(&value).map_err(serde::de::Error::custom)
    }
}
