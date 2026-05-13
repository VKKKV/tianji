use std::collections::HashMap;
use std::collections::VecDeque;
use std::path::Path;
use std::sync::Arc;
use std::sync::Condvar;
use std::sync::Mutex;

use crate::get_latest_run_id;
use crate::run_fixture_path;
use crate::TianJiError;

pub const ALLOWED_JOB_STATES: [&str; 4] = ["queued", "running", "succeeded", "failed"];
pub const LOOPBACK_HOSTS: [&str; 3] = ["127.0.0.1", "localhost", "::1"];
pub const DEFAULT_HTTP_API_PORT: u16 = 8765;
pub const DEFAULT_SQLITE_PATH: &str = "runs/tianji.sqlite3";

// ---------------------------------------------------------------------------
// Loopback validation
// ---------------------------------------------------------------------------

pub fn validate_loopback_host(host: &str) -> Result<String, TianJiError> {
    let normalized = host.trim();
    if !LOOPBACK_HOSTS.contains(&normalized) {
        return Err(TianJiError::Usage(format!(
            "TianJi daemon is local-only and requires a loopback host; got '{normalized}'."
        )));
    }
    Ok(normalized.to_string())
}

// ---------------------------------------------------------------------------
// RunJobRequest
// ---------------------------------------------------------------------------

#[derive(Clone, Debug)]
pub struct RunJobRequest {
    pub fixture_paths: Vec<String>,
    pub fetch: bool,
    pub source_urls: Vec<String>,
    pub fetch_policy: String,
    pub source_fetch_details: Vec<serde_json::Value>,
    pub output_path: Option<String>,
    pub sqlite_path: Option<String>,
}

impl RunJobRequest {
    pub fn from_payload(payload: &serde_json::Value) -> Result<Self, String> {
        let fixture_paths = coerce_string_list(payload.get("fixture_paths"), "fixture_paths")?;

        let source_urls = coerce_string_list(payload.get("source_urls"), "source_urls")?;

        let fetch = payload
            .get("fetch")
            .and_then(|v| v.as_bool())
            .unwrap_or(false);

        let fetch_policy = payload
            .get("fetch_policy")
            .and_then(|v| v.as_str())
            .unwrap_or("always")
            .to_string();

        let output_path = coerce_optional_string(payload.get("output_path"), "output_path")?;

        let sqlite_path = coerce_optional_string(payload.get("sqlite_path"), "sqlite_path")?;

        let source_fetch_details =
            coerce_source_fetch_details(payload.get("source_fetch_details"))?;

        Ok(Self {
            fixture_paths,
            fetch,
            source_urls,
            fetch_policy,
            source_fetch_details,
            output_path,
            sqlite_path,
        })
    }
}

fn coerce_string_list(
    value: Option<&serde_json::Value>,
    field_name: &str,
) -> Result<Vec<String>, String> {
    match value {
        None => Ok(Vec::new()),
        Some(serde_json::Value::Array(arr)) => {
            let mut result = Vec::new();
            for item in arr {
                match item.as_str() {
                    Some(s) => result.push(s.to_string()),
                    None => {
                        return Err(format!(
                            "queue request field '{field_name}' must be a list of strings"
                        ))
                    }
                }
            }
            Ok(result)
        }
        _ => Err(format!(
            "queue request field '{field_name}' must be a list of strings"
        )),
    }
}

fn coerce_optional_string(
    value: Option<&serde_json::Value>,
    field_name: &str,
) -> Result<Option<String>, String> {
    match value {
        None | Some(serde_json::Value::Null) => Ok(None),
        Some(serde_json::Value::String(s)) => Ok(Some(s.clone())),
        _ => Err(format!(
            "queue request field '{field_name}' must be a string or null"
        )),
    }
}

