# Milestone 3: Local Runtime Parity

**Created**: 2026-05-13
**Assignee**: kita
**Priority**: P1

## Goal

Port the Python daemon/API runtime layer to Rust so that `tianji daemon start/stop/status/run/schedule` and the read-first loopback HTTP API produce field-for-field compatible behavior with the Python oracle.

## What I Already Know

* Milestone 1A+1B complete (feed + normalize + score + group + backtrack).
* Milestone 2 complete (SQLite storage + history CLI, 33 tests pass).
* Python daemon layer spans 4 modules: `daemon.py` (core), `api.py` (HTTP), `webui_server.py` (web UI), `cli_daemon.py` (CLI subcommands).
* Python uses `ThreadingMixIn + UnixStreamServer` for socket, `ThreadingHTTPServer` for API — Rust will use `tokio` + `axum` + Unix socket.
* Daemon contract: UNIX socket control plane (JSON-lines), HTTP read API, loopback-only, queue-oriented.
* 4 job states: `queued → running → succeeded/failed`. No retry, no cancellation, no persistence across restarts.
* Web UI (port 8766) is optional, off by default, consumes daemon API via reverse proxy.
* PID file convention: `{socket_path}.pid` sibling to socket file.
* Schedule validation: `--every-seconds >= 60`, `--count >= 1`.

## Requirements

### 3A — Daemon Core (tokio runtime + worker loop)

* `tianji daemon start --socket-path <path> --sqlite-path <path> --host 127.0.0.1 --port 8765` — spawn daemon process.
* `tianji daemon stop --socket-path <path>` — SIGTERM then SIGKILL after 2s timeout, remove PID file and socket file.
* `tianji daemon status --socket-path <path>` — check PID file + socket existence.
* `tianji daemon run --socket-path <path> --fixture <path>` — queue a run via UNIX socket.
* `tianji daemon schedule` — **deferred** (see 3C in Decisions).
* Worker loop: pop job from queue → run pipeline → set succeeded(run_id) / failed(error).
* Jobs in-memory only (HashMap + channel), not persisted.
* Job ID format: `"job-{uuid4_hex[:12]}"`.
* Loopback enforcement: host must be `127.0.0.1`, `localhost`, or `::1`.

### 3B — UNIX Socket Control Plane

* AF_UNIX, SOCK_STREAM, JSON-lines protocol (one JSON object per line).
* `queue_run` action: accept `payload` (RunJobRequest), return `{ok: true, data: {job_id, state: "queued"}, error: null}`.
* `job_status` action: accept `job_id`, return `{ok: true, data: {job_id, state, run_id, error}, error: null}`.
* Unrecognized action: return `{ok: false, error: {message: "..."}}`.
* `queue_run` response always says `state: "queued"` even if job has already transitioned to `running`.

### 3C — HTTP Read API (axum)

* `GET /api/v1/meta` — static contract metadata, matches `local_api_meta_v1.json`.
* `GET /api/v1/runs?limit=N` — mirrors `history` list, matches `local_api_runs_v1.json` envelope.
* `GET /api/v1/runs/{run_id}` — mirrors `history-show`, 404 with `run_not_found` on missing.
* `GET /api/v1/runs/latest` — convenience alias for newest run.
* `GET /api/v1/compare?left_run_id=X&right_run_id=Y` — mirrors `history-compare`, 400 with `invalid_query` on malformed params.
* Response envelope: `{api_version: "v1", data: <payload>, error: null}` or `{api_version: "v1", data: null, error: {code, message}}`.
* Content-Type: `application/json; charset=utf-8`.
* API_VERSION = `"v1"`.
* Loopback-only binding.

### 3D — Web UI Server (optional, off by default)

* `tianji webui --host 127.0.0.1 --port 8766 --api-base-url http://127.0.0.1:8765 --socket-path runs/tianji.sock` — serve static files + reverse proxy + queue-run endpoint.
* Reverse proxy: `/api/v1/*` → upstream API server, 5s timeout, 502 `upstream_unavailable` on failure.
* `POST /queue-run` — accept JSON `{fixture_path}`, send `queue_run` to socket with 2s retry for FileNotFoundError/ConnectionRefusedError.
* Static files from `tianji/webui/` with `Cache-Control: no-store`.
* Redirect `/` → `/index.html`.

### 3E — CLI Daemon Subcommands (clap)

* `daemon start` — spawn daemon subprocess, write PID file, wait for socket + API readiness.
* `daemon stop` — SIGTERM/SIGKILL, remove PID file + socket.
* `daemon status [--job-id]` — check daemon state or specific job.
* `daemon run` — queue a run via socket.
* `daemon schedule` — scheduled queue runs.
* All subcommands output JSON to stdout.

## Acceptance Criteria

* [ ] `tianji daemon start` spawns daemon, socket + API become ready within 2s.
* [ ] `tianji daemon stop` cleanly shuts down daemon, removes PID + socket files.
* [ ] `GET /api/v1/meta` returns envelope matching `local_api_meta_v1.json` fixture.
* [ ] `GET /api/v1/runs` returns items matching `history_list_item_v1.json` vocabulary.
* [ ] `GET /api/v1/runs/{id}` returns detail matching `history_detail_v1.json`.
* [ ] `GET /api/v1/compare` returns payload matching `history_compare_v1.json`.
* [ ] `queue_run` via socket returns `{ok: true, data: {job_id, state: "queued"}}`.
* [ ] `job_status` via socket returns `{job_id, state, run_id, error}` matching `daemon_job_status_v1.json`.
* [ ] `tianji daemon run --fixture <path>` queues and completes a run.
* [ ] ~~`tianji daemon schedule`~~ — deferred to 3C.
* [ ] Non-loopback host rejected with error.
* [ ] Web UI serves static files + proxies API + handles `/queue-run`.
* [ ] `cargo test` passes, `cargo fmt --check` clean, `cargo clippy -- -D warnings` clean.

