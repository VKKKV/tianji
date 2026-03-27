# TUI Persisted Navigation Parity

## TL;DR
> **Summary**: Align the shipped read-only TUI with storage-backed persisted navigation semantics so `[` / `]` and compare-target stepping work beyond the currently loaded list window. Keep storage-backed previous/next behavior as the source of truth and preserve the TUI's read-only contract.
> **Deliverables**:
> - TUI state transitions that resolve previous/next runs through storage adjacency first
> - compare-target stepping that skips the staged-left run while preserving persisted ordering and boundary messages
> - regression coverage in unit and session-level TUI tests
> - light contract/doc wording updates for the shipped TUI behavior
> **Effort**: Medium
> **Parallel**: YES - 2 waves
> **Critical Path**: Task 1 → Task 2 → Task 3 → Task 4 → Task 5

## Context
### Original Request
Pick a feature from `DEV_PLAN.md` and prepare the work plan for it.

### Interview Summary
- Selected feature: TUI persisted-navigation parity from Phase 5.
- Chosen test strategy: TDD.
- Scope is intentionally narrow: keep the TUI read-only, preserve CLI/storage as the source of truth, and avoid list filtering, threshold entry, daemon/API work, or broader redesign.

### Metis Review (gaps addressed)
- Lock parity to exact storage-backed semantics, not just approximate user-visible behavior.
- Define what happens when the persisted previous/next target is outside the loaded TUI window: jump selection to that run by merging it into the current window.
- Define compare-target stepping over persisted adjacency, always skipping the staged-left run and preserving true first/last boundary messages.
- Keep fallback-to-loaded-rows behavior only for SQLite read failures; do not invent a second navigation model.

## Work Objectives
### Core Objective
Make the shipped Rich TUI use persisted previous/next run semantics for detail stepping and compare-target stepping, even when the target run is outside the currently loaded rows.

### Deliverables
- `tianji/tui_state.py` implements storage-backed stepping invariants consistently.
- `tests/test_tui_state.py` covers state-level persisted stepping and boundary behavior.
- `tests/test_tui.py` covers interactive session behavior for persisted stepping and compare-target changes.
- `TUI_CONTRACT.md` reflects the shipped parity behavior without broadening feature scope.

### Definition of Done (verifiable conditions with commands)
- `.venv/bin/python -m unittest tests.test_tui_state -v` exits 0.
- `.venv/bin/python -m unittest tests.test_tui -v` exits 0.
- `.venv/bin/python -m unittest discover -s tests -v` exits 0.
- In a live TUI session over persisted runs, `[` / `]` move to true persisted previous/next runs even when those runs were not originally in the loaded window.
- In compare view, `[` / `]` step the right-hand compare target through true persisted adjacency, never selecting the staged-left run as the right-hand target.

### Must Have
- Storage-backed previous/next semantics remain the authoritative navigation source.
- Compare stepping skips the staged-left run and preserves boundary messages (`first compare target`, `last compare target`).
- Cache invalidation remains correct when detail or compare targets change.
- Loaded-row fallback remains only for `sqlite3.OperationalError` or equivalent storage-read failure.
- TUI behavior stays read-only and continues reusing `storage.py` semantics.

### Must NOT Have (guardrails, AI slop patterns, scope boundaries)
- Must NOT add list filtering to the TUI list pane.
- Must NOT add numeric threshold entry, freeform prompts, write actions, or daemon/API dependencies.
- Must NOT move history semantics out of `storage.py` into a TUI-only navigation model.
- Must NOT change CLI `history-show` / `history-compare` output contracts while implementing parity.
- Must NOT broaden this slice into general TUI refactoring outside the stepping/cache invariants needed for persisted parity.

## Verification Strategy
> ZERO HUMAN INTERVENTION — all verification is agent-executed.
- Test decision: TDD + `unittest`
- QA policy: Every task has agent-executed scenarios
- Evidence: `.sisyphus/evidence/task-{N}-{slug}.{ext}`

