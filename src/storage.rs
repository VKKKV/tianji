use std::collections::BTreeMap;
use std::fs;
use std::io::Write;
use std::ops::Deref;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Condvar, Mutex};

use rusqlite::{params, Connection, OpenFlags, OptionalExtension};

use crate::fetch::{derive_canonical_content_hash, derive_canonical_entry_identity_hash};
use crate::models::{InterventionCandidate, NormalizedEvent, RawItem, RunArtifact, ScoredEvent};
use crate::time_utils::parse_iso_rfc3339_timestamp_seconds;
use crate::utils::round2;
use crate::worldline::baseline::Baseline;
use crate::worldline::types::Worldline;
use crate::TianJiError;

pub const RETENTION_REPORT_SCHEMA_VERSION: &str = "tianji.retention-report.v1";
pub const MAINTENANCE_CHECK_REPORT_SCHEMA_VERSION: &str = "tianji.maintenance-check-report.v1";
pub const BACKUP_REPORT_SCHEMA_VERSION: &str = "tianji.backup-report.v1";
pub const EXPORT_REPORT_SCHEMA_VERSION: &str = "tianji.export-report.v1";
pub const COMPACT_REPORT_SCHEMA_VERSION: &str = "tianji.compact-report.v1";

pub const DEFAULT_RUN_SUMMARY_EVENT_LIMIT: usize = 200;
pub const MAX_RUN_SUMMARY_EVENT_LIMIT: usize = 500;
pub const DEFAULT_RUN_SUMMARY_GROUP_LIMIT: usize = 200;
pub const MAX_RUN_SUMMARY_GROUP_LIMIT: usize = 500;

const RUN_LIST_FILTER_PAGE_SIZE: usize = 100;
pub const DEFAULT_SQLITE_POOL_SIZE: usize = 4;

// ---------------------------------------------------------------------------
// SQLite connection pool
// ---------------------------------------------------------------------------

#[derive(Clone)]
pub struct SqlitePool {
    inner: Arc<SqlitePoolInner>,
}

struct SqlitePoolInner {
    connections: Mutex<Vec<Connection>>,
    condvar: Condvar,
}

pub struct PooledConnection {
    connection: Option<Connection>,
    inner: Arc<SqlitePoolInner>,
}

impl SqlitePool {
    pub fn new(path: impl Into<PathBuf>, max_connections: usize) -> Result<Self, TianJiError> {
        if max_connections == 0 {
            return Err(TianJiError::Usage(
                "SQLite pool size must be greater than zero".to_string(),
            ));
        }

        let path = path.into();
        let mut connections = Vec::with_capacity(max_connections);
        for _ in 0..max_connections {
            connections.push(open_initialized_connection(&path)?);
        }

        Ok(Self {
            inner: Arc::new(SqlitePoolInner {
                connections: Mutex::new(connections),
                condvar: Condvar::new(),
            }),
        })
    }

    pub fn default(path: impl Into<PathBuf>) -> Result<Self, TianJiError> {
        Self::new(path, DEFAULT_SQLITE_POOL_SIZE)
    }

    pub fn get(&self) -> Result<PooledConnection, TianJiError> {
        let mut connections = self
            .inner
            .connections
            .lock()
            .map_err(|_| TianJiError::Usage("SQLite pool lock poisoned".to_string()))?;
        while connections.is_empty() {
            connections = self
                .inner
                .condvar
                .wait(connections)
                .map_err(|_| TianJiError::Usage("SQLite pool lock poisoned".to_string()))?;
        }

        Ok(PooledConnection {
            connection: connections.pop(),
            inner: self.inner.clone(),
        })
    }
}

impl Deref for PooledConnection {
    type Target = Connection;

    fn deref(&self) -> &Self::Target {
        self.connection
            .as_ref()
            .expect("pooled connection missing before drop")
    }
}

impl Drop for PooledConnection {
    fn drop(&mut self) {
        if let Some(connection) = self.connection.take() {
            if let Ok(mut connections) = self.inner.connections.lock() {
                connections.push(connection);
                self.inner.condvar.notify_one();
            }
        }
    }
}

fn open_initialized_connection(path: &Path) -> Result<Connection, TianJiError> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }

    let connection = Connection::open(path)?;
    connection.execute_batch("PRAGMA foreign_keys = ON")?;
    connection.execute_batch("PRAGMA journal_mode = WAL")?;
    initialize_schema(&connection)?;
    Ok(connection)
}

// ---------------------------------------------------------------------------
// Write path
// ---------------------------------------------------------------------------

pub fn persist_run(
    sqlite_path: &str,
    artifact: &RunArtifact,
    raw_items: &[RawItem],
    normalized_events: &[NormalizedEvent],
    scored_events: &[ScoredEvent],
    intervention_candidates: &[InterventionCandidate],
) -> Result<(), TianJiError> {
    let mut connection = open_initialized_connection(Path::new(sqlite_path))?;

    let tx = connection.transaction()?;
    let run_id = insert_run(&tx, artifact)?;
    let canonical_ids = ensure_canonical_source_items(&tx, raw_items)?;
    insert_raw_items(&tx, run_id, raw_items, &canonical_ids)?;
    insert_normalized_events(&tx, run_id, normalized_events, &canonical_ids)?;
    insert_scored_events(&tx, run_id, scored_events)?;
    insert_intervention_candidates(&tx, run_id, intervention_candidates)?;
    tx.commit()?;

    Ok(())
}

// ---------------------------------------------------------------------------
// Retention policy
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, serde::Serialize, PartialEq, Eq)]
pub struct RetentionReport {
    pub schema_version: String,
    pub sqlite_path: String,
    pub keep_last_runs: usize,
    pub runs_before: usize,
    pub runs_after: usize,
    pub deleted_runs: usize,
    pub deleted_source_items: usize,
}

pub fn apply_retention_policy(
    sqlite_path: &str,
    keep_last_runs: usize,
) -> Result<RetentionReport, TianJiError> {
    let mut connection = open_initialized_connection(Path::new(sqlite_path))?;
    let tx = connection.transaction()?;

    let runs_before = count_rows(&tx, "runs")?;
    let deleted_runs = if keep_last_runs >= runs_before {
        0
    } else {
        tx.execute(
            "DELETE FROM runs WHERE id NOT IN (SELECT id FROM runs ORDER BY id DESC LIMIT ?1)",
            params![keep_last_runs as i64],
        )?
    };
    let deleted_source_items = tx.execute(
        "DELETE FROM source_items
         WHERE NOT EXISTS (
             SELECT 1 FROM raw_items WHERE raw_items.canonical_source_item_id = source_items.id
         )
         AND NOT EXISTS (
             SELECT 1 FROM normalized_events WHERE normalized_events.canonical_source_item_id = source_items.id
         )",
        [],
    )?;
    let runs_after = count_rows(&tx, "runs")?;
    tx.commit()?;

    Ok(RetentionReport {
        schema_version: RETENTION_REPORT_SCHEMA_VERSION.to_string(),
        sqlite_path: sqlite_path.to_string(),
        keep_last_runs,
        runs_before,
        runs_after,
        deleted_runs,
        deleted_source_items,
    })
}

// ---------------------------------------------------------------------------
// Maintenance operations
// ---------------------------------------------------------------------------

const HISTORY_TABLES: [&str; 6] = [
    "runs",
    "source_items",
    "raw_items",
    "normalized_events",
    "scored_events",
    "intervention_candidates",
];

#[derive(Debug, Clone, serde::Serialize, PartialEq, Eq)]
pub struct MaintenanceFileSizes {
    pub main_bytes: u64,
    pub wal_bytes: u64,
    pub shm_bytes: u64,
    pub total_bytes: u64,
}

#[derive(Debug, Clone, serde::Serialize, PartialEq)]
pub struct MaintenanceCheckReport {
    pub schema_version: String,
    pub sqlite_path: String,
    pub quick_check: String,
    pub foreign_key_violation_count: usize,
    pub table_counts: BTreeMap<String, usize>,
    pub latest_run_id: Option<i64>,
    pub file_sizes: MaintenanceFileSizes,
    pub page_count: i64,
    pub freelist_count: i64,
    pub journal_mode: String,
}

#[derive(Debug, Clone, serde::Serialize, PartialEq, Eq)]
pub struct BackupReport {
    pub schema_version: String,
    pub source_path: String,
    pub output_path: String,
    pub source_bytes: u64,
    pub output_bytes: u64,
    pub run_count: usize,
}

#[derive(Debug, Clone, serde::Serialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum ExportFormat {
    Json,
    Jsonl,
}

#[derive(Debug, Clone, serde::Serialize, PartialEq, Eq)]
pub struct ExportReport {
    pub schema_version: String,
    pub sqlite_path: String,
    pub output_path: String,
    pub format: ExportFormat,
    pub include_details: bool,
    pub run_count: usize,
    pub bytes: u64,
}

#[derive(Debug, Clone, serde::Serialize, PartialEq, Eq)]
pub struct CheckpointReport {
    pub busy: i64,
    pub log_frames: i64,
    pub checkpointed_frames: i64,
}

#[derive(Debug, Clone, serde::Serialize, PartialEq, Eq)]
pub struct CompactReport {
    pub schema_version: String,
    pub sqlite_path: String,
    pub vacuum: bool,
    pub before_file_sizes: MaintenanceFileSizes,
    pub after_file_sizes: MaintenanceFileSizes,
    pub before_page_count: i64,
    pub after_page_count: i64,
    pub before_freelist_count: i64,
    pub after_freelist_count: i64,
    pub checkpoint: CheckpointReport,
}

pub fn maintenance_check(sqlite_path: &str) -> Result<MaintenanceCheckReport, TianJiError> {
    ensure_existing_sqlite_source(sqlite_path)?;
    let connection = open_existing_read_only_connection(sqlite_path)?;
    let quick_check: String = connection.query_row("PRAGMA quick_check", [], |row| row.get(0))?;
    let foreign_key_violation_count = foreign_key_violation_count(&connection)?;
    let table_counts = history_table_counts(&connection)?;
    let latest_run_id = get_latest_run_id_with_conn(&connection)?;
    let page_count = pragma_i64(&connection, "page_count")?;
    let freelist_count = pragma_i64(&connection, "freelist_count")?;
    let journal_mode = pragma_string(&connection, "journal_mode")?;

    Ok(MaintenanceCheckReport {
        schema_version: MAINTENANCE_CHECK_REPORT_SCHEMA_VERSION.to_string(),
        sqlite_path: sqlite_path.to_string(),
        quick_check,
        foreign_key_violation_count,
        table_counts,
        latest_run_id,
        file_sizes: maintenance_file_sizes(sqlite_path)?,
        page_count,
        freelist_count,
        journal_mode,
    })
}

pub fn backup_sqlite_database(
    sqlite_path: &str,
    output_path: &str,
    overwrite: bool,
) -> Result<BackupReport, TianJiError> {
    ensure_existing_sqlite_source(sqlite_path)?;
    prepare_output_path(sqlite_path, output_path, overwrite)?;
    let connection = open_existing_read_only_connection(sqlite_path)?;
    let run_count = count_table_rows(&connection, "runs")?;
    connection.execute("VACUUM INTO ?1", params![output_path])?;
    let output_connection = open_existing_read_only_connection(output_path)?;
    let output_run_count = count_table_rows(&output_connection, "runs")?;
    if output_run_count != run_count {
        return Err(TianJiError::DataIntegrity(format!(
            "backup run count mismatch: source={run_count} output={output_run_count}"
        )));
    }

    Ok(BackupReport {
        schema_version: BACKUP_REPORT_SCHEMA_VERSION.to_string(),
        source_path: sqlite_path.to_string(),
        output_path: output_path.to_string(),
        source_bytes: maintenance_file_sizes(sqlite_path)?.total_bytes,
        output_bytes: maintenance_file_sizes(output_path)?.total_bytes,
        run_count,
    })
}

pub fn export_run_history(
    sqlite_path: &str,
    output_path: &str,
    format: ExportFormat,
    include_details: bool,
    overwrite: bool,
) -> Result<ExportReport, TianJiError> {
    ensure_existing_sqlite_source(sqlite_path)?;
    prepare_output_path(sqlite_path, output_path, overwrite)?;
    let connection = open_existing_read_only_connection(sqlite_path)?;
    let run_count = count_table_rows(&connection, "runs")?;
    let summaries = list_runs_with_conn(&connection, run_count, &RunListFilters::default())?;
    let runs = if include_details {
        let scored_filters = ScoredEventFilters::default();
        let group_filters = EventGroupFilters::default();
        summaries
            .iter()
            .map(|summary| {
                let run_id = summary
                    .get("run_id")
                    .and_then(|value| value.as_i64())
                    .ok_or_else(|| {
                        TianJiError::DataIntegrity("run summary missing integer run_id".to_string())
                    })?;
                get_run_summary_with_conn(
                    &connection,
                    run_id,
                    &scored_filters,
                    false,
                    &group_filters,
                )?
                .ok_or_else(|| {
                    TianJiError::DataIntegrity(format!("run disappeared during export: {run_id}"))
                })
            })
            .collect::<Result<Vec<_>, _>>()?
    } else {
        summaries
    };
    let metadata = serde_json::json!({
        "schema_version": "tianji.run-history-export.v1",
        "sqlite_path": sqlite_path,
        "format": format,
        "include_details": include_details,
        "run_count": runs.len(),
    });
    write_export_file(output_path, &format, &metadata, &runs)?;
    let bytes = fs::metadata(output_path)?.len();
    Ok(ExportReport {
        schema_version: EXPORT_REPORT_SCHEMA_VERSION.to_string(),
        sqlite_path: sqlite_path.to_string(),
        output_path: output_path.to_string(),
        format,
        include_details,
        run_count: runs.len(),
        bytes,
    })
}

