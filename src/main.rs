use std::os::unix::process::CommandExt;
use std::path::{Path, PathBuf};
use std::process::ExitCode;

use std::collections::{BTreeMap, BTreeSet};
use std::str::FromStr;

use clap::{CommandFactory, Parser, Subcommand, ValueEnum};
use clap_complete::{generate, Shell};
use rusqlite::Connection;
use serde::Serialize;
use tianji::{
    apply_retention_policy, artifact_json, backup_sqlite_database, classify_delta_tier,
    clear_baseline, compact_sqlite_database, compare_runs, compute_delta, export_run_history,
    get_latest_run_id, get_latest_run_pair, get_next_run_id, get_previous_run_id, get_run_summary,
    list_runs, load_baseline, load_latest_source_health, maintenance_check,
    persist_source_health_checks, run_feed_text, run_fixture_path, save_baseline,
    source_registry::{
        build_sources_report_with_health, load_source_manifest, source_health_inputs_from_runs,
    },
    storage::{EventGroupFilters, RunListFilters, ScoredEventFilters},
    worldline::{
        baseline::Baseline,
        types::{FieldKey, Worldline},
    },
    TianJiError,
};
use tracing::error;
use tracing_subscriber::EnvFilter;

/// Shell names for completion generation
#[derive(Clone, Debug, ValueEnum)]
enum ShellName {
    Bash,
    Zsh,
    Fish,
}

#[derive(Clone, Debug, ValueEnum)]
enum MaintenanceExportFormat {
    Json,
    Jsonl,
}

impl From<MaintenanceExportFormat> for tianji::ExportFormat {
    fn from(value: MaintenanceExportFormat) -> Self {
        match value {
            MaintenanceExportFormat::Json => Self::Json,
            MaintenanceExportFormat::Jsonl => Self::Jsonl,
        }
    }
}

impl std::fmt::Display for MaintenanceExportFormat {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Json => write!(formatter, "json"),
            Self::Jsonl => write!(formatter, "jsonl"),
        }
    }
}

#[derive(Parser)]
#[command(name = "tianji", about = "TianJi — geopolitical intelligence engine")]
enum Cli {
    /// Run the pipeline on a fixture
    Run {
        /// Path to a local RSS/Atom fixture file
        #[arg(long = "fixture")]
        fixture: String,
        /// Optional SQLite database path for persisting run data
        #[arg(long = "sqlite-path")]
        sqlite_path: Option<String>,
        /// Include delta and alert tier context in a wrapper JSON payload
        #[arg(long = "show-delta")]
        show_delta: bool,
    },
    /// List persisted runs
    History {
        /// SQLite database path containing persisted TianJi runs
        #[arg(long = "sqlite-path")]
        sqlite_path: String,
        /// Maximum number of runs to list
        #[arg(long = "limit", default_value_t = 20)]
        limit: usize,
        /// Optional run mode filter
        #[arg(long = "mode")]
        mode: Option<String>,
        /// Optional dominant field filter
        #[arg(long = "dominant-field")]
        dominant_field: Option<String>,
        /// Optional risk level filter
        #[arg(long = "risk-level")]
        risk_level: Option<String>,
        /// Optional inclusive lower bound for generated_at (ISO timestamp)
        #[arg(long = "since")]
        since: Option<String>,
        /// Optional inclusive upper bound for generated_at (ISO timestamp)
        #[arg(long = "until")]
        until: Option<String>,
        /// Optional minimum impact_score for the top scored event
        #[arg(long = "min-top-impact-score")]
        min_top_impact_score: Option<f64>,
        /// Optional maximum impact_score for the top scored event
        #[arg(long = "max-top-impact-score")]
        max_top_impact_score: Option<f64>,
        /// Optional minimum field_attraction for the top scored event
        #[arg(long = "min-top-field-attraction")]
        min_top_field_attraction: Option<f64>,
        /// Optional maximum field_attraction for the top scored event
        #[arg(long = "max-top-field-attraction")]
        max_top_field_attraction: Option<f64>,
        /// Optional minimum divergence_score for the top scored event
        #[arg(long = "min-top-divergence-score")]
        min_top_divergence_score: Option<f64>,
        /// Optional maximum divergence_score for the top scored event
        #[arg(long = "max-top-divergence-score")]
        max_top_divergence_score: Option<f64>,
        /// Optional dominant field filter for the top event group
        #[arg(long = "top-group-dominant-field")]
        top_group_dominant_field: Option<String>,
        /// Optional minimum event-group count
        #[arg(long = "min-event-group-count")]
        min_event_group_count: Option<i64>,
        /// Optional maximum event-group count
        #[arg(long = "max-event-group-count")]
        max_event_group_count: Option<i64>,
    },
    /// Show details for a single persisted run
    HistoryShow {
        /// SQLite database path containing persisted TianJi runs
        #[arg(long = "sqlite-path")]
        sqlite_path: String,
        /// Run identifier to inspect
        #[arg(long = "run-id")]
        run_id: Option<i64>,
        /// Show the latest persisted run
        #[arg(long = "latest")]
        latest: bool,
        /// Show the run immediately before --run-id
        #[arg(long = "previous")]
        previous: bool,
        /// Show the run immediately after --run-id
        #[arg(long = "next")]
        next: bool,
        /// Optional dominant field filter for scored events
        #[arg(long = "dominant-field")]
        dominant_field: Option<String>,
        /// Optional minimum impact_score for scored events
        #[arg(long = "min-impact-score")]
        min_impact_score: Option<f64>,
        /// Optional maximum impact_score for scored events
        #[arg(long = "max-impact-score")]
        max_impact_score: Option<f64>,
        /// Optional minimum field_attraction for scored events
        #[arg(long = "min-field-attraction")]
        min_field_attraction: Option<f64>,
        /// Optional maximum field_attraction for scored events
        #[arg(long = "max-field-attraction")]
        max_field_attraction: Option<f64>,
        /// Optional minimum divergence_score for scored events
        #[arg(long = "min-divergence-score")]
        min_divergence_score: Option<f64>,
        /// Optional maximum divergence_score for scored events
        #[arg(long = "max-divergence-score")]
        max_divergence_score: Option<f64>,
        /// Optional maximum number of scored events to return
        #[arg(long = "limit-scored-events")]
        limit_scored_events: Option<usize>,
        /// Keep only interventions whose event_id is in the visible scored-event set
        #[arg(long = "only-matching-interventions")]
        only_matching_interventions: bool,
        /// Optional dominant field filter for event groups
        #[arg(long = "group-dominant-field")]
        group_dominant_field: Option<String>,
        /// Optional maximum number of event groups to return
        #[arg(long = "limit-event-groups")]
        limit_event_groups: Option<usize>,
    },
    /// Compare two persisted runs
    HistoryCompare {
        /// SQLite database path containing persisted TianJi runs
        #[arg(long = "sqlite-path")]
        sqlite_path: String,
        /// Left-side run identifier
        #[arg(long = "left-run-id")]
        left_run_id: Option<i64>,
        /// Right-side run identifier
        #[arg(long = "right-run-id")]
        right_run_id: Option<i64>,
        /// Compare the two latest persisted runs
        #[arg(long = "latest-pair")]
        latest_pair: bool,
        /// Compare one run against the latest
        #[arg(long = "run-id")]
        run_id: Option<i64>,
        /// Use the latest persisted run as the right-hand side
        #[arg(long = "against-latest")]
        against_latest: bool,
        /// Use the immediately previous persisted run as the left-hand side
        #[arg(long = "against-previous")]
        against_previous: bool,
        /// Optional dominant field filter for scored events
        #[arg(long = "dominant-field")]
        dominant_field: Option<String>,
        /// Optional minimum impact_score for scored events
        #[arg(long = "min-impact-score")]
        min_impact_score: Option<f64>,
        /// Optional maximum impact_score for scored events
        #[arg(long = "max-impact-score")]
        max_impact_score: Option<f64>,
        /// Optional minimum field_attraction for scored events
        #[arg(long = "min-field-attraction")]
        min_field_attraction: Option<f64>,
        /// Optional maximum field_attraction for scored events
        #[arg(long = "max-field-attraction")]
        max_field_attraction: Option<f64>,
        /// Optional minimum divergence_score for scored events
        #[arg(long = "min-divergence-score")]
        min_divergence_score: Option<f64>,
        /// Optional maximum divergence_score for scored events
        #[arg(long = "max-divergence-score")]
        max_divergence_score: Option<f64>,
        /// Optional maximum number of scored events to return
        #[arg(long = "limit-scored-events")]
        limit_scored_events: Option<usize>,
        /// Keep only interventions whose event_id is in the visible scored-event set
        #[arg(long = "only-matching-interventions")]
        only_matching_interventions: bool,
        /// Optional dominant field filter for event groups
        #[arg(long = "group-dominant-field")]
        group_dominant_field: Option<String>,
        /// Optional maximum number of event groups to return
        #[arg(long = "limit-event-groups")]
        limit_event_groups: Option<usize>,
    },
    /// Daemon lifecycle and run queue
    Daemon {
        #[command(subcommand)]
        command: DaemonCommands,
    },
    /// Serve the optional local web UI
    Webui {
        /// Loopback host for the web UI
        #[arg(long = "host", default_value = "127.0.0.1")]
        host: String,
        /// Loopback port for the web UI
        #[arg(long = "port", default_value_t = 8766)]
        port: u16,
        /// API base URL the web UI consumes
        #[arg(long = "api-base-url", default_value = "http://127.0.0.1:8765")]
        api_base_url: String,
        /// UNIX socket path for queue-run proxy
        #[arg(long = "socket-path", default_value = "runs/tianji.sock")]
        socket_path: String,
        /// Optional SQLite path forwarded to queued runs
        #[arg(long = "sqlite-path")]
        sqlite_path: Option<String>,
    },
    /// Browse persisted runs in a read-only terminal UI
    Tui {
        /// SQLite database path containing persisted TianJi runs
        #[arg(long = "sqlite-path")]
        sqlite_path: String,
        /// Maximum number of runs to list
        #[arg(long = "limit", default_value_t = 20)]
        limit: usize,
        /// Optional simulation spec in "field:horizon" format (e.g. "east-asia.conflict:30")
        #[arg(long = "simulate")]
        simulate: Option<String>,
        /// Enable interactive pruning during simulation
        #[arg(long)]
        interactive: bool,
    },
    /// Show delta between the latest runs or an explicit run pair
    Delta {
        /// SQLite database path containing persisted TianJi runs
        #[arg(long = "sqlite-path")]
        sqlite_path: String,
        /// Left-side run identifier (older run for explicit pairs)
        #[arg(long = "left-run-id")]
        left_run_id: Option<i64>,
        /// Right-side run identifier (newer run for explicit pairs)
        #[arg(long = "right-run-id")]
        right_run_id: Option<i64>,
        /// Compare the two latest persisted runs
        #[arg(long = "latest-pair")]
        latest_pair: bool,
    },
    /// Run forward simulation from current worldline state
    Predict {
        /// Target field in region.domain format (e.g. "east-asia.conflict")
        #[arg(long)]
        field: String,
        /// Simulation horizon in ticks
        #[arg(long, default_value_t = 30)]
        horizon: u64,
        /// Directory containing actor profile YAML files
        #[arg(long, default_value = "profiles/")]
        profile_dir: String,
        /// Optional path to TianJi config YAML
        #[arg(long)]
        config: Option<String>,
        /// Optional JSONL trace export path for replay/import tooling
        #[arg(long = "trace-jsonl")]
        trace_jsonl: Option<String>,
    },
    /// Run backward constraint search for intervention paths
    Backtrack {
        /// Goal description for the backward search
        #[arg(long)]
        goal: String,
        /// Field constraint in region.domain:min:max format (repeatable)
        #[arg(long = "field-constraint", value_parser = parse_field_constraint)]
        field_constraints: Vec<(tianji::worldline::types::FieldKey, f64, f64)>,
        /// Maximum number of interventions per path
        #[arg(long, default_value_t = 5)]
        max_interventions: usize,
        /// Directory containing actor profile YAML files
        #[arg(long, default_value = "profiles/")]
        profile_dir: String,
        /// Optional path to TianJi config YAML
        #[arg(long)]
        config: Option<String>,
    },
    /// Manage worldline baseline for divergence tracking
    Baseline {
        /// Lock current worldline state as baseline
        #[arg(long)]
        set: bool,
        /// Show current baseline info
        #[arg(long)]
        show: bool,
        /// Remove the baseline
        #[arg(long)]
        clear: bool,
        /// SQLite database path for locating hot-memory
        #[arg(long)]
        sqlite_path: Option<String>,
    },
    /// Daemon mode: poll feeds and run pipeline on new items
    Watch {
        /// RSS/Atom feed URL to watch
        #[arg(long = "source-url")]
        source_url: String,
        /// Polling interval in seconds
        #[arg(long, default_value_t = 300)]
        interval: u64,
        /// Optional SQLite database path for persisting run data
        #[arg(long)]
        sqlite_path: Option<String>,
        /// Optional path to TianJi config YAML
        #[arg(long)]
        config: Option<String>,
    },
    /// Validate local configuration readiness without printing secrets
    Doctor {
        /// Optional path to TianJi config YAML
        #[arg(long)]
        config: Option<String>,
        /// Optional SQLite database path to check for parent readiness
        #[arg(long = "sqlite-path")]
        sqlite_path: Option<String>,
        /// Emit JSON instead of human-readable output
        #[arg(long = "json")]
        json: bool,
    },
    /// Run deterministic fixture evaluation corpus and report drift
    Eval {
        /// Path to eval corpus manifest YAML
        #[arg(long = "manifest")]
        manifest: String,
        /// Refresh golden snapshots listed in the manifest
        #[arg(long = "update-golden")]
        update_golden: bool,
    },
    /// Inspect source registry manifests and optionally run enabled fixtures
    Sources {
        /// Path to source registry YAML
        #[arg(long = "config")]
        config: String,
        /// Run enabled fixture sources through deterministic pipeline
        #[arg(long = "run-fixtures")]
        run_fixtures: bool,
        /// Fetch enabled RSS/Atom sources explicitly
        #[arg(long = "fetch-live")]
        fetch_live: bool,
        /// Optional SQLite database path for persisting/reading source health history
        #[arg(long = "sqlite-path")]
        sqlite_path: Option<String>,
    },
    /// Operator maintenance commands for local storage
    Maintenance {
        #[command(subcommand)]
        command: MaintenanceCommands,
    },
    /// Generate shell completion scripts
    Completions {
        /// Shell to generate completions for
        shell: ShellName,
    },
}

