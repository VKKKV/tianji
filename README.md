# TianJi (天机)

TianJi is a local-first intelligence prototype aimed at one core loop: fetch signals, infer the current branch, and backtrack likely intervention points.

The repository now ships a mature local stack with clear boundaries: the CLI remains the write authority, the TUI is read-only and storage-backed, the daemon and HTTP API stay local-only and loopback-bound, and the web UI is optional and off by default. The long-term "God Engine" vision is still broader than what ships here, but the current local operator stack is real, tested, and runnable now.

![TianJi concept diagram](img/Gemini_Generated_Image_h1wiykh1wiykh1wi.png)

_Concept illustration for TianJi's long-term direction. The currently shipped product surface is the terminal-first local stack described below._

## Current Reality

The implemented slice is a local-first Python operator workflow under `tianji/`.

What ships now:

- one-shot `run` execution from fixtures or one-time fetches
- config-driven source selection from a JSON registry
- deterministic normalization, scoring, grouping, and backtracking
- schema-versioned JSON artifact output
- optional SQLite persistence through `tianji/storage.py`
- persisted run browsing with `history`, `history-show`, and `history-compare`
- thin daemon CLI controls for local `start`, `status`, `stop`, bounded queued `run`, and bounded `schedule`
- read-first loopback HTTP API routes for local `meta`, persisted `runs`, run detail, latest-run detail, and explicit-pair compare reads
- read-time scored-event and event-group lenses that shape the operator view without mutating stored runs
- an early Rich-based `tui` command for read-only persisted list, detail, and compare browsing
- an optional local web UI served separately from `tianji/webui_server.py`, with plain static HTML/CSS/JS over the same loopback API for run history, detail, compare, and intervention browsing
- operator-facing validation for malformed feeds, bad filter windows, invalid compare presets, and missing relative-history targets

This keeps the shipped slice testable, local, and reproducible while keeping write-triggering HTTP endpoints and broader web-runtime ideas clearly outside the current boundary.

The synchronous `run` command remains the source-of-truth write path for one immediate pipeline invocation. The `daemon` subcommands are a separate local control surface over the Task 11 UNIX socket backend: `daemon run` queues one pipeline unit for background execution, `daemon schedule` queues that same one-run pipeline unit repeatedly with a bounded local `--every-seconds` + `--count` contract, and `daemon status` reports either process-level daemon availability or one queued job's lifecycle state. The currently documented job lifecycle states are exactly `queued`, `running`, `succeeded`, and `failed`.

The local API slice is intentionally read-first and loopback-only. Inside the existing local daemon/server boundary, TianJi now serves `GET /api/v1/meta`, `GET /api/v1/runs`, `GET /api/v1/runs/{run_id}`, optional `GET /api/v1/runs/latest`, and `GET /api/v1/compare?left_run_id=<id>&right_run_id=<id>`, all under the stable JSON envelope `api_version` / `data` / `error` while reusing the existing storage payload vocabulary directly. The shipped startup path is `python -m tianji daemon start --sqlite-path runs/tianji.sqlite3 --socket-path runs/tianji.sock --host 127.0.0.1 --port 8765`, which keeps the UNIX socket as the control plane and exposes the read API at `http://127.0.0.1:8765/api/v1/...`.

## Quick Start

### Create the local uv environment

```bash
uv venv .venv
```

All commands below use the repo-local environment directly via `.venv/bin/python`.

### Run from fixture

```bash
.venv/bin/python -m tianji run --fixture tests/fixtures/sample_feed.xml
```

### Run with output file

```bash
.venv/bin/python -m tianji run --fixture tests/fixtures/sample_feed.xml --output runs/latest-run.json
```

### Run with one-time fetch

```bash
.venv/bin/python -m tianji run --fetch --source-url https://example.com/feed.xml
```

### Run with optional SQLite persistence

```bash
.venv/bin/python -m tianji run --fixture tests/fixtures/sample_feed.xml --sqlite-path runs/tianji.sqlite3
```

### Run with a source registry

Create a JSON file shaped like:

