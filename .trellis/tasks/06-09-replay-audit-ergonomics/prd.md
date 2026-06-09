# Replay audit ergonomics

## Goal

Finish the Post-K candidate "Improve operator-facing replay/audit ergonomics without changing API schemas" with one focused local-first slice.

## Scope

Improve the operator experience of trace-backed replay/audit output without changing external schemas or adding live/provider/daemon behavior.

Preferred small improvements:

- Make `tianji tui --replay-bundle-dir <DIR> --render-once` output more self-explanatory for operators.
- Include enough replay/audit context in render-once text to understand selected frame, available frame count, field changes, event sequence length, and agent audit fields.
- If useful, expose clear error/help text for replay flag conflicts or malformed bundles, but do not change flag names or schemas.
- Update README/plan to document the ergonomic improvement.
- Keep behavior deterministic and local-only.

## Non-goals

- No schema changes to `tianji.sim-trace.v1` or `tianji.replay-bundle.v1`.
- No new API endpoints.
- No provider-backed simulation.
- No daemon/web UI/live fetch changes.

## Verification

Run:

```bash
cargo fmt --check
bash scripts/check-replay-smoke.sh
cargo test --quiet tui
cargo test --quiet trace
cargo test --quiet
cargo clippy -- -D warnings
git diff --check
```
