use std::collections::HashSet;
use std::sync::Once;

use rusqlite::types::Value;
use rusqlite::{Connection, OptionalExtension, params, params_from_iter};
use serde::Serialize;

use crate::{Embedder, Embedding, Error, Result};

#[derive(Clone, Debug, Default)]
pub struct VectorFilter {
    pub scopes: Option<Vec<String>>,
    pub kinds: Option<Vec<String>>,
    pub statuses: Option<Vec<String>>,
    pub accesses: Option<Vec<String>>,
}

#[derive(Clone, Debug, Serialize)]
pub struct VectorHit {
    pub record_id: String,
    pub chunk_id: String,
    pub scope: String,
    pub kind: String,
    pub status: String,
    pub access: String,
    pub distance: f64,
}

#[derive(Clone, Debug, Serialize)]
pub struct VectorMetadata {
    pub index_id: i64,
    pub model_version: String,
    pub model_checksum: String,
    pub dimension: usize,
    pub table_name: String,
    pub canonical_fingerprint: String,
    pub projection_fingerprint: String,
}

pub trait VectorIndex {
    fn metadata(&self) -> &VectorMetadata;
    fn search(&self, embedding: &Embedding, filter: &VectorFilter, limit: usize) -> Result<Vec<VectorHit>>;
}

pub struct SqliteVectorIndex<'a> {
    connection: &'a Connection,
    metadata: VectorMetadata,
}

impl<'a> SqliteVectorIndex<'a> {
    pub fn active(connection: &'a Connection) -> Result<Option<Self>> {
        register_sqlite_vec();
        let row = connection
            .query_row(
                "SELECT index_id, model_version, model_checksum, dimension, table_name, canonical_fingerprint, projection_fingerprint FROM vector_indexes WHERE active = 1 ORDER BY index_id DESC LIMIT 1",
                [],
                |row| {
                    Ok(VectorMetadata {
                        index_id: row.get(0)?,
                        model_version: row.get(1)?,
                        model_checksum: row.get(2)?,
                        dimension: row.get::<_, i64>(3)? as usize,
                        table_name: row.get(4)?,
                        canonical_fingerprint: row.get(5)?,
                        projection_fingerprint: row.get(6)?,
                    })
                },
            )
            .optional()
            .map_err(|source| db_error("read active vector metadata", source))?;
        let Some(metadata) = row else {
            return Ok(None);
        };
        validate_table_name(&metadata.table_name)?;
        Ok(Some(Self { connection, metadata }))
    }

