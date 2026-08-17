use std::future::Future;
use std::net::{IpAddr, SocketAddr};
use std::sync::Arc;

use anyhow::{Context, Result as AnyhowResult, bail};
use axum::extract::{Path, Query, State};
use axum::http::{HeaderMap, HeaderValue, StatusCode, header};
use axum::middleware::{self, Next};
use axum::response::{IntoResponse, Response};
use axum::routing::{get, post};
use axum::{Json, Router};
use serde::{Deserialize, Serialize};
use stormbuffer_core::{self as core, RecordRepository, RepositoryError, StoreScope};
use tokio::net::TcpListener;
use utoipa::{IntoParams, OpenApi, ToSchema};

use crate::command::ServeArgs;
use crate::echo::Echo;
use crate::{FAILURE, report_error, resolve};

#[cfg(test)]
const DEFAULT_PORT: u16 = 7342;
const MAX_REQUEST_BYTES: usize = 256 * 1024;

#[derive(Clone)]
struct AppState {
    repository: Arc<RecordRepository>,
    paths: core::StorePaths,
    record_scope: core::Scope,
}

impl AppState {
    fn new(paths: core::StorePaths) -> AnyhowResult<Self> {
        let record_scope = core::record_scope(&paths).context("could not read the store identity")?;
        Ok(Self { repository: Arc::new(RecordRepository::new(paths.clone())), paths, record_scope })
    }
}

pub(super) fn run(scope: StoreScope, arguments: ServeArgs, output: &Echo) -> i32 {
    let address = match serve_address(arguments.bind, arguments.port) {
        Ok(address) => address,
        Err(error) => return report_error(error, output),
    };
    let paths = match resolve(scope) {
        Ok(paths) => paths,
        Err(error) => return report_error(error, output),
    };
    if !paths.root.join("store.toml").is_file() {
        output.error("could not start server: store is not initialized");
        return FAILURE;
    }
    let state = match AppState::new(paths) {
        Ok(state) => state,
        Err(error) => return report_error(error, output),
    };
    let runtime = match tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()
        .context("could not start the local server runtime")
    {
        Ok(runtime) => runtime,
        Err(error) => return report_error(error, output),
    };

    match runtime.block_on(serve(address, state)) {
        Ok(()) => 0,
        Err(error) => report_error(error, output),
    }
}

fn serve_address(bind: IpAddr, port: u16) -> AnyhowResult<SocketAddr> {
    if !bind.is_loopback() {
        bail!("serve only accepts loopback addresses; remote binding requires authentication and a threat model");
    }
    Ok(SocketAddr::new(bind, port))
}

async fn serve(address: SocketAddr, state: AppState) -> AnyhowResult<()> {
    let listener = TcpListener::bind(address)
        .await
        .with_context(|| format!("could not bind local server to {address}"))?;
    let local_address = listener
        .local_addr()
        .context("could not determine the local server address")?;
    tracing::info!(address = %local_address, "Stormbuffer local API listening");

    serve_listener(listener, state, shutdown_signal()).await
}

async fn serve_listener<F>(listener: TcpListener, state: AppState, shutdown: F) -> AnyhowResult<()>
where
    F: Future<Output = ()> + Send + 'static,
{
    axum::serve(listener, router(state))
        .with_graceful_shutdown(shutdown)
        .await
        .context("local server stopped unexpectedly")
}

fn router(state: AppState) -> Router {
    Router::new()
        .route("/openapi.json", get(openapi))
        .route("/v1/records", get(list_records).post(create_record))
        .route("/v1/records/{id}", get(get_record).put(replace_record))
        .route("/v1/records/{id}/approve", post(approve_record))
        .route("/v1/records/{id}/reject", post(reject_record))
        .route("/v1/records/{id}/archive", post(archive_record))
        .route("/v1/records/{id}/restore", post(restore_record))
        .route("/v1/search", get(search_records))
        .layer(axum::extract::DefaultBodyLimit::max(MAX_REQUEST_BYTES))
        .layer(middleware::from_fn(log_request))
        .with_state(state)
}