fn coerce_source_fetch_details(
    value: Option<&serde_json::Value>,
) -> Result<Vec<serde_json::Value>, String> {
    match value {
        None | Some(serde_json::Value::Null) => Ok(Vec::new()),
        Some(serde_json::Value::Array(arr)) => {
            for item in arr {
                if !item.is_object() {
                    return Err(
                        "queue request field 'source_fetch_details' must be a list of objects"
                            .to_string(),
                    );
                }
            }
            Ok(arr.clone())
        }
        _ => Err("queue request field 'source_fetch_details' must be a list".to_string()),
    }
}

// ---------------------------------------------------------------------------
// JobRecord
// ---------------------------------------------------------------------------

#[derive(Clone, Debug)]
pub struct JobRecord {
    pub job_id: String,
    pub state: String,
    pub request: RunJobRequest,
    pub run_id: Option<i64>,
    pub error: Option<String>,
}

impl JobRecord {
    pub fn to_status_payload(&self) -> serde_json::Value {
        serde_json::json!({
            "job_id": self.job_id,
            "state": self.state,
            "run_id": self.run_id,
            "error": self.error,
        })
    }
}

// ---------------------------------------------------------------------------
// DaemonState (shared mutable state)
// ---------------------------------------------------------------------------

struct DaemonStateInner {
    jobs: HashMap<String, JobRecord>,
    queue: VecDeque<String>,
}

pub struct DaemonState {
    inner: Mutex<DaemonStateInner>,
    queue_condvar: Condvar,
    stop_flag: Mutex<bool>,
}

impl Default for DaemonState {
    fn default() -> Self {
        Self::new()
    }
}

impl DaemonState {
    pub fn new() -> Self {
        Self {
            inner: Mutex::new(DaemonStateInner {
                jobs: HashMap::new(),
                queue: VecDeque::new(),
            }),
            queue_condvar: Condvar::new(),
            stop_flag: Mutex::new(false),
        }
    }

    pub fn enqueue_job(&self, request: RunJobRequest) -> JobRecord {
        let job_id = format!(
            "job-{}",
            &uuid::Uuid::new_v4().to_string().replace("-", "")[..12]
        );
        let record = JobRecord {
            job_id: job_id.clone(),
            state: "queued".to_string(),
            request,
            run_id: None,
            error: None,
        };
        {
            let mut inner = self.inner.lock().unwrap();
            inner.jobs.insert(job_id.clone(), record.clone());
            inner.queue.push_back(job_id);
        }
        self.queue_condvar.notify_all();
        record
    }

    pub fn get_job(&self, job_id: &str) -> Option<JobRecord> {
        let inner = self.inner.lock().unwrap();
        inner.jobs.get(job_id).cloned()
    }

    pub fn set_job_running(&self, job_id: &str) {
        let mut inner = self.inner.lock().unwrap();
        if let Some(record) = inner.jobs.get_mut(job_id) {
            record.state = "running".to_string();
        }
    }

    pub fn set_job_succeeded(&self, job_id: &str, run_id: Option<i64>) {
        let mut inner = self.inner.lock().unwrap();
        if let Some(record) = inner.jobs.get_mut(job_id) {
            record.state = "succeeded".to_string();
            record.run_id = run_id;
        }
    }

    pub fn set_job_failed(&self, job_id: &str, error: String) {
        let mut inner = self.inner.lock().unwrap();
        if let Some(record) = inner.jobs.get_mut(job_id) {
            record.state = "failed".to_string();
            record.error = Some(error);
        }
    }

    pub fn pop_next_job(&self, timeout_ms: u64) -> Option<JobRecord> {
        let inner = self.inner.lock().unwrap();
        let result = self.queue_condvar.wait_timeout_while(
            inner,
            std::time::Duration::from_millis(timeout_ms),
            |inner| {
                let stop = *self.stop_flag.lock().unwrap();
                !stop && inner.queue.is_empty()
            },
        );

        match result {
            Ok((mut inner, timeout_result)) => {
                if !timeout_result.timed_out() || !inner.queue.is_empty() {
                    if let Some(job_id) = inner.queue.pop_front() {
                        return inner.jobs.get(&job_id).cloned();
                    }
                }
                None
            }
            Err(_) => None,
        }
    }