#[derive(Subcommand)]
enum DaemonCommands {
    /// Start the TianJi daemon
    Start {
        /// UNIX socket path for daemon control
        #[arg(long = "socket-path", default_value = "runs/tianji.sock")]
        socket_path: String,
        /// SQLite database path backing the read API
        #[arg(long = "sqlite-path", default_value = "runs/tianji.sqlite3")]
        sqlite_path: String,
        /// Loopback host marker
        #[arg(long = "host", default_value = "127.0.0.1")]
        host: String,
        /// Loopback HTTP API port
        #[arg(long = "port", default_value_t = 8765)]
        port: u16,
    },
    /// Stop the TianJi daemon
    Stop {
        /// UNIX socket path for daemon control
        #[arg(long = "socket-path", default_value = "runs/tianji.sock")]
        socket_path: String,
    },
    /// Check daemon status or a specific job
    Status {
        /// UNIX socket path for daemon control
        #[arg(long = "socket-path", default_value = "runs/tianji.sock")]
        socket_path: String,
        /// Optional job identifier to inspect
        #[arg(long = "job-id")]
        job_id: Option<String>,
    },
    /// Queue a run via the daemon
    Run {
        /// UNIX socket path for daemon control
        #[arg(long = "socket-path", default_value = "runs/tianji.sock")]
        socket_path: String,
        /// Path to a local RSS/Atom fixture file
        #[arg(long = "fixture")]
        fixture: String,
        /// Optional SQLite database path for persisting run data
        #[arg(long = "sqlite-path")]
        sqlite_path: Option<String>,
    },
    /// Queue a bounded repeated run set via the daemon
    Schedule {
        /// UNIX socket path for daemon control
        #[arg(long = "socket-path", default_value = "runs/tianji.sock")]
        socket_path: String,
        /// Path to a local RSS/Atom fixture file
        #[arg(long = "fixture")]
        fixture: String,
        /// Optional SQLite database path for persisting run data
        #[arg(long = "sqlite-path")]
        sqlite_path: Option<String>,
        /// Seconds to wait between queued submissions; must be at least 60
        #[arg(long = "every-seconds")]
        every_seconds: u64,
        /// Number of runs to queue; must be at least 1
        #[arg(long = "count")]
        count: usize,
    },
    /// Internal: run the daemon server (called by `daemon start`)
    #[command(hide = true)]
    Serve {
        /// UNIX socket path for daemon control
        #[arg(long = "socket-path")]
        socket_path: String,
        /// SQLite database path backing the read API
        #[arg(long = "sqlite-path")]
        sqlite_path: String,
        /// Loopback host marker
        #[arg(long = "host", default_value = "127.0.0.1")]
        host: String,
        /// Loopback HTTP API port
        #[arg(long = "port", default_value_t = 8765)]
        port: u16,
    },
}

#[derive(Subcommand)]
enum MaintenanceCommands {
    /// Run read-only SQLite diagnostics
    Check {
        /// SQLite database path containing persisted TianJi runs
        #[arg(long = "sqlite-path")]
        sqlite_path: String,
    },
    /// Create an online-safe SQLite backup using SQLite-native VACUUM INTO
    Backup {
        /// SQLite database path containing persisted TianJi runs
        #[arg(long = "sqlite-path")]
        sqlite_path: String,
        /// Output SQLite database path
        #[arg(long = "output")]
        output: String,
        /// Replace an existing output file
        #[arg(long = "overwrite")]
        overwrite: bool,
    },
    /// Export persisted run history to JSON or JSONL
    Export {
        /// SQLite database path containing persisted TianJi runs
        #[arg(long = "sqlite-path")]
        sqlite_path: String,
        /// Output export path
        #[arg(long = "output")]
        output: String,
        /// Export format
        #[arg(long = "format", value_enum, default_value_t = MaintenanceExportFormat::Json)]
        format: MaintenanceExportFormat,
        /// Include full run details instead of list summaries
        #[arg(long = "include-details")]
        include_details: bool,
        /// Replace an existing output file
        #[arg(long = "overwrite")]
        overwrite: bool,
    },
    /// Checkpoint WAL and optionally VACUUM the database
    Compact {
        /// SQLite database path containing persisted TianJi runs
        #[arg(long = "sqlite-path")]
        sqlite_path: String,
        /// Run VACUUM after WAL checkpoint truncate
        #[arg(long = "vacuum")]
        vacuum: bool,
    },
    /// Apply SQLite run-history retention policy
    Retain {
        /// SQLite database path containing persisted TianJi runs
        #[arg(long = "sqlite-path")]
        sqlite_path: String,
        /// Number of most recent runs to preserve by run id descending
        #[arg(long = "keep-last-runs")]
        keep_last_runs: usize,
    },
}