async fn shutdown_signal() {
    let interrupt = async {
        if let Err(error) = tokio::signal::ctrl_c().await {
            tracing::error!(%error, "could not listen for Ctrl-C");
        }
        tracing::info!("received interrupt signal; shutting down local API");
    };

    #[cfg(unix)]
    {
        let terminate = async {
            match tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate()) {
                Ok(mut signal) => {
                    signal.recv().await;
                    tracing::info!("received termination signal; shutting down local API");
                }
                Err(error) => tracing::error!(%error, "could not listen for SIGTERM"),
            }
        };
        tokio::select! {
            () = interrupt => {},
            () = terminate => {},
        }
    }

    #[cfg(not(unix))]
    interrupt.await;
}

async fn log_request(request: axum::extract::Request, next: Next) -> Response {
    let method = request.method().clone();
    let path = request.uri().path().to_owned();
    let response = next.run(request).await;
    tracing::info!(%method, %path, status = %response.status(), "local API request");
    response
}

#[derive(Debug, Deserialize, IntoParams)]
#[serde(deny_unknown_fields)]
struct ListRecordsQuery {
    /// Include candidate, superseded, and archived records.
    #[serde(default)]
    all: bool,
}

#[derive(Debug, Deserialize, IntoParams)]
#[serde(deny_unknown_fields)]
struct SearchQuery {
    /// Search query.
    query: String,
    /// Maximum number of record chunks to return, from 1 to 100.
    #[serde(default = "default_search_limit")]
    limit: usize,
    /// Include candidate, superseded, and archived records.
    #[serde(default)]
    all: bool,
}

fn default_search_limit() -> usize {
    20
}

#[derive(Debug, Deserialize, ToSchema)]
#[serde(deny_unknown_fields)]
struct RecordInput {
    title: String,
    /// One of fact, decision, procedure, or checkpoint.
    kind: String,
    /// One of human or agent.
    access: String,
    #[serde(default)]
    tags: Vec<String>,
    #[serde(default)]
    aliases: Vec<String>,
    #[serde(default)]
    supersedes: Vec<String>,
    sources: Vec<SourceInput>,
    body: String,
}

#[derive(Debug, Deserialize, ToSchema)]
#[serde(deny_unknown_fields)]
struct SourceInput {
    /// One of conversation, document, issue, or url.
    kind: String,
    reference: String,
    actor: String,
    #[serde(default)]
    observed_at: Option<String>,
    #[serde(default)]
    revision: Option<String>,
    #[serde(default)]
    content_hash: Option<String>,
}

impl RecordInput {
    fn into_record(
        self, id: core::RecordId, scope: core::Scope, created_at: core::Timestamp,
    ) -> Result<core::Record, ApiError> {
        let kind = self
            .kind
            .parse()
            .map_err(|message: String| ApiError::validation("kind", message))?;
        let access = self
            .access
            .parse()
            .map_err(|message: String| ApiError::validation("access", message))?;
        let supersedes = self
            .supersedes
            .into_iter()
            .map(|value| {
                value
                    .parse()
                    .map_err(|message: String| ApiError::validation("supersedes", message))
            })
            .collect::<Result<Vec<_>, _>>()?;
        let sources = self
            .sources
            .into_iter()
            .map(SourceInput::into_source)
            .collect::<Result<Vec<_>, _>>()?;
        Ok(core::Record {
            id,
            title: self.title,
            kind,
            scope,
            status: core::RecordStatus::Active,
            access,
            created_at,
            updated_at: core::Timestamp::now_utc(),
            tags: self.tags,
            aliases: self.aliases,
            supersedes,
            sources,
            body: self.body,
        })
    }
}

impl SourceInput {
    fn into_source(self) -> Result<core::Source, ApiError> {
        let kind = self
            .kind
            .parse()
            .map_err(|message: String| ApiError::validation("source kind", message))?;
        let observed_at = self
            .observed_at
            .map(|value| {
                value
                    .parse()
                    .map_err(|message: String| ApiError::validation("source observed_at", message))
            })
            .transpose()?;
        Ok(core::Source {
            kind,
            reference: self.reference,
            actor: self.actor,
            observed_at,
            revision: self.revision,
            content_hash: self.content_hash,
        })
    }
}

#[derive(Debug, Serialize, ToSchema)]
struct RecordResponse {
    id: String,
    title: String,
    kind: String,
    scope: String,
    status: String,
    access: String,
    created_at: String,
    updated_at: String,
    tags: Vec<String>,
    aliases: Vec<String>,
    supersedes: Vec<String>,
    sources: Vec<SourceResponse>,
    body: String,
}

