use std::fs;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use rusqlite::Connection;
use serde_json::{Value, json};
use stormbuffer_core as core;
use stormbuffer_mcp::McpServer;

const DEFAULT_SIZES: &[usize] = &[100, 1_000, 10_000];
const DEFAULT_SAMPLES: usize = 20;
const EMBEDDING_DIMENSION: usize = 24;
const QUERIES: &[&str] = &[
    "deployment rollback procedure",
    "database migration decision",
    "authentication timeout",
    "release checkpoint owner",
];

struct Config {
    sizes: Vec<usize>,
    samples: usize,
    root: Option<PathBuf>,
}

impl Config {
    fn parse() -> Result<Self, String> {
        let mut sizes = DEFAULT_SIZES.to_vec();
        let mut samples = DEFAULT_SAMPLES;
        let mut root = None;
        let mut arguments = std::env::args().skip(1);
        while let Some(argument) = arguments.next() {
            match argument.as_str() {
                "--sizes" => {
                    let value = arguments.next().ok_or("--sizes requires a value")?;
                    sizes = value
                        .split(',')
                        .map(|size| {
                            size.parse::<usize>()
                                .map_err(|_| format!("invalid store size: {size}"))
                        })
                        .collect::<Result<Vec<_>, _>>()?;
                    if sizes.is_empty() || sizes.contains(&0) {
                        return Err("store sizes must be positive".to_owned());
                    }
                }
                "--samples" => {
                    let value = arguments.next().ok_or("--samples requires a value")?;
                    samples = value
                        .parse()
                        .map_err(|_| format!("invalid sample count: {value}"))?;
                    if samples == 0 {
                        return Err("sample count must be positive".to_owned());
                    }
                }
                "--root" => {
                    root = Some(PathBuf::from(
                        arguments.next().ok_or("--root requires a path")?,
                    ));
                }
                "--help" | "-h" => {
                    println!(
                        "Usage: cargo run --release -p stormbuffer-mcp --example scale -- \\\n                         [--sizes 100,1000,10000] [--samples 20] [--root PATH]"
                    );
                    std::process::exit(0);
                }
                _ => return Err(format!("unknown argument: {argument}")),
            }
        }
        Ok(Self {
            sizes,
            samples,
            root,
        })
    }
}

struct TemporaryRoot {
    path: PathBuf,
    remove_on_drop: bool,
}

impl TemporaryRoot {
    fn new(configured: Option<PathBuf>) -> Result<Self, String> {
        if let Some(path) = configured {
            fs::create_dir_all(&path).map_err(|error| format!("create benchmark root: {error}"))?;
            return Ok(Self {
                path,
                remove_on_drop: false,
            });
        }
        let suffix = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_err(|error| format!("read system time: {error}"))?
            .as_nanos();
        let path =
            std::env::temp_dir().join(format!("stormbuffer-scale-{}-{suffix}", std::process::id()));
        fs::create_dir(&path).map_err(|error| format!("create benchmark root: {error}"))?;
        Ok(Self {
            path,
            remove_on_drop: true,
        })
    }
}

impl Drop for TemporaryRoot {
    fn drop(&mut self) {
        if self.remove_on_drop {
            let _ = fs::remove_dir_all(&self.path);
        }
    }
}

fn main() {
    if let Err(error) = run() {
        eprintln!("scale benchmark failed: {error}");
        std::process::exit(1);
    }
}

