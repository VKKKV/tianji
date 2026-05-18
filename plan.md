# TianJi (天机) — Development Plan v5

> Branch: `main` | Updated: 2026-05-17
> Target: 智库级信号分析引擎 — 确定性管线 + 跨 run 变化追踪 + 多 Agent 仿真
> Current: ALL PHASES COMPLETE. 22 bugs fixed post-review. TianJi v0.2.1-post-bugfix.

---

## 1. Current State

```
M1A ████████████████████ ✅ Feed + Normalization
M1B ████████████████████ ✅ Scoring + Grouping + Backtrack
M2  ████████████████████ ✅ Storage + History CLI
M3A ████████████████████ ✅ Daemon + Local API
M3B ████████████████████ ✅ Optional Web UI
M3C ████████████████████ ✅ Bounded schedule
M3.5████████████████████ ✅ Crucix Delta Engine
TUI ████████████████████ ✅ 4 views + search + fallback + scroll
 2.1 ████████████████████ ✅ LLM Provider + Config
 2.2 ████████████████████ ✅ Worldline + Baseline
 2.3 ████████████████████ ✅ Actor Profiles (3 tiers, 8 YAML)
 2.4 ████████████████████ ✅ Hongmeng Orchestration
 2.5 ████████████████████ ✅ Nuwa Simulation Sandbox
 3.0 ████████████████████ ✅ Real LLM chat() via reqwest
 3.1 ████████████████████ ✅ CLI: predict/backtrack/baseline/watch
 3.2 ████████████████████ ✅ TUI Simulation view
 4.0 ████████████████████ ✅ Agent LLM integration (stub→real)
 4.1 ████████████████████ ✅ Rich TUI Dashboard
 4.2 ████████████████████ ✅ TUI Search/Filter
 4.3 ████████████████████ ✅ Nerd Font / ASCII fallback
 4.4 ████████████████████ ✅ Ctrl+d/u half-page scroll
 4.5 ████████████████████ ✅ TUI submodule split
 5.1 ████████████████████ ✅ Live feed watch e2e test
 5.2 ████████████████████ ✅ Worldline SQLite persistence
 5.3 ████████████████████ ✅ Human-in-the-loop pruning (TUI ↔ Sim)

Bugfix ████████████████████ ✅ 22 bugs (15C+7H), 4 commits, 2026-05-17
```

  源码: ~20K 行 Rust / 52 源文件
  测试: 296 pass / 0 fail
  构建: cargo build + clippy -D warnings zero
  依赖: 18 crates
  Python: 已退役

---

## 2. Architecture (Implemented)

```
RSS/Atom XML
  │  roxmltree parse + SHA-256 canonical hash
  ▼
Vec<RawItem>
  │  LazyLock regex: keywords, actors, regions, field_scores
  ▼
Vec<NormalizedEvent>
  │  Im = actor_weight + region_weight + keyword_density + ...
  │  Fa = dominant_field_strength + dominance_margin + ...
  │  divergence_score = f(Im, Fa)
  ▼
Vec<ScoredEvent>
  │  shared signals + 24h time window → event groups
  │  dominant_field → intervention candidates
  ▼
RunArtifact JSON ──────────────────────────► stdout
  │
  ├─ [optional] SQLite persist (6 tables) → history / compare
  │
  └─ [optional] Delta Engine
       ├─ compute_delta(prev, current) → DeltaReport
       ├─ HotMemory (hot/cold storage, atomic I/O)
       ├─ AlertDecayModel (0h/6h/12h/24h cooldown)
       └─ AlertTier: Flash / Priority / Routine
```

四子系统:
- Cangjie (仓颉): Feed → Normalize (已实现)
- Fuxi (伏羲): Scoring → Backtrack → Delta (已实现)
- Hongmeng (鸿蒙): Agent 编排层 (已实现, LLM stub+real)
- Nuwa (女娲): 仿真沙盒 (已实现, forward+backward+interactive)

---

## 3. Next Development Plan (v5)

After the bugfix blitz (22 bugs, 4 commits), the project needs polish, hardening,
and selective feature work. Ordered by priority.

### Phase A — Immediate Cleanup (low risk, high impact)