impl From<&core::Record> for RecordResponse {
    fn from(record: &core::Record) -> Self {
        Self {
            id: record.id.to_string(),
            title: record.title.clone(),
            kind: record.kind.to_string(),
            scope: record.scope.to_string(),
            status: record.status.to_string(),
            access: record.access.to_string(),
            created_at: record.created_at.to_string(),
            updated_at: record.updated_at.to_string(),
            tags: record.tags.clone(),
            aliases: record.aliases.clone(),
            supersedes: record.supersedes.iter().map(ToString::to_string).collect(),
            sources: record.sources.iter().map(SourceResponse::from).collect(),
            body: record.body.clone(),
        }
    }
}

#[derive(Debug, Serialize, ToSchema)]
struct SourceResponse {
    kind: String,
    reference: String,
    actor: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    observed_at: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    revision: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    content_hash: Option<String>,
}

impl From<&core::Source> for SourceResponse {
    fn from(source: &core::Source) -> Self {
        Self {
            kind: source.kind.to_string(),
            reference: source.reference.clone(),
            actor: source.actor.clone(),
            observed_at: source.observed_at.map(|value| value.to_string()),
            revision: source.revision.clone(),
            content_hash: source.content_hash.clone(),
        }
    }
}

#[derive(Debug, Serialize, ToSchema)]
struct SearchResponse {
    record_id: String,
    chunk_id: String,
    title: String,
    kind: String,
    scope: String,
    status: String,
    access: String,
    excerpt: String,
    sources: Vec<SourceResponse>,
    score: f64,
    lexical_match_reason: String,
    match_reasons: Vec<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    vector_distance: Option<f64>,
}

impl From<core::SearchResult> for SearchResponse {
    fn from(result: core::SearchResult) -> Self {
        Self {
            record_id: result.record_id,
            chunk_id: result.chunk_id,
            title: result.title,
            kind: result.kind,
            scope: result.scope,
            status: result.status,
            access: result.access,
            excerpt: result.excerpt,
            sources: result
                .sources
                .into_iter()
                .map(|source| SourceResponse {
                    kind: source.kind,
                    reference: source.reference,
                    actor: source.actor,
                    observed_at: source.observed_at,
                    revision: source.revision,
                    content_hash: source.content_hash,
                })
                .collect(),
            score: result.score,
            lexical_match_reason: result.lexical_match_reason,
            match_reasons: result.match_reasons,
            vector_distance: result.vector_distance,
        }
    }
}

#[derive(Debug, Serialize, ToSchema)]
struct ProposalResponse {
    outcome: String,
    record_id: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    related_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    status: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    message: Option<String>,
}

impl From<core::ProposalResult> for ProposalResponse {
    fn from(result: core::ProposalResult) -> Self {
        Self {
            outcome: result.outcome.to_string(),
            record_id: result.record_id,
            related_id: result.related_id,
            status: result.status,
            message: result.message,
        }
    }
}

#[derive(Debug, Serialize, ToSchema)]
struct ErrorResponse {
    error: ErrorDetail,
}

#[derive(Debug, Serialize, ToSchema)]
struct ErrorDetail {
    code: String,
    message: String,
}

#[derive(Debug)]
struct ApiError {
    status: StatusCode,
    code: &'static str,
    message: String,
    etag: Option<String>,
}

impl ApiError {
    fn validation(field: &'static str, message: String) -> Self {
        Self {
            status: StatusCode::UNPROCESSABLE_ENTITY,
            code: "validation_error",
            message: format!("field `{field}` is invalid: {message}"),
            etag: None,
        }
    }

    fn precondition_required() -> Self {
        Self {
            status: StatusCode::PRECONDITION_REQUIRED,
            code: "precondition_required",
            message: "supply the current ETag in an If-Match header before editing a record".to_owned(),
            etag: None,
        }
    }

    fn revision_conflict(etag: Option<String>) -> Self {
        Self {
            status: StatusCode::PRECONDITION_FAILED,
            code: "revision_conflict",
            message: "record changed while it was being edited; reload it before trying again".to_owned(),
            etag,
        }
    }
}

impl IntoResponse for ApiError {
    fn into_response(self) -> Response {
        let mut response = (
            self.status,
            Json(ErrorResponse { error: ErrorDetail { code: self.code.to_owned(), message: self.message } }),
        )
            .into_response();
        if let Some(etag) = self.etag.and_then(|value| HeaderValue::from_str(&value).ok()) {
            response.headers_mut().insert(header::ETAG, etag);
        }
        response
    }
}

