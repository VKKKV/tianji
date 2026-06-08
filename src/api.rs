use axum::{
    body::Bytes,
    extract::{Path, Query, State},
    http::{HeaderMap, HeaderValue, StatusCode},
    response::IntoResponse,
    routing::{get, post},
    Router,
};
use hmac::{Hmac, KeyInit, Mac};
use serde::Deserialize;
use serde_json::Value as JsonValue;
use sha2::{Digest, Sha256};
use std::collections::{HashSet, VecDeque};
use std::sync::{Arc, Mutex};
use std::time::{SystemTime, UNIX_EPOCH};

use crate::storage::SqlitePool;

const JSON_CONTENT_TYPE: &str = "application/json; charset=utf-8";

/// Wrapper that ensures `Content-Type: application/json; charset=utf-8`.
struct JsonEnvelope(JsonValue);

impl IntoResponse for JsonEnvelope {
    fn into_response(self) -> axum::response::Response {
        let mut response = axum::Json(self.0).into_response();
        let headers = response.headers_mut();
        headers.insert(
            axum::http::header::CONTENT_TYPE,
            HeaderValue::from_static(JSON_CONTENT_TYPE),
        );
        response
    }
}

const API_VERSION: &str = "v1";
const RUN_ARTIFACT_SCHEMA_VERSION: &str = "tianji.run-artifact.v1";
const MAX_RUNS_LIMIT: usize = 200;
const AGENT_COMMAND_TIMESTAMP_TOLERANCE_SECS: i64 = 300;
const AGENT_COMMAND_NONCE_CACHE_CAPACITY: usize = 1024;
const HMAC_SHA256_SIGNATURE_BYTES: usize = 32;

type HmacSha256 = Hmac<Sha256>;

// ---------------------------------------------------------------------------
// Error mapping
// ---------------------------------------------------------------------------

fn tianji_error_to_api(e: crate::TianJiError) -> ApiError {
    use crate::TianJiError;
    match &e {
        TianJiError::Usage(_) | TianJiError::Input(_) => ApiError {
            status: StatusCode::BAD_REQUEST,
            body: error_envelope("invalid_request", "Bad request"),
        },
        TianJiError::Io(io_err) if io_err.kind() == std::io::ErrorKind::NotFound => ApiError {
            status: StatusCode::NOT_FOUND,
            body: error_envelope("not_found", "Resource not found"),
        },
        TianJiError::Storage(rusqlite::Error::QueryReturnedNoRows) => ApiError {
            status: StatusCode::NOT_FOUND,
            body: error_envelope("not_found", "Resource not found"),
        },
        TianJiError::Storage(_) => ApiError {
            status: StatusCode::INTERNAL_SERVER_ERROR,
            body: error_envelope("internal_error", "An internal error occurred"),
        },
        _ => ApiError {
            status: StatusCode::INTERNAL_SERVER_ERROR,
            body: error_envelope("internal_error", "An internal error occurred"),
        },
    }
}
const API_RUN_SUMMARY_EVENT_LIMIT: usize = 200;
const API_RUN_SUMMARY_GROUP_LIMIT: usize = 200;

fn api_meta_data() -> JsonValue {
    serde_json::json!({
        "artifact_schema_version": RUN_ARTIFACT_SCHEMA_VERSION,
        "cli_source_of_truth": true,
        "compare_resources": {
            "mirrored_backend_surface": ["history-compare"],
            "payload_fixture": "tests/fixtures/contracts/history_compare_v1.json",
            "v1_routes": [
                "GET /api/v1/compare?left_run_id=<id>&right_run_id=<id>",
            ],
        },
        "persistence": {
            "sqlite_optional": true,
        },
        "resources": [
            "/api/v1/meta",
            "/api/v1/health",
            "/api/v1/ready",
            "/api/v1/runs",
            "/api/v1/runs/{run_id}",
            "/api/v1/compare?left_run_id=<id>&right_run_id=<id>",
            "/api/v1/delta/latest",
        ],
    })
}

// ---------------------------------------------------------------------------
// App state
// ---------------------------------------------------------------------------

#[derive(Clone)]
pub struct AppState {
    pub sqlite_pool: SqlitePool,
    pub sqlite_path: String,
    agent_command_secret: Option<Arc<[u8]>>,
    agent_command_nonces: Arc<Mutex<NonceCache>>,
}

