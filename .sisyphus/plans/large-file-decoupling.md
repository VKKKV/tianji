# TianJi Large File Decoupling

## TL;DR
> **Summary**: Refactor the two main bloated source files, `tianji/cli.py` and `tianji/storage.py`, by extracting focused modules behind unchanged facades so operator behavior, persistence semantics, and import paths remain stable.
> **Deliverables**:
> - `tianji/cli.py` reduced to a thin Click registration and facade shell
> - `tianji/storage.py` reduced to a stable facade over write, read, filter, and compare helpers
> - Characterization coverage tightened around every extracted seam
> - Full refactor evidence captured with targeted unittest runs plus full-suite validation
> **Effort**: Large
> **Parallel**: YES - 4 waves
> **Critical Path**: 1 → 2/3/4 → 5 → 6/7/8/9 → 10

## Context
### Original Request
- Plan a refactor to decouple large bloated files in this project, especially files over 1000 lines.

### Interview Summary
- Scope selected: source-first.
- Priority files: `tianji/cli.py` (1633 lines) and `tianji/storage.py` (1531 lines).
- Refactor style selected: balanced, but still behavior-preserving.
- Test strategy selected: tests-after using existing unittest coverage, adding seam tests only where needed.

### Metis Review (gaps addressed)
- Freeze contracts explicitly: CLI help/error/exit-code behavior, JSON payload shapes, import paths used by tests, SQLite schema semantics, and compare/list/show ordering semantics must not change during refactor.
- Use facade-first extraction: keep `tianji.cli` and `tianji.storage` as stable entrypoints while moving internals behind them.
- Extract one seam at a time; do not combine moves with logic rewrites or opportunistic cleanup.
- End each seam with targeted tests; end each wave with the full unittest suite.

## Work Objectives
### Core Objective
Decompose `tianji/cli.py` and `tianji/storage.py` into smaller focused modules that match existing TianJi flat-module patterns without changing shipped operator behavior, persistence behavior, or public test-imported symbols.

### Deliverables
- A stable `tianji/cli.py` facade that delegates to extracted modules for source resolution, validation, daemon control, and history flows.
- A stable `tianji/storage.py` facade that delegates to extracted modules for schema/write path, read-model shaping, filters/projections, and compare diff logic.
- Supporting tests tightened or minimally split only when needed to preserve characterization of extracted seams.
- Evidence files for each task’s targeted verification and full-suite verification.

### Definition of Done (verifiable conditions with commands)
- `tianji/cli.py` keeps existing importable symbols used by tests and still dispatches all Click commands successfully.
- `tianji/storage.py` keeps existing importable symbols used by tests and preserves SQLite read/write behavior.
- Targeted seam tests pass after each extraction task.
- Full suite passes: `.venv/bin/python -m unittest discover -s tests -v`
- No new generic `utils.py`, no nested architecture packages unless strictly needed to break an import cycle.

### Must Have
- Stable facade-first extraction in `tianji/cli.py` and `tianji/storage.py`
- Flat focused helper modules consistent with `fetch.py`, `normalize.py`, `scoring.py`, `backtrack.py`, and `pipeline.py`
- Characterization coverage around source policy precedence, daemon readiness behavior, history navigation/compare presets, storage filter semantics, and compare diff semantics
- Exact evidence capture for every task

### Must NOT Have (guardrails, AI slop patterns, scope boundaries)
- Must NOT redesign flags, help text structure, payload vocabulary, SQLite schema, or history/compare semantics
- Must NOT move business logic into generic helper modules or introduce a repository/service abstraction layer foreign to this repo
- Must NOT broaden scope into `pipeline.py`, daemon internals, web UI, or TUI behavior beyond call-site compatibility
- Must NOT split tests for aesthetics alone; only split when necessary to keep seam characterization clear and maintainable
- Must NOT perform move-and-rewrite in one step

## Verification Strategy
> ZERO HUMAN INTERVENTION — all verification is agent-executed.
- Test decision: tests-after with existing `unittest` framework
- QA policy: Every task includes agent-executed targeted tests plus a concrete happy/edge scenario
- Evidence: `.sisyphus/evidence/task-{N}-{slug}.{ext}`
- Canonical full-suite gate: `.venv/bin/python -m unittest discover -s tests -v`
- Canonical targeted gates:
  - `.venv/bin/python -m unittest tests.test_cli_inputs -v`
  - `.venv/bin/python -m unittest tests.test_history_list -v`
  - `.venv/bin/python -m unittest tests.test_history_show -v`
  - `.venv/bin/python -m unittest tests.test_history_compare -v`
  - `.venv/bin/python -m unittest tests.test_tui -v`
  - `.venv/bin/python -m unittest tests.test_pipeline -v`
  - `.venv/bin/python -m unittest tests.test_daemon -v`

## Execution Strategy
### Parallel Execution Waves
> Target: 5-8 tasks per wave. <3 per wave (except final) = under-splitting.
> Extract shared dependencies as Wave-1 tasks for max parallelism.