fn core_error(error: core::Error) -> ApiError {
    match error {
        core::Error::Repository { source: RepositoryError::NotFound { .. } } => ApiError {
            status: StatusCode::NOT_FOUND,
            code: "not_found",
            message: "record was not found".to_owned(),
            etag: None,
        },
        core::Error::Repository { source: RepositoryError::ConcurrentModification { .. } } => {
            ApiError::revision_conflict(None)
        }
        core::Error::Repository { source: RepositoryError::MutationBusy { .. } } | core::Error::IndexBusy => ApiError {
            status: StatusCode::CONFLICT,
            code: "store_busy",
            message: "the store is busy; retry the operation".to_owned(),
            etag: None,
        },
        core::Error::InvalidRecord { .. } => ApiError {
            status: StatusCode::UNPROCESSABLE_ENTITY,
            code: "invalid_record",
            message: "a canonical record is invalid".to_owned(),
            etag: None,
        },
        core::Error::Repository { .. } => ApiError {
            status: StatusCode::UNPROCESSABLE_ENTITY,
            code: "invalid_state",
            message: error.to_string(),
            etag: None,
        },
        core::Error::InvalidInput { .. } => ApiError {
            status: StatusCode::UNPROCESSABLE_ENTITY,
            code: "invalid_request",
            message: error.to_string(),
            etag: None,
        },
        error => {
            tracing::error!(%error, "local API core operation failed");
            ApiError {
                status: StatusCode::INTERNAL_SERVER_ERROR,
                code: "internal_error",
                message: "the local API could not complete the operation".to_owned(),
                etag: None,
            }
        }
    }
}

async fn blocking<T, F>(operation: F) -> Result<T, ApiError>
where
    T: Send + 'static,
    F: FnOnce() -> core::Result<T> + Send + 'static,
{
    match tokio::task::spawn_blocking(operation).await {
        Ok(result) => result.map_err(core_error),
        Err(error) => {
            tracing::error!(%error, "local API worker stopped unexpectedly");
            Err(ApiError {
                status: StatusCode::INTERNAL_SERVER_ERROR,
                code: "internal_error",
                message: "the local API could not complete the operation".to_owned(),
                etag: None,
            })
        }
    }
}

fn parse_id(id: &str) -> Result<core::RecordId, ApiError> {
    id.parse()
        .map_err(|message: String| ApiError::validation("id", message))
}

fn etag(markdown: &str) -> String {
    format!("\"{}\"", core::content_hash(markdown))
}

fn if_match(headers: &HeaderMap) -> Result<String, ApiError> {
    headers
        .get(header::IF_MATCH)
        .and_then(|value| value.to_str().ok())
        .filter(|value| !value.is_empty() && *value != "*")
        .map(ToOwned::to_owned)
        .ok_or_else(ApiError::precondition_required)
}

fn record_response(stored: core::StoredRecord) -> Response {
    let etag = etag(stored.markdown());
    let mut response = Json(RecordResponse::from(stored.record())).into_response();
    if let Ok(value) = HeaderValue::from_str(&etag) {
        response.headers_mut().insert(header::ETAG, value);
    }
    response
        .headers_mut()
        .insert(header::CACHE_CONTROL, HeaderValue::from_static("no-store"));
    response
}

#[utoipa::path(
    get,
    path = "/v1/records",
    params(ListRecordsQuery),
    responses((status = 200, description = "Records in the selected canonical store", body = [RecordResponse])),
    tag = "records"
)]
async fn list_records(
    State(state): State<AppState>, Query(query): Query<ListRecordsQuery>,
) -> Result<Json<Vec<RecordResponse>>, ApiError> {
    let repository = state.repository.clone();
    let records = blocking(move || repository.list(query.all)).await?;
    Ok(Json(
        records
            .iter()
            .map(|stored| RecordResponse::from(stored.record()))
            .collect(),
    ))
}