impl AppState {
    pub fn new(sqlite_path: impl Into<String>) -> Result<Self, crate::TianJiError> {
        let sqlite_path = sqlite_path.into();
        Ok(Self {
            sqlite_pool: SqlitePool::default(&sqlite_path)?,
            sqlite_path,
            agent_command_secret: None,
            agent_command_nonces: Arc::new(Mutex::new(NonceCache::default())),
        })
    }

    pub fn with_agent_command_secret(
        sqlite_path: impl Into<String>,
        secret: impl Into<Vec<u8>>,
    ) -> Result<Self, crate::TianJiError> {
        let mut state = Self::new(sqlite_path)?;
        state.agent_command_secret = Some(Arc::from(secret.into().into_boxed_slice()));
        Ok(state)
    }
}

#[derive(Default)]
struct NonceCache {
    order: VecDeque<NonceEntry>,
    keys: HashSet<String>,
}

struct NonceEntry {
    key: String,
    timestamp: i64,
}

impl NonceCache {
    fn insert_once(&mut self, key: String, timestamp: i64, now: i64) -> bool {
        self.prune(now);
        if self.keys.contains(&key) {
            return false;
        }
        self.keys.insert(key.clone());
        self.order.push_back(NonceEntry { key, timestamp });
        while self.order.len() > AGENT_COMMAND_NONCE_CACHE_CAPACITY {
            if let Some(entry) = self.order.pop_front() {
                self.keys.remove(&entry.key);
            }
        }
        true
    }

    fn prune(&mut self, now: i64) {
        while let Some(entry) = self.order.front() {
            if now - entry.timestamp <= AGENT_COMMAND_TIMESTAMP_TOLERANCE_SECS {
                break;
            }
            if let Some(entry) = self.order.pop_front() {
                self.keys.remove(&entry.key);
            }
        }
    }
}

// ---------------------------------------------------------------------------
// Response envelope
// ---------------------------------------------------------------------------

fn success_envelope(data: JsonValue) -> JsonValue {
    serde_json::json!({
        "api_version": API_VERSION,
        "data": data,
        "error": null,
    })
}

fn error_envelope(code: &str, message: &str) -> JsonValue {
    serde_json::json!({
        "api_version": API_VERSION,
        "data": null,
        "error": {
            "code": code,
            "message": message,
        },
    })
}

// ---------------------------------------------------------------------------
// Error response
// ---------------------------------------------------------------------------

struct ApiError {
    status: StatusCode,
    body: JsonValue,
}

impl IntoResponse for ApiError {
    fn into_response(self) -> axum::response::Response {
        let mut response = (self.status, axum::Json(self.body)).into_response();
        let headers = response.headers_mut();
        headers.insert(
            axum::http::header::CONTENT_TYPE,
            HeaderValue::from_static(JSON_CONTENT_TYPE),
        );
        response
    }
}

// ---------------------------------------------------------------------------
// Query params
// ---------------------------------------------------------------------------

#[derive(Deserialize, Default)]
pub struct RunsQuery {
    pub limit: Option<u32>,
}

#[derive(Deserialize, Default)]
pub struct CompareQuery {
    pub left_run_id: Option<String>,
    pub right_run_id: Option<String>,
}

#[derive(Deserialize, Default)]
pub struct DeltaLatestQuery {}

#[derive(Deserialize)]
struct AgentCommandRequest {
    command_id: String,
    command_type: String,
    #[allow(dead_code)]
    payload: JsonValue,
}

// ---------------------------------------------------------------------------
// Route handlers
// ---------------------------------------------------------------------------

async fn get_meta() -> impl IntoResponse {
    let data = api_meta_data();
    JsonEnvelope(success_envelope(data))
}

async fn get_health() -> impl IntoResponse {
    let data = serde_json::json!({
        "status": "ok",
        "checks": {
            "api": "ok",
        },
    });
    JsonEnvelope(success_envelope(data))
}

