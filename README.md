# TianJi (天机)

TianJi is a local-first intelligence prototype aimed at one core loop: fetch signals, infer the current branch, and backtrack likely intervention points.

The repository currently ships a **one-shot CLI MVP**, not the full daemonized "God Engine" yet. The near-term goal is to prove the end-to-end value of `单次数据获取 -> 推演 -> 反推` with deterministic, inspectable output before expanding into a long-running orchestration system.

## Current Reality

The implemented slice is a Python CLI pipeline under `tianji/`.

It supports:

- loading one or more local RSS/Atom fixtures
- optionally fetching one or more live feeds once
- loading named fetch targets from a JSON source registry
- normalizing events into a deterministic internal model
- scoring events with rule-based divergence-style heuristics
- producing ranked intervention candidates
- writing a structured JSON artifact
- optionally persisting runs to local SQLite
- listing persisted runs and inspecting stored run details from SQLite
- surfacing clean CLI errors for malformed feeds and failed fetches

This keeps the first version testable, local, and reproducible.

## Quick Start

### Run from fixture

```bash
python3 -m tianji run --fixture tests/fixtures/sample_feed.xml
```

### Run with output file

```bash
python3 -m tianji run --fixture tests/fixtures/sample_feed.xml --output runs/latest-run.json
```

### Run with one-time fetch

```bash
python3 -m tianji run --fetch --source-url https://example.com/feed.xml
```

### Run with optional SQLite persistence

```bash
python3 -m tianji run --fixture tests/fixtures/sample_feed.xml --sqlite-path runs/tianji.sqlite3
```

### Run with a source registry

Create a JSON file shaped like:

```json
{
  "sources": [
    {
      "name": "example-feed",
      "url": "https://example.com/feed.xml"
    }
  ]
}
```

Then run:

```bash
python3 -m tianji run --fetch --source-config /path/to/sources.json --source-name example-feed
```

### Inspect persisted run history

```bash
python3 -m tianji history --sqlite-path runs/tianji.sqlite3
python3 -m tianji history-show --sqlite-path runs/tianji.sqlite3 --run-id 1
```

### Run tests

```bash
python3 -m unittest discover -s tests -v
```

## What the Pipeline Does

The current MVP flow is:

1. **Fetch / Load**  
   Load RSS or Atom input from local fixture files, or fetch a feed once from a URL.

2. **Normalize**  
   Convert raw feed items into normalized events with extracted keywords, actors, regions, and field scores.

3. **Infer / Score**  
   Apply deterministic scoring rules to estimate event impact and field attraction, then rank the most important events.

4. **Backtrack**  
   Generate likely intervention targets and intervention types from the highest-ranked events.

5. **Emit Artifact**  
   Write a JSON report with input summary, scenario summary, scored events, and intervention candidates.

6. **Persist Run (Optional)**  
   Store run metadata plus raw, normalized, scored, and intervention rows in SQLite when `--sqlite-path` is provided.

7. **Inspect Run History (Optional)**  
   Query persisted run summaries later with `history` and inspect one stored run's summaries, scored events, and intervention candidates with `history-show`.

## Output Artifact

By default, the CLI writes to `runs/latest-run.json`.

The artifact includes:

- `schema_version`: stable top-level artifact contract version
- `input_summary`: item counts and source list
- `scenario_summary`: dominant field, top actors, top regions, risk level, and a short headline
- `scored_events`: normalized events with impact score, field attraction, divergence score, and rationale
- `intervention_candidates`: ranked backtracked actions derived from the top events

Persisted run history now exposes both compact run summaries and per-run drill-down over stored scored events and intervention candidates.

## Repository Layout

```text
.
├── tianji/
│   ├── cli.py
│   ├── pipeline.py
│   ├── fetch.py
│   ├── normalize.py
│   ├── scoring.py
│   ├── backtrack.py
│   └── models.py
├── tests/
│   ├── fixtures/sample_feed.xml
│   └── test_pipeline.py
├── pyproject.toml
└── README.md
```

## Implementation Notes

- **Language:** Python first, to keep the MVP small and fast to iterate
- **Style:** stdlib-first, deterministic where possible
- **Verification:** fixture-first tests plus fetch, Atom, mixed-input, config, and failure-path coverage
- **Current scope:** one-shot execution only; no daemon, scheduler, IPC bus, or web UI yet

## Long-Term Vision

The broader TianJi direction remains the same:

- **Cangjie (仓颉):** headless OSINT ingestion and retrieval
- **Hongmeng (鸿蒙):** daemonized orchestration and IPC
- **Fuxi (伏羲):** divergence modeling and strategic inference
- **Nuwa (女娲):** simulation and intervention sandboxing

But those are still future architecture targets. The repository is intentionally starting from a narrower vertical slice that can be tested end-to-end now.

## Local Reference Repositories

The workspace also includes four local reference projects that informed the design direction:

- `worldmonitor/` — one-shot ingestion and deterministic signal-scoring patterns
- `DivergenceMeter/` — conceptual divergence vocabulary such as `Im` and `Fa`
- `MiroFish/` — parsing and backtracking-oriented workflow ideas
- `oh-my-openagent/` — orchestration and tool-chain structure patterns

These are reference inputs, not part of the initial TianJi repo history.

## Roadmap

### Current

- one-shot CLI
- local fixture-first execution
- optional one-time live fetch
- config-driven source registry
- optional SQLite persistence
- SQLite-backed run history inspection
- richer `history-show` drill-down over stored scored events and interventions
- deterministic scoring and backtracking JSON report
- schema-versioned artifacts
- hardened input and fetch failure handling

Future local API contract now lives in `LOCAL_API_CONTRACT.md`; it is a draft contract only, not a shipped server.

### Next

- more formalized `Im` / `Fa`-style scoring model
- richer backtracking and causal grouping
- future local API implementation when a real local service boundary exists

### Later

- Hongmeng daemon and UNIX socket IPC
- scheduled ingestion
- local LLM-assisted inference as an optional layer
- constrained Nuwa replay / perturbation sandbox
- optional decoupled web UI

## Principles

1. **Local First** — the system should remain usable without external cloud dependencies.
2. **No Bloatware** — keep the core lean and understandable.
3. **Deterministic First** — inference should be inspectable before adding model-driven layers.
4. **CLI First** — prove the operator workflow before introducing services or UI.
