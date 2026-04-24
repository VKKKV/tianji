# Directory Structure

> How backend code is organized in this project.

---

## Overview

TianJi uses a **flat module organization** inside the single `tianji/` package. Files are named by their stage in the pipeline (`fetch.py`, `normalize.py`, `scoring.py`, `backtrack.py`). When a file grows too large, it splits on a prefix convention (`cli_*.py`, `storage_*.py`, `tui_*.py`). The test suite mirrors this flatness with `tests/support.py` as the shared import hub.

---

## Directory Layout

```
tianji/
├── tianji/                  # Owned Python source; current product surface
│   ├── __init__.py          # Empty (package marker)
│   ├── __main__.py          # Entry: delegates to cli.main()
│   ├── cli.py               # Click CLI entry: run/daemon/history/history-show/history-compare/tui commands
│   ├── cli_history.py       # History subcommand handlers (list, show, compare)
│   ├── cli_sources.py       # Source config registry loading and resolution
│   ├── cli_validation.py    # Parameter validation (score ranges, run IDs, schedule specs)
│   ├── cli_daemon.py        # Daemon subcommand handlers (start, stop, status, run, schedule)
│   ├── models.py            # Dataclasses: RawItem, NormalizedEvent, ScoredEvent, InterventionCandidate, RunArtifact
│   ├── fetch.py             # Feed parsing (RSS/Atom), URL fetching, fixture reading, canonical hashing
│   ├── normalize.py         # Event extraction: keyword extraction, region/actor matching, field scoring
│   ├── scoring.py           # Divergence scoring: Im (impact) and Fa (field attraction) with rationale
│   ├── backtrack.py         # Intervention candidate generation from scored events and event groups
│   ├── pipeline.py          # Orchestration spine + event grouping/clustering algorithms
│   ├── storage.py           # Public re-export hub for storage_write, storage_views, storage_filters, storage_compare
│   ├── storage_write.py     # Schema init, column migration, all INSERTs, persist_run()
│   ├── storage_views.py     # Read queries: list_runs, get_run_summary, navigation helpers, row coercion
│   ├── storage_filters.py   # In-memory filtering for events, candidates, groups, run lists
│   ├── storage_compare.py   # Run comparison: compare_runs, diff building, evidence chain diffing
│   ├── daemon.py            # UNIX-socket daemon: job queue, thread pool, HTTP API wrapper
│   ├── api.py               # Loopback HTTP API (meta, runs, compare endpoints)
│   ├── tui.py               # Rich TUI entry: terminal raw mode, browser session
│   ├── tui_render.py        # TUI rendering: layout, panel formatting, row formatting
│   ├── tui_state.py         # TUI state machine: HistoryListState, key handling, projection
│   ├── webui_server.py      # Optional web UI server; proxies API calls to daemon
│   └── webui/               # Static web frontend assets
├── tests/                   # Owned verification surface; fixture-first unittest suite
│   ├── support.py           # Shared imports hub + FIXTURE_PATH + load_contract_fixture()
│   ├── test_pipeline.py     # Integration: pipeline, persistence, canonical hashing, RSS/Atom parsing
│   ├── test_scoring.py      # Unit: Im/Fa scoring semantics, field attraction, rationale
│   ├── test_cli_inputs.py   # CLI: source config resolution, failure paths
│   ├── test_history_*.py    # History: list, show, compare operations
│   ├── test_daemon.py       # Daemon: lifecycle, job queue
│   ├── test_tui*.py         # TUI: render, state, integration
│   ├── test_webui*.py       # Web UI: server and browser tests
│   └── fixtures/            # Test data
│       ├── sample_feed.xml  # Canonical deterministic RSS 2.0 feed
│       └── contracts/       # Expected API payload schemas (run_artifact, history_list, etc.)
├── pyproject.toml
├── AGENTS.md
└── README.md
```

---

## Module Organization

### Flat Package with Stage-Oriented Files

Each pipeline stage gets its own file:
- `fetch.py` → `normalize.py` → `scoring.py` → `backtrack.py`
- `pipeline.py` orchestrates them

### Naming Conventions

| Convention | Pattern | When to Use |
|------------|---------|-------------|
| Stage files | `{stage}.py` | One file per pipeline stage (`fetch.py`, `scoring.py`) |
| Prefixed sub-files | `{prefix}_{name}.py` | When a module grows too large — share a prefix (`cli_*.py`, `storage_*.py`, `tui_*.py`) |
| Hub re-exports | `{prefix}.py` | Re-export the public API from sub-modules (`storage.py` re-exports from `storage_write.py`, `storage_views.py`, etc.) |
| Test files | `test_{feature}.py` | One test file per feature, all in the flat `tests/` directory |
| Test support | `tests/support.py` | Single shared import hub for all tests |

### Spec Document Naming

Specification documents under `.trellis/spec/` use **lowercase kebab-case** filenames instead of root-doc uppercase names.

| Document Type | Pattern | Examples |
|---------------|---------|----------|
| Backend specs | `lowercase-kebab-case.md` | `scoring-spec.md`, `development-plan.md` |
| Backend contracts | `lowercase-kebab-case.md` | `daemon-contract.md`, `local-api-contract.md`, `tui-contract.md`, `web-ui-contract.md` |
| Guide docs | `lowercase-kebab-case.md` | `code-reuse-thinking-guide.md`, `cross-layer-thinking-guide.md` |

Why this project uses it:
- spec files behave like a structured documentation tree, not root-level product docs
- kebab-case keeps paths visually consistent in indexes and cross-links
- it avoids carrying older root-doc naming style such as `SCORING_SPEC.md` or `TUI_CONTRACT.md` into the Trellis spec tree

Examples:

```text
.trellis/spec/backend/scoring-spec.md
.trellis/spec/backend/development-plan.md
.trellis/spec/backend/contracts/local-api-contract.md
```

```text
# Don't do this inside .trellis/spec/
.trellis/spec/backend/SCORING_SPEC.md
.trellis/spec/backend/contracts/LOCAL_API_CONTRACT.md
```

### Forbidden Patterns

- **No `utils.py` catch-all** — every file has a specific purpose and name
- **No deeply nested sub-packages** — keep the package flat until multiple files per stage justify nesting
- **No `__init__.py` that does work** — `tianji/__init__.py` is empty
- **No test subdirectories** — `tests/` stays flat
- **No root-doc uppercase names inside `.trellis/spec/`** — use lowercase kebab-case for spec and contract documents

---

## Examples of Well-Organized Modules

- **Storage split**: `storage_write.py:402` handles writes, `storage_views.py:475` handles reads, `storage_filters.py:251` handles in-memory filtering, `storage_compare.py:423` handles comparisons — all re-exported through `storage.py:81`
- **CLI split**: `cli.py:789` defines the command tree, delegates heavy handlers to `cli_history.py`, `cli_daemon.py`, `cli_sources.py`, `cli_validation.py`
- **TUI split**: `tui.py:77` (entry), `tui_render.py` (rendering), `tui_state.py` (state machine)

---

**Language**: English
