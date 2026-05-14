use axum::{
    extract::State,
    http::{HeaderMap, StatusCode},
    response::IntoResponse,
    routing::{get, post},
    Router,
};
use serde::Deserialize;

use crate::TianJiError;

const QUEUE_RUN_SOCKET_READY_TIMEOUT_MS: u64 = 2000;
const QUEUE_RUN_SOCKET_RETRY_INTERVAL_MS: u64 = 50;

// ---------------------------------------------------------------------------
// App state
// ---------------------------------------------------------------------------

#[derive(Clone)]
pub struct WebUiState {
    pub api_base_url: String,
    pub socket_path: String,
    pub sqlite_path: Option<String>,
}

// ---------------------------------------------------------------------------
// Static file embedding
// ---------------------------------------------------------------------------

const INDEX_HTML: &str = include_str!("webui/index.html");
const APP_JS: &str = include_str!("webui/app.js");
const STYLES_CSS: &str = include_str!("webui/styles.css");

// ---------------------------------------------------------------------------
// Route handlers
// ---------------------------------------------------------------------------

async fn redirect_index() -> impl IntoResponse {
    (StatusCode::FOUND, [("Location", "/index.html")], "")
}

async fn serve_index() -> impl IntoResponse {
    let mut headers = HeaderMap::new();
    headers.insert("Content-Type", "text/html; charset=utf-8".parse().unwrap());
    headers.insert("Cache-Control", "no-store".parse().unwrap());
    (StatusCode::OK, headers, INDEX_HTML)
}

async fn serve_app_js() -> impl IntoResponse {
    let mut headers = HeaderMap::new();
    headers.insert(
        "Content-Type",
        "application/javascript; charset=utf-8".parse().unwrap(),
    );
    headers.insert("Cache-Control", "no-store".parse().unwrap());
    (StatusCode::OK, headers, APP_JS)
}

async fn serve_styles_css() -> impl IntoResponse {
    let mut headers = HeaderMap::new();
    headers.insert("Content-Type", "text/css; charset=utf-8".parse().unwrap());
    headers.insert("Cache-Control", "no-store".parse().unwrap());
    (StatusCode::OK, headers, STYLES_CSS)
}

async fn proxy_api(
    State(state): State<WebUiState>,
    axum::extract::OriginalUri(uri): axum::extract::OriginalUri,
) -> impl IntoResponse {
    let path_and_query = uri
        .path_and_query()
        .map(|pq| pq.as_str())
        .unwrap_or(uri.path());

    let upstream_url = format!(
        "{}{}",
        state.api_base_url.trim_end_matches('/'),
        path_and_query
    );

    let client = reqwest::Client::new();
    match client
        .get(&upstream_url)
        .timeout(std::time::Duration::from_secs(5))
        .header("Accept", "application/json")
        .send()
        .await
    {
        Ok(response) => {
            let status =
                StatusCode::from_u16(response.status().as_u16()).unwrap_or(StatusCode::BAD_GATEWAY);
            let body = response.bytes().await.unwrap_or_default();
            let mut headers = HeaderMap::new();
            headers.insert(
                "Content-Type",
                "application/json; charset=utf-8".parse().unwrap(),
            );
            headers.insert("Cache-Control", "no-store".parse().unwrap());
            (status, headers, body.to_vec())
        }
        Err(_) => {
            let body = serde_json::json!({
                "api_version": "v1",
                "data": null,
                "error": {
                    "code": "upstream_unavailable",
                    "message": "Optional web UI could not reach the local API.",
                },
            });
            let body_bytes = serde_json::to_vec(&body).unwrap_or_default();
            let mut headers = HeaderMap::new();
            headers.insert(
                "Content-Type",
                "application/json; charset=utf-8".parse().unwrap(),
            );
            headers.insert("Cache-Control", "no-store".parse().unwrap());
            (StatusCode::BAD_GATEWAY, headers, body_bytes)
        }
    }
}

// ---------------------------------------------------------------------------
// Queue-run handler
// ---------------------------------------------------------------------------

#[derive(Deserialize)]
struct QueueRunRequest {
    fixture_path: String,
}

struct QueueRunError {
    status: StatusCode,
    body: serde_json::Value,
}

impl IntoResponse for QueueRunError {
    fn into_response(self) -> axum::response::Response {
        (self.status, axum::Json(self.body)).into_response()
    }
}

