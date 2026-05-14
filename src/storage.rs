use std::collections::BTreeMap;
use std::path::Path;
use std::sync::LazyLock;

use rusqlite::{params, Connection};

use crate::fetch::{derive_canonical_content_hash, derive_canonical_entry_identity_hash};
use crate::models::{InterventionCandidate, NormalizedEvent, RawItem, RunArtifact, ScoredEvent};
use crate::TianJiError;

static HISTORY_TIMESTAMP_RE: LazyLock<regex::Regex> = LazyLock::new(|| {
    regex::Regex::new(r"^(\d{4})-(\d{2})-(\d{2})T(\d{2}):(\d{2}):(\d{2})")
        .expect("valid history timestamp regex")
});

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
    let db_path = Path::new(sqlite_path);
    if let Some(parent) = db_path.parent() {
        std::fs::create_dir_all(parent)?;
    }

    let mut connection = Connection::open(db_path)?;
    connection.execute_batch("PRAGMA foreign_keys = ON")?;
    initialize_schema(&connection)?;

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

fn ensure_canonical_source_items(
    connection: &Connection,
    raw_items: &[RawItem],
) -> Result<BTreeMap<(String, String), i64>, TianJiError> {
    let mut canonical_ids: BTreeMap<(String, String), i64> = BTreeMap::new();
    for item in raw_items {
        let identity_hash = if item.entry_identity_hash.is_empty() {
            derive_canonical_entry_identity_hash(item)
        } else {
            item.entry_identity_hash.clone()
        };
        let content_hash = if item.content_hash.is_empty() {
            derive_canonical_content_hash(item)
        } else {
            item.content_hash.clone()
        };
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
        let key = (item.entry_identity_hash.clone(), item.content_hash.clone());
        let canonical_id = canonical_ids.get(&key).ok_or_else(|| {
            TianJiError::Storage(rusqlite::Error::InvalidParameterName(
                "missing canonical source item id".to_string(),
            ))
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
        let key = (
            event.entry_identity_hash.clone(),
            event.content_hash.clone(),
        );
        let canonical_id = canonical_ids.get(&key).ok_or_else(|| {
            TianJiError::Storage(rusqlite::Error::InvalidParameterName(
                "missing canonical source item id for normalized event".to_string(),
            ))
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
    let connection = Connection::open(sqlite_path)?;
    connection.execute_batch("PRAGMA foreign_keys = ON")?;

    let has_filters = has_run_list_filters(filters);
    let sql = if has_filters {
        "SELECT id, schema_version, mode, generated_at, input_summary_json, scenario_summary_json FROM runs ORDER BY id DESC"
    } else {
        "SELECT id, schema_version, mode, generated_at, input_summary_json, scenario_summary_json FROM runs ORDER BY id DESC LIMIT ?1"
    };
    let mut stmt = connection.prepare(sql)?;
    let mut rows = if has_filters {
        stmt.query([])?
    } else {
        stmt.query(params![limit as i64])?
    };

    let mut run_rows: Vec<(i64, String, String, String, String, String)> = Vec::new();
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

    let run_ids: Vec<i64> = run_rows.iter().map(|r| r.0).collect();
    let top_scored_events = get_top_scored_event_summaries(&connection, &run_ids)?;

    let mut items: Vec<serde_json::Value> = Vec::new();
    for (run_id, schema_version, mode, generated_at, input_summary_json, scenario_summary_json) in
        &run_rows
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

    let filtered = filter_run_list_items(items, filters);
    Ok(filtered.into_iter().take(limit).collect())
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
    let connection = Connection::open(sqlite_path)?;
    connection.execute_batch("PRAGMA foreign_keys = ON")?;

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
    let mut stmt = connection.prepare(
        "SELECT event_id, title, source, link, published_at, actors_json, regions_json, keywords_json, dominant_field, impact_score, field_attraction, divergence_score, rationale_json FROM scored_events WHERE run_id = ?1 ORDER BY divergence_score DESC, id ASC",
    )?;
    let mut rows = stmt.query(params![run_id])?;
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
    scored_events = filter_scored_event_details(scored_events, scored_filters);

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
    let connection = Connection::open(sqlite_path)?;
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
    let left = get_run_summary(
        sqlite_path,
        left_run_id,
        scored_filters,
        only_matching_interventions,
        group_filters,
    )?;
    let right = get_run_summary(
        sqlite_path,
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

/// Parse ISO timestamp to approximate unix seconds for comparison.
fn parse_history_timestamp(value: Option<&str>) -> Option<i64> {
    let value = value?;
    let value = value.replace('Z', "+00:00");
    // Simple parser for ISO timestamp prefix
    let caps = HISTORY_TIMESTAMP_RE.captures(&value)?;
    let year: i64 = caps[1].parse().ok()?;
    let month: i64 = caps[2].parse().ok()?;
    let day: i64 = caps[3].parse().ok()?;
    let hour: i64 = caps[4].parse().ok()?;
    let minute: i64 = caps[5].parse().ok()?;
    let second: i64 = caps[6].parse().ok()?;
    let days = days_since_epoch(year, month, day);
    Some(days * 86400 + hour * 3600 + minute * 60 + second)
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

fn days_since_epoch(year: i64, month: i64, day: i64) -> i64 {
    let y = year - 1;
    let leap_years = y / 4 - y / 100 + y / 400;
    let days_from_years = y * 365 + leap_years;
    let cumulative_days_before_month: [i64; 12] =
        [0, 31, 59, 90, 120, 151, 181, 212, 243, 273, 304, 334];
    let is_leap = year % 4 == 0 && (year % 100 != 0 || year % 400 == 0);
    let month_offset = if month >= 3 && is_leap {
        cumulative_days_before_month[month as usize - 1] + 1
    } else {
        cumulative_days_before_month[month as usize - 1]
    };
    days_from_years + month_offset + day - 719528
}

/// Round to 2 decimal places (matches Python round(value, 2)).
fn round2(value: f64) -> f64 {
    format!("{:.2}", value)
        .parse::<f64>()
        .expect("round2: formatted f64 should parse")
}
