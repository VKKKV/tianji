# Phase G1 — Roadmap and Spec Authority Refresh

## Purpose

Refresh TianJi's documentation authority after Phase F release readiness. This is a documentation-only phase that prevents future agents from following stale Python-era or pre-Phase-F guidance.

## Rules

1. Root authority
   - `plan.md` remains the authoritative roadmap.
   - It must record Phase F as complete and define the next development direction before any new feature work.

2. Documentation consistency
   - README current-state numbers must not contradict `plan.md`.
   - Trellis specs may preserve historical context, but stale implementation paths must be labelled as historical or replaced with Rust paths.
   - Avoid deleting historical specs unless they are actively misleading and redundant.

3. Local-first safety
   - Do not introduce credential examples with real secrets.
   - Do not add live provider, webhook, or network checks as requirements for future phases unless explicitly scoped.

4. Scope control
   - Documentation-only.
   - No code, API, schema, or dependency changes.

## Required audit strings

Run targeted searches before finishing:

```bash
rg -n "337 unit|32 integration|2026-05-19|Phase F .*NEXT|Phase D .*In progress|tianji/scoring.py|tianji/normalize.py|python3 -m tianji|Rich-based" README.md plan.md .trellis/spec
```

Remaining matches are acceptable only when they are explicitly marked as archived/historical/superseded.

## Verification

Even though this phase is documentation-only, keep the standard gate:

```bash
cargo test --quiet
cargo clippy -- -D warnings
git diff --check
```
