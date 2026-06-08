# K2 replay bundle packaging

## Purpose

Continue Phase K after JSONL trace export by packaging replay artifacts into a portable local directory bundle.

## Scope

In scope:
- Add replay bundle manifest type with schema `tianji.replay-bundle.v1`.
- Add helper to write a bundle directory containing:
  - `manifest.json`
  - `trace.jsonl`
  - `outcome.json`
- Extend `tianji predict` with `--replay-bundle-dir <DIR>`.
- When set, write trace/outcome/manifest bundle and preserve default stdout outcome JSON.
- The bundle must contain no raw secrets/config/API keys.
- Reuse K1 trace generation helpers; do not duplicate trace serialization.
- Add tests for bundle writing, manifest references, trace import, and CLI parse/predict bundle creation.
- Update README and plan for K2.

Out of scope:
- TUI bundle loading.
- Checksums beyond simple file size metadata unless cheap.
- Compression/archive format.

## Bundle contract

`manifest.json` minimum fields:
- `schema_version`: `tianji.replay-bundle.v1`
- `created_at`
- `simulation_id` or deterministic/local generated id
- `mode`
- `target_field`
- `horizon_ticks`
- `frame_count`
- `trace_file`: `trace.jsonl`
- `outcome_file`: `outcome.json`
- `trace_bytes`
- `outcome_bytes`

## Acceptance criteria

1. `predict --replay-bundle-dir <DIR>` creates the directory and three files.
2. `trace.jsonl` can be read via K1 trace reader.
3. `outcome.json` parses as `SimulationOutcome`.
4. `manifest.json` references existing files and correct frame count/byte sizes.
5. Existing `predict` default stdout remains unchanged.
6. README documents bundle export.
7. plan.md records K2 complete and updates real metrics.

## Verification commands

```bash
cargo test bundle
cargo test trace
cargo test predict
cargo test
cargo fmt --check
cargo clippy -- -D warnings
git diff --check
```
