use std::collections::{HashMap, HashSet};
use std::fs::{self, File};
use std::io;
use std::path::{Path, PathBuf};
use std::thread;
use std::time::Duration;

use rusqlite::types::Value;
use rusqlite::{Connection, OptionalExtension, Transaction, params, params_from_iter};
use serde::Serialize;

use crate::record::Access;
use crate::repository::{acquire_store_mutation_lock, replace_file};
use crate::{Error, Record, StorePaths, StoreScope};

pub const INDEX_SCHEMA_VERSION: u32 = 2;
const MAX_CHUNK_WORDS: usize = 160;
const DEFAULT_SEARCH_LIMIT: usize = 20;
const MAX_SEARCH_LIMIT: usize = 100;

#[derive(Clone, Debug)]
pub struct SearchOptions {
    pub limit: usize,
    pub include_inactive: bool,
    pub current_scope: Option<String>,
    pub allowed_scopes: Option<Vec<String>>,
    pub allowed_access: Option<Vec<Access>>,
}

impl Default for SearchOptions {
    fn default() -> Self {
        Self {
            limit: DEFAULT_SEARCH_LIMIT,
            include_inactive: false,
            current_scope: None,
            allowed_scopes: None,
            allowed_access: None,
        }
    }
}

impl SearchOptions {
    pub fn for_store(paths: &StorePaths) -> Self {
        let current_scope = current_scope(paths);
        let mut allowed_scopes = Vec::new();
        if let Some(scope) = current_scope.clone() {
            allowed_scopes.push(scope);
        }
        if !allowed_scopes.iter().any(|scope| scope == "global") {
            allowed_scopes.push("global".to_owned());
        }
        Self {
            current_scope,
            allowed_scopes: Some(allowed_scopes),
            ..Self::default()
        }
    }

    fn bounded_limit(&self) -> usize {
        self.limit.clamp(1, MAX_SEARCH_LIMIT)
    }
}

#[derive(Clone, Debug)]
pub struct ContextOptions {
    pub budget: usize,
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
pub struct SourceReceipt {
    pub kind: String,
    pub reference: String,
    pub actor: String,
}

#[derive(Clone, Debug, Serialize)]
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
    pub text: String,
    pub token_count: usize,
    pub score: f64,
    pub ranking_reasons: Vec<String>,
}

#[derive(Clone, Debug, Serialize)]
pub struct ContextReceipt {
    pub query: String,
    pub scopes: Vec<String>,
    pub statuses: Vec<String>,
    pub access: Vec<String>,
    pub budget: usize,
    pub used_tokens: usize,
    pub truncated: bool,
    pub omitted_results: usize,
    pub index_version: u32,
    pub embedding_version: Option<String>,
}

#[derive(Clone, Debug, Serialize)]
pub struct ContextResult {
    pub blocks: Vec<ContextBlock>,
    pub receipt: ContextReceipt,
}

#[derive(Clone, Debug, Serialize)]
pub struct SyncInvalidFile {
    pub path: String,
    pub error: String,
}

#[derive(Clone, Debug, Default, Serialize)]
pub struct SyncReport {
    pub indexed: usize,
    pub skipped: usize,
    pub removed: usize,
    pub invalid_files: Vec<SyncInvalidFile>,
}

#[derive(Clone, Debug, Serialize)]
pub struct WatchReport {
    pub cycles: usize,
    pub indexed: usize,
    pub skipped: usize,
    pub removed: usize,
    pub invalid_files: Vec<SyncInvalidFile>,
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
    pub failures: usize,
    pub warnings: usize,
    pub issues: Vec<DoctorIssue>,
}

#[derive(Clone, Debug)]
struct SearchHit {
    record_id: String,
    chunk_id: String,
    title: String,
    kind: String,
    scope: String,
    status: String,
    access: String,
    text: String,
    sources: Vec<SourceReceipt>,
    path: String,
    score: f64,
    lexical_match_reason: String,
}

#[derive(Clone, Debug)]
struct ProjectedRecord {
    record_id: String,
    path: String,
    content_hash: String,
}

pub fn index_path(paths: &StorePaths) -> PathBuf {
    match paths.scope {
        StoreScope::Global => paths.cache.join("global.sqlite3"),
        StoreScope::Project => paths.root.join("index.sqlite3"),
    }
}

pub fn content_hash(markdown: &str) -> String {
    blake3::hash(markdown.as_bytes()).to_hex().to_string()
}

fn flush_section(
    sections: &mut Vec<(String, Option<String>, String, bool)>,
    current_lines: &mut Vec<&str>,
    heading_stack: &[(usize, String)],
    current_atomic: &mut bool,
) {
    if current_lines.is_empty() {
        return;
    }
    let heading = heading_stack
        .iter()
        .map(|(_, value)| value.as_str())
        .collect::<Vec<_>>()
        .join(" / ");
    sections.push((
        heading.clone(),
        (!heading.is_empty()).then_some(heading),
        current_lines.join("\n"),
        *current_atomic,
    ));
    current_lines.clear();
    *current_atomic = false;
}