## Definition of Done

* Tests added/updated (unit + integration)
* Lint / typecheck / CI green
* Contract fixture verification (API + socket responses match Python vocabulary)
* Python code preserved

## Out of Scope

* TUI (Milestone 4)
* LLM provider configuration (later phase)
* Job persistence across daemon restarts (future)
* WebSocket / streaming endpoints
* Cron-style scheduler (beyond `schedule` command)
* Authentication / multi-tenant
* Write HTTP routes (POST /runs etc.)

## Technical Notes

### Files Inspected

* `tianji/daemon.py` — DaemonState, TianJiUnixDaemonServer, TianJiHttpApiServer, worker loop, serve()
* `tianji/api.py` — HTTP routes, response envelope, error codes
* `tianji/webui_server.py` — reverse proxy, queue-run, static file serving
* `tianji/cli_daemon.py` — daemon start/stop/status/run/schedule subcommands, PID file management
* `tianji/cli.py` — CLI group wiring
* `tests/test_daemon.py` — 10 test cases covering API parity, socket protocol, lifecycle, loopback enforcement
* `tests/test_webui.py` — 2 test cases covering static shell, API proxy, queue-run
* `.trellis/spec/backend/contracts/daemon-contract.md` — daemon contract
* `.trellis/spec/backend/contracts/local-api-contract.md` — API contract
* `.trellis/spec/backend/contracts/web-ui-contract.md` — web UI contract
* `tests/fixtures/contracts/daemon_queue_request_v1.json` — frozen queue request
* `tests/fixtures/contracts/daemon_job_status_v1.json` — frozen job status
* `tests/fixtures/contracts/local_api_meta_v1.json` — frozen meta response
* `tests/fixtures/contracts/local_api_runs_v1.json` — frozen runs response
* `tests/fixtures/contracts/local_api_compare_v1.json` — frozen compare response

### Key Constraints

* Dependencies to add: `tokio`, `axum`, `reqwest`, `uuid` (for job IDs), `hyper` (axum brings this).
* `plan.md` §11 lists tokio, axum, reqwest for Milestone 5 but they can be added at Milestone 3 since daemon/API needs them now.
* No async runtimes existed before this milestone — this is the first tokio introduction.
* Current flat `src/*.rs` structure continues; new modules: `src/daemon.rs`, `src/api.rs`, `src/webui.rs`.
* Python uses ThreadingMixIn; Rust uses tokio tasks. Parity is at the protocol/behavior level, not the concurrency model.
* `tianji/webui/` static files remain as-is (served by Rust instead of Python).
* PID file + socket file management on daemon start/stop must match Python exactly for CLI parity.

## Decisions (ADR-lite)

| # | Decision | Rationale |
|---|---|---|
| D1 | Flat modules: `src/daemon.rs`, `src/api.rs`, `src/webui.rs` | Match M2 D6 principle: parity before architecture. Split into `src/daemon/` later when it exceeds ~500-700 lines and interfaces are stable. |
| D2 | Single tokio runtime + spawned tasks | Local daemon is naturally one process with 3 concurrent concerns (socket, API, worker). No CPU isolation or different reactor policy needs. Shared `Arc<AppState>` for job state. |
| D3 | Subprocess model: `daemon start` spawns `tianji daemon serve ...` as child | Behavior parity with Python's `start_new_session=True`. PID management, parent-child exit boundaries, and CLI return semantics are all well-defined. `daemon serve` is a hidden internal subcommand. |
| D4 | `tianji webui` top-level subcommand | Matches web-ui-contract.md and Python oracle (`python -m tianji webui` is top-level). Web UI is a separate optional surface, not a daemon sub-operation. |
| D5 | Schedule deferred to post-M3 | M3 scope = bounded daemon controls + read-first API. Schedule adds timer lifecycle, repeated submission semantics, and doubles the test matrix. Mark as 3C/future. |
| D6 | Compile-time embed for static files (`include_str!`/`include_bytes!` or `include_dir`) | Single-binary distribution. No runtime dependency on `tianji/webui/` directory. Contract alignment is at the behavior level, not the Python file-reading implementation. |

## Milestone 3 Slice Plan

### 3A — Daemon + API (core)

- `src/daemon.rs` — DaemonState (Arc<RwLock<HashMap>> + channel), worker loop, socket listener, `serve` entry
- `src/api.rs` — axum router, 5 read routes, response envelope, error codes
- CLI: `daemon start`, `daemon stop`, `daemon status`, `daemon run`, hidden `daemon serve`
- Dependencies: `tokio`, `axum`, `uuid`, `hyper` (via axum)

### 3B — Web UI (optional surface)

- `src/webui.rs` — static file serve (compile-time embed), reverse proxy, `/queue-run` POST
- CLI: `tianji webui`
- Dependencies: `reqwest` (for reverse proxy)

### 3C — Schedule (deferred)

- `daemon schedule --every-seconds N --count N --fixture <path>`
- Timer lifecycle, repeated submission
- Separate mini-milestone after 3A+3B are stable