pub fn compact_sqlite_database(
    sqlite_path: &str,
    vacuum: bool,
) -> Result<CompactReport, TianJiError> {
    ensure_existing_sqlite_source(sqlite_path)?;
    let connection = open_existing_read_write_connection(sqlite_path)?;
    let before_file_sizes = maintenance_file_sizes(sqlite_path)?;
    let before_page_count = pragma_i64(&connection, "page_count")?;
    let before_freelist_count = pragma_i64(&connection, "freelist_count")?;
    let checkpoint = connection.query_row("PRAGMA wal_checkpoint(TRUNCATE)", [], |row| {
        Ok(CheckpointReport {
            busy: row.get(0)?,
            log_frames: row.get(1)?,
            checkpointed_frames: row.get(2)?,
        })
    })?;
    if vacuum {
        connection.execute_batch("VACUUM")?;
    }
    let after_page_count = pragma_i64(&connection, "page_count")?;
    let after_freelist_count = pragma_i64(&connection, "freelist_count")?;
    drop(connection);
    let after_file_sizes = maintenance_file_sizes(sqlite_path)?;
    Ok(CompactReport {
        schema_version: COMPACT_REPORT_SCHEMA_VERSION.to_string(),
        sqlite_path: sqlite_path.to_string(),
        vacuum,
        before_file_sizes,
        after_file_sizes,
        before_page_count,
        after_page_count,
        before_freelist_count,
        after_freelist_count,
        checkpoint,
    })
}

fn ensure_existing_sqlite_source(sqlite_path: &str) -> Result<(), TianJiError> {
    let path = Path::new(sqlite_path);
    if !path.exists() {
        return Err(TianJiError::Usage(format!(
            "SQLite database does not exist: {sqlite_path}"
        )));
    }
    if !path.is_file() {
        return Err(TianJiError::Usage(format!(
            "SQLite path is not a file: {sqlite_path}"
        )));
    }
    Ok(())
}

fn open_existing_read_only_connection(sqlite_path: &str) -> Result<Connection, TianJiError> {
    let connection = Connection::open_with_flags(
        sqlite_path,
        OpenFlags::SQLITE_OPEN_READ_ONLY | OpenFlags::SQLITE_OPEN_URI,
    )?;
    connection.execute_batch("PRAGMA foreign_keys = ON")?;
    Ok(connection)
}

fn open_existing_read_write_connection(sqlite_path: &str) -> Result<Connection, TianJiError> {
    let connection = Connection::open_with_flags(
        sqlite_path,
        OpenFlags::SQLITE_OPEN_READ_WRITE | OpenFlags::SQLITE_OPEN_URI,
    )?;
    connection.execute_batch("PRAGMA foreign_keys = ON")?;
    Ok(connection)
}

fn prepare_output_path(
    sqlite_path: &str,
    output_path: &str,
    overwrite: bool,
) -> Result<(), TianJiError> {
    if Path::new(sqlite_path) == Path::new(output_path) {
        return Err(TianJiError::Usage(
            "Output path must differ from --sqlite-path.".to_string(),
        ));
    }
    let output = Path::new(output_path);
    if output.exists() {
        if !overwrite {
            return Err(TianJiError::Usage(format!(
                "Output already exists: {output_path}. Use --overwrite to replace it."
            )));
        }
        fs::remove_file(output)?;
    }
    if let Some(parent) = output
        .parent()
        .filter(|parent| !parent.as_os_str().is_empty())
    {
        fs::create_dir_all(parent)?;
    }
    Ok(())
}

fn maintenance_file_sizes(sqlite_path: &str) -> Result<MaintenanceFileSizes, TianJiError> {
    let main_bytes = file_size_or_zero(Path::new(sqlite_path))?;
    let wal_bytes = file_size_or_zero(Path::new(&format!("{sqlite_path}-wal")))?;
    let shm_bytes = file_size_or_zero(Path::new(&format!("{sqlite_path}-shm")))?;
    Ok(MaintenanceFileSizes {
        main_bytes,
        wal_bytes,
        shm_bytes,
        total_bytes: main_bytes + wal_bytes + shm_bytes,
    })
}

fn file_size_or_zero(path: &Path) -> Result<u64, TianJiError> {
    match fs::metadata(path) {
        Ok(metadata) => Ok(metadata.len()),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(0),
        Err(error) => Err(TianJiError::Io(error)),
    }
}

fn history_table_counts(connection: &Connection) -> Result<BTreeMap<String, usize>, TianJiError> {
    let mut counts = BTreeMap::new();
    for table_name in HISTORY_TABLES {
        counts.insert(
            table_name.to_string(),
            count_table_rows(connection, table_name)?,
        );
    }
    Ok(counts)
}

fn count_table_rows(connection: &Connection, table_name: &str) -> Result<usize, TianJiError> {
    let query = match table_name {
        "runs" => "SELECT COUNT(*) FROM runs",
        "source_items" => "SELECT COUNT(*) FROM source_items",
        "raw_items" => "SELECT COUNT(*) FROM raw_items",
        "normalized_events" => "SELECT COUNT(*) FROM normalized_events",
        "scored_events" => "SELECT COUNT(*) FROM scored_events",
        "intervention_candidates" => "SELECT COUNT(*) FROM intervention_candidates",
        _ => {
            return Err(TianJiError::DataIntegrity(format!(
                "unsupported row-count table: {table_name}"
            )))
        }
    };
    let count: i64 = connection.query_row(query, [], |row| row.get(0))?;
    usize::try_from(count).map_err(|_| {
        TianJiError::DataIntegrity(format!("negative row count returned for {table_name}"))
    })
}

fn foreign_key_violation_count(connection: &Connection) -> Result<usize, TianJiError> {
    let mut stmt = connection.prepare("PRAGMA foreign_key_check")?;
    let mut rows = stmt.query([])?;
    let mut count = 0usize;
    while rows.next()?.is_some() {
        count += 1;
    }
    Ok(count)
}

fn pragma_i64(connection: &Connection, name: &str) -> Result<i64, TianJiError> {
    let query = match name {
        "page_count" => "PRAGMA page_count",
        "freelist_count" => "PRAGMA freelist_count",
        _ => {
            return Err(TianJiError::DataIntegrity(format!(
                "unsupported pragma: {name}"
            )))
        }
    };
    Ok(connection.query_row(query, [], |row| row.get(0))?)
}

fn pragma_string(connection: &Connection, name: &str) -> Result<String, TianJiError> {
    let query = match name {
        "journal_mode" => "PRAGMA journal_mode",
        _ => {
            return Err(TianJiError::DataIntegrity(format!(
                "unsupported pragma: {name}"
            )))
        }
    };
    Ok(connection.query_row(query, [], |row| row.get(0))?)
}

fn write_export_file(
    output_path: &str,
    format: &ExportFormat,
    metadata: &serde_json::Value,
    runs: &[serde_json::Value],
) -> Result<(), TianJiError> {
    let mut file = fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(output_path)?;
    match format {
        ExportFormat::Json => {
            let payload = serde_json::json!({
                "metadata": metadata,
                "runs": runs,
            });
            file.write_all(serde_json::to_string_pretty(&payload)?.as_bytes())?;
            file.write_all(b"\n")?;
        }
        ExportFormat::Jsonl => {
            let metadata_record = serde_json::json!({
                "record_type": "metadata",
                "metadata": metadata,
            });
            writeln!(file, "{}", serde_json::to_string(&metadata_record)?)?;
            for run in runs {
                let run_record = serde_json::json!({
                    "record_type": "run",
                    "run": run,
                });
                writeln!(file, "{}", serde_json::to_string(&run_record)?)?;
            }
        }
    }
    Ok(())
}

fn count_rows(connection: &Connection, table_name: &str) -> Result<usize, TianJiError> {
    let query = match table_name {
        "runs" => "SELECT COUNT(*) FROM runs",
        "source_items" => "SELECT COUNT(*) FROM source_items",
        _ => {
            return Err(TianJiError::DataIntegrity(format!(
                "unsupported retention row-count table: {table_name}"
            )))
        }
    };
    let count: i64 = connection.query_row(query, [], |row| row.get(0))?;
    usize::try_from(count).map_err(|_| {
        TianJiError::DataIntegrity(format!("negative row count returned for {table_name}"))
    })
}

fn insert_run(connection: &Connection, artifact: &RunArtifact) -> Result<i64, TianJiError> {
    let input_summary_json =
        serde_json::to_string(&artifact.input_summary).map_err(TianJiError::Json)?;
    let scenario_summary_json =
        serde_json::to_string(&artifact.scenario_summary).map_err(TianJiError::Json)?;

    connection.execute(
        "INSERT INTO runs (schema_version, mode, generated_at, input_summary_json, scenario_summary_json) VALUES (?1, ?2, ?3, ?4, ?5)",
        params![
            artifact.schema_version,
            artifact.mode,
            artifact.generated_at,
            input_summary_json,
            scenario_summary_json,
        ],
    )?;
    Ok(connection.last_insert_rowid())
}

// ---------------------------------------------------------------------------
// Canonical hash helpers
// ---------------------------------------------------------------------------

/// Returns the effective (identity_hash, content_hash) for a RawItem,
/// deriving from canonical fields when either hash is empty.
fn canonical_hashes_for_raw_item(item: &RawItem) -> (String, String) {
    let identity = if item.entry_identity_hash.is_empty() {
        derive_canonical_entry_identity_hash(item)
    } else {
        item.entry_identity_hash.clone()
    };
    let content = if item.content_hash.is_empty() {
        derive_canonical_content_hash(item)
    } else {
        item.content_hash.clone()
    };
    (identity, content)
}

fn ensure_canonical_source_items(
    connection: &Connection,
    raw_items: &[RawItem],
) -> Result<BTreeMap<(String, String), i64>, TianJiError> {
    let mut canonical_ids: BTreeMap<(String, String), i64> = BTreeMap::new();
    for item in raw_items {
        let (identity_hash, content_hash) = canonical_hashes_for_raw_item(item);
        let key = (identity_hash.clone(), content_hash.clone());
        if canonical_ids.contains_key(&key) {
            continue;
        }
        // Check if already exists
        let mut stmt = connection.prepare(
            "SELECT id FROM source_items WHERE entry_identity_hash = ?1 AND content_hash = ?2",
        )?;
        let mut rows = stmt.query(params![identity_hash, content_hash])?;
        if let Some(row) = rows.next()? {
            let id: i64 = row.get(0)?;
            canonical_ids.insert(key, id);
            continue;
        }
        // Insert new
        connection.execute(
            "INSERT INTO source_items (entry_identity_hash, content_hash, source, title, summary, link, published_at) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
            params![
                identity_hash,
                content_hash,
                item.source,
                item.title,
                item.summary,
                item.link,
                item.published_at,
            ],
        )?;
        canonical_ids.insert(key, connection.last_insert_rowid());
    }
    Ok(canonical_ids)
}

fn insert_raw_items(
    connection: &Connection,
    run_id: i64,
    raw_items: &[RawItem],
    canonical_ids: &BTreeMap<(String, String), i64>,
) -> Result<(), TianJiError> {
    for item in raw_items {
        let key = canonical_hashes_for_raw_item(item);
        let canonical_id = canonical_ids.get(&key).ok_or_else(|| {
            TianJiError::DataIntegrity("missing canonical source item id".to_string())
        })?;
        connection.execute(
            "INSERT INTO raw_items (run_id, canonical_source_item_id, source, title, summary, link, published_at) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
            params![
                run_id,
                canonical_id,
                item.source,
                item.title,
                item.summary,
                item.link,
                item.published_at,
            ],
        )?;
    }
    Ok(())
}

