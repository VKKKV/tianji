use std::collections::{BTreeMap, VecDeque};
use std::path::Path;
use std::time::{SystemTime, UNIX_EPOCH};

use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::delta::{DeltaReport, RiskDirection};
use crate::TianJiError;

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AlertTier {
    Flash,
    Priority,
    Routine,
}

impl AlertTier {
    pub fn cooldown_secs(&self) -> u64 {
        match self {
            Self::Flash => 5 * 60,
            Self::Priority => 30 * 60,
            Self::Routine => 60 * 60,
        }
    }

    pub fn max_per_hour(&self) -> usize {
        match self {
            Self::Flash => 6,
            Self::Priority => 4,
            Self::Routine => 2,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct AlertDecayModel {
    pub decay_tiers_hours: Vec<u64>,
    pub prune_single_hours: u64,
    pub prune_repeat_hours: u64,
}

impl Default for AlertDecayModel {
    fn default() -> Self {
        Self {
            decay_tiers_hours: vec![0, 6, 12, 24],
            prune_single_hours: 24,
            prune_repeat_hours: 48,
        }
    }
}

impl AlertDecayModel {
    pub fn cooldown_for_count(&self, occurrence_count: usize) -> u64 {
        if self.decay_tiers_hours.is_empty() {
            return 0;
        }
        let idx = occurrence_count
            .saturating_sub(1)
            .min(self.decay_tiers_hours.len() - 1);
        self.decay_tiers_hours[idx] * 3600
    }
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct HotMemory {
    pub runs: VecDeque<HotRunEntry>,
    pub alerted_signals: BTreeMap<String, AlertedSignalEntry>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct HotRunEntry {
    pub timestamp: String,
    pub run_id: i64,
    pub compact: CompactRunData,
    pub delta: Option<DeltaReport>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct AlertedSignalEntry {
    pub first_seen: String,
    pub last_alerted: String,
    pub count: usize,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct CompactRunData {
    pub meta: CompactMeta,
    pub field_summary: BTreeMap<String, FieldCompact>,
    pub top_event_ids: Vec<String>,
    pub top_actor_ids: Vec<String>,
    pub top_region_ids: Vec<String>,
    pub group_ids: Vec<String>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct CompactMeta {
    pub run_id: i64,
    pub mode: String,
    pub generated_at: String,
    pub dominant_field: String,
    pub risk_level: String,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct FieldCompact {
    pub dominant_field: String,
    pub top_impact_score: f64,
    pub top_divergence_score: f64,
    pub top_field_attraction: f64,
    pub event_count: usize,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct DeltaConfig {
    pub numeric_thresholds: BTreeMap<String, u64>,
    pub count_thresholds: BTreeMap<String, i64>,
    pub alert_decay: AlertDecayModel,
    pub hot_run_count: usize,
    pub auto_notify: bool,
}

impl Default for DeltaConfig {
    fn default() -> Self {
        Self {
            numeric_thresholds: BTreeMap::new(),
            count_thresholds: BTreeMap::new(),
            alert_decay: AlertDecayModel::default(),
            hot_run_count: 3,
            auto_notify: true,
        }
    }
}

impl HotMemory {
    pub fn load(path: &Path) -> Self {
        let bak_path = path.with_extension("json.bak");
        for candidate in [path, bak_path.as_path()] {
            if let Ok(raw) = std::fs::read_to_string(candidate) {
                if let Ok(memory) = serde_json::from_str::<Self>(&raw) {
                    return memory;
                }
            }
        }
        Self::default()
    }

    pub fn save_atomic(&self, path: &Path) -> Result<(), TianJiError> {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let tmp_path = path.with_extension("json.tmp");
        let bak_path = path.with_extension("json.bak");
        let json = serde_json::to_string_pretty(self)?;
        std::fs::write(&tmp_path, json)?;
        if path.exists() {
            let _ = std::fs::rename(path, &bak_path);
        }
        std::fs::rename(&tmp_path, path)?;
        Ok(())
    }

    pub fn push_run(&mut self, run: CompactRunData, delta: Option<DeltaReport>, max_runs: usize) {
        self.runs.push_front(HotRunEntry {
            timestamp: run.meta.generated_at.clone(),
            run_id: run.meta.run_id,
            compact: run,
            delta,
        });
        while self.runs.len() > max_runs {
            self.runs.pop_back();
        }
    }

    pub fn is_signal_suppressed(&self, signal_key: &str, decay: &AlertDecayModel) -> bool {
        self.is_signal_suppressed_at(signal_key, decay, unix_now())
    }

    pub fn is_signal_suppressed_at(
        &self,
        signal_key: &str,
        decay: &AlertDecayModel,
        now_unix_secs: i64,
    ) -> bool {
        let Some(entry) = self.alerted_signals.get(signal_key) else {
            return false;
        };
        let Some(last_alerted) = parse_rfc3339_utc_seconds(&entry.last_alerted) else {
            return false;
        };
        let cooldown_secs = decay.cooldown_for_count(entry.count) as i64;
        now_unix_secs.saturating_sub(last_alerted) < cooldown_secs
    }

    pub fn mark_alerted(&mut self, signal_key: &str) {
        self.mark_alerted_at(signal_key, &unix_now().to_string());
    }

    pub fn mark_alerted_at(&mut self, signal_key: &str, timestamp: &str) {
        self.alerted_signals
            .entry(signal_key.to_string())
            .and_modify(|entry| {
                entry.count += 1;
                entry.last_alerted = timestamp.to_string();
            })
            .or_insert_with(|| AlertedSignalEntry {
                first_seen: timestamp.to_string(),
                last_alerted: timestamp.to_string(),
                count: 1,
            });
    }

    pub fn prune_stale_signals(&mut self, decay: &AlertDecayModel) {
        self.prune_stale_signals_at(decay, unix_now());
    }

    pub fn prune_stale_signals_at(&mut self, decay: &AlertDecayModel, now_unix_secs: i64) {
        self.alerted_signals.retain(|_, entry| {
            let Some(last_alerted) = parse_rfc3339_utc_seconds(&entry.last_alerted) else {
                return false;
            };
            let max_age_hours = if entry.count >= 2 {
                decay.prune_repeat_hours
            } else {
                decay.prune_single_hours
            };
            now_unix_secs.saturating_sub(last_alerted) < (max_age_hours * 3600) as i64
        });
    }

    pub fn prune_stale_signals_at_timestamp(&mut self, decay: &AlertDecayModel, timestamp: &str) {
        if let Some(now_unix_secs) = parse_rfc3339_utc_seconds(timestamp) {
            self.prune_stale_signals_at(decay, now_unix_secs);
        }
    }
}

pub fn classify_delta_tier(delta: &DeltaReport) -> Option<AlertTier> {
    let summary = &delta.summary;
    if summary.critical_changes >= 2 && summary.direction == RiskDirection::RiskOff {
        return Some(AlertTier::Flash);
    }
    if summary.critical_changes >= 3 {
        return Some(AlertTier::Flash);
    }
    if summary.critical_changes >= 1 || summary.total_changes >= 3 {
        return Some(AlertTier::Priority);
    }
    if summary.total_changes >= 1 {
        return Some(AlertTier::Routine);
    }
    None
}

pub fn compact_run_data(run: &Value) -> CompactRunData {
    let run_id = run.get("run_id").and_then(|v| v.as_i64()).unwrap_or(0);
    let scenario = run.get("scenario_summary").unwrap_or(&Value::Null);
    let scored_events = run
        .get("scored_events")
        .and_then(|v| v.as_array())
        .cloned()
        .unwrap_or_default();
    let event_groups = scenario
        .get("event_groups")
        .and_then(|v| v.as_array())
        .cloned()
        .unwrap_or_default();

    let mut field_summary: BTreeMap<String, FieldCompact> = BTreeMap::new();
    let mut top_event_ids = Vec::new();
    let mut actors = std::collections::BTreeSet::new();
    let mut regions = std::collections::BTreeSet::new();

    for event in &scored_events {
        if let Some(event_id) = event.get("event_id").and_then(|v| v.as_str()) {
            top_event_ids.push(event_id.to_string());
        }
        collect_string_array(event, "actors", &mut actors);
        collect_string_array(event, "regions", &mut regions);

        let field = event
            .get("dominant_field")
            .and_then(|v| v.as_str())
            .unwrap_or("uncategorized")
            .to_string();
        let impact = event
            .get("impact_score")
            .and_then(|v| v.as_f64())
            .unwrap_or(0.0);
        let divergence = event
            .get("divergence_score")
            .and_then(|v| v.as_f64())
            .unwrap_or(0.0);
        let attraction = event
            .get("field_attraction")
            .and_then(|v| v.as_f64())
            .unwrap_or(0.0);
        field_summary
            .entry(field.clone())
            .and_modify(|summary| {
                summary.top_impact_score = summary.top_impact_score.max(impact);
                summary.top_divergence_score = summary.top_divergence_score.max(divergence);
                summary.top_field_attraction = summary.top_field_attraction.max(attraction);
                summary.event_count += 1;
            })
            .or_insert(FieldCompact {
                dominant_field: field,
                top_impact_score: impact,
                top_divergence_score: divergence,
                top_field_attraction: attraction,
                event_count: 1,
            });
    }

    CompactRunData {
        meta: CompactMeta {
            run_id,
            mode: run
                .get("mode")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string(),
            generated_at: run
                .get("generated_at")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string(),
            dominant_field: scenario
                .get("dominant_field")
                .and_then(|v| v.as_str())
                .unwrap_or("uncategorized")
                .to_string(),
            risk_level: scenario
                .get("risk_level")
                .and_then(|v| v.as_str())
                .unwrap_or("low")
                .to_string(),
        },
        field_summary,
        top_event_ids: top_event_ids.into_iter().take(10).collect(),
        top_actor_ids: actors.into_iter().take(10).collect(),
        top_region_ids: regions.into_iter().take(10).collect(),
        group_ids: event_groups
            .iter()
            .filter_map(|group| {
                group
                    .get("headline_event_id")
                    .and_then(|v| v.as_str())
                    .map(String::from)
            })
            .collect(),
    }
}

fn collect_string_array(value: &Value, key: &str, target: &mut std::collections::BTreeSet<String>) {
    if let Some(values) = value.get(key).and_then(|v| v.as_array()) {
        for value in values {
            if let Some(text) = value.as_str() {
                target.insert(text.to_string());
            }
        }
    }
}

fn unix_now() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_secs() as i64)
        .unwrap_or(0)
}

fn parse_rfc3339_utc_seconds(timestamp: &str) -> Option<i64> {
    if let Ok(raw) = timestamp.parse::<i64>() {
        return Some(raw);
    }
    let normalized = timestamp.strip_suffix('Z').unwrap_or(timestamp);
    let normalized = normalized.strip_suffix("+00:00").unwrap_or(normalized);
    let (date, time) = normalized.split_once('T')?;
    let mut date_parts = date.split('-');
    let year = date_parts.next()?.parse::<i32>().ok()?;
    let month = date_parts.next()?.parse::<u32>().ok()?;
    let day = date_parts.next()?.parse::<u32>().ok()?;
    let mut time_parts = time.split(':');
    let hour = time_parts.next()?.parse::<u32>().ok()?;
    let minute = time_parts.next()?.parse::<u32>().ok()?;
    let second = time_parts.next()?.parse::<u32>().ok()?;
    Some(datetime_to_unix_seconds(
        year, month, day, hour, minute, second,
    ))
}

fn datetime_to_unix_seconds(
    year: i32,
    month: u32,
    day: u32,
    hour: u32,
    minute: u32,
    second: u32,
) -> i64 {
    let days = days_from_civil(year, month, day);
    days * 86_400 + hour as i64 * 3_600 + minute as i64 * 60 + second as i64
}

fn days_from_civil(year: i32, month: u32, day: u32) -> i64 {
    let year = year - i32::from(month <= 2);
    let era = if year >= 0 { year } else { year - 399 } / 400;
    let yoe = year - era * 400;
    let month = month as i32;
    let doy = (153 * (month + if month > 2 { -3 } else { 9 }) + 2) / 5 + day as i32 - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    (era * 146_097 + doe - 719_468) as i64
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicU64, Ordering};

    fn compact(run_id: i64) -> CompactRunData {
        CompactRunData {
            meta: CompactMeta {
                run_id,
                mode: "fixture".to_string(),
                generated_at: format!("1970-01-01T00:00:0{run_id}+00:00"),
                dominant_field: "technology".to_string(),
                risk_level: "high".to_string(),
            },
            field_summary: BTreeMap::new(),
            top_event_ids: Vec::new(),
            top_actor_ids: Vec::new(),
            top_region_ids: Vec::new(),
            group_ids: Vec::new(),
        }
    }

    #[test]
    fn alert_decay_uses_last_tier_for_repeats() {
        let decay = AlertDecayModel::default();
        assert_eq!(decay.cooldown_for_count(1), 0);
        assert_eq!(decay.cooldown_for_count(2), 6 * 3600);
        assert_eq!(decay.cooldown_for_count(99), 24 * 3600);
    }

    #[test]
    fn hot_memory_push_run_keeps_latest_entries() {
        let mut memory = HotMemory::default();
        memory.push_run(compact(1), None, 3);
        memory.push_run(compact(2), None, 3);
        memory.push_run(compact(3), None, 3);
        memory.push_run(compact(4), None, 3);
        let ids: Vec<i64> = memory.runs.iter().map(|r| r.run_id).collect();
        assert_eq!(ids, vec![4, 3, 2]);
    }

    #[test]
    fn hot_memory_save_and_load_roundtrip() {
        static COUNTER: AtomicU64 = AtomicU64::new(0);
        let id = COUNTER.fetch_add(1, Ordering::Relaxed);
        let dir = std::env::temp_dir().join(format!("tianji_delta_memory_{id}"));
        let path = dir.join("hot.json");
        let mut memory = HotMemory::default();
        memory.push_run(compact(7), None, 3);
        memory.save_atomic(&path).expect("save hot memory");
        let loaded = HotMemory::load(&path);
        assert_eq!(loaded.runs.len(), 1);
        assert_eq!(loaded.runs[0].run_id, 7);
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn hot_memory_alert_decay_suppresses_and_prunes_stale_signals() {
        let mut memory = HotMemory::default();
        let decay = AlertDecayModel::default();
        memory.mark_alerted_at("event:1", "1970-01-01T00:00:00+00:00");
        memory.mark_alerted_at("event:1", "1970-01-01T00:00:00+00:00");

        assert!(memory.is_signal_suppressed_at("event:1", &decay, 60));
        assert!(!memory.is_signal_suppressed_at("event:1", &decay, 7 * 3600));

        memory.prune_stale_signals_at(&decay, 49 * 3600);
        assert!(!memory.alerted_signals.contains_key("event:1"));
    }

    #[test]
    fn hot_memory_prunes_with_explicit_timestamp() {
        let mut memory = HotMemory::default();
        let decay = AlertDecayModel::default();
        memory.mark_alerted_at("event:old", "1970-01-01T00:00:00+00:00");

        memory.prune_stale_signals_at_timestamp(&decay, "1970-01-02T01:00:00+00:00");

        assert!(!memory.alerted_signals.contains_key("event:old"));
    }

    #[test]
    fn compact_run_data_extracts_delta_memory_subset() {
        let payload = serde_json::json!({
            "run_id": 9,
            "mode": "fixture",
            "generated_at": "1970-01-01T00:00:00+00:00",
            "scenario_summary": {
                "dominant_field": "technology",
                "risk_level": "high",
                "event_groups": [{"headline_event_id": "event-1"}]
            },
            "scored_events": [{
                "event_id": "event-1",
                "actors": ["usa", "china"],
                "regions": ["east-asia"],
                "dominant_field": "technology",
                "impact_score": 10.0,
                "field_attraction": 4.0,
                "divergence_score": 7.0
            }]
        });

        let compact = compact_run_data(&payload);

        assert_eq!(compact.meta.run_id, 9);
        assert_eq!(compact.top_event_ids, vec!["event-1"]);
        assert_eq!(compact.top_actor_ids, vec!["china", "usa"]);
        assert_eq!(compact.group_ids, vec!["event-1"]);
        assert_eq!(compact.field_summary["technology"].event_count, 1);
    }
}