```json
{
  "default_fetch_policy": "if-missing",
  "sources": [
    {
      "name": "example-feed",
      "url": "https://example.com/feed.xml"
    },
    {
      "name": "priority-feed",
      "url": "https://example.com/priority.xml",
      "fetch_policy": "if-changed"
    }
  ]
}
```

`default_fetch_policy` and per-source `fetch_policy` use the same bounded vocabulary: `always`, `if-missing`, and `if-changed`. `tianji/cli.py` resolves that policy in this order for one run: CLI `--fetch-policy` override, then per-source `fetch_policy`, then config-level `default_fetch_policy`, with ad hoc `--source-url` inputs defaulting to `always` unless the CLI override is present.

Then run:

```bash
.venv/bin/python -m tianji run --fetch --source-config /path/to/sources.json --source-name example-feed
.venv/bin/python -m tianji run --fetch --source-config /path/to/sources.json --source-name priority-feed --fetch-policy if-changed
```

This operator contract is now part of the shipped Phase 3 persistence model. The fetch-policy vocabulary stays intentionally bounded to `always`, `if-missing`, and `if-changed`, and that same vocabulary is what persisted runs record in `input_summary` for each one-shot invocation. Persistence reuse happens at canonical source-item storage, not by suppressing run creation: each successful invocation still creates one `runs` row, while identical canonical content can reuse existing `source_items` rows underneath that run-centric history surface.

### Inspect persisted run history

```bash
.venv/bin/python -m tianji history --sqlite-path runs/tianji.sqlite3
.venv/bin/python -m tianji history --sqlite-path runs/tianji.sqlite3 --dominant-field technology --risk-level high
.venv/bin/python -m tianji history --sqlite-path runs/tianji.sqlite3 --since 2026-03-22T10:00:00+00:00 --until 2026-03-22T12:00:00+00:00
.venv/bin/python -m tianji history --sqlite-path runs/tianji.sqlite3 --min-top-divergence-score 18 --min-top-impact-score 10
.venv/bin/python -m tianji history --sqlite-path runs/tianji.sqlite3 --top-group-dominant-field technology --min-event-group-count 1
.venv/bin/python -m tianji history --sqlite-path runs/tianji.sqlite3 --max-event-group-count 0
.venv/bin/python -m tianji history-show --sqlite-path runs/tianji.sqlite3 --run-id 1
.venv/bin/python -m tianji history-show --sqlite-path runs/tianji.sqlite3 --run-id 1 --min-divergence-score 15 --limit-scored-events 2
.venv/bin/python -m tianji history-show --sqlite-path runs/tianji.sqlite3 --run-id 1 --group-dominant-field diplomacy --limit-event-groups 1
.venv/bin/python -m tianji history-show --sqlite-path runs/tianji.sqlite3 --latest
.venv/bin/python -m tianji history-show --sqlite-path runs/tianji.sqlite3 --run-id 3 --previous
.venv/bin/python -m tianji history-show --sqlite-path runs/tianji.sqlite3 --run-id 1 --next
.venv/bin/python -m tianji history-compare --sqlite-path runs/tianji.sqlite3 --left-run-id 1 --right-run-id 2
.venv/bin/python -m tianji history-compare --sqlite-path runs/tianji.sqlite3 --left-run-id 1 --right-run-id 2 --dominant-field diplomacy --group-dominant-field diplomacy --limit-scored-events 1 --limit-event-groups 1
.venv/bin/python -m tianji history-compare --sqlite-path runs/tianji.sqlite3 --latest-pair
.venv/bin/python -m tianji history-compare --sqlite-path runs/tianji.sqlite3 --run-id 3 --against-latest
.venv/bin/python -m tianji history-compare --sqlite-path runs/tianji.sqlite3 --run-id 3 --against-previous
```

### Run tests

```bash
.venv/bin/python -m unittest discover -s tests -v
```

### Control the local daemon