Wave 1: contract freezing + CLI source/validation seams

Wave 2: CLI daemon/history extractions + facade slimming

Wave 3: storage write/read/filter/compare extractions

Wave 4: storage facade slimming + regression consolidation

### Dependency Matrix (full, all tasks)
- 1 blocks 2, 3, 4, 6, 7, 8, 9
- 2 blocks 3 and 4
- 3 and 4 block 5
- 6 blocks 7, 8, 9
- 7, 8, and 9 block 10
- 5 and 10 block Final Verification Wave

### Agent Dispatch Summary (wave → task count → categories)
- Wave 1 → 4 tasks → unspecified-high / quick
- Wave 2 → 1 task → unspecified-high
- Wave 3 → 4 tasks → unspecified-high
- Wave 4 → 1 task → unspecified-high
- Final verification → 4 review tasks in parallel

## TODOs
> Implementation + Test = ONE task. Never separate.
> EVERY task MUST have: Agent Profile + Parallelization + QA Scenarios.

- [x] 1. Freeze facade contracts for CLI and storage before extraction

  **What to do**: Add or tighten characterization coverage for the refactor invariants that must remain unchanged during all later tasks. Cover: `tianji.cli` import paths used by tests, `tianji.storage` import paths used by tests, CLI help/usage exit behavior, history JSON vocabulary, compare payload vocabulary, and persistence full-suite baseline. Do not start extraction yet; this task only strengthens the regression net around the existing facades.
  **Must NOT do**: Must NOT rename modules, move code, change public behavior, or introduce pytest-only patterns.

  **Recommended Agent Profile**:
  - Category: `unspecified-high` — Reason: broad contract-mapping across CLI and storage surfaces
  - Skills: `[]` — no additional skill required for plan execution
  - Omitted: `test-driven-development` — user selected tests-after, not strict RED-GREEN workflow

  **Parallelization**: Can Parallel: NO | Wave 1 | Blocks: 2, 3, 4, 6, 7, 8, 9 | Blocked By: none

  **References**:
  - Pattern: `tianji/cli.py:51-123` — current source-registry imports that tests reference directly
  - Pattern: `tianji/cli.py:938-1633` — current Click registration shell whose help and exit behavior must stay stable
  - Pattern: `tianji/storage.py:37-63` — top-level write facade surface
  - Pattern: `tianji/storage.py:66-275` — top-level read/compare facade surface
  - Test: `tests/test_cli_inputs.py:10-38` — help/command-surface characterization
  - Test: `tests/test_history_list.py:4-47` — history list vocabulary freeze
  - Test: `tests/test_history_show.py:4-126` — detail vocabulary freeze and API metadata freeze
  - Test: `tests/test_history_compare.py:4-137` — compare vocabulary freeze
  - Test: `tests/support.py:15-40` — public imports used pervasively by the flat test suite

  **Acceptance Criteria**:
  - [ ] `tests/support.py` still imports `main` from `tianji.cli` and `storage` from `tianji` unchanged after this task
  - [ ] `.venv/bin/python -m unittest tests.test_cli_inputs tests.test_history_list tests.test_history_show tests.test_history_compare -v` exits `0`
  - [ ] Evidence log saved to `.sisyphus/evidence/task-1-contract-freeze.txt`

  **QA Scenarios** (MANDATORY — task incomplete without these):
  ```
  Scenario: Contract baseline holds before extraction
    Tool: Bash
    Steps: Run `.venv/bin/python -m unittest tests.test_cli_inputs tests.test_history_list tests.test_history_show tests.test_history_compare -v | tee .sisyphus/evidence/task-1-contract-freeze.txt`
    Expected: Command exits 0; help, history list/show, and compare contract tests all pass
    Evidence: .sisyphus/evidence/task-1-contract-freeze.txt

  Scenario: Persistence baseline still intact
    Tool: Bash
    Steps: Run `.venv/bin/python -m unittest tests.test_pipeline -v | tee .sisyphus/evidence/task-1-contract-freeze-error.txt`
    Expected: Command exits 0; persistence path remains green before any extraction starts
    Evidence: .sisyphus/evidence/task-1-contract-freeze-error.txt
  ```

  **Commit**: YES | Message: `test(refactor): freeze cli and storage facade contracts` | Files: `tests/test_cli_inputs.py`, `tests/test_history_list.py`, `tests/test_history_show.py`, `tests/test_history_compare.py`, optional supporting contract tests only