#[utoipa::path(
    post,
    path = "/v1/records",
    request_body = RecordInput,
    responses(
        (status = 201, description = "Human-authored record was added", body = ProposalResponse),
        (status = 409, description = "Record duplicates an existing memory", body = ProposalResponse),
        (status = 422, description = "Invalid record", body = ProposalResponse)
    ),
    tag = "records"
)]
async fn create_record(
    State(state): State<AppState>, Json(input): Json<RecordInput>,
) -> Result<(StatusCode, Json<ProposalResponse>), ApiError> {
    let record = input.into_record(
        core::RecordId::new_v7(),
        state.record_scope.clone(),
        core::Timestamp::now_utc(),
    )?;
    let repository = state.repository.clone();
    let result = blocking(move || repository.propose(record, core::ProposalActor::Human)).await?;
    let status = match result.outcome {
        core::ProposalOutcome::Invalid => StatusCode::UNPROCESSABLE_ENTITY,
        core::ProposalOutcome::DuplicateOf => StatusCode::CONFLICT,
        _ => StatusCode::CREATED,
    };
    Ok((status, Json(ProposalResponse::from(result))))
}

#[utoipa::path(
    get,
    path = "/v1/records/{id}",
    params(("id" = String, Path, description = "Record UUID")),
    responses(
        (status = 200, description = "Canonical record", body = RecordResponse),
        (status = 404, description = "Record was not found", body = ErrorResponse)
    ),
    tag = "records"
)]
async fn get_record(State(state): State<AppState>, Path(raw_id): Path<String>) -> Result<Response, ApiError> {
    let id = parse_id(&raw_id)?;
    let repository = state.repository.clone();
    let stored = blocking(move || repository.find(id)).await?;
    Ok(record_response(stored))
}

#[utoipa::path(
    put,
    path = "/v1/records/{id}",
    params(("id" = String, Path, description = "Active record UUID")),
    request_body = RecordInput,
    responses(
        (status = 200, description = "Updated canonical record", body = RecordResponse),
        (status = 412, description = "The supplied ETag is stale", body = ErrorResponse),
        (status = 428, description = "If-Match header is required", body = ErrorResponse)
    ),
    tag = "records"
)]
async fn replace_record(
    State(state): State<AppState>, Path(raw_id): Path<String>, headers: HeaderMap, Json(input): Json<RecordInput>,
) -> Result<Response, ApiError> {
    let id = parse_id(&raw_id)?;
    let expected_etag = if_match(&headers)?;
    let repository = state.repository.clone();
    let current = blocking(move || repository.find(id)).await?;
    let current_etag = etag(current.markdown());
    if expected_etag != current_etag {
        return Err(ApiError::revision_conflict(Some(current_etag)));
    }
    let replacement = input.into_record(id, current.record().scope.clone(), current.record().created_at)?;
    let repository = state.repository.clone();
    match blocking(move || repository.replace_if_unchanged(&current, replacement)).await {
        Ok(stored) => Ok(record_response(stored)),
        Err(error) if error.code == "revision_conflict" => {
            let repository = state.repository.clone();
            let latest = blocking(move || repository.find(id)).await.ok();
            Err(ApiError::revision_conflict(
                latest.map(|stored| etag(stored.markdown())),
            ))
        }
        Err(error) => Err(error),
    }
}

#[utoipa::path(
    post,
    path = "/v1/records/{id}/approve",
    params(("id" = String, Path, description = "Candidate record UUID")),
    responses((status = 200, description = "Candidate lifecycle result", body = ProposalResponse)),
    tag = "lifecycle"
)]
async fn approve_record(
    State(state): State<AppState>, Path(raw_id): Path<String>,
) -> Result<Json<ProposalResponse>, ApiError> {
    let id = parse_id(&raw_id)?;
    let repository = state.repository.clone();
    let result = blocking(move || repository.approve(id)).await?;
    Ok(Json(ProposalResponse::from(result)))
}

#[utoipa::path(
    post,
    path = "/v1/records/{id}/reject",
    params(("id" = String, Path, description = "Candidate record UUID")),
    responses((status = 200, description = "Candidate lifecycle result", body = ProposalResponse)),
    tag = "lifecycle"
)]
async fn reject_record(
    State(state): State<AppState>, Path(raw_id): Path<String>,
) -> Result<Json<ProposalResponse>, ApiError> {
    let id = parse_id(&raw_id)?;
    let repository = state.repository.clone();
    let result = blocking(move || repository.reject(id)).await?;
    Ok(Json(ProposalResponse::from(result)))
}

