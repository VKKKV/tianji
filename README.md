# TianJi (天机)

TianJi is a geopolitical intelligence engine — ingest signals, compute divergence, generate intervention candidates, and track changes across runs. Deterministic by default. Daemon-ready. Single binary.

## Current State (2026-05-15)

Pure Rust project. 111 tests, zero failures. Single binary, no Python dependencies.

| Milestone | Status |
|-----------|--------|
| M1A Feed + Normalization (RSS/Atom, SHA-256 hash, keywords/actors/regions) | ✅ |
| M1B Scoring + Grouping + Backtrack (Im/Fa, divergence, intervention candidates) | ✅ |
| M2 Storage + History CLI (SQLite 6 tables, history/history-show/history-compare) | ✅ |
| M3A Daemon + Local API (UNIX socket, axum 5-route HTTP API, job queue) | ✅ |
| M3B Web UI (embedded static files, API proxy, /queue-run) | ✅ |
| M3C Daemon schedule (bounded repeated run queue) | ✅ |
| M4 TUI (ratatui history browser MVP, Kanagawa Dark, Vim keybindings) | ✅ |
| Crucix Delta Engine (cross-run change tracking, AlertTier FLASH/PRIORITY/ROUTINE, alert decay) | ✅ |

---

## Quick Start

```bash
git clone <repo> && cd tianji
cargo build

# One-shot analysis — zero config
cargo run -- run --fixture tests/fixtures/sample_feed.xml

# With persistence (enables history, delta, daemon)
cargo run -- run --fixture tests/fixtures/sample_feed.xml --sqlite-path runs/tianji.sqlite3

# Browse history
cargo run -- history --sqlite-path runs/tianji.sqlite3

# Terminal UI browser
cargo run -- tui --sqlite-path runs/tianji.sqlite3

# Start daemon + API
cargo run -- daemon start --sqlite-path runs/tianji.sqlite3
```

---

## One-Shot Analysis

No config files. No API keys. No LLM.

```bash
cargo run -- run --fixture tests/fixtures/sample_feed.xml
```

This runs the full pipeline on a local RSS/Atom XML file and prints a JSON artifact to stdout:

```
feed.xml → parse → normalize → score → group → backtrack → JSON
```

The output is a `RunArtifact` with:
- `scored_events`: every event ranked by impact, field attraction, divergence
- `scenario_summary`: dominant field, top actors, top regions, risk level, headline
- `intervention_candidates`: ranked interventions with type, target, reason, expected effect

Add `--sqlite-path runs/tianji.sqlite3` to persist the run for later history queries, comparisons, and delta tracking.

### Input format

TianJi ingests **RSS 2.0 and Atom 1.0 XML files**. The sample fixture is a three-event geopolitical feed:

```xml
<rss version="2.0"><channel>
  <item>
    <title>Iran nuclear talks resume in Vienna after cyber dispute</title>
    <link>https://example.com/iran-talks</link>
    <description>European mediators opened a new negotiation channel...</description>
  </item>
  <!-- ... -->
</channel></rss>
```

When run on a fixture, TianJi extracts:
- **Keywords**: tokenized from title + description
- **Actors**: nations and organizations (usa, china, nato, eu, iran, russia...)
- **Regions**: geographic areas (east-asia, middle-east, europe, united-states...)
- **Field scores**: conflict, diplomacy, economy, technology — keyword-weighted
- **Event IDs**: deterministic SHA-256 hash of source + title + link

### Output

Everything is JSON. One command, one output:

```bash
$ cargo run -- run --fixture tests/fixtures/sample_feed.xml
{
  "schema_version": "tianji.run-artifact.v1",
  "mode": "fixture",
  "generated_at": "1970-01-01T00:00:00+00:00",
  "scenario_summary": {
    "dominant_field": "technology",
    "risk_level": "high",
    "headline": "...",
    "top_actors": ["usa", "china", "iran"],
    "top_regions": ["east-asia", "united-states", "middle-east"]
  },
  "scored_events": [ ... ],
  "intervention_candidates": [ ... ]
}
```

