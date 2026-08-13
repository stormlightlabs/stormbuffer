use anyhow::Result as AnyhowResult;
use stormbuffer_core::{self as core, StoreScope};

use crate::command::{AuditArgs, InboxArgs};
use crate::echo::Echo;
use crate::{report_error, resolve};

pub(super) fn run_inbox(scope: StoreScope, arguments: InboxArgs, output: &Echo) -> i32 {
    let paths = match resolve(scope) {
        Ok(paths) => paths,
        Err(error) => return report_error(error, output),
    };
    let filter = match inbox_filter(&arguments) {
        Ok(filter) => filter,
        Err(error) => return report_error(error, output),
    };
    let entries = match core::candidate_inbox(&paths, &filter) {
        Ok(entries) => entries,
        Err(error) => return report_error(anyhow::Error::new(error), output),
    };
    if arguments.json {
        return match serde_json::to_vec(&entries) {
            Ok(mut encoded) => {
                encoded.push(b'\n');
                output.raw(&encoded);
                0
            }
            Err(error) => report_error(anyhow::Error::new(error), output),
        };
    }
    for entry in &entries {
        output.line(&format!(
            "{} | {} | {} | {} | {} days{}",
            entry.id,
            entry.kind,
            entry.scope,
            entry.title,
            entry.age_days,
            entry
                .possible_overlap_id
                .as_ref()
                .map_or_else(String::new, |id| format!(" | possible overlap: {id}"))
        ));
    }
    output.field("Candidates", entries.len());
    0
}

pub(super) fn run_audit(scope: StoreScope, arguments: AuditArgs, output: &Echo) -> i32 {
    let paths = match resolve(scope) {
        Ok(paths) => paths,
        Err(error) => return report_error(error, output),
    };
    let report = match core::audit_store(&paths, arguments.stale_after_days) {
        Ok(report) => report,
        Err(error) => return report_error(anyhow::Error::new(error), output),
    };
    if arguments.json {
        return match serde_json::to_vec(&report) {
            Ok(mut encoded) => {
                encoded.push(b'\n');
                output.raw(&encoded);
                0
            }
            Err(error) => report_error(anyhow::Error::new(error), output),
        };
    }
    for finding in &report.findings {
        output.line(&format!(
            "{} | {} | {}",
            finding.kind,
            finding.confidence,
            finding.record_ids.join(", ")
        ));
        output.field("Evidence", &finding.evidence);
        output.field("Rule", &finding.rule);
        output.field("Follow up", &finding.follow_up);
    }
    output.field("Findings", report.findings.len());
    0
}

fn inbox_filter(arguments: &InboxArgs) -> AnyhowResult<core::InboxFilter> {
    Ok(core::InboxFilter {
        min_age_days: arguments.min_age_days,
        kind: arguments
            .kind
            .as_deref()
            .map(str::parse)
            .transpose()
            .map_err(anyhow::Error::msg)?,
        source: arguments
            .source
            .as_deref()
            .map(str::parse)
            .transpose()
            .map_err(anyhow::Error::msg)?,
        scope: arguments
            .scope
            .as_deref()
            .map(str::parse)
            .transpose()
            .map_err(anyhow::Error::msg)?,
        possible_overlap: arguments.possible_overlap,
    })
}