fn run() -> Result<(), String> {
    let config = Config::parse()?;
    let root = TemporaryRoot::new(config.root.clone())?;
    let embedder = Arc::new(
        core::DeterministicEmbedder::new("scale-harness-v1", EMBEDDING_DIMENSION)
            .map_err(|error| error.to_string())?,
    );
    let mut stores = Vec::with_capacity(config.sizes.len());
    for size in &config.sizes {
        stores.push(run_store(
            &root.path,
            *size,
            config.samples,
            Arc::clone(&embedder),
        )?);
    }
    let rustc = std::process::Command::new("rustc")
        .arg("--version")
        .output()
        .ok()
        .filter(|output| output.status.success())
        .and_then(|output| String::from_utf8(output.stdout).ok())
        .map(|version| version.trim().to_owned());
    let report = json!({
        "format_version": 1,
        "environment": {
            "os": std::env::consts::OS,
            "arch": std::env::consts::ARCH,
            "rustc": rustc,
            "package_version": env!("CARGO_PKG_VERSION"),
            "release_build": !cfg!(debug_assertions),
            "available_parallelism": std::thread::available_parallelism().map(usize::from).ok(),
        },
        "inputs": {
            "sizes": config.sizes,
            "samples": config.samples,
            "corpus_seed": 703,
            "queries": QUERIES,
            "embedder": {
                "model": "stormbuffer/deterministic",
                "version": "scale-harness-v1",
                "dimension": EMBEDDING_DIMENSION,
                "remote_downloads": false,
            },
        },
        "stores": stores,
    });
    println!(
        "{}",
        serde_json::to_string_pretty(&report).map_err(|error| error.to_string())?
    );
    Ok(())
}

fn run_store(
    root: &Path,
    size: usize,
    samples: usize,
    embedder: Arc<core::DeterministicEmbedder>,
) -> Result<Value, String> {
    let store_root = root.join(format!("records-{size}"));
    if store_root.exists() {
        return Err(format!(
            "benchmark store already exists: {}",
            store_root.display()
        ));
    }
    let paths = core::StorePaths {
        scope: core::StoreScope::Global,
        records: store_root.join("records"),
        cache: store_root.join("cache"),
        root: store_root,
    };
    core::initialize_store(&paths, core::StoreInitMode::Default)
        .map_err(|error| error.to_string())?;
    generate_records(&paths, size)?;

    let cold_reconciliation = measure(1, || {
        core::sync_store(&paths)
            .map(|_| ())
            .map_err(|error| error.to_string())
    })?;
    let warm_reconciliation = measure(samples, || {
        core::sync_store(&paths)
            .map(|_| ())
            .map_err(|error| error.to_string())
    })?;

    let lexical = search_options(core::RetrievalMode::Lexical);
    let vector = search_options(core::RetrievalMode::Semantic);
    let hybrid = search_options(core::RetrievalMode::Hybrid);
    let cold_fts = measure(1, || {
        core::search_store(&paths, QUERIES[0], lexical.clone())
            .map(|_| ())
            .map_err(|error| error.to_string())
    })?;
    let warm_fts = measure_queries(samples, |query| {
        core::search_store(&paths, query, lexical.clone())
            .map(|_| ())
            .map_err(|error| error.to_string())
    })?;

    let vector_build = measure(1, || {
        core::rebuild_vector_index(&paths, embedder.as_ref())
            .map(|_| ())
            .map_err(|error| error.to_string())
    })?;
    let cold_vector = measure(1, || {
        core::search_stores_with_embedder(
            std::slice::from_ref(&paths),
            QUERIES[0],
            vector.clone(),
            embedder.as_ref(),
        )
        .map(|_| ())
        .map_err(|error| error.to_string())
    })?;
    let warm_vector = measure_queries(samples, |query| {
        core::search_stores_with_embedder(
            std::slice::from_ref(&paths),
            query,
            vector.clone(),
            embedder.as_ref(),
        )
        .map(|_| ())
        .map_err(|error| error.to_string())
    })?;
    let cold_hybrid = measure(1, || {
        core::search_stores_with_embedder(
            std::slice::from_ref(&paths),
            QUERIES[0],
            hybrid.clone(),
            embedder.as_ref(),
        )
        .map(|_| ())
        .map_err(|error| error.to_string())
    })?;
    let warm_hybrid = measure_queries(samples, |query| {
        core::search_stores_with_embedder(
            std::slice::from_ref(&paths),
            query,
            hybrid.clone(),
            embedder.as_ref(),
        )
        .map(|_| ())
        .map_err(|error| error.to_string())
    })?;

    let context_options = core::ContextOptions {
        budget: 512,
        search: hybrid,
    };
    let cold_context = measure(1, || {
        core::context_stores_with_embedder(
            std::slice::from_ref(&paths),
            QUERIES[0],
            context_options.clone(),
            embedder.as_ref(),
        )
        .map(|_| ())
        .map_err(|error| error.to_string())
    })?;
    let warm_context = measure_queries(samples, |query| {
        core::context_stores_with_embedder(
            std::slice::from_ref(&paths),
            query,
            context_options.clone(),
            embedder.as_ref(),
        )
        .map(|_| ())
        .map_err(|error| error.to_string())
    })?;

    let mut edit_index = 0;
    let incremental_reindex = measure(samples, || {
        update_record(&paths, edit_index % size)?;
        edit_index += 1;
        core::sync_store(&paths)
            .map(|_| ())
            .map_err(|error| error.to_string())
    })?;
    core::rebuild_vector_index(&paths, embedder.as_ref()).map_err(|error| error.to_string())?;

    let server = McpServer::with_embedder(paths.clone(), false, embedder);
    let warm_mcp_recall = measure_queries(samples, |query| {
        let arguments = serde_json::from_value(json!({"query": query, "budget": 512}))
            .map_err(|error| error.to_string())?;
        let result = server
            .call_sync("context", arguments, false)
            .map_err(|error| error.to_string())?;
        require_mcp_success(&result)
    })?;
    let (index_size_bytes, vector_index_size_bytes) = projection_sizes(&paths)?;

    Ok(json!({
        "record_count": size,
        "index_size_bytes": index_size_bytes,
        "vector_index_size_bytes": vector_index_size_bytes,
        "latency_ms": {
            "reconciliation_cold": cold_reconciliation,
            "reconciliation_warm": warm_reconciliation,
            "fts_cold": cold_fts,
            "fts_warm": warm_fts,
            "vector_index_build": vector_build,
            "vector_cold": cold_vector,
            "vector_warm": warm_vector,
            "hybrid_cold": cold_hybrid,
            "hybrid_warm": warm_hybrid,
            "context_cold": cold_context,
            "context_warm": warm_context,
            "incremental_reindex": incremental_reindex,
            "mcp_recall_warm": warm_mcp_recall,
        }
    }))
}