- [x] 2. Extract CLI source-resolution seam behind `tianji.cli`

  **What to do**: Move `validate_fetch_policy`, `load_source_registry`, `resolve_sources`, `dedupe_sources`, and `_resolve_run_request` from `tianji/cli.py` into a new focused module named `tianji/cli_sources.py`. Leave `tianji/cli.py` re-exporting `load_source_registry` and `resolve_sources` so `tests/support.py` and direct imports remain unchanged. `_handle_run`, `_handle_daemon_run`, and `_handle_daemon_schedule` must delegate to the extracted source-resolution helpers without changing fetch-policy precedence or output defaults.
  **Must NOT do**: Must NOT change fetch-policy precedence, mutate `run_pipeline` call shape, rename exported helper names, or move Click decorators out of `cli.py` yet.

  **Recommended Agent Profile**:
  - Category: `unspecified-high` — Reason: extraction with direct public-import compatibility constraints
  - Skills: `[]` — plan already specifies exact seam and constraints
  - Omitted: `subagent-driven-development` — one seam only, no parallel implementation needed inside task

  **Parallelization**: Can Parallel: NO | Wave 1 | Blocks: 3, 4 | Blocked By: 1

  **References**:
  - Pattern: `tianji/cli.py:51-123` — exact helper set to extract and re-export
  - Pattern: `tianji/cli.py:247-319` — request-resolution behavior that must move with the source seam
  - Pattern: `tianji/cli.py:473-553` — daemon run/schedule handlers consuming `_resolve_run_request`
  - Pattern: `tianji/cli.py:555-596` — synchronous run handler consuming `_resolve_run_request`
  - Test: `tests/test_cli_inputs.py:39-197` — daemon schedule and run validation tied to source request resolution
  - Test: `tests/test_cli_inputs.py:6-8` — direct imports of `load_source_registry` and `resolve_sources`
  - Guidance: `tianji/AGENTS.md` package notes — keep package flat and avoid widening `cli.py`

  **Acceptance Criteria**:
  - [ ] `from tianji.cli import load_source_registry, resolve_sources` still works unchanged
  - [ ] `.venv/bin/python -m unittest tests.test_cli_inputs -v` exits `0`
  - [ ] `.venv/bin/python -m unittest tests.test_pipeline -v` exits `0`
  - [ ] Evidence logs saved to `.sisyphus/evidence/task-2-cli-sources.txt` and `.sisyphus/evidence/task-2-cli-sources-error.txt`

  **QA Scenarios** (MANDATORY — task incomplete without these):
  ```
  Scenario: CLI source resolution still honors current operator contract
    Tool: Bash
    Steps: Run `.venv/bin/python -m unittest tests.test_cli_inputs -v | tee .sisyphus/evidence/task-2-cli-sources.txt`
    Expected: Command exits 0; direct helper imports, source-config parsing, fetch-policy precedence, and daemon run/schedule flows remain green
    Evidence: .sisyphus/evidence/task-2-cli-sources.txt

  Scenario: Extracted source seam does not break pipeline entry
    Tool: Bash
    Steps: Run `.venv/bin/python -m unittest tests.test_pipeline -v | tee .sisyphus/evidence/task-2-cli-sources-error.txt`
    Expected: Command exits 0; run entry still invokes pipeline with unchanged behavior
    Evidence: .sisyphus/evidence/task-2-cli-sources-error.txt
  ```

  **Commit**: YES | Message: `refactor(cli): extract source resolution helpers` | Files: `tianji/cli.py`, `tianji/cli_sources.py`, optionally `tests/test_cli_inputs.py`

- [x] 3. Extract CLI validation and history-run-id resolution seam

  **What to do**: Move CLI-only validation helpers into `tianji/cli_validation.py`: `validate_score_range`, `validate_positive_run_id`, `_validate_schedule_spec`, and `_resolve_compare_run_ids`. Keep `tianji/cli.py` as the facade that imports and uses these helpers. Preserve exact UsageError timing and wording for invalid score windows, invalid run ids, and mixed compare presets.
  **Must NOT do**: Must NOT alter error strings, combine validation with handler logic, or change which command surface raises which error.

  **Recommended Agent Profile**:
  - Category: `quick` — Reason: narrow extraction seam with well-bounded tests
  - Skills: `[]` — no extra skill needed
  - Omitted: `systematic-debugging` — task is planned as straight extraction, not active defect triage

  **Parallelization**: Can Parallel: YES | Wave 1 | Blocks: 5 | Blocked By: 2, 1

  **References**:
  - Pattern: `tianji/cli.py:135-149` — score/range and run-id validation helpers
  - Pattern: `tianji/cli.py:320-329` — schedule validation contract
  - Pattern: `tianji/cli.py:671-775` — history-show navigation validation path
  - Pattern: `tianji/cli.py:776-929` — compare preset resolution and handler validation path
  - Test: `tests/test_cli_inputs.py:39-100` — schedule validation wording and exit behavior
  - Test: `tests/test_history_show.py` — `history-show` invalid run-id and relative navigation coverage
  - Test: `tests/test_history_compare.py:232-240` and surrounding invalid-preset cases — compare validation contract

  **Acceptance Criteria**:
  - [ ] `.venv/bin/python -m unittest tests.test_cli_inputs tests.test_history_show tests.test_history_compare -v` exits `0`
  - [ ] Invalid window and run-id errors remain raised from the same CLI surfaces with unchanged wording
  - [ ] Evidence log saved to `.sisyphus/evidence/task-3-cli-validation.txt`

  **QA Scenarios** (MANDATORY — task incomplete without these):
  ```
  Scenario: Validation extraction preserves parse-time failures
    Tool: Bash
    Steps: Run `.venv/bin/python -m unittest tests.test_cli_inputs tests.test_history_show tests.test_history_compare -v | tee .sisyphus/evidence/task-3-cli-validation.txt`
    Expected: Command exits 0; invalid schedule, invalid run-id, and compare preset exclusivity tests all pass
    Evidence: .sisyphus/evidence/task-3-cli-validation.txt

  Scenario: Validation extraction does not break CLI help or normal dispatch
    Tool: Bash
    Steps: Run `.venv/bin/python -m unittest tests.test_tui -v | tee .sisyphus/evidence/task-3-cli-validation-error.txt`
    Expected: Command exits 0; CLI still dispatches `tui` through unchanged facade wiring
    Evidence: .sisyphus/evidence/task-3-cli-validation-error.txt
  ```

  **Commit**: YES | Message: `refactor(cli): extract validation and compare resolution` | Files: `tianji/cli.py`, `tianji/cli_validation.py`, optionally targeted CLI-history tests

