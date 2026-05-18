# B1 Extract time_utils Module

## Goal

Consolidate duplicated timestamp parsing and date-to-epoch conversion into a
single `src/time_utils.rs` module so grouping, storage filters, and delta memory
use one deterministic time implementation.

## What I Already Know

- `plan.md` Phase B1 requires a new `src/time_utils.rs`, consolidation of ISO parsing, and standardization on Howard Hinnant's `days_from_civil` algorithm.
- `src/grouping.rs` has ISO and RFC2822 event timestamp parsing for grouping windows.
- `src/storage.rs` has ISO timestamp parsing for history filters and currently imports `utils::days_since_epoch`.
- `src/delta_memory.rs` has RFC3339/Unix-seconds parsing plus local `datetime_to_unix_seconds` and `days_from_civil` helpers.
- `src/utils.rs` currently exposes `days_since_epoch` using a separate calendar algorithm.

## Requirements

- Add `pub mod time_utils;` in `src/lib.rs`.
- Create `src/time_utils.rs` containing shared deterministic helpers for:
  - `days_from_civil(year, month, day)` using Howard Hinnant's algorithm.
  - `datetime_to_unix_seconds(year, month, day, hour, minute, second)`.
  - ISO/RFC3339 timestamp parsing for strings such as `2026-03-22T07:00:00Z`, `2026-03-22T07:00:00+00:00`, and integer Unix seconds.
  - RFC2822-ish parsing currently used by RSS fixtures, such as `Sun, 22 Mar 2026 07:00:00 GMT`.
- Update `grouping.rs`, `storage.rs`, and `delta_memory.rs` to call shared helpers instead of local/duplicated implementations.
- Remove now-unused local regexes/helpers from those modules.
- Remove or deprecate `utils::days_since_epoch` only if all call sites migrate; keep `utils` focused on non-time utilities.
- Preserve current timestamp behavior used by fixtures, history filters, and delta memory tests.

## Acceptance Criteria

- [ ] `src/time_utils.rs` exists and is exported from `lib.rs`.
- [ ] No duplicate `days_since_epoch`, `days_from_civil`, or `datetime_to_unix_seconds` implementations remain outside `time_utils`.
- [ ] `grouping`, `storage`, and `delta_memory` use `crate::time_utils` for timestamp parsing/conversion.
- [ ] Existing grouping/storage/delta memory timestamp tests pass, with new tests added for shared helpers as needed.
- [ ] `cargo test` passes.
- [ ] `cargo clippy -- -D warnings` passes.
- [ ] `cargo fmt --check` status is recorded.

## Definition of Done

- Time parsing/conversion duplication is removed.
- Shared time helpers are covered by focused unit tests.
- No new dependencies.
- Spec update considered after implementation.

## Technical Approach

- Move the Howard Hinnant algorithm from `delta_memory.rs` into `time_utils` and make it the canonical date conversion path.
- Use `std::sync::LazyLock<regex::Regex>` in `time_utils` for reusable timestamp regexes.
- Keep parser behavior intentionally narrow: support the formats already accepted by current modules and tests; do not introduce timezone offset math beyond existing `Z`/`+00:00` normalization unless required to preserve current behavior.
- Keep functions returning `Option<i64>` for parsing failures to match existing call sites.

## Decision (ADR-lite)

**Context**: Multiple modules parse timestamps and convert dates independently, increasing the chance of subtle disagreement in grouping windows, history filters, and alert decay.

**Decision**: Centralize timestamp parsing and epoch conversion in `time_utils`, using Howard Hinnant's civil-date algorithm as the canonical date conversion implementation.

**Consequences**: Time behavior has one maintenance point and tests can assert shared behavior directly. This is a refactor; public CLI/API timestamp semantics should remain unchanged.

## Out of Scope

- Adding new timestamp formats beyond those already accepted.
- Switching to chrono for these hot-path deterministic helpers.
- Changing history filter inclusivity or grouping time-window thresholds.

## Technical Notes

- Relevant files: `src/time_utils.rs` (new), `src/lib.rs`, `src/grouping.rs`, `src/storage.rs`, `src/delta_memory.rs`, `src/utils.rs`.
- Relevant specs: `plan.md`, `.trellis/spec/backend/quality-guidelines.md`.