#[tokio::main]
async fn main() -> ExitCode {
    tracing_subscriber::fmt()
        .with_env_filter(EnvFilter::from_default_env())
        .init();
    let cli = Cli::parse();
    match run(cli).await {
        Ok(output) => {
            if !output.is_empty() {
                println!("{output}");
            }
            ExitCode::SUCCESS
        }
        Err(TianJiError::ReportFailure(output)) => {
            println!("{output}");
            ExitCode::from(1)
        }
        Err(error) => {
            error!("{error}");
            ExitCode::from(1)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::Value;

    const SAMPLE_FIXTURE: &str = "tests/fixtures/sample_feed.xml";

    fn temp_sqlite_path(label: &str) -> String {
        std::env::temp_dir()
            .join(format!(
                "tianji_cli_{label}_{}_{}.sqlite3",
                std::process::id(),
                std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .expect("system time after epoch")
                    .as_nanos()
            ))
            .to_string_lossy()
            .to_string()
    }

    fn cleanup_sqlite_path(path: &str) {
        let _ = std::fs::remove_file(path);
        let memory_path = tianji::delta_memory_path(path);
        if let Some(parent) = memory_path.parent() {
            let _ = std::fs::remove_dir_all(parent);
        }
    }

    fn temp_doctor_path(label: &str) -> PathBuf {
        std::env::temp_dir().join(format!(
            "tianji_doctor_{label}_{}_{}.yaml",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .expect("system time after epoch")
                .as_nanos()
        ))
    }

    fn write_doctor_config(label: &str, yaml: &str) -> PathBuf {
        let path = temp_doctor_path(label);
        std::fs::write(&path, yaml).expect("write doctor config");
        path
    }

    fn cleanup_doctor_path(path: &Path) {
        let _ = std::fs::remove_file(path);
    }

    #[tokio::test]
    async fn run_default_output_remains_run_artifact_json() {
        let db_path = temp_sqlite_path("default_output");
        let _ = run(Cli::Run {
            fixture: SAMPLE_FIXTURE.to_string(),
            sqlite_path: Some(db_path.clone()),
            show_delta: false,
        })
        .await
        .expect("seed run");

        let output = run(Cli::Run {
            fixture: SAMPLE_FIXTURE.to_string(),
            sqlite_path: Some(db_path.clone()),
            show_delta: false,
        })
        .await
        .expect("default output run");
        let value: Value = serde_json::from_str(&output).expect("json output");

        assert_eq!(value["schema_version"], tianji::RUN_ARTIFACT_SCHEMA_VERSION);
        assert!(value.get("input_summary").is_some());
        assert!(value.get("scenario_summary").is_some());
        assert!(value.get("scored_events").is_some());
        assert!(value.get("intervention_candidates").is_some());
        assert!(value.get("artifact").is_none());
        assert!(value.get("delta").is_none());
        assert!(value.get("alert_tier").is_none());

        cleanup_sqlite_path(&db_path);
    }

    #[tokio::test]
    async fn run_show_delta_outputs_wrapper_json() {
        let db_path = temp_sqlite_path("show_delta");
        let _ = run(Cli::Run {
            fixture: SAMPLE_FIXTURE.to_string(),
            sqlite_path: Some(db_path.clone()),
            show_delta: false,
        })
        .await
        .expect("seed run");

        let output = run(Cli::Run {
            fixture: SAMPLE_FIXTURE.to_string(),
            sqlite_path: Some(db_path.clone()),
            show_delta: true,
        })
        .await
        .expect("show delta run");
        let value: Value = serde_json::from_str(&output).expect("json output");

        assert_eq!(
            value["artifact"]["schema_version"],
            tianji::RUN_ARTIFACT_SCHEMA_VERSION
        );
        assert!(value.get("delta").is_some());
        assert!(value.get("alert_tier").is_some());
        assert!(value.get("schema_version").is_none());
        assert_eq!(value["delta"]["summary"]["total_changes"], 0);
        assert!(value["alert_tier"].is_null() || value["alert_tier"].is_string());

        cleanup_sqlite_path(&db_path);
    }

    #[test]
    fn daemon_schedule_rejects_every_seconds_below_minimum() {
        let result = handle_daemon_schedule_with(
            "tests.sock",
            SAMPLE_FIXTURE,
            None,
            59,
            1,
            |_socket_path, _fixture, _sqlite_path| {
                Ok(serde_json::json!({"job_id": "job-unused", "state": "queued"}))
            },
            |_duration| {},
        );

        assert!(
            matches!(result, Err(TianJiError::Usage(message)) if message == "--every-seconds must be at least 60.")
        );
    }

    #[test]
    fn daemon_schedule_rejects_zero_count() {
        let result = handle_daemon_schedule_with(
            "tests.sock",
            SAMPLE_FIXTURE,
            None,
            60,
            0,
            |_socket_path, _fixture, _sqlite_path| {
                Ok(serde_json::json!({"job_id": "job-unused", "state": "queued"}))
            },
            |_duration| {},
        );

        assert!(
            matches!(result, Err(TianJiError::Usage(message)) if message == "--count must be at least 1.")
        );
    }

    #[test]
    fn daemon_schedule_outputs_metadata_and_sleeps_between_submissions_only() {
        let mut queued_calls = Vec::new();
        let mut sleep_calls = Vec::new();
        let output = handle_daemon_schedule_with(
            "tests.sock",
            SAMPLE_FIXTURE,
            Some("runs/test.sqlite3"),
            60,
            2,
            |socket_path, fixture, sqlite_path| {
                queued_calls.push((
                    socket_path.to_string(),
                    fixture.to_string(),
                    sqlite_path.map(str::to_string),
                ));
                Ok(serde_json::json!({
                    "job_id": format!("job-{}", queued_calls.len()),
                    "state": "queued"
                }))
            },
            |duration| sleep_calls.push(duration),
        )
        .expect("schedule output");

        let value: Value = serde_json::from_str(&output).expect("json output");
        assert_eq!(value["schedule"]["every_seconds"], 60);
        assert_eq!(value["schedule"]["count"], 2);
        assert_eq!(
            value["queued_runs"].as_array().expect("queued runs").len(),
            2
        );
        assert_eq!(value["queued_runs"][0]["job_id"], "job-1");
        assert_eq!(value["queued_runs"][1]["job_id"], "job-2");
        assert_eq!(
            value["job_states"],
            serde_json::json!(tianji::daemon::ALLOWED_JOB_STATES)
        );
        assert_eq!(queued_calls.len(), 2);
        assert!(queued_calls.iter().all(|call| call.0 == "tests.sock"));
        assert!(queued_calls.iter().all(|call| call.1 == SAMPLE_FIXTURE));
        assert!(queued_calls
            .iter()
            .all(|call| call.2.as_deref() == Some("runs/test.sqlite3")));
        assert_eq!(sleep_calls, vec![std::time::Duration::from_secs(60)]);
    }

    #[test]
    fn daemon_api_readiness_url_uses_ready_endpoint_and_brackets_ipv6() {
        assert_eq!(
            api_readiness_url("127.0.0.1", 8765),
            "http://127.0.0.1:8765/api/v1/ready"
        );
        assert_eq!(
            api_readiness_url("::1", 8765),
            "http://[::1]:8765/api/v1/ready"
        );
    }

    // -----------------------------------------------------------------------
    // Predict / Backtrack / Baseline / Watch tests
    // -----------------------------------------------------------------------

    #[test]
    fn parse_field_constraint_valid() {
        let result = parse_field_constraint("east-asia.conflict:0:0.5");
        assert!(result.is_ok());
        let (key, min, max) = result.unwrap();
        assert_eq!(key.region, "east-asia");
        assert_eq!(key.domain, "conflict");
        assert!((min - 0.0).abs() < f64::EPSILON);
        assert!((max - 0.5).abs() < f64::EPSILON);
    }

    #[test]
    fn parse_field_constraint_missing_colons() {
        let result = parse_field_constraint("east-asia.conflict");
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("region.domain:min:max"));
    }

    #[test]
    fn parse_field_constraint_invalid_field_format() {
        let result = parse_field_constraint("conflict:0:0.5");
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("region.domain"));
    }

    #[test]
    fn parse_field_constraint_invalid_min() {
        let result = parse_field_constraint("east-asia.conflict:abc:0.5");
        assert!(result.is_err());
    }

    #[test]
    fn parse_field_constraint_min_greater_than_max() {
        let result = parse_field_constraint("east-asia.conflict:1.0:0.5");
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("cannot be greater than"));
    }

    #[test]
    fn predict_output_is_valid_json_with_branches() {
        let rt = tokio::runtime::Runtime::new().expect("runtime");
        let output = rt
            .block_on(handle_predict(
                "global.conflict",
                5,
                "profiles/",
                None,
                None,
            ))
            .expect("predict output");
        let value: Value = serde_json::from_str(&output).expect("json output");

        assert!(value.get("mode").is_some());
        assert!(value.get("branches").is_some());
        let branches = value["branches"].as_array().expect("branches array");
        assert!(!branches.is_empty());
        assert!(value.get("tick_count").is_some());
        assert!(value.get("convergence_reason").is_some());
    }

    #[test]
    fn predict_rejects_invalid_field_format() {
        let rt = tokio::runtime::Runtime::new().expect("runtime");
        let result = rt.block_on(handle_predict("conflict", 5, "profiles/", None, None));
        assert!(result.is_err());
        assert!(matches!(result, Err(TianJiError::Usage(msg)) if msg.contains("region.domain")));
    }

    #[test]
    fn backtrack_output_is_valid_json_with_intervention_paths() {
        let constraints = vec![(
            tianji::worldline::types::FieldKey {
                region: "global".to_string(),
                domain: "conflict".to_string(),
            },
            0.0,
            100.0,
        )];
        let rt = tokio::runtime::Runtime::new().expect("runtime");
        let output = rt
            .block_on(handle_backtrack(
                "keep conflict low",
                &constraints,
                3,
                "profiles/",
                None,
            ))
            .expect("backtrack output");
        let value: Value = serde_json::from_str(&output).expect("json output");

        assert!(value.get("mode").is_some());
        assert!(value.get("intervention_paths").is_some());
        let paths = value["intervention_paths"].as_array().expect("paths array");
        assert!(!paths.is_empty());
    }

    #[test]
    fn backtrack_rejects_empty_constraints() {
        let rt = tokio::runtime::Runtime::new().expect("runtime");
        let result = rt.block_on(handle_backtrack("test", &[], 3, "profiles/", None));
        assert!(result.is_err());
        assert!(matches!(result, Err(TianJiError::Usage(msg)) if msg.contains("field-constraint")));
    }

    #[tokio::test]
    async fn baseline_set_and_show_roundtrip() {
        let db_path = temp_sqlite_path("baseline_set_show");
        let _ = run(Cli::Run {
            fixture: SAMPLE_FIXTURE.to_string(),
            sqlite_path: Some(db_path.clone()),
            show_delta: false,
        })
        .await
        .expect("seed run for baseline");

        let set_output = handle_baseline(true, false, false, Some(&db_path)).expect("baseline set");
        let set_value: Value = serde_json::from_str(&set_output).expect("set json");
        assert_eq!(set_value["locked_by"], "cli");

        let show_output =
            handle_baseline(false, true, false, Some(&db_path)).expect("baseline show");
        let show_value: Value = serde_json::from_str(&show_output).expect("show json");
        assert_eq!(show_value["locked_by"], "cli");

        let clear_output =
            handle_baseline(false, false, true, Some(&db_path)).expect("baseline clear");
        let clear_value: Value = serde_json::from_str(&clear_output).expect("clear json");
        assert_eq!(clear_value["action"], "clear");

        cleanup_sqlite_path(&db_path);
    }

    #[test]
    fn baseline_requires_exactly_one_action() {
        let err = handle_baseline(false, false, false, Some("unused.db"));
        assert!(matches!(err, Err(TianJiError::Usage(msg)) if msg.contains("exactly one")));

        let err = handle_baseline(true, true, false, Some("unused.db"));
        assert!(matches!(err, Err(TianJiError::Usage(msg)) if msg.contains("only one")));
    }

    #[test]
    fn baseline_requires_sqlite_path() {
        let err = handle_baseline(true, false, false, None);
        assert!(matches!(err, Err(TianJiError::Usage(msg)) if msg.contains("--sqlite-path")));
    }

    #[tokio::test]
    async fn baseline_show_without_set_returns_error() {
        let db_path = temp_sqlite_path("baseline_no_set");
        let _ = run(Cli::Run {
            fixture: SAMPLE_FIXTURE.to_string(),
            sqlite_path: Some(db_path.clone()),
            show_delta: false,
        })
        .await
        .expect("seed run");

        let err = handle_baseline(false, true, false, Some(&db_path));
        assert!(matches!(err, Err(TianJiError::Usage(msg)) if msg.contains("No baseline")));

        cleanup_sqlite_path(&db_path);
    }

    #[test]
    fn watch_rejects_interval_below_minimum() {
        let result = handle_watch("https://example.com/feed.xml", 5, None, None);
        assert!(matches!(result, Err(TianJiError::Usage(msg)) if msg.contains("at least 10")));
    }

    #[test]
    fn tiered_watch_rejects_invalid_intervals() {
        let feeds = vec![WatchedFeed {
            source_url: "https://example.com/fast.xml".to_string(),
            tier: FeedTier::Fast,
        }];

        let too_fast = validate_tiered_watch(
            &feeds,
            WatchSchedulerConfig {
                fast_interval: 9,
                slow_interval: 300,
            },
        );
        assert!(matches!(too_fast, Err(TianJiError::Usage(msg)) if msg.contains("at least 10")));

        let slow_below_fast = validate_tiered_watch(
            &feeds,
            WatchSchedulerConfig {
                fast_interval: 60,
                slow_interval: 30,
            },
        );
        assert!(
            matches!(slow_below_fast, Err(TianJiError::Usage(msg)) if msg.contains("below fast"))
        );
    }

    #[test]
    fn tiered_watch_rejects_missing_or_empty_feeds() {
        let config = WatchSchedulerConfig::default();
        let no_feeds = validate_tiered_watch(&[], config);
        assert!(matches!(no_feeds, Err(TianJiError::Usage(msg)) if msg.contains("at least one")));

        let empty_url = validate_tiered_watch(
            &[WatchedFeed {
                source_url: "  ".to_string(),
                tier: FeedTier::Slow,
            }],
            config,
        );
        assert!(matches!(empty_url, Err(TianJiError::Usage(msg)) if msg.contains("non-empty")));
    }

    #[test]
    fn tiered_watch_schedules_fast_more_often_than_slow() {
        let feeds = vec![
            WatchedFeed {
                source_url: "https://example.com/fast.xml".to_string(),
                tier: FeedTier::Fast,
            },
            WatchedFeed {
                source_url: "https://example.com/slow.xml".to_string(),
                tier: FeedTier::Slow,
            },
        ];
        let config = WatchSchedulerConfig {
            fast_interval: 10,
            slow_interval: 30,
        };

        let mut fast_count = 0;
        let mut slow_count = 0;
        for iteration in 1..=4 {
            for feed in due_feeds_for_iteration(&feeds, config, iteration) {
                match feed.tier {
                    FeedTier::Fast => fast_count += 1,
                    FeedTier::Slow => slow_count += 1,
                }
            }
        }

        assert_eq!(fast_count, 4);
        assert_eq!(slow_count, 2);
        assert!(fast_count > slow_count);
    }

    #[test]
    fn watch_injected_fetcher_runs_real_feed_pipeline() {
        let fixture = std::fs::read_to_string(SAMPLE_FIXTURE).expect("fixture feed");
        let output = handle_watch_with_fetcher(
            "https://example.com/feed.xml",
            10,
            None,
            |_| Ok(fixture.clone()),
            |_| {},
        )
        .expect("watch output");
        let payload: Value = serde_json::from_str(&output).expect("watch json");

        assert_eq!(
            payload["watch"]["source_url"],
            "https://example.com/feed.xml"
        );
        assert_eq!(payload["watch"]["iterations"], 3);
        assert!(payload["watch"].get("note").is_none());
        assert_eq!(payload["results"].as_array().unwrap().len(), 3);
        assert_eq!(payload["results"][0]["status"], "ok");
        assert_eq!(payload["results"][0]["raw_item_count"], 3);
        assert_eq!(payload["results"][0]["normalized_event_count"], 3);
        assert!(!payload["results"][0]["headline"]
            .as_str()
            .unwrap()
            .is_empty());
    }

    #[test]
    fn watch_injected_fetcher_records_fetch_errors() {
        let output = handle_watch_with_fetcher(
            "https://example.com/feed.xml",
            10,
            None,
            |_| Err(TianJiError::Input("network down".to_string())),
            |_| {},
        )
        .expect("watch output");
        let payload: Value = serde_json::from_str(&output).expect("watch json");

        assert_eq!(payload["results"].as_array().unwrap().len(), 3);
        assert_eq!(payload["results"][0]["status"], "error");
        assert!(payload["results"][0]["error"]
            .as_str()
            .unwrap()
            .contains("network down"));
    }

    #[test]
    fn tiered_watch_injected_fetcher_outputs_feed_metadata() {
        let fixture = std::fs::read_to_string(SAMPLE_FIXTURE).expect("fixture feed");
        let feeds = vec![
            WatchedFeed {
                source_url: "https://example.com/fast.xml".to_string(),
                tier: FeedTier::Fast,
            },
            WatchedFeed {
                source_url: "https://example.com/slow.xml".to_string(),
                tier: FeedTier::Slow,
            },
        ];
        let config = WatchSchedulerConfig {
            fast_interval: 10,
            slow_interval: 30,
        };
        let mut fetched_urls = Vec::new();
        let mut sleep_calls = Vec::new();

        let output = handle_tiered_watch_with_fetcher(
            &feeds,
            config,
            4,
            None,
            |source_url| {
                fetched_urls.push(source_url.to_string());
                Ok(fixture.clone())
            },
            |duration| sleep_calls.push(duration),
        )
        .expect("tiered watch output");
        let payload: Value = serde_json::from_str(&output).expect("watch json");

        assert_eq!(payload["watch"]["fast_interval"], 10);
        assert_eq!(payload["watch"]["slow_interval"], 30);
        assert_eq!(payload["watch"]["iterations"], 4);
        assert_eq!(payload["watch"]["feeds"].as_array().unwrap().len(), 2);
        assert_eq!(payload["watch"]["feeds"][0]["tier"], "fast");
        assert_eq!(payload["watch"]["feeds"][1]["tier"], "slow");

        let results = payload["results"].as_array().expect("results");
        assert_eq!(results.len(), 6);
        assert_eq!(results[0]["source_url"], "https://example.com/fast.xml");
        assert_eq!(results[0]["tier"], "fast");
        assert_eq!(results[0]["interval"], 10);
        assert_eq!(results[0]["status"], "ok");
        assert_eq!(results[1]["source_url"], "https://example.com/slow.xml");
        assert_eq!(results[1]["tier"], "slow");
        assert_eq!(results[1]["interval"], 30);
        assert_eq!(results[1]["raw_item_count"], 3);
        assert_eq!(results.last().unwrap()["tier"], "slow");

        assert_eq!(
            fetched_urls,
            vec![
                "https://example.com/fast.xml",
                "https://example.com/slow.xml",
                "https://example.com/fast.xml",
                "https://example.com/fast.xml",
                "https://example.com/fast.xml",
                "https://example.com/slow.xml",
            ]
        );
        assert_eq!(sleep_calls, vec![std::time::Duration::from_secs(10); 3]);
    }

    #[test]
    fn fetch_feed_url_rejects_non_http_sources_before_network() {
        let err = fetch_feed_url("file:///tmp/feed.xml");
        assert!(matches!(err, Err(TianJiError::Usage(msg)) if msg.contains("HTTP or HTTPS")));
    }

    #[test]
    fn cli_parse_predict() {
        let cli = Cli::try_parse_from([
            "tianji",
            "predict",
            "--field",
            "east-asia.conflict",
            "--horizon",
            "10",
        ])
        .expect("parse predict");
        match cli {
            Cli::Predict {
                field,
                horizon,
                trace_jsonl,
                ..
            } => {
                assert_eq!(field, "east-asia.conflict");
                assert_eq!(horizon, 10);
                assert_eq!(trace_jsonl, None);
            }
            _ => panic!("expected Predict variant"),
        }
    }

    #[test]
    fn cli_parse_predict_trace_jsonl() {
        let cli = Cli::try_parse_from([
            "tianji",
            "predict",
            "--field",
            "east-asia.conflict",
            "--trace-jsonl",
            "runs/trace.jsonl",
        ])
        .expect("parse predict trace jsonl");
        match cli {
            Cli::Predict { trace_jsonl, .. } => {
                assert_eq!(trace_jsonl.as_deref(), Some("runs/trace.jsonl"));
            }
            _ => panic!("expected Predict variant"),
        }
    }

    #[test]
    fn predict_trace_jsonl_writes_trace_and_preserves_stdout_outcome() {
        let trace_path = std::env::temp_dir().join(format!(
            "tianji_predict_trace_{}_{}.jsonl",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .expect("system time after epoch")
                .as_nanos()
        ));
        let rt = tokio::runtime::Runtime::new().expect("runtime");
        let output = rt
            .block_on(handle_predict(
                "global.conflict",
                3,
                "profiles/",
                None,
                Some(&trace_path),
            ))
            .expect("predict output");
        let stdout: Value = serde_json::from_str(&output).expect("json output");
        assert!(stdout.get("branches").is_some());
        assert!(stdout.get("metadata").is_none());

        let trace = tianji::nuwa::read_trace_jsonl(&trace_path).expect("trace jsonl");
        let _ = std::fs::remove_file(&trace_path);
        assert_eq!(
            trace.metadata.schema_version,
            tianji::nuwa::SIM_TRACE_SCHEMA_VERSION
        );
        assert_eq!(trace.metadata.frame_count, trace.frames.len());
        assert_eq!(
            trace.frames.len(),
            stdout["tick_count"].as_u64().unwrap() as usize
        );
    }

    #[test]
    fn cli_parse_backtrack() {
        let cli = Cli::try_parse_from([
            "tianji",
            "backtrack",
            "--goal",
            "test goal",
            "--field-constraint",
            "global.conflict:0:0.5",
        ])
        .expect("parse backtrack");
        match cli {
            Cli::Backtrack {
                goal,
                field_constraints,
                max_interventions,
                ..
            } => {
                assert_eq!(goal, "test goal");
                assert_eq!(field_constraints.len(), 1);
                assert_eq!(field_constraints[0].0.region, "global");
                assert_eq!(field_constraints[0].0.domain, "conflict");
                assert_eq!(max_interventions, 5);
            }
            _ => panic!("expected Backtrack variant"),
        }
    }

    #[test]
    fn cli_parse_baseline_set() {
        let cli = Cli::try_parse_from(["tianji", "baseline", "--set", "--sqlite-path", "test.db"])
            .expect("parse baseline set");
        match cli {
            Cli::Baseline {
                set, show, clear, ..
            } => {
                assert!(set);
                assert!(!show);
                assert!(!clear);
            }
            _ => panic!("expected Baseline variant"),
        }
    }

    #[test]
    fn cli_parse_watch() {
        let cli = Cli::try_parse_from([
            "tianji",
            "watch",
            "--source-url",
            "https://example.com/feed.xml",
            "--interval",
            "60",
        ])
        .expect("parse watch");
        match cli {
            Cli::Watch {
                source_url,
                interval,
                ..
            } => {
                assert_eq!(source_url, "https://example.com/feed.xml");
                assert_eq!(interval, 60);
            }
            _ => panic!("expected Watch variant"),
        }
    }

    #[test]
    fn cli_parse_doctor() {
        let cli = Cli::try_parse_from([
            "tianji",
            "doctor",
            "--config",
            "tests/config.yaml",
            "--sqlite-path",
            "runs/tianji.sqlite3",
            "--json",
        ])
        .expect("parse doctor");
        match cli {
            Cli::Doctor {
                config,
                sqlite_path,
                json,
            } => {
                assert_eq!(config.as_deref(), Some("tests/config.yaml"));
                assert_eq!(sqlite_path.as_deref(), Some("runs/tianji.sqlite3"));
                assert!(json);
            }
            _ => panic!("expected Doctor variant"),
        }
    }

    #[test]
    fn cli_parse_eval() {
        let cli = Cli::try_parse_from([
            "tianji",
            "eval",
            "--manifest",
            "tests/fixtures/eval/corpus.yaml",
        ])
        .expect("parse eval");
        match cli {
            Cli::Eval {
                manifest,
                update_golden,
            } => {
                assert_eq!(manifest, "tests/fixtures/eval/corpus.yaml");
                assert!(!update_golden);
            }
            _ => panic!("expected Eval variant"),
        }
    }

    #[test]
    fn cli_parse_eval_update_golden() {
        let cli = Cli::try_parse_from([
            "tianji",
            "eval",
            "--manifest",
            "tests/fixtures/eval/corpus.yaml",
            "--update-golden",
        ])
        .expect("parse eval update golden");
        match cli {
            Cli::Eval {
                manifest,
                update_golden,
            } => {
                assert_eq!(manifest, "tests/fixtures/eval/corpus.yaml");
                assert!(update_golden);
            }
            _ => panic!("expected Eval variant"),
        }
    }

    #[test]
    fn cli_parse_sources() {
        let cli = Cli::try_parse_from([
            "tianji",
            "sources",
            "--config",
            "examples/sources.example.yaml",
        ])
        .expect("parse sources");
        match cli {
            Cli::Sources {
                config,
                run_fixtures,
                fetch_live,
                sqlite_path,
            } => {
                assert_eq!(config, "examples/sources.example.yaml");
                assert!(!run_fixtures);
                assert!(!fetch_live);
                assert_eq!(sqlite_path, None);
            }
            _ => panic!("expected Sources variant"),
        }
    }

    #[test]
    fn cli_parse_sources_run_fixtures() {
        let cli = Cli::try_parse_from([
            "tianji",
            "sources",
            "--config",
            "examples/sources.example.yaml",
            "--run-fixtures",
        ])
        .expect("parse sources run fixtures");
        match cli {
            Cli::Sources {
                run_fixtures,
                fetch_live,
                sqlite_path,
                ..
            } => {
                assert!(run_fixtures);
                assert!(!fetch_live);
                assert_eq!(sqlite_path, None);
            }
            _ => panic!("expected Sources variant"),
        }
    }

    #[test]
    fn cli_parse_sources_fetch_live() {
        let cli = Cli::try_parse_from([
            "tianji",
            "sources",
            "--config",
            "examples/sources.example.yaml",
            "--fetch-live",
        ])
        .expect("parse sources fetch live");
        match cli {
            Cli::Sources { fetch_live, .. } => assert!(fetch_live),
            _ => panic!("expected Sources variant"),
        }
    }

    #[test]
    fn cli_parse_sources_sqlite_path() {
        let cli = Cli::try_parse_from([
            "tianji",
            "sources",
            "--config",
            "examples/sources.example.yaml",
            "--sqlite-path",
            "runs/source-health.sqlite3",
        ])
        .expect("parse sources sqlite path");
        match cli {
            Cli::Sources { sqlite_path, .. } => {
                assert_eq!(sqlite_path.as_deref(), Some("runs/source-health.sqlite3"));
            }
            _ => panic!("expected Sources variant"),
        }
    }

    #[test]
    fn cli_parse_maintenance_retain() {
        let cli = Cli::try_parse_from([
            "tianji",
            "maintenance",
            "retain",
            "--sqlite-path",
            "runs/tianji.sqlite3",
            "--keep-last-runs",
            "2",
        ])
        .expect("parse maintenance retain");

        match cli {
            Cli::Maintenance {
                command:
                    MaintenanceCommands::Retain {
                        sqlite_path,
                        keep_last_runs,
                    },
            } => {
                assert_eq!(sqlite_path, "runs/tianji.sqlite3");
                assert_eq!(keep_last_runs, 2);
            }
            _ => panic!("expected Maintenance::Retain variant"),
        }
    }

    #[test]
    fn cli_parse_maintenance_check() {
        let cli = Cli::try_parse_from([
            "tianji",
            "maintenance",
            "check",
            "--sqlite-path",
            "runs/tianji.sqlite3",
        ])
        .expect("parse maintenance check");

        match cli {
            Cli::Maintenance {
                command: MaintenanceCommands::Check { sqlite_path },
            } => assert_eq!(sqlite_path, "runs/tianji.sqlite3"),
            _ => panic!("expected Maintenance::Check variant"),
        }
    }

    #[test]
    fn cli_parse_maintenance_backup() {
        let cli = Cli::try_parse_from([
            "tianji",
            "maintenance",
            "backup",
            "--sqlite-path",
            "runs/tianji.sqlite3",
            "--output",
            "runs/backup.sqlite3",
            "--overwrite",
        ])
        .expect("parse maintenance backup");

        match cli {
            Cli::Maintenance {
                command:
                    MaintenanceCommands::Backup {
                        sqlite_path,
                        output,
                        overwrite,
                    },
            } => {
                assert_eq!(sqlite_path, "runs/tianji.sqlite3");
                assert_eq!(output, "runs/backup.sqlite3");
                assert!(overwrite);
            }
            _ => panic!("expected Maintenance::Backup variant"),
        }
    }

    #[test]
    fn cli_parse_maintenance_export_jsonl_details() {
        let cli = Cli::try_parse_from([
            "tianji",
            "maintenance",
            "export",
            "--sqlite-path",
            "runs/tianji.sqlite3",
            "--output",
            "runs/history.jsonl",
            "--format",
            "jsonl",
            "--include-details",
        ])
        .expect("parse maintenance export");

        match cli {
            Cli::Maintenance {
                command:
                    MaintenanceCommands::Export {
                        sqlite_path,
                        output,
                        format,
                        include_details,
                        overwrite,
                    },
            } => {
                assert_eq!(sqlite_path, "runs/tianji.sqlite3");
                assert_eq!(output, "runs/history.jsonl");
                assert!(matches!(format, MaintenanceExportFormat::Jsonl));
                assert!(include_details);
                assert!(!overwrite);
            }
            _ => panic!("expected Maintenance::Export variant"),
        }
    }

    #[test]
    fn cli_parse_maintenance_compact() {
        let cli = Cli::try_parse_from([
            "tianji",
            "maintenance",
            "compact",
            "--sqlite-path",
            "runs/tianji.sqlite3",
            "--vacuum",
        ])
        .expect("parse maintenance compact");

        match cli {
            Cli::Maintenance {
                command:
                    MaintenanceCommands::Compact {
                        sqlite_path,
                        vacuum,
                    },
            } => {
                assert_eq!(sqlite_path, "runs/tianji.sqlite3");
                assert!(vacuum);
            }
            _ => panic!("expected Maintenance::Compact variant"),
        }
    }

    #[tokio::test]
    async fn maintenance_retain_outputs_report_and_prunes_history() {
        let db_path = temp_sqlite_path("maintenance_retain");
        for _ in 0..3 {
            run(Cli::Run {
                fixture: SAMPLE_FIXTURE.to_string(),
                sqlite_path: Some(db_path.clone()),
                show_delta: false,
            })
            .await
            .expect("seed run");
        }

        let output = run(Cli::Maintenance {
            command: MaintenanceCommands::Retain {
                sqlite_path: db_path.clone(),
                keep_last_runs: 2,
            },
        })
        .await
        .expect("maintenance retain output");
        let value: Value = serde_json::from_str(&output).expect("retention json");

        assert_eq!(value["schema_version"], "tianji.retention-report.v1");
        assert_eq!(value["sqlite_path"], db_path);
        assert_eq!(value["keep_last_runs"], 2);
        assert_eq!(value["runs_before"], 3);
        assert_eq!(value["runs_after"], 2);
        assert_eq!(value["deleted_runs"], 1);

        let runs = list_runs(&db_path, 10, &RunListFilters::default()).expect("list runs");
        let run_ids: Vec<i64> = runs
            .iter()
            .map(|run| run["run_id"].as_i64().expect("run id"))
            .collect();
        assert_eq!(run_ids, vec![3, 2]);

        cleanup_sqlite_path(&db_path);
    }

    #[tokio::test]
    async fn maintenance_check_backup_export_compact_execute_via_cli() {
        let db_path = temp_sqlite_path("maintenance_execute");
        let backup_path = temp_sqlite_path("maintenance_execute_backup");
        let export_path = temp_sqlite_path("maintenance_execute_export");
        cleanup_sqlite_path(&backup_path);
        cleanup_sqlite_path(&export_path);
        for _ in 0..2 {
            run(Cli::Run {
                fixture: SAMPLE_FIXTURE.to_string(),
                sqlite_path: Some(db_path.clone()),
                show_delta: false,
            })
            .await
            .expect("seed run");
        }

        let check_output = run(Cli::Maintenance {
            command: MaintenanceCommands::Check {
                sqlite_path: db_path.clone(),
            },
        })
        .await
        .expect("maintenance check output");
        let check_value: Value = serde_json::from_str(&check_output).expect("check json");
        assert_eq!(
            check_value["schema_version"],
            "tianji.maintenance-check-report.v1"
        );
        assert_eq!(check_value["latest_run_id"], 2);

        let backup_output = run(Cli::Maintenance {
            command: MaintenanceCommands::Backup {
                sqlite_path: db_path.clone(),
                output: backup_path.clone(),
                overwrite: false,
            },
        })
        .await
        .expect("maintenance backup output");
        let backup_value: Value = serde_json::from_str(&backup_output).expect("backup json");
        assert_eq!(backup_value["schema_version"], "tianji.backup-report.v1");
        assert_eq!(backup_value["run_count"], 2);
        let backup_runs = list_runs(&backup_path, 10, &RunListFilters::default())
            .expect("backup readable from cli test");
        assert_eq!(backup_runs.len(), 2);

        let export_output = run(Cli::Maintenance {
            command: MaintenanceCommands::Export {
                sqlite_path: db_path.clone(),
                output: export_path.clone(),
                format: MaintenanceExportFormat::Jsonl,
                include_details: false,
                overwrite: false,
            },
        })
        .await
        .expect("maintenance export output");
        let export_value: Value = serde_json::from_str(&export_output).expect("export json");
        assert_eq!(export_value["schema_version"], "tianji.export-report.v1");
        assert_eq!(export_value["run_count"], 2);
        assert_eq!(
            std::fs::read_to_string(&export_path)
                .expect("export file")
                .lines()
                .count(),
            3
        );

        let compact_output = run(Cli::Maintenance {
            command: MaintenanceCommands::Compact {
                sqlite_path: db_path.clone(),
                vacuum: false,
            },
        })
        .await
        .expect("maintenance compact output");
        let compact_value: Value = serde_json::from_str(&compact_output).expect("compact json");
        assert_eq!(compact_value["schema_version"], "tianji.compact-report.v1");
        let compacted_runs = list_runs(&db_path, 10, &RunListFilters::default())
            .expect("compacted readable from cli test");
        assert_eq!(compacted_runs.len(), 2);

        cleanup_sqlite_path(&db_path);
        cleanup_sqlite_path(&backup_path);
        let _ = std::fs::remove_file(&export_path);
    }

    #[test]
    fn doctor_missing_config_succeeds_with_warning() {
        let path = temp_doctor_path("missing");
        cleanup_doctor_path(&path);

        let report = build_doctor_report(Some(&path.to_string_lossy()), None).expect("report");

        assert!(report.ok);
        assert!(!report.config_present);
        assert!(report.checks.iter().any(|check| {
            check.name == "config_present" && check.severity == DoctorSeverity::Warning
        }));
    }

    #[test]
    fn doctor_malformed_config_returns_error() {
        let path = write_doctor_config("malformed", "providers: [not: valid: yaml");

        let result = build_doctor_report(Some(&path.to_string_lossy()), None);

        assert!(
            matches!(result, Err(TianJiError::Usage(message)) if message.contains("Failed to parse config YAML"))
        );
        cleanup_doctor_path(&path);
    }

    #[test]
    fn doctor_valid_config_reports_providers_and_json_without_secrets() {
        std::env::set_var("TIANJI_DOCTOR_SET_KEY", "secret-do-not-print");
        let path = write_doctor_config(
            "valid",
            r#"
providers:
  local:
    type: ollama
    model: qwen3:14b
    base_url: http://localhost:11434
    max_concurrency: 2
  remote:
    type: openai
    model: gpt-4o
    api_key_env: TIANJI_DOCTOR_SET_KEY
    api_key: inline-secret-do-not-print
    fallback: local
agent_model_map:
  forward_default: local
  backward_fine: remote
"#,
        );

        let output = handle_doctor(Some(&path.to_string_lossy()), None, true).expect("json output");
        let value: Value = serde_json::from_str(&output).expect("doctor json");

        assert_eq!(value["ok"], true);
        assert_eq!(value["provider_count"], 2);
        assert!(output.contains("inline_api_key_present"));
        assert!(!output.contains("secret-do-not-print"));
        assert!(!output.contains("inline-secret-do-not-print"));

        std::env::remove_var("TIANJI_DOCTOR_SET_KEY");
        cleanup_doctor_path(&path);
    }

    #[test]
    fn doctor_missing_env_is_warning_without_secret_value() {
        std::env::remove_var("TIANJI_DOCTOR_MISSING_KEY");
        let path = write_doctor_config(
            "missing_env",
            r#"
providers:
  remote:
    type: openai
    model: gpt-4o
    api_key_env: TIANJI_DOCTOR_MISSING_KEY
"#,
        );

        let report = build_doctor_report(Some(&path.to_string_lossy()), None).expect("report");
        let output = format_doctor_report(&report);

        assert!(report.ok);
        assert!(report.checks.iter().any(|check| {
            check.name == "provider.remote.api_key_env"
                && check.severity == DoctorSeverity::Warning
                && check.message.contains("TIANJI_DOCTOR_MISSING_KEY")
        }));
        assert!(!output.contains("sk-"));
        cleanup_doctor_path(&path);
    }

    #[test]
    fn doctor_bad_references_and_concurrency_fail_report() {
        let path = write_doctor_config(
            "bad_refs",
            r#"
providers:
  remote:
    type: openai
    model: gpt-4o
    max_concurrency: 0
    fallback: missing_provider
agent_model_map:
  forward_default: another_missing_provider
"#,
        );

        let report = build_doctor_report(Some(&path.to_string_lossy()), None).expect("report");

        assert!(!report.ok);
        assert!(report
            .checks
            .iter()
            .any(|check| check.name == "provider.remote.max_concurrency"
                && check.severity == DoctorSeverity::Error));
        assert!(report
            .checks
            .iter()
            .any(|check| check.name == "provider.remote.fallback"
                && check.severity == DoctorSeverity::Error));
        assert!(report
            .checks
            .iter()
            .any(|check| check.name == "agent_model_map.forward_default"
                && check.severity == DoctorSeverity::Error));
        cleanup_doctor_path(&path);
    }

    #[test]
    fn doctor_sqlite_parent_check_reports_ready_path() {
        let path = temp_doctor_path("missing_with_sqlite");
        cleanup_doctor_path(&path);
        let sqlite_path = std::env::temp_dir().join(format!(
            "tianji_doctor_{}_{}.sqlite3",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .expect("system time after epoch")
                .as_nanos()
        ));

        let report = build_doctor_report(
            Some(&path.to_string_lossy()),
            Some(&sqlite_path.to_string_lossy()),
        )
        .expect("report");

        assert!(report
            .checks
            .iter()
            .any(|check| { check.name == "sqlite_path" && check.severity == DoctorSeverity::Ok }));
    }

    #[test]
    fn cli_parse_tui_with_simulate() {
        let cli = Cli::try_parse_from([
            "tianji",
            "tui",
            "--sqlite-path",
            "runs/tianji.sqlite3",
            "--simulate",
            "east-asia.conflict:30",
        ])
        .expect("parse tui with simulate");
        match cli {
            Cli::Tui {
                sqlite_path,
                limit,
                simulate,
                interactive,
            } => {
                assert_eq!(sqlite_path, "runs/tianji.sqlite3");
                assert_eq!(limit, 20);
                assert_eq!(simulate, Some("east-asia.conflict:30".to_string()));
                assert!(!interactive);
            }
            _ => panic!("expected Tui variant"),
        }
    }

    #[test]
    fn cli_parse_tui_without_simulate() {
        let cli = Cli::try_parse_from(["tianji", "tui", "--sqlite-path", "runs/tianji.sqlite3"])
            .expect("parse tui without simulate");
        match cli {
            Cli::Tui {
                sqlite_path,
                simulate,
                ..
            } => {
                assert_eq!(sqlite_path, "runs/tianji.sqlite3");
                assert!(simulate.is_none());
            }
            _ => panic!("expected Tui variant"),
        }
    }
}