- [x] 4. Extract CLI daemon process/control seam

  **What to do**: Move daemon-specific process helpers and handlers into `tianji/cli_daemon.py`: `_pid_file_for_socket`, `_read_pid_file`, `_write_pid_file`, `_remove_pid_file`, `_is_pid_running`, `_wait_for_socket`, `_wait_for_api`, `_send_daemon_payload`, `_handle_daemon_start`, `_handle_daemon_stop`, `_handle_daemon_status`, `_handle_daemon_run`, and `_handle_daemon_schedule`. Keep `cli.py` holding only the Click command definitions and delegating to the extracted functions.
  **Must NOT do**: Must NOT change PID-file naming, socket/API readiness semantics, daemon JSON output shape, or CLI subcommand names/options.

  **Recommended Agent Profile**:
  - Category: `unspecified-high` — Reason: process-control extraction with many edge-case contracts
  - Skills: `[]` — plan already defines the seam and guardrails
  - Omitted: `verification-before-completion` — task QA is already fully specified here

  **Parallelization**: Can Parallel: YES | Wave 1 | Blocks: 5 | Blocked By: 2, 1

  **References**:
  - Pattern: `tianji/cli.py:151-245` — daemon filesystem/socket/API helper cluster
  - Pattern: `tianji/cli.py:330-554` — daemon command handlers to extract intact
  - Test: `tests/test_cli_inputs.py:102-240` — queueing, status, bounded schedule, and start/stop behavior
  - Test: `tests/test_daemon.py` — daemon/API integration guardrails
  - Pattern: `tianji/daemon.py` — downstream daemon server contract consumed by CLI handlers

  **Acceptance Criteria**:
  - [ ] `.venv/bin/python -m unittest tests.test_cli_inputs tests.test_daemon -v` exits `0`
  - [ ] `.venv/bin/python -m unittest tests.test_pipeline -v` exits `0`
  - [ ] Evidence logs saved to `.sisyphus/evidence/task-4-cli-daemon.txt` and `.sisyphus/evidence/task-4-cli-daemon-error.txt`

  **QA Scenarios** (MANDATORY — task incomplete without these):
  ```
  Scenario: Daemon extraction preserves process and queue contracts
    Tool: Bash
    Steps: Run `.venv/bin/python -m unittest tests.test_cli_inputs tests.test_daemon -v | tee .sisyphus/evidence/task-4-cli-daemon.txt`
    Expected: Command exits 0; daemon start/status/stop/run/schedule behavior and local API contracts stay unchanged
    Evidence: .sisyphus/evidence/task-4-cli-daemon.txt

  Scenario: Daemon extraction does not regress synchronous run path
    Tool: Bash
    Steps: Run `.venv/bin/python -m unittest tests.test_pipeline -v | tee .sisyphus/evidence/task-4-cli-daemon-error.txt`
    Expected: Command exits 0; synchronous pipeline invocation still works after daemon seam extraction
    Evidence: .sisyphus/evidence/task-4-cli-daemon-error.txt
  ```

  **Commit**: YES | Message: `refactor(cli): extract daemon control helpers` | Files: `tianji/cli.py`, `tianji/cli_daemon.py`, optionally targeted daemon tests