**A1. Input size limits** (H9 from review)
Files: `src/fetch.rs`, `src/lib.rs`
- `const MAX_RAW_ITEMS: usize = 500;` — truncate in parse_feed
- `const MAX_SCORED_EVENTS: usize = 500;` — enforce in pipeline

**A2. Remove deprecated delta_memory functions**
Files: `src/delta_memory.rs`
- Delete `is_signal_suppressed`, `mark_alerted`, `prune_stale_signals`
- All callers already use `_at` timestamp-persisted variants

**A3. Unify clean_text**
Files: `src/fetch.rs:161`, `src/normalize.rs:150`, `src/utils.rs`
- Two implementations with different whitespace semantics
- Merge into single `utils::clean_text` with `trim()` behavior

**A4. TianJiError::DataIntegrity variant**
Files: `src/lib.rs`, `src/storage.rs`
- Replace `rusqlite::Error::InvalidParameterName("missing canonical...")` hack
- Add proper error variant for data integrity failures

### Phase B — Code Quality (medium risk)

**B1. Extract time_utils module**
Files: NEW `src/time_utils.rs`, modify 4 files
- Consolidate ISO parsing (3 implementations), days_since_epoch (2 implementations)
- Standardize on Howard Hinnant's `days_from_civil` algorithm

**B2. C7: Async TUI data loading**
Files: `src/tui/mod.rs`, `src/tui/state.rs`
- Wrap SQLite queries in `tokio::task::spawn_blocking`
- Poll JoinHandle in main loop, display "Loading..." placeholder
- Only applies to detail/compare views (history already in-memory)

**B3. Structured logging**
Files: `src/daemon.rs`, `src/api.rs`, `src/webui.rs`, `src/main.rs`
- Replace `eprintln!` with `tracing` (already in deps)
- Spans for daemon's 3 components; `RUST_LOG` env var

**B4. Scoring parameters configurable**
Files: `src/scoring.rs`, NEW `src/scoring_params.rs`
- Extract 29 constants into `ScoreParams` struct with Default
- YAML deserialization for per-environment tuning

### Phase C — Architecture (medium-high risk)

**C1. H8: serde_json::Value → strong types (incremental)**
Files: `src/models.rs`, `src/lib.rs`, `src/delta.rs`, `src/delta_memory.rs`, `src/storage.rs`
- Phase 1: Add typed fields alongside Value (backward compat)
- Phase 2: Migrate delta computation to typed
- Phase 3: Remove Value fields

**C2. TUI view state decoupling**
Files: `src/tui/state.rs`, `src/tui/mod.rs`
- Replace monolithic `TuiState` with `enum ViewState`
- Move search/prune into separate modules
- Simplify `handle_key` via view dispatch

**C3. forward.rs deduplication**
Files: `src/nuwa/forward.rs`
- `run_forward` and `run_interactive_forward` share ~80% of loop
- Extract `tick_simulation` core function

**C4. fork_worldline unification**
Files: `src/nuwa/sandbox.rs`, `src/nuwa/forward.rs`, `src/nuwa/backward.rs`
- All worldline branching through `sandbox::fork_worldline`
- Decouple from `rusqlite::Connection` via `WorldlineStore` trait

### Phase D — Production & Features (lower priority)

**D1. Integration test coverage**
- persist_run → get_run_summary → compare_runs full flow
- In-memory SQLite, parallel to existing worldline persistence tests

**D2. ActorProfile YAML validation**
- `validate()` method: salience ∈ [0,1], Capabilities ∈ [0,1]
- Call in `ProfileRegistry::load_yaml_files_from_dir`

**D3. SQLite connection pool** — `r2d2-sqlite` replacing per-request `Connection::open`

**D4. Ollama /api/chat migration** — structured messages instead of plain-text /api/generate

**D5. LLM concurrency limiting** — implement `max_concurrency` with `tokio::sync::Semaphore`

**D6. Worldline causal_graph serialization** — custom serde for DiGraph

---

## 3.5 Cross-Project Borrowings (ShadowBroker v0.9.7 → TianJi)

> Full analysis: `.trellis/reviews/shadowbroker-cross-project-analysis.md`
> Repo: `/home/kita/code/Shadowbroker`