fn validate_score_range(
    min: Option<f64>,
    max: Option<f64>,
    min_flag: &str,
    max_flag: &str,
) -> Result<(), TianJiError> {
    match (min, max) {
        (Some(mn), Some(mx)) if mn > mx => Err(TianJiError::Usage(format!(
            "{min_flag} cannot be greater than {max_flag}."
        ))),
        _ => Ok(()),
    }
}

fn validate_int_range(
    min: Option<i64>,
    max: Option<i64>,
    min_flag: &str,
    max_flag: &str,
) -> Result<(), TianJiError> {
    match (min, max) {
        (Some(mn), Some(mx)) if mn > mx => Err(TianJiError::Usage(format!(
            "{min_flag} cannot be greater than {max_flag}."
        ))),
        _ => Ok(()),
    }
}

fn parse_field_constraint(
    s: &str,
) -> Result<(tianji::worldline::types::FieldKey, f64, f64), String> {
    let parts: Vec<&str> = s.split(':').collect();
    if parts.len() != 3 {
        return Err(format!(
            "field-constraint must be in region.domain:min:max format, got: {s}"
        ));
    }
    let field_parts: Vec<&str> = parts[0].split('.').collect();
    if field_parts.len() != 2 {
        return Err(format!(
            "field must be in region.domain format, got: {}",
            parts[0]
        ));
    }
    let region = field_parts[0].to_string();
    let domain = field_parts[1].to_string();
    let min =
        f64::from_str(parts[1]).map_err(|e| format!("invalid min value '{}': {e}", parts[1]))?;
    let max =
        f64::from_str(parts[2]).map_err(|e| format!("invalid max value '{}': {e}", parts[2]))?;
    if min > max {
        return Err(format!("min ({min}) cannot be greater than max ({max})"));
    }
    Ok((
        tianji::worldline::types::FieldKey { region, domain },
        min,
        max,
    ))
}