pub fn chunk_record(record: &Record) -> Vec<(String, Option<String>, String, usize)> {
    let mut sections = Vec::new();
    let mut heading_stack: Vec<(usize, String)> = Vec::new();
    let mut current_lines: Vec<&str> = Vec::new();
    let mut current_atomic = false;
    let mut in_fence: Option<char> = None;

    for line in record.body.lines() {
        let trimmed = line.trim_start();
        if let Some(fence) = in_fence {
            current_lines.push(line);
            if is_fence_end(trimmed, fence) {
                in_fence = None;
            }
            continue;
        }

        if let Some(fence) = fence_start(trimmed) {
            flush_section(
                &mut sections,
                &mut current_lines,
                &heading_stack,
                &mut current_atomic,
            );
            current_lines.push(line);
            current_atomic = true;
            in_fence = Some(fence);
            continue;
        }

        if let Some((level, heading)) = parse_heading(trimmed) {
            flush_section(
                &mut sections,
                &mut current_lines,
                &heading_stack,
                &mut current_atomic,
            );
            while heading_stack
                .last()
                .is_some_and(|(previous, _)| *previous >= level)
            {
                heading_stack.pop();
            }
            heading_stack.push((level, heading));
            continue;
        }

        if line.trim().is_empty() {
            flush_section(
                &mut sections,
                &mut current_lines,
                &heading_stack,
                &mut current_atomic,
            );
            continue;
        }

        if is_list_start(trimmed) && !current_atomic {
            flush_section(
                &mut sections,
                &mut current_lines,
                &heading_stack,
                &mut current_atomic,
            );
            current_atomic = true;
        }
        current_lines.push(line);
    }
    flush_section(
        &mut sections,
        &mut current_lines,
        &heading_stack,
        &mut current_atomic,
    );

    if sections.is_empty() {
        let heading = heading_stack
            .iter()
            .map(|(_, value)| value.as_str())
            .collect::<Vec<_>>()
            .join(" / ");
        sections.push((
            heading.clone(),
            (!heading.is_empty()).then_some(heading),
            String::new(),
            false,
        ));
    }

    let mut chunks = Vec::new();
    let mut current_heading: Option<String> = None;
    let mut current_text = String::new();
    let mut current_words = 0;

    let push_chunk = |chunks: &mut Vec<(String, Option<String>, String, usize)>,
                      heading: &mut Option<String>,
                      text: &mut String,
                      words: &mut usize| {
        if text.trim().is_empty() {
            return;
        }
        let ordinal = chunks.len();
        chunks.push((
            format!("{}:{ordinal}", record.id),
            heading.clone(),
            text.clone(),
            *words,
        ));
        text.clear();
        *words = 0;
    };

    for (_, heading, text, atomic) in sections {
        let word_count = text.split_whitespace().count();
        if atomic {
            push_chunk(
                &mut chunks,
                &mut current_heading,
                &mut current_text,
                &mut current_words,
            );
            current_heading = heading.clone();
            current_text = text;
            current_words = word_count;
            push_chunk(
                &mut chunks,
                &mut current_heading,
                &mut current_text,
                &mut current_words,
            );
            continue;
        }

        if word_count > MAX_CHUNK_WORDS {
            push_chunk(
                &mut chunks,
                &mut current_heading,
                &mut current_text,
                &mut current_words,
            );
            let words: Vec<_> = text.split_whitespace().collect();
            for piece in words.chunks(MAX_CHUNK_WORDS) {
                let piece_text = piece.join(" ");
                chunks.push((
                    format!("{}:{}", record.id, chunks.len()),
                    heading.clone(),
                    piece_text,
                    piece.len(),
                ));
            }
            continue;
        }

        let same_heading = current_heading == heading;
        let separator_words = usize::from(!current_text.is_empty());
        if !same_heading || current_words + separator_words + word_count > MAX_CHUNK_WORDS {
            push_chunk(
                &mut chunks,
                &mut current_heading,
                &mut current_text,
                &mut current_words,
            );
            current_heading = heading.clone();
        }
        if !current_text.is_empty() {
            current_text.push_str("\n\n");
            current_words += 1;
        }
        current_text.push_str(&text);
        current_words += word_count;
    }
    push_chunk(
        &mut chunks,
        &mut current_heading,
        &mut current_text,
        &mut current_words,
    );

    chunks
}

pub fn sync_store(paths: &StorePaths) -> crate::Result<SyncReport> {
    let _lock = acquire_store_mutation_lock(paths)?;
    let mut index = Index::open_at(&index_path(paths))?;
    index.sync_canonical(paths)
}

pub fn reindex_store(paths: &StorePaths) -> crate::Result<SyncReport> {
    let _lock = acquire_store_mutation_lock(paths)?;
    let destination = index_path(paths);
    let parent = destination.parent().ok_or_else(|| {
        Error::io(
            "resolve the index directory",
            io::Error::other("index path has no parent"),
        )
    })?;
    fs::create_dir_all(parent).map_err(|source| Error::io("create the index directory", source))?;
    let temporary = parent.join(format!(
        ".{}.{}.{}.tmp",
        destination
            .file_name()
            .and_then(|name| name.to_str())
            .unwrap_or("index.sqlite3"),
        std::process::id(),
        std::thread::current().name().unwrap_or("reindex")
    ));
    let _ = fs::remove_file(&temporary);

    let result = (|| {
        let mut fresh = Index::open_at(&temporary)?;
        let report = fresh.sync_canonical(paths)?;
        fresh.checkpoint()?;
        drop(fresh);

        replace_file(&temporary, &destination)
            .map_err(|source| Error::io("switch the rebuilt index", source))?;
        remove_sqlite_sidecars(&destination);
        sync_parent_directory(&destination)
            .map_err(|source| Error::io("sync the index directory", source))?;
        Ok(report)
    })();
    let _ = fs::remove_file(&temporary);
    result
}

pub fn search_store(
    paths: &StorePaths,
    query: &str,
    options: SearchOptions,
) -> crate::Result<Vec<SearchResult>> {
    search_stores(std::slice::from_ref(paths), query, options)
}