**No OpenAI-compatible API is used or needed.** The Cangjie/Fuxi pipeline (feed → scoring → backtrack) is 100% deterministic rule-based. LLM integration is planned for the deferred Hongmeng (orchestration) and Nuwa (simulation) phases, where multi-agent game-theoretic reasoning will call external models. When that ships, the provider config will follow the YAML spec in `plan.md` §7:

```yaml
# ~/.tianji/config.yaml  (future, not yet implemented)
providers:
  openai_compatible:
    type: openai
    model: gpt-4o
    api_key_env: OPENAI_API_KEY       # reads from environment variable
    base_url: https://api.openai.com   # or any compatible endpoint
```

For now: **no LLM, no API key, no network calls**. Everything runs locally from fixture files.

---

## Full CLI Reference

All commands are top-level clap subcommands.

```
tianji run              One-shot pipeline on a fixture file
tianji history          List persisted runs with filters
tianji history-show     Show details for a single persisted run
tianji history-compare  Compare two persisted runs side-by-side
tianji delta            Show cross-run change tracking between two runs
tianji daemon           Daemon lifecycle and job queue
tianji webui            Serve the optional local web dashboard
tianji tui              Browse persisted runs in a read-only terminal UI
tianji completions      Generate shell completion scripts (bash/zsh/fish)
```

### `tianji run`

```
tianji run --fixture <PATH> [--sqlite-path <PATH>]
```

Examples:
```bash
# Stdout only, no persistence
tianji run --fixture tests/fixtures/sample_feed.xml

# Persist to SQLite for later history queries
tianji run --fixture tests/fixtures/sample_feed.xml --sqlite-path runs/tianji.sqlite3
```

### `tianji history`

List persisted runs. Supports rich filtering.

```
tianji history --sqlite-path <PATH>
    [--limit 20]
    [--mode fixture|fetch]
    [--dominant-field conflict|diplomacy|economy|technology]
    [--risk-level low|medium|high|critical]
    [--since <ISO>] [--until <ISO>]
    [--min-top-impact-score <f64>] [--max-top-impact-score <f64>]
    [--min-top-divergence-score <f64>] [--max-top-divergence-score <f64>]
    [--min-event-group-count <i64>] [--max-event-group-count <i64>]
```

Example:
```bash
tianji history --sqlite-path runs/tianji.sqlite3 --limit 5 --dominant-field technology --risk-level high
```

### `tianji history-show`

Show full detail for a single run.

```
tianji history-show --sqlite-path <PATH>
    [--run-id <i64> | --latest | --previous | --next]
    [--dominant-field <str>]
    [--min-impact-score <f64>] [--max-impact-score <f64>]
    [--limit-scored-events <usize>]
    [--only-matching-interventions]
    [--limit-event-groups <usize>]
```

Examples:
```bash
tianji history-show --sqlite-path runs/tianji.sqlite3 --latest
tianji history-show --sqlite-path runs/tianji.sqlite3 --run-id 42 --limit-scored-events 5
```

### `tianji history-compare`

Diff two runs side-by-side. Shows scored events with `comparable` field, intervention diffs, and count deltas.

```
tianji history-compare --sqlite-path <PATH>
    [--left-run-id <i64> --right-run-id <i64>]
    [--latest-pair | --run-id <i64> --against-latest | --run-id <i64> --against-previous]
    [same filters as history-show]
```

Examples:
```bash
tianji history-compare --sqlite-path runs/tianji.sqlite3 --latest-pair
tianji history-compare --sqlite-path runs/tianji.sqlite3 --run-id 5 --against-latest
```

### `tianji delta`

Cross-run change tracking. Shows what changed between two runs: numeric metric deltas (impact_score, divergence_score), count deltas (event counts, actor counts), new signals (new events, dominant field changes, new intervention candidates), and overall risk direction.