fn insert_normalized_events(
    connection: &Connection,
    run_id: i64,
    normalized_events: &[NormalizedEvent],
    canonical_ids: &BTreeMap<(String, String), i64>,
) -> Result<(), TianJiError> {
    for event in normalized_events {
        if event.entry_identity_hash.is_empty() || event.content_hash.is_empty() {
            return Err(TianJiError::Input(format!(
                "normalized event {} has empty hash fields",
                event.event_id
            )));
        }
        let key = (
            event.entry_identity_hash.clone(),
            event.content_hash.clone(),
        );
        let canonical_id = canonical_ids.get(&key).ok_or_else(|| {
            TianJiError::DataIntegrity(
                "missing canonical source item id for normalized event".to_string(),
            )
        })?;
        let keywords_json = serde_json::to_string(&event.keywords).map_err(TianJiError::Json)?;
        let actors_json = serde_json::to_string(&event.actors).map_err(TianJiError::Json)?;
        let regions_json = serde_json::to_string(&event.regions).map_err(TianJiError::Json)?;
        let field_scores_json =
            serde_json::to_string(&event.field_scores).map_err(TianJiError::Json)?;
        connection.execute(
            "INSERT INTO normalized_events (run_id, canonical_source_item_id, event_id, source, title, summary, link, published_at, keywords_json, actors_json, regions_json, field_scores_json) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12)",
            params![
                run_id,
                canonical_id,
                event.event_id,
                event.source,
                event.title,
                event.summary,
                event.link,
                event.published_at,
                keywords_json,
                actors_json,
                regions_json,
                field_scores_json,
            ],
        )?;
    }
    Ok(())
}

fn insert_scored_events(
    connection: &Connection,
    run_id: i64,
    scored_events: &[ScoredEvent],
) -> Result<(), TianJiError> {
    for event in scored_events {
        let actors_json = serde_json::to_string(&event.actors).map_err(TianJiError::Json)?;
        let regions_json = serde_json::to_string(&event.regions).map_err(TianJiError::Json)?;
        let keywords_json = serde_json::to_string(&event.keywords).map_err(TianJiError::Json)?;
        let rationale_json = serde_json::to_string(&event.rationale).map_err(TianJiError::Json)?;
        connection.execute(
            "INSERT INTO scored_events (run_id, event_id, title, source, link, published_at, actors_json, regions_json, keywords_json, dominant_field, impact_score, field_attraction, divergence_score, rationale_json) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14)",
            params![
                run_id,
                event.event_id,
                event.title,
                event.source,
                event.link,
                event.published_at,
                actors_json,
                regions_json,
                keywords_json,
                event.dominant_field,
                event.impact_score,
                event.field_attraction,
                event.divergence_score,
                rationale_json,
            ],
        )?;
    }
    Ok(())
}

fn insert_intervention_candidates(
    connection: &Connection,
    run_id: i64,
    intervention_candidates: &[InterventionCandidate],
) -> Result<(), TianJiError> {
    for candidate in intervention_candidates {
        connection.execute(
            "INSERT INTO intervention_candidates (run_id, priority, event_id, target, intervention_type, reason, expected_effect) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
            params![
                run_id,
                candidate.priority as i64,
                candidate.event_id,
                candidate.target,
                candidate.intervention_type,
                candidate.reason,
                candidate.expected_effect,
            ],
        )?;
    }
    Ok(())
}

// ---------------------------------------------------------------------------
// Schema
// ---------------------------------------------------------------------------

fn initialize_schema(connection: &Connection) -> Result<(), TianJiError> {
    connection.execute_batch(
        "
        CREATE TABLE IF NOT EXISTS runs (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            schema_version TEXT NOT NULL,
            mode TEXT NOT NULL,
            generated_at TEXT NOT NULL,
            input_summary_json TEXT NOT NULL,
            scenario_summary_json TEXT NOT NULL
        );

        CREATE TABLE IF NOT EXISTS source_items (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            entry_identity_hash TEXT NOT NULL,
            content_hash TEXT NOT NULL,
            source TEXT NOT NULL,
            title TEXT NOT NULL,
            summary TEXT NOT NULL,
            link TEXT NOT NULL,
            published_at TEXT,
            UNIQUE(entry_identity_hash, content_hash)
        );

        CREATE TABLE IF NOT EXISTS raw_items (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            run_id INTEGER NOT NULL,
            canonical_source_item_id INTEGER,
            source TEXT NOT NULL,
            title TEXT NOT NULL,
            summary TEXT NOT NULL,
            link TEXT NOT NULL,
            published_at TEXT,
            FOREIGN KEY (run_id) REFERENCES runs(id) ON DELETE CASCADE,
            FOREIGN KEY (canonical_source_item_id) REFERENCES source_items(id)
        );

        CREATE TABLE IF NOT EXISTS normalized_events (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            run_id INTEGER NOT NULL,
            canonical_source_item_id INTEGER,
            event_id TEXT NOT NULL,
            source TEXT NOT NULL,
            title TEXT NOT NULL,
            summary TEXT NOT NULL,
            link TEXT NOT NULL,
            published_at TEXT,
            keywords_json TEXT NOT NULL,
            actors_json TEXT NOT NULL,
            regions_json TEXT NOT NULL,
            field_scores_json TEXT NOT NULL,
            FOREIGN KEY (run_id) REFERENCES runs(id) ON DELETE CASCADE,
            FOREIGN KEY (canonical_source_item_id) REFERENCES source_items(id)
        );

        CREATE TABLE IF NOT EXISTS scored_events (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            run_id INTEGER NOT NULL,
            event_id TEXT NOT NULL,
            title TEXT NOT NULL,
            source TEXT NOT NULL,
            link TEXT NOT NULL,
            published_at TEXT,
            actors_json TEXT NOT NULL,
            regions_json TEXT NOT NULL,
            keywords_json TEXT NOT NULL,
            dominant_field TEXT NOT NULL,
            impact_score REAL NOT NULL,
            field_attraction REAL NOT NULL,
            divergence_score REAL NOT NULL,
            rationale_json TEXT NOT NULL,
            FOREIGN KEY (run_id) REFERENCES runs(id) ON DELETE CASCADE
        );

        CREATE TABLE IF NOT EXISTS intervention_candidates (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            run_id INTEGER NOT NULL,
            priority INTEGER NOT NULL,
            event_id TEXT NOT NULL,
            target TEXT NOT NULL,
            intervention_type TEXT NOT NULL,
            reason TEXT NOT NULL,
            expected_effect TEXT NOT NULL,
            FOREIGN KEY (run_id) REFERENCES runs(id) ON DELETE CASCADE
        );

        CREATE TABLE IF NOT EXISTS worldlines (
            id INTEGER PRIMARY KEY,
            parent_id INTEGER,
            worldline_json TEXT NOT NULL,
            created_at TEXT NOT NULL
        );

        CREATE TABLE IF NOT EXISTS baselines (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            baseline_json TEXT NOT NULL,
            locked_at TEXT NOT NULL
        );
        ",
    )?;
    ensure_column(
        connection,
        "raw_items",
        "canonical_source_item_id",
        "INTEGER REFERENCES source_items(id)",
    )?;
    ensure_column(
        connection,
        "normalized_events",
        "canonical_source_item_id",
        "INTEGER REFERENCES source_items(id)",
    )?;
    Ok(())
}

fn ensure_column(
    connection: &Connection,
    table_name: &str,
    column_name: &str,
    column_definition: &str,
) -> Result<(), TianJiError> {
    let mut stmt = connection.prepare(&format!("PRAGMA table_info({table_name})"))?;
    let mut rows = stmt.query([])?;
    let mut existing = std::collections::HashSet::new();
    while let Some(row) = rows.next()? {
        let name: String = row.get(1)?;
        existing.insert(name);
    }
    if !existing.contains(column_name) {
        connection.execute(
            &format!("ALTER TABLE {table_name} ADD COLUMN {column_name} {column_definition}"),
            [],
        )?;
    }
    Ok(())
}

// ---------------------------------------------------------------------------
// Read path — filter structs
// ---------------------------------------------------------------------------

#[derive(Clone, Debug, Default)]
pub struct RunListFilters {
    pub mode: Option<String>,
    pub dominant_field: Option<String>,
    pub risk_level: Option<String>,
    pub since: Option<String>,
    pub until: Option<String>,
    pub min_top_impact_score: Option<f64>,
    pub max_top_impact_score: Option<f64>,
    pub min_top_field_attraction: Option<f64>,
    pub max_top_field_attraction: Option<f64>,
    pub min_top_divergence_score: Option<f64>,
    pub max_top_divergence_score: Option<f64>,
    pub top_group_dominant_field: Option<String>,
    pub min_event_group_count: Option<i64>,
    pub max_event_group_count: Option<i64>,
}

#[derive(Clone, Debug, Default)]
pub struct ScoredEventFilters {
    pub dominant_field: Option<String>,
    pub min_impact_score: Option<f64>,
    pub max_impact_score: Option<f64>,
    pub min_field_attraction: Option<f64>,
    pub max_field_attraction: Option<f64>,
    pub min_divergence_score: Option<f64>,
    pub max_divergence_score: Option<f64>,
    pub limit_scored_events: Option<usize>,
}

#[derive(Clone, Debug, Default)]
pub struct EventGroupFilters {
    pub dominant_field: Option<String>,
    pub limit_event_groups: Option<usize>,
}

// ---------------------------------------------------------------------------
// Read path — data structures
// ---------------------------------------------------------------------------

#[derive(Debug, Clone)]
pub struct CompareResult {
    pub left_run_id: i64,
    pub right_run_id: i64,
    pub left: serde_json::Value,
    pub right: serde_json::Value,
    pub diff: serde_json::Value,
}

// ---------------------------------------------------------------------------
// Read path — list_runs
// ---------------------------------------------------------------------------

pub fn list_runs(
    sqlite_path: &str,
    limit: usize,
    filters: &RunListFilters,
) -> Result<Vec<serde_json::Value>, TianJiError> {
    let connection = open_initialized_connection(Path::new(sqlite_path))?;
    list_runs_with_conn(&connection, limit, filters)
}

pub fn list_runs_with_conn(
    connection: &Connection,
    limit: usize,
    filters: &RunListFilters,
) -> Result<Vec<serde_json::Value>, TianJiError> {
    if limit == 0 {
        return Ok(Vec::new());
    }

    let has_filters = has_run_list_filters(filters);
    if !has_filters {
        let run_rows = query_run_list_rows(connection, limit, 0)?;
        return build_run_list_items(connection, &run_rows);
    }

    let mut items: Vec<serde_json::Value> = Vec::new();
    let mut offset = 0usize;
    while items.len() < limit {
        let run_rows = query_run_list_rows(connection, RUN_LIST_FILTER_PAGE_SIZE, offset)?;
        if run_rows.is_empty() {
            break;
        }
        offset += run_rows.len();

        let page_items = build_run_list_items(connection, &run_rows)?;
        items.extend(filter_run_list_items(page_items, filters));

        if run_rows.len() < RUN_LIST_FILTER_PAGE_SIZE {
            break;
        }
    }

    items.truncate(limit);
    Ok(items)
}

type RunListRow = (i64, String, String, String, String, String);

fn query_run_list_rows(
    connection: &Connection,
    limit: usize,
    offset: usize,
) -> Result<Vec<RunListRow>, TianJiError> {
    let mut stmt = connection.prepare(
        "SELECT id, schema_version, mode, generated_at, input_summary_json, scenario_summary_json FROM runs ORDER BY id DESC LIMIT ?1 OFFSET ?2",
    )?;
    let mut rows = stmt.query(params![limit as i64, offset as i64])?;

    let mut run_rows: Vec<RunListRow> = Vec::new();
    while let Some(row) = rows.next()? {
        run_rows.push((
            row.get(0)?,
            row.get(1)?,
            row.get(2)?,
            row.get(3)?,
            row.get(4)?,
            row.get(5)?,
        ));
    }
    Ok(run_rows)
}