pub fn search_stores(
    stores: &[StorePaths],
    query: &str,
    options: SearchOptions,
) -> crate::Result<Vec<SearchResult>> {
    let mut hits = Vec::new();
    for paths in stores {
        let index = Index::open_at(&index_path(paths))?;
        hits.extend(index.search_hits(paths, query, &options)?);
    }
    let current = options
        .current_scope
        .clone()
        .or_else(|| stores.first().and_then(current_scope));
    sort_hits(&mut hits, current.as_deref());
    hits.truncate(options.bounded_limit());
    Ok(hits.into_iter().map(SearchResult::from).collect())
}

pub fn context_store(
    paths: &StorePaths,
    query: &str,
    options: ContextOptions,
) -> crate::Result<ContextResult> {
    context_stores(std::slice::from_ref(paths), query, options)
}

pub fn context_stores(
    stores: &[StorePaths],
    query: &str,
    mut options: ContextOptions,
) -> crate::Result<ContextResult> {
    if options.search.allowed_scopes.is_none() && options.search.current_scope.is_none() {
        if let Some(paths) = stores.first() {
            options.search = SearchOptions::for_store(paths);
        }
    }
    let mut hits = Vec::new();
    for paths in stores {
        let index = Index::open_at(&index_path(paths))?;
        hits.extend(index.search_hits(paths, query, &options.search)?);
    }
    sort_hits(&mut hits, options.search.current_scope.as_deref());
    hits.truncate(options.search.bounded_limit());
    let budget = options.budget;
    let mut used_tokens = 0;
    let mut truncated = false;
    let mut blocks = Vec::new();

    for hit in &hits {
        if used_tokens >= budget {
            break;
        }
        let words: Vec<_> = hit.text.split_whitespace().collect();
        let remaining = budget - used_tokens;
        let selected = if words.len() > remaining {
            truncated = true;
            words[..remaining].join(" ")
        } else {
            hit.text.clone()
        };
        let token_count = selected.split_whitespace().count();
        if token_count == 0 {
            continue;
        }
        used_tokens += token_count;
        blocks.push(ContextBlock {
            record_id: hit.record_id.clone(),
            chunk_id: hit.chunk_id.clone(),
            title: hit.title.clone(),
            kind: hit.kind.clone(),
            scope: hit.scope.clone(),
            status: hit.status.clone(),
            access: hit.access.clone(),
            sources: hit.sources.clone(),
            text: selected,
            token_count,
            score: hit.score,
            ranking_reasons: vec![hit.lexical_match_reason.clone()],
        });
    }

    let scopes = options.search.allowed_scopes.clone().unwrap_or_default();
    let access = options
        .search
        .allowed_access
        .as_ref()
        .map(|values| values.iter().map(ToString::to_string).collect())
        .unwrap_or_else(|| vec!["human".to_owned(), "agent".to_owned()]);
    let omitted_results = hits.len().saturating_sub(blocks.len());
    Ok(ContextResult {
        blocks,
        receipt: ContextReceipt {
            query: query.to_owned(),
            scopes,
            statuses: if options.search.include_inactive {
                vec!["candidate", "active", "superseded", "archived"]
                    .into_iter()
                    .map(str::to_owned)
                    .collect()
            } else {
                vec!["active".to_owned()]
            },
            access,
            budget,
            used_tokens,
            truncated,
            omitted_results,
            index_version: INDEX_SCHEMA_VERSION,
            embedding_version: None,
        },
    })
}

pub fn watch_store(paths: &StorePaths, options: WatchOptions) -> crate::Result<WatchReport> {
    let mut aggregate = WatchReport {
        cycles: 0,
        indexed: 0,
        skipped: 0,
        removed: 0,
        invalid_files: Vec::new(),
    };
    loop {
        let report = sync_store(paths)?;
        aggregate.cycles += 1;
        aggregate.indexed += report.indexed;
        aggregate.skipped += report.skipped;
        aggregate.removed += report.removed;
        aggregate.invalid_files.extend(report.invalid_files);
        if options.once {
            return Ok(aggregate);
        }
        thread::sleep(options.interval);
    }
}

pub fn doctor_store(paths: &StorePaths) -> crate::Result<DoctorReport> {
    let destination = index_path(paths);
    let mut report = DoctorReport {
        index_path: destination.display().to_string(),
        failures: 0,
        warnings: 0,
        issues: Vec::new(),
    };
    let issue = |report: &mut DoctorReport, severity: &str, message: String, repair: &str| {
        if severity == "failure" {
            report.failures += 1;
        } else {
            report.warnings += 1;
        }
        report.issues.push(DoctorIssue {
            severity: severity.to_owned(),
            message,
            repair: repair.to_owned(),
        });
    };

    if !paths.root.join("store.toml").is_file() {
        issue(
            &mut report,
            "failure",
            "the selected store is not initialized".to_owned(),
            "run `stormbuffer init` (or `stormbuffer --project init`)",
        );
        return Ok(report);
    }
    if !paths.records.is_dir() {
        issue(
            &mut report,
            "failure",
            "the canonical records directory is missing".to_owned(),
            "restore the records directory or initialize the store again",
        );
        return Ok(report);
    }

    let canonical = collect_canonical_records(&paths.records);
    let mut valid = HashMap::new();
    for path in canonical {
        match read_canonical(&path) {
            Ok((record, markdown)) => {
                valid.insert(
                    path.display().to_string(),
                    (record.id.to_string(), content_hash(&markdown)),
                );
            }
            Err(error) => issue(
                &mut report,
                "failure",
                format!("canonical record {} is invalid: {error}", path.display()),
                "repair the Markdown, then run `stormbuffer sync`",
            ),
        }
    }

    if !destination.is_file() {
        issue(
            &mut report,
            "warning",
            "the SQLite projection is missing".to_owned(),
            "run `stormbuffer reindex`",
        );
    } else {
        match Index::open_at(&destination).and_then(|index| index.projected_records()) {
            Ok(projected) => {
                let mut projected_paths = HashSet::new();
                for record in projected {
                    projected_paths.insert(record.path.clone());
                    match valid.get(&record.path) {
                        Some((id, hash))
                            if id == &record.record_id && hash == &record.content_hash => {}
                        Some(_) => issue(
                            &mut report,
                            "warning",
                            format!("projection is stale for {}", record.path),
                            "run `stormbuffer sync`",
                        ),
                        None => issue(
                            &mut report,
                            "warning",
                            format!("projection contains deleted record {}", record.path),
                            "run `stormbuffer sync`",
                        ),
                    }
                }
                for path in valid.keys() {
                    if !projected_paths.contains(path) {
                        issue(
                            &mut report,
                            "warning",
                            format!("canonical record is not indexed: {path}"),
                            "run `stormbuffer sync`",
                        );
                    }
                }
            }
            Err(error) => issue(
                &mut report,
                "failure",
                format!("the SQLite projection cannot be opened: {error}"),
                "run `stormbuffer reindex`",
            ),
        }
    }

    issue(
        &mut report,
        "warning",
        "semantic model is not configured; lexical search is available".to_owned(),
        "no repair is needed for lexical search; configure semantic retrieval when available",
    );
    Ok(report)
}