async fn get_ready(State(state): State<AppState>) -> Result<impl IntoResponse, ApiError> {
    let connection = state.sqlite_pool.get().map_err(|_| ApiError {
        status: StatusCode::SERVICE_UNAVAILABLE,
        body: error_envelope("not_ready", "SQLite connection pool is not ready"),
    })?;

    let query_result = connection.query_row("SELECT 1", [], |row| row.get::<_, i64>(0));
    match query_result {
        Ok(1) => {
            let data = serde_json::json!({
                "status": "ready",
                "checks": {
                    "api": "ok",
                    "sqlite": "ok",
                },
                "sqlite_path": state.sqlite_path,
            });
            Ok(JsonEnvelope(success_envelope(data)))
        }
        Ok(_) | Err(_) => Err(ApiError {
            status: StatusCode::SERVICE_UNAVAILABLE,
            body: error_envelope("not_ready", "SQLite readiness query failed"),
        }),
    }
}

async fn get_runs(
    State(state): State<AppState>,
    Query(params): Query<RunsQuery>,
) -> Result<impl IntoResponse, ApiError> {
    let limit = match params.limit {
        Some(l) if l as usize > MAX_RUNS_LIMIT => {
            return Err(ApiError {
                status: StatusCode::BAD_REQUEST,
                body: error_envelope(
                    "invalid_query",
                    &format!("limit exceeds maximum of {MAX_RUNS_LIMIT}"),
                ),
            });
        }
        Some(l) if l > 0 => l as usize,
        _ => 20,
    };

    let filters = crate::storage::RunListFilters::default();
    let connection = state.sqlite_pool.get().map_err(tianji_error_to_api)?;
    let items = crate::storage::list_runs_with_conn(&connection, limit, &filters)
        .map_err(tianji_error_to_api)?;

    let data = serde_json::json!({
        "resource": "/api/v1/runs",
        "item_contract_fixture": "tests/fixtures/contracts/history_list_item_v1.json",
        "items": items,
    });

    Ok(JsonEnvelope(success_envelope(data)))
}

async fn get_run_by_id(
    State(state): State<AppState>,
    Path(run_id): Path<i64>,
) -> Result<impl IntoResponse, ApiError> {
    if run_id <= 0 {
        return Err(ApiError {
            status: StatusCode::BAD_REQUEST,
            body: error_envelope("invalid_query", "run_id must be a positive integer"),
        });
    }

    let scored_filters = api_scored_event_filters();
    let group_filters = api_event_group_filters();
    let connection = state.sqlite_pool.get().map_err(tianji_error_to_api)?;
    let payload = crate::storage::get_run_summary_with_conn(
        &connection,
        run_id,
        &scored_filters,
        false,
        &group_filters,
    )
    .map_err(tianji_error_to_api)?;

    match payload {
        Some(p) => Ok(JsonEnvelope(success_envelope(p))),
        None => Err(ApiError {
            status: StatusCode::NOT_FOUND,
            body: error_envelope("run_not_found", &format!("Run not found: {run_id}")),
        }),
    }
}

async fn get_latest_run(State(state): State<AppState>) -> Result<impl IntoResponse, ApiError> {
    let connection = state.sqlite_pool.get().map_err(tianji_error_to_api)?;
    let latest_run_id =
        crate::storage::get_latest_run_id_with_conn(&connection).map_err(tianji_error_to_api)?;

    let run_id = match latest_run_id {
        Some(id) => id,
        None => {
            return Err(ApiError {
                status: StatusCode::NOT_FOUND,
                body: error_envelope("run_not_found", "Run not found: latest"),
            });
        }
    };

    let scored_filters = api_scored_event_filters();
    let group_filters = api_event_group_filters();
    let payload = crate::storage::get_run_summary_with_conn(
        &connection,
        run_id,
        &scored_filters,
        false,
        &group_filters,
    )
    .map_err(tianji_error_to_api)?;

    match payload {
        Some(p) => Ok(JsonEnvelope(success_envelope(p))),
        None => Err(ApiError {
            status: StatusCode::NOT_FOUND,
            body: error_envelope("run_not_found", &format!("Run not found: {run_id}")),
        }),
    }
}

