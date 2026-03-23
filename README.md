# TianJi (天机)

TianJi is a local-first intelligence prototype aimed at one core loop: fetch signals, infer the current branch, and backtrack likely intervention points.

The repository currently ships a **one-shot CLI MVP**, not the full daemonized "God Engine" yet. The near-term goal is to prove the end-to-end value of `单次数据获取 -> 推演 -> 反推` with deterministic, inspectable output before expanding into a long-running orchestration system.

![TianJi concept diagram](img/Gemini_Generated_Image_h1wiykh1wiykh1wi.png)

_Concept illustration for TianJi's long-term direction. The currently shipped product surface is still the CLI-first MVP described below._

## Current Reality

The implemented slice is a local-first Python operator workflow under `tianji/`.

What ships now:

- one-shot `run` execution from fixtures or one-time fetches
- config-driven source selection from a JSON registry
- deterministic normalization, scoring, grouping, and backtracking
- schema-versioned JSON artifact output
- optional SQLite persistence through `tianji/storage.py`
- persisted run browsing with `history`, `history-show`, and `history-compare`
- read-time scored-event and event-group lenses that shape the operator view without mutating stored runs
- an early Rich-based `tui` command for read-only persisted list, detail, and compare browsing
- operator-facing validation for malformed feeds, bad filter windows, invalid compare presets, and missing relative-history targets

This keeps the shipped slice testable, local, and reproducible while leaving daemon, API, and web-runtime ideas clearly in the future bucket.

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
.venv/bin/python -m tianji run --fetch --source-config /path/to/sources.json --source-name example-feed
```

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

## What the Pipeline Does

The current MVP flow is:

1. **Fetch / Load**  
   Load RSS or Atom input from local fixture files, or fetch a feed once from a URL.

2. **Normalize**  
   Convert raw feed items into normalized events with extracted keywords, actors, regions, and field scores.

3. **Infer / Score**  
   Apply deterministic scoring rules to estimate event impact and field attraction, using weighted actors/regions, bounded title salience, bounded dominant-field impact scaling, field-alignment structure, and dominant-field text-signal intensity before ranking the most important events. The shipped rationale output is now richer and more inspectable, exposing additive `Im` and `Fa` terms directly so operators can see which bounded bonuses and penalties contributed without changing the current ranking formula.

4. **Backtrack**  
   Generate likely intervention targets and intervention types from the highest-ranked events.

5. **Emit Artifact**  
   Write a JSON report with input summary, scenario summary, scored events, and intervention candidates.

6. **Persist Run (Optional)**  
   Store run metadata plus raw, normalized, scored, and intervention rows in SQLite when `--sqlite-path` is provided.

7. **Inspect Run History (Optional)**  
   Query persisted run summaries later with `history`, optionally filter them by mode, dominant field, risk level, generated-at range, top scored-event `impact_score` / `field_attraction` / `divergence_score`, or grouped-analysis fields. Inspect one stored run with `history-show`, `history-show --latest`, `history-show --previous`, or `history-show --next`. `history-show` can also narrow visible scored events, event groups, and optionally aligned interventions, while stored run-summary fields remain the persisted truth.

8. **Compare Persisted Runs (Optional)**  
   Compare two stored runs with `history-compare`, compare the newest two stored runs with `history-compare --latest-pair`, compare one chosen run against the newest run with `history-compare --run-id N --against-latest`, or compare one chosen run against its immediate predecessor with `history-compare --run-id N --against-previous`. `history-compare` reuses the same read-time scored-event and event-group lenses as `history-show`, so compare-side projections can be focused without rewriting stored run data.

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

`history-show` now supports score-aware filtering and limiting over the selected run's persisted `scored_events`, using the same `impact_score` / `field_attraction` / `divergence_score` vocabulary as the stored scored-event details while leaving the run summary intact. By default the intervention list remains intact even if scored-event filters hide some events, but `--only-matching-interventions` can align intervention candidates to the final visible scored-event selection after both filters and limits. In other words, stored run-summary fields remain persisted truth, while `scored_events`, grouped-analysis slices, and optionally aligned interventions can be projected into a narrower operator lens. Inverted score windows and non-positive explicit `--run-id` values are rejected at parse time.

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
├── LOCAL_API_CONTRACT.md
├── TUI_CONTRACT.md
└── README.md
```

## Implementation Notes

- **Language:** Python first, to keep the MVP small and fast to iterate
- **Style:** stdlib-first, deterministic where possible
- **Verification:** fixture-first tests plus history list/detail/compare, TUI, grouping, scoring, fetch, Atom, mixed-input, config, and failure-path coverage
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

- Click-based CLI commands for `run`, `history`, `history-show`, `history-compare`, and `tui`
- local fixture-first execution plus optional one-time live fetch
- config-driven source registry
- optional SQLite persistence
- persisted history list, single-run detail, and run-compare read surfaces
- score-aware and group-aware read-time lenses over persisted runs
- deterministic scoring, grouped analysis, and backtracking JSON artifacts
- Rich-based read-only TUI for persisted list, detail, and compare browsing
- schema-versioned artifacts and hardened operator-facing validation

Future contract drafts live in `LOCAL_API_CONTRACT.md` and `TUI_CONTRACT.md`. The local API is still planning-only. The TUI draft now documents a read-only slice that already ships.

### Next

- keep scoring docs aligned with the shipped additive `Im` / `Fa` contract; no new `Fa` rule landed on this branch because the mixed-field no-gap review did not prove a meaningful uncovered weakness
- richer backtracking and causal grouping
- finish the Click-based CLI-first operator workflow for persisted analysis
- expand the Rich-based Vim-motion TUI from list/detail browsing toward fuller persisted operator workflows
- future local API implementation only when a real local service boundary is chosen

### Later

- Hongmeng daemon and UNIX socket IPC
- scheduled ingestion
- local LLM-assisted inference as an optional layer
- constrained Nuwa replay / perturbation sandbox
- optional decoupled web UI after CLI and TUI are mature

## Principles

1. **Local First** — the system should remain usable without external cloud dependencies.
2. **No Bloatware** — keep the core lean and understandable.
3. **Deterministic First** — inference should be inspectable before adding model-driven layers.
4. **CLI First** — prove the operator workflow before introducing services or UI.
