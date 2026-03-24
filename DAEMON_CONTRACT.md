# TianJi Daemon Contract

## Purpose

This document defines the shipped daemon contract for TianJi's local background runtime.

The daemon is intentionally narrow. It gives operators one local process that:

- listens on a UNIX socket control plane at `runs/tianji.sock` by default
- hosts the read-first loopback HTTP API at `127.0.0.1:8765` by default
- queues the same one-run pipeline work unit that synchronous CLI `run` executes directly

The daemon is not a second source of truth for writes. The synchronous CLI `run` command remains the canonical immediate write path.

## Boundaries

1. **CLI remains the write authority**
   - `python3 -m tianji run ...` is still the direct source-of-truth write path.
   - `python3 -m tianji daemon run ...` and `python3 -m tianji daemon schedule ...` submit that same work unit for background execution.

2. **Local-first and loopback-only**
   - The daemon socket is local filesystem IPC.
   - The hosted HTTP API binds to `127.0.0.1` by default.
   - No remote deployment, auth model, or multi-tenant assumptions are part of this contract.

3. **Queue-oriented, not a general scheduler**
   - Background work is a bounded queue of one-run pipeline invocations.
   - Repeated scheduling is limited to `--every-seconds N` plus `--count M`.
   - Cron-like calendars and distributed workers are outside this contract.

4. **Read HTTP, control socket**
   - UNIX socket commands control daemon lifecycle and queued jobs.
   - HTTP remains read-first for metadata and persisted run history.

## Default Runtime Values

- socket path: `runs/tianji.sock`
- API host: `127.0.0.1`
- API port: `8765`
- SQLite path in examples: `runs/tianji.sqlite3`

## Operator Commands

Start the daemon and hosted read API:

```bash
.venv/bin/python -m tianji daemon start --sqlite-path runs/tianji.sqlite3 --socket-path runs/tianji.sock --host 127.0.0.1 --port 8765
```

Inspect daemon availability:

```bash
.venv/bin/python -m tianji daemon status --socket-path runs/tianji.sock
```

Inspect one queued job:

```bash
.venv/bin/python -m tianji daemon status --socket-path runs/tianji.sock --job-id 1
```

Queue one background run:

```bash
.venv/bin/python -m tianji daemon run --socket-path runs/tianji.sock --fixture tests/fixtures/sample_feed.xml
```

Queue a bounded repeated run set:

```bash
.venv/bin/python -m tianji daemon schedule --socket-path runs/tianji.sock --every-seconds 300 --count 3 --fixture tests/fixtures/sample_feed.xml
```

Stop the daemon:

```bash
.venv/bin/python -m tianji daemon stop --socket-path runs/tianji.sock
```

## Job Lifecycle

Queued jobs move through exactly these lifecycle states:

- `queued`
- `running`
- `succeeded`
- `failed`

No additional persisted or API-exposed lifecycle vocabulary is part of this contract.

## Relationship to Other Surfaces

- `README.md` is the operator-facing summary.
- `LOCAL_API_CONTRACT.md` defines the read-first HTTP routes hosted by the daemon.
- `TUI_CONTRACT.md` defines the storage-backed read-only terminal browser.
- `WEB_UI_CONTRACT.md` defines the optional separate browser UI that consumes the same local API.

## Non-Goals

This contract does not include:

- remote daemon access
- HTTP write endpoints for run submission or scheduling
- cron or calendar scheduling
- streaming progress over HTTP
- browser-specific backend routes outside the existing local surfaces