async fn get_compare(
    State(state): State<AppState>,
    Query(params): Query<CompareQuery>,
) -> Result<impl IntoResponse, ApiError> {
    let left_run_id = match params.left_run_id.as_deref() {
        Some(s) => match s.parse::<i64>() {
            Ok(id) if id > 0 => id,
            _ => {
                return Err(ApiError {
                    status: StatusCode::BAD_REQUEST,
                    body: error_envelope(
                        "invalid_query",
                        "Malformed compare query: expected positive integer query fields 'left_run_id' and 'right_run_id'",
                    ),
                });
            }
        },
        None => {
            return Err(ApiError {
                status: StatusCode::BAD_REQUEST,
                body: error_envelope(
                    "invalid_query",
                    "Malformed compare query: expected positive integer query fields 'left_run_id' and 'right_run_id'",
                ),
            });
        }
    };

    let right_run_id = match params.right_run_id.as_deref() {
        Some(s) => match s.parse::<i64>() {
            Ok(id) if id > 0 => id,
            _ => {
                return Err(ApiError {
                    status: StatusCode::BAD_REQUEST,
                    body: error_envelope(
                        "invalid_query",
                        "Malformed compare query: expected positive integer query fields 'left_run_id' and 'right_run_id'",
                    ),
                });
            }
        },
        None => {
            return Err(ApiError {
                status: StatusCode::BAD_REQUEST,
                body: error_envelope(
                    "invalid_query",
                    "Malformed compare query: expected positive integer query fields 'left_run_id' and 'right_run_id'",
                ),
            });
        }
    };

    let scored_filters = api_scored_event_filters();
    let group_filters = api_event_group_filters();
    let connection = state.sqlite_pool.get().map_err(tianji_error_to_api)?;
    let result = crate::storage::compare_runs_with_conn(
        &connection,
        left_run_id,
        right_run_id,
        &scored_filters,
        false,
        &group_filters,
    )
    .map_err(tianji_error_to_api)?;

    match result {
        Some(r) => {
            let payload = serde_json::json!({
                "left_run_id": r.left_run_id,
                "right_run_id": r.right_run_id,
                "left": r.left,
                "right": r.right,
                "diff": r.diff,
            });
            Ok(JsonEnvelope(success_envelope(payload)))
        }
        None => Err(ApiError {
            status: StatusCode::NOT_FOUND,
            body: error_envelope(
                "run_not_found",
                &format!(
                    "Run not found for compare pair: left_run_id={left_run_id}, right_run_id={right_run_id}"
                ),
            ),
        }),
    }
}

async fn get_latest_delta(State(state): State<AppState>) -> Result<impl IntoResponse, ApiError> {
    let sqlite_path = &state.sqlite_path;
    let memory = crate::HotMemory::load(&crate::delta_memory_path(sqlite_path));
    let Some(run) = memory.runs.front() else {
        let payload = serde_json::json!({
            "run_id": null,
            "timestamp": null,
            "alert_tier": null,
            "delta": null,
        });
        return Ok(JsonEnvelope(success_envelope(payload)));
    };
    let Some(delta) = run.delta.as_ref() else {
        let payload = serde_json::json!({
            "run_id": run.run_id,
            "timestamp": run.timestamp,
            "alert_tier": null,
            "delta": null,
        });
        return Ok(JsonEnvelope(success_envelope(payload)));
    };

    let payload = serde_json::json!({
        "run_id": run.run_id,
        "timestamp": run.timestamp,
        "alert_tier": crate::classify_delta_tier(delta),
        "delta": delta,
    });
    Ok(JsonEnvelope(success_envelope(payload)))
}

async fn post_agent_command(
    State(state): State<AppState>,
    headers: HeaderMap,
    body: Bytes,
) -> Result<impl IntoResponse, ApiError> {
    let auth = validate_agent_command_auth(&state, &headers, &body)?;
    let command: AgentCommandRequest = serde_json::from_slice(&body).map_err(|_| ApiError {
        status: StatusCode::BAD_REQUEST,
        body: error_envelope("invalid_command", "Invalid agent command"),
    })?;

    if !agent_tier_allows_command(&auth.tier, &command.command_type) {
        return Err(ApiError {
            status: StatusCode::FORBIDDEN,
            body: error_envelope("forbidden", "Agent tier cannot perform this command"),
        });
    }

    let payload = serde_json::json!({
        "accepted": true,
        "command_id": command.command_id,
        "agent_id": auth.agent_id,
        "tier": auth.tier,
        "command_type": command.command_type,
    });
    Ok(JsonEnvelope(success_envelope(payload)))
}

struct AgentCommandAuth {
    agent_id: String,
    tier: String,
}