## Execution Strategy
### Parallel Execution Waves
> Target: 5-8 tasks per wave. <3 per wave (except final) = under-splitting.
> Extract shared dependencies as Wave-1 tasks for max parallelism.

Wave 1: TUI state invariants + state-level failing tests + contract wording
Wave 2: session-level TUI regression tests + implementation finalization + docs alignment

### Dependency Matrix (full, all tasks)
- Task 1 blocks Tasks 2-5.
- Task 2 blocks Task 4.
- Task 3 can run after Task 1 and before/alongside Task 4.
- Task 4 blocks Task 5.
- Task 5 blocks final verification.

### Agent Dispatch Summary (wave → task count → categories)
- Wave 1 → 3 tasks → `quick`, `writing`
- Wave 2 → 2 tasks → `quick`, `unspecified-high`

## TODOs
> Implementation + Test = ONE task. Never separate.
> EVERY task MUST have: Agent Profile + Parallelization + QA Scenarios.

- [x] 1. Map persisted-navigation invariants in TUI state

  **What to do**: Audit `HistoryListState.step_run`, `step_compare_target`, `select_run_id`, `_resolve_adjacent_run_id`, `_step_to_persisted_run`, and `_step_compare_target_in_loaded_rows` in `tianji/tui_state.py`. Document the exact invariants that implementation must preserve: storage-backed adjacency first, loaded-row fallback only on storage failure, staged-left compare target skip, boundary messages, selected-index visibility, and cache invalidation semantics.
  **Must NOT do**: Do not change behavior yet. Do not refactor unrelated key handling, render code, or list movement semantics.

  **Recommended Agent Profile**:
  - Category: `quick` — Reason: single-file analysis and bounded invariant definition
  - Skills: `[]` — no extra skill needed beyond local repo patterns
  - Omitted: `writing-plans` — executor is implementing, not replanning

  **Parallelization**: Can Parallel: NO | Wave 1 | Blocks: 2, 3, 4, 5 | Blocked By: none

  **References** (executor has NO interview context — be exhaustive):
  - Pattern: `tianji/tui_state.py:265-394` — current persisted-stepping seam and fallback behavior
  - Pattern: `tianji/tui_state.py:501-513` — key dispatch for `[` / `]`
  - API/Type: `tianji/storage.py:26-33,66-70` — exported storage adjacency and summary helpers
  - Test: `tests/test_tui_state.py:98-239` — current state-level persisted stepping expectations
  - Test: `tests/test_tui.py:517-687` — current session-level persisted previous/next coverage

  **Acceptance Criteria** (agent-executable only):
  - [ ] Invariant list is reflected in code comments, tests, or implementation decisions with no ambiguity about parity behavior.
  - [ ] Executor can point to exact functions handling storage-first adjacency, fallback, compare skip, and boundary messages.

  **QA Scenarios** (MANDATORY — task incomplete without these):
  ```
  Scenario: Inspect current stepping seam
    Tool: Bash
    Steps: python - <<'PY'
from pathlib import Path
text = Path('tianji/tui_state.py').read_text()
for token in ['def step_run', 'def step_compare_target', 'def _resolve_adjacent_run_id', 'def _step_to_persisted_run']:
    print(token, token in text)
PY
    Expected: All listed functions are present and identifiable for implementation work.
    Evidence: .sisyphus/evidence/task-1-tui-state-seam.txt

  Scenario: Inspect existing regression anchors
    Tool: Bash
    Steps: python - <<'PY'
from pathlib import Path
text = Path('tests/test_tui_state.py').read_text()
checks = [
    'test_step_run_uses_persisted_previous_beyond_loaded_window',
    'test_step_run_uses_persisted_next_beyond_loaded_window',
    'test_step_compare_target_skips_staged_left_when_stepping_previous',
]
for name in checks:
    print(name, name in text)
PY
    Expected: Existing regression anchors are present and ready to extend.
    Evidence: .sisyphus/evidence/task-1-tui-test-anchors.txt
  ```

  **Commit**: NO | Message: `n/a` | Files: `[]`