fn build_run_list_items(
    connection: &Connection,
    run_rows: &[RunListRow],
) -> Result<Vec<serde_json::Value>, TianJiError> {
    let run_ids: Vec<i64> = run_rows.iter().map(|r| r.0).collect();
    let top_scored_events = get_top_scored_event_summaries(connection, &run_ids)?;

    let mut items: Vec<serde_json::Value> = Vec::new();
    for (run_id, schema_version, mode, generated_at, input_summary_json, scenario_summary_json) in
        run_rows
    {
        let input_summary: serde_json::Value =
            serde_json::from_str(input_summary_json).map_err(TianJiError::Json)?;
        let scenario_summary: serde_json::Value =
            serde_json::from_str(scenario_summary_json).map_err(TianJiError::Json)?;

        let event_groups = scenario_summary
            .get("event_groups")
            .and_then(|v| v.as_array())
            .cloned()
            .unwrap_or_default();
        let top_event_group = event_groups.first();

        let top_se = top_scored_events.get(run_id);

        let raw_item_count = input_summary
            .get("raw_item_count")
            .and_then(|v| v.as_u64())
            .unwrap_or(0) as usize;
        let normalized_event_count = input_summary
            .get("normalized_event_count")
            .and_then(|v| v.as_u64())
            .unwrap_or(0) as usize;

        let item = serde_json::json!({
            "run_id": *run_id,
            "schema_version": schema_version,
            "mode": mode,
            "generated_at": generated_at,
            "raw_item_count": raw_item_count,
            "normalized_event_count": normalized_event_count,
            "dominant_field": scenario_summary.get("dominant_field").and_then(|v| v.as_str()).unwrap_or("uncategorized"),
            "risk_level": scenario_summary.get("risk_level").and_then(|v| v.as_str()).unwrap_or("low"),
            "headline": scenario_summary.get("headline").and_then(|v| v.as_str()).unwrap_or(""),
            "event_group_count": event_groups.len(),
            "top_event_group_headline_event_id": top_event_group.and_then(|g| g.get("headline_event_id")).and_then(|v| v.as_str()),
            "top_event_group_dominant_field": top_event_group.and_then(|g| g.get("dominant_field")).and_then(|v| v.as_str()),
            "top_event_group_member_count": top_event_group.and_then(|g| g.get("member_count")).and_then(|v| v.as_u64()),
            "top_scored_event_id": top_se.and_then(|s| s.get("event_id")).and_then(|v| v.as_str()),
            "top_scored_event_dominant_field": top_se.and_then(|s| s.get("dominant_field")).and_then(|v| v.as_str()),
            "top_impact_score": top_se.and_then(|s| s.get("impact_score")).and_then(|v| v.as_f64()),
            "top_field_attraction": top_se.and_then(|s| s.get("field_attraction")).and_then(|v| v.as_f64()),
            "top_divergence_score": top_se.and_then(|s| s.get("divergence_score")).and_then(|v| v.as_f64()),
        });

        items.push(item);
    }

    Ok(items)
}

fn get_top_scored_event_summaries(
    connection: &Connection,
    run_ids: &[i64],
) -> Result<BTreeMap<i64, serde_json::Value>, TianJiError> {
    if run_ids.is_empty() {
        return Ok(BTreeMap::new());
    }
    let placeholders: Vec<String> = run_ids
        .iter()
        .enumerate()
        .map(|(i, _)| format!("?{}", i + 1))
        .collect();
    let query = format!(
        "SELECT run_id, event_id, dominant_field, impact_score, field_attraction, divergence_score FROM scored_events WHERE run_id IN ({}) ORDER BY run_id ASC, divergence_score DESC, id ASC",
        placeholders.join(", ")
    );
    let params: Vec<&dyn rusqlite::types::ToSql> = run_ids
        .iter()
        .map(|id| id as &dyn rusqlite::types::ToSql)
        .collect();

    let mut stmt = connection.prepare(&query)?;
    let mut rows = stmt.query(params.as_slice())?;

    let mut summaries: BTreeMap<i64, serde_json::Value> = BTreeMap::new();
    while let Some(row) = rows.next()? {
        let run_id: i64 = row.get(0)?;
        if summaries.contains_key(&run_id) {
            continue;
        }
        let event_id: String = row.get(1)?;
        let dominant_field: String = row.get(2)?;
        let impact_score: f64 = row.get(3)?;
        let field_attraction: f64 = row.get(4)?;
        let divergence_score: f64 = row.get(5)?;
        summaries.insert(
            run_id,
            serde_json::json!({
                "event_id": event_id,
                "dominant_field": dominant_field,
                "impact_score": impact_score,
                "field_attraction": field_attraction,
                "divergence_score": divergence_score,
            }),
        );
    }
    Ok(summaries)
}

fn has_run_list_filters(filters: &RunListFilters) -> bool {
    filters.mode.is_some()
        || filters.dominant_field.is_some()
        || filters.risk_level.is_some()
        || filters.since.is_some()
        || filters.until.is_some()
        || filters.min_top_impact_score.is_some()
        || filters.max_top_impact_score.is_some()
        || filters.min_top_field_attraction.is_some()
        || filters.max_top_field_attraction.is_some()
        || filters.min_top_divergence_score.is_some()
        || filters.max_top_divergence_score.is_some()
        || filters.top_group_dominant_field.is_some()
        || filters.min_event_group_count.is_some()
        || filters.max_event_group_count.is_some()
}

// ---------------------------------------------------------------------------
// Read path — get_run_summary (history-show)
// ---------------------------------------------------------------------------

pub fn get_run_summary(
    sqlite_path: &str,
    run_id: i64,
    scored_filters: &ScoredEventFilters,
    only_matching_interventions: bool,
    group_filters: &EventGroupFilters,
) -> Result<Option<serde_json::Value>, TianJiError> {
    let connection = open_initialized_connection(Path::new(sqlite_path))?;
    get_run_summary_with_conn(
        &connection,
        run_id,
        scored_filters,
        only_matching_interventions,
        group_filters,
    )
}

pub fn get_run_summary_with_conn(
    connection: &Connection,
    run_id: i64,
    scored_filters: &ScoredEventFilters,
    only_matching_interventions: bool,
    group_filters: &EventGroupFilters,
) -> Result<Option<serde_json::Value>, TianJiError> {
    let run_row: (i64, String, String, String, String, String) = match connection.query_row(
        "SELECT id, schema_version, mode, generated_at, input_summary_json, scenario_summary_json FROM runs WHERE id = ?1",
        params![run_id],
        |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?, row.get(4)?, row.get(5)?)),
    ) {
        Ok(r) => r,
        Err(rusqlite::Error::QueryReturnedNoRows) => return Ok(None),
        Err(e) => return Err(TianJiError::Storage(e)),
    };
    let (_, schema_version, mode, generated_at, input_summary_json, scenario_summary_json) =
        run_row;

    let input_summary: serde_json::Value =
        serde_json::from_str(&input_summary_json).map_err(TianJiError::Json)?;
    let mut scenario_summary: serde_json::Value =
        serde_json::from_str(&scenario_summary_json).map_err(TianJiError::Json)?;
    let scored_filters = bounded_scored_filters(scored_filters);
    let group_filters = bounded_group_filters(group_filters);

    // Filter event groups
    if let Some(groups) = scenario_summary
        .get_mut("event_groups")
        .and_then(|v| v.as_array_mut())
    {
        if let Some(ref df) = group_filters.dominant_field {
            groups
                .retain(|g| g.get("dominant_field").and_then(|v| v.as_str()) == Some(df.as_str()));
        }
        if let Some(limit) = group_filters.limit_event_groups {
            groups.truncate(limit);
        }
    }

    // Fetch scored events
    let event_query_limit = if scored_filters_have_predicates(&scored_filters) {
        None
    } else {
        scored_filters.limit_scored_events
    };
    let sql = match event_query_limit {
        Some(_) => "SELECT event_id, title, source, link, published_at, actors_json, regions_json, keywords_json, dominant_field, impact_score, field_attraction, divergence_score, rationale_json FROM scored_events WHERE run_id = ?1 ORDER BY divergence_score DESC, id ASC LIMIT ?2",
        None => "SELECT event_id, title, source, link, published_at, actors_json, regions_json, keywords_json, dominant_field, impact_score, field_attraction, divergence_score, rationale_json FROM scored_events WHERE run_id = ?1 ORDER BY divergence_score DESC, id ASC",
    };
    let mut stmt = connection.prepare(sql)?;
    let mut rows = match event_query_limit {
        Some(limit) => stmt.query(params![run_id, limit as i64])?,
        None => stmt.query(params![run_id])?,
    };
    let mut scored_events: Vec<serde_json::Value> = Vec::new();
    while let Some(row) = rows.next()? {
        let event_id: String = row.get(0)?;
        let title: String = row.get(1)?;
        let source: String = row.get(2)?;
        let link: String = row.get(3)?;
        let published_at: Option<String> = row.get(4)?;
        let actors_json: String = row.get(5)?;
        let regions_json: String = row.get(6)?;
        let keywords_json: String = row.get(7)?;
        let dominant_field: String = row.get(8)?;
        let impact_score: f64 = row.get(9)?;
        let field_attraction: f64 = row.get(10)?;
        let divergence_score: f64 = row.get(11)?;
        let rationale_json: String = row.get(12)?;
        let actors: serde_json::Value =
            serde_json::from_str(&actors_json).map_err(TianJiError::Json)?;
        let regions: serde_json::Value =
            serde_json::from_str(&regions_json).map_err(TianJiError::Json)?;
        let keywords: serde_json::Value =
            serde_json::from_str(&keywords_json).map_err(TianJiError::Json)?;
        let rationale: serde_json::Value =
            serde_json::from_str(&rationale_json).map_err(TianJiError::Json)?;
        scored_events.push(serde_json::json!({
            "event_id": event_id,
            "title": title,
            "source": source,
            "link": link,
            "published_at": published_at,
            "actors": actors,
            "regions": regions,
            "keywords": keywords,
            "dominant_field": dominant_field,
            "impact_score": impact_score,
            "field_attraction": field_attraction,
            "divergence_score": divergence_score,
            "rationale": rationale,
        }));
    }

    // Apply scored event filters
    scored_events = filter_scored_event_details(scored_events, &scored_filters);

    // Fetch intervention candidates
    let mut stmt = connection.prepare(
        "SELECT priority, event_id, target, intervention_type, reason, expected_effect FROM intervention_candidates WHERE run_id = ?1 ORDER BY priority ASC, id ASC",
    )?;
    let mut rows = stmt.query(params![run_id])?;
    let mut interventions: Vec<serde_json::Value> = Vec::new();
    while let Some(row) = rows.next()? {
        let priority: i64 = row.get(0)?;
        let event_id: String = row.get(1)?;
        let target: String = row.get(2)?;
        let intervention_type: String = row.get(3)?;
        let reason: String = row.get(4)?;
        let expected_effect: String = row.get(5)?;
        interventions.push(serde_json::json!({
            "priority": priority,
            "event_id": event_id,
            "target": target,
            "intervention_type": intervention_type,
            "reason": reason,
            "expected_effect": expected_effect,
        }));
    }

    // Filter interventions if needed
    if only_matching_interventions {
        let visible_ids: std::collections::HashSet<String> = scored_events
            .iter()
            .filter_map(|e| e.get("event_id").and_then(|v| v.as_str()).map(String::from))
            .collect();
        interventions.retain(|c| {
            c.get("event_id")
                .and_then(|v| v.as_str())
                .is_some_and(|id| visible_ids.contains(id))
        });
    }

    Ok(Some(serde_json::json!({
        "run_id": run_id,
        "schema_version": schema_version,
        "mode": mode,
        "generated_at": generated_at,
        "input_summary": input_summary,
        "scenario_summary": scenario_summary,
        "scored_events": scored_events,
        "intervention_candidates": interventions,
    })))
}

// ---------------------------------------------------------------------------
// Read path — navigation helpers
// ---------------------------------------------------------------------------

pub fn get_latest_run_id(sqlite_path: &str) -> Result<Option<i64>, TianJiError> {
    let connection = open_initialized_connection(Path::new(sqlite_path))?;
    get_latest_run_id_with_conn(&connection)
}

pub fn get_latest_run_id_with_conn(connection: &Connection) -> Result<Option<i64>, TianJiError> {
    match connection.query_row("SELECT id FROM runs ORDER BY id DESC LIMIT 1", [], |row| {
        row.get(0)
    }) {
        Ok(id) => Ok(Some(id)),
        Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
        Err(e) => Err(TianJiError::Storage(e)),
    }
}

pub fn get_latest_run_pair(sqlite_path: &str) -> Result<Option<(i64, i64)>, TianJiError> {
    let connection = Connection::open(sqlite_path)?;
    let mut stmt = connection.prepare("SELECT id FROM runs ORDER BY id DESC LIMIT 2")?;
    let mut rows = stmt.query([])?;
    let mut ids: Vec<i64> = Vec::new();
    while let Some(row) = rows.next()? {
        ids.push(row.get(0)?);
    }
    if ids.len() < 2 {
        return Ok(None);
    }
    // older, newer (same as Python: returns (older, newer))
    Ok(Some((ids[1], ids[0])))
}

pub fn get_previous_run_id(sqlite_path: &str, run_id: i64) -> Result<Option<i64>, TianJiError> {
    let connection = Connection::open(sqlite_path)?;
    match connection.query_row(
        "SELECT id FROM runs WHERE id < ?1 ORDER BY id DESC LIMIT 1",
        params![run_id],
        |row| row.get(0),
    ) {
        Ok(id) => Ok(Some(id)),
        Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
        Err(e) => Err(TianJiError::Storage(e)),
    }
}

pub fn get_next_run_id(sqlite_path: &str, run_id: i64) -> Result<Option<i64>, TianJiError> {
    let connection = Connection::open(sqlite_path)?;
    match connection.query_row(
        "SELECT id FROM runs WHERE id > ?1 ORDER BY id ASC LIMIT 1",
        params![run_id],
        |row| row.get(0),
    ) {
        Ok(id) => Ok(Some(id)),
        Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
        Err(e) => Err(TianJiError::Storage(e)),
    }
}

// ---------------------------------------------------------------------------
// Read path — compare
// ---------------------------------------------------------------------------