fn search_options(mode: core::RetrievalMode) -> core::SearchOptions {
    core::SearchOptions {
        limit: 10,
        mode,
        ..core::SearchOptions::default()
    }
}

fn generate_records(paths: &core::StorePaths, count: usize) -> Result<(), String> {
    let kinds = [
        core::RecordKind::Fact,
        core::RecordKind::Decision,
        core::RecordKind::Procedure,
        core::RecordKind::Checkpoint,
    ];
    let topics = ["authentication", "database", "deployment", "release"];
    let details = [
        "The authentication timeout is reviewed before each release and owned by the platform team.",
        "The database migration decision requires a backup, a dry run, and a written rollback point.",
        "The deployment rollback procedure restores the previous artifact and verifies service health.",
        "The release checkpoint owner confirms monitoring, support coverage, and the change window.",
    ];
    for index in 0..count {
        let topic = topics[index % topics.len()];
        let id: core::RecordId = format!("00000000-0000-7000-8000-{index:012x}")
            .parse()
            .map_err(|error: String| error)?;
        let timestamp: core::Timestamp = "2026-01-01T00:00:00Z"
            .parse()
            .map_err(|error: String| error)?;
        let record = core::Record {
            id,
            title: format!("{topic} operational memory {index}"),
            kind: kinds[index % kinds.len()],
            scope: "global".parse().map_err(|error: String| error)?,
            status: core::RecordStatus::Active,
            access: core::Access::Agent,
            created_at: timestamp,
            updated_at: timestamp,
            tags: vec![topic.to_owned(), format!("team-{}", index % 12)],
            aliases: vec![format!("{topic} note {index}")],
            supersedes: Vec::new(),
            sources: vec![core::Source {
                kind: core::SourceKind::Document,
                reference: format!("benchmark/project-{}/runbook.md", index % 24),
                actor: "scale-harness".to_owned(),
                observed_at: None,
                revision: None,
                content_hash: None,
            }],
            body: format!(
                "## Context\n\n{}\n\n## Project note\n\nProject {} revision {} records deterministic operational history and a realistic distractor phrase.",
                details[index % details.len()],
                index % 24,
                index / 24
            ),
        };
        let markdown = core::render_markdown(&record).map_err(|error| error.to_string())?;
        fs::write(paths.records.join(format!("{id}.md")), markdown)
            .map_err(|error| format!("write benchmark record: {error}"))?;
    }
    Ok(())
}

