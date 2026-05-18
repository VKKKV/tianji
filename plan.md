# TianJi (天机) — Development Plan v5

> Branch: `main` | Updated: 2026-05-18
> Target: 智库级信号分析引擎 — 确定性管线 + 跨 run 变化追踪 + 多 Agent 仿真
> Current: Core product complete. Phase A/B/C hardening complete; Phase D1 coverage complete. Next: selective production features.
> Tests: 321 pass / 0 fail

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

  源码: 21,722 行 Rust / 55 源文件
  测试: 321 pass / 0 fail
  构建: cargo build + clippy -D warnings zero
  依赖: 23 manifest dependencies
  Python: 已退役

### Phase A — Immediate Cleanup (COMPLETE)

**A1. Input size limits** ✅
- `MAX_RAW_ITEMS` and `MAX_SCORED_EVENTS` cap oversized feeds/pipeline output.

**A2. Remove deprecated delta_memory functions** ✅
- Timestamp-injected `_at` variants are the only public alert suppression/marking path.

**A3. Unify clean_text** ✅
- `fetch` and `normalize` share `utils::clean_text` with trim/collapse semantics.

**A4. TianJiError::DataIntegrity variant** ✅
- Storage/worldline integrity failures now use a first-class error variant.

### Phase B — Code Quality (COMPLETE)

**B1. Extract time_utils module** ✅
- Consolidated ISO/RFC timestamp parsing and days-since-epoch helpers into `src/time_utils.rs`.

**B2. Async TUI data loading** ✅
- Background threads for detail/compare SQLite queries.
- mpsc channel polling in event loop, "loading..." indicator in title bar.

**B3. Structured logging** ✅
- All `eprintln!` calls replaced with `tracing::{error,warn}`.
- `tracing_subscriber::fmt` with `RUST_LOG` env-var support.

**B4. Scoring parameters configurable** ✅
- `src/scoring_params.rs`: `ScoreParams` with `Default` + YAML deserialization.
- All scoring functions threaded with `&ScoreParams`.
- Backward-compat `score_events()` uses default params.

### Phase C — Architecture (COMPLETE)

**C1. H8: serde_json::Value → strong types** ✅
- Hongmeng agent private state and board stick values use typed Rust structures.
- Legacy JSON compatibility remains at API/prompt boundaries.

**C2. TUI view state decoupling** ✅
- View state preserved through dispatch; TUI state no longer relies on one monolithic mode bag.

**C3. forward.rs deduplication** ✅
- Shared Nuwa forward tick logic extracted into `tick_simulation`.

**C4. fork_worldline unification** ✅
- Worldline branching flows through `sandbox::fork_worldline` and `WorldlineStore`.
- Failure contract documented; SQLite coupling removed from Nuwa simulation signatures.

### Phase D — Production & Features (IN PROGRESS)

**D1. Integration test coverage** ✅
- Added end-to-end storage history coverage for `persist_run → get_run_summary → compare_runs`.

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

The core Rust engine is complete and the post-review A/B/C hardening pass is
finished. Development now moves from correctness/architecture cleanup to
selective production features. Ordered by priority.

### Completed hardening

- Phase A: input limits, delta-memory cleanup, shared text normalization,
  first-class data-integrity errors.
- Phase B: shared time utilities, async TUI data loading, structured logging,
  configurable scoring parameters.
- Phase C: typed Hongmeng state, TUI view-state decoupling, Nuwa tick
  deduplication, unified worldline forking.
- Phase D1: storage history integration coverage.

### Phase D — Remaining Production & Features

**D2. ActorProfile YAML validation**
Files: `src/profile/types.rs`, `src/profile/registry.rs`
- `validate()` method: salience ∈ [0,1], capabilities ∈ [0,1]
- Reject malformed profiles at `ProfileRegistry::load_from_dir`
- Tests for good/base/bad YAML profiles

**D3. SQLite connection pool**
Files: `src/api.rs`, `src/daemon.rs`, `src/storage.rs`
- Replace per-request `Connection::open` in API paths with a small pool
- Keep CLI one-shot storage simple; pool only long-lived runtime paths
- Preserve SQLite foreign-key and read-limit contracts

**D4. Ollama /api/chat migration**
Files: `src/llm/*`, `src/hongmeng/agent.rs`
- Prefer structured chat messages over plain-text `/api/generate`
- Preserve provider registry compatibility and deterministic test clients

**D5. LLM concurrency limiting**
Files: `src/llm/*`, `src/hongmeng/*`, `src/nuwa/*`
- Implement `max_concurrency` with `tokio::sync::Semaphore`
- Tests must prove bounded concurrent calls without serializing deterministic paths

**D6. Worldline causal_graph serialization**
Files: `src/worldline/types.rs`, `src/nuwa/*`
- Add explicit serde contract for `petgraph::DiGraph`
- Verify snapshot round-trip and compatibility with stored worldlines

**D7. Alert dispatch to external channels**
Files: NEW `src/alert_dispatch.rs`, config docs
- Map `AlertTier` to Telegram/Discord/webhook delivery policies
- Include chunking, dry-run mode, and no-secret logging rules

**D8. Fast/slow feed tier separation**
Files: `src/daemon.rs`, config docs
- Extend daemon scheduling config with fast/slow intervals
- Group watched feeds by urgency to reduce API/LLM cost

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
- `cargo test` all green (currently 321)
- `cargo clippy -- -D warnings` zero warning
- `tianji run --fixture ...` output field-level consistent with contracts
- `tianji delta --latest-pair` cross-run change tracking functional
- Single binary < 25MB release