fn validate_agent_command_auth(
    state: &AppState,
    headers: &HeaderMap,
    body: &[u8],
) -> Result<AgentCommandAuth, ApiError> {
    let secret = state
        .agent_command_secret
        .as_ref()
        .ok_or_else(|| ApiError {
            status: StatusCode::SERVICE_UNAVAILABLE,
            body: error_envelope(
                "agent_command_unavailable",
                "Agent command channel is unavailable",
            ),
        })?;

    let agent_id = required_header(headers, "x-tianji-agent-id")?;
    let tier = required_header(headers, "x-tianji-agent-tier")?;
    if !matches!(tier.as_str(), "restricted" | "full") {
        return Err(agent_auth_error());
    }

    let timestamp = required_header(headers, "x-tianji-timestamp")?;
    let timestamp_value = timestamp.parse::<i64>().map_err(|_| agent_auth_error())?;
    let now = unix_timestamp_secs().map_err(|_| ApiError {
        status: StatusCode::INTERNAL_SERVER_ERROR,
        body: error_envelope("internal_error", "An internal error occurred"),
    })?;
    if (now - timestamp_value).abs() > AGENT_COMMAND_TIMESTAMP_TOLERANCE_SECS {
        return Err(agent_auth_error());
    }

    let nonce = required_header(headers, "x-tianji-nonce")?;
    if nonce.is_empty() || nonce.len() > 128 {
        return Err(agent_auth_error());
    }
    let signature = required_header(headers, "x-tianji-signature")?;
    let signature_bytes = decode_hex_sha256_signature(&signature).ok_or_else(agent_auth_error)?;

    let message = agent_command_signature_message(&timestamp, &nonce, body);
    let mut mac = HmacSha256::new_from_slice(secret).map_err(|_| ApiError {
        status: StatusCode::INTERNAL_SERVER_ERROR,
        body: error_envelope("internal_error", "An internal error occurred"),
    })?;
    mac.update(message.as_bytes());
    mac.verify_slice(&signature_bytes)
        .map_err(|_| agent_auth_error())?;

    let nonce_key = format!("{agent_id}\n{nonce}");
    let inserted = state
        .agent_command_nonces
        .lock()
        .map_err(|_| ApiError {
            status: StatusCode::INTERNAL_SERVER_ERROR,
            body: error_envelope("internal_error", "An internal error occurred"),
        })?
        .insert_once(nonce_key, timestamp_value, now);
    if !inserted {
        return Err(agent_auth_error());
    }

    Ok(AgentCommandAuth { agent_id, tier })
}

fn required_header(headers: &HeaderMap, name: &'static str) -> Result<String, ApiError> {
    headers
        .get(name)
        .and_then(|value| value.to_str().ok())
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(ToOwned::to_owned)
        .ok_or_else(agent_auth_error)
}

fn agent_auth_error() -> ApiError {
    ApiError {
        status: StatusCode::UNAUTHORIZED,
        body: error_envelope("agent_auth_failed", "Agent command authentication failed"),
    }
}

fn agent_tier_allows_command(tier: &str, command_type: &str) -> bool {
    match tier {
        "restricted" => matches!(command_type, "observe" | "query"),
        "full" => matches!(command_type, "observe" | "query" | "simulate" | "intervene"),
        _ => false,
    }
}

fn agent_command_signature_message(timestamp: &str, nonce: &str, body: &[u8]) -> String {
    let digest = Sha256::digest(body);
    format!("{timestamp}\n{nonce}\n{}", encode_hex(&digest))
}

fn decode_hex_sha256_signature(value: &str) -> Option<[u8; HMAC_SHA256_SIGNATURE_BYTES]> {
    if value.len() != HMAC_SHA256_SIGNATURE_BYTES * 2 {
        return None;
    }
    let mut bytes = [0_u8; HMAC_SHA256_SIGNATURE_BYTES];
    for (index, chunk) in value.as_bytes().chunks_exact(2).enumerate() {
        let high = hex_nibble(chunk[0])?;
        let low = hex_nibble(chunk[1])?;
        bytes[index] = (high << 4) | low;
    }
    Some(bytes)
}

fn encode_hex(bytes: &[u8]) -> String {
    const HEX: &[u8; 16] = b"0123456789abcdef";
    let mut output = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        output.push(HEX[(byte >> 4) as usize] as char);
        output.push(HEX[(byte & 0x0f) as usize] as char);
    }
    output
}

