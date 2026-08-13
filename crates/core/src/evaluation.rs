use std::collections::{BTreeMap, HashMap, HashSet};
use std::fs;
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

use serde::{Deserialize, Serialize};

mod capture_policy;
mod usefulness;

pub use capture_policy::{
    CaptureDisposition, CaptureEvent, CapturePolicyReport, CaptureReason,
    run_synthetic_capture_policy_evaluation,
};
pub use usefulness::{
    ProposalOutcomeRates, UsefulnessBreakdown, UsefulnessComparisonReport, UsefulnessReport,
    run_synthetic_usefulness_evaluation,
};

use crate::{
    ContextOptions, DeterministicEmbedder, Embedder, LocalEmbedder, PlatformDirs, Record,
    RecordKind, RecordStatus, Scope, SearchOptions, Source, SourceKind, StoreInitMode, StorePaths,
    StoreScope, Timestamp, context_stores, context_stores_with_embedder, ensure_default_model,
    initialize_store, rebuild_vector_index, render_markdown, search_stores,
};

const CORPUS_JSON: &str = include_str!("../tests/fixtures/evaluation/corpus.json");
const QUERIES_JSON: &str = include_str!("../tests/fixtures/evaluation/queries.json");
const SUMMARY_JSON: &str = include_str!("../tests/fixtures/evaluation/summary.json");
#[allow(dead_code)]
const RAG_JSON: &str = include_str!("../tests/fixtures/evaluation/rag.json");
#[allow(dead_code)]
const ANSWERS_JSON: &str = include_str!("../tests/fixtures/evaluation/answer-artifacts.json");

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
    /// Fraction of queries whose unscoped ranking probe returned a different scope.
    pub wrong_scope_retrieval_rate: f64,
    pub superseded_memory_retrieval_rate: f64,
    /// Fraction of expected records recovered for conflict queries within the top-five window.
    pub duplicate_or_conflicting_retrieval_rate: f64,
    pub context_tokens_per_useful_memory: f64,
}