#[utoipa::path(
    post,
    path = "/v1/records/{id}/archive",
    params(("id" = String, Path, description = "Active record UUID")),
    responses((status = 200, description = "Archived canonical record", body = RecordResponse)),
    tag = "lifecycle"
)]
async fn archive_record(State(state): State<AppState>, Path(raw_id): Path<String>) -> Result<Response, ApiError> {
    let id = parse_id(&raw_id)?;
    let repository = state.repository.clone();
    let stored = blocking(move || repository.archive(id)).await?;
    Ok(record_response(stored))
}

#[utoipa::path(
    post,
    path = "/v1/records/{id}/restore",
    params(("id" = String, Path, description = "Archived record UUID")),
    responses((status = 200, description = "Restored canonical record", body = RecordResponse)),
    tag = "lifecycle"
)]
async fn restore_record(State(state): State<AppState>, Path(raw_id): Path<String>) -> Result<Response, ApiError> {
    let id = parse_id(&raw_id)?;
    let repository = state.repository.clone();
    let stored = blocking(move || repository.restore(id)).await?;
    Ok(record_response(stored))
}

#[utoipa::path(
    get,
    path = "/v1/search",
    params(SearchQuery),
    responses((status = 200, description = "Lexically ranked record chunks", body = [SearchResponse])),
    tag = "search"
)]
async fn search_records(
    State(state): State<AppState>, Query(query): Query<SearchQuery>,
) -> Result<Json<Vec<SearchResponse>>, ApiError> {
    if query.query.trim().is_empty() {
        return Err(ApiError::validation("query", "must not be empty".to_owned()));
    }
    let paths = state.paths.clone();
    let options = core::SearchOptions {
        limit: query.limit,
        include_inactive: query.all,
        mode: core::RetrievalMode::Lexical,
        ..core::SearchOptions::for_store(&state.paths)
    };
    let stores = blocking(move || {
        let cwd = std::env::current_dir().map_err(|_| core::Error::InvalidWorkingDirectory)?;
        let stores = core::retrieval_stores(&paths, &cwd)?;
        for store in &stores {
            let report = core::sync_store(store)?;
            if !report.is_complete() {
                return Err(core::Error::InvalidInput {
                    message: "one or more canonical records are invalid".to_owned(),
                });
            }
        }
        core::search_stores(&stores, &query.query, options)
    })
    .await?;
    Ok(Json(stores.into_iter().map(SearchResponse::from).collect()))
}

#[derive(OpenApi)]
#[openapi(
    paths(
        list_records,
        create_record,
        get_record,
        replace_record,
        approve_record,
        reject_record,
        archive_record,
        restore_record,
        search_records
    ),
    components(schemas(
        RecordInput,
        SourceInput,
        RecordResponse,
        SourceResponse,
        SearchResponse,
        ProposalResponse,
        ErrorResponse,
        ErrorDetail
    )),
    tags(
        (name = "records", description = "Browse, create, and edit canonical records."),
        (name = "lifecycle", description = "Approve, reject, archive, and restore records."),
        (name = "search", description = "Search the selected store view.")
    ),
    info(title = "Stormbuffer local API", version = "1.0.0")
)]
struct ApiDoc;

async fn openapi() -> Json<utoipa::openapi::OpenApi> {
    Json(ApiDoc::openapi())
}

#[cfg(test)]
mod tests {
    use std::fs;
    use std::time::{Duration, SystemTime, UNIX_EPOCH};

    use tokio::io::{AsyncReadExt, AsyncWriteExt};
    use tokio::net::TcpStream;
    use tokio::sync::oneshot;

    use super::*;

    fn fixture_state(name: &str) -> (AppState, core::StorePaths) {
        let suffix = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system clock")
            .as_nanos();
        let root = std::env::temp_dir().join(format!("stormbuffer-server-{name}-{suffix}"));
        let paths = core::StorePaths {
            scope: StoreScope::Global,
            records: root.join("records"),
            cache: root.join("cache"),
            root,
        };
        core::initialize_store(&paths, core::StoreInitMode::Default).expect("initialize store");
        let state = AppState::new(paths.clone()).expect("create server state");
        (state, paths)
    }

