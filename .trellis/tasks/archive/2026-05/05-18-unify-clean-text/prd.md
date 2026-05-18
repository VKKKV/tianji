# A3 Unify clean_text

## Goal

Replace duplicate text-cleaning helpers with a single shared `utils::clean_text`
implementation so canonical hashing and normalization use the same whitespace
semantics.

## What I Already Know

- `plan.md` Phase A3 requires unifying `src/fetch.rs:clean_text` and `src/normalize.rs:clean_text` into `src/utils.rs`.
- `src/fetch.rs` currently has a private `clean_text` using `split_whitespace().join(" ")`.
- `src/normalize.rs` currently has a public `clean_text` using a precompiled whitespace regex and `trim()`.
- `src/utils.rs` already contains shared helpers and tests but no text cleaner.
- The plan explicitly wants a single `utils::clean_text` with `trim()` behavior.

## Requirements

- Add `pub fn clean_text(text: &str) -> String` to `src/utils.rs`.
- Use trim semantics: collapse runs of whitespace to one space and remove leading/trailing whitespace.
- Update `src/fetch.rs` to use `crate::utils::clean_text` for canonical hash inputs.
- Update `src/normalize.rs` to use `crate::utils::clean_text` for title, summary, and combined text.
- Remove duplicate module-local `clean_text` implementations and any now-unused whitespace regex state.
- Preserve current fixture/hash/scoring behavior or update tests only if the unified trim behavior intentionally changes previously inconsistent edge cases.

## Acceptance Criteria

- [ ] Repository search finds only one `fn clean_text` definition, in `src/utils.rs`.
- [ ] `fetch` and `normalize` both import/use `crate::utils::clean_text`.
- [ ] Utility tests cover whitespace collapsing and leading/trailing trim.
- [ ] `cargo test` passes.
- [ ] `cargo clippy -- -D warnings` passes.
- [ ] `cargo fmt --check` status is recorded.

## Definition of Done

- Duplicate helpers are removed.
- Shared utility behavior is tested.
- No new dependencies.
- Spec update considered after implementation.

## Technical Approach

- Implement `utils::clean_text` with standard-library `split_whitespace().collect::<Vec<_>>().join(" ")`, which already collapses all whitespace and trims boundaries.
- Import this helper in `fetch` and `normalize`.
- Remove `WHITESPACE_RE` from `normalize` if no longer needed.
- Keep `TOKEN_RE` and other normalization regexes unchanged.

## Decision (ADR-lite)

**Context**: Two helpers perform similar cleanup with different implementations, creating a risk that canonical hashes and normalized event text diverge on whitespace edge cases.

**Decision**: Use one shared `utils::clean_text` with trim behavior and route both modules through it.

**Consequences**: Whitespace handling becomes consistent across hashing and normalization. Any future text-cleaning change has one implementation point.

## Out of Scope

- Changing keyword extraction or actor/region matching semantics.
- Changing XML parsing trim behavior before RawItem construction.
- Introducing more advanced HTML/entity sanitization.

## Technical Notes

- Relevant files: `src/fetch.rs`, `src/normalize.rs`, `src/utils.rs`.
- Relevant specs: `plan.md`, `.trellis/spec/backend/index.md`, `.trellis/spec/backend/quality-guidelines.md`.