```bash
.venv/bin/python -m tianji daemon start
.venv/bin/python -m tianji daemon status --socket-path runs/tianji.sock
.venv/bin/python -m tianji daemon run --socket-path runs/tianji.sock --fixture tests/fixtures/sample_feed.xml
.venv/bin/python -m tianji daemon schedule --socket-path runs/tianji.sock --every-seconds 300 --count 3 --fixture tests/fixtures/sample_feed.xml
.venv/bin/python -m tianji daemon stop --socket-path runs/tianji.sock
```

This daemon surface stays intentionally narrow. `daemon start` now also hosts the read-only loopback HTTP API using the same process, with default startup values `--sqlite-path runs/tianji.sqlite3 --socket-path runs/tianji.sock --host 127.0.0.1 --port 8765`. `daemon run` and `daemon schedule` still submit the same one-run pipeline work unit that synchronous `run` executes directly; they do not introduce a second artifact vocabulary, a new persistence model, or cron-style calendar recurrence. The default daemon socket path is `runs/tianji.sock`, the default API port is `8765`, and interval scheduling is intentionally bounded to `--every-seconds N` where `N >= 60`.

### Inspect the loopback HTTP API

Start the daemon first, because the local API is hosted inside that process:

```bash
.venv/bin/python -m tianji daemon start --sqlite-path runs/tianji.sqlite3 --socket-path runs/tianji.sock --host 127.0.0.1 --port 8765
curl http://127.0.0.1:8765/api/v1/meta
curl http://127.0.0.1:8765/api/v1/runs
curl http://127.0.0.1:8765/api/v1/runs/latest
curl "http://127.0.0.1:8765/api/v1/compare?left_run_id=1&right_run_id=2"
```

The HTTP API is read-first and loopback-only. It exists to expose persisted metadata and run history for local readers. It is not the write authority for starting new runs or mutating stored state.

### Start the optional web UI

```bash
.venv/bin/python -m tianji.webui_server --api-base-url http://127.0.0.1:8765 --host 127.0.0.1 --port 8766
```

Then open `http://127.0.0.1:8766/` in a browser. The web UI is optional, separate from the daemon, and off by default. It reuses the same local API payload vocabulary and stays a convenience read surface rather than a second source of truth.

## What the Pipeline Does

The current MVP flow is:

1. **Fetch / Load**  
   Load RSS or Atom input from local fixture files, or fetch a feed once from a URL. When `--fetch` is used, the operator surface now carries a bounded fetch policy contract (`always`, `if-missing`, `if-changed`) from CLI/config resolution into the pipeline boundary so repeated local runs can later reuse the same vocabulary.

2. **Normalize**  
   Convert raw feed items into normalized events with extracted keywords, actors, regions, and field scores.

3. **Infer / Score**  
   Apply deterministic scoring rules to estimate event impact and field attraction, using weighted actors/regions, bounded title salience, bounded dominant-field impact scaling, field-alignment structure, and dominant-field text-signal intensity before ranking the most important events. The shipped rationale output is now richer and more inspectable, exposing additive `Im` and `Fa` terms directly so operators can see which bounded bonuses and penalties contributed without changing the current ranking formula.

4. **Backtrack**  
   Generate likely intervention targets and intervention types from the highest-ranked events.

5. **Emit Artifact**  
   Write a JSON report with input summary, scenario summary, scored events, and intervention candidates.

6. **Persist Run (Optional)**  
   Store run metadata plus raw, normalized, scored, and intervention rows in SQLite when `--sqlite-path` is provided. Each successful one-shot invocation creates exactly one persisted `runs` row. Persistence remains run-centric for history/detail/compare reads, but source-item storage is now content-addressed underneath that read model: `entry_identity_hash` identifies the same logical feed entry across runs, `content_hash` identifies the canonicalized content body stored for that entry, replayed identical entries reuse the same canonical stored content in `source_items` while still creating a fresh `runs` row, and changed content under the same identity creates a new canonical content row while preserving both runs.

7. **Inspect Run History (Optional)**  
   Query persisted run summaries later with `history`, optionally filter them by mode, dominant field, risk level, generated-at range, top scored-event `impact_score` / `field_attraction` / `divergence_score`, or grouped-analysis fields. Inspect one stored run with `history-show --run-id N`, jump to the newest persisted run with `history-show --latest`, move to an immediate predecessor with `history-show --run-id N --previous`, or move to an immediate successor with `history-show --run-id N --next`. `history-show` can also narrow visible scored events, event groups, and optionally aligned interventions, while stored run-summary fields remain the persisted truth.