#[derive(Clone, Debug, Serialize)]
pub struct EvaluationReport {
    pub corpus_revision: String,
    pub model_version: String,
    pub query_count: usize,
    pub metrics: BTreeMap<String, EvaluationModeReport>,
    pub usefulness: UsefulnessComparisonReport,
    pub capture_policy: CapturePolicyReport,
    pub thresholds: BTreeMap<String, f64>,
    pub passed: bool,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct ExpectedClaim {
    pub id: String,
    pub text: String,
    pub supporting_record_ids: Vec<String>,
    pub contradicting_record_ids: Vec<String>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct RagQuestion {
    pub id: String,
    pub query: String,
    pub scope: String,
    pub budget: usize,
    pub expected_context_record_ids: Vec<String>,
    pub expected_claims: Vec<ExpectedClaim>,
    pub expected_abstention: bool,
    pub answer_keywords: Vec<String>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct AnswerClaim {
    pub claim_id: String,
    pub text: String,
    pub citations: Vec<String>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct AnswerArtifact {
    pub question_id: String,
    pub answer: String,
    pub abstained: bool,
    pub claims: Vec<AnswerClaim>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct EvaluationMetadata {
    pub corpus_revision: String,
    pub generator: String,
    pub model: String,
    pub model_version: String,
    pub prompt_contract_version: String,
    pub parameters: BTreeMap<String, String>,
}

#[derive(Clone, Debug, Serialize)]
pub struct EvaluationStageReport {
    pub failure_count: usize,
    pub failure_question_ids: Vec<String>,
}

#[derive(Clone, Debug, Serialize)]
pub struct GroundedEvaluationMetrics {
    pub context_precision: f64,
    pub context_recall: f64,
    pub claim_support: f64,
    pub citation_precision: f64,
    pub citation_recall: f64,
    pub answer_relevance: f64,
    pub correct_abstention: f64,
    pub scope_leakage: f64,
}

#[derive(Clone, Debug, Serialize)]
pub struct GroundedQuestionReport {
    pub question_id: String,
    pub retrieved_record_ids: Vec<String>,
    pub context_record_ids: Vec<String>,
    pub retrieval_passed: bool,
    pub context_precision: f64,
    pub context_recall: f64,
    pub context_assembly_passed: bool,
    pub claim_support: f64,
    pub citation_precision: f64,
    pub citation_recall: f64,
    pub answer_relevance: f64,
    pub correct_abstention: bool,
    pub generation_passed: bool,
    pub failure_stages: Vec<String>,
}

#[derive(Clone, Debug, Serialize)]
pub struct GroundedEvaluationReport {
    pub corpus_revision: String,
    pub reproducibility: EvaluationMetadata,
    pub question_count: usize,
    pub retrieval: EvaluationStageReport,
    pub context_assembly: EvaluationStageReport,
    pub generation: EvaluationStageReport,
    pub metrics: GroundedEvaluationMetrics,
    pub questions: Vec<GroundedQuestionReport>,
    pub passed: bool,
}

/// A provider-neutral adapter for evaluating answer artifacts supplied by a host.
/// It never starts a generator and has no network or provider dependency.
#[allow(dead_code)]
#[derive(Clone, Debug)]
pub struct HostModelEvaluationAdapter {
    pub metadata: EvaluationMetadata,
}

#[allow(dead_code)]
impl HostModelEvaluationAdapter {
    pub fn new(metadata: EvaluationMetadata) -> Self {
        Self { metadata }
    }

    pub fn evaluate(&self, answers: &[AnswerArtifact]) -> crate::Result<GroundedEvaluationReport> {
        run_grounded_evaluation(&self.metadata, answers)
    }
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

/// Evaluate the checked-in RAG fixtures using supplied answer artifacts. This
/// deterministic path is useful in CI and does not invoke or contact a model.
#[allow(dead_code)]
pub fn run_synthetic_grounded_evaluation() -> crate::Result<GroundedEvaluationReport> {
    let fixture: RagFixtureFile = serde_json::from_str(RAG_JSON)
        .map_err(|error| crate::Error::invalid_input(format!("invalid RAG fixture: {error}")))?;
    let answers = parse_answer_file(ANSWERS_JSON)?;
    let metadata = EvaluationMetadata {
        corpus_revision: fixture.revision,
        generator: "supplied-answer-artifact-adapter".to_owned(),
        model: "host-model".to_owned(),
        model_version: "fixture-answer-v1".to_owned(),
        prompt_contract_version: fixture.prompt_contract_version,
        parameters: BTreeMap::from([
            ("temperature".to_owned(), "0".to_owned()),
            ("top_p".to_owned(), "1".to_owned()),
            ("artifact_source".to_owned(), "checked-in".to_owned()),
        ]),
    };
    HostModelEvaluationAdapter::new(metadata).evaluate(&answers)
}

#[allow(dead_code)]
#[derive(Clone, Debug, Deserialize)]
struct RagFixtureFile {
    revision: String,
    prompt_contract_version: String,
    records: Vec<FixtureRecord>,
    questions: Vec<RagQuestion>,
}

#[allow(dead_code)]
#[derive(Clone, Debug, Deserialize)]
struct AnswerFile {
    answers: Vec<AnswerArtifact>,
}

#[allow(dead_code)]
fn parse_answer_file(contents: &str) -> crate::Result<Vec<AnswerArtifact>> {
    serde_json::from_str::<AnswerFile>(contents)
        .map(|file| file.answers)
        .map_err(|error| crate::Error::invalid_input(format!("invalid answer artifacts: {error}")))
}

#[allow(dead_code)]
fn run_grounded_evaluation(
    metadata: &EvaluationMetadata,
    answers: &[AnswerArtifact],
) -> crate::Result<GroundedEvaluationReport> {
    let fixture: RagFixtureFile = serde_json::from_str(RAG_JSON)
        .map_err(|error| crate::Error::invalid_input(format!("invalid RAG fixture: {error}")))?;
    if metadata.corpus_revision != fixture.revision {
        return Err(crate::Error::invalid_input(
            "RAG evaluation corpus revision does not match the configured run",
        ));
    }
    if metadata.prompt_contract_version != fixture.prompt_contract_version {
        return Err(crate::Error::invalid_input(
            "RAG evaluation prompt contract version does not match the fixture",
        ));
    }
    validate_rag_fixture(&fixture)?;
    let mut answer_by_question = HashMap::new();
    for answer in answers {
        if answer_by_question
            .insert(answer.question_id.clone(), answer)
            .is_some()
        {
            return Err(crate::Error::invalid_input(format!(
                "duplicate answer artifact for question {}",
                answer.question_id
            )));
        }
    }
    let question_ids: HashSet<_> = fixture
        .questions
        .iter()
        .map(|question| question.id.as_str())
        .collect();
    if answer_by_question
        .keys()
        .any(|id| !question_ids.contains(id.as_str()))
    {
        return Err(crate::Error::invalid_input(
            "answer artifact names an unknown RAG question",
        ));
    }
    if answer_by_question.len() != fixture.questions.len() {
        return Err(crate::Error::invalid_input(
            "RAG evaluation requires one answer artifact for every fixture question",
        ));
    }

    let root = temporary_root();
    let paths = StorePaths {
        scope: StoreScope::Global,
        root: root.clone(),
        records: root.join("records"),
        cache: root.join("cache"),
    };
    let result = (|| {
        initialize_store(&paths, StoreInitMode::Default)?;
        for record in &fixture.records {
            let record = fixture_record(record)?;
            let path = paths.records.join(format!("{}.md", record.id));
            fs::write(&path, render_markdown(&record)?)
                .map_err(|source| crate::Error::io("write RAG evaluation record", source))?;
        }
        crate::sync_store(&paths)?;

        let mut question_reports = Vec::with_capacity(fixture.questions.len());
        let mut retrieval_failures = Vec::new();
        let mut context_failures = Vec::new();
        let mut generation_failures = Vec::new();
        let mut metric_totals = GroundedMetricTotals::default();
        for question in &fixture.questions {
            let search = SearchOptions {
                limit: 20,
                current_scope: Some(question.scope.clone()),
                allowed_scopes: Some(vec![question.scope.clone()]),
                ..SearchOptions::default()
            };
            let search_results = search_stores(
                std::slice::from_ref(&paths),
                &question.query,
                search.clone(),
            )?;
            let retrieved_record_ids = unique_ids(
                search_results
                    .iter()
                    .map(|result| result.record_id.as_str()),
            );
            let expected: HashSet<_> = question
                .expected_context_record_ids
                .iter()
                .map(String::as_str)
                .collect();
            let retrieved: HashSet<_> = retrieved_record_ids.iter().map(String::as_str).collect();
            let retrieval_passed = expected.is_subset(&retrieved)
                && (expected.is_empty() == retrieved.is_empty())
                && search_results
                    .iter()
                    .all(|result| result.scope == question.scope);
            if !retrieval_passed {
                retrieval_failures.push(question.id.clone());
            }

            let context = context_stores(
                std::slice::from_ref(&paths),
                &question.query,
                ContextOptions {
                    budget: question.budget,
                    search,
                },
            )?;
            let context_record_ids =
                unique_ids(context.blocks.iter().map(|block| block.record_id.as_str()));
            let context_precision = precision(&context_record_ids, &expected);
            let context_recall = recall(&context_record_ids, &expected);
            let scope_leak = context
                .blocks
                .iter()
                .any(|block| block.scope != question.scope);
            let context_assembly_passed =
                context_precision == 1.0 && context_recall == 1.0 && !scope_leak;
            if !context_assembly_passed {
                context_failures.push(question.id.clone());
            }

            let answer = answer_by_question
                .get(&question.id)
                .copied()
                .ok_or_else(|| crate::Error::invalid_input("missing RAG answer artifact"))?;
            let generation = evaluate_generation(question, answer, &context_record_ids);
            if !generation.passed {
                generation_failures.push(question.id.clone());
            }
            let mut failure_stages = Vec::new();
            if !retrieval_passed {
                failure_stages.push("retrieval".to_owned());
            }
            if !context_assembly_passed {
                failure_stages.push("context_assembly".to_owned());
            }
            if !generation.passed {
                failure_stages.push("generation".to_owned());
            }
            metric_totals.add(context_precision, context_recall, &generation, scope_leak);
            question_reports.push(GroundedQuestionReport {
                question_id: question.id.clone(),
                retrieved_record_ids,
                context_record_ids,
                retrieval_passed,
                context_precision,
                context_recall,
                context_assembly_passed,
                claim_support: generation.claim_support,
                citation_precision: generation.citation_precision,
                citation_recall: generation.citation_recall,
                answer_relevance: generation.answer_relevance,
                correct_abstention: generation.correct_abstention,
                generation_passed: generation.passed,
                failure_stages,
            });
        }
        let question_count = fixture.questions.len();
        let metrics = metric_totals.finish(question_count);
        Ok(GroundedEvaluationReport {
            corpus_revision: fixture.revision,
            reproducibility: metadata.clone(),
            question_count,
            retrieval: stage_report(retrieval_failures),
            context_assembly: stage_report(context_failures),
            generation: stage_report(generation_failures),
            metrics,
            passed: question_reports
                .iter()
                .all(|report| report.failure_stages.is_empty()),
            questions: question_reports,
        })
    })();
    let _ = fs::remove_dir_all(root);
    result
}

#[allow(dead_code)]
fn validate_rag_fixture(fixture: &RagFixtureFile) -> crate::Result<()> {
    let records: HashSet<_> = fixture
        .records
        .iter()
        .map(|record| record.id.as_str())
        .collect();
    if records.len() != fixture.records.len() {
        return Err(crate::Error::invalid_input(
            "RAG fixture contains duplicate record IDs",
        ));
    }
    let mut question_ids = HashSet::new();
    for question in &fixture.questions {
        if !question_ids.insert(question.id.as_str()) {
            return Err(crate::Error::invalid_input(
                "RAG fixture contains duplicate question IDs",
            ));
        }
        for id in &question.expected_context_record_ids {
            if !records.contains(id.as_str()) {
                return Err(crate::Error::invalid_input(format!(
                    "RAG question {} names a missing context record",
                    question.id
                )));
            }
        }
        for claim in &question.expected_claims {
            if claim.supporting_record_ids.is_empty() && claim.contradicting_record_ids.is_empty() {
                return Err(crate::Error::invalid_input(format!(
                    "RAG claim {} names no supporting or contradicting record",
                    claim.id
                )));
            }
            for id in claim
                .supporting_record_ids
                .iter()
                .chain(&claim.contradicting_record_ids)
            {
                if !records.contains(id.as_str()) {
                    return Err(crate::Error::invalid_input(format!(
                        "RAG claim {} names a missing record",
                        claim.id
                    )));
                }
            }
        }
        if !question.expected_abstention && question.expected_claims.is_empty() {
            return Err(crate::Error::invalid_input(format!(
                "answerable RAG question {} has no expected claims",
                question.id
            )));
        }
    }
    Ok(())
}

#[allow(dead_code)]
fn unique_ids<'a>(ids: impl Iterator<Item = &'a str>) -> Vec<String> {
    let mut seen = HashSet::new();
    ids.filter(|id| seen.insert(*id))
        .map(str::to_owned)
        .collect()
}

#[allow(dead_code)]
fn precision(retrieved: &[String], expected: &HashSet<&str>) -> f64 {
    if retrieved.is_empty() {
        return if expected.is_empty() { 1.0 } else { 0.0 };
    }
    retrieved
        .iter()
        .filter(|id| expected.contains(id.as_str()))
        .count() as f64
        / retrieved.len() as f64
}

#[allow(dead_code)]
fn recall(retrieved: &[String], expected: &HashSet<&str>) -> f64 {
    if expected.is_empty() {
        return if retrieved.is_empty() { 1.0 } else { 0.0 };
    }
    retrieved
        .iter()
        .filter(|id| expected.contains(id.as_str()))
        .collect::<HashSet<_>>()
        .len() as f64
        / expected.len() as f64
}

#[allow(dead_code)]
#[derive(Default)]
struct GroundedMetricTotals {
    context_precision: f64,
    context_recall: f64,
    claim_support: f64,
    citation_precision: f64,
    citation_recall: f64,
    answer_relevance: f64,
    correct_abstention: f64,
    scope_leakage: f64,
}

#[allow(dead_code)]
impl GroundedMetricTotals {
    fn add(
        &mut self,
        context_precision: f64,
        context_recall: f64,
        generation: &GenerationResult,
        scope_leak: bool,
    ) {
        self.context_precision += context_precision;
        self.context_recall += context_recall;
        self.claim_support += generation.claim_support;
        self.citation_precision += generation.citation_precision;
        self.citation_recall += generation.citation_recall;
        self.answer_relevance += generation.answer_relevance;
        self.correct_abstention += if generation.correct_abstention {
            1.0
        } else {
            0.0
        };
        self.scope_leakage += if scope_leak { 1.0 } else { 0.0 };
    }

    fn finish(self, question_count: usize) -> GroundedEvaluationMetrics {
        let count = question_count.max(1) as f64;
        GroundedEvaluationMetrics {
            context_precision: self.context_precision / count,
            context_recall: self.context_recall / count,
            claim_support: self.claim_support / count,
            citation_precision: self.citation_precision / count,
            citation_recall: self.citation_recall / count,
            answer_relevance: self.answer_relevance / count,
            correct_abstention: self.correct_abstention / count,
            scope_leakage: self.scope_leakage / count,
        }
    }
}

#[allow(dead_code)]
struct GenerationResult {
    claim_support: f64,
    citation_precision: f64,
    citation_recall: f64,
    answer_relevance: f64,
    correct_abstention: bool,
    passed: bool,
}

#[allow(dead_code)]
fn evaluate_generation(
    question: &RagQuestion,
    answer: &AnswerArtifact,
    context_record_ids: &[String],
) -> GenerationResult {
    let expected_claims: HashMap<_, _> = question
        .expected_claims
        .iter()
        .map(|claim| (claim.id.as_str(), claim))
        .collect();
    let mut supported_claims = 0;
    let mut valid_citations = 0;
    let mut generated_citations = 0;
    let mut cited_expected = HashSet::new();
    let context_ids: HashSet<_> = context_record_ids.iter().map(String::as_str).collect();
    let generated_claim_ids: HashSet<_> = answer
        .claims
        .iter()
        .map(|claim| claim.claim_id.as_str())
        .collect();
    let claims_complete = generated_claim_ids.len() == answer.claims.len()
        && generated_claim_ids.len() == expected_claims.len()
        && expected_claims
            .keys()
            .all(|claim_id| generated_claim_ids.contains(claim_id));
    for claim in &answer.claims {
        let expected_ids: HashSet<_> = expected_claims
            .get(claim.claim_id.as_str())
            .map(|expected| {
                expected
                    .supporting_record_ids
                    .iter()
                    .chain(&expected.contradicting_record_ids)
                    .map(String::as_str)
                    .collect()
            })
            .unwrap_or_default();
        let claim_citations: HashSet<_> = claim.citations.iter().map(String::as_str).collect();
        if expected_claims
            .get(claim.claim_id.as_str())
            .is_some_and(|expected| {
                normalize_claim(&claim.text) == normalize_claim(&expected.text)
                    && expected_ids.is_subset(&claim_citations)
                    && claim_citations.is_subset(&context_ids)
                    && !expected.supporting_record_ids.is_empty()
                    && expected
                        .supporting_record_ids
                        .iter()
                        .all(|id| claim_citations.contains(id.as_str()))
            })
        {
            supported_claims += 1;
        }
        for citation in &claim.citations {
            generated_citations += 1;
            if expected_ids.contains(citation.as_str()) && context_ids.contains(citation.as_str()) {
                valid_citations += 1;
                cited_expected.insert(citation.as_str());
            }
        }
    }
    let expected_citations: HashSet<_> = question
        .expected_claims
        .iter()
        .flat_map(|claim| {
            claim
                .supporting_record_ids
                .iter()
                .chain(&claim.contradicting_record_ids)
                .map(String::as_str)
        })
        .collect();
    let claim_support = if question.expected_claims.is_empty() {
        if answer.claims.is_empty() { 1.0 } else { 0.0 }
    } else {
        supported_claims as f64 / question.expected_claims.len() as f64
    };
    let citation_precision = if generated_citations == 0 {
        if expected_citations.is_empty() {
            1.0
        } else {
            0.0
        }
    } else {
        valid_citations as f64 / generated_citations as f64
    };
    let citation_recall = if expected_citations.is_empty() {
        if cited_expected.is_empty() { 1.0 } else { 0.0 }
    } else {
        cited_expected.len() as f64 / expected_citations.len() as f64
    };
    let correct_abstention = answer.abstained == question.expected_abstention
        && (!question.expected_abstention || answer.claims.is_empty());
    let answer_lower = answer.answer.to_lowercase();
    let answer_relevance = if question.answer_keywords.is_empty() {
        1.0
    } else {
        question
            .answer_keywords
            .iter()
            .filter(|keyword| answer_lower.contains(&keyword.to_lowercase()))
            .count() as f64
            / question.answer_keywords.len() as f64
    };
    let passed = correct_abstention
        && claims_complete
        && claim_support == 1.0
        && citation_precision == 1.0
        && citation_recall == 1.0
        && answer_relevance == 1.0;
    GenerationResult {
        claim_support,
        citation_precision,
        citation_recall,
        answer_relevance,
        correct_abstention,
        passed,
    }
}

fn normalize_claim(value: &str) -> String {
    value
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
        .to_lowercase()
}

#[allow(dead_code)]
fn stage_report(mut failure_question_ids: Vec<String>) -> EvaluationStageReport {
    failure_question_ids.sort();
    EvaluationStageReport {
        failure_count: failure_question_ids.len(),
        failure_question_ids,
    }
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
        let usefulness = run_synthetic_usefulness_evaluation()?;
        let capture_policy = run_synthetic_capture_policy_evaluation()?;
        let passed = capture_policy.passed
            && metrics
                .values()
                .all(|report| meets_thresholds(report, &thresholds));
        Ok(EvaluationReport {
            corpus_revision: corpus.revision,
            model_version: embedder.model_version().to_owned(),
            query_count: queries.queries.len(),
            metrics,
            usefulness,
            capture_policy,
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
            let found = query
                .expected_record_ids
                .iter()
                .filter(|id| results.iter().any(|result| &result.record_id == *id))
                .count();
            conflict_found += found as f64 / query.expected_record_ids.len() as f64;
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
        // Wrong-scope retrieval is intentionally measured on an unscoped probe. The
        // release gate remains the stable core policy boundary, which filters scopes before
        // returning results; the probe is reported for ranking review rather than hidden.
        && report.superseded_memory_retrieval_rate
            <= thresholds["superseded_memory_retrieval_rate_max"]
        && report.duplicate_or_conflicting_retrieval_rate
            >= thresholds["duplicate_or_conflicting_retrieval_rate_min"]
        && report.context_tokens_per_useful_memory
            <= thresholds["context_tokens_per_useful_memory_max"]
}

static NEXT_EVALUATION_ROOT: AtomicU64 = AtomicU64::new(0);

fn temporary_root() -> PathBuf {
    let stamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("clock")
        .as_nanos();
    let counter = NEXT_EVALUATION_ROOT.fetch_add(1, Ordering::Relaxed);
    std::env::temp_dir().join(format!(
        "stormbuffer-evaluation-{}-{stamp}-{counter}",
        std::process::id()
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn evaluation_reports_all_release_metrics_without_silent_expectation_updates() {
        let report = run_synthetic_evaluation().expect("evaluation");
        assert_eq!(report.corpus_revision, "m3-fixture-1");
        assert_eq!(report.query_count, 5);
        assert_eq!(report.usefulness.revision, "m5-usefulness-1");
        assert!(report.capture_policy.passed);
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

    #[test]
    fn grounded_fixture_evaluation_is_complete_reproducible_and_stage_separated() {
        let first = run_synthetic_grounded_evaluation().expect("grounded evaluation");
        let second = run_synthetic_grounded_evaluation().expect("grounded evaluation");
        assert!(
            first.passed,
            "{}",
            serde_json::to_string_pretty(&first).expect("report JSON")
        );
        assert_eq!(first.corpus_revision, "m4-rag-fixture-1");
        assert_eq!(first.question_count, 6);
        assert_eq!(first.retrieval.failure_count, 0);
        assert_eq!(first.context_assembly.failure_count, 0);
        assert_eq!(first.generation.failure_count, 0);
        assert_eq!(
            first.reproducibility.generator,
            "supplied-answer-artifact-adapter"
        );
        assert_eq!(
            first.reproducibility.prompt_contract_version,
            "stormbuffer-context-v1"
        );
        assert_eq!(
            serde_json::to_string(&first).expect("serialize report"),
            serde_json::to_string(&second).expect("serialize report")
        );
        assert!(first.metrics.context_precision.is_finite());
        assert!(first.metrics.context_recall.is_finite());
        assert!(first.metrics.claim_support.is_finite());
        assert!(first.metrics.citation_precision.is_finite());
        assert!(first.metrics.citation_recall.is_finite());
        assert!(first.metrics.answer_relevance.is_finite());
        assert!(first.metrics.correct_abstention.is_finite());
        assert_eq!(first.metrics.scope_leakage, 0.0);
    }

    #[test]
    fn supplied_bad_answer_is_a_generation_failure_only() {
        let mut answers = parse_answer_file(ANSWERS_JSON).expect("answer fixtures");
        let answer = answers
            .iter_mut()
            .find(|answer| answer.question_id == "prompt-injection")
            .expect("injection answer");
        answer.claims[0].citations = vec!["01989af2-4305-7b19-88b1-e8ae4ea9b105".to_owned()];
        let fixture: RagFixtureFile = serde_json::from_str(RAG_JSON).expect("RAG fixture");
        let metadata = EvaluationMetadata {
            corpus_revision: fixture.revision,
            generator: "test-adapter".to_owned(),
            model: "supplied".to_owned(),
            model_version: "test-v1".to_owned(),
            prompt_contract_version: fixture.prompt_contract_version,
            parameters: BTreeMap::new(),
        };
        let report = HostModelEvaluationAdapter::new(metadata)
            .evaluate(&answers)
            .expect("evaluate bad answer");
        assert_eq!(report.retrieval.failure_count, 0);
        assert_eq!(report.context_assembly.failure_count, 0);
        assert_eq!(report.generation.failure_count, 1);
        let question = report
            .questions
            .iter()
            .find(|question| question.question_id == "prompt-injection")
            .expect("question report");
        assert_eq!(question.failure_stages, vec!["generation"]);
        assert_eq!(question.citation_precision, 0.0);
    }

    #[test]
    fn claim_text_must_match_the_checked_in_expectation() {
        let fixture: RagFixtureFile = serde_json::from_str(RAG_JSON).expect("RAG fixture");
        let question = fixture
            .questions
            .iter()
            .find(|question| !question.expected_claims.is_empty())
            .expect("answerable question");
        let mut answers = parse_answer_file(ANSWERS_JSON).expect("answer fixtures");
        let answer = answers
            .iter_mut()
            .find(|answer| answer.question_id == question.id)
            .expect("answer artifact");

        assert!(
            evaluate_generation(question, answer, &question.expected_context_record_ids).passed
        );
        answer.claims[0].text = "A fabricated claim with a borrowed citation.".to_owned();
        assert!(
            !evaluate_generation(question, answer, &question.expected_context_record_ids).passed
        );
    }
}
