# Update Design Development Plan

## Goal

Bring the authoritative TianJi design/development plan up to date with the current repository state after Phase C and D1 work.

## Current Facts Verified

- Branch: `main`.
- Latest relevant commits include:
  - `233a45e` / `45db1e8`: C2 TUI view-state decoupling.
  - `92eb26c`: C3 Nuwa forward tick-simulation extraction.
  - `836df75` / `b1c307d`: C4 worldline fork unification and failure contract.
  - `20f705c` / `48c4ca3`: C1 Hongmeng JSON state replaced with strong types and compatibility contract.
  - `604fe89`: D1 storage history compare integration coverage.
- Active Trellis tasks: none before this task.
- `cargo test --quiet`: 293 lib tests + 28 integration tests = 321 passed / 0 failed.
- `cargo test --quiet -- --list`: 321 test cases.
- Source snapshot: 55 Rust files under `src/`, 21,722 source lines, 23 manifest dependencies.
- `plan.md` still says Updated 2026-05-17, Phase B ongoing, Phase C/D1 pending, tests 310/296 in different places.
- `.trellis/spec/backend/development-plan.md` still contains stale migration milestone counts and says Hongmeng/Nuwa deferred in its table.

## Requirements

1. Update root `plan.md` only as documentation/design-plan work:
   - reflect current date and current status;
   - mark Phase C1-C4 complete;
   - mark Phase D1 complete;
   - update test counts to 321 pass / 0 fail;
   - update source/dependency snapshot using verified values;
   - keep future Phase D roadmap items clear and ordered.
2. Update `.trellis/spec/backend/development-plan.md` so Trellis backend spec aligns with root `plan.md`:
   - remove stale "Hongmeng/Nuwa deferred" table entries;
   - add current post-v0.2.0 progress section for Phase A-D;
   - update verification counts;
   - preserve existing long-lived contracts, especially storage paging and delta contracts.
3. Do not change Rust code.
4. Verify docs diff and run at least `cargo test --quiet` before commit.
5. Commit all documentation/task changes locally.

## Acceptance Criteria

- `plan.md` no longer claims Phase B is ongoing or Phase C/D1 are pending.
- `plan.md` verification criteria no longer says tests are currently 296.
- Trellis development spec no longer labels Hongmeng/Nuwa as deferred.
- Git working tree is clean after commit.
