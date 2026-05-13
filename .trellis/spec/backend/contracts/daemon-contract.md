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
   - `tianji run ...` (Rust binary) is the direct source-of-truth write path.
   - `tianji daemon run ...` and `tianji daemon schedule ...` submit that same work unit for background execution.
   - Python oracle equivalent: `python3 -m tianji run ...`

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

## Process Lifecycle Contract

### 1. Scope / Trigger

- Trigger: `tianji daemon start` launches the current executable as a detached `daemon serve` child process and the parent process returns after socket/API readiness checks.

### 2. Signatures

- `tianji daemon start --sqlite-path <path> --socket-path <path> --pid-path <path> --host <loopback> --port <port>`
- Internal child command: `tianji daemon serve --sqlite-path <path> --socket-path <path> --host <loopback> --port <port>`

### 3. Contracts

- `--host` must validate to a loopback host before bind or readiness URL construction.
- IPv6 loopback hosts must be bracketed when formatted as socket addresses or URLs, e.g. `[::1]:8765` and `http://[::1]:8765/api/v1/meta`.
- The parent process must retain the spawned `Child` until the readiness checks complete.
- On startup failure after spawn, the parent must terminate and wait on the child before returning an error. Dropping `Child` without `wait` is forbidden because it can leave zombie processes.
- On startup success, the parent may return without waiting because the daemon child has become the long-lived process advertised by the PID file.

### 4. Validation & Error Matrix

| Condition | Behavior |
|-----------|----------|
| Host is not loopback | Return a usage/input error before spawn/bind |
| Child process cannot be spawned | Return an I/O error and do not write a PID file |
| Socket or API readiness times out | Remove the PID file, terminate the child, wait/reap it, then return a startup error |
| Child exits before readiness | Reap the child and return a startup error |

### 5. Good/Base/Bad Cases

- Good: `tianji daemon start --host 127.0.0.1 --port 8765` writes a PID file only for a daemon that passed readiness checks.
- Base: `tianji daemon start --host ::1 --port 8765` formats checks as `[::1]:8765` / `http://[::1]:8765/...`.
- Bad: spawning the child, timing out, calling `kill`, and returning without `wait`.

### 6. Tests Required

- Unit-test loopback address formatting for IPv4 and IPv6.
- Unit-test host validation rejects non-loopback hosts.
- Regression-test failure-path cleanup where feasible without depending on a long-lived external process.

### 7. Wrong vs Correct

#### Wrong

```rust
let child = cmd.spawn()?;
if !ready {
    child.kill()?;
    return Err(error);
}
```

#### Correct

```rust
let mut child = cmd.spawn()?;
if !ready {
    let _ = child.kill();
    let _ = child.wait();
    return Err(error);
}
```

## Operator Commands

Start the daemon and hosted read API:

```bash
# Rust (Milestone 5+)
tianji daemon start --sqlite-path runs/tianji.sqlite3 --socket-path runs/tianji.sock --host 127.0.0.1 --port 8765

# Python oracle (current)
python3 -m tianji daemon start --sqlite-path runs/tianji.sqlite3 --socket-path runs/tianji.sock --host 127.0.0.1 --port 8765
```

Inspect daemon availability:

```bash
# Python oracle
python3 -m tianji daemon status --socket-path runs/tianji.sock
```

Inspect one queued job:

```bash
python3 -m tianji daemon status --socket-path runs/tianji.sock --job-id 1
```

Queue one background run:

```bash
python3 -m tianji daemon run --socket-path runs/tianji.sock --fixture tests/fixtures/sample_feed.xml
```

Queue a bounded repeated run set:

```bash
python3 -m tianji daemon schedule --socket-path runs/tianji.sock --every-seconds 300 --count 3 --fixture tests/fixtures/sample_feed.xml
```

Stop the daemon:

```bash
python3 -m tianji daemon stop --socket-path runs/tianji.sock
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
- `local-api-contract.md` defines the read-first HTTP routes hosted by the daemon.
- `tui-contract.md` defines the storage-backed read-only terminal browser.
- `web-ui-contract.md` defines the optional separate browser UI that consumes the same local API.

## Non-Goals

This contract does not include:

- remote daemon access
- HTTP write endpoints for run submission or scheduling
- cron or calendar scheduling
- streaming progress over HTTP
- browser-specific backend routes outside the existing local surfaces