    fn record(paths: &core::StorePaths) -> core::StoredRecord {
        let now = core::Timestamp::now_utc();
        let record = core::Record {
            id: core::RecordId::new_v7(),
            title: "Server fixture".to_owned(),
            kind: core::RecordKind::Fact,
            scope: core::record_scope(paths).expect("fixture scope"),
            status: core::RecordStatus::Active,
            access: core::Access::Human,
            created_at: now,
            updated_at: now,
            tags: Vec::new(),
            aliases: Vec::new(),
            supersedes: Vec::new(),
            sources: vec![core::Source {
                kind: core::SourceKind::Document,
                reference: "server-test".to_owned(),
                actor: "test".to_owned(),
                observed_at: None,
                revision: None,
                content_hash: None,
            }],
            body: "A local server fixture.".to_owned(),
        };
        core::RecordRepository::new(paths.clone())
            .add(record)
            .expect("add fixture record")
    }

    async fn request(address: SocketAddr, request: &str) -> String {
        let mut stream = TcpStream::connect(address).await.expect("connect to API");
        stream.write_all(request.as_bytes()).await.expect("send API request");
        let mut response = String::new();
        stream.read_to_string(&mut response).await.expect("read API response");
        response
    }

    #[test]
    fn only_loopback_addresses_are_accepted() {
        assert!(serve_address("127.0.0.1".parse().unwrap(), DEFAULT_PORT).is_ok());
        assert!(serve_address("::1".parse().unwrap(), DEFAULT_PORT).is_ok());
        assert!(serve_address("0.0.0.0".parse().unwrap(), DEFAULT_PORT).is_err());
        assert!(serve_address("192.0.2.1".parse().unwrap(), DEFAULT_PORT).is_err());
    }

    #[test]
    fn openapi_describes_the_local_contract() {
        let document = ApiDoc::openapi();
        assert!(document.paths.paths.contains_key("/v1/records"));
        assert!(document.paths.paths.contains_key("/v1/search"));
    }

    #[tokio::test]
    async fn service_manager_shutdown_stops_the_listener_after_serving_requests() {
        let (state, paths) = fixture_state("graceful-shutdown");
        let listener = TcpListener::bind("127.0.0.1:0").await.expect("bind test listener");
        let address = listener.local_addr().expect("test listener address");
        let (shutdown, shutdown_signal) = oneshot::channel();
        let task = tokio::spawn(serve_listener(listener, state, async move {
            let _ = shutdown_signal.await;
        }));

        let response = request(
            address,
            "GET /v1/records HTTP/1.1\r\nHost: localhost\r\nConnection: close\r\n\r\n",
        )
        .await;
        assert!(response.starts_with("HTTP/1.1 200"), "{response}");
        assert!(response.ends_with("[]"), "{response}");

        shutdown.send(()).expect("request server shutdown");
        let result = tokio::time::timeout(Duration::from_secs(2), task)
            .await
            .expect("server should stop after shutdown signal")
            .expect("server task should not panic");
        result.expect("server should stop cleanly");
        fs::remove_dir_all(paths.root).expect("remove fixture store");
    }

