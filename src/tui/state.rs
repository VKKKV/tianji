use ratatui::widgets::ListState;

use crate::storage::{
    compare_runs, get_run_summary, CompareResult, EventGroupFilters, ScoredEventFilters,
};
use crate::AlertTier;

pub const EMPTY_TUI_MESSAGE: &str = "No persisted runs are available for the TUI browser.";

// ── Glyph set: Nerd Font / ASCII fallback ──────────────────────────

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct GlyphSet {
    pub up: &'static str,
    pub down: &'static str,
    pub nav_hint: &'static str,
    pub bullet: &'static str,
    pub warning: &'static str,
}

pub static NERD_GLYPHS: GlyphSet = GlyphSet {
    up: "\u{2191}",                  // ↑
    down: "\u{2193}",                // ↓
    nav_hint: "[\u{2191}/\u{2193}]", // [↑/↓]
    bullet: "\u{2022}",              // •
    warning: "!",
};

pub static ASCII_GLYPHS: GlyphSet = GlyphSet {
    up: "^",
    down: "v",
    nav_hint: "[j/k]",
    bullet: "-",
    warning: "!",
};

/// Detect which glyph set to use.
///
/// Priority:
/// 1. `TIANJI_NERD_FONT=1` env → Nerd Font glyphs
/// 2. `TERM_PROGRAM` matching known Nerd-Font-capable terminals → Nerd Font
/// 3. Otherwise → ASCII fallback (safe for CI / basic terminals)
pub fn detect_glyph_mode() -> &'static GlyphSet {
    if std::env::var("TIANJI_NERD_FONT").as_deref() == Ok("1") {
        return &NERD_GLYPHS;
    }
    if let Ok(term_program) = std::env::var("TERM_PROGRAM") {
        if matches!(
            term_program.as_str(),
            "kitty" | "ghostty" | "wezterm" | "alacritty"
        ) {
            return &NERD_GLYPHS;
        }
    }
    &ASCII_GLYPHS
}

#[derive(Debug, Clone, PartialEq)]
pub struct HistoryRow {
    pub run_id: i64,
    pub generated_at: String,
    pub mode: String,
    pub dominant_field: String,
    pub risk_level: String,
    pub top_divergence_score: Option<f64>,
    pub headline: String,
}

#[derive(Debug, Clone, PartialEq)]
pub struct FieldStat {
    pub field: String,
    pub count: usize,
    pub avg_impact: f64,
}

#[derive(Debug, Clone, PartialEq)]
pub struct TopEvent {
    pub title: String,
    pub impact_score: f64,
    pub dominant_field: String,
}

#[derive(Debug, Clone, PartialEq)]
pub struct DashboardState {
    // Run metadata
    pub latest_run_id: String,
    pub latest_generated_at: String,
    pub latest_mode: String,
    pub headline: String,
    // Field breakdown
    pub field_summary: Vec<FieldStat>,
    pub total_scored_events: usize,
    // Top events
    pub top_events: Vec<TopEvent>,
    // Delta
    pub alert_tier: String,
    pub delta_summary: String,
    pub delta_direction: String,
}