struct Index {
    connection: Connection,
}

impl Index {
    fn open_at(path: &Path) -> crate::Result<Self> {
        let parent = path.parent().ok_or_else(|| {
            Error::io(
                "resolve the index directory",
                io::Error::other("index path has no parent"),
            )
        })?;
        fs::create_dir_all(parent)
            .map_err(|source| Error::io("create the index directory", source))?;
        let connection =
            Connection::open(path).map_err(|source| db_error("open the index", source))?;
        connection
            .execute_batch("PRAGMA foreign_keys = ON; PRAGMA journal_mode = WAL; PRAGMA synchronous = NORMAL; PRAGMA busy_timeout = 5000;")
            .map_err(|source| db_error("configure the index", source))?;
        migrate(&connection)?;
        Ok(Self { connection })
    }

    fn checkpoint(&mut self) -> crate::Result<()> {
        self.connection
            .execute_batch("PRAGMA wal_checkpoint(TRUNCATE);")
            .map_err(|source| db_error("checkpoint the index", source))
    }

    fn sync_canonical(&mut self, paths: &StorePaths) -> crate::Result<SyncReport> {
        let files = collect_markdown_paths(&paths.records)?;
        let projected = self.projected_records()?;
        let by_path: HashMap<_, _> = projected
            .iter()
            .map(|record| (record.path.clone(), record.clone()))
            .collect();
        let mut seen_paths = HashSet::new();
        let mut seen_ids = HashMap::new();
        let mut report = SyncReport::default();

        for path in files {
            let path_string = path.display().to_string();
            seen_paths.insert(path_string.clone());
            let (record, markdown) = match read_canonical(&path) {
                Ok(value) => value,
                Err(error) => {
                    report.invalid_files.push(SyncInvalidFile {
                        path: path_string.clone(),
                        error: error.to_string(),
                    });
                    if self.delete_projection_by_path(&path_string)? {
                        report.removed += 1;
                    }
                    continue;
                }
            };
            if let Some(first) = seen_ids.insert(record.id, path.clone()) {
                report.invalid_files.push(SyncInvalidFile {
                    path: path_string.clone(),
                    error: format!("duplicate record id; first seen at {}", first.display()),
                });
                if self.delete_projection_by_path(&path_string)? {
                    report.removed += 1;
                }
                continue;
            }
            let hash = content_hash(&markdown);
            if by_path.get(&path_string).is_some_and(|entry| {
                entry.content_hash == hash && entry.record_id == record.id.to_string()
            }) {
                report.skipped += 1;
                continue;
            }
            self.project_record(&record, &path_string, &hash)?;
            report.indexed += 1;
        }

        for record in projected {
            if !seen_paths.contains(&record.path) && self.delete_projection_by_path(&record.path)? {
                report.removed += 1;
            }
        }
        self.connection
            .execute(
                "INSERT INTO index_metadata(key, value) VALUES ('last_sync', strftime('%Y-%m-%dT%H:%M:%fZ', 'now')) ON CONFLICT(key) DO UPDATE SET value=excluded.value",
                [],
            )
            .map_err(|source| db_error("record the sync time", source))?;
        Ok(report)
    }

