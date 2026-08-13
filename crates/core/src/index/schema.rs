use rusqlite::{Connection, Transaction, params};

use super::{INDEX_SCHEMA_VERSION, db_error};
use crate::Error;

pub(super) fn migrate(connection: &Connection) -> crate::Result<()> {
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
    if version < 3 {
        transaction
            .execute_batch(
                "CREATE TABLE vector_indexes (
                   index_id INTEGER PRIMARY KEY,
                   model_version TEXT NOT NULL,
                   model_checksum TEXT NOT NULL,
                   dimension INTEGER NOT NULL,
                   table_name TEXT NOT NULL UNIQUE,
                   active INTEGER NOT NULL CHECK (active IN (0, 1))
                 );
                 INSERT INTO index_metadata(key, value) VALUES ('vector_schema_version', '1') ON CONFLICT(key) DO UPDATE SET value=excluded.value;
                 PRAGMA user_version = 3;",
            )
            .map_err(|source| db_error("apply index migration 3", source))?;
    }
    if version < 4 {
        transaction
            .execute_batch(
                "ALTER TABLE vector_indexes ADD COLUMN canonical_fingerprint TEXT NOT NULL DEFAULT '';\n                 ALTER TABLE vector_indexes ADD COLUMN projection_fingerprint TEXT NOT NULL DEFAULT '';\n                 INSERT INTO index_metadata(key, value) VALUES ('vector_schema_version', '2') ON CONFLICT(key) DO UPDATE SET value=excluded.value;\n                 PRAGMA user_version = 4;",
            )
            .map_err(|source| db_error("apply index migration 4", source))?;
    }
    if version < 5 {
        transaction
            .execute_batch(
                "CREATE TABLE advisory_relations (
                   left_record_id TEXT NOT NULL,
                   right_record_id TEXT NOT NULL,
                   relation TEXT NOT NULL,
                   evidence_json TEXT NOT NULL,
                   confidence TEXT NOT NULL,
                   analyzer_fingerprint TEXT NOT NULL,
                   PRIMARY KEY(left_record_id, right_record_id, analyzer_fingerprint)
                 );
                 INSERT INTO index_metadata(key, value) VALUES ('relation_schema_version', '1') ON CONFLICT(key) DO UPDATE SET value=excluded.value;
                 PRAGMA user_version = 5;",
            )
            .map_err(|source| db_error("apply index migration 5", source))?;
    }
    if version < 6 {
        transaction
            .execute_batch(
                "ALTER TABLE sources ADD COLUMN observed_at TEXT;
                 ALTER TABLE sources ADD COLUMN revision TEXT;
                 ALTER TABLE sources ADD COLUMN content_hash TEXT;
                 PRAGMA user_version = 6;",
            )
            .map_err(|source| db_error("apply index migration 6", source))?;
    }
    transaction
        .commit()
        .map_err(|source| db_error("commit index migration", source))
}

pub(super) fn delete_projection_tx(
    transaction: &Transaction<'_>,
    record_id: &str,
) -> crate::Result<()> {
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
