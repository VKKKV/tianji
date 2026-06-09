# TianJi handoff

Date: 2026-06-09
Repo: `/home/kita/code/tianji`
Branch: `main`
Agent workflow: Hermes plans/verifies/commits; OpenCode implements non-trivial Rust code changes with model `jun/gpt-5.5`. Do not let OpenCode commit unless explicitly requested.

## Current status

Active task: none. Last completed task: `.trellis/tasks/archive/2026-06/06-09-roadmap-closure/`.

The explicit Post-K roadmap in `plan.md` is complete. Future runtime behavior changes should start from a new PRD and remain local-first by default.

## Completed Post-K closure

- Refreshed release/readiness handoff after K3/K4 was already committed.
- Added `scripts/check-replay-smoke.sh` as a credential-free `/tmp`-only replay bundle + TUI render-once gate.
- Improved replay/audit ergonomics with replay controls, selected-frame replay summary, and audit coverage counts in TUI replay output.
- Closed `plan.md` so it no longer advertises unfinished explicit Post-K candidate directions.

## Current shipped Phase K behavior

- `tianji predict --trace-jsonl <PATH>` writes `tianji.sim-trace.v1` JSONL traces.
- `tianji predict --replay-bundle-dir <DIR>` writes a local replay bundle containing `manifest.json`, `trace.jsonl`, and `outcome.json`.
- `tianji tui --trace-jsonl <PATH> [--render-once]` loads trace-backed simulation replay without provider execution.
- `tianji tui --replay-bundle-dir <DIR> [--render-once]` reads only the three replay bundle files above.
- Replay bundle validation checks schema version, fixed file names, trace/outcome sizes, frame counts, and manifest mode/target/horizon against trace metadata.
- Simulation replay scrubbing with `Left`/`h` and `Right`/`l` updates the selected frame display, including field metadata, replay controls, field changes, event sequence length, structured agent audit fields, and audit coverage counts.
- Trace strings are sanitized before rendering.
- Replay flags conflict with each other and with `--simulate`.
- Plain `tianji tui` defaults to `runs/tianji.sqlite3`.

## Verified counters

Measured on 2026-06-09:

```text
Rust lines/files: 30,465 / 59
cargo test -- --list: 445 tests
```

## Recommended verification before commit

```bash
cargo fmt --check
bash scripts/check-eval.sh
bash scripts/check-replay-smoke.sh
cargo test --quiet
cargo clippy -- -D warnings
git diff --check
```

## Optional local replay smoke

```bash
bash scripts/check-replay-smoke.sh
```

Expected output is a compact JSON summary with bundle files, schema version, frame count, trace record count, and TUI render byte count. The script writes transient files only under `/tmp` and does not use provider config, network, daemon/API, live feeds, or secrets.