- [x] 2. Extend `tests/test_tui_state.py` with failing parity cases first

  **What to do**: Add or refine state-level TDD coverage for the exact parity cases that remain ambiguous or partially covered: persisted previous/next when target is outside loaded window, loaded-row fallback only on `sqlite3.OperationalError`, compare-target stepping skipping the staged-left run in both directions, and correct first/last compare boundary messages after persisted adjacency exhaustion.
  **Must NOT do**: Do not implement production changes in this task. Do not weaken current tests or replace precise assertions with broad smoke coverage.

  **Recommended Agent Profile**:
  - Category: `quick` — Reason: single-test-file TDD expansion
  - Skills: `[]` — existing unittest patterns are enough
  - Omitted: `test-driven-development` — not available in this environment; still follow strict test-first behavior

  **Parallelization**: Can Parallel: NO | Wave 1 | Blocks: 4 | Blocked By: 1

  **References**:
  - Pattern: `tests/test_tui_state.py:98-239` — existing persisted previous/next and compare-target tests
  - API/Type: `tianji/tui_state.py:265-394` — functions under test
  - Pattern: `tests/support.py` — shared unittest imports and helpers
  - External: `DEV_PLAN.md:197-218` — explicit roadmap requirement for persisted-navigation parity

  **Acceptance Criteria**:
  - [ ] New or refined tests fail against the pre-change behavior if parity is incomplete.
  - [ ] Tests explicitly cover both detail stepping and compare-target stepping semantics.
  - [ ] Tests distinguish storage-backed adjacency from loaded-row fallback behavior.

  **QA Scenarios**:
  ```
  Scenario: Run TUI state tests in red phase
    Tool: Bash
    Steps: PYTHONPATH=tests .venv/bin/python -m unittest tests.test_tui_state -v
    Expected: At least one newly added parity test fails before implementation, and failures are specific to persisted-navigation semantics.
    Evidence: .sisyphus/evidence/task-2-tui-state-red.txt

  Scenario: Verify fallback coverage exists
    Tool: Bash
    Steps: python - <<'PY'
from pathlib import Path
text = Path('tests/test_tui_state.py').read_text()
print('sqlite3.OperationalError' in text)
print('first compare target' in text)
print('last compare target' in text)
PY
    Expected: Test file explicitly covers storage-failure fallback and compare boundary messaging.
    Evidence: .sisyphus/evidence/task-2-fallback-coverage.txt
  ```

  **Commit**: NO | Message: `n/a` | Files: `["tests/test_tui_state.py"]`

- [x] 3. Update TUI contract wording for shipped persisted-navigation parity

  **What to do**: Update `TUI_CONTRACT.md` to state that detail previous/next and compare-target stepping are storage-backed persisted-navigation behaviors, not merely movement within the current loaded list window. Keep wording focused on shipped behavior and Phase 5 scope.
  **Must NOT do**: Do not expand the contract into future list filtering, numeric score entry, daemon/API coupling, or write actions.

  **Recommended Agent Profile**:
  - Category: `writing` — Reason: concise contract wording alignment
  - Skills: `[]` — local contract wording is enough
  - Omitted: `quick` — wording precision matters more than raw speed

  **Parallelization**: Can Parallel: YES | Wave 1 | Blocks: 5 | Blocked By: 1

  **References**:
  - Pattern: `TUI_CONTRACT.md` — existing read-only TUI contract and current/next behavior sections
  - Pattern: `README.md:22-27,154-162` — current shipped TUI/API/web UI boundary language
  - Pattern: `DEV_PLAN.md:177-218` — Phase 5 current status and next work
  - API/Type: `tianji/tui_state.py:265-394` — concrete parity behavior to describe

  **Acceptance Criteria**:
  - [ ] Contract doc states persisted previous/next and compare-target stepping behavior in present-tense shipped language.
  - [ ] Contract wording keeps CLI/storage as the source of truth.
  - [ ] No future-scope creep is introduced.

  **QA Scenarios**:
  ```
  Scenario: Search contract wording for parity language
    Tool: Bash
    Steps: python - <<'PY'
from pathlib import Path
text = Path('TUI_CONTRACT.md').read_text()
for phrase in ['persisted', 'previous', 'next', 'compare target']:
    print(phrase, phrase in text.lower())
PY
    Expected: Contract includes persisted-navigation parity language.
    Evidence: .sisyphus/evidence/task-3-contract-parity.txt

  Scenario: Guard against scope creep in contract
    Tool: Bash
    Steps: python - <<'PY'
from pathlib import Path
text = Path('TUI_CONTRACT.md').read_text().lower()
for forbidden in ['write action', 'list filtering', 'numeric threshold entry']:
    print(forbidden, forbidden in text)
PY
    Expected: No accidental broadening into forbidden next-slice behavior.
    Evidence: .sisyphus/evidence/task-3-contract-scope.txt
  ```

  **Commit**: NO | Message: `n/a` | Files: `["TUI_CONTRACT.md"]`