// ---------------------------------------------------------------------------
// PID file management (for daemon start/stop)
// ---------------------------------------------------------------------------

use std::os::fd::AsRawFd;
fn pid_file_for_socket(socket_path: &str) -> PathBuf {
    let socket_file = PathBuf::from(socket_path);
    let file_name = socket_file
        .file_name()
        .map(|n| n.to_string_lossy().to_string())
        .unwrap_or_else(|| "tianji.sock".to_string());
    let dir = socket_file
        .parent()
        .unwrap_or_else(|| std::path::Path::new("."));
    dir.join(format!("{file_name}.pid"))
}

fn pid_lock_file_for_socket(socket_path: &str) -> PathBuf {
    let mut path = pid_file_for_socket(socket_path);
    path.set_extension("pid.lock");
    path
}

fn acquire_pid_lock(socket_path: &str) -> Result<std::fs::File, TianJiError> {
    let lock_path = pid_lock_file_for_socket(socket_path);
    if let Some(parent) = lock_path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let lock_file = std::fs::OpenOptions::new()
        .create(true)
        .truncate(false)
        .read(true)
        .write(true)
        .open(&lock_path)?;
    let rc = unsafe { libc::flock(lock_file.as_raw_fd(), libc::LOCK_EX) };
    if rc == 0 {
        Ok(lock_file)
    } else {
        Err(TianJiError::Io(std::io::Error::last_os_error()))
    }
}

fn read_pid_file(socket_path: &str) -> Result<Option<u32>, TianJiError> {
    let pid_file = pid_file_for_socket(socket_path);
    if !pid_file.exists() {
        return Ok(None);
    }
    let raw_value = std::fs::read_to_string(&pid_file)?;
    let trimmed = raw_value.trim();
    if trimmed.is_empty() {
        return Ok(None);
    }
    trimmed
        .parse::<u32>()
        .map(Some)
        .map_err(|_| TianJiError::Usage(format!("Daemon pid file is malformed: {pid_file:?}")))
}

fn write_pid_file(socket_path: &str, pid: u32) -> Result<(), TianJiError> {
    let pid_file = pid_file_for_socket(socket_path);
    if let Some(parent) = pid_file.parent() {
        std::fs::create_dir_all(parent)?;
    }
    std::fs::write(&pid_file, format!("{pid}\n"))?;
    Ok(())
}

fn remove_pid_file(socket_path: &str) {
    let pid_file = pid_file_for_socket(socket_path);
    let _ = std::fs::remove_file(pid_file);
}

fn is_pid_running(pid: u32) -> bool {
    // Send signal 0 to check if process exists
    if unsafe { libc::kill(pid as i32, 0) == 0 } {
        return true;
    }
    std::io::Error::last_os_error().raw_os_error() == Some(libc::EPERM)
}

fn wait_for_socket(socket_path: &str, timeout_secs: f64) -> bool {
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs_f64(timeout_secs);
    while std::time::Instant::now() < deadline {
        if std::path::Path::new(socket_path).exists() {
            return true;
        }
        std::thread::sleep(std::time::Duration::from_millis(100));
    }
    false
}

fn wait_for_api(host: &str, port: u16, timeout_secs: f64) -> bool {
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs_f64(timeout_secs);
    let url = api_readiness_url(host, port);
    while std::time::Instant::now() < deadline {
        if let Ok(resp) = reqwest::blocking::Client::new()
            .get(&url)
            .timeout(std::time::Duration::from_millis(500))
            .send()
        {
            if resp.status().is_success() {
                return true;
            }
        }
        std::thread::sleep(std::time::Duration::from_millis(100));
    }
    false
}

fn api_readiness_url(host: &str, port: u16) -> String {
    format!(
        "{}/api/v1/ready",
        tianji::daemon::loopback_http_base_url(host, port)
    )
}

// ---------------------------------------------------------------------------
// Daemon start / stop / status / run handlers
// ---------------------------------------------------------------------------

fn handle_daemon_start(
    socket_path: &str,
    sqlite_path: &str,
    host: &str,
    port: u16,
) -> Result<String, TianJiError> {
    let validated_host = tianji::daemon::validate_loopback_host(host)?;

    let start_timeout_secs = 2.0;
    let _pid_lock = acquire_pid_lock(socket_path)?;

    // Check existing PID
    if let Some(existing_pid) = read_pid_file(socket_path)? {
        if is_pid_running(existing_pid) {
            return Err(TianJiError::Usage(format!(
                "Daemon already appears to be running for {socket_path} with pid {existing_pid}."
            )));
        }
        remove_pid_file(socket_path);
    }

    // Ensure socket parent dir exists
    if let Some(parent) = std::path::Path::new(socket_path).parent() {
        std::fs::create_dir_all(parent)?;
    }

    // Spawn the daemon serve subprocess
    let current_exe = std::env::current_exe()?;
    let mut cmd = std::process::Command::new(current_exe);
    cmd.args(["daemon", "serve"])
        .arg("--socket-path")
        .arg(socket_path)
        .arg("--sqlite-path")
        .arg(sqlite_path)
        .arg("--host")
        .arg(&validated_host)
        .arg("--port")
        .arg(port.to_string());

    // Redirect child stdout/stderr to a log file next to the socket path
    // so daemon crashes and panics are captured for diagnostics.
    let log_path = format!("{socket_path}.log");
    let log_file = std::fs::File::create(&log_path)?;
    let log_file_err = log_file.try_clone()?;
    cmd.stdout(std::process::Stdio::from(log_file))
        .stderr(std::process::Stdio::from(log_file_err));

    // Set process group (start_new_session equivalent)
    unsafe {
        cmd.pre_exec(|| {
            libc::setsid();
            Ok(())
        });
    }

    let mut child = cmd.spawn()?;
    let pid = child.id();
    write_pid_file(socket_path, pid)?;

    // Wait for socket
    if !wait_for_socket(socket_path, start_timeout_secs) {
        remove_pid_file(socket_path);
        terminate_child(&mut child, start_timeout_secs);
        return Err(TianJiError::Usage(format!(
            "Daemon did not become ready within {start_timeout_secs:.1}s for socket {socket_path}."
        )));
    }

    // Wait for API
    if !wait_for_api(&validated_host, port, start_timeout_secs) {
        remove_pid_file(socket_path);
        terminate_child(&mut child, start_timeout_secs);
        return Err(TianJiError::Usage(format!(
            "Daemon HTTP API did not become ready within {start_timeout_secs:.1}s at {}.",
            api_readiness_url(&validated_host, port)
        )));
    }

    // Intentionally detach from the child: the daemon must outlive the
    // parent CLI process. Dropping Child without wait() leaves a zombie;
    // mem::forget leaks the handle on purpose so the OS reaps the child
    // only when it eventually exits.
    std::mem::forget(child);

    let api_base_url = tianji::daemon::loopback_http_base_url(&validated_host, port);

    let payload = serde_json::json!({
        "socket_path": socket_path,
        "sqlite_path": sqlite_path,
        "pid": pid,
        "host": validated_host,
        "port": port,
        "api_base_url": api_base_url,
        "running": true,
    });
    Ok(serde_json::to_string_pretty(&payload)?)
}

fn terminate_child(child: &mut std::process::Child, timeout_secs: f64) {
    let pid = child.id() as i32;
    unsafe {
        libc::kill(pid, libc::SIGTERM);
    }
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs_f64(timeout_secs);
    while std::time::Instant::now() < deadline {
        if matches!(child.try_wait(), Ok(Some(_))) {
            return;
        }
        std::thread::sleep(std::time::Duration::from_millis(50));
    }
    let _ = child.kill();
    let _ = child.wait();
}

fn handle_daemon_stop(socket_path: &str) -> Result<String, TianJiError> {
    let stop_timeout_secs = 2.0;
    let poll_interval = std::time::Duration::from_millis(100);
    let _pid_lock = acquire_pid_lock(socket_path)?;

    let pid = read_pid_file(socket_path)?.ok_or_else(|| {
        TianJiError::Usage(format!(
            "No daemon pid file found for socket {socket_path}. Start the daemon first."
        ))
    })?;

    if !is_pid_running(pid) {
        remove_pid_file(socket_path);
        return Err(TianJiError::Usage(format!(
            "Daemon pid {pid} is not running."
        )));
    }

    // SIGTERM
    unsafe {
        libc::kill(pid as i32, libc::SIGTERM);
    }

    let deadline =
        std::time::Instant::now() + std::time::Duration::from_secs_f64(stop_timeout_secs);
    while std::time::Instant::now() < deadline {
        if !is_pid_running(pid) {
            remove_pid_file(socket_path);
            let _ = std::fs::remove_file(socket_path);
            let payload = serde_json::json!({
                "socket_path": socket_path,
                "pid": pid,
                "running": false,
            });
            return Ok(serde_json::to_string_pretty(&payload)?);
        }
        std::thread::sleep(poll_interval);
    }

    // SIGKILL
    unsafe {
        libc::kill(pid as i32, libc::SIGKILL);
    }

    let deadline =
        std::time::Instant::now() + std::time::Duration::from_secs_f64(stop_timeout_secs);
    while std::time::Instant::now() < deadline {
        if !is_pid_running(pid) {
            remove_pid_file(socket_path);
            let _ = std::fs::remove_file(socket_path);
            let payload = serde_json::json!({
                "socket_path": socket_path,
                "pid": pid,
                "running": false,
            });
            return Ok(serde_json::to_string_pretty(&payload)?);
        }
        std::thread::sleep(poll_interval);
    }

    Err(TianJiError::Usage(format!(
        "Daemon pid {pid} did not stop within {stop_timeout_secs:.1}s."
    )))
}

fn handle_daemon_status(socket_path: &str, job_id: Option<&str>) -> Result<String, TianJiError> {
    let pid = read_pid_file(socket_path)?;
    let running = pid
        .map(|p| is_pid_running(p) && std::path::Path::new(socket_path).exists())
        .unwrap_or(false);

    if let Some(jid) = job_id {
        let payload = serde_json::json!({
            "action": "job_status",
            "job_id": jid,
        });
        let response = tianji::daemon::send_daemon_request(socket_path, &payload)?;
        let ok = response
            .get("ok")
            .and_then(|v| v.as_bool())
            .unwrap_or(false);
        if ok {
            let data = response
                .get("data")
                .cloned()
                .unwrap_or(serde_json::Value::Null);
            return Ok(serde_json::to_string_pretty(&data)?);
        }
        let error_msg = response
            .get("error")
            .and_then(|e| e.get("message"))
            .and_then(|m| m.as_str())
            .unwrap_or("Unknown error");
        return Err(TianJiError::Usage(error_msg.to_string()));
    }

    let job_states: Vec<&str> = tianji::daemon::ALLOWED_JOB_STATES.to_vec();
    let payload = serde_json::json!({
        "socket_path": socket_path,
        "pid": pid,
        "running": running,
        "job_states": job_states,
    });
    Ok(serde_json::to_string_pretty(&payload)?)
}

fn handle_daemon_run(
    socket_path: &str,
    fixture: &str,
    sqlite_path: Option<&str>,
) -> Result<String, TianJiError> {
    let data = queue_daemon_run(socket_path, fixture, sqlite_path)?;
    Ok(serde_json::to_string_pretty(&data)?)
}

fn build_daemon_run_payload(fixture: &str, sqlite_path: Option<&str>) -> serde_json::Value {
    let mut run_payload = serde_json::json!({
        "fixture_paths": [fixture],
    });
    if let Some(sp) = sqlite_path {
        run_payload["sqlite_path"] = serde_json::Value::String(sp.to_string());
    }
    run_payload
}

fn queue_daemon_run(
    socket_path: &str,
    fixture: &str,
    sqlite_path: Option<&str>,
) -> Result<serde_json::Value, TianJiError> {
    let payload = serde_json::json!({
        "action": "queue_run",
        "payload": build_daemon_run_payload(fixture, sqlite_path),
    });

    let response = tianji::daemon::send_daemon_request(socket_path, &payload)?;
    let ok = response
        .get("ok")
        .and_then(|v| v.as_bool())
        .unwrap_or(false);
    if ok {
        return Ok(response
            .get("data")
            .cloned()
            .unwrap_or(serde_json::Value::Null));
    }
    let error_msg = response
        .get("error")
        .and_then(|e| e.get("message"))
        .and_then(|m| m.as_str())
        .unwrap_or("Daemon returned an invalid error response.");
    Err(TianJiError::Usage(error_msg.to_string()))
}

fn handle_daemon_schedule(
    socket_path: &str,
    fixture: &str,
    sqlite_path: Option<&str>,
    every_seconds: u64,
    count: usize,
) -> Result<String, TianJiError> {
    handle_daemon_schedule_with(
        socket_path,
        fixture,
        sqlite_path,
        every_seconds,
        count,
        queue_daemon_run,
        std::thread::sleep,
    )
}

fn handle_daemon_schedule_with<Q, S>(
    socket_path: &str,
    fixture: &str,
    sqlite_path: Option<&str>,
    every_seconds: u64,
    count: usize,
    mut queue_run: Q,
    mut sleep: S,
) -> Result<String, TianJiError>
where
    Q: FnMut(&str, &str, Option<&str>) -> Result<serde_json::Value, TianJiError>,
    S: FnMut(std::time::Duration),
{
    if every_seconds < 60 {
        return Err(TianJiError::Usage(
            "--every-seconds must be at least 60.".to_string(),
        ));
    }
    if count < 1 {
        return Err(TianJiError::Usage(
            "--count must be at least 1.".to_string(),
        ));
    }

    let mut queued_runs = Vec::with_capacity(count);
    for index in 0..count {
        queued_runs.push(queue_run(socket_path, fixture, sqlite_path)?);
        if index + 1 < count {
            sleep(std::time::Duration::from_secs(every_seconds));
        }
    }

    let job_states: Vec<&str> = tianji::daemon::ALLOWED_JOB_STATES.to_vec();
    let payload = serde_json::json!({
        "schedule": {
            "every_seconds": every_seconds,
            "count": count,
        },
        "queued_runs": queued_runs,
        "job_states": job_states,
    });
    Ok(serde_json::to_string_pretty(&payload)?)
}