- [x] 5. Extract CLI history/read seam and slim `tianji.cli` to a registration shell

  **What to do**: Move `_handle_history`, `_handle_history_show`, `_handle_history_compare`, and `_handle_tui` into `tianji/cli_history.py`. Keep all Click decorators and command definitions in `tianji/cli.py`, but reduce each command body to a thin delegate call into `cli_sources`, `cli_validation`, `cli_daemon`, or `cli_history`. Preserve `main()` in `tianji.cli` exactly as the authoritative CLI entrypoint.
  **Must NOT do**: Must NOT change command names/options, help grouping, `main()` signature, JSON output formatting, or TUI launch contract.

  **Recommended Agent Profile**:
  - Category: `unspecified-high` — Reason: final CLI consolidation with broad cross-surface regression risk
  - Skills: `[]` — plan already nails the target structure
  - Omitted: `using-git-worktrees` — execution should stay in current workspace unless executor chooses otherwise

  **Parallelization**: Can Parallel: NO | Wave 2 | Blocks: 10 | Blocked By: 3, 4

  **References**:
  - Pattern: `tianji/cli.py:597-932` — history, history-show, history-compare, and TUI handlers to extract
  - Pattern: `tianji/cli.py:938-1633` — final Click registration shell that should remain in-place
  - Test: `tests/test_history_list.py:4-220` — history list CLI behavior
  - Test: `tests/test_history_show.py:217-1040` — history-show CLI behavior and filter semantics
  - Test: `tests/test_history_compare.py:138-1288` — compare CLI behavior and diff semantics
  - Test: `tests/test_tui.py` — TUI dispatch behavior from CLI facade

  **Acceptance Criteria**:
  - [ ] `tianji.cli.main(argv)` remains unchanged and all command bodies delegate cleanly
  - [ ] `.venv/bin/python -m unittest tests.test_history_list tests.test_history_show tests.test_history_compare tests.test_tui -v` exits `0`
  - [ ] `.venv/bin/python -m unittest tests.test_cli_inputs -v` exits `0`
  - [ ] Evidence logs saved to `.sisyphus/evidence/task-5-cli-history.txt` and `.sisyphus/evidence/task-5-cli-history-error.txt`

  **QA Scenarios** (MANDATORY — task incomplete without these):
  ```
  Scenario: Slim CLI shell still serves all read-only surfaces
    Tool: Bash
    Steps: Run `.venv/bin/python -m unittest tests.test_history_list tests.test_history_show tests.test_history_compare tests.test_tui -v | tee .sisyphus/evidence/task-5-cli-history.txt`
    Expected: Command exits 0; history list/show/compare and TUI routing behave identically after extraction
    Evidence: .sisyphus/evidence/task-5-cli-history.txt

  Scenario: Slim CLI shell still keeps top-level help and daemon separation intact
    Tool: Bash
    Steps: Run `.venv/bin/python -m unittest tests.test_cli_inputs -v | tee .sisyphus/evidence/task-5-cli-history-error.txt`
    Expected: Command exits 0; top-level help, daemon help, and operator-facing CLI messages remain stable
    Evidence: .sisyphus/evidence/task-5-cli-history-error.txt
  ```

  **Commit**: YES | Message: `refactor(cli): extract history handlers and slim facade` | Files: `tianji/cli.py`, `tianji/cli_history.py`, optionally supporting CLI tests

- [x] 6. Extract storage schema and write-path seam behind `tianji.storage`

  **What to do**: Move write-path functions into `tianji/storage_write.py`: `initialize_schema`, `ensure_column`, `insert_run`, `ensure_canonical_source_items`, `insert_raw_items`, `insert_normalized_events`, `insert_scored_events`, and `insert_intervention_candidates`. Keep `persist_run` in `tianji/storage.py` as the stable facade calling the extracted write module. Preserve one-run-row-per-invocation semantics, canonical source-item reuse/versioning, and current SQLite schema definitions.
  **Must NOT do**: Must NOT alter table names, column definitions, migration behavior, foreign-key behavior, or hash derivation semantics.

  **Recommended Agent Profile**:
  - Category: `unspecified-high` — Reason: persistence extraction with schema invariants and side-effect risk
  - Skills: `[]` — plan provides complete seam boundaries
  - Omitted: `systematic-debugging` — not a bugfix task

  **Parallelization**: Can Parallel: NO | Wave 3 | Blocks: 7, 8, 9 | Blocked By: 1

  **References**:
  - Pattern: `tianji/storage.py:37-63` — stable `persist_run` facade that must remain
  - Pattern: `tianji/storage.py:332-687` — schema/write path to extract intact
  - Pattern: `tianji/fetch.py` hash helpers imported by storage — preserve current canonical-hash dependency
  - Test: `tests/test_pipeline.py` — authoritative persistence/integration coverage
  - Test: `tests/test_history_list.py` — persisted run summaries consumed downstream

  **Acceptance Criteria**:
  - [ ] `tianji.storage.persist_run(...)` still exists at the same import path
  - [ ] `.venv/bin/python -m unittest tests.test_pipeline tests.test_history_list -v` exits `0`
  - [ ] Evidence log saved to `.sisyphus/evidence/task-6-storage-write.txt`

  **QA Scenarios** (MANDATORY — task incomplete without these):
  ```
  Scenario: Storage write extraction preserves SQLite persistence semantics
    Tool: Bash
    Steps: Run `.venv/bin/python -m unittest tests.test_pipeline tests.test_history_list -v | tee .sisyphus/evidence/task-6-storage-write.txt`
    Expected: Command exits 0; schema init, canonical source-item reuse, persisted run creation, and list reads remain stable
    Evidence: .sisyphus/evidence/task-6-storage-write.txt

  Scenario: Write-path extraction does not regress daemon-backed persisted reads
    Tool: Bash
    Steps: Run `.venv/bin/python -m unittest tests.test_daemon -v | tee .sisyphus/evidence/task-6-storage-write-error.txt`
    Expected: Command exits 0; daemon/API surfaces still read persisted runs correctly
    Evidence: .sisyphus/evidence/task-6-storage-write-error.txt
  ```

  **Commit**: YES | Message: `refactor(storage): extract schema and write path` | Files: `tianji/storage.py`, `tianji/storage_write.py`, optionally targeted persistence tests