- [x] 4. Implement storage-backed parity in `tianji/tui_state.py`

  **What to do**: Make `HistoryListState` consistently use persisted adjacency for detail stepping and compare-target stepping. Preserve current `select_run_id` merge behavior for targets outside the loaded window, invalidate projected panes correctly after target changes, keep boundary messages accurate, and use loaded-row stepping only when storage adjacency resolution fails with `sqlite3.OperationalError`.
  **Must NOT do**: Do not change list-pane movement (`j`, `k`, page up/down) to use persisted adjacency. Do not move render logic into state logic or vice versa. Do not add new keybindings.

  **Recommended Agent Profile**:
  - Category: `quick` — Reason: bounded runtime change centered in one state file
  - Skills: `[]` — existing repo patterns are sufficient
  - Omitted: `subagent-driven-development` — task is single-surface and tightly scoped

  **Parallelization**: Can Parallel: NO | Wave 2 | Blocks: 5 | Blocked By: 2

  **References**:
  - Pattern: `tianji/tui_state.py:265-394` — current stepping and fallback implementation
  - Pattern: `tianji/tui_state.py:501-513` — dispatch rules for `step_previous` / `step_next`
  - API/Type: `tianji/storage.py:26-30,66-70` — `get_next_run_id`, `get_previous_run_id`, `get_run_summary`
  - Test: `tests/test_tui_state.py:98-239` — state-level expected behavior
  - Test: `tests/test_tui.py:268-441,517-687,856-973` — session-level expected behavior

  **Acceptance Criteria**:
  - [ ] `step_run` reaches persisted previous/next targets outside the loaded window without losing selection visibility.
  - [ ] `step_compare_target` skips the staged-left run in both directions and keeps compare cache invalidation correct.
  - [ ] `sqlite3.OperationalError` still falls back to loaded-row stepping instead of hard-failing.
  - [ ] Boundary messages remain `first run`, `last run`, `first compare target`, and `last compare target` where appropriate.

  **QA Scenarios**:
  ```
  Scenario: Run TUI state tests in green phase
    Tool: Bash
    Steps: PYTHONPATH=tests .venv/bin/python -m unittest tests.test_tui_state -v
    Expected: All TUI state tests pass, including newly added persisted parity cases.
    Evidence: .sisyphus/evidence/task-4-tui-state-green.txt

  Scenario: Verify no keybinding drift
    Tool: Bash
    Steps: python - <<'PY'
from tianji.tui_state import KEY_ACTION_ALIASES
print(KEY_ACTION_ALIASES['['])
print(KEY_ACTION_ALIASES[']'])
PY
    Expected: Output remains `step_previous` and `step_next`; no new keybinding contract drift.
    Evidence: .sisyphus/evidence/task-4-keybinding-contract.txt
  ```

  **Commit**: NO | Message: `n/a` | Files: `["tianji/tui_state.py"]`

