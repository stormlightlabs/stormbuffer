use std::collections::{BTreeMap, HashSet};
use std::fs;
use std::path::PathBuf;
use std::time::{SystemTime, UNIX_EPOCH};

use serde::{Deserialize, Serialize};

use crate::{
    ContextOptions, DeterministicEmbedder, Embedder, LocalEmbedder, PlatformDirs, Record,
    RecordKind, RecordStatus, Scope, SearchOptions, Source, SourceKind, StoreInitMode, StorePaths,
    StoreScope, Timestamp, context_stores, context_stores_with_embedder, ensure_default_model,
    initialize_store, rebuild_vector_index, render_markdown, search_stores,
};

const CORPUS_JSON: &str = include_str!("../tests/fixtures/evaluation/corpus.json");
const QUERIES_JSON: &str = include_str!("../tests/fixtures/evaluation/queries.json");
const SUMMARY_JSON: &str = include_str!("../tests/fixtures/evaluation/summary.json");

#[derive(Clone, Debug, Deserialize)]
struct CorpusFile {
    revision: String,
    records: Vec<FixtureRecord>,
}

#[derive(Clone, Debug, Deserialize)]
struct QueryFile {
    queries: Vec<EvaluationQuery>,
}

#[derive(Clone, Debug, Deserialize)]
struct CheckedSummary {
    corpus_revision: String,
    metrics: BTreeMap<String, EvaluationModeReport>,
}