pub fn compare_runs(
    sqlite_path: &str,
    left_run_id: i64,
    right_run_id: i64,
    scored_filters: &ScoredEventFilters,
    only_matching_interventions: bool,
    group_filters: &EventGroupFilters,
) -> Result<Option<CompareResult>, TianJiError> {
    let connection = open_initialized_connection(Path::new(sqlite_path))?;
    compare_runs_with_conn(
        &connection,
        left_run_id,
        right_run_id,
        scored_filters,
        only_matching_interventions,
        group_filters,
    )
}

pub fn compare_runs_with_conn(
    connection: &Connection,
    left_run_id: i64,
    right_run_id: i64,
    scored_filters: &ScoredEventFilters,
    only_matching_interventions: bool,
    group_filters: &EventGroupFilters,
) -> Result<Option<CompareResult>, TianJiError> {
    let left = get_run_summary_with_conn(
        connection,
        left_run_id,
        scored_filters,
        only_matching_interventions,
        group_filters,
    )?;
    let right = get_run_summary_with_conn(
        connection,
        right_run_id,
        scored_filters,
        only_matching_interventions,
        group_filters,
    )?;

    match (left, right) {
        (Some(l), Some(r)) => {
            let left_summary = build_compare_side(&l);
            let right_summary = build_compare_side(&r);
            let diff = build_compare_diff(&left_summary, &right_summary);
            Ok(Some(CompareResult {
                left_run_id,
                right_run_id,
                left: left_summary,
                right: right_summary,
                diff,
            }))
        }
        _ => Ok(None),
    }
}

fn build_compare_side(run_payload: &serde_json::Value) -> serde_json::Value {
    let input_summary = &run_payload["input_summary"];
    let scenario_summary = &run_payload["scenario_summary"];
    let event_groups = scenario_summary
        .get("event_groups")
        .and_then(|v| v.as_array())
        .cloned()
        .unwrap_or_default();
    let scored_events = run_payload
        .get("scored_events")
        .and_then(|v| v.as_array())
        .cloned()
        .unwrap_or_default();
    let intervention_candidates = run_payload
        .get("intervention_candidates")
        .and_then(|v| v.as_array())
        .cloned()
        .unwrap_or_default();

    let top_event_group = event_groups.first();
    let top_scored_event = scored_events.first();
    let top_intervention = intervention_candidates.first();

    let event_group_headline_event_ids: Vec<serde_json::Value> = event_groups
        .iter()
        .filter_map(|g| g.get("headline_event_id").cloned())
        .collect();

    let intervention_event_ids: Vec<serde_json::Value> = intervention_candidates
        .iter()
        .filter_map(|c| c.get("event_id").cloned())
        .collect();

    serde_json::json!({
        "run_id": run_payload["run_id"],
        "schema_version": run_payload["schema_version"],
        "mode": run_payload["mode"],
        "raw_item_count": input_summary.get("raw_item_count").and_then(|v| v.as_u64()).unwrap_or(0),
        "normalized_event_count": input_summary.get("normalized_event_count").and_then(|v| v.as_u64()).unwrap_or(0),
        "dominant_field": scenario_summary.get("dominant_field").and_then(|v| v.as_str()).unwrap_or("uncategorized"),
        "risk_level": scenario_summary.get("risk_level").and_then(|v| v.as_str()).unwrap_or("low"),
        "headline": scenario_summary.get("headline").and_then(|v| v.as_str()).unwrap_or(""),
        "event_group_count": event_groups.len(),
        "event_group_headline_event_ids": event_group_headline_event_ids,
        "top_event_group": top_event_group,
        "top_scored_event": top_scored_event,
        "top_intervention": top_intervention,
        "intervention_event_ids": intervention_event_ids,
    })
}

fn build_compare_diff(left: &serde_json::Value, right: &serde_json::Value) -> serde_json::Value {
    let left_intervention_ids: Vec<String> = left["intervention_event_ids"]
        .as_array()
        .map(|a| {
            a.iter()
                .filter_map(|v| v.as_str().map(String::from))
                .collect()
        })
        .unwrap_or_default();
    let right_intervention_ids: Vec<String> = right["intervention_event_ids"]
        .as_array()
        .map(|a| {
            a.iter()
                .filter_map(|v| v.as_str().map(String::from))
                .collect()
        })
        .unwrap_or_default();

    let left_top_se = left.get("top_scored_event");
    let right_top_se = right.get("top_scored_event");

    let left_top_eg = left.get("top_event_group");
    let right_top_eg = right.get("top_event_group");

    let left_top_intervention = left.get("top_intervention");
    let right_top_intervention = right.get("top_intervention");

    let left_top_eg_headline_id = left_top_eg
        .and_then(|g| g.get("headline_event_id"))
        .and_then(|v| v.as_str());
    let right_top_eg_headline_id = right_top_eg
        .and_then(|g| g.get("headline_event_id"))
        .and_then(|v| v.as_str());

    let left_top_se_id = left_top_se
        .and_then(|e| e.get("event_id"))
        .and_then(|v| v.as_str());
    let right_top_se_id = right_top_se
        .and_then(|e| e.get("event_id"))
        .and_then(|v| v.as_str());

    let left_top_impact = get_top_score_metric(left_top_se, "impact_score");
    let right_top_impact = get_top_score_metric(right_top_se, "impact_score");
    let left_top_fa = get_top_score_metric(left_top_se, "field_attraction");
    let right_top_fa = get_top_score_metric(right_top_se, "field_attraction");
    let left_top_ds = get_top_score_metric(left_top_se, "divergence_score");
    let right_top_ds = get_top_score_metric(right_top_se, "divergence_score");

    let left_top_intervention_event_id = left_top_intervention
        .and_then(|i| i.get("event_id"))
        .and_then(|v| v.as_str());
    let right_top_intervention_event_id = right_top_intervention
        .and_then(|i| i.get("event_id"))
        .and_then(|v| v.as_str());

    let left_eg_headline_ids: Vec<String> = left["event_group_headline_event_ids"]
        .as_array()
        .map(|a| {
            a.iter()
                .filter_map(|v| v.as_str().map(String::from))
                .collect()
        })
        .unwrap_or_default();
    let right_eg_headline_ids: Vec<String> = right["event_group_headline_event_ids"]
        .as_array()
        .map(|a| {
            a.iter()
                .filter_map(|v| v.as_str().map(String::from))
                .collect()
        })
        .unwrap_or_default();

    let right_ri: i64 = right["raw_item_count"].as_u64().unwrap_or(0) as i64;
    let left_ri: i64 = left["raw_item_count"].as_u64().unwrap_or(0) as i64;
    let right_ne: i64 = right["normalized_event_count"].as_u64().unwrap_or(0) as i64;
    let left_ne: i64 = left["normalized_event_count"].as_u64().unwrap_or(0) as i64;
    let right_egc: i64 = right["event_group_count"].as_u64().unwrap_or(0) as i64;
    let left_egc: i64 = left["event_group_count"].as_u64().unwrap_or(0) as i64;

    let left_only_eg_ids: Vec<&str> = left_eg_headline_ids
        .iter()
        .filter(|id| !right_eg_headline_ids.contains(id))
        .map(|s| s.as_str())
        .collect();
    let right_only_eg_ids: Vec<&str> = right_eg_headline_ids
        .iter()
        .filter(|id| !left_eg_headline_ids.contains(id))
        .map(|s| s.as_str())
        .collect();

    let left_only_int_ids: Vec<&str> = left_intervention_ids
        .iter()
        .filter(|id| !right_intervention_ids.contains(id))
        .map(|s| s.as_str())
        .collect();
    let right_only_int_ids: Vec<&str> = right_intervention_ids
        .iter()
        .filter(|id| !left_intervention_ids.contains(id))
        .map(|s| s.as_str())
        .collect();

    serde_json::json!({
        "raw_item_count_delta": right_ri - left_ri,
        "normalized_event_count_delta": right_ne - left_ne,
        "event_group_count_delta": right_egc - left_egc,
        "dominant_field_changed": left["dominant_field"] != right["dominant_field"],
        "risk_level_changed": left["risk_level"] != right["risk_level"],
        "top_event_group_changed": left_top_eg_headline_id != right_top_eg_headline_id,
        "left_top_event_group_headline_event_id": left_top_eg_headline_id,
        "right_top_event_group_headline_event_id": right_top_eg_headline_id,
        "top_scored_event_changed": left_top_se_id != right_top_se_id,
        "top_scored_event_comparable": left_top_se_id.is_some() && right_top_se_id.is_some() && left_top_se_id == right_top_se_id,
        "top_intervention_changed": left_top_intervention_event_id != right_top_intervention_event_id,
        "left_top_scored_event_id": left_top_se_id,
        "right_top_scored_event_id": right_top_se_id,
        "left_top_impact_score": left_top_impact,
        "right_top_impact_score": right_top_impact,
        "top_impact_score_delta": build_score_delta(left_top_impact, right_top_impact),
        "left_top_field_attraction": left_top_fa,
        "right_top_field_attraction": right_top_fa,
        "top_field_attraction_delta": build_score_delta(left_top_fa, right_top_fa),
        "left_top_divergence_score": left_top_ds,
        "right_top_divergence_score": right_top_ds,
        "top_divergence_score_delta": build_score_delta(left_top_ds, right_top_ds),
        "left_top_intervention_event_id": left_top_intervention_event_id,
        "right_top_intervention_event_id": right_top_intervention_event_id,
        "left_only_event_group_headline_event_ids": left_only_eg_ids,
        "right_only_event_group_headline_event_ids": right_only_eg_ids,
        "top_event_group_evidence_diff": build_top_event_group_evidence_diff(left_top_eg, right_top_eg),
        "left_only_intervention_event_ids": left_only_int_ids,
        "right_only_intervention_event_ids": right_only_int_ids,
    })
}

fn get_top_score_metric(top_se: Option<&serde_json::Value>, metric: &str) -> Option<f64> {
    top_se?.get(metric)?.as_f64()
}

fn build_score_delta(left: Option<f64>, right: Option<f64>) -> Option<f64> {
    let l = left?;
    let r = right?;
    Some(round2(r - l))
}

