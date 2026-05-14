use std::os::unix::process::CommandExt;
use std::process::ExitCode;

use clap::{Parser, Subcommand};
use tianji::{
    artifact_json, classify_delta_tier, compare_runs, compute_delta, get_latest_run_id,
    get_latest_run_pair, get_next_run_id, get_previous_run_id, get_run_summary, list_runs,
    run_fixture_path,
    storage::{EventGroupFilters, RunListFilters, ScoredEventFilters},
    TianJiError,
};

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

fn main() -> ExitCode {
    let cli = Cli::parse();
    match run(cli) {
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

// ---------------------------------------------------------------------------
// PID file management (for daemon start/stop)
// ---------------------------------------------------------------------------

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
    unsafe { libc::kill(pid as i32, 0) == 0 }
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
    let mut run_payload = serde_json::json!({
        "fixture_paths": [fixture],
    });
    if let Some(sp) = sqlite_path {
        run_payload["sqlite_path"] = serde_json::Value::String(sp.to_string());
    }

    let payload = serde_json::json!({
        "action": "queue_run",
        "payload": run_payload,
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
        .unwrap_or("Daemon returned an invalid error response.");
    Err(TianJiError::Usage(error_msg.to_string()))
}

// ---------------------------------------------------------------------------
// Main run dispatch
// ---------------------------------------------------------------------------

fn run(cli: Cli) -> Result<String, TianJiError> {
    match cli {
        Cli::Run {
            fixture,
            sqlite_path,
        } => {
            let result = run_fixture_path(fixture, sqlite_path.as_deref())?;
            artifact_json(&result.artifact)
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
            let rt = tokio::runtime::Runtime::new()
                .map_err(|e| TianJiError::Usage(format!("Failed to create tokio runtime: {e}")))?;
            rt.block_on(async {
                tianji::webui::serve_webui(
                    &host,
                    port,
                    &api_base_url,
                    &socket_path,
                    sqlite_path.as_deref(),
                )
                .await
                .map_err(TianJiError::Usage)
            })?;
            Ok(String::new())
        }
        Cli::Tui { sqlite_path, limit } => tianji::tui::run_history_browser(&sqlite_path, limit),
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
    }
}
