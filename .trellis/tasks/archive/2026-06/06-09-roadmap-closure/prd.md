# Roadmap closure

## Goal

Finish the user's "continue until finish plan" request by closing the explicit Post-K roadmap after completing all listed development candidates.

## Scope

Documentation-only.

- Update `plan.md` so the Post-K section no longer advertises unfinished candidate directions.
- Preserve the safety constraint that provider-backed simulation, live feed fetch, daemon write paths, and alert dispatch remain optional unless a future PRD scopes them.
- Update `handoff.md` to reference the latest completed work, current counters, and replay smoke gate.
- Do not change code behavior.

## Verification

Run:

```bash
rg -n "Candidate directions|Refresh release|Expand the credential-free|Improve operator-facing|dirty worktree|not committed|30,399|30,439|443 tests" plan.md README.md handoff.md .trellis/spec || true
cargo fmt --check
bash scripts/check-replay-smoke.sh
cargo test --quiet
cargo clippy -- -D warnings
git diff --check
```