- [x] 5. Add session-level TUI regression coverage and verify full suite

  **What to do**: Extend `tests/test_tui.py` so browser-session flows prove the user-visible parity behavior: detail stepping reaches persisted runs outside the loaded limit, compare-target stepping uses persisted adjacency and skips staged-left, boundary messages appear only at true first/last persisted boundaries, and projected-empty detail/compare panes still show persisted truth after navigation changes. Then run the focused and full unittest suites.
  **Must NOT do**: Do not add brittle formatting assertions unrelated to shipped semantics. Do not rely on manual-only verification; keep tests executable and deterministic.

  **Recommended Agent Profile**:
  - Category: `unspecified-high` — Reason: dense regression coverage with multiple session flows
  - Skills: `[]` — existing unittest + mocked TUI session pattern is enough
  - Omitted: `playwright` — this is terminal UI, not browser UI

  **Parallelization**: Can Parallel: NO | Wave 2 | Blocks: F1-F4 | Blocked By: 3, 4

  **References**:
  - Pattern: `tests/test_tui.py:268-441` — mixed detail/compare/lens flow coverage
  - Pattern: `tests/test_tui.py:517-687` — persisted previous/next session behavior
  - Pattern: `tests/test_tui.py:856-973` — compare boundary behavior with staged-left skip
  - Pattern: `tianji/tui.py:57-76` — session loop wiring
  - Pattern: `tianji/tui_state.py:405-513` — key handling branches that drive session behavior
  - Test: `tests/test_history_show.py:311-370,1049-1147` — CLI persisted previous/next semantics to mirror
  - Test: `tests/test_history_compare.py:366-395,503-548` — CLI compare previous/latest semantics to mirror conceptually

  **Acceptance Criteria**:
  - [ ] Session-level tests prove user-visible persisted parity in both detail and compare flows.
  - [ ] Focused TUI test modules pass.
  - [ ] Full unittest discovery passes with no regressions.
  - [ ] Contract/doc wording remains aligned with shipped behavior.

  **QA Scenarios**:
  ```
  Scenario: Run focused TUI session tests
    Tool: Bash
    Steps: PYTHONPATH=tests .venv/bin/python -m unittest tests.test_tui -v
    Expected: TUI integration/session tests pass, including persisted previous/next and compare-target parity scenarios.
    Evidence: .sisyphus/evidence/task-5-tui-session-tests.txt

  Scenario: Run full regression suite
    Tool: Bash
    Steps: PYTHONPATH=tests .venv/bin/python -m unittest discover -s tests -v
    Expected: Full suite passes and no unrelated terminal/browser/API regressions appear.
    Evidence: .sisyphus/evidence/task-5-full-suite.txt
  ```

  **Commit**: YES | Message: `fix(tui): align persisted navigation parity` | Files: `["tianji/tui_state.py", "tests/test_tui_state.py", "tests/test_tui.py", "TUI_CONTRACT.md"]`

## Final Verification Wave (MANDATORY — after ALL implementation tasks)
> 4 review agents run in PARALLEL. ALL must APPROVE. Present consolidated results to user and get explicit "okay" before completing.
> **Do NOT auto-proceed after verification. Wait for user's explicit approval before marking work complete.**
> **Never mark F1-F4 as checked before getting user's okay.** Rejection or user feedback -> fix -> re-run -> present again -> wait for okay.
- [x] F1. Plan Compliance Audit — oracle
- [x] F2. Code Quality Review — unspecified-high
- [x] F3. Real Manual QA — unspecified-high (+ interactive_bash for TUI)
- [x] F4. Scope Fidelity Check — deep

## Commit Strategy
- Keep work as one bounded feature commit after green tests and contract alignment.
- Preferred commit: `fix(tui): align persisted navigation parity`
- Do not commit during red-phase test authoring.

## Success Criteria
- TUI navigation semantics match storage-backed persisted previous/next behavior rather than only loaded-window movement for detail and compare stepping.
- No write surfaces, list filtering, or threshold-entry scope creep is introduced.
- Tests prove both state-level and session-level parity behavior.
- Contract wording reflects the shipped TUI behavior clearly enough that future sessions do not re-open the same ambiguity.