8. **Compare Persisted Runs (Optional)**  
   Compare two stored runs with `history-compare`, compare the newest two stored runs with `history-compare --latest-pair`, compare one chosen run against the newest run with `history-compare --run-id N --against-latest`, or compare one chosen run against its immediate predecessor with `history-compare --run-id N --against-previous`. `history-compare` reuses the same read-time scored-event and event-group lenses as `history-show`, so compare-side projections can be focused without rewriting stored run data.

9. **Queue Background Runs (Optional)**  
   Start the local daemon with `daemon start`, inspect process or job status with `daemon status`, queue one background pipeline unit with `daemon run`, or queue a bounded repeated set of those same one-run pipeline units with `daemon schedule --every-seconds N --count M`. Queued jobs move through exactly four lifecycle states: `queued`, `running`, `succeeded`, and `failed`.

## Output Artifact

By default, the CLI writes to `runs/latest-run.json`.

The artifact includes:

- `schema_version`: stable top-level artifact contract version
- `input_summary`: item counts and source list
- `scenario_summary`: dominant field, top actors, top regions, risk level, and a short headline; when dominant-field counts tie across scored events, the stored summary resolves the winner deterministically by strongest tied `divergence_score`, using field-name order only as the final fallback
- `scored_events`: normalized events with impact score, field attraction, divergence score, and rationale
- `intervention_candidates`: ranked backtracked actions derived from the top events

Current scored-event rationale is shipped as an additive, explicit, and inspectable contract. It always exposes top-level `Im` / `Fa` values, keeps the current `divergence_score` blend unchanged, and now surfaces richer bounded rationale terms for the shipped `Im` and `Fa` components. That includes fixed additive `Im` terms such as actor weight, region weight, keyword density, dominant-field bonus, and nonzero-field bonus, conditional `Im` terms such as title salience and dominant-field impact scaling when they contribute, the shipped text-signal-intensity term for dominant-field cue concentration, and additive or subtractive `Fa` terms such as dominant-field strength, dominance-margin bonus, coherence bonus, near-tie penalty, and diffuse-third-field penalty.

Persisted run history now exposes both compact run summaries and per-run drill-down over stored scored events, grouped analysis, and intervention candidates.

History list items now also expose the persisted run's top scored-event identity plus its `impact_score`, `field_attraction`, and `divergence_score`, so operators can query stored runs by the strongest scored branch signal without opening each run individually. These score filters operate on that single persisted top scored event for each run, and runs with no scored events expose `null` top metrics that do not satisfy numeric thresholds. Negative `--limit` values and inverted score windows such as `--min-top-impact-score` greater than `--max-top-impact-score` are rejected at parse time.

History list items now also expose grouped-run triage fields such as `event_group_count`, `top_event_group_headline_event_id`, `top_event_group_dominant_field`, and `top_event_group_member_count`, so operators can query stored runs by whether grouped scenarios emerged at all and what kind of top group led the run. Runs with no event groups report `event_group_count=0` and `null` top-group fields.

`history-show` now supports score-aware filtering and limiting over the selected run's persisted `scored_events`, using the same `impact_score` / `field_attraction` / `divergence_score` vocabulary as the stored scored-event details while leaving the run summary intact. By default the intervention list remains intact even if scored-event filters hide some events, but `--only-matching-interventions` (storage field: `only_matching_interventions`) can align intervention candidates to the final visible scored-event selection after both filters and limits. In other words, stored run-summary fields remain persisted truth, while `scored_events`, grouped-analysis slices, and optionally aligned interventions can be projected into a narrower operator lens. Inverted score windows and non-positive explicit `--run-id` values are rejected at parse time.

`history-show` now also supports group-aware drill-down over persisted `scenario_summary.event_groups` via `--group-dominant-field` and `--limit-event-groups`, so single-run grouped analysis can be narrowed without changing the stored scenario summary itself.