ShadowBroker is a 60+ feed real-time OSINT geospatial dashboard
(Next.js + MapLibre + FastAPI + Rust privacy-core). Six patterns
are worth adapting for TianJi, ordered by priority:

### Borrowing 1 (high): Alert Dispatch to External Channels
ShadowBroker's `AlertDispatcher` sends branded alerts to Discord
webhooks, Telegram bots, and generic webhooks — with automatic
message chunking for platform character limits.

**TianJi adoption:** Wire `AlertTier` (Flash/Priority/Routine) to
real delivery channels. New file `src/alert_dispatch.rs`, config
in `~/.tianji/config.yaml`. Reuses existing `reqwest` dep.

### Borrowing 2 (medium): HMAC-Signed Agent Command Channel
ShadowBroker exposes `POST /api/ai/channel/command` and `/batch`
endpoints with HMAC-SHA256 signing (timestamp + nonce + body digest),
tier-gated access (restricted/full), and SSE push for layer changes.

**TianJi adoption:** Add `/api/v1/agent/command` to daemon's axum
router, enabling external AI agents (Hermes, OpenClaw, etc.) to
participate in Hongmeng simulations as first-class actors.

### Borrowing 3 (medium): Typed Data Model (H8 fix reference)
ShadowBroker's frontend has ~1100 lines of TypeScript interfaces
covering every entity (Flight, Ship, Satellite, Earthquake, GDELT, ...).
Every field is typed. No `any` overuse.

**TianJi adoption:** This is the reference pattern for fixing H8
(`serde_json::Value` → strong types). Define `#[derive(Serialize,
Deserialize)]` structs for ScoredEvent, InterventionCandidate,
EventGroupSummary, etc. Phase C1 in this plan.

### Borrowing 4 (medium): Fast/Slow Feed Tier Separation
ShadowBroker splits 60+ feeds: fast tier (15-30s: flights, ships,
satellites) vs slow tier (5-15min: GDELT, news, earthquakes, fires).

**TianJi adoption:** Extend `DaemonConfig` with `fast_interval_secs`
and `slow_interval_secs`. Group watched feeds by urgency. Lowers
API costs and LLM token consumption in Hongmeng simulations.

### Borrowing 5 (low): Structured Agent Output (Analysis Zones)
ShadowBroker agents place analysis zones on the map with category
(contradiction/warning/observation/hypothesis), severity, cell size,
and evidence drivers.

**TianJi adoption:** Enrich `AgentAction` with structured rationale:
`{assessment, category, confidence, drivers[]}`. Makes Nuwa
simulation paths auditable and the TUI simulation view richer.

### Borrowing 6 (low): Snapshot Timeline Replay
ShadowBroker's Time Machine captures hourly snapshots with frame
interpolation for moving entities, variable playback speed.

**TianJi adoption:** Extend TUI history mode with arrow-key timeline
scrubbing through persisted runs, replaying field changes tick-by-tick.

---

## 4. Dependencies

```toml
[dependencies]
clap = { version = "4.6", features = ["derive"] }
serde = { version = "1.0", features = ["derive"] }
serde_json = "1.0"
serde_yaml = "0.9"
roxmltree = "0.21"
regex = "1.12"
sha2 = "0.11"
blake3 = "1"
rusqlite = { version = "0.39", features = ["bundled"] }
reqwest = { version = "0.13", default-features = false, features = ["rustls", "blocking"] }
axum = "0.8"
uuid = { version = "1", features = ["v4"] }
tokio = { version = "1", features = ["full"] }
libc = "0.2"
ratatui = "0.30"
crossterm = "0.28"
chrono = { version = "0.4", features = ["serde"] }
petgraph = { version = "0.7", features = ["serde-1"] }
anyhow = "1"
thiserror = "2"
tracing = "0.1"
tracing-subscriber = "0.3"
clap_complete = "4"

[profile.release]
opt-level = 3
lto = true
```

---

## 5. Verification Criteria

Each phase must pass:
- `cargo build` / `cargo build --release` zero error
- `cargo test` all green (currently 296)
- `cargo clippy -- -D warnings` zero warning
- `tianji run --fixture ...` output field-level consistent with contracts
- `tianji delta --latest-pair` cross-run change tracking functional
- Single binary < 25MB release
