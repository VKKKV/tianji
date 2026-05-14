# Fix Known Bugs B4/B10

## Goal

Fix the last two known bugs from plan.md §12 to decouple Rust from the Python source tree and harden backtrack string matching.

## Bugs

### B4 — include_str! hardcoded to Python source tree [HIGH]

**File**: `src/webui.rs:30–32`
**Problem**: Three `include_str!` calls reference `../tianji/webui/{index.html,app.js,styles.css}`, pointing into the Python oracle package (`tianji/`). The Rust build breaks if the Python directory is absent or reorganized.
**Fix**: Move webui static files from `tianji/webui/` to `src/webui/`. Update `include_str!` paths to `include_str!("webui/...")`. This makes the assets owned by the Rust crate, not the Python oracle.

### B10 — Backtrack string exact-match fragile [HIGH]

**File**: `src/backtrack.rs` (multiple locations)
**Problem**: Two categories of fragile exact-match:

1. **`dominant_field` vs constant keys** (lines 165, 174, 185, 194, 228): `*field == event_group.dominant_field` and `match event.dominant_field.as_str()`. If `dominant_field` arrives with any case/whitespace variation, lookup silently falls through to wrong defaults.

2. **Stringly-typed headline roles** (lines 117–118, 249, 258–262): Comparisons like `== " headline role=chain endpoint;"` against self-generated text. Brittle if vocabulary changes.

**Fix approach**:

For Category 1 (dominant_field): Add a normalization function `fn normalize_field(f: &str) -> String` that trims and lowercases. Apply it before every lookup and match arm. This is minimal and safe — no enum refactor needed for fields since the set is open-ended (future fields like `"cyber"` or `"energy"` can be added without changing an enum).

For Category 2 (headline roles): Replace stringly-typed roles with a `HeadlineRole` enum (`Standalone`, `ChainOrigin`, `ChainEndpoint`, `ChainPivot`). The display text (`" headline role=chain endpoint;"`) becomes a `Display` impl. This eliminates all the stringly-typed comparisons in one move and makes the type system enforce correctness.

## Scope

- Fix B4: move `tianji/webui/` → `src/webui/`, update `include_str!` paths
- Fix B10 Cat.1: add `normalize_field()`, apply before dominant_field lookups
- Fix B10 Cat.2: introduce `HeadlineRole` enum, replace string comparisons
- Update plan.md §12 bug table
- Run `cargo test`, `cargo fmt --check`, `cargo clippy -- -D warnings`

## Acceptance Criteria

1. No `include_str!` path references `tianji/` (Python oracle tree)
2. `dominant_field` lookups are case-insensitive and trim-tolerant
3. No string literal comparisons for headline roles — only enum variants
4. All existing tests pass (artifact output must be unchanged — display text preserved)
5. `cargo fmt --check` + `cargo clippy -- -D warnings` clean
6. plan.md §12 bug table updated

## Out of Scope

- `DominantField` enum (field set is open-ended; normalization is sufficient)
- Crucix Delta Engine Phase 2
- M3C schedule, TUI, Hongmeng, Nuwa

## Technical Notes

- B4 requires `git mv tianji/webui/ src/webui/` to preserve git history
- B10 display text must be preserved for artifact parity with Python oracle
- The `HeadlineRole` enum `Display` impl must produce identical output to current stringly-typed text
