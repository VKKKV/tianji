use std::os::unix::process::CommandExt;
use std::process::ExitCode;

use std::collections::BTreeMap;
use std::str::FromStr;

use clap::{CommandFactory, Parser, Subcommand, ValueEnum};
use clap_complete::{generate, Shell};
use tianji::{
    artifact_json, classify_delta_tier, compare_runs, compute_delta, delta_memory_path,
    get_latest_run_id, get_latest_run_pair, get_next_run_id, get_previous_run_id, get_run_summary,
    list_runs, run_fixture_path,
    storage::{EventGroupFilters, RunListFilters, ScoredEventFilters},
    TianJiError,
};

/// Shell names for completion generation
#[derive(Clone, Debug, ValueEnum)]
enum ShellName {
    Bash,
    Zsh,
    Fish,
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

#[tokio::main]
async fn main() -> ExitCode {
    let cli = Cli::parse();
    match run(cli).await {
        Ok(output) => {
            if !output.is_empty() {
                println!("{output}");
            }
            ExitCode::SUCCESS
        }
        Err(error) => {
            eprintln!("error: {error}");
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
            .block_on(handle_predict("global.conflict", 5, "profiles/", None))
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
        let result = rt.block_on(handle_predict("conflict", 5, "profiles/", None));
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
            Cli::Predict { field, horizon, .. } => {
                assert_eq!(field, "east-asia.conflict");
                assert_eq!(horizon, 10);
            }
            _ => panic!("expected Predict variant"),
        }
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
            } => {
                assert_eq!(sqlite_path, "runs/tianji.sqlite3");
                assert_eq!(limit, 20);
                assert_eq!(simulate, Some("east-asia.conflict:30".to_string()));
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
use std::path::PathBuf;

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
    let url = format!(
        "{}/api/v1/meta",
        tianji::daemon::loopback_http_base_url(host, port)
    );
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
        return Err(TianJiError::Usage(
            format!("Daemon HTTP API did not become ready within {start_timeout_secs:.1}s at {}/api/v1/meta.", tianji::daemon::loopback_http_base_url(&validated_host, port))
        ));
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
// Predict / Backtrack / Baseline / Watch handlers
// ---------------------------------------------------------------------------

async fn handle_predict(
    field: &str,
    horizon: u64,
    profile_dir: &str,
    config_path: Option<&str>,
) -> Result<String, TianJiError> {
    use tianji::hongmeng::Agent;
    use tianji::llm::ProviderRegistry;
    use tianji::nuwa::forward::run_forward;
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

    let outcome = run_forward(&worldline, &agents, &mode, &sim_config, Some(&provider)).await;
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
    let memory_path = delta_memory_path(db_path);
    let baseline_path = memory_path.with_file_name("baseline.json");

    if show {
        if !baseline_path.exists() {
            return Err(TianJiError::Usage(
                "No baseline is currently set.".to_string(),
            ));
        }
        let content = std::fs::read_to_string(&baseline_path)?;
        // Validate it's valid JSON
        let value: serde_json::Value = serde_json::from_str(&content)
            .map_err(|e| TianJiError::Usage(format!("Baseline file is corrupt: {e}")))?;
        Ok(serde_json::to_string_pretty(&value)?)
    } else if set {
        // Write a placeholder baseline to hot-memory directory
        // (Real worldline persistence is deferred — write a stub for now)
        if let Some(parent) = baseline_path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let baseline = serde_json::json!({
            "worldline_id": 0,
            "snapshot_hash": "placeholder",
            "fields": {},
            "locked_at": chrono::Utc::now().to_rfc3339(),
            "locked_by": "cli",
            "note": "stub baseline — worldline not yet persisted"
        });
        let json = serde_json::to_string_pretty(&baseline)?;
        std::fs::write(&baseline_path, &json)?;
        Ok(json)
    } else {
        // clear
        if !baseline_path.exists() {
            return Err(TianJiError::Usage(
                "No baseline is currently set to clear.".to_string(),
            ));
        }
        std::fs::remove_file(&baseline_path)?;
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
    if interval < 10 {
        return Err(TianJiError::Usage(
            "--interval must be at least 10 seconds.".to_string(),
        ));
    }

    // Stub: 3-iteration loop with sleep, uses fixture path for now
    // Live feed fetching via reqwest is deferred
    let max_iterations = 3;
    let mut results = Vec::new();

    for i in 1..=max_iterations {
        let fixture_path = "tests/fixtures/sample_feed.xml";
        match run_fixture_path(fixture_path, sqlite_path) {
            Ok(run_result) => {
                results.push(serde_json::json!({
                    "iteration": i,
                    "source_url": source_url,
                    "status": "ok",
                    "run_id": run_result.artifact.input_summary.normalized_event_count,
                }));
            }
            Err(e) => {
                results.push(serde_json::json!({
                    "iteration": i,
                    "source_url": source_url,
                    "status": "error",
                    "error": e.to_string(),
                }));
            }
        }
        if i < max_iterations {
            std::thread::sleep(std::time::Duration::from_secs(interval));
        }
    }

    let payload = serde_json::json!({
        "watch": {
            "source_url": source_url,
            "interval": interval,
            "iterations": max_iterations,
            "note": "stub: using fixture path, live feed fetching deferred",
        },
        "results": results,
    });
    Ok(serde_json::to_string_pretty(&payload)?)
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
        } => tianji::tui::run_history_browser(&sqlite_path, limit, simulate.as_deref()).await,
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
        } => handle_predict(&field, horizon, &profile_dir, config.as_deref()).await,
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