    fn project_record(&mut self, record: &Record, path: &str, hash: &str) -> crate::Result<()> {
        let transaction = self
            .connection
            .transaction()
            .map_err(|source| db_error("begin record projection", source))?;
        delete_projection_tx(&transaction, &record.id.to_string())?;
        transaction
            .execute(
                "INSERT INTO scopes(name) VALUES (?1) ON CONFLICT(name) DO NOTHING",
                params![record.scope.as_str()],
            )
            .map_err(|source| db_error("project the record scope", source))?;
        let scope_id: i64 = transaction
            .query_row(
                "SELECT scope_id FROM scopes WHERE name = ?1",
                params![record.scope.as_str()],
                |row| row.get(0),
            )
            .map_err(|source| db_error("read the record scope", source))?;
        let aliases =
            serde_json::to_string(&record.aliases).map_err(|source| Error::InvalidInput {
                message: source.to_string(),
            })?;
        let tags = serde_json::to_string(&record.tags).map_err(|source| Error::InvalidInput {
            message: source.to_string(),
        })?;
        transaction
            .execute(
                "INSERT INTO records(record_id, scope_id, path, title, kind, status, access, created_at, updated_at, aliases_json, tags_json, content_hash) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12)",
                params![
                    record.id.to_string(),
                    scope_id,
                    path,
                    record.title,
                    record.kind.to_string(),
                    record.status.to_string(),
                    record.access.to_string(),
                    record.created_at.to_string(),
                    record.updated_at.to_string(),
                    aliases,
                    tags,
                    hash,
                ],
            )
            .map_err(|source| db_error("project the record metadata", source))?;

        for source in &record.sources {
            transaction
                .execute(
                    "INSERT INTO sources(record_id, kind, reference, actor) VALUES (?1, ?2, ?3, ?4)",
                    params![
                        record.id.to_string(),
                        source.kind.to_string(),
                        source.reference,
                        source.actor,
                    ],
                )
                .map_err(|source| db_error("project the record source", source))?;
        }

        for (ordinal, (chunk_id, heading, text, token_count)) in
            chunk_record(record).into_iter().enumerate()
        {
            let heading_text = heading.clone().unwrap_or_default();
            let filename = Path::new(path)
                .file_name()
                .and_then(|name| name.to_str())
                .unwrap_or(path);
            let retrieval_text = [
                record.title.as_str(),
                heading_text.as_str(),
                record.aliases.join(" ").as_str(),
                record.tags.join(" ").as_str(),
                filename,
                text.as_str(),
            ]
            .join("\n");
            transaction
                .execute(
                    "INSERT INTO chunks(record_id, chunk_id, ordinal, heading, text, retrieval_text, token_count) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
                    params![
                        record.id.to_string(),
                        chunk_id,
                        ordinal as i64,
                        heading,
                        text,
                        retrieval_text,
                        token_count as i64,
                    ],
                )
                .map_err(|source| db_error("project the record chunk", source))?;
            let rowid = transaction.last_insert_rowid();
            transaction
                .execute(
                    "INSERT INTO chunks_fts(rowid, record_id, chunk_id, retrieval_text) VALUES (?1, ?2, ?3, ?4)",
                    params![rowid, record.id.to_string(), chunk_id, retrieval_text],
                )
                .map_err(|source| db_error("project the FTS chunk", source))?;
        }
        transaction
            .commit()
            .map_err(|source| db_error("commit the record projection", source))
    }

    fn delete_projection_by_path(&mut self, path: &str) -> crate::Result<bool> {
        let record_id: Option<String> = self
            .connection
            .query_row(
                "SELECT record_id FROM records WHERE path = ?1",
                params![path],
                |row| row.get(0),
            )
            .optional()
            .map_err(|source| db_error("find a stale projection", source))?;
        let Some(record_id) = record_id else {
            return Ok(false);
        };
        let transaction = self
            .connection
            .transaction()
            .map_err(|source| db_error("begin stale projection removal", source))?;
        delete_projection_tx(&transaction, &record_id)?;
        transaction
            .commit()
            .map_err(|source| db_error("commit stale projection removal", source))?;
        Ok(true)
    }

    fn projected_records(&self) -> crate::Result<Vec<ProjectedRecord>> {
        let mut statement = self
            .connection
            .prepare("SELECT record_id, path, content_hash FROM records ORDER BY path")
            .map_err(|source| db_error("read projection metadata", source))?;
        let records = statement
            .query_map([], |row| {
                Ok(ProjectedRecord {
                    record_id: row.get(0)?,
                    path: row.get(1)?,
                    content_hash: row.get(2)?,
                })
            })
            .map_err(|source| db_error("read projection metadata", source))?
            .collect::<rusqlite::Result<Vec<_>>>()
            .map_err(|source| db_error("read projection metadata", source))?;
        Ok(records)
    }