    #[tokio::test]
    async fn validation_rejects_invalid_records_before_writing() {
        let (state, paths) = fixture_state("validation");
        let error = create_record(
            State(state.clone()),
            Json(RecordInput {
                title: "Invalid server record".to_owned(),
                kind: "invalid".to_owned(),
                access: "human".to_owned(),
                tags: Vec::new(),
                aliases: Vec::new(),
                supersedes: Vec::new(),
                sources: vec![SourceInput {
                    kind: "document".to_owned(),
                    reference: "server-test".to_owned(),
                    actor: "test".to_owned(),
                    observed_at: None,
                    revision: None,
                    content_hash: None,
                }],
                body: "This must not be written.".to_owned(),
            }),
        )
        .await
        .expect_err("invalid record should fail");

        assert_eq!(error.status, StatusCode::UNPROCESSABLE_ENTITY);
        assert_eq!(error.code, "validation_error");

        let (status, _) = create_record(
            State(state.clone()),
            Json(RecordInput {
                title: " ".to_owned(),
                kind: "fact".to_owned(),
                access: "human".to_owned(),
                tags: Vec::new(),
                aliases: Vec::new(),
                supersedes: Vec::new(),
                sources: vec![SourceInput {
                    kind: "document".to_owned(),
                    reference: "server-test".to_owned(),
                    actor: "test".to_owned(),
                    observed_at: None,
                    revision: None,
                    content_hash: None,
                }],
                body: "This must also not be written.".to_owned(),
            }),
        )
        .await
        .expect("core validation should produce a structured response");
        assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY);
        assert!(state.repository.list(true).expect("list fixture records").is_empty());
        fs::remove_dir_all(paths.root).expect("remove fixture store");
    }

    #[tokio::test]
    async fn lifecycle_endpoints_match_core_transitions() {
        let (state, paths) = fixture_state("lifecycle");
        let active = record(&paths);
        let active_id = active.record().id;
        let repository = core::RecordRepository::new(paths.clone());

        archive_record(State(state.clone()), Path(active_id.to_string()))
            .await
            .expect("archive active record");
        assert_eq!(
            repository
                .find(active_id)
                .expect("read archived record")
                .record()
                .status,
            core::RecordStatus::Archived
        );
        restore_record(State(state.clone()), Path(active_id.to_string()))
            .await
            .expect("restore archived record");
        assert_eq!(
            repository
                .find(active_id)
                .expect("read restored record")
                .record()
                .status,
            core::RecordStatus::Active
        );

        let mut candidate = repository.find(active_id).expect("read active record").record().clone();
        candidate.id = core::RecordId::new_v7();
        candidate.title = "Candidate server fixture".to_owned();
        candidate.body = "A candidate needs review.".to_owned();
        candidate.created_at = core::Timestamp::now_utc();
        candidate.updated_at = candidate.created_at;
        candidate.status = core::RecordStatus::Active;
        let candidate_id = candidate.id;
        repository
            .propose(candidate, core::ProposalActor::Agent)
            .expect("create candidate");
        let _ = approve_record(State(state.clone()), Path(candidate_id.to_string()))
            .await
            .expect("approve candidate");
        assert_eq!(
            repository
                .find(candidate_id)
                .expect("read approved candidate")
                .record()
                .status,
            core::RecordStatus::Active
        );

        let mut rejected = repository
            .find(candidate_id)
            .expect("read approved candidate")
            .record()
            .clone();
        rejected.id = core::RecordId::new_v7();
        rejected.title = "Rejected server fixture".to_owned();
        rejected.body = "A second candidate needs review.".to_owned();
        rejected.created_at = core::Timestamp::now_utc();
        rejected.updated_at = rejected.created_at;
        let rejected_id = rejected.id;
        repository
            .propose(rejected, core::ProposalActor::Agent)
            .expect("create second candidate");
        let _ = reject_record(State(state.clone()), Path(rejected_id.to_string()))
            .await
            .expect("reject candidate");
        assert_eq!(
            repository
                .find(rejected_id)
                .expect("read rejected candidate")
                .record()
                .status,
            core::RecordStatus::Archived
        );
        fs::remove_dir_all(paths.root).expect("remove fixture store");
    }

    #[tokio::test]
    async fn stale_edit_reports_the_current_etag_without_overwriting() {
        let (state, paths) = fixture_state("stale-edit");
        let stored = record(&paths);
        let id = stored.record().id;
        let stale_etag = etag(stored.markdown());

        let repository = core::RecordRepository::new(paths.clone());
        let current = repository.find(id).expect("read fixture record");
        let mut external = current.record().clone();
        external.body = "Changed outside the server.".to_owned();
        external.updated_at = core::Timestamp::now_utc();
        repository
            .replace_if_unchanged(&current, external)
            .expect("external update");

        let response = replace_record(
            State(state),
            Path(id.to_string()),
            HeaderMap::from_iter([(header::IF_MATCH, HeaderValue::from_str(&stale_etag).unwrap())]),
            Json(RecordInput {
                title: "Server fixture".to_owned(),
                kind: "fact".to_owned(),
                access: "human".to_owned(),
                tags: Vec::new(),
                aliases: Vec::new(),
                supersedes: Vec::new(),
                sources: vec![SourceInput {
                    kind: "document".to_owned(),
                    reference: "server-test".to_owned(),
                    actor: "test".to_owned(),
                    observed_at: None,
                    revision: None,
                    content_hash: None,
                }],
                body: "Stale API update.".to_owned(),
            }),
        )
        .await
        .expect_err("stale update should fail")
        .into_response();

        assert_eq!(response.status(), StatusCode::PRECONDITION_FAILED);
        assert!(response.headers().contains_key(header::ETAG));
        assert_eq!(
            repository.find(id).expect("read external update").record().body,
            "Changed outside the server."
        );
        fs::remove_dir_all(paths.root).expect("remove fixture store");
    }
}
