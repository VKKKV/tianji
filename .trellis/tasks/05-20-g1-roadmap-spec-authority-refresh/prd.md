# G1 — Roadmap and Spec Authority Refresh

## Purpose

Phase F completed a release-ready local-first checkpoint. The root roadmap and several Trellis specs still contain stale dates, counts, Python-era paths, or older Phase D status. Refresh the documentation authority before starting the next feature phase.

## Scope

In scope:

- Update root `plan.md` to reflect Phase F completion and define the next phase direction.
- Update README current-state counts/date if stale.
- Audit Trellis backend/docs specs for stale roadmap/status references.
- Mark superseded Python/Rich-era documents clearly and point readers to Rust files/current contracts.
- Define a concrete next roadmap after Phase G, with Phase H focused on evaluation harness design.

Out of scope:

- No Rust implementation changes.
- No API/schema changes.
- No release tagging or publishing.
- No live LLM/provider/webhook calls.

## Acceptance criteria

- `plan.md` no longer says Phase F is NEXT and records current Phase G/Phase H direction.
- README current-state date/test counts match the latest verified baseline.
- `.trellis/spec/backend/development-plan.md` no longer claims Phase D is in progress.
- Python-era docs that remain for history are explicitly marked as historical/superseded.
- Search for stale high-signal strings is either clean or only shows intentional historical/archive content.
- Verification passes:
  - `cargo test --quiet`
  - `cargo clippy -- -D warnings`
  - `git diff --check`