    pub fn stop(&self) {
        *self.stop_flag.lock().unwrap() = true;
        self.queue_condvar.notify_all();
    }

    pub fn is_stopped(&self) -> bool {
        *self.stop_flag.lock().unwrap()
    }
}

// ---------------------------------------------------------------------------
// Socket client
// ---------------------------------------------------------------------------

pub fn send_daemon_request(
    socket_path: &str,
    payload: &serde_json::Value,
) -> Result<serde_json::Value, TianJiError> {
    use std::io::{Read, Write};
    use std::os::unix::net::UnixStream;

    let mut stream = UnixStream::connect(socket_path)?;
    let request_bytes = serde_json::to_string(payload).map_err(TianJiError::Json)? + "\n";
    stream.write_all(request_bytes.as_bytes())?;
    stream.shutdown(std::net::Shutdown::Write)?;

    let mut response = String::new();
    stream.read_to_string(&mut response)?;

    let decoded: serde_json::Value =
        serde_json::from_str(response.trim()).map_err(TianJiError::Json)?;
    if !decoded.is_object() {
        return Err(TianJiError::Usage(
            "daemon response was not a JSON object".to_string(),
        ));
    }
    Ok(decoded)
}

// ---------------------------------------------------------------------------
// Socket server handler
// ---------------------------------------------------------------------------

pub fn handle_socket_request(
    state: &Arc<DaemonState>,
    request: &serde_json::Value,
) -> serde_json::Value {
    let action = match request.get("action").and_then(|v| v.as_str()) {
        Some(a) => a,
        None => {
            return serde_json::json!({
                "ok": false,
                "error": { "message": "request field 'action' must be a string" }
            });
        }
    };

    if action == "queue_run" {
        let payload = match request.get("payload") {
            Some(p) if p.is_object() => p,
            _ => {
                return serde_json::json!({
                    "ok": false,
                    "error": { "message": "queue_run requires an object 'payload'" }
                });
            }
        };

        let run_request = match RunJobRequest::from_payload(payload) {
            Ok(r) => r,
            Err(e) => {
                return serde_json::json!({
                    "ok": false,
                    "error": { "message": e }
                });
            }
        };

        let record = state.enqueue_job(run_request);
        // ALWAYS return state: "queued" per contract parity
        return serde_json::json!({
            "ok": true,
            "data": {
                "job_id": record.job_id,
                "state": "queued",
            },
            "error": null,
        });
    }

    if action == "job_status" {
        let job_id = match request.get("job_id").and_then(|v| v.as_str()) {
            Some(id) => id,
            None => {
                return serde_json::json!({
                    "ok": false,
                    "error": { "message": "job_status requires string field 'job_id'" }
                });
            }
        };

        match state.get_job(job_id) {
            Some(record) => {
                return serde_json::json!({
                    "ok": true,
                    "data": record.to_status_payload(),
                    "error": null,
                });
            }
            None => {
                return serde_json::json!({
                    "ok": false,
                    "error": { "message": format!("unknown job_id '{job_id}'") }
                });
            }
        }
    }

    serde_json::json!({
        "ok": false,
        "error": { "message": format!("unsupported action '{action}'") }
    })
}

// ---------------------------------------------------------------------------
// Worker loop
// ---------------------------------------------------------------------------

pub fn worker_loop(state: &Arc<DaemonState>) {
    loop {
        if state.is_stopped() {
            break;
        }

        let record = state.pop_next_job(100);
        let record = match record {
            Some(r) => r,
            None => continue,
        };

        state.set_job_running(&record.job_id);

        // Run pipeline
        let result = run_pipeline_for_job(&record.request);

        match result {
            Ok(()) => {
                let run_id = if let Some(ref sqlite_path) = record.request.sqlite_path {
                    get_latest_run_id(sqlite_path).ok().flatten()
                } else {
                    None
                };
                state.set_job_succeeded(&record.job_id, run_id);
            }
            Err(e) => {
                let error_message = e.to_string();
                state.set_job_failed(&record.job_id, error_message);
            }
        }
    }
}