- [x] 7. Extract storage read-model shaping seam

  **What to do**: Move row coercion and read payload builders into `tianji/storage_views.py`: `build_run_list_item`, `get_top_scored_event_summaries`, `build_run_detail`, `coerce_run_row`, `build_scored_event_detail`, `build_intervention_candidate_detail`, and any tightly coupled row coercers. Keep `list_runs` and `get_run_summary` in `tianji/storage.py` initially as the public facade functions that call the extracted view builders.
  **Must NOT do**: Must NOT alter payload field names, default values, row ordering, JSON loading behavior, or top-scored-event selection semantics.

  **Recommended Agent Profile**:
  - Category: `quick` — Reason: cohesive shaping seam with strong contract tests
  - Skills: `[]` — no extra skill needed
  - Omitted: `requesting-code-review` — review belongs in final verification wave, not this task

  **Parallelization**: Can Parallel: YES | Wave 3 | Blocks: 10 | Blocked By: 6, 1

  **References**:
  - Pattern: `tianji/storage.py:66-123` — `list_runs` facade consuming list-item builders
  - Pattern: `tianji/storage.py:126-214` — `get_run_summary` facade consuming detail builders
  - Pattern: `tianji/storage.py:689-974` — read-model shaping cluster to extract
  - Test: `tests/test_history_list.py:4-220` — list payload and top-score/top-group contract checks
  - Test: `tests/test_history_show.py:4-216` — detail payload and filter input shape checks

  **Acceptance Criteria**:
  - [ ] `list_runs` and `get_run_summary` stay importable from `tianji.storage`
  - [ ] `.venv/bin/python -m unittest tests.test_history_list tests.test_history_show -v` exits `0`
  - [ ] Evidence log saved to `.sisyphus/evidence/task-7-storage-views.txt`

  **QA Scenarios** (MANDATORY — task incomplete without these):
  ```
  Scenario: Read-model extraction preserves list and detail payloads
    Tool: Bash
    Steps: Run `.venv/bin/python -m unittest tests.test_history_list tests.test_history_show -v | tee .sisyphus/evidence/task-7-storage-views.txt`
    Expected: Command exits 0; list/detail contracts, top-score fields, and event-group shaping remain unchanged
    Evidence: .sisyphus/evidence/task-7-storage-views.txt

  Scenario: Read-model extraction does not regress compare consumers
    Tool: Bash
    Steps: Run `.venv/bin/python -m unittest tests.test_history_compare -v | tee .sisyphus/evidence/task-7-storage-views-error.txt`
    Expected: Command exits 0; compare still receives the same run-summary shape from storage facades
    Evidence: .sisyphus/evidence/task-7-storage-views-error.txt
  ```

  **Commit**: YES | Message: `refactor(storage): extract read-model builders` | Files: `tianji/storage.py`, `tianji/storage_views.py`, optionally targeted history tests