    pub(crate) fn rebuild(
        connection: &mut Connection, embedder: &dyn Embedder, documents: &[VectorDocument],
        canonical_fingerprint: String, projection_fingerprint: String,
    ) -> Result<VectorMetadata> {
        register_sqlite_vec();
        let dimension = embedder.dimension();
        if dimension == 0 {
            return Err(Error::embedding(
                "build vector index",
                "embedder dimension must be positive",
            ));
        }
        let mut embeddings = Vec::with_capacity(documents.len());
        for document in documents {
            let embedding = embedder.embed(&document.text)?;
            if embedding.dimension() != dimension {
                return Err(Error::embedding(
                    "build vector index",
                    format!(
                        "embedder returned dimension {}, expected {dimension}",
                        embedding.dimension()
                    ),
                ));
            }
            embeddings.push(embedding);
        }

        let index_id: i64 = connection
            .query_row("SELECT COALESCE(MAX(index_id), 0) + 1 FROM vector_indexes", [], |row| {
                row.get(0)
            })
            .map_err(|source| db_error("allocate vector index version", source))?;
        let table_name = format!("vectors_{index_id}");
        let metadata = VectorMetadata {
            index_id,
            model_version: embedder.model_version().to_owned(),
            model_checksum: embedder.model_checksum().to_owned(),
            dimension,
            table_name: table_name.clone(),
            canonical_fingerprint,
            projection_fingerprint,
        };
        let quoted = quote_identifier(&table_name)?;
        let result = (|| {
            connection
                .execute(
                    &format!("CREATE VIRTUAL TABLE {quoted} USING vec0(embedding float[{dimension}], +record_id text, +chunk_id text, +scope text, +kind text, +status text, +access text)"),
                    [],
                )
                .map_err(|source| db_error("create vector table", source))?;
            connection
                .execute(
                    "INSERT INTO vector_indexes(index_id, model_version, model_checksum, dimension, table_name, canonical_fingerprint, projection_fingerprint, active) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, 0)",
                    params![
                        metadata.index_id,
                        metadata.model_version,
                        metadata.model_checksum,
                        metadata.dimension as i64,
                        metadata.table_name,
                        metadata.canonical_fingerprint,
                        metadata.projection_fingerprint,
                    ],
                )
                .map_err(|source| db_error("record vector metadata", source))?;
            for (document, embedding) in documents.iter().zip(embeddings.iter()) {
                connection
                    .execute(
                        &format!("INSERT INTO {quoted}(embedding, record_id, chunk_id, scope, kind, status, access) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)"),
                        params![
                            vector_blob(&embedding.values),
                            document.record_id,
                            document.chunk_id,
                            document.scope,
                            document.kind,
                            document.status,
                            document.access,
                        ],
                    )
                    .map_err(|source| db_error("backfill vector table", source))?;
            }
            let count: i64 = connection
                .query_row(&format!("SELECT count(*) FROM {quoted}"), [], |row| row.get(0))
                .map_err(|source| db_error("validate vector backfill", source))?;
            if count != documents.len() as i64 {
                return Err(Error::embedding(
                    "validate vector backfill",
                    format!("vector table contains {count} rows, expected {}", documents.len()),
                ));
            }
            let transaction = connection
                .transaction()
                .map_err(|source| db_error("begin vector index switch", source))?;
            transaction
                .execute("UPDATE vector_indexes SET active = 0 WHERE active = 1", [])
                .map_err(|source| db_error("switch vector index", source))?;
            transaction
                .execute(
                    "UPDATE vector_indexes SET active = 1 WHERE index_id = ?1",
                    params![index_id],
                )
                .map_err(|source| db_error("activate vector index", source))?;
            transaction
                .commit()
                .map_err(|source| db_error("commit vector index switch", source))?;
            Ok::<_, Error>(())
        })();
        if result.is_err() {
            let _ = connection.execute(&format!("DROP TABLE IF EXISTS {quoted}"), []);
            let _ = connection.execute("DELETE FROM vector_indexes WHERE index_id = ?1", params![index_id]);
        }
        result.map(|()| metadata)
    }

    pub(crate) fn cleanup_obsolete(connection: &Connection, active_index_id: i64) -> Result<()> {
        let mut statement = connection
            .prepare("SELECT index_id, table_name FROM vector_indexes WHERE index_id != ?1")
            .map_err(|source| db_error("find obsolete vector indexes", source))?;
        let obsolete = statement
            .query_map(params![active_index_id], |row| {
                Ok((row.get::<_, i64>(0)?, row.get::<_, String>(1)?))
            })
            .map_err(|source| db_error("read obsolete vector indexes", source))?
            .collect::<rusqlite::Result<Vec<_>>>()
            .map_err(|source| db_error("read obsolete vector indexes", source))?;
        drop(statement);

        for (_, table_name) in &obsolete {
            let quoted = quote_identifier(table_name)?;
            connection
                .execute(&format!("DROP TABLE IF EXISTS {quoted}"), [])
                .map_err(|source| db_error("remove obsolete vector table", source))?;
        }
        connection
            .execute(
                "DELETE FROM vector_indexes WHERE index_id != ?1",
                params![active_index_id],
            )
            .map_err(|source| db_error("remove obsolete vector metadata", source))?;
        Ok(())
    }
}