fn hex_nibble(byte: u8) -> Option<u8> {
    match byte {
        b'0'..=b'9' => Some(byte - b'0'),
        b'a'..=b'f' => Some(byte - b'a' + 10),
        _ => None,
    }
}

fn unix_timestamp_secs() -> Result<i64, std::time::SystemTimeError> {
    Ok(SystemTime::now().duration_since(UNIX_EPOCH)?.as_secs() as i64)
}

fn api_scored_event_filters() -> crate::storage::ScoredEventFilters {
    crate::storage::ScoredEventFilters {
        limit_scored_events: Some(API_RUN_SUMMARY_EVENT_LIMIT),
        ..Default::default()
    }
}

fn api_event_group_filters() -> crate::storage::EventGroupFilters {
    crate::storage::EventGroupFilters {
        limit_event_groups: Some(API_RUN_SUMMARY_GROUP_LIMIT),
        ..Default::default()
    }
}

// ---------------------------------------------------------------------------
// Fallback (route_not_found)
// ---------------------------------------------------------------------------

async fn fallback() -> impl IntoResponse {
    ApiError {
        status: StatusCode::NOT_FOUND,
        body: error_envelope("route_not_found", "Route not found"),
    }
    .into_response()
}

// ---------------------------------------------------------------------------
// Serve API
// ---------------------------------------------------------------------------

/// Build the API router without state (for testing or composition).
pub fn build_router() -> Router<AppState> {
    Router::new()
        .route("/api/v1/meta", get(get_meta))
        .route("/api/v1/health", get(get_health))
        .route("/api/v1/ready", get(get_ready))
        .route("/api/v1/runs", get(get_runs))
        .route("/api/v1/runs/latest", get(get_latest_run))
        .route("/api/v1/runs/{run_id}", get(get_run_by_id))
        .route("/api/v1/compare", get(get_compare))
        .route("/api/v1/delta/latest", get(get_latest_delta))
        .route("/api/v1/agent/command", post(post_agent_command))
        .fallback(fallback)
}

pub async fn serve_api(host: &str, port: u16, sqlite_path: &str) -> Result<(), String> {
    let state =
        AppState::new(sqlite_path).map_err(|e| format!("Failed to initialize API state: {e}"))?;

    serve_api_with_state(host, port, state).await
}