fn update_record(paths: &core::StorePaths, index: usize) -> Result<(), String> {
    let id = format!("00000000-0000-7000-8000-{index:012x}");
    let path = paths.records.join(format!("{id}.md"));
    let markdown =
        fs::read_to_string(&path).map_err(|error| format!("read benchmark record: {error}"))?;
    let mut record = core::parse_markdown(&path, &markdown).map_err(|error| error.to_string())?;
    record.body.push_str("\n\nIncremental benchmark edit.");
    record.updated_at = core::Timestamp::now_utc();
    fs::write(
        &path,
        core::render_markdown(&record).map_err(|error| error.to_string())?,
    )
    .map_err(|error| format!("write benchmark record: {error}"))
}

fn measure<F>(samples: usize, mut operation: F) -> Result<Value, String>
where
    F: FnMut() -> Result<(), String>,
{
    let mut durations = Vec::with_capacity(samples);
    for _ in 0..samples {
        let started = Instant::now();
        operation()?;
        durations.push(started.elapsed());
    }
    Ok(latency_summary(durations))
}

fn measure_queries<F>(samples: usize, mut operation: F) -> Result<Value, String>
where
    F: FnMut(&str) -> Result<(), String>,
{
    let mut index = 0;
    measure(samples, || {
        let result = operation(QUERIES[index % QUERIES.len()]);
        index += 1;
        result
    })
}

fn latency_summary(mut durations: Vec<Duration>) -> Value {
    durations.sort_unstable();
    let percentile = |numerator: usize, denominator: usize| {
        let rank = (durations.len() * numerator).div_ceil(denominator);
        duration_ms(durations[rank.saturating_sub(1)])
    };
    json!({
        "samples": durations.len(),
        "p50": percentile(50, 100),
        "p95": percentile(95, 100),
        "max": duration_ms(*durations.last().expect("positive sample count")),
    })
}

fn duration_ms(duration: Duration) -> f64 {
    duration.as_secs_f64() * 1_000.0
}

fn require_mcp_success(result: &rmcp::model::CallToolResult) -> Result<(), String> {
    if result.is_error == Some(true)
        || result
            .structured_content
            .as_ref()
            .and_then(|envelope| envelope.get("ok"))
            != Some(&Value::Bool(true))
    {
        return Err("MCP recall returned an unsuccessful result".to_owned());
    }
    Ok(())
}

fn projection_sizes(paths: &core::StorePaths) -> Result<(u64, u64), String> {
    let index_path = core::index_path(paths);
    let index_size = [index_path.clone(), index_path.with_extension("sqlite3-wal")]
        .iter()
        .filter_map(|path| fs::metadata(path).ok())
        .map(|metadata| metadata.len())
        .sum();
    let connection = Connection::open(&index_path).map_err(|error| error.to_string())?;
    let table: String = connection
        .query_row(
            "SELECT table_name FROM vector_indexes WHERE active = 1",
            [],
            |row| row.get(0),
        )
        .map_err(|error| error.to_string())?;
    let pattern = format!("{table}_%");
    let vector_size: u64 = connection
        .query_row(
            "SELECT COALESCE(SUM(pgsize), 0) FROM dbstat WHERE name = ?1 OR name LIKE ?2",
            (&table, &pattern),
            |row| row.get(0),
        )
        .map_err(|error| error.to_string())?;
    Ok((index_size, vector_size))
}

#[cfg(test)]
mod tests {
    use super::{latency_summary, require_mcp_success};
    use rmcp::model::CallToolResult;
    use serde_json::json;
    use std::time::Duration;

    #[test]
    fn latency_summary_uses_nearest_rank_percentiles() {
        let summary = latency_summary((1..=20).map(Duration::from_millis).collect::<Vec<_>>());
        assert_eq!(summary["p50"], 10.0);
        assert_eq!(summary["p95"], 19.0);
        assert_eq!(summary["max"], 20.0);
    }

    #[test]
    fn mcp_errors_do_not_count_as_successful_samples() {
        let result = CallToolResult::structured_error(json!({
            "ok": false,
            "error": {"code": "test_error", "message": "test failure"}
        }));

        assert!(require_mcp_success(&result).is_err());
    }
}