- [x] 8. Extract storage filtering and projection seam

  **What to do**: Move projection/filter helpers into `tianji/storage_filters.py`: `filter_scored_event_details`, `filter_intervention_candidate_details`, `filter_event_group_details`, `filter_run_list_items`, `is_numeric_run_metric_at_or_above`, `is_numeric_run_metric_at_or_below`, `parse_history_timestamp`, `is_history_timestamp_on_or_after`, and `is_history_timestamp_on_or_before`. Update `list_runs` and `get_run_summary` to call the extracted helpers while preserving current filtered-lens semantics.
  **Must NOT do**: Must NOT change filter thresholds, null-handling for runs without top scores, intervention alignment behavior, or history timestamp comparison semantics.

  **Recommended Agent Profile**:
  - Category: `quick` — Reason: bounded functional seam with highly targeted regression tests
  - Skills: `[]` — no extra skill needed
  - Omitted: `test-driven-development` — tests-after strategy already chosen

  **Parallelization**: Can Parallel: YES | Wave 3 | Blocks: 10 | Blocked By: 6, 1

  **References**:
  - Pattern: `tianji/storage.py:174-213` — `get_run_summary` projection application order
  - Pattern: `tianji/storage.py:860-960` — scored-event, intervention, and event-group filters to extract
  - Pattern: `tianji/storage.py:1332-1477` — run-list filters and timestamp helpers to extract
  - Test: `tests/test_history_show.py:127-216` — direct unit-style filter helper contracts
  - Test: `tests/test_history_list.py` — run-list filter and top-score threshold behavior
  - Test: `tests/test_history_compare.py` — compare-side projected-lens semantics depending on the same filters

  **Acceptance Criteria**:
  - [ ] Existing direct test calls to storage filter helpers still work through stable imports or explicit re-exports from `tianji.storage`
  - [ ] `.venv/bin/python -m unittest tests.test_history_list tests.test_history_show tests.test_history_compare -v` exits `0`
  - [ ] Evidence log saved to `.sisyphus/evidence/task-8-storage-filters.txt`

  **QA Scenarios** (MANDATORY — task incomplete without these):
  ```
  Scenario: Filter extraction preserves projection semantics
    Tool: Bash
    Steps: Run `.venv/bin/python -m unittest tests.test_history_list tests.test_history_show tests.test_history_compare -v | tee .sisyphus/evidence/task-8-storage-filters.txt`
    Expected: Command exits 0; score filters, event-group lenses, intervention alignment, and compare projections remain unchanged
    Evidence: .sisyphus/evidence/task-8-storage-filters.txt

  Scenario: Filter extraction preserves null/top-score edge behavior in persisted flows
    Tool: Bash
    Steps: Run `.venv/bin/python -m unittest tests.test_pipeline -v | tee .sisyphus/evidence/task-8-storage-filters-error.txt`
    Expected: Command exits 0; runs without scored events remain representable and do not break numeric filter consumers
    Evidence: .sisyphus/evidence/task-8-storage-filters-error.txt
  ```

  **Commit**: YES | Message: `refactor(storage): extract filtering helpers` | Files: `tianji/storage.py`, `tianji/storage_filters.py`, optionally targeted history tests

- [x] 9. Extract storage compare projection and diff seam

  **What to do**: Move compare-specific shaping into `tianji/storage_compare.py`: `build_compare_side`, `build_compare_diff`, `get_top_score_metric`, `build_score_delta`, `build_top_event_group_evidence_diff`, and `format_evidence_chain_link`. Keep `compare_runs` in `tianji/storage.py` as the stable facade assembling left/right summaries and then delegating compare shaping to the extracted module.
  **Must NOT do**: Must NOT change compare payload vocabulary, comparable-vs-contrast semantics, evidence diff formatting, delta rounding behavior, or compare-side top-item selection.

  **Recommended Agent Profile**:
  - Category: `unspecified-high` — Reason: compare diff logic has many nested contract semantics
  - Skills: `[]` — plan already specifies seam and invariants
  - Omitted: `systematic-debugging` — treat as controlled extraction, not semantic redesign

  **Parallelization**: Can Parallel: YES | Wave 3 | Blocks: 10 | Blocked By: 6, 1

  **References**:
  - Pattern: `tianji/storage.py:216-275` — `compare_runs` facade to preserve
  - Pattern: `tianji/storage.py:975-1314` — compare-side/diff cluster to extract
  - Test: `tests/test_history_compare.py:4-230` — compare contract vocabulary and diff-field expectations
  - Test: `tests/test_daemon.py` — API compare responses consuming the same compare payload semantics
  - Test: `tianji/tui_render.py:472-744` — compare rendering logic expects current compare-side/diff field structure

  **Acceptance Criteria**:
  - [ ] `compare_runs` stays importable from `tianji.storage`
  - [ ] `.venv/bin/python -m unittest tests.test_history_compare tests.test_daemon -v` exits `0`
  - [ ] `.venv/bin/python -m unittest tests.test_tui -v` exits `0`
  - [ ] Evidence logs saved to `.sisyphus/evidence/task-9-storage-compare.txt` and `.sisyphus/evidence/task-9-storage-compare-error.txt`

  **QA Scenarios** (MANDATORY — task incomplete without these):
  ```
  Scenario: Compare extraction preserves stored compare semantics
    Tool: Bash
    Steps: Run `.venv/bin/python -m unittest tests.test_history_compare tests.test_daemon -v | tee .sisyphus/evidence/task-9-storage-compare.txt`
    Expected: Command exits 0; compare payload fields, comparability flags, and daemon/API compare responses remain stable
    Evidence: .sisyphus/evidence/task-9-storage-compare.txt

  Scenario: Compare extraction preserves TUI compare consumers
    Tool: Bash
    Steps: Run `.venv/bin/python -m unittest tests.test_tui -v | tee .sisyphus/evidence/task-9-storage-compare-error.txt`
    Expected: Command exits 0; TUI compare rendering still understands current compare-side and diff shapes
    Evidence: .sisyphus/evidence/task-9-storage-compare-error.txt
  ```

  **Commit**: YES | Message: `refactor(storage): extract compare builders` | Files: `tianji/storage.py`, `tianji/storage_compare.py`, optionally targeted compare tests

