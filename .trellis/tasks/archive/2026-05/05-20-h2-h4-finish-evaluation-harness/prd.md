# H2-H4 — Finish Evaluation Harness

## Goal

Finish Phase H by turning the H1 first-slice eval harness into a maintainable
local quality gate:

- H2: expand checked-in golden snapshot coverage;
- H3: make drift reporting richer and more useful across fixture families;
- H4: document the eval workflow and add a lightweight local/CI verification gate.

## Background

H1 already added:

- `src/eval.rs`
- `tianji eval --manifest tests/fixtures/eval/corpus.yaml`
- `tests/fixtures/eval/corpus.yaml`
- `tests/fixtures/eval/golden/sample_feed_technology_high.json`

Current H1 eval report passes one case with 13 checks. Phase H is incomplete until
fixture/golden workflow, drift detail, and documentation/CI gate are complete.

## Requirements

### H2 — Golden snapshot coverage and refresh workflow

1. Add at least one additional checked-in fixture case.
   - If no second XML fixture exists, create a tiny local RSS fixture under `tests/fixtures/`.
   - Keep it credential-free and deterministic.
   - Prefer a scenario that produces a different dominant field/risk shape than `sample_feed_technology_high`.

2. Add a checked-in golden snapshot for every manifest case.
   - Keep golden JSON semantic and stable.
   - Include top scored event and top intervention where present.

3. Add a safe snapshot refresh workflow.
   - Preferred CLI shape:
     ```bash
     tianji eval --manifest tests/fixtures/eval/corpus.yaml --update-golden
     ```
   - Normal `tianji eval` must remain read-only.
   - `--update-golden` may overwrite only golden files listed in the manifest.
   - The report should identify updated golden paths.

### H3 — Drift reporter

Improve the JSON report so failures are easy to triage:

- Include per-case `description`.
- Include `check_count` and `failed_check_count`.
- Include global `max_score_delta`.
- For numeric score checks, include absolute `delta` and `tolerance` in each check.
- Continue returning non-zero status when any check fails.
- Preserve `schema_version = "tianji.eval-report.v1"` unless a breaking report shape is introduced; if breaking, bump intentionally and update docs/tests.

### H4 — Documentation and local gate

1. Add docs for:
   - running eval;
   - adding a fixture case;
   - refreshing golden snapshots;
   - interpreting drift failures.

2. Add a lightweight local verification script or documented command.
   - Preferred path: `scripts/check-eval.sh`
   - Should run:
     ```bash
     cargo run --quiet -- eval --manifest tests/fixtures/eval/corpus.yaml
     ```
   - It must not require network, daemon, LLM, or credentials.

3. Update README and `plan.md` to mark Phase H complete only after verification passes.

## Non-goals

- No live feeds.
- No live LLM/provider calls.
- No daemon/API/web UI integration.
- Do not rewrite scoring logic.
- Do not change unrelated pipeline semantics.

## Verification commands

```bash
cargo fmt
cargo test --quiet eval
cargo test --quiet
cargo clippy -- -D warnings
git diff --check
cargo run --quiet -- eval --manifest tests/fixtures/eval/corpus.yaml
bash scripts/check-eval.sh
```

## Acceptance

- Eval corpus contains at least two cases.
- Every case has a checked-in golden snapshot.
- `tianji eval --manifest tests/fixtures/eval/corpus.yaml` exits 0.
- `--update-golden` works and is tested without hiding drift in normal mode.
- Report includes richer drift details.
- Docs explain run/add/refresh/failure workflow.
- Full tests/clippy/diff-check pass.
- `plan.md` marks all H2/H3/H4 complete and Phase H no longer shows NEXT.