#[derive(Debug, Clone, PartialEq)]
pub struct DetailState {
    pub run_id: i64,
    pub status: String,
    pub schema_version: String,
    pub mode: String,
    pub generated_at: String,
    pub input_summary: String,
    pub scenario_summary: String,
    pub scored_events: Vec<String>,
    pub event_groups: Vec<String>,
    pub intervention_candidates: Vec<String>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct SimField {
    pub region: String,
    pub domain: String,
    pub value: f64,
    pub delta: f64,
}

#[derive(Debug, Clone, PartialEq)]
pub struct SimAgent {
    pub actor_id: String,
    pub status: String,
    pub last_action: String,
}

#[derive(Debug, Clone, PartialEq)]
pub struct SimulationState {
    pub mode: String,
    pub target: String,
    pub horizon: u64,
    pub tick: u64,
    pub total_ticks: u64,
    pub status: String,
    pub field_values: Vec<SimField>,
    pub agent_statuses: Vec<SimAgent>,
    pub event_log: Vec<String>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct CompareState {
    pub left_run_id: i64,
    pub right_run_id: i64,
    pub status: String,
    pub left_summary: Vec<String>,
    pub right_summary: Vec<String>,
    pub diff_lines: Vec<String>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TuiView {
    Dashboard,
    History,
    Detail,
    Compare,
    Simulation,
}

#[derive(Debug, Clone)]
pub struct TuiState {
    pub rows: Vec<HistoryRow>,
    pub dashboard: DashboardState,
    pub view: TuiView,
    pub detail: Option<DetailState>,
    pub compare: Option<CompareState>,
    pub simulation: Option<SimulationState>,
    pub staged_left_run_id: Option<i64>,
    pub sqlite_path: Option<String>,
    pub selected: usize,
    pub pending_g: bool,
    pub search_query: String,
    pub search_active: bool,
    pub all_rows: Vec<HistoryRow>,
    pub glyphs: &'static GlyphSet,
}

impl TuiState {
    pub fn new(rows: Vec<HistoryRow>, dashboard: DashboardState) -> Self {
        let all_rows = rows.clone();
        Self {
            rows,
            dashboard,
            view: TuiView::Dashboard,
            detail: None,
            compare: None,
            simulation: None,
            staged_left_run_id: None,
            sqlite_path: None,
            selected: 0,
            pending_g: false,
            search_query: String::new(),
            search_active: false,
            all_rows,
            glyphs: detect_glyph_mode(),
        }
    }

    pub fn new_with_storage(
        rows: Vec<HistoryRow>,
        dashboard: DashboardState,
        sqlite_path: impl Into<String>,
    ) -> Self {
        let mut state = Self::new(rows, dashboard);
        state.sqlite_path = Some(sqlite_path.into());
        state
    }

    pub fn apply_search(&mut self) {
        if self.search_query.is_empty() {
            self.rows = self.all_rows.clone();
        } else {
            let q = self.search_query.to_lowercase();
            self.rows = self
                .all_rows
                .iter()
                .filter(|row| {
                    row.dominant_field.to_lowercase().contains(&q)
                        || row.headline.to_lowercase().contains(&q)
                        || row.risk_level.to_lowercase().contains(&q)
                })
                .cloned()
                .collect();
        }
        self.selected = 0;
        self.search_active = false;
    }

    pub fn selected(&self) -> usize {
        self.selected
    }

    pub fn select_next(&mut self) {
        if self.rows.is_empty() {
            self.selected = 0;
        } else {
            self.selected = (self.selected + 1).min(self.rows.len() - 1);
        }
    }

    pub fn select_previous(&mut self) {
        self.selected = self.selected.saturating_sub(1);
    }

    pub fn select_first(&mut self) {
        self.selected = 0;
    }

    pub fn select_last(&mut self) {
        self.selected = self.rows.len().saturating_sub(1);
    }

    pub fn show_dashboard(&mut self) {
        self.pending_g = false;
        self.view = TuiView::Dashboard;
    }

    pub fn show_history(&mut self) {
        self.pending_g = false;
        self.view = TuiView::History;
    }

    pub fn show_detail(&mut self, detail: DetailState) {
        self.pending_g = false;
        self.detail = Some(detail);
        self.view = TuiView::Detail;
    }

    pub fn show_compare(&mut self, compare: CompareState) {
        self.pending_g = false;
        self.compare = Some(compare);
        self.view = TuiView::Compare;
    }

    pub fn show_simulation(&mut self, sim: SimulationState) {
        self.pending_g = false;
        self.simulation = Some(sim);
        self.view = TuiView::Simulation;
    }

    pub fn stage_selected_for_compare(&mut self) {
        self.pending_g = false;
        if self.view != TuiView::History {
            return;
        }
        self.staged_left_run_id = self.rows.get(self.selected).map(|row| row.run_id);
    }

    pub fn open_selected_detail(&mut self) {
        self.pending_g = false;
        if self.view != TuiView::History {
            return;
        }
        let Some(row) = self.rows.get(self.selected) else {
            return;
        };
        let detail = match self.sqlite_path.as_deref() {
            Some(sqlite_path) => load_detail_state(sqlite_path, row.run_id),
            None => DetailState::missing(row.run_id),
        };
        self.show_detail(detail);
    }

    pub fn open_selected_compare(&mut self) -> bool {
        self.pending_g = false;
        if self.view != TuiView::History {
            return false;
        }
        let Some(left_run_id) = self.staged_left_run_id else {
            return false;
        };
        let Some(right_row) = self.rows.get(self.selected) else {
            return false;
        };
        let right_run_id = right_row.run_id;
        let compare = match self.sqlite_path.as_deref() {
            Some(sqlite_path) => load_compare_state(sqlite_path, left_run_id, right_run_id),
            None => CompareState::missing(left_run_id, right_run_id),
        };
        self.show_compare(compare);
        true
    }

    pub fn list_state(&self) -> ListState {
        let mut state = ListState::default();
        if !self.rows.is_empty() {
            state.select(Some(self.selected));
        }
        state
    }
}

// ── Shared helper functions (used by multiple modules) ────────────────

pub fn compact_timestamp(value: &str) -> String {
    value
        .replace('T', " ")
        .trim_end_matches("+00:00")
        .trim_end_matches('Z')
        .to_string()
}

pub fn placeholder_or_value(value: &str, placeholder: &str) -> String {
    if value.trim().is_empty() {
        placeholder.to_string()
    } else {
        value.to_string()
    }
}

pub fn capitalize_first(value: &str) -> String {
    let mut chars = value.chars();
    match chars.next() {
        None => String::new(),
        Some(first) => {
            let upper: String = first.to_uppercase().collect();
            upper + chars.as_str()
        }
    }
}

pub fn format_alert_tier(tier: AlertTier) -> String {
    match tier {
        AlertTier::Flash => "flash",
        AlertTier::Priority => "priority",
        AlertTier::Routine => "routine",
    }
    .to_string()
}

pub fn string_field(value: &serde_json::Value, key: &str, placeholder: &str) -> String {
    value
        .get(key)
        .and_then(|v| v.as_str())
        .map(|s| placeholder_or_value(s, placeholder))
        .unwrap_or_else(|| placeholder.to_string())
}

pub fn compact_json_value(value: &serde_json::Value) -> String {
    match value {
        serde_json::Value::String(s) => s.clone(),
        serde_json::Value::Array(items) => format!("{} items", items.len()),
        serde_json::Value::Object(object) => format!("{} fields", object.len()),
        serde_json::Value::Null => "null".to_string(),
        _ => value.to_string(),
    }
}

pub fn compact_json_field(value: &serde_json::Value, key: &str, placeholder: &str) -> String {
    value
        .get(key)
        .map(compact_json_value)
        .filter(|text| !text.trim().is_empty() && text != "null")
        .unwrap_or_else(|| placeholder.to_string())
}

pub fn numeric_field(value: &serde_json::Value, key: &str) -> String {
    value
        .get(key)
        .and_then(|v| v.as_i64().or_else(|| v.as_u64().map(|n| n as i64)))
        .map(|number| number.to_string())
        .unwrap_or_else(|| "0".to_string())
}

pub fn signed_numeric_field(value: &serde_json::Value, key: &str) -> String {
    value
        .get(key)
        .and_then(|v| v.as_i64())
        .map(|number| format!("{number:+}"))
        .unwrap_or_else(|| "unavailable".to_string())
}

pub fn bool_field(value: &serde_json::Value, key: &str) -> String {
    value
        .get(key)
        .and_then(|v| v.as_bool())
        .map(|flag| flag.to_string())
        .unwrap_or_else(|| "unavailable".to_string())
}

pub fn optional_f64_field(value: &serde_json::Value, key: &str) -> String {
    value
        .get(key)
        .and_then(|v| v.as_f64())
        .map(|number| format!("{number:+.6}"))
        .unwrap_or_else(|| "unavailable".to_string())
}

pub fn array_string_field(value: &serde_json::Value, key: &str) -> String {
    let items: Vec<String> = value
        .get(key)
        .and_then(|v| v.as_array())
        .map(|array| {
            array
                .iter()
                .filter_map(|item| item.as_str().map(ToString::to_string))
                .collect()
        })
        .unwrap_or_default();
    if items.is_empty() {
        "none".to_string()
    } else {
        items.join(", ")
    }
}

fn load_detail_state(sqlite_path: &str, run_id: i64) -> DetailState {
    if !std::path::Path::new(sqlite_path).exists() {
        return DetailState::missing(run_id);
    }

    match get_run_summary(
        sqlite_path,
        run_id,
        &ScoredEventFilters::default(),
        false,
        &EventGroupFilters::default(),
    ) {
        Ok(Some(value)) => DetailState::from_json(&value),
        Ok(None) => DetailState::missing(run_id),
        Err(error) => DetailState::error(run_id, error.to_string()),
    }
}

fn load_compare_state(sqlite_path: &str, left_run_id: i64, right_run_id: i64) -> CompareState {
    if left_run_id == right_run_id {
        return CompareState::invalid(
            left_run_id,
            right_run_id,
            format!("Choose a different right run before comparing staged run #{left_run_id}."),
        );
    }
    if !std::path::Path::new(sqlite_path).exists() {
        return CompareState::missing(left_run_id, right_run_id);
    }

    match compare_runs(
        sqlite_path,
        left_run_id,
        right_run_id,
        &ScoredEventFilters::default(),
        false,
        &EventGroupFilters::default(),
    ) {
        Ok(Some(result)) => CompareState::from_result(&result),
        Ok(None) => CompareState::missing(left_run_id, right_run_id),
        Err(error) => CompareState::error(left_run_id, right_run_id, error.to_string()),
    }
}

// ── Re-export DetailState constructors from detail module ───────────

impl DetailState {
    pub fn missing(run_id: i64) -> Self {
        Self {
            run_id,
            status: format!("Run #{run_id} could not be loaded."),
            schema_version: "unavailable".to_string(),
            mode: "unavailable".to_string(),
            generated_at: "unavailable".to_string(),
            input_summary: "No input summary available.".to_string(),
            scenario_summary: "No scenario summary available.".to_string(),
            scored_events: vec!["No scored events available.".to_string()],
            event_groups: vec!["No event groups available.".to_string()],
            intervention_candidates: vec!["No intervention candidates available.".to_string()],
        }
    }

    pub fn error(run_id: i64, message: impl Into<String>) -> Self {
        let mut state = Self::missing(run_id);
        state.status = format!("Run #{run_id} detail error: {}", message.into());
        state
    }

    pub fn from_json(value: &serde_json::Value) -> Self {
        let run_id = value.get("run_id").and_then(|v| v.as_i64()).unwrap_or(0);
        let schema_version = string_field(value, "schema_version", "unavailable");
        let mode = string_field(value, "mode", "unavailable");
        let generated_at = compact_timestamp(&string_field(value, "generated_at", "unavailable"));
        let input_summary_value = value
            .get("input_summary")
            .cloned()
            .unwrap_or(serde_json::Value::Null);
        let scenario_summary_value = value
            .get("scenario_summary")
            .cloned()
            .unwrap_or(serde_json::Value::Null);
        let scored_events = format_scored_event_lines(value.get("scored_events"));
        let event_groups = format_event_group_lines(scenario_summary_value.get("event_groups"));
        let intervention_candidates =
            format_intervention_lines(value.get("intervention_candidates"));

        Self {
            run_id,
            status: "loaded".to_string(),
            schema_version,
            mode,
            generated_at,
            input_summary: format_summary_value(&input_summary_value),
            scenario_summary: format_scenario_summary(&scenario_summary_value),
            scored_events,
            event_groups,
            intervention_candidates,
        }
    }
}

// ── Re-export CompareState constructors from compare module ─────────

impl CompareState {
    pub fn missing(left_run_id: i64, right_run_id: i64) -> Self {
        Self {
            left_run_id,
            right_run_id,
            status: format!("Compare pair #{left_run_id} → #{right_run_id} could not be loaded."),
            left_summary: vec!["No left run summary available.".to_string()],
            right_summary: vec!["No right run summary available.".to_string()],
            diff_lines: vec!["No diff available.".to_string()],
        }
    }

    pub fn invalid(left_run_id: i64, right_run_id: i64, message: impl Into<String>) -> Self {
        let mut state = Self::missing(left_run_id, right_run_id);
        state.status = message.into();
        state
    }

    pub fn error(left_run_id: i64, right_run_id: i64, message: impl Into<String>) -> Self {
        let mut state = Self::missing(left_run_id, right_run_id);
        state.status = format!(
            "Compare pair #{left_run_id} → #{right_run_id} error: {}",
            message.into()
        );
        state
    }

    pub fn from_result(result: &CompareResult) -> Self {
        Self {
            left_run_id: result.left_run_id,
            right_run_id: result.right_run_id,
            status: "loaded".to_string(),
            left_summary: format_compare_side_lines(&result.left),
            right_summary: format_compare_side_lines(&result.right),
            diff_lines: format_compare_diff_lines(&result.diff),
        }
    }
}

// ── Formatting helpers used by multiple modules ────────────────────

fn format_summary_value(value: &serde_json::Value) -> String {
    if value.is_null() {
        return "No summary available.".to_string();
    }
    let Some(object) = value.as_object() else {
        return value.to_string();
    };
    if object.is_empty() {
        return "No summary available.".to_string();
    }
    object
        .iter()
        .map(|(key, value)| format!("{key}: {}", compact_json_value(value)))
        .collect::<Vec<_>>()
        .join("; ")
}

fn format_scenario_summary(value: &serde_json::Value) -> String {
    if value.is_null() {
        return "No scenario summary available.".to_string();
    }
    let Some(object) = value.as_object() else {
        return value.to_string();
    };
    let fields = ["headline", "dominant_field", "risk_level"];
    let parts: Vec<String> = fields
        .iter()
        .filter_map(|key| {
            object
                .get(*key)
                .map(|value| format!("{key}: {}", compact_json_value(value)))
        })
        .collect();
    if parts.is_empty() {
        format_summary_value(value)
    } else {
        parts.join("; ")
    }
}

fn format_scored_event_lines(value: Option<&serde_json::Value>) -> Vec<String> {
    let lines: Vec<String> = value
        .and_then(|v| v.as_array())
        .map(|events| {
            events
                .iter()
                .map(|event| {
                    let event_id = string_field(event, "event_id", "unknown");
                    let title = string_field(event, "title", "Untitled event");
                    let dominant_field = string_field(event, "dominant_field", "uncategorized");
                    let divergence = event
                        .get("divergence_score")
                        .and_then(|v| v.as_f64())
                        .map(|value| format!("{value:.6}"))
                        .unwrap_or_else(|| "-".to_string());
                    format!("{event_id} · {dominant_field} · div {divergence} · {title}")
                })
                .collect()
        })
        .unwrap_or_default();
    if lines.is_empty() {
        vec!["No scored events available.".to_string()]
    } else {
        lines
    }
}

fn format_event_group_lines(value: Option<&serde_json::Value>) -> Vec<String> {
    let lines: Vec<String> = value
        .and_then(|v| v.as_array())
        .map(|groups| {
            groups
                .iter()
                .map(|group| {
                    let headline_id = string_field(group, "headline_event_id", "unknown");
                    let dominant_field = string_field(group, "dominant_field", "uncategorized");
                    let member_count = group
                        .get("member_count")
                        .and_then(|v| v.as_u64())
                        .map(|value| value.to_string())
                        .unwrap_or_else(|| "0".to_string());
                    format!("{headline_id} · {dominant_field} · {member_count} members")
                })
                .collect()
        })
        .unwrap_or_default();
    if lines.is_empty() {
        vec!["No event groups available.".to_string()]
    } else {
        lines
    }
}

fn format_intervention_lines(value: Option<&serde_json::Value>) -> Vec<String> {
    let lines: Vec<String> = value
        .and_then(|v| v.as_array())
        .map(|items| {
            items
                .iter()
                .map(|item| {
                    let priority = item
                        .get("priority")
                        .and_then(|v| v.as_i64())
                        .map(|value| value.to_string())
                        .unwrap_or_else(|| "-".to_string());
                    let event_id = string_field(item, "event_id", "unknown");
                    let target = string_field(item, "target", "unknown target");
                    let intervention_type = string_field(item, "intervention_type", "unknown type");
                    format!("{priority}. {event_id} · {target} · {intervention_type}")
                })
                .collect()
        })
        .unwrap_or_default();
    if lines.is_empty() {
        vec!["No intervention candidates available.".to_string()]
    } else {
        lines
    }
}

fn format_compare_side_lines(value: &serde_json::Value) -> Vec<String> {
    let top_scored_event_id = value
        .get("top_scored_event")
        .and_then(|event| event.get("event_id"))
        .and_then(|v| v.as_str())
        .unwrap_or("none");
    let top_event_group_id = value
        .get("top_event_group")
        .and_then(|group| group.get("headline_event_id"))
        .and_then(|v| v.as_str())
        .unwrap_or("none");
    vec![
        format!("run: #{}", numeric_field(value, "run_id")),
        format!(
            "schema: {}",
            compact_json_field(value, "schema_version", "unavailable")
        ),
        format!("mode: {}", compact_json_field(value, "mode", "unavailable")),
        format!(
            "dominant field: {}",
            compact_json_field(value, "dominant_field", "uncategorized")
        ),
        format!(
            "risk: {}",
            compact_json_field(value, "risk_level", "unknown")
        ),
        format!(
            "headline: {}",
            compact_json_field(value, "headline", "No headline available.")
        ),
        format!(
            "raw/normalized: {}/{}",
            numeric_field(value, "raw_item_count"),
            numeric_field(value, "normalized_event_count")
        ),
        format!(
            "event groups: {} (top {top_event_group_id})",
            numeric_field(value, "event_group_count")
        ),
        format!("top scored event: {top_scored_event_id}"),
    ]
}

fn format_compare_diff_lines(value: &serde_json::Value) -> Vec<String> {
    if value
        .as_object()
        .map(|object| object.is_empty())
        .unwrap_or(true)
    {
        return vec!["No diff available.".to_string()];
    }
    vec![
        format!(
            "raw item delta: {}",
            signed_numeric_field(value, "raw_item_count_delta")
        ),
        format!(
            "normalized event delta: {}",
            signed_numeric_field(value, "normalized_event_count_delta")
        ),
        format!(
            "event group delta: {}",
            signed_numeric_field(value, "event_group_count_delta")
        ),
        format!(
            "dominant field changed: {}",
            bool_field(value, "dominant_field_changed")
        ),
        format!(
            "risk level changed: {}",
            bool_field(value, "risk_level_changed")
        ),
        format!(
            "top event group changed: {}",
            bool_field(value, "top_event_group_changed")
        ),
        format!(
            "top scored event changed: {}",
            bool_field(value, "top_scored_event_changed")
        ),
        format!(
            "top scored event comparable: {}",
            bool_field(value, "top_scored_event_comparable")
        ),
        format!(
            "top intervention changed: {}",
            bool_field(value, "top_intervention_changed")
        ),
        format!(
            "top divergence delta: {}",
            optional_f64_field(value, "top_divergence_score_delta")
        ),
        format!(
            "left-only event groups: {}",
            array_string_field(value, "left_only_event_group_headline_event_ids")
        ),
        format!(
            "right-only event groups: {}",
            array_string_field(value, "right_only_event_group_headline_event_ids")
        ),
        format!(
            "left-only interventions: {}",
            array_string_field(value, "left_only_intervention_event_ids")
        ),
        format!(
            "right-only interventions: {}",
            array_string_field(value, "right_only_intervention_event_ids")
        ),
    ]
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::{BTreeMap, VecDeque};

    use crate::delta::{DeltaReport, DeltaSummary, RiskDirection, SignalBreakdown};
    use crate::delta_memory::HotRunEntry;
    use crate::storage::CompareResult;
    use crate::HotMemory;

    fn row(run_id: i64) -> HistoryRow {
        HistoryRow {
            run_id,
            generated_at: "1970-01-01T00:00:00+00:00".to_string(),
            mode: "fixture".to_string(),
            dominant_field: "technology".to_string(),
            risk_level: "high".to_string(),
            top_divergence_score: Some(20.73),
            headline: "headline".to_string(),
        }
    }

    fn dashboard() -> DashboardState {
        DashboardState::from_run_summary(&[row(1)], &HotMemory::default(), None)
    }

    fn dashboard_with_summary(summary: serde_json::Value) -> DashboardState {
        DashboardState::from_run_summary(&[row(1)], &HotMemory::default(), Some(summary))
    }

    fn history_state(rows: Vec<HistoryRow>) -> TuiState {
        let mut state = TuiState::new(rows, dashboard());
        state.show_history();
        state
    }

    fn temp_sqlite_path() -> String {
        use std::sync::atomic::{AtomicU64, Ordering};
        static COUNTER: AtomicU64 = AtomicU64::new(10_000);
        let id = COUNTER.fetch_add(1, Ordering::Relaxed);
        let path = format!("/tmp/tianji_tui_test_{id}.sqlite3");
        let _ = std::fs::remove_file(&path);
        path
    }

    fn delta_report(total_changes: usize, critical_changes: usize) -> DeltaReport {
        DeltaReport {
            timestamp: "1970-01-01T00:00:00+00:00".to_string(),
            previous_timestamp: Some("1969-12-31T00:00:00+00:00".to_string()),
            numeric_deltas: Vec::new(),
            count_deltas: Vec::new(),
            new_signals: Vec::new(),
            summary: DeltaSummary {
                total_changes,
                critical_changes,
                direction: RiskDirection::RiskOn,
                signal_breakdown: SignalBreakdown {
                    new_count: 2,
                    escalated_count: 1,
                    deescalated_count: 0,
                    unchanged_count: 3,
                },
            },
        }
    }

    #[test]
    fn state_navigation_clamps_to_available_rows() {
        let mut state = history_state(vec![row(1), row(2)]);

        assert_eq!(state.selected(), 0);
        state.select_previous();
        assert_eq!(state.selected(), 0);
        state.select_next();
        assert_eq!(state.selected(), 1);
        state.select_next();
        assert_eq!(state.selected(), 1);
    }

    #[test]
    fn state_selects_first_and_last_rows() {
        let mut state = history_state(vec![row(1), row(2), row(3)]);

        state.select_last();
        assert_eq!(state.selected(), 2);
        state.select_first();
        assert_eq!(state.selected(), 0);
    }

    #[test]
    fn dashboard_maps_latest_run_and_missing_delta_placeholders() {
        let dashboard = DashboardState::from_run_summary(&[row(9)], &HotMemory::default(), None);

        assert_eq!(dashboard.latest_run_id, "#9");
        assert_eq!(dashboard.latest_mode, "fixture");
        assert_eq!(dashboard.headline, "headline");
        assert!(dashboard.field_summary.is_empty());
        assert_eq!(dashboard.total_scored_events, 0);
        assert!(dashboard.top_events.is_empty());
        assert_eq!(dashboard.alert_tier, "none");
        assert_eq!(dashboard.delta_summary, "No recent delta available.");
    }

    #[test]
    fn dashboard_maps_delta_summary_and_alert_tier() {
        let report = delta_report(4, 1);
        let mut memory = HotMemory {
            runs: VecDeque::new(),
            alerted_signals: BTreeMap::new(),
        };
        memory.runs.push_front(HotRunEntry {
            timestamp: "1970-01-01T00:00:00+00:00".to_string(),
            run_id: 1,
            compact: crate::delta_memory::compact_run_data(&serde_json::json!({})),
            delta: Some(report),
        });

        let dashboard = DashboardState::from_run_summary(&[row(1)], &memory, None);

        assert_eq!(dashboard.alert_tier, "priority");
        assert_eq!(
            dashboard.delta_summary,
            "4 total / 1 critical / 2 new signals"
        );
        assert_eq!(dashboard.delta_direction, "RiskOn");
    }

    #[test]
    fn field_stat_extraction_from_run_summary_json() {
        let summary = serde_json::json!({
            "scenario_summary": { "headline": "test headline" },
            "scored_events": [
                { "title": "Event A", "dominant_field": "conflict", "impact_score": 15.0 },
                { "title": "Event B", "dominant_field": "conflict", "impact_score": 5.0 },
                { "title": "Event C", "dominant_field": "diplomacy", "impact_score": 12.0 },
                { "title": "Event D", "dominant_field": "technology", "impact_score": 18.0 },
                { "title": "Event E", "dominant_field": "technology", "impact_score": 10.0 },
                { "title": "Event F", "dominant_field": "economy", "impact_score": 4.0 },
            ]
        });

        let dash = dashboard_with_summary(summary);

        assert_eq!(dash.total_scored_events, 6);
        assert_eq!(dash.headline, "test headline");

        // Field summary sorted by count desc
        assert_eq!(dash.field_summary.len(), 4);
        assert_eq!(dash.field_summary[0].field, "conflict");
        assert_eq!(dash.field_summary[0].count, 2);
        assert!((dash.field_summary[0].avg_impact - 10.0).abs() < 0.01);

        assert_eq!(dash.field_summary[1].field, "technology");
        assert_eq!(dash.field_summary[1].count, 2);
        assert!((dash.field_summary[1].avg_impact - 14.0).abs() < 0.01);

        assert_eq!(dash.field_summary[2].field, "diplomacy");
        assert_eq!(dash.field_summary[2].count, 1);

        assert_eq!(dash.field_summary[3].field, "economy");
        assert_eq!(dash.field_summary[3].count, 1);
    }

    #[test]
    fn top_event_extraction_sorted_by_impact() {
        let summary = serde_json::json!({
            "scenario_summary": { "headline": "test" },
            "scored_events": [
                { "title": "Low event", "dominant_field": "economy", "impact_score": 2.0 },
                { "title": "Mid event", "dominant_field": "diplomacy", "impact_score": 8.0 },
                { "title": "High event", "dominant_field": "conflict", "impact_score": 20.0 },
                { "title": "Very high event", "dominant_field": "conflict", "impact_score": 25.0 },
                { "title": "Another mid", "dominant_field": "technology", "impact_score": 12.0 },
                { "title": "Third mid", "dominant_field": "diplomacy", "impact_score": 10.0 },
                { "title": "Extra event", "dominant_field": "economy", "impact_score": 6.0 },
            ]
        });

        let dash = dashboard_with_summary(summary);

        // Top 5 by impact desc
        assert_eq!(dash.top_events.len(), 5);
        assert_eq!(dash.top_events[0].title, "Very high event");
        assert!((dash.top_events[0].impact_score - 25.0).abs() < 0.01);
        assert_eq!(dash.top_events[0].dominant_field, "conflict");

        assert_eq!(dash.top_events[1].title, "High event");
        assert!((dash.top_events[1].impact_score - 20.0).abs() < 0.01);

        assert_eq!(dash.top_events[2].title, "Another mid");
        assert!((dash.top_events[2].impact_score - 12.0).abs() < 0.01);

        assert_eq!(dash.top_events[3].title, "Third mid");
        assert!((dash.top_events[3].impact_score - 10.0).abs() < 0.01);

        assert_eq!(dash.top_events[4].title, "Mid event");
        assert!((dash.top_events[4].impact_score - 8.0).abs() < 0.01);
    }

    #[test]
    fn dashboard_with_no_run_summary_shows_empty_fields() {
        let dash = DashboardState::from_run_summary(&[], &HotMemory::default(), None);

        assert_eq!(dash.latest_run_id, "unavailable");
        assert!(dash.field_summary.is_empty());
        assert_eq!(dash.total_scored_events, 0);
        assert!(dash.top_events.is_empty());
        assert_eq!(dash.headline, "No headline available.");
    }

    #[test]
    fn detail_state_maps_from_history_show_payload() {
        let payload = serde_json::json!({
            "run_id": 42,
            "schema_version": "1.0",
            "mode": "fixture",
            "generated_at": "1970-01-01T00:00:00+00:00",
            "input_summary": {"raw_item_count": 3, "normalized_event_count": 2},
            "scenario_summary": {
                "headline": "technology pressure rises",
                "dominant_field": "technology",
                "risk_level": "high",
                "event_groups": [{
                    "headline_event_id": "evt-1",
                    "dominant_field": "technology",
                    "member_count": 2
                }]
            },
            "scored_events": [{
                "event_id": "evt-1",
                "title": "AI export controls expand",
                "dominant_field": "technology",
                "divergence_score": 8.5
            }],
            "intervention_candidates": [{
                "priority": 1,
                "event_id": "evt-1",
                "target": "technology",
                "intervention_type": "monitor"
            }]
        });

        let detail = DetailState::from_json(&payload);

        assert_eq!(detail.run_id, 42);
        assert_eq!(detail.schema_version, "1.0");
        assert_eq!(detail.mode, "fixture");
        assert!(detail.input_summary.contains("raw_item_count: 3"));
        assert!(detail
            .scenario_summary
            .contains("technology pressure rises"));
        assert!(detail.scored_events[0].contains("evt-1"));
        assert!(detail.event_groups[0].contains("2 members"));
        assert!(detail.intervention_candidates[0].contains("monitor"));
    }

    #[test]
    fn detail_load_uses_storage_summary_defaults() {
        let db_path = temp_sqlite_path();
        let _ = crate::run_fixture_path("tests/fixtures/sample_feed.xml", Some(&db_path))
            .expect("run + persist");

        let detail = load_detail_state(&db_path, 1);

        assert_eq!(detail.status, "loaded");
        assert_eq!(detail.run_id, 1);
        assert_eq!(detail.mode, "fixture");
        assert!(detail.input_summary.contains("raw_item_count: 3"));
        assert!(detail
            .scored_events
            .iter()
            .any(|line| line.contains("technology")));
        assert!(!detail.intervention_candidates.is_empty());

        let _ = std::fs::remove_file(&db_path);
    }

    #[test]
    fn detail_missing_state_is_stable_placeholder() {
        let db_path = temp_sqlite_path();
        let detail = load_detail_state(&db_path, 99);

        assert_eq!(detail.run_id, 99);
        assert!(detail.status.contains("could not"));
        assert!(
            crate::tui::detail::format_detail(&detail).contains("No scored events available.")
                || detail
                    .scored_events
                    .iter()
                    .any(|l| l.contains("No scored events"))
        );
        assert!(!std::path::Path::new(&db_path).exists());
    }

    #[test]
    fn compare_state_maps_from_storage_compare_result() {
        let result = CompareResult {
            left_run_id: 1,
            right_run_id: 2,
            left: serde_json::json!({
                "run_id": 1,
                "schema_version": "1.0",
                "mode": "fixture",
                "raw_item_count": 3,
                "normalized_event_count": 2,
                "dominant_field": "technology",
                "risk_level": "high",
                "headline": "left headline",
                "event_group_count": 1,
                "top_event_group": {"headline_event_id": "evt-left"},
                "top_scored_event": {"event_id": "scored-left"}
            }),
            right: serde_json::json!({
                "run_id": 2,
                "schema_version": "1.0",
                "mode": "fixture",
                "raw_item_count": 4,
                "normalized_event_count": 3,
                "dominant_field": "conflict",
                "risk_level": "critical",
                "headline": "right headline",
                "event_group_count": 2,
                "top_event_group": {"headline_event_id": "evt-right"},
                "top_scored_event": {"event_id": "scored-right"}
            }),
            diff: serde_json::json!({
                "raw_item_count_delta": 1,
                "normalized_event_count_delta": 1,
                "event_group_count_delta": 1,
                "dominant_field_changed": true,
                "risk_level_changed": true,
                "top_event_group_changed": true,
                "top_scored_event_changed": true,
                "top_scored_event_comparable": false,
                "top_intervention_changed": false,
                "top_divergence_score_delta": 0.75,
                "left_only_event_group_headline_event_ids": ["evt-left"],
                "right_only_event_group_headline_event_ids": ["evt-right"],
                "left_only_intervention_event_ids": [],
                "right_only_intervention_event_ids": ["scored-right"]
            }),
        };

        let compare = CompareState::from_result(&result);

        assert_eq!(compare.left_run_id, 1);
        assert_eq!(compare.right_run_id, 2);
        assert_eq!(compare.status, "loaded");
        assert!(compare
            .left_summary
            .iter()
            .any(|line| line.contains("technology")));
        assert!(compare
            .right_summary
            .iter()
            .any(|line| line.contains("right headline")));
        assert!(compare
            .diff_lines
            .iter()
            .any(|line| line.contains("dominant field changed: true")));
        assert!(compare
            .diff_lines
            .iter()
            .any(|line| line.contains("right-only interventions: scored-right")));
    }

    #[test]
    fn compare_load_uses_storage_compare_defaults() {
        let db_path = temp_sqlite_path();
        let _ = crate::run_fixture_path("tests/fixtures/sample_feed.xml", Some(&db_path))
            .expect("first run + persist");
        let _ = crate::run_fixture_path("tests/fixtures/sample_feed.xml", Some(&db_path))
            .expect("second run + persist");

        let compare = load_compare_state(&db_path, 1, 2);

        assert_eq!(compare.status, "loaded");
        assert_eq!(compare.left_run_id, 1);
        assert_eq!(compare.right_run_id, 2);
        assert!(compare
            .left_summary
            .iter()
            .any(|line| line.contains("run: #1")));
        assert!(compare
            .right_summary
            .iter()
            .any(|line| line.contains("run: #2")));
        assert!(compare
            .diff_lines
            .iter()
            .any(|line| line.contains("dominant field changed")));

        let _ = std::fs::remove_file(&db_path);
    }

    #[test]
    fn compare_missing_state_is_stable_placeholder_without_creating_db() {
        let db_path = temp_sqlite_path();
        let compare = load_compare_state(&db_path, 1, 2);

        assert_eq!(compare.left_run_id, 1);
        assert_eq!(compare.right_run_id, 2);
        assert!(compare.status.contains("could not"));
        assert!(compare
            .diff_lines
            .iter()
            .any(|l| l.contains("No diff available.")));
        assert!(!std::path::Path::new(&db_path).exists());
    }

    #[test]
    fn search_filter_matches_dominant_field() {
        let rows = vec![
            HistoryRow {
                run_id: 1,
                generated_at: String::new(),
                mode: String::new(),
                dominant_field: "conflict".to_string(),
                risk_level: "high".to_string(),
                top_divergence_score: None,
                headline: "event A".to_string(),
            },
            HistoryRow {
                run_id: 2,
                generated_at: String::new(),
                mode: String::new(),
                dominant_field: "diplomacy".to_string(),
                risk_level: "low".to_string(),
                top_divergence_score: None,
                headline: "event B".to_string(),
            },
            HistoryRow {
                run_id: 3,
                generated_at: String::new(),
                mode: String::new(),
                dominant_field: "technology".to_string(),
                risk_level: "medium".to_string(),
                top_divergence_score: None,
                headline: "event C".to_string(),
            },
        ];
        let mut state = history_state(rows);
        state.search_query = "conflict".to_string();
        state.apply_search();
        assert_eq!(state.rows.len(), 1);
        assert_eq!(state.rows[0].run_id, 1);
    }

    #[test]
    fn search_filter_matches_headline_case_insensitive() {
        let rows = vec![
            HistoryRow {
                run_id: 1,
                generated_at: String::new(),
                mode: String::new(),
                dominant_field: "conflict".to_string(),
                risk_level: "high".to_string(),
                top_divergence_score: None,
                headline: "Nuclear Talks Resume".to_string(),
            },
            HistoryRow {
                run_id: 2,
                generated_at: String::new(),
                mode: String::new(),
                dominant_field: "diplomacy".to_string(),
                risk_level: "low".to_string(),
                top_divergence_score: None,
                headline: "Trade deal signed".to_string(),
            },
        ];
        let mut state = history_state(rows);
        state.search_query = "nuclear talks".to_string();
        state.apply_search();
        assert_eq!(state.rows.len(), 1);
        assert_eq!(state.rows[0].run_id, 1);
    }

    #[test]
    fn search_filter_matches_risk_level() {
        let rows = vec![
            HistoryRow {
                run_id: 1,
                generated_at: String::new(),
                mode: String::new(),
                dominant_field: "conflict".to_string(),
                risk_level: "critical".to_string(),
                top_divergence_score: None,
                headline: "event A".to_string(),
            },
            HistoryRow {
                run_id: 2,
                generated_at: String::new(),
                mode: String::new(),
                dominant_field: "diplomacy".to_string(),
                risk_level: "low".to_string(),
                top_divergence_score: None,
                headline: "event B".to_string(),
            },
        ];
        let mut state = history_state(rows);
        state.search_query = "critical".to_string();
        state.apply_search();
        assert_eq!(state.rows.len(), 1);
        assert_eq!(state.rows[0].run_id, 1);
    }

    #[test]
    fn search_empty_query_restores_full_list() {
        let rows = vec![row(1), row(2), row(3)];
        let mut state = history_state(rows);
        state.search_query = "nonexistent".to_string();
        state.apply_search();
        assert!(state.rows.len() < state.all_rows.len());

        state.search_query.clear();
        state.apply_search();
        assert_eq!(state.rows.len(), state.all_rows.len());
    }

    #[test]
    fn search_no_matches_shows_empty_list() {
        let rows = vec![row(1), row(2)];
        let mut state = history_state(rows);
        state.search_query = "xyznonexistent".to_string();
        state.apply_search();
        assert_eq!(state.rows.len(), 0);
        assert_eq!(state.all_rows.len(), 2);
    }

    #[test]
    fn search_does_not_mutate_all_rows() {
        let rows = vec![row(1), row(2), row(3)];
        let mut state = history_state(rows);
        let original_count = state.all_rows.len();
        state.search_query = "nonexistent".to_string();
        state.apply_search();
        assert_eq!(state.all_rows.len(), original_count);
        // Restore by clearing search
        state.search_query.clear();
        state.apply_search();
        assert_eq!(state.rows.len(), original_count);
    }

    #[test]
    fn search_case_insensitive_matching() {
        let rows = vec![HistoryRow {
            run_id: 1,
            generated_at: String::new(),
            mode: String::new(),
            dominant_field: "Conflict".to_string(),
            risk_level: "HIGH".to_string(),
            top_divergence_score: None,
            headline: "Major Event".to_string(),
        }];
        let mut state = history_state(rows);
        state.search_query = "conflict".to_string();
        state.apply_search();
        assert_eq!(state.rows.len(), 1);

        state.search_query = "high".to_string();
        state.apply_search();
        assert_eq!(state.rows.len(), 1);

        state.search_query = "major event".to_string();
        state.apply_search();
        assert_eq!(state.rows.len(), 1);
    }

    #[test]
    fn search_navigation_works_on_filtered_subset() {
        let rows = vec![
            HistoryRow {
                run_id: 1,
                generated_at: String::new(),
                mode: String::new(),
                dominant_field: "conflict".to_string(),
                risk_level: "high".to_string(),
                top_divergence_score: None,
                headline: "event A".to_string(),
            },
            HistoryRow {
                run_id: 2,
                generated_at: String::new(),
                mode: String::new(),
                dominant_field: "diplomacy".to_string(),
                risk_level: "low".to_string(),
                top_divergence_score: None,
                headline: "event B".to_string(),
            },
            HistoryRow {
                run_id: 3,
                generated_at: String::new(),
                mode: String::new(),
                dominant_field: "conflict".to_string(),
                risk_level: "critical".to_string(),
                top_divergence_score: None,
                headline: "event C".to_string(),
            },
        ];
        let mut state = history_state(rows);
        state.search_query = "conflict".to_string();
        state.apply_search();
        assert_eq!(state.rows.len(), 2);
        assert_eq!(state.selected(), 0);

        state.select_next();
        assert_eq!(state.selected(), 1);
        assert_eq!(state.rows[1].run_id, 3);

        state.select_next();
        assert_eq!(state.selected(), 1); // clamped to filtered subset

        state.select_first();
        assert_eq!(state.selected(), 0);
        state.select_last();
        assert_eq!(state.selected(), 1);
    }

    // ── GlyphSet / detect_glyph_mode tests ──────────────────────────

    #[test]
    fn detect_glyph_mode_env_detection() {
        // All env-dependent checks in one test to avoid parallel races.

        // 1. TIANJI_NERD_FONT=1 → NERD
        std::env::set_var("TIANJI_NERD_FONT", "1");
        assert!(std::ptr::eq(detect_glyph_mode(), &NERD_GLYPHS));
        std::env::remove_var("TIANJI_NERD_FONT");

        // 2. No env → depends on TERM_PROGRAM; remove both → ASCII
        std::env::remove_var("TERM_PROGRAM");
        assert!(std::ptr::eq(detect_glyph_mode(), &ASCII_GLYPHS));

        // 3. Known TERM_PROGRAM values → NERD
        for program in &["kitty", "ghostty", "wezterm", "alacritty"] {
            std::env::set_var("TERM_PROGRAM", program);
            assert!(
                std::ptr::eq(detect_glyph_mode(), &NERD_GLYPHS),
                "TERM_PROGRAM={program}"
            );
        }

        // 4. Unknown TERM_PROGRAM → ASCII
        std::env::remove_var("TIANJI_NERD_FONT");
        std::env::set_var("TERM_PROGRAM", "xterm-256color");
        assert!(std::ptr::eq(detect_glyph_mode(), &ASCII_GLYPHS));

        // Clean up
        std::env::remove_var("TERM_PROGRAM");
    }

    #[test]
    fn ascii_glyph_set_contains_no_unicode() {
        // Every ASCII glyph string must be pure ASCII
        fn is_pure_ascii(s: &str) -> bool {
            s.chars().all(|c| c.is_ascii())
        }
        assert!(is_pure_ascii(ASCII_GLYPHS.up));
        assert!(is_pure_ascii(ASCII_GLYPHS.down));
        assert!(is_pure_ascii(ASCII_GLYPHS.nav_hint));
        assert!(is_pure_ascii(ASCII_GLYPHS.bullet));
        assert!(is_pure_ascii(ASCII_GLYPHS.warning));
    }

    #[test]
    fn nerd_glyph_set_contains_unicode_arrows() {
        assert!(NERD_GLYPHS.up.contains('\u{2191}')); // ↑
        assert!(NERD_GLYPHS.down.contains('\u{2193}')); // ↓
        assert!(NERD_GLYPHS.nav_hint.contains('\u{2191}'));
        assert!(NERD_GLYPHS.bullet.contains('\u{2022}')); // •
    }

    // ── SimulationState tests ──────────────────────────────────────────

    #[test]
    fn simulation_state_construction() {
        let sim = SimulationState {
            mode: "forward".to_string(),
            target: "east-asia.conflict".to_string(),
            horizon: 30,
            tick: 5,
            total_ticks: 30,
            status: "running".to_string(),
            field_values: vec![SimField {
                region: "east-asia".to_string(),
                domain: "conflict".to_string(),
                value: 0.84,
                delta: 0.12,
            }],
            agent_statuses: vec![SimAgent {
                actor_id: "china".to_string(),
                status: "thinking".to_string(),
                last_action: "naval exercise".to_string(),
            }],
            event_log: vec!["tick 1: conflict increased by 0.12".to_string()],
        };

        assert_eq!(sim.mode, "forward");
        assert_eq!(sim.target, "east-asia.conflict");
        assert_eq!(sim.horizon, 30);
        assert_eq!(sim.tick, 5);
        assert_eq!(sim.total_ticks, 30);
        assert_eq!(sim.status, "running");
        assert_eq!(sim.field_values.len(), 1);
        assert_eq!(sim.agent_statuses.len(), 1);
        assert_eq!(sim.event_log.len(), 1);
    }

    #[test]
    fn show_simulation_switches_view_and_stores_state() {
        let mut state = TuiState::new(vec![row(1)], dashboard());
        assert_eq!(state.view, TuiView::Dashboard);
        assert!(state.simulation.is_none());

        let sim = SimulationState {
            mode: "forward".to_string(),
            target: "global.conflict".to_string(),
            horizon: 10,
            tick: 3,
            total_ticks: 10,
            status: "running".to_string(),
            field_values: vec![],
            agent_statuses: vec![],
            event_log: vec![],
        };
        state.show_simulation(sim.clone());

        assert_eq!(state.view, TuiView::Simulation);
        assert!(state.simulation.is_some());
        assert_eq!(state.simulation.as_ref().unwrap().target, "global.conflict");
    }

    #[test]
    fn sim_field_delta_direction() {
        let positive = SimField {
            region: "east-asia".to_string(),
            domain: "conflict".to_string(),
            value: 5.0,
            delta: 1.5,
        };
        assert!(positive.delta > 0.0);

        let negative = SimField {
            region: "global".to_string(),
            domain: "trade_volume".to_string(),
            value: 3.0,
            delta: -0.8,
        };
        assert!(negative.delta < 0.0);

        let stable = SimField {
            region: "europe".to_string(),
            domain: "stability".to_string(),
            value: 4.0,
            delta: 0.0,
        };
        assert!(stable.delta.abs() < f64::EPSILON);
    }
}
