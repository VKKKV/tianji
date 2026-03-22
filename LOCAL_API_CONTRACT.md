# TianJi Local API Contract (Draft)

## Purpose

This document defines the first future-facing local API contract for TianJi.

It is a **contract draft**, not a claim that TianJi already ships an HTTP server, daemon, or web UI. The goal is to preserve alignment between:

- the current CLI operator surface
- the current SQLite-backed run-history surface
- a later optional local API layer for a decoupled web UI

## Design Principles

1. **CLI remains the source of truth for writes first**
   - The current product surface is still `python3 -m tianji ...`.
   - The first local API slice should be read-first.

2. **Reuse existing artifact and history shapes**
   - Do not invent a separate domain model for the API if the CLI/storage payloads are already stable enough.
   - Field names should stay aligned with current output such as `schema_version`, `scenario_summary`, `scored_events`, and `intervention_candidates`.

3. **Local-only assumptions**
   - No cloud dependency, auth layer, or distributed service assumptions in this contract draft.
   - The API is intended for a future local process boundary only.

4. **Read-only first**
   - Expose current persisted data and metadata first.
   - Defer write-triggering endpoints until a real local service runtime exists.

## Current Stable Backend Surfaces

These are the current product-aligned surfaces the future API should mirror:

- `python3 -m tianji run ...`
  - one synchronous pipeline execution
  - emits one schema-versioned artifact

- `python3 -m tianji history --sqlite-path ...`
  - lists persisted runs from SQLite

- `python3 -m tianji history-show --sqlite-path ... --run-id N`
  - returns persisted run-level detail
  - includes `input_summary`, `scenario_summary`, `scored_events`, and `intervention_candidates`

## Recommended First API Resources

### 1. `GET /api/v1/meta`

Purpose:
- expose static contract metadata for a future UI
- avoid implying live runtime status before a daemon exists

Recommended response:

```json
{
  "api_version": "v1",
  "data": {
    "cli_source_of_truth": true,
    "artifact_schema_version": "tianji.run-artifact.v1",
    "persistence": {
      "sqlite_optional": true
    }
  },
  "error": null
}
```

### 2. `GET /api/v1/runs?limit=N`

Purpose:
- API form of current `history`

Recommended `data` shape:

```json
[
  {
    "run_id": 3,
    "schema_version": "tianji.run-artifact.v1",
    "mode": "fixture",
    "generated_at": "2026-03-22T10:00:00+00:00",
    "raw_item_count": 3,
    "normalized_event_count": 3,
    "dominant_field": "technology",
    "risk_level": "high",
    "headline": "The strongest current branch is technology..."
  }
]
```

### 3. `GET /api/v1/runs/{run_id}`

Purpose:
- API form of current `history-show`

Recommended `data` shape:
- `run_id`
- `schema_version`
- `mode`
- `generated_at`
- `input_summary`
- `scenario_summary`
- `scored_events`
- `intervention_candidates`

This endpoint should preserve the current persisted detail vocabulary rather than remapping fields.

### 4. `GET /api/v1/runs/latest`

Purpose:
- convenience alias for the newest persisted run

Notes:
- optional in the first implementation
- should return the same payload shape as `GET /api/v1/runs/{run_id}`
- undefined when no persisted runs exist yet

## Response Envelope

To keep future UI consumers stable, use one envelope pattern for all responses:

```json
{
  "api_version": "v1",
  "data": {},
  "error": null
}
```

Error example:

```json
{
  "api_version": "v1",
  "data": null,
  "error": {
    "code": "run_not_found",
    "message": "Run not found: 7"
  }
}
```

## What To Defer

Do **not** include these in the first local API slice:

- live progress or status endpoints
- WebSocket streaming
- scheduler or daemon control endpoints
- source-registry management endpoints
- compare endpoints across runs
- standalone resources for `event_groups`, `scored_events`, or `intervention_candidates`
- auth, users, sessions, or multi-tenant concepts

These either do not exist in the current product surface or would force architecture choices before TianJi has a stable long-running runtime.

## Mapping to Current TianJi Code

- `tianji/cli.py`
  - current operator commands: `run`, `history`, `history-show`

- `tianji/pipeline.py`
  - defines the unit of work: one run -> one artifact

- `tianji/models.py`
  - current artifact schema vocabulary

- `tianji/storage.py`
  - current persisted read/write boundary for runs and run details

## Risks to Avoid

1. **Shape drift**
   - If API payloads diverge from CLI artifact/history payloads too early, TianJi will maintain two domain models for the same concept.

2. **Premature live-runtime assumptions**
   - Current TianJi is still one-shot and CLI-first. Avoid “status” or “job” semantics until a daemon actually exists.

3. **Overcommitting writes**
   - A future `POST /runs` may exist, but the first contract draft should not require it to ship before the local service boundary is ready.

## Recommended Implementation Order (Later)

When TianJi is ready to actually implement the local API:

1. `GET /api/v1/meta`
2. `GET /api/v1/runs`
3. `GET /api/v1/runs/{run_id}`
4. optional `GET /api/v1/runs/latest`
5. only then consider write endpoints

This keeps the first API slice aligned with the current stable product reality: read-first, local-first, deterministic, and optional.