impl VectorIndex for SqliteVectorIndex<'_> {
    fn metadata(&self) -> &VectorMetadata {
        &self.metadata
    }

    fn search(&self, embedding: &Embedding, filter: &VectorFilter, limit: usize) -> Result<Vec<VectorHit>> {
        if embedding.dimension() != self.metadata.dimension {
            return Err(Error::embedding(
                "search vector index",
                format!(
                    "query dimension {} does not match index dimension {}",
                    embedding.dimension(),
                    self.metadata.dimension
                ),
            ));
        }
        let requested = limit.max(1);
        if filter.scopes.as_ref().is_some_and(Vec::is_empty)
            || filter.kinds.as_ref().is_some_and(Vec::is_empty)
            || filter.statuses.as_ref().is_some_and(Vec::is_empty)
            || filter.accesses.as_ref().is_some_and(Vec::is_empty)
        {
            return Ok(Vec::new());
        }
        let quoted = quote_identifier(&self.metadata.table_name)?;
        let total: usize = self
            .connection
            .query_row(&format!("SELECT count(*) FROM {quoted}"), [], |row| {
                row.get::<_, i64>(0).map(|count| count.max(0) as usize)
            })
            .map_err(|source| db_error("count vector candidates", source))?;
        if total == 0 {
            return Ok(Vec::new());
        }

        // sqlite-vec auxiliary columns cannot be constrained in a KNN WHERE clause,
        // so adaptively over-fetch until the filtered result is complete or all rows
        // have been examined.
        let mut candidate_count = requested.saturating_mul(4).max(requested).min(total);
        loop {
            let sql = format!(
                "SELECT record_id, chunk_id, scope, kind, status, access, distance FROM {quoted} WHERE embedding MATCH ?1 AND k = ?2"
            );
            let values = [
                Value::Blob(vector_blob(&embedding.values)),
                Value::Integer(candidate_count as i64),
            ];
            let mut statement = self
                .connection
                .prepare(&sql)
                .map_err(|source| db_error("prepare vector search", source))?;
            let rows = statement
                .query_map(params_from_iter(values.iter()), |row| {
                    Ok(VectorHit {
                        record_id: row.get(0)?,
                        chunk_id: row.get(1)?,
                        scope: row.get(2)?,
                        kind: row.get(3)?,
                        status: row.get(4)?,
                        access: row.get(5)?,
                        distance: row.get(6)?,
                    })
                })
                .map_err(|source| db_error("run vector search", source))?;
            let mut hits = Vec::new();
            let mut seen_chunks = HashSet::new();
            for row in rows {
                let hit = row.map_err(|source| db_error("read vector search result", source))?;
                if matches_filter(&hit, filter) && seen_chunks.insert((hit.record_id.clone(), hit.chunk_id.clone())) {
                    hits.push(hit);
                    if hits.len() >= requested {
                        break;
                    }
                }
            }
            if hits.len() >= requested || candidate_count == total {
                return Ok(hits);
            }
            candidate_count = candidate_count.saturating_mul(2).min(total);
        }
    }
}

#[derive(Clone, Debug)]
pub(crate) struct VectorDocument {
    pub record_id: String,
    pub chunk_id: String,
    pub scope: String,
    pub kind: String,
    pub status: String,
    pub access: String,
    pub text: String,
}

pub(crate) fn register_sqlite_vec() {
    static REGISTER: Once = Once::new();
    REGISTER.call_once(|| {
        // sqlite-vec exposes a SQLite extension entry point. Registering it once before
        // opening connections is the safe boundary used by the crate's rusqlite example.
        let init = unsafe {
            std::mem::transmute::<*const (), rusqlite::auto_extension::RawAutoExtension>(
                sqlite_vec::sqlite3_vec_init as *const (),
            )
        };
        // sqlite-vec's entry point is a process-wide SQLite auto-extension and is
        // registered once before any vector connection is opened.
        let _ = unsafe { rusqlite::auto_extension::register_auto_extension(init) };
    });
}

fn matches_filter(hit: &VectorHit, filter: &VectorFilter) -> bool {
    filter
        .scopes
        .as_ref()
        .is_none_or(|values| values.iter().any(|value| value == &hit.scope))
        && filter
            .kinds
            .as_ref()
            .is_none_or(|values| values.iter().any(|value| value == &hit.kind))
        && filter
            .statuses
            .as_ref()
            .is_none_or(|values| values.iter().any(|value| value == &hit.status))
        && filter
            .accesses
            .as_ref()
            .is_none_or(|values| values.iter().any(|value| value == &hit.access))
}

fn vector_blob(values: &[f32]) -> Vec<u8> {
    values.iter().flat_map(|value| value.to_le_bytes()).collect()
}

fn validate_table_name(name: &str) -> Result<()> {
    if name.is_empty() || !name.bytes().all(|byte| byte.is_ascii_alphanumeric() || byte == b'_') {
        return Err(Error::embedding("read vector metadata", "vector table name is invalid"));
    }
    Ok(())
}

fn quote_identifier(name: &str) -> Result<String> {
    validate_table_name(name)?;
    Ok(format!("\"{name}\""))
}

fn db_error(operation: &'static str, source: rusqlite::Error) -> Error {
    Error::Index { operation, source }
}