fn build_top_event_group_evidence_diff(
    left_top_eg: Option<&serde_json::Value>,
    right_top_eg: Option<&serde_json::Value>,
) -> serde_json::Value {
    let left_member_ids: Vec<String> = left_top_eg
        .and_then(|g| g.get("member_event_ids"))
        .and_then(|v| v.as_array())
        .map(|a| {
            let mut ids: Vec<String> = a
                .iter()
                .filter_map(|v| v.as_str().map(String::from))
                .collect();
            ids.sort();
            ids
        })
        .unwrap_or_default();

    let right_member_ids: Vec<String> = right_top_eg
        .and_then(|g| g.get("member_event_ids"))
        .and_then(|v| v.as_array())
        .map(|a| {
            let mut ids: Vec<String> = a
                .iter()
                .filter_map(|v| v.as_str().map(String::from))
                .collect();
            ids.sort();
            ids
        })
        .unwrap_or_default();

    let left_shared_keywords: Vec<String> = left_top_eg
        .and_then(|g| g.get("shared_keywords"))
        .and_then(|v| v.as_array())
        .map(|a| {
            let mut kws: Vec<String> = a
                .iter()
                .filter_map(|v| v.as_str().map(String::from))
                .collect();
            kws.sort();
            kws
        })
        .unwrap_or_default();

    let right_shared_keywords: Vec<String> = right_top_eg
        .and_then(|g| g.get("shared_keywords"))
        .and_then(|v| v.as_array())
        .map(|a| {
            let mut kws: Vec<String> = a
                .iter()
                .filter_map(|v| v.as_str().map(String::from))
                .collect();
            kws.sort();
            kws
        })
        .unwrap_or_default();

    let left_shared_actors: Vec<String> = left_top_eg
        .and_then(|g| g.get("shared_actors"))
        .and_then(|v| v.as_array())
        .map(|a| {
            let mut actors: Vec<String> = a
                .iter()
                .filter_map(|v| v.as_str().map(String::from))
                .collect();
            actors.sort();
            actors
        })
        .unwrap_or_default();

    let right_shared_actors: Vec<String> = right_top_eg
        .and_then(|g| g.get("shared_actors"))
        .and_then(|v| v.as_array())
        .map(|a| {
            let mut actors: Vec<String> = a
                .iter()
                .filter_map(|v| v.as_str().map(String::from))
                .collect();
            actors.sort();
            actors
        })
        .unwrap_or_default();

    let left_shared_regions: Vec<String> = left_top_eg
        .and_then(|g| g.get("shared_regions"))
        .and_then(|v| v.as_array())
        .map(|a| {
            let mut regions: Vec<String> = a
                .iter()
                .filter_map(|v| v.as_str().map(String::from))
                .collect();
            regions.sort();
            regions
        })
        .unwrap_or_default();

    let right_shared_regions: Vec<String> = right_top_eg
        .and_then(|g| g.get("shared_regions"))
        .and_then(|v| v.as_array())
        .map(|a| {
            let mut regions: Vec<String> = a
                .iter()
                .filter_map(|v| v.as_str().map(String::from))
                .collect();
            regions.sort();
            regions
        })
        .unwrap_or_default();

    let left_chain_summary: Option<String> = left_top_eg
        .and_then(|g| g.get("chain_summary"))
        .and_then(|v| v.as_str().map(String::from))
        .filter(|s| !s.is_empty());

    let right_chain_summary: Option<String> = right_top_eg
        .and_then(|g| g.get("chain_summary"))
        .and_then(|v| v.as_str().map(String::from))
        .filter(|s| !s.is_empty());

    let left_evidence_links: Vec<String> = left_top_eg
        .and_then(|g| g.get("evidence_chain"))
        .and_then(|v| v.as_array())
        .map(|a| {
            let mut links: Vec<String> = a.iter().map(format_evidence_chain_link).collect();
            links.sort();
            links
        })
        .unwrap_or_default();

    let right_evidence_links: Vec<String> = right_top_eg
        .and_then(|g| g.get("evidence_chain"))
        .and_then(|v| v.as_array())
        .map(|a| {
            let mut links: Vec<String> = a.iter().map(format_evidence_chain_link).collect();
            links.sort();
            links
        })
        .unwrap_or_default();

    let left_headline_id: Option<&str> = left_top_eg
        .and_then(|g| g.get("headline_event_id"))
        .and_then(|v| v.as_str());
    let right_headline_id: Option<&str> = right_top_eg
        .and_then(|g| g.get("headline_event_id"))
        .and_then(|v| v.as_str());

    let left_only_member_ids: Vec<&str> = left_member_ids
        .iter()
        .filter(|id| !right_member_ids.contains(id))
        .map(|s| s.as_str())
        .collect();
    let right_only_member_ids: Vec<&str> = right_member_ids
        .iter()
        .filter(|id| !left_member_ids.contains(id))
        .map(|s| s.as_str())
        .collect();

    let kw_added: Vec<&str> = right_shared_keywords
        .iter()
        .filter(|kw| !left_shared_keywords.contains(kw))
        .map(|s| s.as_str())
        .collect();
    let kw_removed: Vec<&str> = left_shared_keywords
        .iter()
        .filter(|kw| !right_shared_keywords.contains(kw))
        .map(|s| s.as_str())
        .collect();

    let actors_added: Vec<&str> = right_shared_actors
        .iter()
        .filter(|a| !left_shared_actors.contains(a))
        .map(|s| s.as_str())
        .collect();
    let actors_removed: Vec<&str> = left_shared_actors
        .iter()
        .filter(|a| !right_shared_actors.contains(a))
        .map(|s| s.as_str())
        .collect();

    let regions_added: Vec<&str> = right_shared_regions
        .iter()
        .filter(|r| !left_shared_regions.contains(r))
        .map(|s| s.as_str())
        .collect();
    let regions_removed: Vec<&str> = left_shared_regions
        .iter()
        .filter(|r| !right_shared_regions.contains(r))
        .map(|s| s.as_str())
        .collect();

    let links_added: Vec<&str> = right_evidence_links
        .iter()
        .filter(|l| !left_evidence_links.contains(l))
        .map(|s| s.as_str())
        .collect();
    let links_removed: Vec<&str> = left_evidence_links
        .iter()
        .filter(|l| !right_evidence_links.contains(l))
        .map(|s| s.as_str())
        .collect();

    serde_json::json!({
        "comparable": left_headline_id.is_some() && right_headline_id.is_some() && left_headline_id == right_headline_id,
        "same_headline_event_id": left_headline_id == right_headline_id,
        "member_count_delta": right_member_ids.len() as i64 - left_member_ids.len() as i64,
        "left_member_event_ids": left_member_ids,
        "right_member_event_ids": right_member_ids,
        "left_only_member_event_ids": left_only_member_ids,
        "right_only_member_event_ids": right_only_member_ids,
        "shared_keywords_added": kw_added,
        "shared_keywords_removed": kw_removed,
        "shared_actors_added": actors_added,
        "shared_actors_removed": actors_removed,
        "shared_regions_added": regions_added,
        "shared_regions_removed": regions_removed,
        "evidence_chain_link_count_delta": right_evidence_links.len() as i64 - left_evidence_links.len() as i64,
        "left_evidence_chain_links": left_evidence_links,
        "right_evidence_chain_links": right_evidence_links,
        "evidence_chain_links_added": links_added,
        "evidence_chain_links_removed": links_removed,
        "chain_summary_changed": left_chain_summary != right_chain_summary,
        "left_chain_summary": left_chain_summary,
        "right_chain_summary": right_chain_summary,
    })
}

fn format_evidence_chain_link(link: &serde_json::Value) -> String {
    let from_id = link
        .get("from_event_id")
        .and_then(|v| v.as_str())
        .unwrap_or("?");
    let to_id = link
        .get("to_event_id")
        .and_then(|v| v.as_str())
        .unwrap_or("?");

    let shared_keywords: Vec<String> = link
        .get("shared_keywords")
        .and_then(|v| v.as_array())
        .map(|a| {
            let mut kws: Vec<String> = a
                .iter()
                .filter_map(|v| v.as_str().map(String::from))
                .collect();
            kws.sort();
            kws
        })
        .unwrap_or_default();

    let shared_actors: Vec<String> = link
        .get("shared_actors")
        .and_then(|v| v.as_array())
        .map(|a| {
            let mut actors: Vec<String> = a
                .iter()
                .filter_map(|v| v.as_str().map(String::from))
                .collect();
            actors.sort();
            actors
        })
        .unwrap_or_default();

    let shared_regions: Vec<String> = link
        .get("shared_regions")
        .and_then(|v| v.as_array())
        .map(|a| {
            let mut regions: Vec<String> = a
                .iter()
                .filter_map(|v| v.as_str().map(String::from))
                .collect();
            regions.sort();
            regions
        })
        .unwrap_or_default();

    let time_delta_hours = link.get("time_delta_hours").and_then(|v| v.as_f64());
    let time_delta_text = match time_delta_hours {
        Some(h) => {
            let s = format!("{h}");
            // Match Python: strip trailing zeros then trailing dot
            let s = s.trim_end_matches('0');
            let s = s.trim_end_matches('.');
            s.to_string()
        }
        None => "unknown".to_string(),
    };

    format!(
        "{from_id}->{to_id}|keywords={}|actors={}|regions={}|delta_h={time_delta_text}",
        shared_keywords.join(","),
        shared_actors.join(","),
        shared_regions.join(","),
    )
}

// ---------------------------------------------------------------------------
// Filter functions (matching Python storage_filters.py)
// ---------------------------------------------------------------------------

fn filter_scored_event_details(
    events: Vec<serde_json::Value>,
    filters: &ScoredEventFilters,
) -> Vec<serde_json::Value> {
    let mut filtered = events;
    if let Some(ref df) = filters.dominant_field {
        filtered.retain(|e| {
            e.get("dominant_field")
                .and_then(|v| v.as_str())
                .is_some_and(|d| d == df)
        });
    }
    if let Some(min) = filters.min_impact_score {
        filtered.retain(|e| is_numeric_at_or_above(&e["impact_score"], min));
    }
    if let Some(max) = filters.max_impact_score {
        filtered.retain(|e| is_numeric_at_or_below(&e["impact_score"], max));
    }
    if let Some(min) = filters.min_field_attraction {
        filtered.retain(|e| is_numeric_at_or_above(&e["field_attraction"], min));
    }
    if let Some(max) = filters.max_field_attraction {
        filtered.retain(|e| is_numeric_at_or_below(&e["field_attraction"], max));
    }
    if let Some(min) = filters.min_divergence_score {
        filtered.retain(|e| is_numeric_at_or_above(&e["divergence_score"], min));
    }
    if let Some(max) = filters.max_divergence_score {
        filtered.retain(|e| is_numeric_at_or_below(&e["divergence_score"], max));
    }
    if let Some(limit) = filters.limit_scored_events {
        filtered.truncate(limit);
    }
    filtered
}

fn bounded_scored_filters(filters: &ScoredEventFilters) -> ScoredEventFilters {
    ScoredEventFilters {
        dominant_field: filters.dominant_field.clone(),
        min_impact_score: filters.min_impact_score,
        max_impact_score: filters.max_impact_score,
        min_field_attraction: filters.min_field_attraction,
        max_field_attraction: filters.max_field_attraction,
        min_divergence_score: filters.min_divergence_score,
        max_divergence_score: filters.max_divergence_score,
        limit_scored_events: filters
            .limit_scored_events
            .map(|limit| limit.min(MAX_RUN_SUMMARY_EVENT_LIMIT)),
    }
}

fn scored_filters_have_predicates(filters: &ScoredEventFilters) -> bool {
    filters.dominant_field.is_some()
        || filters.min_impact_score.is_some()
        || filters.max_impact_score.is_some()
        || filters.min_field_attraction.is_some()
        || filters.max_field_attraction.is_some()
        || filters.min_divergence_score.is_some()
        || filters.max_divergence_score.is_some()
}

fn bounded_group_filters(filters: &EventGroupFilters) -> EventGroupFilters {
    EventGroupFilters {
        dominant_field: filters.dominant_field.clone(),
        limit_event_groups: filters
            .limit_event_groups
            .map(|limit| limit.min(MAX_RUN_SUMMARY_GROUP_LIMIT)),
    }
}

fn filter_run_list_items(
    items: Vec<serde_json::Value>,
    filters: &RunListFilters,
) -> Vec<serde_json::Value> {
    let mut filtered = items;
    if let Some(ref mode) = filters.mode {
        filtered.retain(|i| i.get("mode").and_then(|v| v.as_str()) == Some(mode.as_str()));
    }
    if let Some(ref df) = filters.dominant_field {
        filtered.retain(|i| i.get("dominant_field").and_then(|v| v.as_str()) == Some(df.as_str()));
    }
    if let Some(ref rl) = filters.risk_level {
        filtered.retain(|i| i.get("risk_level").and_then(|v| v.as_str()) == Some(rl.as_str()));
    }
    if let Some(ref since) = filters.since {
        let threshold = parse_history_timestamp(Some(since.as_str()));
        if let Some(t) = threshold {
            filtered.retain(|i| is_history_timestamp_on_or_after(&i["generated_at"], &t));
        }
    }
    if let Some(ref until) = filters.until {
        let threshold = parse_history_timestamp(Some(until.as_str()));
        if let Some(t) = threshold {
            filtered.retain(|i| is_history_timestamp_on_or_before(&i["generated_at"], &t));
        }
    }
    if let Some(min) = filters.min_top_impact_score {
        filtered.retain(|i| is_numeric_at_or_above(&i["top_impact_score"], min));
    }
    if let Some(max) = filters.max_top_impact_score {
        filtered.retain(|i| is_numeric_at_or_below(&i["top_impact_score"], max));
    }
    if let Some(min) = filters.min_top_field_attraction {
        filtered.retain(|i| is_numeric_at_or_above(&i["top_field_attraction"], min));
    }
    if let Some(max) = filters.max_top_field_attraction {
        filtered.retain(|i| is_numeric_at_or_below(&i["top_field_attraction"], max));
    }
    if let Some(min) = filters.min_top_divergence_score {
        filtered.retain(|i| is_numeric_at_or_above(&i["top_divergence_score"], min));
    }
    if let Some(max) = filters.max_top_divergence_score {
        filtered.retain(|i| is_numeric_at_or_below(&i["top_divergence_score"], max));
    }
    if let Some(ref tgdf) = filters.top_group_dominant_field {
        filtered.retain(|i| {
            i.get("top_event_group_dominant_field")
                .and_then(|v| v.as_str())
                == Some(tgdf.as_str())
        });
    }
    if let Some(min) = filters.min_event_group_count {
        filtered.retain(|i| is_numeric_at_or_above(&i["event_group_count"], min as f64));
    }
    if let Some(max) = filters.max_event_group_count {
        filtered.retain(|i| is_numeric_at_or_below(&i["event_group_count"], max as f64));
    }
    filtered
}

fn is_numeric_at_or_above(value: &serde_json::Value, threshold: f64) -> bool {
    value.as_f64().is_some_and(|v| v >= threshold)
}

fn is_numeric_at_or_below(value: &serde_json::Value, threshold: f64) -> bool {
    value.as_f64().is_some_and(|v| v <= threshold)
}

fn parse_history_timestamp(value: Option<&str>) -> Option<i64> {
    parse_iso_rfc3339_timestamp_seconds(value?)
}

fn is_history_timestamp_on_or_after(value: &serde_json::Value, threshold: &i64) -> bool {
    let s = match value.as_str() {
        Some(s) => s,
        None => return false,
    };
    match parse_history_timestamp(Some(s)) {
        Some(ts) => ts >= *threshold,
        None => false,
    }
}

