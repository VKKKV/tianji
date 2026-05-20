# H1 — Evaluation Harness First Slice

## Goal

Implement the first local-first evaluation harness for TianJi. The harness must
run checked-in RSS/Atom fixtures through the deterministic Rust pipeline, compare
semantic outputs against a checked-in manifest and golden snapshot, and report
pass/fail drift as JSON.

## Background

Phase G made root docs and Trellis specs authoritative. Phase H is now the next
feature track. The project needs an eval gate before future scoring, simulation,
or source-management changes, so quality drift can be detected intentionally
instead of by incidental contract test failures.

## Requirements

1. Add `tests/fixtures/eval/corpus.yaml`.
   - Start with at least `tests/fixtures/sample_feed.xml`.
   - Prefer including `tests/fixtures/grouped.xml` if current expectations are easy to derive deterministically.
   - Keep all cases local-first and credential-free.

2. Add checked-in golden snapshot(s) under `tests/fixtures/eval/golden/`.
   - Snapshot content may be full artifacts, but comparison must focus on semantic fields.
   - Do not include wall-clock time or environment-dependent data.

3. Add Rust eval harness support.
   - A new module such as `src/eval.rs` is acceptable.
   - Read manifest via `serde_yaml`.
   - Run each case through `run_fixture_path(fixture, None)` or equivalent deterministic library path.
   - Build a stable report struct serializable as JSON.

4. Add CLI command:

   ```bash
   tianji eval --manifest tests/fixtures/eval/corpus.yaml
   ```

   Behavior:
   - stdout: JSON report with `schema_version = "tianji.eval-report.v1"`.
   - exit 0 if all cases pass.
   - exit non-zero if any case fails.
   - no network, no LLM, no daemon required.

5. Drift checks:
   - schema_version
   - mode
   - raw_item_count
   - normalized_event_count
   - scored_event_count
   - intervention_count
   - scenario_summary.dominant_field
   - scenario_summary.risk_level
   - top scored event id or stable identity if specified
   - at least one numeric score delta using absolute tolerance

6. Tests:
   - manifest load/parse test
   - passing checked-in corpus test
   - intentional failure test proving non-zero/failure report behavior at library level
   - CLI parse test for `eval --manifest ...`

## Non-goals

- No live feeds.
- No live LLM/provider calls.
- No daemon/API/web UI integration.
- No snapshot auto-update unless it is tiny and does not risk masking drift.
- Do not rewrite scoring logic.

## Files likely involved

- `src/main.rs`
- `src/lib.rs`
- new `src/eval.rs`
- `tests/fixtures/eval/corpus.yaml`
- `tests/fixtures/eval/golden/*.json`
- `README.md` only if a short command mention is useful
- `plan.md` to mark H1 complete after verification, not during implementation

## Verification commands

```bash
cargo fmt
cargo test --quiet eval
cargo test --quiet
cargo clippy -- -D warnings
git diff --check
cargo run --quiet -- eval --manifest tests/fixtures/eval/corpus.yaml
```

## Acceptance

The task is complete when:

- `tianji eval --manifest tests/fixtures/eval/corpus.yaml` exits 0.
- Its JSON report includes all cases and no failures.
- An intentional mismatch is tested and reported as failure.
- Full tests/clippy/diff-check pass.
- H1 is marked complete in `plan.md` only after implementation verification.