    fn search_hits(
        &self,
        paths: &StorePaths,
        query: &str,
        options: &SearchOptions,
    ) -> crate::Result<Vec<SearchHit>> {
        let terms = query_terms(query);
        if terms.is_empty() {
            return Ok(Vec::new());
        }
        let scopes = options.allowed_scopes.clone().unwrap_or_else(|| {
            SearchOptions::for_store(paths)
                .allowed_scopes
                .unwrap_or_default()
        });
        if scopes.is_empty() {
            return Ok(Vec::new());
        }
        let mut sql = String::from(
            "SELECT c.chunk_id, c.text, r.record_id, r.title, r.kind, s.name, r.status, r.access, r.path, r.aliases_json, bm25(chunks_fts) FROM chunks_fts JOIN chunks c ON c.rowid = chunks_fts.rowid JOIN records r ON r.record_id = c.record_id JOIN scopes s ON s.scope_id = r.scope_id WHERE chunks_fts MATCH ?1",
        );
        let mut values = vec![Value::Text(fts_query(&terms))];
        let mut next_parameter = 2;
        if !options.include_inactive {
            sql.push_str(&format!(" AND r.status = ?{next_parameter}"));
            values.push(Value::Text("active".to_owned()));
            next_parameter += 1;
        }
        if let Some(access) = &options.allowed_access {
            if access.is_empty() {
                return Ok(Vec::new());
            }
            let placeholders = (0..access.len())
                .map(|offset| format!("?{}", next_parameter + offset))
                .collect::<Vec<_>>()
                .join(", ");
            sql.push_str(&format!(" AND r.access IN ({placeholders})"));
            for value in access {
                values.push(Value::Text(value.to_string()));
            }
            next_parameter += access.len();
        }
        let placeholders = (0..scopes.len())
            .map(|offset| format!("?{}", next_parameter + offset))
            .collect::<Vec<_>>()
            .join(", ");
        sql.push_str(&format!(" AND s.name IN ({placeholders}) ORDER BY bm25(chunks_fts), c.record_id, c.ordinal LIMIT ?{}", next_parameter + scopes.len()));
        for scope in scopes {
            values.push(Value::Text(scope));
        }
        values.push(Value::Integer(
            (options.bounded_limit() * 10).min(1000) as i64
        ));

        let mut statement = self
            .connection
            .prepare(&sql)
            .map_err(|source| db_error("prepare lexical search", source))?;
        let rows = statement
            .query_map(params_from_iter(values.iter()), |row| {
                let aliases_json: String = row.get(9)?;
                let aliases =
                    serde_json::from_str::<Vec<String>>(&aliases_json).unwrap_or_default();
                let rank: f64 = row.get(10)?;
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, String>(2)?,
                    row.get::<_, String>(3)?,
                    row.get::<_, String>(4)?,
                    row.get::<_, String>(5)?,
                    row.get::<_, String>(6)?,
                    row.get::<_, String>(7)?,
                    row.get::<_, String>(8)?,
                    aliases,
                    rank,
                ))
            })
            .map_err(|source| db_error("run lexical search", source))?;

        let query_lower = query.trim().to_lowercase();
        let mut hits = Vec::new();
        for row in rows {
            let (
                chunk_id,
                text,
                record_id,
                title,
                kind,
                scope,
                status,
                access,
                path,
                aliases,
                rank,
            ) = row.map_err(|source| db_error("read lexical search result", source))?;
            let sources = self.sources_for(&record_id)?;
            let filename = Path::new(&path)
                .file_name()
                .and_then(|name| name.to_str())
                .unwrap_or(&path);
            let reason = if title.to_lowercase() == query_lower {
                "exact_title"
            } else if filename.to_lowercase() == query_lower {
                "exact_filename"
            } else if aliases
                .iter()
                .any(|alias| alias.to_lowercase() == query_lower)
            {
                "exact_alias"
            } else if query_lower.contains(' ')
                && (text.to_lowercase().contains(&query_lower)
                    || aliases
                        .iter()
                        .any(|alias| alias.to_lowercase().contains(&query_lower)))
            {
                "phrase"
            } else if terms.iter().any(|term| text_has_prefix(&text, term)) {
                "prefix"
            } else {
                "term"
            };
            let boost = match reason {
                "exact_title" => 3.0,
                "exact_filename" => 2.5,
                "exact_alias" => 2.0,
                "phrase" => 1.0,
                "prefix" => 0.5,
                _ => 0.0,
            };
            hits.push(SearchHit {
                record_id,
                chunk_id,
                title,
                kind,
                scope,
                status,
                access,
                text,
                sources,
                path,
                score: 1.0 / (1.0 + rank.abs()) + boost,
                lexical_match_reason: reason.to_owned(),
            });
        }
        let current = options
            .current_scope
            .clone()
            .or_else(|| current_scope(paths));
        sort_hits(&mut hits, current.as_deref());
        hits.truncate(options.bounded_limit());
        Ok(hits)
    }

    fn sources_for(&self, record_id: &str) -> crate::Result<Vec<SourceReceipt>> {
        let mut statement = self
            .connection
            .prepare("SELECT kind, reference, actor FROM sources WHERE record_id = ?1 ORDER BY source_id")
            .map_err(|source| db_error("prepare source lookup", source))?;
        let sources = statement
            .query_map(params![record_id], |row| {
                Ok(SourceReceipt {
                    kind: row.get(0)?,
                    reference: row.get(1)?,
                    actor: row.get(2)?,
                })
            })
            .map_err(|source| db_error("read source lookup", source))?
            .collect::<rusqlite::Result<Vec<_>>>()
            .map_err(|source| db_error("read source lookup", source))?;
        Ok(sources)
    }
}

impl From<SearchHit> for SearchResult {
    fn from(hit: SearchHit) -> Self {
        let excerpt = excerpt(&hit.text, 280);
        Self {
            record_id: hit.record_id,
            chunk_id: hit.chunk_id,
            title: hit.title,
            kind: hit.kind,
            scope: hit.scope,
            status: hit.status,
            access: hit.access,
            excerpt,
            sources: hit.sources,
            path: hit.path,
            score: hit.score,
            lexical_match_reason: hit.lexical_match_reason,
        }
    }
}