`history-compare` now supports those same scored-event and event-group projections on both compared runs, including optional intervention alignment to the final visible scored-event set, so compare output can stay focused on one operator lens instead of always diffing the full stored run payload. Those projections affect compare-side projected fields such as `top_scored_event`, `intervention_event_ids`, `event_group_count`, and `top_event_group`, but stored run-summary fields like `headline`, `dominant_field`, and `risk_level` still describe the persisted run itself rather than the filtered lens.

`history-compare` preset selection is intentionally exclusive: use exactly one of explicit `--left-run-id/--right-run-id`, `--latest-pair`, `--run-id ... --against-latest`, or `--run-id ... --against-previous`. Mixed preset combinations, non-positive explicit run ids, negative per-side compare limits, and inverted compare score windows are rejected at parse time so the compare surface stays unambiguous.

Persisted comparison currently exposes left/right run summaries plus explicit diff fields for counts, dominant-field/risk changes, top/intervention deltas, top scored-event score deltas, and grouped-analysis deltas such as event-group count, top-group identity changes, and top-group evidence/member/link changes. Group detail now stays nested under each side's `top_event_group`, while `top_event_group_evidence_diff` carries the operator-facing evidence/member/link comparison instead of duplicating those fields in flattened side-level keys.

Within `top_event_group_evidence_diff`, `comparable=true` now means both runs kept the same top-group `headline_event_id`, so the evidence/member/link deltas describe one persisted grouped scenario evolving over time rather than two different top groups being contrasted. The diff fields are still present when `comparable=false`, but in that case they should be read as a contrast between different top groups, not as one group changing over time.

The same distinction now applies to top scored-event score deltas: `top_scored_event_comparable=true` means both runs kept the same top scored `event_id`, so the top-score deltas describe one persisted leading signal evolving over time. When `top_scored_event_comparable=false`, the score deltas are still present, but they should be read as contrast between different top events rather than the same event changing across runs.

Grouped event summaries now also carry lightweight evidence-chain metadata so intervention reasons can cite why multiple related events were collapsed into one operator-facing action.

Grouped event summaries now also carry additive causal-cluster metadata such as `causal_ordered_event_ids`, `causal_span_hours`, and `causal_summary`, so operators can distinguish simple shared-signal overlap from a linked reinforcing chain. That causal order reflects the group admission path, not necessarily strict timestamp order. `causal_span_hours` is computed from the earliest and latest known timestamps inside the group when at least two timestamps are available; otherwise it stays `null`, and the summary text falls back to non-span wording.

Scoring-contract coverage now also includes isolated `Im` checks for actor weighting, region weighting, actor/region title-salience behavior, raw keyword-density cap behavior, dominant-field-strength bonus behavior, dominant-field-specific impact scaling, nonzero-field-count bonus behavior, direct keyword/title/summary text-signal surface contributions, and isolated `Fa` checks for dominance-margin and coherence behavior, so future scoring changes can be caught at the additive-term level instead of only through full aggregate snapshots.

## Repository Layout

```text
.
├── tianji/
│   ├── __main__.py
│   ├── cli.py
│   ├── pipeline.py
│   ├── fetch.py
│   ├── normalize.py
│   ├── scoring.py
│   ├── backtrack.py
│   ├── storage.py
│   ├── tui.py
│   └── models.py
├── tests/
│   ├── fixtures/sample_feed.xml
│   ├── test_pipeline.py
│   ├── test_tui.py
│   ├── test_history_list.py
│   ├── test_history_show.py
│   ├── test_history_compare.py
│   ├── test_scoring.py
│   ├── test_grouping.py
│   ├── test_cli_inputs.py
│   └── support.py
├── pyproject.toml
├── DAEMON_CONTRACT.md
├── LOCAL_API_CONTRACT.md
├── TUI_CONTRACT.md
├── WEB_UI_CONTRACT.md
└── README.md
```

## Implementation Notes