// ---------------------------------------------------------------------------
// Predict / Backtrack / Baseline / Watch / Doctor handlers
// ---------------------------------------------------------------------------

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
enum DoctorSeverity {
    Ok,
    Warning,
    Error,
}

impl DoctorSeverity {
    fn as_str(&self) -> &'static str {
        match self {
            Self::Ok => "ok",
            Self::Warning => "warning",
            Self::Error => "error",
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
struct DoctorCheck {
    name: String,
    severity: DoctorSeverity,
    message: String,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
struct DoctorProviderReport {
    name: String,
    provider_type: String,
    model: String,
    max_concurrency: usize,
    base_url_present: bool,
    api_key_env: Option<String>,
    api_key_env_present: Option<bool>,
    inline_api_key_present: bool,
    fallback: Option<String>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
struct DoctorAgentMappingReport {
    agent: String,
    provider: String,
    provider_exists: bool,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
struct DoctorReport {
    ok: bool,
    config_path: String,
    config_present: bool,
    provider_count: usize,
    providers: Vec<DoctorProviderReport>,
    agent_model_map: Vec<DoctorAgentMappingReport>,
    sqlite_path: Option<String>,
    checks: Vec<DoctorCheck>,
}

fn handle_doctor(
    config_path: Option<&str>,
    sqlite_path: Option<&str>,
    json: bool,
) -> Result<String, TianJiError> {
    let report = build_doctor_report(config_path, sqlite_path)?;
    if json {
        serde_json::to_string_pretty(&report).map_err(TianJiError::Json)
    } else {
        Ok(format_doctor_report(&report))
    }
}

fn build_doctor_report(
    config_path: Option<&str>,
    sqlite_path: Option<&str>,
) -> Result<DoctorReport, TianJiError> {
    let config_path = config_path
        .map(PathBuf::from)
        .unwrap_or_else(tianji::llm::TianJiConfig::default_path);
    let config_path_text = config_path.to_string_lossy().to_string();
    let mut checks = Vec::new();
    let mut providers = Vec::new();
    let mut mappings = Vec::new();

    if !config_path.exists() {
        checks.push(DoctorCheck {
            name: "config_present".to_string(),
            severity: DoctorSeverity::Warning,
            message: format!(
                "Config file not found at {config_path_text}; deterministic mode can run without LLM config."
            ),
        });
        if let Some(sqlite_path) = sqlite_path {
            check_sqlite_path(sqlite_path, &mut checks);
        }
        return Ok(DoctorReport {
            ok: true,
            config_path: config_path_text,
            config_present: false,
            provider_count: 0,
            providers,
            agent_model_map: mappings,
            sqlite_path: sqlite_path.map(str::to_string),
            checks,
        });
    }

    checks.push(DoctorCheck {
        name: "config_present".to_string(),
        severity: DoctorSeverity::Ok,
        message: format!("Config file found at {config_path_text}."),
    });

    let raw_config = std::fs::read_to_string(&config_path).map_err(|error| {
        TianJiError::Usage(format!(
            "Failed to read config file {config_path_text}: {error}"
        ))
    })?;
    let config: tianji::llm::TianJiConfig = serde_yaml::from_str(&raw_config).map_err(|error| {
        TianJiError::Usage(format!(
            "Failed to parse config YAML at {config_path_text}: {error}"
        ))
    })?;
    checks.push(DoctorCheck {
        name: "config_parse".to_string(),
        severity: DoctorSeverity::Ok,
        message: "Config YAML parsed successfully.".to_string(),
    });

    if config.providers.is_empty() {
        checks.push(DoctorCheck {
            name: "providers".to_string(),
            severity: DoctorSeverity::Warning,
            message: "No providers configured; deterministic mode remains available.".to_string(),
        });
    } else {
        checks.push(DoctorCheck {
            name: "providers".to_string(),
            severity: DoctorSeverity::Ok,
            message: format!("{} provider(s) configured.", config.providers.len()),
        });
    }

    let provider_names: BTreeSet<_> = config.providers.keys().cloned().collect();
    for (name, provider) in &config.providers {
        let model = provider.model.trim();
        if model.is_empty() {
            checks.push(DoctorCheck {
                name: format!("provider.{name}.model"),
                severity: DoctorSeverity::Error,
                message: format!("Provider {name} must define a non-empty model."),
            });
        }
        if provider.max_concurrency < 1 {
            checks.push(DoctorCheck {
                name: format!("provider.{name}.max_concurrency"),
                severity: DoctorSeverity::Error,
                message: format!("Provider {name} max_concurrency must be at least 1."),
            });
        }
        if let Some(env_var) = provider.api_key_env.as_deref() {
            match std::env::var(env_var) {
                Ok(value) if !value.is_empty() => checks.push(DoctorCheck {
                    name: format!("provider.{name}.api_key_env"),
                    severity: DoctorSeverity::Ok,
                    message: format!("Provider {name} env var {env_var} is set."),
                }),
                _ => checks.push(DoctorCheck {
                    name: format!("provider.{name}.api_key_env"),
                    severity: DoctorSeverity::Warning,
                    message: format!("Provider {name} env var {env_var} is missing or empty."),
                }),
            }
        }
        if provider.api_key.is_some() {
            checks.push(DoctorCheck {
                name: format!("provider.{name}.api_key"),
                severity: DoctorSeverity::Ok,
                message: format!(
                    "Provider {name} has an inline API key configured (value hidden)."
                ),
            });
        }
        if let Some(fallback) = provider.fallback.as_deref() {
            if provider_names.contains(fallback) {
                checks.push(DoctorCheck {
                    name: format!("provider.{name}.fallback"),
                    severity: DoctorSeverity::Ok,
                    message: format!("Provider {name} fallback references {fallback}."),
                });
            } else {
                checks.push(DoctorCheck {
                    name: format!("provider.{name}.fallback"),
                    severity: DoctorSeverity::Error,
                    message: format!(
                        "Provider {name} fallback references unknown provider {fallback}."
                    ),
                });
            }
        }

        providers.push(DoctorProviderReport {
            name: name.clone(),
            provider_type: match provider.provider_type {
                tianji::llm::ProviderType::OpenAI => "openai".to_string(),
                tianji::llm::ProviderType::Ollama => "ollama".to_string(),
            },
            model: provider.model.clone(),
            max_concurrency: provider.max_concurrency,
            base_url_present: provider.base_url.is_some(),
            api_key_env: provider.api_key_env.clone(),
            api_key_env_present: provider.api_key_env.as_deref().map(|env_var| {
                std::env::var(env_var)
                    .map(|value| !value.is_empty())
                    .unwrap_or(false)
            }),
            inline_api_key_present: provider.api_key.is_some(),
            fallback: provider.fallback.clone(),
        });
    }

    for (agent, provider_name) in &config.agent_model_map {
        let provider_exists = provider_names.contains(provider_name);
        checks.push(DoctorCheck {
            name: format!("agent_model_map.{agent}"),
            severity: if provider_exists {
                DoctorSeverity::Ok
            } else {
                DoctorSeverity::Error
            },
            message: if provider_exists {
                format!("Agent {agent} maps to provider {provider_name}.")
            } else {
                format!("Agent {agent} maps to unknown provider {provider_name}.")
            },
        });
        mappings.push(DoctorAgentMappingReport {
            agent: agent.clone(),
            provider: provider_name.clone(),
            provider_exists,
        });
    }

    if let Some(sqlite_path) = sqlite_path {
        check_sqlite_path(sqlite_path, &mut checks);
    }

    let ok = !checks
        .iter()
        .any(|check| check.severity == DoctorSeverity::Error);
    Ok(DoctorReport {
        ok,
        config_path: config_path_text,
        config_present: true,
        provider_count: providers.len(),
        providers,
        agent_model_map: mappings,
        sqlite_path: sqlite_path.map(str::to_string),
        checks,
    })
}

fn check_sqlite_path(sqlite_path: &str, checks: &mut Vec<DoctorCheck>) {
    let path = Path::new(sqlite_path);
    let parent = path.parent().unwrap_or_else(|| Path::new("."));
    let severity = if parent.exists() {
        match std::fs::metadata(parent) {
            Ok(metadata) if metadata.is_dir() && !metadata.permissions().readonly() => {
                DoctorSeverity::Ok
            }
            Ok(metadata) if !metadata.is_dir() => DoctorSeverity::Error,
            Ok(_) => DoctorSeverity::Warning,
            Err(_) => DoctorSeverity::Error,
        }
    } else if parent.parent().is_some_and(Path::exists) {
        DoctorSeverity::Ok
    } else {
        DoctorSeverity::Error
    };
    let message = match severity {
        DoctorSeverity::Ok if parent.exists() => {
            format!("SQLite parent directory is ready: {}.", parent.display())
        }
        DoctorSeverity::Ok => format!(
            "SQLite parent directory can be created from existing parent: {}.",
            parent.display()
        ),
        DoctorSeverity::Warning => format!(
            "SQLite parent directory exists but may not be writable: {}.",
            parent.display()
        ),
        DoctorSeverity::Error => format!(
            "SQLite parent directory is not ready or creatable: {}.",
            parent.display()
        ),
    };
    checks.push(DoctorCheck {
        name: "sqlite_path".to_string(),
        severity,
        message,
    });
}

fn format_doctor_report(report: &DoctorReport) -> String {
    let mut lines = Vec::new();
    lines.push(format!(
        "TianJi doctor: {}",
        if report.ok { "ok" } else { "errors found" }
    ));
    lines.push(format!("Config: {}", report.config_path));
    lines.push(format!("Config present: {}", report.config_present));
    lines.push(format!("Providers: {}", report.provider_count));
    for provider in &report.providers {
        lines.push(format!(
            "- provider {}: type={}, model={}, max_concurrency={}, api_key_env={}, inline_api_key_present={}, fallback={}",
            provider.name,
            provider.provider_type,
            provider.model,
            provider.max_concurrency,
            provider.api_key_env.as_deref().unwrap_or("<none>"),
            provider.inline_api_key_present,
            provider.fallback.as_deref().unwrap_or("<none>")
        ));
    }
    if !report.agent_model_map.is_empty() {
        lines.push("Agent model map:".to_string());
        for mapping in &report.agent_model_map {
            lines.push(format!(
                "- {} -> {} ({})",
                mapping.agent,
                mapping.provider,
                if mapping.provider_exists {
                    "ok"
                } else {
                    "missing"
                }
            ));
        }
    }
    if let Some(sqlite_path) = &report.sqlite_path {
        lines.push(format!("SQLite path: {sqlite_path}"));
    }
    lines.push("Checks:".to_string());
    for check in &report.checks {
        lines.push(format!(
            "- [{}] {}: {}",
            check.severity.as_str(),
            check.name,
            check.message
        ));
    }
    lines.join("\n")
}

async fn handle_predict(
    field: &str,
    horizon: u64,
    profile_dir: &str,
    config_path: Option<&str>,
    trace_jsonl: Option<&Path>,
) -> Result<String, TianJiError> {
    use tianji::hongmeng::Agent;
    use tianji::llm::ProviderRegistry;
    use tianji::nuwa::forward::run_forward_with_trace;
    use tianji::nuwa::sandbox::SimulationMode;
    use tianji::profile::ProfileRegistry;
    use tianji::worldline::types::{FieldKey, Worldline};

    // Parse target field
    let field_parts: Vec<&str> = field.split('.').collect();
    if field_parts.len() != 2 {
        return Err(TianJiError::Usage(format!(
            "--field must be in region.domain format, got: {field}"
        )));
    }
    let target_field = FieldKey {
        region: field_parts[0].to_string(),
        domain: field_parts[1].to_string(),
    };

    // Load profiles (stub: create a default agent if no profiles found)
    let agents = match ProfileRegistry::load_from_dir(std::path::Path::new(profile_dir)) {
        Ok(registry) if !registry.profiles.is_empty() => registry
            .profiles
            .values()
            .map(|p| Agent::from_profile(p.clone()))
            .collect::<Vec<_>>(),
        _ => {
            // Fallback: one stub agent
            let stub_profile = tianji::profile::types::ActorProfile {
                id: "stub".to_string(),
                name: "Stub Agent".to_string(),
                tier: tianji::profile::types::ActorTier::Nation,
                interests: vec![],
                red_lines: vec![],
                capabilities: tianji::profile::types::Capabilities::default(),
                behavior_patterns: vec!["observe".to_string(), "diplomatic_protest".to_string()],
                historical_analogues: vec![],
            };
            vec![Agent::from_profile(stub_profile)]
        }
    };

    // Load config (or default)
    let tianji_config = match config_path {
        Some(path) => tianji::llm::TianJiConfig::load_from(path)
            .map_err(|e| TianJiError::Usage(format!("Failed to load config: {e}")))?,
        None => tianji::llm::TianJiConfig::default(),
    };
    let provider = ProviderRegistry::from_config(tianji_config)
        .map_err(|e| TianJiError::Usage(format!("Failed to create provider registry: {e}")))?;

    // Create stub worldline with the target field
    let mut fields = BTreeMap::new();
    fields.insert(target_field.clone(), 3.5);
    let hash = Worldline::compute_snapshot_hash(&fields);
    let worldline = Worldline {
        id: 0,
        fields,
        events: vec![],
        causal_graph: petgraph::graph::DiGraph::new(),
        active_actors: std::collections::BTreeSet::new(),
        divergence: 0.0,
        parent: None,
        diverge_tick: 0,
        snapshot_hash: hash,
        created_at: chrono::Utc::now(),
    };

    let mode = SimulationMode::Forward {
        target_field,
        horizon_ticks: horizon,
    };
    let sim_config = tianji::hongmeng::HongmengConfig::default();

    // Build sandbox to validate (unused directly — run_forward is standalone)
    let _sandbox = tianji::nuwa::sandbox::NuwaSandbox::new(
        worldline.clone(),
        agents.clone(),
        provider.clone(),
        mode.clone(),
        sim_config.clone(),
    );

    let (outcome, trace) =
        run_forward_with_trace(&worldline, &agents, &mode, &sim_config, Some(&provider)).await;
    if let Some(path) = trace_jsonl {
        tianji::nuwa::write_trace_jsonl(path, &trace)?;
    }
    Ok(serde_json::to_string_pretty(&outcome)?)
}

async fn handle_backtrack(
    goal: &str,
    constraints: &[(tianji::worldline::types::FieldKey, f64, f64)],
    max_interventions: usize,
    profile_dir: &str,
    config_path: Option<&str>,
) -> Result<String, TianJiError> {
    use tianji::hongmeng::Agent;
    use tianji::llm::ProviderRegistry;
    use tianji::nuwa::backward::run_backward;
    use tianji::nuwa::sandbox::SimulationMode;
    use tianji::profile::ProfileRegistry;
    use tianji::worldline::types::Worldline;

    if constraints.is_empty() {
        return Err(TianJiError::Usage(
            "backtrack requires at least one --field-constraint.".to_string(),
        ));
    }

    // Load profiles (stub: create a default agent if no profiles found)
    let agents = match ProfileRegistry::load_from_dir(std::path::Path::new(profile_dir)) {
        Ok(registry) if !registry.profiles.is_empty() => registry
            .profiles
            .values()
            .map(|p| Agent::from_profile(p.clone()))
            .collect::<Vec<_>>(),
        _ => {
            let stub_profile = tianji::profile::types::ActorProfile {
                id: "stub".to_string(),
                name: "Stub Agent".to_string(),
                tier: tianji::profile::types::ActorTier::Nation,
                interests: vec![],
                red_lines: vec![],
                capabilities: tianji::profile::types::Capabilities::default(),
                behavior_patterns: vec![
                    "observe".to_string(),
                    "diplomatic_protest".to_string(),
                    "negotiation".to_string(),
                ],
                historical_analogues: vec![],
            };
            vec![Agent::from_profile(stub_profile)]
        }
    };

    // Load config (or default)
    let tianji_config = match config_path {
        Some(path) => tianji::llm::TianJiConfig::load_from(path)
            .map_err(|e| TianJiError::Usage(format!("Failed to load config: {e}")))?,
        None => tianji::llm::TianJiConfig::default(),
    };
    let provider = ProviderRegistry::from_config(tianji_config)
        .map_err(|e| TianJiError::Usage(format!("Failed to create provider registry: {e}")))?;

    // Build goal_field_constraints from parsed constraints
    let mut goal_field_constraints = BTreeMap::new();
    for (key, min, max) in constraints {
        goal_field_constraints.insert(key.clone(), (*min, *max));
    }

    // Create stub worldline with initial field values
    let mut fields = BTreeMap::new();
    for (key, _min, _max) in constraints {
        fields.insert(key.clone(), 8.0);
    }
    let hash = Worldline::compute_snapshot_hash(&fields);
    let worldline = Worldline {
        id: 0,
        fields,
        events: vec![],
        causal_graph: petgraph::graph::DiGraph::new(),
        active_actors: std::collections::BTreeSet::new(),
        divergence: 0.0,
        parent: None,
        diverge_tick: 0,
        snapshot_hash: hash,
        created_at: chrono::Utc::now(),
    };

    let mode = SimulationMode::Backward {
        goal_description: goal.to_string(),
        goal_field_constraints,
        max_interventions,
    };

    let _sandbox = tianji::nuwa::sandbox::NuwaSandbox::new(
        worldline.clone(),
        agents.clone(),
        provider.clone(),
        mode.clone(),
        tianji::hongmeng::HongmengConfig::default(),
    );

    let outcome = run_backward(&worldline, &agents, &mode, Some(&provider)).await;
    Ok(serde_json::to_string_pretty(&outcome)?)
}

fn handle_baseline(
    set: bool,
    show: bool,
    clear: bool,
    sqlite_path: Option<&str>,
) -> Result<String, TianJiError> {
    let action_count = [set, show, clear].iter().filter(|&&b| b).count();
    if action_count == 0 {
        return Err(TianJiError::Usage(
            "baseline requires exactly one of --set, --show, or --clear.".to_string(),
        ));
    }
    if action_count > 1 {
        return Err(TianJiError::Usage(
            "baseline accepts only one of --set, --show, or --clear at a time.".to_string(),
        ));
    }

    let db_path = sqlite_path.ok_or_else(|| {
        TianJiError::Usage("--sqlite-path is required for baseline operations.".to_string())
    })?;

    if !Path::new(db_path).exists() {
        return Err(TianJiError::Usage(
            "SQLite database not found. Run tianji run --sqlite-path first.".to_string(),
        ));
    }

    let mut conn = Connection::open(db_path)?;
    conn.execute_batch("PRAGMA foreign_keys = ON")?;

    if show {
        let baseline = load_baseline(&conn)?;
        match baseline {
            Some(b) => Ok(serde_json::to_string_pretty(&b)?),
            None => Err(TianJiError::Usage(
                "No baseline is currently set.".to_string(),
            )),
        }
    } else if set {
        // Build a baseline from the latest run's scored events
        let latest_id = get_latest_run_id(db_path)?
            .ok_or_else(|| TianJiError::Usage("No runs found in database.".to_string()))?;

        let summary = get_run_summary(
            db_path,
            latest_id,
            &ScoredEventFilters::default(),
            false,
            &EventGroupFilters::default(),
        )?
        .ok_or_else(|| TianJiError::Usage("Failed to load latest run summary.".to_string()))?;

        let mut fields: BTreeMap<FieldKey, f64> = BTreeMap::new();
        if let Some(events) = summary.get("scored_events").and_then(|v| v.as_array()) {
            for event in events {
                let field = event
                    .get("dominant_field")
                    .and_then(|v| v.as_str())
                    .unwrap_or("uncategorized");
                let impact = event
                    .get("impact_score")
                    .and_then(|v| v.as_f64())
                    .unwrap_or(0.0);
                let key = FieldKey {
                    region: "global".to_string(),
                    domain: field.to_string(),
                };
                *fields.entry(key).or_insert(0.0) += impact;
            }
        }

        let baseline = Baseline {
            worldline_id: latest_id as u64,
            snapshot_hash: Worldline::compute_snapshot_hash(&fields),
            fields,
            locked_at: chrono::Utc::now(),
            locked_by: Some("cli".to_string()),
        };

        save_baseline(&mut conn, &baseline)?;
        Ok(serde_json::to_string_pretty(&baseline)?)
    } else {
        // clear
        let existing = load_baseline(&conn)?;
        if existing.is_none() {
            return Err(TianJiError::Usage(
                "No baseline is currently set to clear.".to_string(),
            ));
        }
        clear_baseline(&conn)?;
        Ok(serde_json::to_string_pretty(&serde_json::json!({
            "action": "clear",
            "status": "baseline_removed"
        }))?)
    }
}

fn handle_watch(
    source_url: &str,
    interval: u64,
    sqlite_path: Option<&str>,
    _config_path: Option<&str>,
) -> Result<String, TianJiError> {
    handle_watch_with_fetcher(
        source_url,
        interval,
        sqlite_path,
        fetch_feed_url,
        std::thread::sleep,
    )
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum FeedTier {
    Fast,
    Slow,
}

impl FeedTier {
    fn as_str(self) -> &'static str {
        match self {
            Self::Fast => "fast",
            Self::Slow => "slow",
        }
    }
}

fn supported_feed_tiers() -> [FeedTier; 2] {
    [FeedTier::Fast, FeedTier::Slow]
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct WatchedFeed {
    source_url: String,
    tier: FeedTier,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct WatchSchedulerConfig {
    fast_interval: u64,
    slow_interval: u64,
}

impl Default for WatchSchedulerConfig {
    fn default() -> Self {
        Self {
            fast_interval: 30,
            slow_interval: 300,
        }
    }
}

fn validate_tiered_watch(
    feeds: &[WatchedFeed],
    config: WatchSchedulerConfig,
) -> Result<(), TianJiError> {
    if feeds.is_empty() {
        return Err(TianJiError::Usage(
            "watch requires at least one feed.".to_string(),
        ));
    }
    if feeds.iter().any(|feed| feed.source_url.trim().is_empty()) {
        return Err(TianJiError::Usage(
            "watch feed URLs must be non-empty.".to_string(),
        ));
    }
    if config.fast_interval < 10 {
        return Err(TianJiError::Usage(
            "fast interval must be at least 10 seconds.".to_string(),
        ));
    }
    if config.slow_interval < config.fast_interval {
        return Err(TianJiError::Usage(
            "slow interval cannot be below fast interval.".to_string(),
        ));
    }
    Ok(())
}

fn due_feeds_for_iteration(
    feeds: &[WatchedFeed],
    config: WatchSchedulerConfig,
    iteration: usize,
) -> Vec<&WatchedFeed> {
    let elapsed = (iteration.saturating_sub(1) as u64) * config.fast_interval;
    feeds
        .iter()
        .filter(|feed| match feed.tier {
            FeedTier::Fast => true,
            FeedTier::Slow => elapsed.is_multiple_of(config.slow_interval),
        })
        .collect()
}

fn feed_interval(feed: &WatchedFeed, config: WatchSchedulerConfig) -> u64 {
    match feed.tier {
        FeedTier::Fast => config.fast_interval,
        FeedTier::Slow => config.slow_interval,
    }
}

fn watch_result_for_feed<F>(
    fetcher: &mut F,
    source_url: &str,
    sqlite_path: Option<&str>,
    iteration: usize,
) -> serde_json::Value
where
    F: FnMut(&str) -> Result<String, TianJiError>,
{
    match fetcher(source_url)
        .and_then(|feed_text| run_feed_text(&feed_text, source_url, sqlite_path))
    {
        Ok(run_result) => serde_json::json!({
            "iteration": iteration,
            "source_url": source_url,
            "status": "ok",
            "raw_item_count": run_result.artifact.input_summary.raw_item_count,
            "normalized_event_count": run_result.artifact.input_summary.normalized_event_count,
            "dominant_field": run_result.artifact.scenario_summary.dominant_field,
            "risk_level": run_result.artifact.scenario_summary.risk_level,
            "headline": run_result.artifact.scenario_summary.headline,
        }),
        Err(e) => serde_json::json!({
            "iteration": iteration,
            "source_url": source_url,
            "status": "error",
            "error": e.to_string(),
        }),
    }
}

#[cfg(test)]
fn handle_tiered_watch_with_fetcher<F, S>(
    feeds: &[WatchedFeed],
    config: WatchSchedulerConfig,
    max_iterations: usize,
    sqlite_path: Option<&str>,
    mut fetcher: F,
    mut sleeper: S,
) -> Result<String, TianJiError>
where
    F: FnMut(&str) -> Result<String, TianJiError>,
    S: FnMut(std::time::Duration),
{
    validate_tiered_watch(feeds, config)?;

    let mut results = Vec::new();
    for iteration in 1..=max_iterations {
        for feed in due_feeds_for_iteration(feeds, config, iteration) {
            let interval = feed_interval(feed, config);
            let mut result =
                watch_result_for_feed(&mut fetcher, &feed.source_url, sqlite_path, iteration);
            result["tier"] = serde_json::Value::String(feed.tier.as_str().to_string());
            result["interval"] = serde_json::Value::from(interval);
            results.push(result);
        }
        if iteration < max_iterations {
            sleeper(std::time::Duration::from_secs(config.fast_interval));
        }
    }

    let feed_metadata: Vec<_> = feeds
        .iter()
        .map(|feed| {
            serde_json::json!({
                "source_url": feed.source_url,
                "tier": feed.tier.as_str(),
                "interval": feed_interval(feed, config),
            })
        })
        .collect();
    let payload = serde_json::json!({
        "watch": {
            "fast_interval": config.fast_interval,
            "slow_interval": config.slow_interval,
            "iterations": max_iterations,
            "feeds": feed_metadata,
        },
        "results": results,
    });
    Ok(serde_json::to_string_pretty(&payload)?)
}

fn handle_watch_with_fetcher<F, S>(
    source_url: &str,
    interval: u64,
    sqlite_path: Option<&str>,
    mut fetcher: F,
    sleeper: S,
) -> Result<String, TianJiError>
where
    F: FnMut(&str) -> Result<String, TianJiError>,
    S: Fn(std::time::Duration),
{
    if interval < 10 {
        return Err(TianJiError::Usage(
            "--interval must be at least 10 seconds.".to_string(),
        ));
    }
    let single_feed = WatchedFeed {
        source_url: source_url.to_string(),
        tier: FeedTier::Fast,
    };
    let scheduler_config = WatchSchedulerConfig {
        fast_interval: interval,
        slow_interval: interval,
    };
    validate_tiered_watch(std::slice::from_ref(&single_feed), scheduler_config)?;

    let max_iterations = 3;
    let mut results = Vec::new();

    for i in 1..=max_iterations {
        for feed in due_feeds_for_iteration(std::slice::from_ref(&single_feed), scheduler_config, i)
        {
            results.push(watch_result_for_feed(
                &mut fetcher,
                &feed.source_url,
                sqlite_path,
                i,
            ));
        }
        if i < max_iterations {
            sleeper(std::time::Duration::from_secs(interval));
        }
    }

    let supported_tiers: Vec<_> = supported_feed_tiers()
        .into_iter()
        .map(FeedTier::as_str)
        .collect();

    let payload = serde_json::json!({
        "watch": {
            "source_url": source_url,
            "interval": interval,
            "iterations": max_iterations,
            "supported_tiers": supported_tiers,
            "feeds": [{
                "source_url": single_feed.source_url,
                "tier": single_feed.tier.as_str(),
                "interval": feed_interval(&single_feed, scheduler_config),
            }],
        },
        "results": results,
    });
    Ok(serde_json::to_string_pretty(&payload)?)
}

fn fetch_feed_url(source_url: &str) -> Result<String, TianJiError> {
    if !(source_url.starts_with("http://") || source_url.starts_with("https://")) {
        return Err(TianJiError::Usage(
            "--source-url must be an HTTP or HTTPS URL.".to_string(),
        ));
    }
    let response = reqwest::blocking::Client::builder()
        .timeout(std::time::Duration::from_secs(30))
        .build()
        .map_err(|error| TianJiError::Input(format!("Failed to build feed client: {error}")))?
        .get(source_url)
        .send()
        .map_err(|error| {
            TianJiError::Input(format!("Failed to fetch feed {source_url}: {error}"))
        })?;
    let status = response.status();
    if !status.is_success() {
        return Err(TianJiError::Input(format!(
            "Failed to fetch feed {source_url}: HTTP {status}"
        )));
    }
    response
        .text()
        .map_err(|error| TianJiError::Input(format!("Failed to read feed {source_url}: {error}")))
}

// ---------------------------------------------------------------------------
// Main run dispatch
// ---------------------------------------------------------------------------

async fn run(cli: Cli) -> Result<String, TianJiError> {
    match cli {
        Cli::Run {
            fixture,
            sqlite_path,
            show_delta,
        } => {
            let result = run_fixture_path(fixture, sqlite_path.as_deref())?;
            if show_delta {
                let payload = serde_json::json!({
                    "artifact": result.artifact,
                    "delta": result.delta,
                    "alert_tier": result.alert_tier,
                });
                Ok(serde_json::to_string_pretty(&payload)?)
            } else {
                artifact_json(&result.artifact)
            }
        }
        Cli::History {
            sqlite_path,
            limit,
            mode,
            dominant_field,
            risk_level,
            since,
            until,
            min_top_impact_score,
            max_top_impact_score,
            min_top_field_attraction,
            max_top_field_attraction,
            min_top_divergence_score,
            max_top_divergence_score,
            top_group_dominant_field,
            min_event_group_count,
            max_event_group_count,
        } => {
            validate_score_range(
                min_top_impact_score,
                max_top_impact_score,
                "--min-top-impact-score",
                "--max-top-impact-score",
            )?;
            validate_score_range(
                min_top_field_attraction,
                max_top_field_attraction,
                "--min-top-field-attraction",
                "--max-top-field-attraction",
            )?;
            validate_score_range(
                min_top_divergence_score,
                max_top_divergence_score,
                "--min-top-divergence-score",
                "--max-top-divergence-score",
            )?;
            validate_int_range(
                min_event_group_count,
                max_event_group_count,
                "--min-event-group-count",
                "--max-event-group-count",
            )?;

            let filters = RunListFilters {
                mode,
                dominant_field,
                risk_level,
                since,
                until,
                min_top_impact_score,
                max_top_impact_score,
                min_top_field_attraction,
                max_top_field_attraction,
                min_top_divergence_score,
                max_top_divergence_score,
                top_group_dominant_field,
                min_event_group_count,
                max_event_group_count,
            };
            let payload = list_runs(&sqlite_path, limit, &filters)?;
            Ok(serde_json::to_string_pretty(&payload).map_err(TianJiError::Json)?)
        }
        Cli::HistoryShow {
            sqlite_path,
            run_id,
            latest,
            previous,
            next,
            dominant_field,
            min_impact_score,
            max_impact_score,
            min_field_attraction,
            max_field_attraction,
            min_divergence_score,
            max_divergence_score,
            limit_scored_events,
            only_matching_interventions,
            group_dominant_field,
            limit_event_groups,
        } => {
            // Validate navigation
            let nav_count = [latest, previous, next].iter().filter(|&&b| b).count();
            if nav_count > 1 {
                return Err(TianJiError::Usage(
                    "Use only one history-show navigation mode: --latest, --previous, or --next."
                        .to_string(),
                ));
            }
            if latest && run_id.is_some() {
                return Err(TianJiError::Usage(
                    "Use either --run-id or --latest for history-show, not both.".to_string(),
                ));
            }
            if (previous || next) && run_id.is_none() {
                return Err(TianJiError::Usage(
                    "history-show with --previous/--next requires --run-id.".to_string(),
                ));
            }
            if !latest && !previous && !next && run_id.is_none() {
                return Err(TianJiError::Usage(
                    "history-show requires --run-id, --latest, --previous, or --next.".to_string(),
                ));
            }
            validate_score_range(
                min_impact_score,
                max_impact_score,
                "--min-impact-score",
                "--max-impact-score",
            )?;
            validate_score_range(
                min_field_attraction,
                max_field_attraction,
                "--min-field-attraction",
                "--max-field-attraction",
            )?;
            validate_score_range(
                min_divergence_score,
                max_divergence_score,
                "--min-divergence-score",
                "--max-divergence-score",
            )?;

            // Resolve run_id
            let resolved_run_id = if latest {
                get_latest_run_id(&sqlite_path)?.ok_or_else(|| {
                    TianJiError::Usage("No persisted runs are available.".to_string())
                })?
            } else if previous {
                let rid = run_id.expect("validated above");
                get_previous_run_id(&sqlite_path, rid)?.ok_or_else(|| {
                    TianJiError::Usage(format!(
                        "No previous persisted run exists before run {rid}."
                    ))
                })?
            } else if next {
                let rid = run_id.expect("validated above");
                get_next_run_id(&sqlite_path, rid)?.ok_or_else(|| {
                    TianJiError::Usage(format!("No next persisted run exists after run {rid}."))
                })?
            } else {
                run_id.expect("validated above")
            };

            let scored_filters = ScoredEventFilters {
                dominant_field,
                min_impact_score,
                max_impact_score,
                min_field_attraction,
                max_field_attraction,
                min_divergence_score,
                max_divergence_score,
                limit_scored_events,
            };
            let group_filters = EventGroupFilters {
                dominant_field: group_dominant_field,
                limit_event_groups,
            };

            let payload = get_run_summary(
                &sqlite_path,
                resolved_run_id,
                &scored_filters,
                only_matching_interventions,
                &group_filters,
            )?;
            match payload {
                Some(p) => Ok(serde_json::to_string_pretty(&p).map_err(TianJiError::Json)?),
                None => Err(TianJiError::Usage(format!(
                    "Run not found: {resolved_run_id}"
                ))),
            }
        }
        Cli::HistoryCompare {
            sqlite_path,
            left_run_id,
            right_run_id,
            latest_pair,
            run_id,
            against_latest,
            against_previous,
            dominant_field,
            min_impact_score,
            max_impact_score,
            min_field_attraction,
            max_field_attraction,
            min_divergence_score,
            max_divergence_score,
            limit_scored_events,
            only_matching_interventions,
            group_dominant_field,
            limit_event_groups,
        } => {
            validate_score_range(
                min_impact_score,
                max_impact_score,
                "--min-impact-score",
                "--max-impact-score",
            )?;
            validate_score_range(
                min_field_attraction,
                max_field_attraction,
                "--min-field-attraction",
                "--max-field-attraction",
            )?;
            validate_score_range(
                min_divergence_score,
                max_divergence_score,
                "--min-divergence-score",
                "--max-divergence-score",
            )?;

            // Resolve compare run IDs (matching Python _resolve_compare_run_ids)
            let mixed_pair_message = "Use either --latest-pair, --run-id with --against-latest, --run-id with --against-previous, or explicit --left-run-id/--right-run-id, not a mix.";

            let (resolved_left, resolved_right) = if latest_pair {
                if left_run_id.is_some()
                    || right_run_id.is_some()
                    || run_id.is_some()
                    || against_latest
                    || against_previous
                {
                    return Err(TianJiError::Usage(mixed_pair_message.to_string()));
                }
                get_latest_run_pair(&sqlite_path)?.ok_or_else(|| {
                    TianJiError::Usage(
                        "At least two persisted runs are required for --latest-pair.".to_string(),
                    )
                })?
            } else if against_latest {
                if against_previous {
                    return Err(TianJiError::Usage(
                        "Use only one comparison preset: --against-latest or --against-previous."
                            .to_string(),
                    ));
                }
                if left_run_id.is_some() || right_run_id.is_some() {
                    return Err(TianJiError::Usage(mixed_pair_message.to_string()));
                }
                let rid = run_id.ok_or_else(|| {
                    TianJiError::Usage(
                        "history-compare with --against-latest requires --run-id.".to_string(),
                    )
                })?;
                let latest_id = get_latest_run_id(&sqlite_path)?.ok_or_else(|| {
                    TianJiError::Usage("No persisted runs are available.".to_string())
                })?;
                (rid, latest_id)
            } else if against_previous {
                if left_run_id.is_some() || right_run_id.is_some() {
                    return Err(TianJiError::Usage(mixed_pair_message.to_string()));
                }
                let rid = run_id.ok_or_else(|| {
                    TianJiError::Usage(
                        "history-compare with --against-previous requires --run-id.".to_string(),
                    )
                })?;
                let prev_id = get_previous_run_id(&sqlite_path, rid)?.ok_or_else(|| {
                    TianJiError::Usage(format!(
                        "No previous persisted run exists before run {rid}."
                    ))
                })?;
                (prev_id, rid)
            } else if run_id.is_some() {
                return Err(TianJiError::Usage(
                    "Use --run-id only with --against-latest or --against-previous for history-compare.".to_string(),
                ));
            } else {
                match (left_run_id, right_run_id) {
                    (Some(left), Some(right)) => (left, right),
                    _ => {
                        return Err(TianJiError::Usage(
                            "history-compare requires --latest-pair, --run-id with --against-latest, --run-id with --against-previous, or both --left-run-id and --right-run-id.".to_string(),
                        ));
                    }
                }
            };

            let scored_filters = ScoredEventFilters {
                dominant_field,
                min_impact_score,
                max_impact_score,
                min_field_attraction,
                max_field_attraction,
                min_divergence_score,
                max_divergence_score,
                limit_scored_events,
            };
            let group_filters = EventGroupFilters {
                dominant_field: group_dominant_field,
                limit_event_groups,
            };

            let result = compare_runs(
                &sqlite_path,
                resolved_left,
                resolved_right,
                &scored_filters,
                only_matching_interventions,
                &group_filters,
            )?;
            match result {
                Some(r) => {
                    let output = serde_json::json!({
                        "left_run_id": r.left_run_id,
                        "right_run_id": r.right_run_id,
                        "left": r.left,
                        "right": r.right,
                        "diff": r.diff,
                    });
                    Ok(serde_json::to_string_pretty(&output).map_err(TianJiError::Json)?)
                }
                None => Err(TianJiError::Usage(format!(
                    "Run not found for comparison: {resolved_left} vs {resolved_right}"
                ))),
            }
        }
        Cli::Daemon { command } => match command {
            DaemonCommands::Start {
                socket_path,
                sqlite_path,
                host,
                port,
            } => handle_daemon_start(&socket_path, &sqlite_path, &host, port),
            DaemonCommands::Stop { socket_path } => handle_daemon_stop(&socket_path),
            DaemonCommands::Status {
                socket_path,
                job_id,
            } => handle_daemon_status(&socket_path, job_id.as_deref()),
            DaemonCommands::Run {
                socket_path,
                fixture,
                sqlite_path,
            } => handle_daemon_run(&socket_path, &fixture, sqlite_path.as_deref()),
            DaemonCommands::Schedule {
                socket_path,
                fixture,
                sqlite_path,
                every_seconds,
                count,
            } => handle_daemon_schedule(
                &socket_path,
                &fixture,
                sqlite_path.as_deref(),
                every_seconds,
                count,
            ),
            DaemonCommands::Serve {
                socket_path,
                sqlite_path,
                host,
                port,
            } => {
                tianji::daemon::serve(&socket_path, &sqlite_path, &host, port)?;
                Ok(String::new())
            }
        },
        Cli::Webui {
            host,
            port,
            api_base_url,
            socket_path,
            sqlite_path,
        } => {
            tianji::webui::serve_webui(
                &host,
                port,
                &api_base_url,
                &socket_path,
                sqlite_path.as_deref(),
            )
            .await
            .map_err(TianJiError::Usage)?;
            Ok(String::new())
        }
        Cli::Tui {
            sqlite_path,
            limit,
            simulate,
            interactive,
        } => {
            tianji::tui::run_history_browser(&sqlite_path, limit, simulate.as_deref(), interactive)
                .await
        }
        Cli::Delta {
            sqlite_path,
            left_run_id,
            right_run_id,
            latest_pair,
        } => {
            let (left, right) = if latest_pair {
                if left_run_id.is_some() || right_run_id.is_some() {
                    return Err(TianJiError::Usage(
                        "Use either --latest-pair or explicit --left-run-id/--right-run-id for delta, not both."
                            .to_string(),
                    ));
                }
                get_latest_run_pair(&sqlite_path)?.ok_or_else(|| {
                    TianJiError::Usage(
                        "At least two persisted runs are required for --latest-pair.".to_string(),
                    )
                })?
            } else {
                match (left_run_id, right_run_id) {
                    (Some(left), Some(right)) => (left, right),
                    _ => {
                        return Err(TianJiError::Usage(
                            "delta requires --latest-pair or both --left-run-id and --right-run-id."
                                .to_string(),
                        ));
                    }
                }
            };

            let scored_filters = ScoredEventFilters::default();
            let group_filters = EventGroupFilters::default();
            let previous =
                get_run_summary(&sqlite_path, left, &scored_filters, false, &group_filters)?
                    .ok_or_else(|| TianJiError::Usage(format!("Run not found: {left}")))?;
            let current =
                get_run_summary(&sqlite_path, right, &scored_filters, false, &group_filters)?
                    .ok_or_else(|| TianJiError::Usage(format!("Run not found: {right}")))?;
            let report = compute_delta(&current, Some(&previous)).ok_or_else(|| {
                TianJiError::Usage("A previous run is required to compute delta.".to_string())
            })?;
            let alert_tier = classify_delta_tier(&report);
            let output = serde_json::json!({
                "alert_tier": alert_tier,
                "delta": report,
            });
            Ok(serde_json::to_string_pretty(&output).map_err(TianJiError::Json)?)
        }
        Cli::Predict {
            field,
            horizon,
            profile_dir,
            config,
            trace_jsonl,
        } => {
            handle_predict(
                &field,
                horizon,
                &profile_dir,
                config.as_deref(),
                trace_jsonl.as_deref().map(Path::new),
            )
            .await
        }
        Cli::Backtrack {
            goal,
            field_constraints,
            max_interventions,
            profile_dir,
            config,
        } => {
            handle_backtrack(
                &goal,
                &field_constraints,
                max_interventions,
                &profile_dir,
                config.as_deref(),
            )
            .await
        }
        Cli::Baseline {
            set,
            show,
            clear,
            sqlite_path,
        } => handle_baseline(set, show, clear, sqlite_path.as_deref()),
        Cli::Watch {
            source_url,
            interval,
            sqlite_path,
            config,
        } => handle_watch(
            &source_url,
            interval,
            sqlite_path.as_deref(),
            config.as_deref(),
        ),
        Cli::Doctor {
            config,
            sqlite_path,
            json,
        } => handle_doctor(config.as_deref(), sqlite_path.as_deref(), json),
        Cli::Eval {
            manifest,
            update_golden,
        } => {
            let report = tianji::eval::run_eval_manifest(&manifest, update_golden)?;
            let output = serde_json::to_string_pretty(&report)?;
            if report.failed > 0 {
                Err(TianJiError::ReportFailure(output))
            } else {
                Ok(output)
            }
        }
        Cli::Sources {
            config,
            run_fixtures,
            fetch_live,
            sqlite_path,
        } => {
            let manifest = load_source_manifest(&config)?;
            let latest_health = if let Some(path) = sqlite_path.as_deref() {
                load_latest_source_health(path)?
            } else {
                BTreeMap::new()
            };
            let report = build_sources_report_with_health(
                &config,
                manifest,
                run_fixtures,
                fetch_live,
                latest_health,
            )?;
            if let Some(path) = sqlite_path.as_deref() {
                if run_fixtures || fetch_live {
                    let checks = source_health_inputs_from_runs(&report.runs);
                    persist_source_health_checks(path, &checks)?;
                }
            }
            Ok(serde_json::to_string_pretty(&report)?)
        }
        Cli::Maintenance { command } => match command {
            MaintenanceCommands::Check { sqlite_path } => {
                let report = maintenance_check(&sqlite_path)?;
                Ok(serde_json::to_string_pretty(&report)?)
            }
            MaintenanceCommands::Backup {
                sqlite_path,
                output,
                overwrite,
            } => {
                let report = backup_sqlite_database(&sqlite_path, &output, overwrite)?;
                Ok(serde_json::to_string_pretty(&report)?)
            }
            MaintenanceCommands::Export {
                sqlite_path,
                output,
                format,
                include_details,
                overwrite,
            } => {
                let report = export_run_history(
                    &sqlite_path,
                    &output,
                    format.into(),
                    include_details,
                    overwrite,
                )?;
                Ok(serde_json::to_string_pretty(&report)?)
            }
            MaintenanceCommands::Compact {
                sqlite_path,
                vacuum,
            } => {
                let report = compact_sqlite_database(&sqlite_path, vacuum)?;
                Ok(serde_json::to_string_pretty(&report)?)
            }
            MaintenanceCommands::Retain {
                sqlite_path,
                keep_last_runs,
            } => {
                let report = apply_retention_policy(&sqlite_path, keep_last_runs)?;
                Ok(serde_json::to_string_pretty(&report)?)
            }
        },
        Cli::Completions { shell } => {
            let shell = match shell {
                ShellName::Bash => Shell::Bash,
                ShellName::Zsh => Shell::Zsh,
                ShellName::Fish => Shell::Fish,
            };
            let mut cmd = Cli::command();
            let name = cmd.get_name().to_string();
            generate(shell, &mut cmd, name, &mut std::io::stdout());
            Ok(String::new())
        }
    }
}