```
tianji delta --sqlite-path <PATH>
    [--left-run-id <i64> --right-run-id <i64> | --latest-pair]
```

Output includes:
- `alert_tier`: FLASH / PRIORITY / ROUTINE / null
- `delta.numeric_deltas`: per-metric % change with severity
- `delta.count_deltas`: per-metric absolute change with severity
- `delta.new_signals`: newly appeared events and interventions
- `delta.summary.direction`: RiskOff / RiskOn / Mixed

Example:
```bash
tianji delta --sqlite-path runs/tianji.sqlite3 --latest-pair
```

### `tianji daemon`

Daemon lifecycle: start a background process with UNIX socket control plane + loopback HTTP API.

```
tianji daemon start    [--socket-path <PATH>] [--sqlite-path <PATH>] [--host 127.0.0.1] [--port 8765]
tianji daemon stop     [--socket-path <PATH>]
tianji daemon status   [--socket-path <PATH>] [--job-id <ID>]
tianji daemon run      [--socket-path <PATH>] --fixture <PATH> [--sqlite-path <PATH>]
```

Examples:
```bash
# Start daemon
tianji daemon start --sqlite-path runs/tianji.sqlite3

# Queue a run
tianji daemon run --fixture tests/fixtures/sample_feed.xml --sqlite-path runs/tianji.sqlite3

# Check daemon health
tianji daemon status

# Check specific job
tianji daemon status --job-id <id-from-run-output>

# Stop
tianji daemon stop
```

The daemon exposes a read-first HTTP API at `http://127.0.0.1:8765`:

| Endpoint | Description |
|----------|-------------|
| `GET /api/v1/meta` | API metadata, source count, schema version |
| `GET /api/v1/runs?limit=20` | List persisted runs |
| `GET /api/v1/runs/latest` | Latest run summary |
| `GET /api/v1/runs/{run_id}` | Single run detail |
| `GET /api/v1/compare?left_run_id=1&right_run_id=2` | Compare two runs |
| `GET /api/v1/delta/latest` | Latest delta report from hot memory |

All responses use a JSON envelope: `{"api_version": "v1", "data": {...}, "error": null}`.

### `tianji webui`

Serve an optional web dashboard that consumes the daemon API.

```
tianji webui [--host 127.0.0.1] [--port 8766] [--api-base-url http://127.0.0.1:8765]
            [--socket-path runs/tianji.sock] [--sqlite-path <PATH>]
```

The Web UI provides a Jarvis-style HUD with run history, detail view, and a queue-run button. Requires the daemon to be running (`tianji daemon start`).

### `tianji tui`

Read-only terminal UI for browsing persisted runs. ratatui + Kanagawa Dark.

```
tianji tui --sqlite-path <PATH> [--limit 20]
```

Keybindings: `j/k` navigate, `g`/`G` first/last, `Ctrl-d`/`Ctrl-u` page scroll, `Enter` detail view, `q` quit.

### `tianji completions`

Generate shell completion scripts.

```
tianji completions <bash|zsh|fish>
```

Examples:
```bash
# Bash
tianji completions bash > ~/.local/share/bash-completion/completions/tianji

# Zsh
tianji completions zsh > ~/.zfunc/_tianji

# Fish
tianji completions fish > ~/.config/fish/completions/tianji.fish
```

---

## Daemon + Web UI Quick Setup

```bash
# 1. Persist some runs first
cargo run -- run --fixture tests/fixtures/sample_feed.xml --sqlite-path runs/tianji.sqlite3

# 2. Start daemon
cargo run -- daemon start --sqlite-path runs/tianji.sqlite3
# → Daemon runs in background. Logs at runs/tianji.sock.log.
# → API available at http://127.0.0.1:8765

# 3. Open Web UI
cargo run -- webui --sqlite-path runs/tianji.sqlite3
# → Dashboard at http://127.0.0.1:8766

# 4. Queue a run via daemon
cargo run -- daemon run --fixture tests/fixtures/sample_feed.xml --sqlite-path runs/tianji.sqlite3

# 5. Check delta
curl http://127.0.0.1:8765/api/v1/delta/latest

# 6. Stop
cargo run -- daemon stop
```