- [x] 10. Slim `tianji.storage` facade and run full regression consolidation

  **What to do**: Reduce `tianji/storage.py` to a stable facade module that imports from `storage_write.py`, `storage_views.py`, `storage_filters.py`, and `storage_compare.py`, re-exporting the symbols currently used by the test suite and CLI. Remove only dead internal duplication created by earlier extractions. If any test file became unmanageably large or repetitive solely because a seam now needs clearer characterization, perform the minimum supporting split while preserving flat `tests/` discovery and `support.py` import patterns.
  **Must NOT do**: Must NOT add nested packages under `tests/`, break `support.py` import style, change facade symbol names, or make semantic cleanups unrelated to file decoupling.

  **Recommended Agent Profile**:
  - Category: `unspecified-high` — Reason: final consolidation with broad regression surface
  - Skills: `[]` — plan already defines exact limits
  - Omitted: `finishing-a-development-branch` — branch integration is outside this planning scope

  **Parallelization**: Can Parallel: NO | Wave 4 | Blocks: Final Verification Wave | Blocked By: 5, 7, 8, 9

  **References**:
  - Pattern: `tianji/storage.py:37-275` — stable public facades that must remain visible
  - Pattern: `tianji/storage.py:332-1531` — internal clusters that should now live in focused modules
  - Pattern: `tests/support.py:15-40` — flat test import hub that must keep working
  - Guidance: `tests/AGENTS.md:21-35` — keep unittest discovery flat and source-of-truth command unchanged
  - Test: full targeted history/CLI/daemon/TUI/pipeline suites listed in Verification Strategy

  **Acceptance Criteria**:
  - [ ] `tianji/storage.py` is a facade-oriented file and no longer mixes write/read/filter/compare implementation bodies
  - [ ] `.venv/bin/python -m unittest tests.test_cli_inputs tests.test_history_list tests.test_history_show tests.test_history_compare tests.test_tui tests.test_pipeline tests.test_daemon -v` exits `0`
  - [ ] `.venv/bin/python -m unittest discover -s tests -v` exits `0`
  - [ ] Evidence logs saved to `.sisyphus/evidence/task-10-storage-facade.txt` and `.sisyphus/evidence/task-10-full-suite.txt`

  **QA Scenarios** (MANDATORY — task incomplete without these):
  ```
  Scenario: Final focused regression passes after storage facade slimming
    Tool: Bash
    Steps: Run `.venv/bin/python -m unittest tests.test_cli_inputs tests.test_history_list tests.test_history_show tests.test_history_compare tests.test_tui tests.test_pipeline tests.test_daemon -v | tee .sisyphus/evidence/task-10-storage-facade.txt`
    Expected: Command exits 0; all source-first seams remain green together
    Evidence: .sisyphus/evidence/task-10-storage-facade.txt

  Scenario: Full suite still passes under canonical repo command
    Tool: Bash
    Steps: Run `.venv/bin/python -m unittest discover -s tests -v | tee .sisyphus/evidence/task-10-full-suite.txt`
    Expected: Command exits 0; canonical repo verification remains green after full decoupling
    Evidence: .sisyphus/evidence/task-10-full-suite.txt
  ```

  **Commit**: YES | Message: `refactor(storage): slim facade and consolidate regressions` | Files: `tianji/storage.py`, extracted `tianji/storage_*.py` modules, only minimal supporting test files if needed

## Final Verification Wave (MANDATORY — after ALL implementation tasks)
> 4 review agents run in PARALLEL. ALL must APPROVE. Present consolidated results to user and get explicit "okay" before completing.
> **Do NOT auto-proceed after verification. Wait for user's explicit approval before marking work complete.**
> **Never mark F1-F4 as checked before getting user's okay.** Rejection or user feedback -> fix -> re-run -> present again -> wait for okay.
- [x] F1. Plan Compliance Audit — oracle
- [x] F2. Code Quality Review — unspecified-high
- [x] F3. Real Manual QA — unspecified-high (+ playwright if UI)
- [x] F4. Scope Fidelity Check — deep

## Commit Strategy
- Use characterization-test-driven refactoring.
- Commit in seam pairs where practical: tighten characterization coverage, then extract behind unchanged facade.
- One seam per commit or commit pair; no formatting-only or opportunistic behavior changes mixed in.
- End each wave with the full unittest suite.

## Success Criteria
- `tianji/cli.py` no longer mixes source resolution, daemon control, history flows, and Click registration in one file.
- `tianji/storage.py` no longer mixes write path, read shaping, filtering, and compare diff logic in one file.
- All targeted regression suites and the full suite pass.
- Public CLI/operator behavior and persisted payload contracts remain unchanged.