async fn handle_queue_run(
    State(state): State<WebUiState>,
    axum::Json(request): axum::Json<QueueRunRequest>,
) -> Result<impl IntoResponse, QueueRunError> {
    let fixture_path = request.fixture_path.trim().to_string();
    if fixture_path.is_empty() {
        return Err(QueueRunError {
            status: StatusCode::BAD_REQUEST,
            body: serde_json::json!({
                "ok": false,
                "data": null,
                "error": { "message": "fixture_path must be a non-empty string" },
            }),
        });
    }

    let mut run_payload = serde_json::json!({
        "fixture_paths": [fixture_path],
    });
    if let Some(ref sqlite_path) = state.sqlite_path {
        run_payload["sqlite_path"] = serde_json::Value::String(sqlite_path.clone());
    }

    let daemon_request = serde_json::json!({
        "action": "queue_run",
        "payload": run_payload,
    });

    // Retry with timeout
    let deadline = std::time::Instant::now()
        + std::time::Duration::from_millis(QUEUE_RUN_SOCKET_READY_TIMEOUT_MS);
    let mut last_error: Option<String>;

    loop {
        match crate::daemon::send_daemon_request(&state.socket_path, &daemon_request) {
            Ok(response) => {
                let ok = response
                    .get("ok")
                    .and_then(|v| v.as_bool())
                    .unwrap_or(false);
                if ok {
                    return Ok(axum::Json(response));
                } else {
                    let error_msg = response
                        .get("error")
                        .and_then(|e| e.get("message"))
                        .and_then(|m| m.as_str())
                        .unwrap_or("Daemon returned an error");
                    return Err(QueueRunError {
                        status: StatusCode::BAD_REQUEST,
                        body: serde_json::json!({
                            "ok": false,
                            "data": null,
                            "error": { "message": error_msg },
                        }),
                    });
                }
            }
            Err(TianJiError::Io(ref e)) if e.kind() == std::io::ErrorKind::NotFound => {
                last_error = Some(format!("FileNotFoundError: {e}"));
            }
            Err(TianJiError::Io(ref e)) if e.kind() == std::io::ErrorKind::ConnectionRefused => {
                last_error = Some(format!("ConnectionRefusedError: {e}"));
            }
            Err(e) => {
                return Err(QueueRunError {
                    status: StatusCode::BAD_REQUEST,
                    body: serde_json::json!({
                        "ok": false,
                        "data": null,
                        "error": { "message": format!("{e}") },
                    }),
                });
            }
        }

        if std::time::Instant::now() >= deadline {
            let msg = last_error
                .unwrap_or_else(|| "queue-run proxy could not reach daemon socket".to_string());
            return Err(QueueRunError {
                status: StatusCode::BAD_REQUEST,
                body: serde_json::json!({
                    "ok": false,
                    "data": null,
                    "error": { "message": msg },
                }),
            });
        }

        tokio::time::sleep(std::time::Duration::from_millis(
            QUEUE_RUN_SOCKET_RETRY_INTERVAL_MS,
        ))
        .await;
    }
}

// ---------------------------------------------------------------------------
// Fallback static file handler
// ---------------------------------------------------------------------------

async fn fallback_static(
    axum::extract::OriginalUri(uri): axum::extract::OriginalUri,
) -> impl IntoResponse {
    let path = uri.path();
    let mut headers = HeaderMap::new();
    headers.insert("Cache-Control", "no-store".parse().unwrap());

    match path {
        "/index.html" => {
            headers.insert("Content-Type", "text/html; charset=utf-8".parse().unwrap());
            (StatusCode::OK, headers, INDEX_HTML.to_string())
        }
        "/app.js" => {
            headers.insert(
                "Content-Type",
                "application/javascript; charset=utf-8".parse().unwrap(),
            );
            (StatusCode::OK, headers, APP_JS.to_string())
        }
        "/styles.css" => {
            headers.insert("Content-Type", "text/css; charset=utf-8".parse().unwrap());
            (StatusCode::OK, headers, STYLES_CSS.to_string())
        }
        _ => (StatusCode::NOT_FOUND, headers, String::new()),
    }
}

// ---------------------------------------------------------------------------
// Serve entry
// ---------------------------------------------------------------------------

pub async fn serve_webui(
    host: &str,
    port: u16,
    api_base_url: &str,
    socket_path: &str,
    sqlite_path: Option<&str>,
) -> Result<(), String> {
    let validated_host = crate::daemon::validate_loopback_host(host).map_err(|e| e.to_string())?;

    let state = WebUiState {
        api_base_url: api_base_url.to_string(),
        socket_path: socket_path.to_string(),
        sqlite_path: sqlite_path.map(String::from),
    };

    let app = Router::new()
        .route("/", get(redirect_index))
        .route("/index.html", get(serve_index))
        .route("/app.js", get(serve_app_js))
        .route("/styles.css", get(serve_styles_css))
        .route("/api/v1/{*path}", get(proxy_api))
        .route("/queue-run", post(handle_queue_run))
        .fallback(fallback_static)
        .with_state(state);

    let addr = crate::daemon::loopback_socket_addr(&validated_host, port);
    let listener = tokio::net::TcpListener::bind(&addr)
        .await
        .map_err(|e| format!("Failed to bind web UI server: {e}"))?;

    axum::serve(listener, app)
        .await
        .map_err(|e| format!("Web UI server error: {e}"))
}
