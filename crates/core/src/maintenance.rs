use std::collections::HashSet;

use serde::Serialize;
use time::Duration;

use crate::repository::possible_overlap;
use crate::{
    Error, RecordId, RecordKind, RecordRepository, RecordStatus, Scope, SourceKind, StorePaths,
    Timestamp, advisory_relations,
};

#[derive(Clone, Debug, Default)]
pub struct InboxFilter {
    pub min_age_days: Option<u64>,
    pub kind: Option<RecordKind>,
    pub source: Option<SourceKind>,
    pub scope: Option<Scope>,
    pub possible_overlap: bool,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct InboxEntry {
    pub id: String,
    pub title: String,
    pub kind: String,
    pub scope: String,
    pub age_days: i64,
    pub sources: Vec<String>,
    pub possible_overlap_id: Option<String>,
}

pub fn candidate_inbox(paths: &StorePaths, filter: &InboxFilter) -> crate::Result<Vec<InboxEntry>> {
    let records = RecordRepository::new(paths.clone()).list_read_only(true)?;
    let now = Timestamp::now_utc().as_offset_datetime();
    let mut entries = Vec::new();
    for stored in records
        .iter()
        .filter(|stored| stored.record().status == RecordStatus::Candidate)
    {
        let record = stored.record();
        let age_days = (now - record.created_at.as_offset_datetime())
            .whole_days()
            .max(0);
        let overlap = possible_overlap(&records, record);
        if filter
            .min_age_days
            .is_some_and(|days| age_days < days as i64)
            || filter.kind.is_some_and(|kind| record.kind != kind)
            || filter
                .source
                .is_some_and(|source| !record.sources.iter().any(|item| item.kind == source))
            || filter
                .scope
                .as_ref()
                .is_some_and(|scope| &record.scope != scope)
            || (filter.possible_overlap && overlap.is_none())
        {
            continue;
        }
        entries.push(InboxEntry {
            id: record.id.to_string(),
            title: record.title.clone(),
            kind: record.kind.to_string(),
            scope: record.scope.to_string(),
            age_days,
            sources: record
                .sources
                .iter()
                .map(|source| source.kind.to_string())
                .collect(),
            possible_overlap_id: overlap.map(|item| item.record().id.to_string()),
        });
    }
    Ok(entries)
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct AuditFinding {
    pub kind: String,
    pub record_ids: Vec<String>,
    pub evidence: String,
    pub confidence: String,
    pub rule: String,
    pub follow_up: String,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct AuditReport {
    pub findings: Vec<AuditFinding>,
}

pub fn audit_store(paths: &StorePaths, stale_after_days: u64) -> crate::Result<AuditReport> {
    let records = RecordRepository::new(paths.clone()).list_read_only(true)?;
    let ids: HashSet<RecordId> = records.iter().map(|stored| stored.record().id).collect();
    let now = Timestamp::now_utc().as_offset_datetime();
    let stale_seconds = stale_after_days
        .checked_mul(86_400)
        .and_then(|seconds| i64::try_from(seconds).ok())
        .ok_or_else(|| Error::invalid_input("--stale-after-days is too large"))?;
    let stale = Duration::seconds(stale_seconds);
    let command = format!("sbuf --{}", paths.scope);
    let mut findings = Vec::new();

    for stored in &records {
        let record = stored.record();
        if record.status == RecordStatus::Candidate {
            findings.push(AuditFinding {
                kind: "unresolved_candidate".to_owned(),
                record_ids: vec![record.id.to_string()],
                evidence: format!(
                    "candidate created at {} has no lifecycle decision",
                    record.created_at
                ),
                confidence: "certain".to_owned(),
                rule: "status is candidate".to_owned(),
                follow_up: format!(
                    "{command} approve {} or {command} reject {}",
                    record.id, record.id
                ),
            });
        }
        for missing in record.supersedes.iter().filter(|id| !ids.contains(id)) {
            findings.push(AuditFinding {
                kind: "broken_link".to_owned(),
                record_ids: vec![record.id.to_string(), missing.to_string()],
                evidence: format!("{} supersedes missing record {}", record.id, missing),
                confidence: "certain".to_owned(),
                rule: "every supersedes target must exist in the selected store".to_owned(),
                follow_up: format!("{command} edit {}", record.id),
            });
        }
        if record.kind == RecordKind::Checkpoint
            && record.status == RecordStatus::Active
            && now - record.updated_at.as_offset_datetime() >= stale
        {
            findings.push(AuditFinding {
                kind: "stale_checkpoint".to_owned(),
                record_ids: vec![record.id.to_string()],
                evidence: format!("checkpoint was last updated at {}", record.updated_at),
                confidence: "certain".to_owned(),
                rule: format!("active checkpoint age is at least {stale_after_days} days"),
                follow_up: format!("{command} archive {}", record.id),
            });
        }
    }

    for relation in advisory_relations(paths)? {
        let Ok(left_id) = relation.left_record_id.parse::<RecordId>() else {
            continue;
        };
        let Ok(right_id) = relation.right_record_id.parse::<RecordId>() else {
            continue;
        };
        if !ids.contains(&left_id) || !ids.contains(&right_id) {
            continue;
        }
        let target = match relation.relation.as_str() {
            "equivalent" | "entails" => &relation.right_record_id,
            "entailed_by" => &relation.left_record_id,
            _ => continue,
        };
        findings.push(AuditFinding {
            kind: "relation_duplicate_or_refinement".to_owned(),
            record_ids: vec![
                relation.left_record_id.clone(),
                relation.right_record_id.clone(),
            ],
            evidence: relation.evidence_json,
            confidence: relation.confidence,
            rule: format!("advisory relation is {}", relation.relation),
            follow_up: format!("{command} supersede {target}"),
        });
    }
    findings.sort_by(|left, right| {
        left.kind
            .cmp(&right.kind)
            .then(left.record_ids.cmp(&right.record_ids))
    });
    Ok(AuditReport { findings })
}