- **Language:** Python first, to keep the MVP small and fast to iterate
- **Style:** stdlib-first, deterministic where possible
- **Verification:** fixture-first tests plus history list/detail/compare, TUI, grouping, scoring, fetch, Atom, mixed-input, config, and failure-path coverage
- **Current scope:** one-shot execution plus a thin local daemon CLI wrapper for bounded queueing and status, a read-first loopback HTTP API hosted by that daemon, and an optional separate web UI process for local browsing and queue proxying

## Long-Term Vision

The broader TianJi direction remains the same:

- **Cangjie (仓颉):** headless OSINT ingestion and retrieval
- **Hongmeng (鸿蒙):** daemonized orchestration and IPC
- **Fuxi (伏羲):** divergence modeling and strategic inference
- **Nuwa (女娲):** simulation and intervention sandboxing

But those are still future architecture targets. The repository is intentionally starting from a narrower vertical slice that can be tested end-to-end now.

## Upstream Inspiration

TianJi no longer carries local vendored copies of the projects that helped shape the early roadmap. Historical inspiration still comes from earlier upstream work on ingestion and signal ranking, divergence vocabulary like `Im` and `Fa`, parsing and workflow decomposition, and orchestration and tool-boundary patterns.

Those projects are now citation-level context only. TianJi's shipped code, tests, and docs stand on first-party modules in this repository.

## Roadmap

### Current

- Click-based CLI commands for synchronous `run`, persisted history reads, TUI access, and thin local `daemon` controls
- local fixture-first execution plus optional one-time live fetch
- config-driven source registry
- bounded fetch policy semantics for source-registry defaults, per-source overrides, and one-run CLI override
- optional SQLite persistence
- persisted history list, single-run detail, and run-compare read surfaces
- score-aware and group-aware read-time lenses over persisted runs
- deterministic scoring, grouped analysis, and backtracking JSON artifacts
- Rich-based read-only TUI for persisted list, detail, and compare browsing
- daemon-hosted loopback HTTP API at `127.0.0.1:8765` with read-first `meta`, `runs`, `runs/latest`, run-detail, and explicit compare routes
- optional separate web UI at `127.0.0.1:8766`, off by default, reusing the same local API payloads
- schema-versioned artifacts and hardened operator-facing validation

`LOCAL_API_CONTRACT.md`, `DAEMON_CONTRACT.md`, `TUI_CONTRACT.md`, and `WEB_UI_CONTRACT.md` document the shipped mature local stack. The boundary remains strict: CLI writes are authoritative, the TUI is storage-backed and read-only, the daemon and API stay loopback-only, and the web UI remains optional.

### Next

- keep scoring docs aligned with the shipped additive `Im` / `Fa` contract; no new `Fa` rule landed on this branch because the mixed-field no-gap review did not prove a meaningful uncovered weakness
- richer backtracking and causal grouping
- tighten the remaining CLI/docs wording so projected compare fields versus stored run-summary fields are explained in one place without ambiguity
- expand the Rich-based Vim-motion TUI through the remaining Phase 5 slice: treat the already-shipped shared detail/compare lens controls as current behavior, then focus on stronger persisted-navigation parity, input/render separation, and higher-fidelity verification while keeping the list pane on persisted truth
- keep numeric threshold entry and list-pane filtering out of that next TUI slice for now
- keep the local API read-first and loopback-only until a broader local service boundary is justified
- keep daemon scheduling bounded to repeated submission of the same one-run pipeline unit; no cron/calendar expansion in this slice

### Later

- broader Hongmeng daemon/runtime work beyond the current thin CLI wrapper
- richer scheduled ingestion beyond the current bounded repeated queue submission
- local LLM-assisted inference as an optional layer
- constrained Nuwa replay / perturbation sandbox
- optional queued-run browser controls only if they remain behind the separate local web UI proxy rather than widening `/api/v1/*`

## Principles

1. **Local First** — the system should remain usable without external cloud dependencies.
2. **No Bloatware** — keep the core lean and understandable.
3. **Deterministic First** — inference should be inspectable before adding model-driven layers.
4. **CLI First** — prove the operator workflow before introducing services or UI.
