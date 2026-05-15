use axum::{
    extract::{Path, Query, State},
    http::{HeaderValue, StatusCode},
    response::IntoResponse,
    routing::get,
    Router,
};
use serde::Deserialize;
use serde_json::Value as JsonValue;

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
    pub sqlite_path: String,
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
pub struct DeltaLatestQuery {
    pub sqlite_path: Option<String>,
}

// ---------------------------------------------------------------------------
// Route handlers
// ---------------------------------------------------------------------------

async fn get_meta() -> impl IntoResponse {
    let data = api_meta_data();
    JsonEnvelope(success_envelope(data))
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
    let items = crate::list_runs(&state.sqlite_path, limit, &filters).map_err(|e| ApiError {
        status: StatusCode::INTERNAL_SERVER_ERROR,
        body: error_envelope("internal_error", &e.to_string()),
    })?;

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
    let payload = crate::get_run_summary(
        &state.sqlite_path,
        run_id,
        &scored_filters,
        false,
        &group_filters,
    )
    .map_err(|e| ApiError {
        status: StatusCode::INTERNAL_SERVER_ERROR,
        body: error_envelope("internal_error", &e.to_string()),
    })?;

    match payload {
        Some(p) => Ok(JsonEnvelope(success_envelope(p))),
        None => Err(ApiError {
            status: StatusCode::NOT_FOUND,
            body: error_envelope("run_not_found", &format!("Run not found: {run_id}")),
        }),
    }
}

async fn get_latest_run(State(state): State<AppState>) -> Result<impl IntoResponse, ApiError> {
    let latest_run_id = crate::get_latest_run_id(&state.sqlite_path).map_err(|e| ApiError {
        status: StatusCode::INTERNAL_SERVER_ERROR,
        body: error_envelope("internal_error", &e.to_string()),
    })?;

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
    let payload = crate::get_run_summary(
        &state.sqlite_path,
        run_id,
        &scored_filters,
        false,
        &group_filters,
    )
    .map_err(|e| ApiError {
        status: StatusCode::INTERNAL_SERVER_ERROR,
        body: error_envelope("internal_error", &e.to_string()),
    })?;

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
    let result = crate::compare_runs(
        &state.sqlite_path,
        left_run_id,
        right_run_id,
        &scored_filters,
        false,
        &group_filters,
    )
    .map_err(|e| ApiError {
        status: StatusCode::INTERNAL_SERVER_ERROR,
        body: error_envelope("internal_error", &e.to_string()),
    })?;

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

async fn get_latest_delta(
    State(state): State<AppState>,
    Query(params): Query<DeltaLatestQuery>,
) -> Result<impl IntoResponse, ApiError> {
    let sqlite_path = params.sqlite_path.unwrap_or(state.sqlite_path);
    let memory = crate::HotMemory::load(&crate::delta_memory_path(&sqlite_path));
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
        .route("/api/v1/runs", get(get_runs))
        .route("/api/v1/runs/latest", get(get_latest_run))
        .route("/api/v1/runs/{run_id}", get(get_run_by_id))
        .route("/api/v1/compare", get(get_compare))
        .route("/api/v1/delta/latest", get(get_latest_delta))
        .fallback(fallback)
}

pub async fn serve_api(host: &str, port: u16, sqlite_path: &str) -> Result<(), String> {
    let state = AppState {
        sqlite_path: sqlite_path.to_string(),
    };

    let app = build_router().with_state(state);

    let addr = crate::daemon::loopback_socket_addr(host, port);
    let listener = tokio::net::TcpListener::bind(&addr)
        .await
        .map_err(|e| format!("Failed to bind API server: {e}"))?;

    axum::serve(listener, app)
        .await
        .map_err(|e| format!("API server error: {e}"))
}
