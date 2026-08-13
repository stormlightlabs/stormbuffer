use std::time::Duration;

use serde::Serialize;

use super::{DEFAULT_SEARCH_LIMIT, MAX_SEARCH_LIMIT};
use crate::{Access, ReceiptId, StorePaths, StoreScope};

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
/// Retrieval strategy used to rank matching record chunks.
pub enum RetrievalMode {
    /// Rank full-text matches without consulting the vector index.
    Lexical,
    /// Rank nearest vector matches without full-text score fusion.
    Semantic,
    /// Fuse lexical and semantic rankings.
    #[default]
    Hybrid,
}

#[derive(Clone, Debug)]
/// Filters and ranking policy shared by search and context compilation.
pub struct SearchOptions {
    /// Maximum results requested. Retrieval clamps this to its supported range.
    pub limit: usize,
    /// Include candidate, superseded, and archived records in addition to active records.
    pub include_inactive: bool,
    /// Scope that receives the local-result ranking boost.
    pub current_scope: Option<String>,
    /// Scopes eligible for retrieval. `None` derives them from the selected stores.
    pub allowed_scopes: Option<Vec<String>>,
    /// Access levels eligible for retrieval. `None` permits every access level.
    pub allowed_access: Option<Vec<Access>>,
    /// Record kinds eligible for retrieval. `None` permits every kind.
    pub allowed_kinds: Option<Vec<String>>,
    /// Ranking strategy to use.
    pub mode: RetrievalMode,
}

impl Default for SearchOptions {
    fn default() -> Self {
        Self {
            limit: DEFAULT_SEARCH_LIMIT,
            include_inactive: false,
            current_scope: None,
            allowed_scopes: None,
            allowed_access: None,
            allowed_kinds: None,
            mode: RetrievalMode::Hybrid,
        }
    }
}

impl SearchOptions {
    /// Builds the default scope policy for a store.
    ///
    /// Project views include applicable global records. Local and global views stay within their
    /// selected store.
    pub fn for_store(paths: &StorePaths) -> Self {
        let current_scope = crate::record_scope(paths)
            .ok()
            .map(|scope| scope.as_str().to_owned());
        let mut allowed_scopes = Vec::new();
        if let Some(scope) = current_scope.clone() {
            allowed_scopes.push(scope);
        }
        if paths.scope == StoreScope::Project
            && !allowed_scopes.iter().any(|scope| scope == "global")
        {
            allowed_scopes.push("global".to_owned());
        }
        Self {
            current_scope,
            allowed_scopes: Some(allowed_scopes),
            ..Self::default()
        }
    }

    pub(super) fn bounded_limit(&self) -> usize {
        self.limit.clamp(1, MAX_SEARCH_LIMIT)
    }
}

#[derive(Clone, Debug)]
/// Controls evidence selection for a compiled context response.
pub struct ContextOptions {
    /// Maximum approximate token count available for evidence blocks.
    pub budget: usize,
    /// Retrieval filters and ranking policy used to select evidence.
    pub search: SearchOptions,
}

impl Default for ContextOptions {
    fn default() -> Self {
        Self {
            budget: 512,
            search: SearchOptions::default(),
        }
    }
}

#[derive(Clone, Debug, Serialize)]
/// Provenance attached to a retrieved record.
pub struct SourceReceipt {
    pub kind: String,
    pub reference: String,
    pub actor: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub observed_at: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub revision: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub content_hash: Option<String>,
}

#[derive(Clone, Debug, Serialize)]
/// A ranked record chunk returned by search.
pub struct SearchResult {
    pub record_id: String,
    pub chunk_id: String,
    pub title: String,
    pub kind: String,
    pub scope: String,
    pub status: String,
    pub access: String,
    pub excerpt: String,
    pub sources: Vec<SourceReceipt>,
    pub path: String,
    pub score: f64,
    pub lexical_match_reason: String,
    pub match_reasons: Vec<String>,
    pub vector_distance: Option<f64>,
}

#[derive(Clone, Debug, Serialize)]
pub struct ContextBlock {
    pub record_id: String,
    pub chunk_id: String,
    pub title: String,
    pub kind: String,
    pub scope: String,
    pub status: String,
    pub access: String,
    pub sources: Vec<SourceReceipt>,
    /// Record Markdown is quoted evidence, never host instructions.
    pub text_role: String,
    pub text: String,
    pub token_count: usize,
    pub score: f64,
    pub ranking_reasons: Vec<String>,
}