fn migrate(connection: &Connection) -> crate::Result<()> {
    let version: u32 = connection
        .query_row("PRAGMA user_version", [], |row| row.get(0))
        .map_err(|source| db_error("read index schema version", source))?;
    if version > INDEX_SCHEMA_VERSION {
        return Err(Error::InvalidInput {
            message: format!(
                "index schema version {version} is newer than supported version {INDEX_SCHEMA_VERSION}"
            ),
        });
    }
    let transaction = connection
        .unchecked_transaction()
        .map_err(|source| db_error("begin index migration", source))?;
    if version < 1 {
        transaction
            .execute_batch(
                "CREATE TABLE scopes (scope_id INTEGER PRIMARY KEY, name TEXT NOT NULL UNIQUE);
                 CREATE TABLE records (
                   record_id TEXT PRIMARY KEY,
                   scope_id INTEGER NOT NULL REFERENCES scopes(scope_id),
                   path TEXT NOT NULL UNIQUE,
                   title TEXT NOT NULL,
                   kind TEXT NOT NULL,
                   status TEXT NOT NULL,
                   access TEXT NOT NULL,
                   created_at TEXT NOT NULL,
                   updated_at TEXT NOT NULL,
                   aliases_json TEXT NOT NULL,
                   tags_json TEXT NOT NULL,
                   content_hash TEXT NOT NULL
                 );
                 CREATE TABLE chunks (
                   record_id TEXT NOT NULL REFERENCES records(record_id) ON DELETE CASCADE,
                   chunk_id TEXT NOT NULL UNIQUE,
                   ordinal INTEGER NOT NULL,
                   heading TEXT,
                   text TEXT NOT NULL,
                   retrieval_text TEXT NOT NULL,
                   token_count INTEGER NOT NULL,
                   PRIMARY KEY(record_id, ordinal)
                 );
                 CREATE TABLE sources (
                   source_id INTEGER PRIMARY KEY,
                   record_id TEXT NOT NULL REFERENCES records(record_id) ON DELETE CASCADE,
                   kind TEXT NOT NULL,
                   reference TEXT NOT NULL,
                   actor TEXT NOT NULL
                 );
                 CREATE TABLE index_metadata (key TEXT PRIMARY KEY, value TEXT NOT NULL);
                 INSERT INTO index_metadata(key, value) VALUES ('projection', 'stormbuffer-lexical');
                 PRAGMA user_version = 1;",
            )
            .map_err(|source| db_error("apply index migration 1", source))?;
    }
    if version < 2 {
        transaction
            .execute_batch(
                "CREATE VIRTUAL TABLE chunks_fts USING fts5(
                   record_id UNINDEXED,
                   chunk_id UNINDEXED,
                   retrieval_text,
                   content='',
                   contentless_delete=1,
                   tokenize='unicode61 remove_diacritics 0'
                 );
                 INSERT INTO index_metadata(key, value) VALUES ('fts_version', '5-contentless-delete') ON CONFLICT(key) DO UPDATE SET value=excluded.value;
                 PRAGMA user_version = 2;",
            )
            .map_err(|source| db_error("apply index migration 2", source))?;
    }
    transaction
        .commit()
        .map_err(|source| db_error("commit index migration", source))
}

fn delete_projection_tx(transaction: &Transaction<'_>, record_id: &str) -> crate::Result<()> {
    transaction
        .execute(
            "DELETE FROM chunks_fts WHERE rowid IN (SELECT rowid FROM chunks WHERE record_id = ?1)",
            params![record_id],
        )
        .map_err(|source| db_error("remove FTS chunks", source))?;
    transaction
        .execute(
            "DELETE FROM records WHERE record_id = ?1",
            params![record_id],
        )
        .map_err(|source| db_error("remove projected record", source))?;
    Ok(())
}

fn read_canonical(path: &Path) -> crate::Result<(Record, String)> {
    let bytes = fs::read(path).map_err(|source| Error::io("read a canonical record", source))?;
    let markdown = String::from_utf8(bytes)
        .map_err(|_| Error::invalid_input(format!("{} is not valid UTF-8", path.display())))?;
    let record = crate::parse_markdown(path, &markdown)?;
    Ok((record, markdown))
}

fn collect_canonical_records(directory: &Path) -> Vec<PathBuf> {
    collect_markdown_paths(directory).unwrap_or_default()
}

fn collect_markdown_paths(directory: &Path) -> crate::Result<Vec<PathBuf>> {
    let mut paths = Vec::new();
    collect_markdown_paths_inner(directory, &mut paths)?;
    paths.sort();
    Ok(paths)
}

fn collect_markdown_paths_inner(directory: &Path, paths: &mut Vec<PathBuf>) -> crate::Result<()> {
    let entries =
        fs::read_dir(directory).map_err(|source| Error::io("scan canonical records", source))?;
    for entry in entries {
        let entry = entry.map_err(|source| Error::io("scan canonical records", source))?;
        let file_type = entry
            .file_type()
            .map_err(|source| Error::io("inspect canonical records", source))?;
        if file_type.is_dir() {
            collect_markdown_paths_inner(&entry.path(), paths)?;
        } else if file_type.is_file() && entry.path().extension().is_some_and(|ext| ext == "md") {
            paths.push(entry.path());
        }
    }
    Ok(())
}

fn current_scope(paths: &StorePaths) -> Option<String> {
    match paths.scope {
        StoreScope::Global => Some("global".to_owned()),
        StoreScope::Project => {
            let name = paths
                .root
                .parent()
                .and_then(Path::file_name)
                .and_then(|value| value.to_str())?;
            let sanitized: String = name
                .chars()
                .map(|character| {
                    if character.is_whitespace() || character == ':' || character.is_control() {
                        '-'
                    } else {
                        character
                    }
                })
                .collect();
            (!sanitized.is_empty()).then(|| format!("project:{sanitized}"))
        }
    }
}

fn scope_rank(scope: &str, current: Option<&str>) -> u8 {
    if current == Some(scope) {
        2
    } else if scope == "global" {
        1
    } else {
        0
    }
}

fn sort_hits(hits: &mut [SearchHit], current: Option<&str>) {
    hits.sort_by(|left, right| {
        scope_rank(&right.scope, current)
            .cmp(&scope_rank(&left.scope, current))
            .then_with(|| right.score.total_cmp(&left.score))
            .then_with(|| left.record_id.cmp(&right.record_id))
            .then_with(|| left.chunk_id.cmp(&right.chunk_id))
    });
}

fn query_terms(query: &str) -> Vec<String> {
    query
        .split(|character: char| !character.is_alphanumeric() && character != '_')
        .filter(|term| !term.is_empty())
        .map(str::to_lowercase)
        .collect()
}

fn fts_query(terms: &[String]) -> String {
    if terms.len() == 1 {
        return format!("{}*", terms[0]);
    }
    let phrase = format!("\"{}\"", terms.join(" "));
    std::iter::once(phrase)
        .chain(terms.iter().map(|term| format!("{term}*")))
        .collect::<Vec<_>>()
        .join(" OR ")
}