---

## What the Pipeline Does

```
RSS/Atom XML
  │  roxmltree parse + SHA-256 canonical hash
  ▼
Vec<RawItem>
  │  regex: keywords, actors, regions, field_scores
  │  actor patterns: nato, eu, un, usa, china, russia, iran
  │  region patterns: ukraine, russia, middle-east, east-asia, united-states, europe
  │  field keywords: conflict (attack, missile, troops...), diplomacy (talks, summit...),
  │    technology (ai, chip, cyber...), economy (tariff, trade, oil...)
  ▼
Vec<NormalizedEvent>
  │  Im = actor_weight + region_weight + keyword_density
  │       + dominant_field_bonus + field_diversity + text_signal_intensity
  │  Fa = dominant_field_strength + dominance_margin + coherence
  │       - near_tie_penalty - diffuse_third_field_penalty
  │  divergence_score = f(Im, Fa)
  ▼
Vec<ScoredEvent>
  │  shared keyword/actor/region + 24h time window
  │  causal ordering + evidence chain
  ▼
Vec<EventGroupSummary>
  │  dominant_field → intervention_type mapping
  │  strong groups (3+ members, 5+ shared signals) → escalation override
  │  weak groups (2+ members, 1+ link) → containment/stabilization
  ▼
Vec<InterventionCandidate>
  │  if --sqlite-path: persist to SQLite + compute delta vs previous run
  ▼
RunArtifact JSON (stdout) + optional SQLite persistence + optional DeltaReport
```

---

## Repository Layout

```
tianji/
├── Cargo.toml                  # 16 deps (see plan.md §7 for current vs target)
├── src/
│   ├── main.rs                 # CLI entry (9 subcommands)
│   ├── lib.rs                  # Pipeline + 111 integration tests
│   ├── models.rs               # RawItem → NormalizedEvent → ScoredEvent → RunArtifact
│   ├── fetch.rs                # RSS/Atom parsing + SHA-256 canonical hash
│   ├── normalize.rs            # Keyword/actor/region extraction (LazyLock regexes)
│   ├── scoring.rs              # Im/Fa scoring + divergence + rationale
│   ├── grouping.rs             # Event grouping + causal ordering
│   ├── backtrack.rs            # Intervention candidate generation (HeadlineRole enum)
│   ├── storage.rs              # SQLite 6 tables + history CRUD
│   ├── daemon.rs               # UNIX socket + job queue + serve + mark_delta_signals_alerted
│   ├── api.rs                  # axum 6-route HTTP API + response envelope
│   ├── webui.rs                # Embedded static files + API proxy + /queue-run
│   ├── tui.rs                  # ratatui history browser (Kanagawa Dark, Vim keys)
│   ├── delta.rs                # Crucix Delta Engine: compute_delta, MetricSnapshot, severity
│   ├── delta_memory.rs         # HotMemory, AlertDecayModel, AlertTier, atomic I/O
│   └── utils.rs                # round2, days_since_epoch, collect_string_array
├── tests/
│   └── fixtures/               # sample_feed.xml + contract fixtures
├── plan.md                     # Authoritative architecture document
└── README.md
```

---

## Design Principles

1. **Local First** — zero cloud dependencies. Everything runs from local XML fixtures.
2. **Deterministic First** — BTreeMap (not HashMap), LazyLock regex, no wall-clock in pipeline. Same input always produces same output.
3. **CLI First** — every feature ships as a CLI subcommand before any UI or service layer.
4. **Single Binary** — `cargo build` produces one binary. Web UI assets are `include_str!` embedded.
5. **No LLM required** — the current pipeline is 100% rule-based. LLM integration is planned for future multi-agent simulation phases only.
