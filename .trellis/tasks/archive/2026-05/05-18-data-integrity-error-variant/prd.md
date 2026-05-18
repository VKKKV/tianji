# A4 TianJiError DataIntegrity Variant

## Goal

Replace storage-layer misuse of `rusqlite::Error::InvalidParameterName` for
internal data integrity failures with a dedicated `TianJiError::DataIntegrity`
variant.

## What I Already Know

- `plan.md` Phase A4 requires adding `TianJiError::DataIntegrity` in `src/lib.rs` and updating `src/storage.rs`.
- `src/storage.rs` currently creates `TianJiError::Storage(rusqlite::Error::InvalidParameterName(...))` when canonical source item IDs are unexpectedly missing.
- These conditions are not SQLite parameter binding failures; they are pipeline/storage integrity mismatches.
- `src/api.rs` maps explicit user/input errors to `400`, missing storage rows to `404`, and falls through to `500` for internal errors.

## Requirements

- Add `DataIntegrity(String)` to `TianJiError` in `src/lib.rs`.
- Update `Display` for `TianJiError` so `DataIntegrity` displays the message directly.
- Replace both `InvalidParameterName("missing canonical...")` storage hacks with `TianJiError::DataIntegrity(...)`.
- Preserve existing storage behavior other than the error variant classification.
- Update tests or add coverage proving missing canonical IDs now return `DataIntegrity` rather than `Storage`.
- Review any `match TianJiError` sites for exhaustiveness and intended behavior.

## Acceptance Criteria

- [ ] No `InvalidParameterName("missing canonical...`) usage remains.
- [ ] `TianJiError::DataIntegrity` exists and displays the underlying message.
- [ ] Missing canonical source item IDs in storage paths return `DataIntegrity`.
- [ ] API/internal boundaries still treat data-integrity failures as internal errors, not user input errors.
- [ ] `cargo test` passes.
- [ ] `cargo clippy -- -D warnings` passes.
- [ ] `cargo fmt --check` status is recorded.

## Definition of Done

- Error variant added and wired into storage guards.
- Tests cover the new variant for at least one canonical-ID mismatch path.
- No new dependencies.
- Spec update considered after implementation.

## Technical Approach

- Add `DataIntegrity(String)` next to the other domain error variants in `TianJiError`.
- Keep API mapping unchanged unless compile exhaustiveness requires touching it; the wildcard branch should continue mapping this internal error to `500`.
- Prefer small storage tests that call existing private helpers from the in-module test module rather than widening helper visibility.

## Decision (ADR-lite)

**Context**: Storage guard code currently encodes non-SQLite data-integrity failures as a rusqlite parameter error, hiding the real cause and leaking implementation details.

**Decision**: Add a dedicated `DataIntegrity(String)` domain error and use it for missing canonical-source-item mapping failures.

**Consequences**: Error classification becomes accurate while user-facing API behavior remains an internal error. Future storage integrity checks can reuse the variant.

## Out of Scope

- Reworking all storage errors into custom domain types.
- Changing public API error envelopes.
- Changing schema, persistence logic, or canonical hash derivation.

## Technical Notes

- Relevant files: `src/lib.rs`, `src/storage.rs`; possibly `src/api.rs` if explicit mapping is desired.
- Relevant specs: `plan.md`, `.trellis/spec/backend/error-handling.md`, `.trellis/spec/backend/quality-guidelines.md`.