fn is_history_timestamp_on_or_before(value: &serde_json::Value, threshold: &i64) -> bool {
    let s = match value.as_str() {
        Some(s) => s,
        None => return false,
    };
    match parse_history_timestamp(Some(s)) {
        Some(ts) => ts <= *threshold,
        None => false,
    }
}

// ---------------------------------------------------------------------------
// Worldline persistence
// ---------------------------------------------------------------------------

/// Save a worldline to SQLite. Uses INSERT OR REPLACE so idempotent.
/// Returns the worldline's id (which may have been auto-assigned).
pub fn save_worldline(conn: &Connection, worldline: &Worldline) -> Result<i64, TianJiError> {
    let worldline_json = serde_json::to_string(worldline)?;
    let created_at = worldline.created_at.to_rfc3339();
    let parent_id = worldline.parent.map(|pid| pid as i64);

    conn.execute(
        "INSERT OR REPLACE INTO worldlines (id, parent_id, worldline_json, created_at)
         VALUES (?1, ?2, ?3, ?4)",
        params![worldline.id as i64, parent_id, worldline_json, created_at],
    )?;
    Ok(worldline.id as i64)
}

/// Load a worldline by id. Returns None if not found.
pub fn load_worldline(conn: &Connection, id: u64) -> Result<Option<Worldline>, TianJiError> {
    let mut stmt = conn.prepare("SELECT worldline_json FROM worldlines WHERE id = ?1")?;
    let result = stmt.query_row(params![id as i64], |row| {
        let json: String = row.get(0)?;
        Ok(json)
    });

    match result {
        Ok(json) => {
            let mut worldline: Worldline = serde_json::from_str(&json)?;
            // Restore id from DB (serialized JSON may have a different id)
            worldline.id = id;
            Ok(Some(worldline))
        }
        Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
        Err(e) => Err(TianJiError::Storage(e)),
    }
}

/// Load the most recent worldlines, newest first.
pub fn load_latest_worldlines(
    conn: &Connection,
    limit: usize,
) -> Result<Vec<Worldline>, TianJiError> {
    let mut stmt =
        conn.prepare("SELECT id, worldline_json FROM worldlines ORDER BY id DESC LIMIT ?1")?;
    let rows = stmt.query_map(params![limit as i64], |row| {
        let db_id: i64 = row.get(0)?;
        let json: String = row.get(1)?;
        Ok((db_id, json))
    })?;

    let mut worldlines = Vec::new();
    for row in rows {
        let (db_id, json) = row?;
        let mut worldline: Worldline = serde_json::from_str(&json)?;
        worldline.id = db_id as u64;
        worldlines.push(worldline);
    }
    Ok(worldlines)
}

/// Get the next available worldline id.
pub fn next_worldline_id(conn: &Connection) -> Result<u64, TianJiError> {
    let max_id: Option<i64> = conn
        .query_row("SELECT MAX(id) FROM worldlines", [], |row| row.get(0))
        .optional()
        .map_err(TianJiError::Storage)?
        .flatten();
    Ok(max_id.map(|n| n as u64 + 1).unwrap_or(1))
}

// ---------------------------------------------------------------------------
// Baseline persistence
// ---------------------------------------------------------------------------

/// Save a baseline to SQLite. Replaces any existing baseline.
pub fn save_baseline(conn: &mut Connection, baseline: &Baseline) -> Result<(), TianJiError> {
    let baseline_json = serde_json::to_string(baseline)?;
    let locked_at = baseline.locked_at.to_rfc3339();

    let tx = conn.transaction()?;
    tx.execute("DELETE FROM baselines", [])?;
    tx.execute(
        "INSERT INTO baselines (baseline_json, locked_at) VALUES (?1, ?2)",
        params![baseline_json, locked_at],
    )?;
    tx.commit()?;
    Ok(())
}

/// Load the current baseline. Returns None if no baseline is set.
pub fn load_baseline(conn: &Connection) -> Result<Option<Baseline>, TianJiError> {
    let result = conn.query_row(
        "SELECT baseline_json FROM baselines ORDER BY id DESC LIMIT 1",
        [],
        |row| {
            let json: String = row.get(0)?;
            Ok(json)
        },
    );

    match result {
        Ok(json) => {
            let baseline: Baseline = serde_json::from_str(&json)?;
            Ok(Some(baseline))
        }
        Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
        Err(e) => Err(TianJiError::Storage(e)),
    }
}