pub async fn serve_api_with_state(host: &str, port: u16, state: AppState) -> Result<(), String> {
    let app = build_router().with_state(state);

    let addr = crate::daemon::loopback_socket_addr(host, port);
    let listener = tokio::net::TcpListener::bind(&addr)
        .await
        .map_err(|e| format!("Failed to bind API server: {e}"))?;

    axum::serve(listener, app)
        .await
        .map_err(|e| format!("API server error: {e}"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicU64, Ordering};

    const TEST_SECRET: &[u8] = b"deterministic-agent-command-test-secret";

    #[test]
    fn api_health_returns_liveness_envelope() {
        let db_path = temp_sqlite_path();
        let rt = tokio::runtime::Runtime::new().expect("runtime");
        let response = rt.block_on(async {
            let state = AppState::new(db_path.clone()).expect("api state");
            let server = TestServer::start(state).await;
            let response = get_test_request(&server.url, "/api/v1/health").await;
            server.handle.abort();
            response
        });
        cleanup_db(&db_path);

        assert_eq!(response.status, StatusCode::OK);
        assert_eq!(
            response.body,
            serde_json::json!({
                "api_version": "v1",
                "data": {
                    "status": "ok",
                    "checks": {
                        "api": "ok"
                    }
                },
                "error": null
            })
        );
    }

    #[test]
    fn api_ready_returns_sqlite_readiness_envelope() {
        let db_path = temp_sqlite_path();
        let rt = tokio::runtime::Runtime::new().expect("runtime");
        let response = rt.block_on(async {
            let state = AppState::new(db_path.clone()).expect("api state");
            let server = TestServer::start(state).await;
            let response = get_test_request(&server.url, "/api/v1/ready").await;
            server.handle.abort();
            response
        });
        cleanup_db(&db_path);

        assert_eq!(response.status, StatusCode::OK);
        assert_eq!(
            response.body,
            serde_json::json!({
                "api_version": "v1",
                "data": {
                    "status": "ready",
                    "checks": {
                        "api": "ok",
                        "sqlite": "ok"
                    },
                    "sqlite_path": db_path
                },
                "error": null
            })
        );
    }

    #[test]
    fn agent_command_valid_signed_command_is_accepted() {
        let response = run_agent_command_request(
            "restricted",
            "observe",
            current_test_timestamp(),
            "valid-nonce",
            TEST_SECRET,
        );

        assert_eq!(response.status, StatusCode::OK);
        assert_eq!(response.body["error"], JsonValue::Null);
        assert_eq!(response.body["data"]["accepted"], true);
        assert_eq!(response.body["data"]["agent_id"], "agent-a");
        assert_eq!(response.body["data"]["tier"], "restricted");
        assert_eq!(response.body["data"]["command_type"], "observe");
    }

    #[test]
    fn api_agent_command_contract_accepts_signed_command_and_rejects_bad_signature() {
        let accepted = run_agent_command_request(
            "full",
            "simulate",
            current_test_timestamp(),
            "contract-accepted-nonce",
            TEST_SECRET,
        );
        let rejected = run_agent_command_request(
            "full",
            "simulate",
            current_test_timestamp(),
            "contract-rejected-nonce",
            b"wrong-contract-secret",
        );

        assert_eq!(accepted.status, StatusCode::OK);
        assert_eq!(
            accepted.body,
            serde_json::json!({
                "api_version": "v1",
                "data": {
                    "accepted": true,
                    "command_id": "cmd-test-1",
                    "agent_id": "agent-a",
                    "tier": "full",
                    "command_type": "simulate"
                },
                "error": null
            })
        );

        assert_eq!(rejected.status, StatusCode::UNAUTHORIZED);
        assert_eq!(
            rejected.body,
            serde_json::json!({
                "api_version": "v1",
                "data": null,
                "error": {
                    "code": "agent_auth_failed",
                    "message": "Agent command authentication failed"
                }
            })
        );
    }

    #[test]
    fn agent_command_bad_signature_is_rejected() {
        let response = run_agent_command_request(
            "restricted",
            "query",
            current_test_timestamp(),
            "bad-signature-nonce",
            b"wrong-secret",
        );

        assert_eq!(response.status, StatusCode::UNAUTHORIZED);
        assert_eq!(
            response.body["error"]["code"],
            JsonValue::String("agent_auth_failed".to_string())
        );
    }

    #[test]
    fn agent_command_stale_timestamp_is_rejected() {
        let response = run_agent_command_request(
            "restricted",
            "query",
            current_test_timestamp() - 1_000,
            "stale-nonce",
            TEST_SECRET,
        );

        assert_eq!(response.status, StatusCode::UNAUTHORIZED);
        assert_eq!(
            response.body["error"]["code"],
            JsonValue::String("agent_auth_failed".to_string())
        );
    }

    #[test]
    fn agent_command_nonce_replay_is_rejected() {
        let db_path = temp_sqlite_path();
        let rt = tokio::runtime::Runtime::new().expect("runtime");
        rt.block_on(async {
            let state = AppState::with_agent_command_secret(db_path.clone(), TEST_SECRET.to_vec())
                .expect("api state");
            let server = TestServer::start(state).await;
            let timestamp = current_test_timestamp();
            let body = command_body("query");
            let signature =
                test_signature(TEST_SECRET, timestamp, "replayed-nonce", body.as_bytes());
            let client = reqwest::Client::new();

            let first = post_agent_command_test_request(
                &client,
                &server.url,
                "restricted",
                timestamp,
                "replayed-nonce",
                &signature,
                &body,
            )
            .await;
            let replay = post_agent_command_test_request(
                &client,
                &server.url,
                "restricted",
                timestamp,
                "replayed-nonce",
                &signature,
                &body,
            )
            .await;

            assert_eq!(first.status, StatusCode::OK);
            assert_eq!(replay.status, StatusCode::UNAUTHORIZED);

            server.handle.abort();
        });
        cleanup_db(&db_path);
    }

    #[test]
    fn agent_command_restricted_tier_denies_intervene() {
        let response = run_agent_command_request(
            "restricted",
            "intervene",
            current_test_timestamp(),
            "restricted-denied-nonce",
            TEST_SECRET,
        );

        assert_eq!(response.status, StatusCode::FORBIDDEN);
        assert_eq!(
            response.body["error"]["code"],
            JsonValue::String("forbidden".to_string())
        );
    }

    struct TestResponse {
        status: StatusCode,
        body: JsonValue,
    }

    struct TestServer {
        url: String,
        handle: tokio::task::JoinHandle<()>,
    }

    impl TestServer {
        async fn start(state: AppState) -> Self {
            let app = build_router().with_state(state);
            let listener = tokio::net::TcpListener::bind("127.0.0.1:0")
                .await
                .expect("bind");
            let addr = listener.local_addr().expect("addr");
            let handle = tokio::spawn(async move {
                axum::serve(listener, app).await.expect("serve");
            });
            tokio::time::sleep(std::time::Duration::from_millis(50)).await;
            Self {
                url: format!("http://{addr}"),
                handle,
            }
        }
    }

    fn run_agent_command_request(
        tier: &str,
        command_type: &str,
        timestamp: i64,
        nonce: &str,
        signing_secret: &[u8],
    ) -> TestResponse {
        let db_path = temp_sqlite_path();
        let rt = tokio::runtime::Runtime::new().expect("runtime");
        let response = rt.block_on(async {
            let state = AppState::with_agent_command_secret(db_path.clone(), TEST_SECRET.to_vec())
                .expect("api state");
            let server = TestServer::start(state).await;
            let body = command_body(command_type);
            let signature = test_signature(signing_secret, timestamp, nonce, body.as_bytes());
            let client = reqwest::Client::new();
            let response = post_agent_command_test_request(
                &client,
                &server.url,
                tier,
                timestamp,
                nonce,
                &signature,
                &body,
            )
            .await;
            server.handle.abort();
            response
        });
        cleanup_db(&db_path);
        response
    }

    async fn post_agent_command_test_request(
        client: &reqwest::Client,
        base_url: &str,
        tier: &str,
        timestamp: i64,
        nonce: &str,
        signature: &str,
        body: &str,
    ) -> TestResponse {
        let response = client
            .post(format!("{base_url}/api/v1/agent/command"))
            .header("x-tianji-agent-id", "agent-a")
            .header("x-tianji-agent-tier", tier)
            .header("x-tianji-timestamp", timestamp.to_string())
            .header("x-tianji-nonce", nonce)
            .header("x-tianji-signature", signature)
            .header("content-type", "application/json")
            .body(body.to_string())
            .send()
            .await
            .expect("request");
        let status = response.status();
        let body = serde_json::from_str(&response.text().await.expect("text")).expect("json");
        TestResponse { status, body }
    }

    async fn get_test_request(base_url: &str, path: &str) -> TestResponse {
        let response = reqwest::Client::new()
            .get(format!("{base_url}{path}"))
            .send()
            .await
            .expect("request");
        let status = response.status();
        let body = serde_json::from_str(&response.text().await.expect("text")).expect("json");
        TestResponse { status, body }
    }

    fn command_body(command_type: &str) -> String {
        serde_json::json!({
            "command_id": "cmd-test-1",
            "command_type": command_type,
            "payload": {},
        })
        .to_string()
    }

    fn test_signature(secret: &[u8], timestamp: i64, nonce: &str, body: &[u8]) -> String {
        let message = agent_command_signature_message(&timestamp.to_string(), nonce, body);
        let mut mac = HmacSha256::new_from_slice(secret).expect("hmac key");
        mac.update(message.as_bytes());
        encode_hex(&mac.finalize().into_bytes())
    }

    fn current_test_timestamp() -> i64 {
        unix_timestamp_secs().expect("unix timestamp")
    }

    fn temp_sqlite_path() -> String {
        static COUNTER: AtomicU64 = AtomicU64::new(0);
        let id = COUNTER.fetch_add(1, Ordering::Relaxed);
        let process_id = std::process::id();
        let path = format!("/tmp/tianji_agent_command_test_{process_id}_{id}.sqlite3");
        cleanup_db(&path);
        path
    }

    fn cleanup_db(path: &str) {
        let _ = std::fs::remove_file(path);
        let _ = std::fs::remove_file(format!("{path}-wal"));
        let _ = std::fs::remove_file(format!("{path}-shm"));
    }
}
