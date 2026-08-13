use std::collections::{HashMap, HashSet};
use std::fs::{self, File, OpenOptions};
use std::io;
use std::path::Path;

use fs2::FileExt;
use rusqlite::types::Value;
use rusqlite::{Connection, OptionalExtension, params, params_from_iter};

use crate::vector::{
    SqliteVectorIndex, VectorDocument, VectorFilter, VectorIndex, VectorMetadata,
    register_sqlite_vec,
};
use crate::{Embedder, Error, Record, StorePaths};

use super::canonical::{
    canonical_fingerprint, collect_markdown_paths, fingerprint_value, read_canonical,
};
use super::chunking::{chunk_record, retrieval_text, split_embedding_text};
use super::retrieval::{
    SearchHit, current_scope, fts_query, query_terms, scope_rank, sort_hits, text_has_prefix,
    vector_hit_matches_filter,
};
use super::schema::{delete_projection_tx, migrate};
use super::{SearchOptions, SourceReceipt, SyncInvalidFile, SyncReport, content_hash, db_error};

pub(super) struct ProjectionLock {
    file: File,
}

impl ProjectionLock {
    pub(super) fn acquire(index_path: &Path) -> crate::Result<Self> {
        let parent = index_path.parent().ok_or_else(|| {
            Error::io(
                "resolve the index directory",
                io::Error::other("index path has no parent"),
            )
        })?;
        fs::create_dir_all(parent)
            .map_err(|source| Error::io("create the index directory", source))?;
        let file = OpenOptions::new()
            .create(true)
            .truncate(false)
            .read(true)
            .write(true)
            .open(parent.join("projection.lock"))
            .map_err(|source| Error::io("open the projection lock", source))?;
        match file.try_lock_exclusive() {
            Ok(()) => Ok(Self { file }),
            Err(source) if source.kind() == io::ErrorKind::WouldBlock => Err(Error::IndexBusy),
            Err(source) => Err(Error::io("lock the projection", source)),
        }
    }
}

impl Drop for ProjectionLock {
    fn drop(&mut self) {
        let _ = FileExt::unlock(&self.file);
    }
}

#[derive(Clone, Debug)]
pub(super) struct ProjectedRecord {
    pub(super) record_id: String,
    pub(super) path: String,
    pub(super) content_hash: String,
}

pub(super) struct Index {
    pub(super) connection: Connection,
}

impl Index {
    pub(super) fn open_at(path: &Path) -> crate::Result<Self> {
        Self::try_open_at(path).map_err(|source| Error::IndexUnavailable {
            source: Box::new(source),
        })
    }