/// Clear the baseline.
pub fn clear_baseline(conn: &Connection) -> Result<(), TianJiError> {
    conn.execute("DELETE FROM baselines", [])?;
    Ok(())
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod storage_integrity_tests {
    use super::*;
    use std::collections::BTreeMap;
    use std::sync::mpsc;
    use std::time::Duration;

    fn temp_sqlite_path(label: &str) -> String {
        use std::sync::atomic::{AtomicU64, Ordering};
        static COUNTER: AtomicU64 = AtomicU64::new(0);
        let id = COUNTER.fetch_add(1, Ordering::Relaxed);
        let path = format!("/tmp/tianji_storage_{label}_{id}.sqlite3");
        let _ = std::fs::remove_file(&path);
        path
    }

    fn cleanup_db(path: &str) {
        let _ = std::fs::remove_file(path);
        let _ = std::fs::remove_file(format!("{path}-wal"));
        let _ = std::fs::remove_file(format!("{path}-shm"));
    }

    fn table_count(connection: &Connection, table_name: &str) -> usize {
        connection
            .query_row(&format!("SELECT COUNT(*) FROM {table_name}"), [], |row| {
                row.get::<_, i64>(0)
            })
            .expect("table count") as usize
    }

    fn run_ids_desc(connection: &Connection) -> Vec<i64> {
        let mut stmt = connection
            .prepare("SELECT id FROM runs ORDER BY id DESC")
            .expect("prepare run ids");
        stmt.query_map([], |row| row.get::<_, i64>(0))
            .expect("query run ids")
            .collect::<Result<Vec<_>, _>>()
            .expect("collect run ids")
    }

    fn raw_item_with_hashes(identity_hash: &str, content_hash: &str) -> RawItem {
        RawItem {
            source: "fixture:test.xml".to_string(),
            title: "Test title".to_string(),
            summary: "Test summary".to_string(),
            link: "https://example.com/test".to_string(),
            published_at: Some("2026-05-18T00:00:00Z".to_string()),
            entry_identity_hash: identity_hash.to_string(),
            content_hash: content_hash.to_string(),
        }
    }

    fn normalized_event_with_hashes(identity_hash: &str, content_hash: &str) -> NormalizedEvent {
        NormalizedEvent {
            event_id: "event-1".to_string(),
            source: "fixture:test.xml".to_string(),
            title: "Test title".to_string(),
            summary: "Test summary".to_string(),
            link: "https://example.com/test".to_string(),
            published_at: Some("2026-05-18T00:00:00Z".to_string()),
            keywords: vec!["test".to_string()],
            actors: Vec::new(),
            regions: Vec::new(),
            field_scores: BTreeMap::new(),
            entry_identity_hash: identity_hash.to_string(),
            content_hash: content_hash.to_string(),
        }
    }

    #[test]
    fn missing_raw_item_canonical_id_returns_data_integrity() {
        let conn = Connection::open_in_memory().expect("in-memory sqlite");
        let raw_items = vec![raw_item_with_hashes("identity-a", "content-a")];
        let canonical_ids = BTreeMap::new();

        let result = insert_raw_items(&conn, 1, &raw_items, &canonical_ids);

        assert!(matches!(
            result,
            Err(TianJiError::DataIntegrity(message))
                if message == "missing canonical source item id"
        ));
    }

    #[test]
    fn missing_normalized_event_canonical_id_returns_data_integrity() {
        let conn = Connection::open_in_memory().expect("in-memory sqlite");
        let normalized_events = vec![normalized_event_with_hashes("identity-a", "content-a")];
        let canonical_ids = BTreeMap::new();

        let result = insert_normalized_events(&conn, 1, &normalized_events, &canonical_ids);

        assert!(matches!(
            result,
            Err(TianJiError::DataIntegrity(message))
                if message == "missing canonical source item id for normalized event"
        ));
    }

    #[test]
    fn sqlite_pool_initializes_schema_and_pragmas() {
        let db_path = temp_sqlite_path("pool_init");
        let pool = SqlitePool::new(&db_path, 1).expect("pool");
        let connection = pool.get().expect("connection");

        let foreign_keys: i64 = connection
            .query_row("PRAGMA foreign_keys", [], |row| row.get(0))
            .expect("foreign keys pragma");
        let journal_mode: String = connection
            .query_row("PRAGMA journal_mode", [], |row| row.get(0))
            .expect("journal mode pragma");
        let runs_exists: i64 = connection
            .query_row(
                "SELECT COUNT(*) FROM sqlite_master WHERE type = 'table' AND name = 'runs'",
                [],
                |row| row.get(0),
            )
            .expect("runs table exists");

        assert_eq!(foreign_keys, 1);
        assert_eq!(journal_mode.to_lowercase(), "wal");
        assert_eq!(runs_exists, 1);

        drop(connection);
        cleanup_db(&db_path);
    }

    #[test]
    fn sqlite_pool_is_bounded_and_reuses_returned_connections() {
        let db_path = temp_sqlite_path("pool_bounded");
        let pool = SqlitePool::new(&db_path, 1).expect("pool");
        let first = pool.get().expect("first connection");
        first
            .execute("CREATE TEMP TABLE pool_marker (value INTEGER)", [])
            .expect("create temp marker");
        first
            .execute("INSERT INTO pool_marker (value) VALUES (42)", [])
            .expect("insert marker");

        let pool_clone = pool.clone();
        let (tx, rx) = mpsc::channel();
        let handle = std::thread::spawn(move || {
            let second = pool_clone.get().expect("second connection");
            let marker: i64 = second
                .query_row("SELECT value FROM pool_marker", [], |row| row.get(0))
                .expect("marker survives reuse");
            tx.send(marker).expect("send marker");
        });

        assert!(rx.recv_timeout(Duration::from_millis(50)).is_err());
        drop(first);
        assert_eq!(rx.recv_timeout(Duration::from_secs(1)).expect("marker"), 42);
        handle.join().expect("thread join");

        cleanup_db(&db_path);
    }

    #[test]
    fn maintenance_check_rejects_missing_source_without_creating_file() {
        let db_path = temp_sqlite_path("maintenance_check_missing");
        cleanup_db(&db_path);

        let result = maintenance_check(&db_path);

        assert!(
            matches!(result, Err(TianJiError::Usage(message)) if message.contains("does not exist"))
        );
        assert!(!Path::new(&db_path).exists());
    }

    #[test]
    fn maintenance_check_reports_seeded_database_diagnostics() {
        let db_path = temp_sqlite_path("maintenance_check_seeded");
        crate::run_fixture_path("tests/fixtures/sample_feed.xml", Some(&db_path))
            .expect("persist fixture run");

        let report = maintenance_check(&db_path).expect("maintenance check");

        assert_eq!(
            report.schema_version,
            MAINTENANCE_CHECK_REPORT_SCHEMA_VERSION
        );
        assert_eq!(report.sqlite_path, db_path);
        assert_eq!(report.quick_check, "ok");
        assert_eq!(report.foreign_key_violation_count, 0);
        assert_eq!(report.table_counts["runs"], 1);
        assert_eq!(report.table_counts["raw_items"], 3);
        assert_eq!(report.latest_run_id, Some(1));
        assert!(report.file_sizes.total_bytes > 0);
        assert!(report.page_count > 0);

        cleanup_db(&db_path);
    }

    #[test]
    fn backup_rejects_missing_source_and_existing_output() {
        let missing_path = temp_sqlite_path("backup_missing");
        cleanup_db(&missing_path);
        let output_path = temp_sqlite_path("backup_missing_output");
        cleanup_db(&output_path);

        let missing_result = backup_sqlite_database(&missing_path, &output_path, false);
        assert!(
            matches!(missing_result, Err(TianJiError::Usage(message)) if message.contains("does not exist"))
        );
        assert!(!Path::new(&missing_path).exists());

        let db_path = temp_sqlite_path("backup_existing_source");
        crate::run_fixture_path("tests/fixtures/sample_feed.xml", Some(&db_path))
            .expect("persist fixture run");
        std::fs::write(&output_path, b"already here").expect("seed output");

        let existing_result = backup_sqlite_database(&db_path, &output_path, false);
        assert!(
            matches!(existing_result, Err(TianJiError::Usage(message)) if message.contains("already exists"))
        );

        cleanup_db(&db_path);
        cleanup_db(&output_path);
    }

    #[test]
    fn backup_creates_queryable_sqlite_database() {
        let db_path = temp_sqlite_path("backup_source");
        let output_path = temp_sqlite_path("backup_output");
        cleanup_db(&output_path);
        for _ in 0..2 {
            crate::run_fixture_path("tests/fixtures/sample_feed.xml", Some(&db_path))
                .expect("persist fixture run");
        }

        let report = backup_sqlite_database(&db_path, &output_path, false).expect("backup");

        assert_eq!(report.schema_version, BACKUP_REPORT_SCHEMA_VERSION);
        assert_eq!(report.run_count, 2);
        assert!(report.output_bytes > 0);
        let runs =
            list_runs(&output_path, 10, &RunListFilters::default()).expect("list backup runs");
        assert_eq!(runs.len(), 2);
        assert_eq!(runs[0]["run_id"], 2);

        cleanup_db(&db_path);
        cleanup_db(&output_path);
    }

    #[test]
    fn export_rejects_existing_output_and_writes_json_and_jsonl() {
        let db_path = temp_sqlite_path("export_source");
        for _ in 0..2 {
            crate::run_fixture_path("tests/fixtures/sample_feed.xml", Some(&db_path))
                .expect("persist fixture run");
        }
        let json_path = temp_sqlite_path("export_json");
        cleanup_db(&json_path);
        std::fs::write(&json_path, b"exists").expect("seed output");
        let existing_result =
            export_run_history(&db_path, &json_path, ExportFormat::Json, false, false);
        assert!(
            matches!(existing_result, Err(TianJiError::Usage(message)) if message.contains("already exists"))
        );

        let json_report = export_run_history(&db_path, &json_path, ExportFormat::Json, true, true)
            .expect("json export");
        assert_eq!(json_report.schema_version, EXPORT_REPORT_SCHEMA_VERSION);
        assert_eq!(json_report.run_count, 2);
        assert!(json_report.include_details);
        let payload: serde_json::Value =
            serde_json::from_str(&std::fs::read_to_string(&json_path).expect("read json export"))
                .expect("parse json export");
        assert_eq!(payload["metadata"]["run_count"], 2);
        assert_eq!(payload["runs"].as_array().expect("runs array").len(), 2);
        assert!(payload["runs"][0].get("scored_events").is_some());

        let jsonl_path = temp_sqlite_path("export_jsonl");
        cleanup_db(&jsonl_path);
        let jsonl_report =
            export_run_history(&db_path, &jsonl_path, ExportFormat::Jsonl, false, false)
                .expect("jsonl export");
        assert_eq!(jsonl_report.run_count, 2);
        let lines: Vec<String> = std::fs::read_to_string(&jsonl_path)
            .expect("read jsonl export")
            .lines()
            .map(str::to_string)
            .collect();
        assert_eq!(lines.len(), 3);
        let metadata: serde_json::Value = serde_json::from_str(&lines[0]).expect("metadata line");
        assert_eq!(metadata["record_type"], "metadata");
        let run_record: serde_json::Value = serde_json::from_str(&lines[1]).expect("run line");
        assert_eq!(run_record["record_type"], "run");
        assert!(run_record["run"].get("scored_events").is_none());

        cleanup_db(&db_path);
        cleanup_db(&json_path);
        cleanup_db(&jsonl_path);
    }

    #[test]
    fn export_rejects_missing_source_without_creating_file() {
        let db_path = temp_sqlite_path("export_missing");
        cleanup_db(&db_path);
        let output_path = temp_sqlite_path("export_missing_output");
        cleanup_db(&output_path);

        let result = export_run_history(&db_path, &output_path, ExportFormat::Json, false, false);

        assert!(
            matches!(result, Err(TianJiError::Usage(message)) if message.contains("does not exist"))
        );
        assert!(!Path::new(&db_path).exists());
        assert!(!Path::new(&output_path).exists());
    }

    #[test]
    fn compact_checkpoints_and_preserves_readable_history() {
        let db_path = temp_sqlite_path("compact_seeded");
        for _ in 0..2 {
            crate::run_fixture_path("tests/fixtures/sample_feed.xml", Some(&db_path))
                .expect("persist fixture run");
        }

        let report = compact_sqlite_database(&db_path, true).expect("compact");

        assert_eq!(report.schema_version, COMPACT_REPORT_SCHEMA_VERSION);
        assert!(report.vacuum);
        assert!(report.after_page_count > 0);
        let runs =
            list_runs(&db_path, 10, &RunListFilters::default()).expect("list compacted runs");
        assert_eq!(runs.len(), 2);

        cleanup_db(&db_path);
    }

    #[test]
    fn compact_rejects_missing_source_without_creating_file() {
        let db_path = temp_sqlite_path("compact_missing");
        cleanup_db(&db_path);

        let result = compact_sqlite_database(&db_path, false);

        assert!(
            matches!(result, Err(TianJiError::Usage(message)) if message.contains("does not exist"))
        );
        assert!(!Path::new(&db_path).exists());
    }

    #[test]
    fn retention_keep_latest_runs_deletes_older_runs_and_cascades_children() {
        let db_path = temp_sqlite_path("retention_keep_latest");
        for _ in 0..3 {
            crate::run_fixture_path("tests/fixtures/sample_feed.xml", Some(&db_path))
                .expect("persist fixture run");
        }

        let report = apply_retention_policy(&db_path, 2).expect("apply retention");

        assert_eq!(report.schema_version, RETENTION_REPORT_SCHEMA_VERSION);
        assert_eq!(report.sqlite_path, db_path);
        assert_eq!(report.keep_last_runs, 2);
        assert_eq!(report.runs_before, 3);
        assert_eq!(report.runs_after, 2);
        assert_eq!(report.deleted_runs, 1);

        let connection = Connection::open(&db_path).expect("open db");
        assert_eq!(run_ids_desc(&connection), vec![3, 2]);
        assert_eq!(table_count(&connection, "raw_items"), 6);
        assert_eq!(table_count(&connection, "normalized_events"), 6);
        assert_eq!(table_count(&connection, "scored_events"), 6);
        assert_eq!(table_count(&connection, "intervention_candidates"), 6);

        cleanup_db(&db_path);
    }

    #[test]
    fn retention_zero_keeps_no_runs_and_removes_orphan_source_items() {
        let db_path = temp_sqlite_path("retention_zero");
        for _ in 0..2 {
            crate::run_fixture_path("tests/fixtures/sample_feed.xml", Some(&db_path))
                .expect("persist fixture run");
        }

        let report = apply_retention_policy(&db_path, 0).expect("apply retention");

        assert_eq!(report.runs_before, 2);
        assert_eq!(report.runs_after, 0);
        assert_eq!(report.deleted_runs, 2);
        assert_eq!(report.deleted_source_items, 3);

        let connection = Connection::open(&db_path).expect("open db");
        assert_eq!(table_count(&connection, "runs"), 0);
        assert_eq!(table_count(&connection, "raw_items"), 0);
        assert_eq!(table_count(&connection, "normalized_events"), 0);
        assert_eq!(table_count(&connection, "scored_events"), 0);
        assert_eq!(table_count(&connection, "intervention_candidates"), 0);
        assert_eq!(table_count(&connection, "source_items"), 0);

        cleanup_db(&db_path);
    }

    #[test]
    fn retention_keep_more_than_current_runs_is_noop_with_counts() {
        let db_path = temp_sqlite_path("retention_noop");
        for _ in 0..2 {
            crate::run_fixture_path("tests/fixtures/sample_feed.xml", Some(&db_path))
                .expect("persist fixture run");
        }

        let report = apply_retention_policy(&db_path, 10).expect("apply retention");

        assert_eq!(report.runs_before, 2);
        assert_eq!(report.runs_after, 2);
        assert_eq!(report.deleted_runs, 0);
        assert_eq!(report.deleted_source_items, 0);

        let connection = Connection::open(&db_path).expect("open db");
        assert_eq!(run_ids_desc(&connection), vec![2, 1]);
        assert_eq!(table_count(&connection, "source_items"), 3);

        cleanup_db(&db_path);
    }
}

#[cfg(test)]
mod worldline_persistence_tests {
    use super::*;
    use crate::worldline::types::FieldKey;
    use chrono::Utc;
    use std::collections::{BTreeMap, BTreeSet};

    fn sample_worldline(id: u64) -> Worldline {
        let mut fields = BTreeMap::new();
        fields.insert(
            FieldKey {
                region: "east-asia".to_string(),
                domain: "conflict".to_string(),
            },
            3.5,
        );
        fields.insert(
            FieldKey {
                region: "global".to_string(),
                domain: "economy".to_string(),
            },
            2.0,
        );
        let hash = Worldline::compute_snapshot_hash(&fields);
        Worldline {
            id,
            fields,
            events: vec!["evt-1".to_string(), "evt-2".to_string()],
            causal_graph: petgraph::graph::DiGraph::new(),
            active_actors: BTreeSet::from(["usa".to_string()]),
            divergence: 0.5,
            parent: None,
            diverge_tick: 0,
            snapshot_hash: hash,
            created_at: Utc::now(),
        }
    }

    fn temp_connection() -> Connection {
        let conn = Connection::open_in_memory().expect("in-memory sqlite");
        initialize_schema(&conn).expect("schema init");
        conn
    }

    #[test]
    fn save_and_load_worldline_roundtrip() {
        let conn = temp_connection();
        let wl = sample_worldline(1);

        let saved_id = save_worldline(&conn, &wl).expect("save");
        assert_eq!(saved_id, 1);

        let loaded = load_worldline(&conn, 1).expect("load").expect("found");
        assert_eq!(loaded.id, 1);
        assert_eq!(loaded.fields.len(), 2);
        assert_eq!(loaded.events.len(), 2);
        assert!((loaded.divergence - 0.5).abs() < 1e-10);
        assert!(loaded.active_actors.contains("usa"));
        assert_eq!(loaded.parent, None);
        assert_eq!(loaded.diverge_tick, 0);
        assert_eq!(loaded.snapshot_hash, wl.snapshot_hash);
    }

    #[test]
    fn load_missing_worldline_returns_none() {
        let conn = temp_connection();
        let result = load_worldline(&conn, 999).expect("load");
        assert!(result.is_none());
    }

    #[test]
    fn save_worldline_overwrites_existing_id() {
        let conn = temp_connection();
        let wl1 = sample_worldline(1);
        save_worldline(&conn, &wl1).expect("save 1");

        let mut wl2 = sample_worldline(1);
        wl2.divergence = 99.0;
        save_worldline(&conn, &wl2).expect("save 2");

        let loaded = load_worldline(&conn, 1).expect("load").expect("found");
        assert!((loaded.divergence - 99.0).abs() < 1e-10);
    }

    #[test]
    fn load_latest_worldlines_respects_limit() {
        let conn = temp_connection();
        for i in 1..=5u64 {
            save_worldline(&conn, &sample_worldline(i)).expect("save");
        }

        let loaded = load_latest_worldlines(&conn, 3).expect("load");
        assert_eq!(loaded.len(), 3);
        assert_eq!(loaded[0].id, 5); // newest first
    }

    #[test]
    fn next_worldline_id_empty_db_returns_one() {
        let conn = temp_connection();
        let id = next_worldline_id(&conn).expect("next id");
        assert_eq!(id, 1);
    }

    #[test]
    fn next_worldline_id_after_insert_returns_next() {
        let conn = temp_connection();
        save_worldline(&conn, &sample_worldline(1)).expect("save");
        let id = next_worldline_id(&conn).expect("next id");
        assert_eq!(id, 2);
    }

    #[test]
    fn baseline_save_load_and_clear() {
        let mut conn = temp_connection();
        let mut fields = BTreeMap::new();
        fields.insert(
            FieldKey {
                region: "global".to_string(),
                domain: "conflict".to_string(),
            },
            1.0,
        );
        let baseline = Baseline {
            worldline_id: 1,
            snapshot_hash: "abc123".to_string(),
            fields,
            locked_at: Utc::now(),
            locked_by: Some("cli".to_string()),
        };

        save_baseline(&mut conn, &baseline).expect("save");

        let loaded = load_baseline(&conn).expect("load").expect("found");
        assert_eq!(loaded.worldline_id, 1);
        assert_eq!(loaded.snapshot_hash, "abc123");
        assert_eq!(loaded.locked_by.as_deref(), Some("cli"));

        clear_baseline(&conn).expect("clear");
        let after_clear = load_baseline(&conn).expect("load");
        assert!(after_clear.is_none());
    }

    #[test]
    fn baseline_load_returns_none_when_empty() {
        let conn = temp_connection();
        let result = load_baseline(&conn).expect("load");
        assert!(result.is_none());
    }

    #[test]
    fn worldline_with_parent_id_roundtrip() {
        let conn = temp_connection();
        let mut wl = sample_worldline(2);
        wl.parent = Some(1);
        wl.diverge_tick = 5;

        save_worldline(&conn, &wl).expect("save");
        let loaded = load_worldline(&conn, 2).expect("load").expect("found");
        assert_eq!(loaded.parent, Some(1));
        assert_eq!(loaded.diverge_tick, 5);
    }
}
