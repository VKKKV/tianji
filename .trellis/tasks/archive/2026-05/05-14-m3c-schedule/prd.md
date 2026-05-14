# M3C Schedule Daemon Command

## Goal

Complete the deferred M3C daemon schedule slice by adding Rust CLI support for `tianji daemon schedule --every-seconds N --count M`, matching the existing Python oracle behavior for bounded repeated background run submissions.

## What I Already Know

* Prior Milestone 3 PRD deferred `tianji daemon schedule` as slice 3C after daemon/API/web UI stabilization.
* `.trellis/spec/backend/contracts/daemon-contract.md` defines the daemon as queue-oriented, not a general scheduler.
* The contract limits repeated scheduling to `--every-seconds N` plus `--count M`; cron-like calendars, distributed workers, and HTTP write endpoints are out of scope.
* Python oracle validates `--every-seconds >= 60` and `--count >= 1`.
* Python oracle queues the same run request `count` times via existing `queue_run`, sleeping `every_seconds` between submissions except after the last one.
* Current Rust CLI has `daemon start/stop/status/run/serve`, but no `daemon schedule` subcommand yet.
* Current Rust `daemon run` supports a single `--fixture` and optional `--sqlite-path`; schedule should align with that Rust surface rather than broaden source input support in this task.
* User confirmed Option 1: MVP fixture-only schedule. Full Python source/fetch input parity is explicitly out of scope for this slice.

## Requirements

* Add `tianji daemon schedule` as a daemon subcommand.
* Accept `--socket-path`, defaulting to `runs/tianji.sock`, matching other daemon commands.
* Accept `--fixture <path>` as the run input, matching current Rust `daemon run`.
* Accept optional `--sqlite-path <path>`, forwarding it into queued run payloads the same way `daemon run` does.
* Accept `--every-seconds N` and require `N >= 60`.
* Accept `--count M` and require `M >= 1`.
* Queue exactly `count` runs by sending existing `queue_run` socket requests.
* Sleep for `every_seconds` between queued submissions, but not after the final submission.
* Return pretty JSON shaped like the Python oracle:

```json
{
  "schedule": {
    "every_seconds": 300,
    "count": 3
  },
  "queued_runs": [
    {"job_id": "job-...", "state": "queued"}
  ],
  "job_states": ["queued", "running", "succeeded", "failed"]
}
```

* Reuse existing daemon socket error handling behavior where possible.

## Acceptance Criteria

* [ ] `tianji daemon schedule --fixture tests/fixtures/sample_feed.xml --every-seconds 60 --count 1` queues one run and returns schedule metadata plus one queued run.
* [ ] `--every-seconds 59` fails with a usage/input error.
* [ ] `--count 0` fails with a usage/input error.
* [ ] With `--count 2`, the command sends two `queue_run` requests and waits only between submissions.
* [ ] Returned `job_states` uses the same daemon job state vocabulary as `daemon status`.
* [ ] Rust tests cover schedule validation and JSON output behavior without forcing a real 60-second test delay.
* [ ] No special Ctrl+C handling is added; queued jobs already submitted remain daemon-owned, and interruption of the CLI-side schedule loop is acceptable.
* [ ] `cargo test`, `cargo fmt --check`, and `cargo clippy -- -D warnings` pass.

## Definition of Done

* Tests added/updated for the new CLI behavior.
* Lint, format, and tests pass.
* No Python oracle behavior is modified.
* Spec update considered after implementation.

## Out of Scope

* Cron/calendar scheduling.
* Persisted schedules across daemon or CLI restarts.
* Schedule cancellation or status APIs.
* HTTP write endpoints for scheduling.
* Multi-source/fetch/source-config parity beyond the current Rust `daemon run` surface.
* Web UI controls for schedule.

## Technical Approach

Add a `Schedule` variant to `DaemonCommands`, implement a handler adjacent to `handle_daemon_run`, and reuse the existing `queue_run` socket protocol. Keep the scheduling loop client-side in the CLI, matching the Python oracle, rather than adding daemon-side timer state.

For tests, isolate validation/output behavior with a minimal sleep abstraction or helper so `count > 1` can be verified without a 60-second wall-clock delay.

## Decision (ADR-lite)

**Context**: The previous M3 daemon work explicitly deferred schedule because timer lifecycle and repeated submission semantics widened the test matrix.

**Decision**: Implement schedule as a bounded client-side CLI loop that repeatedly sends existing `queue_run` requests to the daemon.

**Consequences**: This preserves the daemon's in-memory queue-only design and matches the Python oracle. Schedules do not survive CLI interruption, and cancellation/status for schedules remains out of scope.

**Scope boundary**: Implement only the current Rust daemon run input surface (`--fixture` plus optional `--sqlite-path`). Do not add Python-only fetch/source/output flags in this task.

## Technical Notes

* Inspected `src/main.rs`: existing daemon subcommands and `handle_daemon_run` live here.
* Inspected `src/daemon.rs`: `ALLOWED_JOB_STATES` and socket request protocol are already available.
* Inspected `tianji/cli_daemon.py`: Python oracle implementation of `_handle_daemon_schedule`.
* Inspected `tianji/cli_validation.py`: Python schedule validation rules.
* Inspected `.trellis/spec/backend/contracts/daemon-contract.md`: schedule boundaries and non-goals.