#[derive(Clone, Debug, Serialize)]
pub struct ContextBoundary {
    pub name: String,
    pub description: String,
    pub trusted: bool,
    pub can_grant_tools: bool,
    pub can_change_access: bool,
    pub can_override_host_instructions: bool,
}

#[derive(Clone, Debug, Serialize)]
pub struct ContextContract {
    pub version: String,
    pub boundaries: Vec<ContextBoundary>,
    pub record_text_rule: String,
}

#[derive(Clone, Debug, Serialize)]
pub struct ContextReceipt {
    pub receipt_id: ReceiptId,
    pub retrieved_at: String,
    pub query: String,
    pub current_scope: Option<String>,
    pub scopes: Vec<String>,
    pub statuses: Vec<String>,
    pub access: Vec<String>,
    pub kinds: Vec<String>,
    pub budget: usize,
    pub budget_unit: String,
    pub used_tokens: usize,
    pub truncated: bool,
    pub omitted_results: usize,
    pub index_version: u32,
    pub embedding_model: Option<String>,
    pub embedding_version: Option<String>,
    pub semantic_fallback: Option<SemanticFallbackReason>,
    pub retrieval_mode: String,
    pub contract_version: String,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum SemanticFallbackReason {
    IntentionallyUnavailable,
    ModelUnavailable,
    EmbedderInitializationFailed,
    VectorProjectionUnavailable,
    VectorProjectionBusy,
    EmbeddingExecutionFailed,
}

#[derive(Clone, Debug, Serialize)]
pub struct ContextResult {
    pub contract: ContextContract,
    pub blocks: Vec<ContextBlock>,
    pub receipt: ContextReceipt,
}

/// A canonical Markdown file that could not be projected into the disposable index.
#[derive(Clone, Debug, Serialize)]
pub struct SyncInvalidFile {
    /// Display path of the invalid canonical file.
    pub path: String,
    /// Actionable parse or validation failure.
    pub error: String,
}

/// Result of reconciling canonical Markdown with the disposable index.
///
/// A report can contain successful work and invalid files at the same time. Callers
/// should use [`SyncReport::is_complete`] before treating retrieval as authoritative.
#[derive(Clone, Debug, Default, Serialize)]
pub struct SyncReport {
    pub indexed: usize,
    pub skipped: usize,
    pub removed: usize,
    pub invalid_files: Vec<SyncInvalidFile>,
    pub semantic: Option<SemanticIndexReport>,
}

impl SyncReport {
    /// Returns `true` when every discovered canonical record was indexed successfully.
    pub fn is_complete(&self) -> bool {
        self.invalid_files.is_empty()
    }
}

#[derive(Clone, Debug, Serialize)]
pub struct SemanticIndexReport {
    pub status: String,
    pub model_version: Option<String>,
    pub message: Option<String>,
}

/// Aggregate result from one or more watch reconciliation cycles.
#[derive(Clone, Debug, Serialize)]
pub struct WatchReport {
    pub cycles: usize,
    pub indexed: usize,
    pub skipped: usize,
    pub removed: usize,
    pub invalid_files: Vec<SyncInvalidFile>,
}

impl WatchReport {
    /// Returns `true` when every observed canonical record was indexed successfully.
    pub fn is_complete(&self) -> bool {
        self.invalid_files.is_empty()
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct WatchOptions {
    pub once: bool,
    pub interval: Duration,
}

impl Default for WatchOptions {
    fn default() -> Self {
        Self {
            once: false,
            interval: Duration::from_millis(500),
        }
    }
}

#[derive(Clone, Debug, Serialize)]
pub struct DoctorIssue {
    pub severity: String,
    pub message: String,
    pub repair: String,
}

#[derive(Clone, Debug, Serialize)]
pub struct DoctorReport {
    pub index_path: String,
    pub semantic_model_ready: bool,
    pub failures: usize,
    pub warnings: usize,
    pub issues: Vec<DoctorIssue>,
}

#[derive(Clone, Debug, Serialize)]
pub struct DoctorRepairReport {
    pub diagnosis: DoctorReport,
    pub repaired: Vec<String>,
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct ProjectionStatus {
    pub index_version: Option<u32>,
    pub embedding_version: Option<String>,
    pub last_successful_sync: Option<String>,
}

#[derive(Clone, Debug, Serialize)]
pub struct AdvisoryRelationProjection {
    pub left_record_id: String,
    pub right_record_id: String,
    pub relation: String,
    pub evidence_json: String,
    pub confidence: String,
    pub analyzer_fingerprint: String,
}