fn run_pipeline_for_job(request: &RunJobRequest) -> Result<(), String> {
    // Currently only fixture mode is supported in Rust
    for fixture_path in &request.fixture_paths {
        run_fixture_path(fixture_path, request.sqlite_path.as_deref())
            .map_err(|e| format!("TianJiError: {e}"))?;
    }
    Ok(())
}

// ---------------------------------------------------------------------------
// Serve entry point
// ---------------------------------------------------------------------------

pub fn serve(
    socket_path: &str,
    sqlite_path: &str,
    host: &str,
    port: u16,
) -> Result<(), TianJiError> {
    let validated_host = validate_loopback_host(host)?;

    // Ensure parent directory exists
    if let Some(parent) = Path::new(socket_path).parent() {
        std::fs::create_dir_all(parent)?;
    }

    // Remove stale socket file
    let _ = std::fs::remove_file(socket_path);

    let state = Arc::new(DaemonState::new());
    let sqlite_path_owned = sqlite_path.to_string();
    let socket_path_owned = socket_path.to_string();

    let rt = tokio::runtime::Runtime::new()
        .map_err(|e| TianJiError::Usage(format!("Failed to create tokio runtime: {e}")))?;

    rt.block_on(async move {
        let state_clone = state.clone();
        let state_clone2 = state.clone();
        let socket_path_for_cleanup = socket_path_owned.clone();

        // Spawn API server
        let api_handle = tokio::spawn(async move {
            if let Err(e) = crate::api::serve_api(&validated_host, port, &sqlite_path_owned).await {
                eprintln!("API server error: {e}");
            }
        });

        // Spawn socket listener
        let socket_handle = tokio::spawn(async move {
            if let Err(e) = listen_socket(&socket_path_owned, &state_clone).await {
                eprintln!("Socket listener error: {e}");
            }
        });

        // Run worker loop in a separate thread (blocking)
        let worker_handle = std::thread::spawn(move || {
            worker_loop(&state_clone2);
        });

        // Wait for shutdown signal
        tokio::signal::ctrl_c().await.ok();

        state.stop();

        // Clean up
        let _ = api_handle.await;
        let _ = socket_handle.await;
        let _ = worker_handle.join();
        let _ = std::fs::remove_file(&socket_path_for_cleanup);
    });

    Ok(())
}

async fn listen_socket(socket_path: &str, state: &Arc<DaemonState>) -> Result<(), TianJiError> {
    use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
    use tokio::net::UnixListener;

    let listener = UnixListener::bind(socket_path)
        .map_err(|e| TianJiError::Usage(format!("Failed to bind socket: {e}")))?;

    loop {
        let (stream, _) = listener.accept().await.map_err(TianJiError::Io)?;
        let state = state.clone();

        tokio::spawn(async move {
            let (reader, mut writer) = stream.into_split();
            let mut buf_reader = BufReader::new(reader);
            let mut line = String::new();

            match buf_reader.read_line(&mut line).await {
                Ok(0) | Err(_) => return,
                Ok(_) => {}
            }

            let request: serde_json::Value =
                match serde_json::from_str::<serde_json::Value>(line.trim()) {
                    Ok(v) if v.is_object() => v,
                    _ => {
                        let error_resp = serde_json::json!({
                            "ok": false,
                            "error": { "message": "request body must be a JSON object" }
                        });
                        let _ = writer.write_all(format!("{error_resp}\n").as_bytes()).await;
                        let _ = writer.shutdown().await;
                        return;
                    }
                };

            let response = handle_socket_request(&state, &request);
            let response_str = serde_json::to_string(&response).unwrap_or_default();
            let _ = writer
                .write_all(format!("{response_str}\n").as_bytes())
                .await;
            let _ = writer.shutdown().await;
        });
    }
}