    pub(super) fn try_open_at(path: &Path) -> crate::Result<Self> {
        register_sqlite_vec();
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

    pub(super) fn checkpoint(&mut self) -> crate::Result<()> {
        self.connection
            .execute_batch("PRAGMA wal_checkpoint(TRUNCATE);")
            .map_err(|source| db_error("checkpoint the index", source))
    }

    pub(super) fn rebuild_vectors(
        &mut self,
        paths: &StorePaths,
        embedder: &dyn Embedder,
    ) -> crate::Result<VectorMetadata> {
        let canonical_fingerprint = canonical_fingerprint(paths)?;
        let projection_fingerprint = self.projection_fingerprint()?;
        if let Some(active) = SqliteVectorIndex::active(&self.connection)?
            && active.metadata().model_version == embedder.model_version()
            && active.metadata().model_checksum == embedder.model_checksum()
            && active.metadata().dimension == embedder.dimension()
            && active.metadata().canonical_fingerprint == canonical_fingerprint
            && active.metadata().projection_fingerprint == projection_fingerprint
        {
            let metadata = active.metadata().clone();
            drop(active);
            SqliteVectorIndex::cleanup_obsolete(&self.connection, metadata.index_id)?;
            return Ok(metadata);
        }

        let mut statement = self.connection.prepare(
            "SELECT r.record_id, c.chunk_id, s.name, r.kind, r.status, r.access, c.retrieval_text FROM chunks c JOIN records r ON r.record_id = c.record_id JOIN scopes s ON s.scope_id = r.scope_id ORDER BY r.record_id, c.ordinal",
        ).map_err(|source| db_error("prepare vector backfill", source))?;
        let lexical_documents = statement
            .query_map([], |row| {
                Ok(VectorDocument {
                    record_id: row.get(0)?,
                    chunk_id: row.get(1)?,
                    scope: row.get(2)?,
                    kind: row.get(3)?,
                    status: row.get(4)?,
                    access: row.get(5)?,
                    text: row.get(6)?,
                })
            })
            .map_err(|source| db_error("read vector backfill", source))?
            .collect::<rusqlite::Result<Vec<_>>>()
            .map_err(|source| db_error("read vector backfill", source))?;
        drop(statement);
        let mut documents = Vec::new();
        for document in lexical_documents {
            for text in split_embedding_text(&document.text, embedder)? {
                documents.push(VectorDocument {
                    text,
                    ..document.clone()
                });
            }
        }
        let metadata = SqliteVectorIndex::rebuild(
            &mut self.connection,
            embedder,
            &documents,
            canonical_fingerprint,
            projection_fingerprint,
        )?;
        SqliteVectorIndex::cleanup_obsolete(&self.connection, metadata.index_id)?;
        Ok(metadata)
    }

    pub(super) fn projection_fingerprint(&self) -> crate::Result<String> {
        let mut hasher = blake3::Hasher::new();
        fingerprint_value(&mut hasher, "vector-chunking-v1");
        let mut records = self
            .connection
            .prepare(
                "SELECT r.record_id, r.path, r.content_hash, r.kind, r.status, r.access, s.name FROM records r JOIN scopes s ON s.scope_id = r.scope_id ORDER BY r.path",
            )
            .map_err(|source| db_error("prepare projection fingerprint", source))?;
        let rows = records
            .query_map([], |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, String>(2)?,
                    row.get::<_, String>(3)?,
                    row.get::<_, String>(4)?,
                    row.get::<_, String>(5)?,
                    row.get::<_, String>(6)?,
                ))
            })
            .map_err(|source| db_error("read projection fingerprint", source))?;
        for row in rows {
            let (record_id, path, content_hash, kind, status, access, scope) =
                row.map_err(|source| db_error("read projection fingerprint", source))?;
            for value in [
                record_id.as_str(),
                path.as_str(),
                content_hash.as_str(),
                kind.as_str(),
                status.as_str(),
                access.as_str(),
                scope.as_str(),
            ] {
                fingerprint_value(&mut hasher, value);
            }
            let mut chunks = self
                .connection
                .prepare(
                    "SELECT chunk_id, retrieval_text, text, token_count FROM chunks WHERE record_id = ?1 ORDER BY ordinal",
                )
                .map_err(|source| db_error("prepare chunk fingerprint", source))?;
            let chunk_rows = chunks
                .query_map(params![record_id], |row| {
                    Ok((
                        row.get::<_, String>(0)?,
                        row.get::<_, String>(1)?,
                        row.get::<_, String>(2)?,
                        row.get::<_, i64>(3)?,
                    ))
                })
                .map_err(|source| db_error("read chunk fingerprint", source))?;
            for chunk in chunk_rows {
                let (chunk_id, retrieval_text, text, token_count) =
                    chunk.map_err(|source| db_error("read chunk fingerprint", source))?;
                let token_count = token_count.to_string();
                for value in [
                    chunk_id.as_str(),
                    retrieval_text.as_str(),
                    text.as_str(),
                    token_count.as_str(),
                ] {
                    fingerprint_value(&mut hasher, value);
                }
            }
        }
        Ok(hasher.finalize().to_hex().to_string())
    }

    pub(super) fn vector_hits(
        &self,
        paths: &StorePaths,
        query: &str,
        options: &SearchOptions,
        embedder: &dyn Embedder,
    ) -> crate::Result<Vec<SearchHit>> {
        let Some(vector) = SqliteVectorIndex::active(&self.connection)? else {
            return Ok(Vec::new());
        };
        let canonical_fingerprint = canonical_fingerprint(paths)?;
        let projection_fingerprint = self.projection_fingerprint()?;
        if vector.metadata().model_version != embedder.model_version()
            || vector.metadata().model_checksum != embedder.model_checksum()
            || vector.metadata().dimension != embedder.dimension()
            || vector.metadata().canonical_fingerprint != canonical_fingerprint
            || vector.metadata().projection_fingerprint != projection_fingerprint
        {
            return Err(Error::embedding(
                "search vector index",
                "active semantic index is stale for the canonical or lexical projection; run `sbuf sync` followed by `sbuf reindex`",
            ));
        }
        let scopes = options.allowed_scopes.clone().unwrap_or_else(|| {
            SearchOptions::for_store(paths)
                .allowed_scopes
                .unwrap_or_default()
        });
        let filter = VectorFilter {
            scopes: Some(scopes),
            kinds: options.allowed_kinds.clone(),
            statuses: Some(if options.include_inactive {
                vec![
                    "candidate".to_owned(),
                    "active".to_owned(),
                    "superseded".to_owned(),
                    "archived".to_owned(),
                ]
            } else {
                vec!["active".to_owned()]
            }),
            accesses: options
                .allowed_access
                .as_ref()
                .map(|values| values.iter().map(ToString::to_string).collect()),
        };
        let embedding = embedder.embed(query)?;
        let vector_hits = vector.search(&embedding, &filter, options.bounded_limit())?;
        let mut hits = Vec::with_capacity(vector_hits.len());
        for vector_hit in vector_hits {
            let row = self.connection.query_row(
                "SELECT c.text, r.record_id, r.title, r.kind, s.name, r.status, r.access, r.path FROM chunks c JOIN records r ON r.record_id = c.record_id JOIN scopes s ON s.scope_id = r.scope_id WHERE c.chunk_id = ?1 AND r.record_id = ?2",
                params![vector_hit.chunk_id, vector_hit.record_id],
                |row| Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, String>(2)?,
                    row.get::<_, String>(3)?,
                    row.get::<_, String>(4)?,
                    row.get::<_, String>(5)?,
                    row.get::<_, String>(6)?,
                    row.get::<_, String>(7)?,
                )),
            ).optional().map_err(|source| db_error("read vector result", source))?;
            let Some((text, record_id, title, kind, scope, status, access, path)) = row else {
                continue;
            };
            let current = crate::vector::VectorHit {
                record_id: record_id.clone(),
                chunk_id: vector_hit.chunk_id.clone(),
                scope: scope.clone(),
                kind: kind.clone(),
                status: status.clone(),
                access: access.clone(),
                distance: vector_hit.distance,
            };
            if !vector_hit_matches_filter(&current, &filter) {
                continue;
            }
            hits.push(SearchHit {
                record_id,
                chunk_id: vector_hit.chunk_id,
                title,
                kind,
                scope,
                status,
                access,
                text,
                sources: self.sources_for(&current.record_id)?,
                path,
                score: 1.0 / (1.0 + vector_hit.distance.abs()),
                lexical_match_reason: "vector".to_owned(),
                match_reasons: vec![format!("vector:distance={:.6}", vector_hit.distance)],
                vector_distance: Some(vector_hit.distance),
            });
        }
        let current = options
            .current_scope
            .clone()
            .or_else(|| current_scope(paths));
        hits.sort_by(|left, right| {
            left.score
                .total_cmp(&right.score)
                .reverse()
                .then_with(|| {
                    scope_rank(&right.scope, current.as_deref())
                        .cmp(&scope_rank(&left.scope, current.as_deref()))
                })
                .then_with(|| left.record_id.cmp(&right.record_id))
                .then_with(|| left.chunk_id.cmp(&right.chunk_id))
        });
        Ok(hits)
    }

    pub(super) fn sync_canonical(&mut self, paths: &StorePaths) -> crate::Result<SyncReport> {
        let expected_scope = crate::record_scope(paths)?;
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
            if record.scope != expected_scope {
                report.invalid_files.push(SyncInvalidFile {
                    path: path_string.clone(),
                    error: "record is outside the selected store scope".to_owned(),
                });
                if self.delete_projection_by_path(&path_string)? {
                    report.removed += 1;
                }
                continue;
            }
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

    pub(super) fn project_record(
        &mut self,
        record: &Record,
        path: &str,
        hash: &str,
    ) -> crate::Result<()> {
        let chunks = chunk_record(record);
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
                    "INSERT INTO sources(record_id, kind, reference, actor, observed_at, revision, content_hash) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
                    params![
                        record.id.to_string(),
                        source.kind.to_string(),
                        source.reference,
                        source.actor,
                        source.observed_at.map(|value| value.to_string()),
                        source.revision,
                        source.content_hash,
                    ],
                )
                .map_err(|source| db_error("project the record source", source))?;
        }

        for (ordinal, (chunk_id, heading, text, token_count)) in chunks.into_iter().enumerate() {
            let heading_text = heading.clone().unwrap_or_default();
            let filename = Path::new(path)
                .file_name()
                .and_then(|name| name.to_str())
                .unwrap_or(path);
            let retrieval_text = retrieval_text(record, &heading_text, filename, &text);
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

    pub(super) fn delete_projection_by_path(&mut self, path: &str) -> crate::Result<bool> {
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

    pub(super) fn projected_records(&self) -> crate::Result<Vec<ProjectedRecord>> {
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

    pub(super) fn search_hits(
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
        if let Some(kinds) = &options.allowed_kinds {
            if kinds.is_empty() {
                return Ok(Vec::new());
            }
            let placeholders = (0..kinds.len())
                .map(|offset| format!("?{}", next_parameter + offset))
                .collect::<Vec<_>>()
                .join(", ");
            sql.push_str(&format!(" AND r.kind IN ({placeholders})"));
            for kind in kinds {
                values.push(Value::Text(kind.clone()));
            }
            next_parameter += kinds.len();
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
                match_reasons: vec![format!("lexical:{reason}")],
                vector_distance: None,
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

    pub(super) fn sources_for(&self, record_id: &str) -> crate::Result<Vec<SourceReceipt>> {
        let mut statement = self
            .connection
            .prepare("SELECT kind, reference, actor, observed_at, revision, content_hash FROM sources WHERE record_id = ?1 ORDER BY source_id")
            .map_err(|source| db_error("prepare source lookup", source))?;
        let sources = statement
            .query_map(params![record_id], |row| {
                Ok(SourceReceipt {
                    kind: row.get(0)?,
                    reference: row.get(1)?,
                    actor: row.get(2)?,
                    observed_at: row.get(3)?,
                    revision: row.get(4)?,
                    content_hash: row.get(5)?,
                })
            })
            .map_err(|source| db_error("read source lookup", source))?
            .collect::<rusqlite::Result<Vec<_>>>()
            .map_err(|source| db_error("read source lookup", source))?;
        Ok(sources)
    }
}
