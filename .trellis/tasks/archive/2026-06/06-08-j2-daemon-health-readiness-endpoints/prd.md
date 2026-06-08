# J2 daemon health readiness endpoints

## Purpose

Continue Phase J operational reliability by adding explicit local HTTP health/readiness probes for TianJi daemon/API operations.

Operators and scripts should not have to infer readiness from `/api/v1/meta`; they should have dedicated endpoints for:
- API process liveness.
- SQLite-backed API readiness.

## Scope

In scope:
- Add `GET /api/v1/health`.
- Add `GET /api/v1/ready`.
- Include both endpoints in `/api/v1/meta` resources.
- Switch daemon startup readiness polling from `/api/v1/meta` to `/api/v1/ready`.
- Add API tests for health and ready success.
- Add a readiness URL helper or testable behavior for daemon startup URL construction if useful.
- Update README and plan for J2.

Out of scope:
- Kubernetes/systemd manifests.
- Socket listener health in HTTP readiness.
- Worker/job queue introspection.
- Schema migrations.
- Network/external dependency checks.

## API contract

### GET /api/v1/health

Liveness only. Must not query SQLite.

Response status: 200 OK

```json
{
  "api_version": "v1",
  "data": {
    "status": "ok",
    "checks": {
      "api": "ok"
    }
  },
  "error": null
}
```

### GET /api/v1/ready

Readiness for serving SQLite-backed API requests. Must verify that the SQLite pool can provide a connection and a trivial query succeeds.

Response status: 200 OK when ready.

```json
{
  "api_version": "v1",
  "data": {
    "status": "ready",
    "checks": {
      "api": "ok",
      "sqlite": "ok"
    },
    "sqlite_path": "runs/tianji.sqlite3"
  },
  "error": null
}
```

If not ready, return 503 Service Unavailable with a diagnostic JSON envelope. Do not leak secrets.

## Acceptance criteria

1. `/api/v1/health` returns 200 and the envelope above.
2. `/api/v1/ready` returns 200 and includes `sqlite_path` plus `api/sqlite` checks when SQLite is available.
3. `/api/v1/meta` resources include `/api/v1/health` and `/api/v1/ready`.
4. `daemon start` readiness wait uses `/api/v1/ready`, and error text points at `/api/v1/ready`.
5. README documents health/readiness endpoints.
6. `plan.md` records J2 as complete and updates test counts if needed.

## Verification commands

```bash
cargo test health
cargo test ready
cargo test daemon
cargo test
cargo fmt --check
cargo clippy -- -D warnings
git diff --check
```