fn text_has_prefix(text: &str, term: &str) -> bool {
    text.split(|character: char| !character.is_alphanumeric() && character != '_')
        .any(|word| word.to_lowercase().starts_with(term))
}

fn excerpt(text: &str, max_chars: usize) -> String {
    if text.chars().count() <= max_chars {
        return text.to_owned();
    }
    let mut result: String = text.chars().take(max_chars).collect();
    result.push('…');
    result
}

fn parse_heading(line: &str) -> Option<(usize, String)> {
    let level = line
        .chars()
        .take_while(|character| *character == '#')
        .count();
    if !(1..=6).contains(&level) || !line.chars().nth(level).is_some_and(char::is_whitespace) {
        return None;
    }
    let heading = line[level..].trim().trim_end_matches('#').trim();
    (!heading.is_empty()).then(|| (level, heading.to_owned()))
}

fn fence_start(line: &str) -> Option<char> {
    if line.starts_with("```") {
        Some('`')
    } else if line.starts_with("~~~") {
        Some('~')
    } else {
        None
    }
}

fn is_fence_end(line: &str, fence: char) -> bool {
    line.chars()
        .take_while(|character| *character == fence)
        .count()
        >= 3
}

fn is_list_start(line: &str) -> bool {
    line.starts_with("- ")
        || line.starts_with("* ")
        || line.starts_with("+ ")
        || line
            .split_once(' ')
            .is_some_and(|(prefix, _)| !prefix.is_empty() && prefix.ends_with('.'))
}

fn db_error(operation: &'static str, source: rusqlite::Error) -> Error {
    Error::Index { operation, source }
}

fn remove_sqlite_sidecars(path: &Path) {
    for suffix in ["-wal", "-shm"] {
        let _ = fs::remove_file(path.with_extension(format!("sqlite3{suffix}")));
        let sidecar = PathBuf::from(format!("{}{}", path.display(), suffix));
        let _ = fs::remove_file(sidecar);
    }
}

fn sync_parent_directory(path: &Path) -> io::Result<()> {
    File::open(path.parent().unwrap_or_else(|| Path::new(".")))?.sync_all()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{Access, RecordId, RecordKind, RecordStatus, Scope, Source, SourceKind, Timestamp};

    fn record(body: &str) -> Record {
        let now = Timestamp::now_utc();
        Record {
            id: RecordId::new_v7(),
            title: "Chunk test".to_owned(),
            kind: RecordKind::Fact,
            scope: Scope::parse("global").expect("scope"),
            status: RecordStatus::Active,
            access: Access::Human,
            created_at: now,
            updated_at: now,
            tags: vec!["test".to_owned()],
            aliases: vec!["chunking".to_owned()],
            supersedes: Vec::new(),
            sources: vec![Source {
                kind: SourceKind::Document,
                reference: "test.md".to_owned(),
                actor: "tester".to_owned(),
            }],
            body: body.to_owned(),
        }
    }

    #[test]
    fn chunks_keep_fenced_code_and_lists_together() {
        let chunks = chunk_record(&record(
            "# Heading\n\n- one\n- two\n\n```rust\nlet x = 1;\nlet y = 2;\n```",
        ));
        assert_eq!(chunks.len(), 2);
        assert!(chunks[0].2.contains("- one\n- two"));
        assert!(chunks[1].2.starts_with("```rust"));
        assert_eq!(chunks[0].1.as_deref(), Some("Heading"));
    }

    #[test]
    fn migration_from_version_one_creates_fts() {
        let path = std::env::temp_dir().join(format!(
            "stormbuffer-migration-{}.sqlite3",
            std::process::id()
        ));
        let _ = fs::remove_file(&path);
        let connection = Connection::open(&path).expect("open database");
        connection.execute_batch(
            "CREATE TABLE scopes (scope_id INTEGER PRIMARY KEY, name TEXT NOT NULL UNIQUE);
             CREATE TABLE records (record_id TEXT PRIMARY KEY, scope_id INTEGER NOT NULL REFERENCES scopes(scope_id), path TEXT NOT NULL UNIQUE, title TEXT NOT NULL, kind TEXT NOT NULL, status TEXT NOT NULL, access TEXT NOT NULL, created_at TEXT NOT NULL, updated_at TEXT NOT NULL, aliases_json TEXT NOT NULL, tags_json TEXT NOT NULL, content_hash TEXT NOT NULL);
             CREATE TABLE chunks (record_id TEXT NOT NULL REFERENCES records(record_id) ON DELETE CASCADE, chunk_id TEXT NOT NULL UNIQUE, ordinal INTEGER NOT NULL, heading TEXT, text TEXT NOT NULL, retrieval_text TEXT NOT NULL, token_count INTEGER NOT NULL, PRIMARY KEY(record_id, ordinal));
             CREATE TABLE sources (source_id INTEGER PRIMARY KEY, record_id TEXT NOT NULL REFERENCES records(record_id) ON DELETE CASCADE, kind TEXT NOT NULL, reference TEXT NOT NULL, actor TEXT NOT NULL);
             CREATE TABLE index_metadata (key TEXT PRIMARY KEY, value TEXT NOT NULL);
             PRAGMA user_version = 1;",
        ).expect("create version one schema");
        drop(connection);
        let index = Index::open_at(&path).expect("migrate version one");
        let version: u32 = index
            .connection
            .query_row("PRAGMA user_version", [], |row| row.get(0))
            .expect("read migrated version");
        assert_eq!(version, INDEX_SCHEMA_VERSION);
        let _ = fs::remove_file(&path);
        let _ = fs::remove_file(format!("{}-wal", path.display()));
        let _ = fs::remove_file(format!("{}-shm", path.display()));
    }
}