#[derive(Clone, Debug, Deserialize)]
struct FixtureRecord {
    id: String,
    title: String,
    kind: String,
    scope: String,
    status: String,
    body: String,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct EvaluationQuery {
    pub id: String,
    pub query: String,
    pub scope: String,
    pub expected_record_ids: Vec<String>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct EvaluationModeReport {
    pub recall_at_5: f64,
    pub mean_reciprocal_rank: f64,
    pub wrong_scope_retrieval_rate: f64,
    pub superseded_memory_retrieval_rate: f64,
    pub duplicate_or_conflicting_retrieval_rate: f64,
    pub context_tokens_per_useful_memory: f64,
}

#[derive(Clone, Debug, Serialize)]
pub struct EvaluationReport {
    pub corpus_revision: String,
    pub model_version: String,
    pub query_count: usize,
    pub metrics: BTreeMap<String, EvaluationModeReport>,
    pub thresholds: BTreeMap<String, f64>,
    pub passed: bool,
}

pub fn run_evaluation() -> crate::Result<EvaluationReport> {
    let dirs = PlatformDirs::from_environment()?;
    let model_paths = StorePaths {
        scope: StoreScope::Global,
        root: dirs.data_root().join("stormbuffer"),
        records: dirs.data_root().join("stormbuffer").join("records"),
        cache: dirs.cache_root().join("stormbuffer"),
    };
    ensure_default_model(&model_paths)?;
    let embedder = LocalEmbedder::from_default_cache(&model_paths)?;
    run_evaluation_with_embedder(&embedder, true)
}

/// Run the deterministic fixture evaluation without installing or loading a model.
/// This is for regression tests; the `evaluate` command uses `run_evaluation`.
pub fn run_synthetic_evaluation() -> crate::Result<EvaluationReport> {
    let embedder = DeterministicEmbedder::new("fixture-m3-v1", 32)?;
    run_evaluation_with_embedder(&embedder, false)
}

fn run_evaluation_with_embedder(
    embedder: &dyn Embedder,
    verify_summary: bool,
) -> crate::Result<EvaluationReport> {
    let corpus: CorpusFile = serde_json::from_str(CORPUS_JSON).map_err(|error| {
        crate::Error::invalid_input(format!("invalid evaluation corpus: {error}"))
    })?;
    let queries: QueryFile = serde_json::from_str(QUERIES_JSON).map_err(|error| {
        crate::Error::invalid_input(format!("invalid evaluation queries: {error}"))
    })?;
    let root = temporary_root();
    let paths = StorePaths {
        scope: StoreScope::Global,
        root: root.clone(),
        records: root.join("records"),
        cache: root.join("cache"),
    };
    let result = (|| {
        initialize_store(&paths, StoreInitMode::Default)?;
        for fixture in &corpus.records {
            let record = fixture_record(fixture)?;
            let path = paths.records.join(format!("{}.md", fixture.id));
            fs::write(&path, render_markdown(&record)?)
                .map_err(|source| crate::Error::io("write evaluation record", source))?;
        }
        crate::sync_store(&paths)?;
        rebuild_vector_index(&paths, embedder)?;
        let allowed_scopes = corpus
            .records
            .iter()
            .map(|record| record.scope.clone())
            .collect::<HashSet<_>>();

        let mut metrics = BTreeMap::new();
        metrics.insert(
            "fts-only".to_owned(),
            evaluate_mode(&paths, &queries.queries, None, &allowed_scopes)?,
        );
        metrics.insert(
            "vector-only".to_owned(),
            evaluate_mode(
                &paths,
                &queries.queries,
                Some((embedder, crate::RetrievalMode::Semantic)),
                &allowed_scopes,
            )?,
        );
        metrics.insert(
            "hybrid".to_owned(),
            evaluate_mode(
                &paths,
                &queries.queries,
                Some((embedder, crate::RetrievalMode::Hybrid)),
                &allowed_scopes,
            )?,
        );
        if verify_summary {
            verify_checked_summary(&corpus.revision, &metrics)?;
        }
        let thresholds = thresholds();
        let passed = metrics
            .values()
            .all(|report| meets_thresholds(report, &thresholds));
        Ok(EvaluationReport {
            corpus_revision: corpus.revision,
            model_version: embedder.model_version().to_owned(),
            query_count: queries.queries.len(),
            metrics,
            thresholds,
            passed,
        })
    })();
    let _ = fs::remove_dir_all(root);
    result
}

fn evaluate_mode(
    paths: &StorePaths,
    queries: &[EvaluationQuery],
    semantic: Option<(&dyn Embedder, crate::RetrievalMode)>,
    allowed_scopes: &HashSet<String>,
) -> crate::Result<EvaluationModeReport> {
    let mut recall = 0.0;
    let mut reciprocal_rank = 0.0;
    let mut wrong_scope = 0.0;
    let mut superseded = 0.0;
    let mut conflict_total = 0.0;
    let mut conflict_found = 0.0;
    let mut context_tokens = 0.0;
    let mut useful_memories = 0.0;
    for query in queries {
        let mut options = SearchOptions::for_store(paths);
        // Deliberately search every fixture scope so cross-scope leakage is measured
        // instead of being hidden by the normal store policy filter.
        options.allowed_scopes = Some(allowed_scopes.iter().cloned().collect());
        options.current_scope = Some(query.scope.clone());
        options.limit = 5;
        let results = match semantic {
            Some((embedder, mode)) => {
                options.mode = mode;
                crate::search_stores_with_embedder(
                    &[paths.clone()],
                    &query.query,
                    options.clone(),
                    embedder,
                )?
            }
            None => search_stores(&[paths.clone()], &query.query, options.clone())?,
        };
        let expected: HashSet<_> = query.expected_record_ids.iter().collect();
        if results.iter().any(|result| result.scope != query.scope) {
            wrong_scope += 1.0;
        }
        if !expected.is_empty() {
            if results
                .iter()
                .any(|result| expected.contains(&result.record_id))
            {
                recall += 1.0;
            }
            if let Some(position) = results
                .iter()
                .position(|result| expected.contains(&result.record_id))
            {
                reciprocal_rank += 1.0 / (position as f64 + 1.0);
            }
        }
        if results.iter().any(|result| result.status == "superseded") {
            superseded += 1.0;
        }
        if query.expected_record_ids.len() > 1 {
            conflict_total += 1.0;
            if query
                .expected_record_ids
                .iter()
                .all(|id| results.iter().any(|result| &result.record_id == id))
            {
                conflict_found += 1.0;
            }
        }
        let context = match semantic {
            Some((embedder, mode)) => {
                options.mode = mode;
                context_stores_with_embedder(
                    &[paths.clone()],
                    &query.query,
                    ContextOptions {
                        budget: 40,
                        search: options,
                    },
                    embedder,
                )?
            }
            None => context_stores(
                &[paths.clone()],
                &query.query,
                ContextOptions {
                    budget: 40,
                    search: options,
                },
            )?,
        };
        let useful = context
            .blocks
            .iter()
            .filter(|block| expected.contains(&block.record_id))
            .count();
        useful_memories += useful as f64;
        context_tokens += context.receipt.used_tokens as f64;
    }
    let query_count = queries.len() as f64;
    Ok(EvaluationModeReport {
        recall_at_5: recall / query_count,
        mean_reciprocal_rank: reciprocal_rank / query_count,
        wrong_scope_retrieval_rate: wrong_scope / query_count,
        superseded_memory_retrieval_rate: superseded / query_count,
        duplicate_or_conflicting_retrieval_rate: if conflict_total == 0.0 {
            0.0
        } else {
            conflict_found / conflict_total
        },
        context_tokens_per_useful_memory: context_tokens / useful_memories.max(1.0),
    })
}

fn fixture_record(fixture: &FixtureRecord) -> crate::Result<Record> {
    let now = Timestamp::parse("2026-08-05T20:09:00Z").map_err(crate::Error::invalid_input)?;
    Ok(Record {
        id: fixture.id.parse().map_err(crate::Error::invalid_input)?,
        title: fixture.title.clone(),
        kind: fixture
            .kind
            .parse::<RecordKind>()
            .map_err(crate::Error::invalid_input)?,
        scope: Scope::parse(&fixture.scope).map_err(crate::Error::invalid_input)?,
        status: fixture
            .status
            .parse::<RecordStatus>()
            .map_err(crate::Error::invalid_input)?,
        access: crate::Access::Human,
        created_at: now,
        updated_at: now,
        tags: vec!["evaluation".to_owned()],
        aliases: Vec::new(),
        supersedes: Vec::new(),
        sources: vec![Source {
            kind: SourceKind::Document,
            reference: "m3-fixture".to_owned(),
            actor: "test".to_owned(),
        }],
        body: fixture.body.clone(),
    })
}

fn verify_checked_summary(
    corpus_revision: &str,
    metrics: &BTreeMap<String, EvaluationModeReport>,
) -> crate::Result<()> {
    let expected: CheckedSummary = serde_json::from_str(SUMMARY_JSON).map_err(|error| {
        crate::Error::invalid_input(format!("invalid checked evaluation summary: {error}"))
    })?;
    if expected.corpus_revision != corpus_revision || expected.metrics.len() != metrics.len() {
        return Err(crate::Error::invalid_input(
            "evaluation differs from the checked-in summary; review corpus and ranking changes",
        ));
    }
    for (mode, actual) in metrics {
        let Some(expected) = expected.metrics.get(mode) else {
            return Err(crate::Error::invalid_input(format!(
                "evaluation mode {mode} is missing from the checked-in summary"
            )));
        };
        let values = [
            (actual.recall_at_5, expected.recall_at_5),
            (actual.mean_reciprocal_rank, expected.mean_reciprocal_rank),
            (
                actual.wrong_scope_retrieval_rate,
                expected.wrong_scope_retrieval_rate,
            ),
            (
                actual.superseded_memory_retrieval_rate,
                expected.superseded_memory_retrieval_rate,
            ),
            (
                actual.duplicate_or_conflicting_retrieval_rate,
                expected.duplicate_or_conflicting_retrieval_rate,
            ),
            (
                actual.context_tokens_per_useful_memory,
                expected.context_tokens_per_useful_memory,
            ),
        ];
        if values
            .iter()
            .any(|(actual, expected)| (actual - expected).abs() > 1e-9)
        {
            return Err(crate::Error::invalid_input(format!(
                "{mode} ranking metrics differ from the checked-in summary; review expected results"
            )));
        }
    }
    Ok(())
}

fn thresholds() -> BTreeMap<String, f64> {
    BTreeMap::from([
        ("recall_at_5_min".to_owned(), 0.80),
        ("mean_reciprocal_rank_min".to_owned(), 0.60),
        ("wrong_scope_retrieval_rate_max".to_owned(), 0.0),
        ("superseded_memory_retrieval_rate_max".to_owned(), 0.0),
        (
            "duplicate_or_conflicting_retrieval_rate_min".to_owned(),
            0.50,
        ),
        ("context_tokens_per_useful_memory_max".to_owned(), 40.0),
    ])
}

fn meets_thresholds(report: &EvaluationModeReport, thresholds: &BTreeMap<String, f64>) -> bool {
    report.recall_at_5 >= thresholds["recall_at_5_min"]
        && report.mean_reciprocal_rank >= thresholds["mean_reciprocal_rank_min"]
        && report.wrong_scope_retrieval_rate <= thresholds["wrong_scope_retrieval_rate_max"]
        && report.superseded_memory_retrieval_rate
            <= thresholds["superseded_memory_retrieval_rate_max"]
        && report.duplicate_or_conflicting_retrieval_rate
            >= thresholds["duplicate_or_conflicting_retrieval_rate_min"]
        && report.context_tokens_per_useful_memory
            <= thresholds["context_tokens_per_useful_memory_max"]
}

fn temporary_root() -> PathBuf {
    let stamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("clock")
        .as_nanos();
    std::env::temp_dir().join(format!("stormbuffer-evaluation-{stamp}"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn evaluation_reports_all_release_metrics_without_silent_expectation_updates() {
        let report = run_synthetic_evaluation().expect("evaluation");
        assert_eq!(report.corpus_revision, "m3-fixture-1");
        assert_eq!(report.query_count, 5);
        assert!(report.metrics["fts-only"].wrong_scope_retrieval_rate > 0.0);
        assert!(report.metrics["vector-only"].wrong_scope_retrieval_rate > 0.0);
        for mode in ["fts-only", "vector-only", "hybrid"] {
            let metrics = &report.metrics[mode];
            assert!(metrics.recall_at_5.is_finite());
            assert!(metrics.mean_reciprocal_rank.is_finite());
            assert!(metrics.wrong_scope_retrieval_rate.is_finite());
            assert!(metrics.superseded_memory_retrieval_rate.is_finite());
            assert!(metrics.duplicate_or_conflicting_retrieval_rate.is_finite());
            assert!(metrics.context_tokens_per_useful_memory.is_finite());
        }
    }
}
