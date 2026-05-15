# TianJi (天机) — Development Plan v4

> Branch: `rust-cli` | Updated: 2026-05-15
> Target: 智库级信号分析引擎 — 确定性管线 + 跨 run 变化追踪 + 远期多 Agent 仿真
> Current: M1A-M3.5 complete + TUI MVP. Python oracle retired.

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
TUI ████████████████████ 🟡 MVP (history browser), 3 more views planned
```

  源码: 10,967 行 Rust / 16 源文件
  测试: 111 pass / 0 fail
  构建: cargo build + clippy -D warnings zero
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

**四子系统参考 (远期)**:
- Cangjie (仓颉): Feed → Normalize (已实现)
- Fuxi (伏羲): Scoring → Backtrack → Delta (已实现)
- Hongmeng (鸿蒙): Agent 编排层 (远期)
- Nuwa (女娲): 仿真沙盒 (远期)

**确定性保证**:
- BTreeMap (非 HashMap) 所有 state-affecting 路径
- LazyLock regex (非热路径重复编译)
- 确定性 PRNG (远期: ChaCha8 seeded)
- 告警管线使用 persisted `generated_at`，非 `SystemTime::now()`
- 无 `Instant` / `SystemTime` 在核心管线中

**Delta Engine 设计 (Crucix 移植)**:
- 结构化 diff: 数值型指标 (NumericMetricDef) + 计数型指标 (CountMetricDef)
- 三级严重度: moderate / high / critical
- 风险方向推断: RiskOff / RiskOn / Mixed
- HotMemory: 热/冷双层存储，原子 I/O (tmp → rename + .bak)
- AlertDecayModel: 0h/6h/12h/24h 阶梯衰减冷却
- 参考: `/home/kita/code/Crucix/lib/delta/engine.mjs`

---

## 3. Project Structure

```
tianji/
├── Cargo.toml                  # 15 deps (see §7)
├── src/
│   ├── main.rs                 # CLI entry (8 subcommands, 1434 lines)
│   ├── lib.rs                  # Pipeline + 111 integration tests (1710 lines)
│   ├── models.rs               # RawItem → NormalizedEvent → ScoredEvent → RunArtifact
│   ├── fetch.rs                # RSS/Atom (roxmltree) + SHA-256 hash
│   ├── normalize.rs            # Keyword/actor/region extraction (LazyLock regex)
│   ├── scoring.rs              # Im/Fa + divergence + rationale
│   ├── grouping.rs             # Event grouping + causal ordering
│   ├── backtrack.rs            # Intervention candidate generation
│   ├── storage.rs              # SQLite 6 tables + history CRUD (1547 lines)
│   ├── daemon.rs               # UNIX socket + job queue + serve (602 lines)
│   ├── api.rs                  # axum 6-route HTTP API + envelope (427 lines)
│   ├── webui.rs                # Embedded static files + API proxy (323 lines)
│   ├── tui.rs                  # ratatui history browser (1776 lines)
│   ├── delta.rs                # Delta Engine: compute_delta, severity, signals
│   ├── delta_memory.rs         # HotMemory, AlertDecayModel, AlertTier, atomic I/O
│   └── utils.rs                # round2, days_since_epoch, collect_string_array
├── tests/
│   └── fixtures/               # sample_feed.xml + contract fixtures
├── plan.md                     # This document
└── README.md                   # Usage docs
```

---

## 4. Phase 6: Cleanup & Release ✅

**目标**: 独立 Rust 二进制，可开源发布。

### 4.1 删除 Python Oracle ✅
- ✅ 删除 `tianji/*.py` `tests/*.py` `pyproject.toml` `uv.lock`
- ✅ 删除 `.venv/` `.pytest_cache/` `__pycache__/` `.ruff_cache/`
- ✅ 删除 `.agents/` `.codex/` `.gemini/`
- ✅ 删除 `dummy.sqlite3`
- ✅ 删除 `plan-crucix.md` (已吸收到本文档 §2)

### 4.2 文档 ✅
- ✅ 更新 `README.md`: 仅 Rust 用法，移除 Python oracle 引用
- ✅ Shell completions: `clap_complete` 生成 bash/zsh/fish
- ✅ 发布第一个 git tag: `v0.2.0`

### 4.3 验证 ✅
- `cargo build --release` 零 error
- `cargo test` 全绿
- `cargo clippy -- -D warnings` 零 warning

---

## 5. Phase 4: TUI Completion

**当前 (MVP)**: 只读 history browser。Kanagawa Dark 硬编码，Vim 键位 (j/k/g/G/Ctrl+d/u/q)，列表+详情双面板。

**目标 (完整规格)**: 四视图 ratatui TUI。

### 5.1 Dashboard 视图
- Worldline 状态总览: divergence, last run, baseline
- Field 变化趋势: 每个 field 当前值 + 变化方向
- Top Events 列表
- 参考 `plan.md` §9 的 dashboard ASCII 布局 (旧文件，仅布局参考)

### 5.2 Simulation 视图
- 仿真监控: 当前轮次、进度条、Agent 状态 (running/done/pending)
- 人工剪枝交互: 暂停→TUI 选项→选择→继续
- 仅在 predict/backtrack 运行时显示

### 5.3 Profiles 视图
- Actor profile 浏览: 国家/组织/企业 三层
- Static profile + dynamic profile + cross-scenario memory

### 5.4 功能补全
- `/` 搜索/过滤 (fuzzy match)
- Nerd Font / ASCII fallback 自动检测
- `Ctrl+d`/`Ctrl+u` 与 `d` 键区分 (bracketed paste 模式)
- 窗口 resize 响应式布局

### 5.5 实现
- 子模块拆分: `src/tui/{mod,dashboard,simulation,history,profiles}.rs`
- 配色集中: `src/tui/theme.rs`
- 事件循环: 100ms 非阻塞 poll
- 无动画，即时 repaint

---

## 6. Phase 2-3: Hongmeng + Nuwa (远期)

**概述**: LLM 驱动的多 Agent 编排层 + 仿真沙盒。

### 6.1 Hongmeng 编排层

```
┌──────────────────────────────────────────────────┐
│ Hongmeng — 编排中枢 (tokio actor 模型)            │
│ ├─ Agent 生命周期 (spawn/kill/pause/resume)      │
│ ├─ Board/Stick 分层信息公开                         │
│ │   Board: 公开声明/决议 (所有 Agent 可见)          │
│ │   Stick: 私密状态/内部动员 (Agent 专属)           │
│ ├─ Referee: World-State Delta 生成                │
│ ├─ Collision: 碰撞检测 + 矛盾解决                  │
│ ├─ Market Agent: 油价/贸易流独立更新               │
│ ├─ Checkpoint: 每 round SQLite snapshot           │
│ └─ 收敛条件: max_rounds / 连续预测不变 / ε 阈值     │
└──────────────────────────────────────────────────┘
```

参考: WarAgent Board/Stick 模型, Geopol-Forecaster Referee 模式

### 6.2 Nuwa 仿真沙盒

- **前向推演 (predict)**:
  - Fork worldline → 沙盒
  - 多轮 Board/Stick 博弈
  - 每个 Agent 一个 tokio task
  - 输出: Vec<WorldlineBranch> (分支概率 + 事件序列)

- **后向反推 (backtrack)**:
  - Goal → field 约束
  - LLM 粗筛 + 约束精剪 (alpha-beta)
  - 人工暂停: 遇歧义→TUI 选项
  - 输出: Vec<InterventionPath> (按 PathScore 降序)
  - PathScore = w1×goal_proximity + w2×path_probability - w3×intervention_count - w4×collateral_damage

### 6.3 Worldline 数据模型

```rust
struct Worldline {
    id: WorldlineId,
    fields: BTreeMap<FieldKey, f64>,  // 确定性排序
    events: Vec<EventId>,
    causal_graph: petgraph::DiGraph<EventId, CausalRelation>,
    active_actors: HashSet<ActorId>,
    divergence: f64,                   // 与 baseline 的向量距离
    parent: Option<WorldlineId>,       // fork 来源
    diverge_tick: u64,
    snapshot_hash: Blake3Hash,
}
```

- Baseline: 操作者显式锁定 (`tianji baseline --set`) 或历史坐标 (`--at-run 42`)
- Field 依赖图: petgraph::DiGraph<FieldKey, CausalRelation>
- 分支: fork → COW，merge 用 OR-Set + LWW register (少见)

### 6.4 Actor Profile 系统

三层 profile (YAML, git 版本化):

| Tier | 类型 | 特征 |
|------|------|------|
| 1 | 国家 | military + economic + diplomatic + cyber |
| 2 | 组织 | influence + member_states, 无 military |
| 3 | 企业 | market_share + supply_chain |

- Static Profile: 手动维护 (interests, red_lines, capabilities, behavior_patterns)
- Dynamic Profile: 每 round LLM 提取 Temporal Pattern
- Cross-Scenario Memory: reputation_score, relationship_graph (SQLite)

### 6.5 LLM Provider 配置

```yaml
# ~/.tianji/config.yaml (远期)
providers:
  ollama_local:
    type: ollama
    model: qwen3:14b
    base_url: http://localhost:11434
  openai_remote:
    type: openai
    model: gpt-4o
    api_key_env: OPENAI_API_KEY
    fallback: ollama_local

agent_model_map:
  forward_default: ollama_local
  backward_coarse: openai_remote
```

降级链: provider → fallback → 历史行为模式 (无 LLM)

### 6.6 崩溃恢复

- 每 round checkpoint 到 SQLite (worldline + agent 状态 + 消息历史)
- 恢复: `tianji daemon resume --sim-id <id>`
- Agent LLM 超时: 重试 2 次 → 降级为历史行为模式
- 进程崩溃: daemon 重启后 detect 未完成仿真 → 提示 resume/abort

### 6.7 测试策略

```
Layer 1: aimock / llmreplay — 录制真实 LLM 响应, CI replay 确定性测试
Layer 2: Snapshot tests — 仿真第 N 轮 world state snapshot
Layer 3: DeepEval — LLM-as-Judge 周期性质量评估
Layer 4: 管线确定性测试 — 无 LLM 路径全量单元 + 集成测试
```

---

## 7. Dependencies

```toml
[dependencies]
# — 当前 (15 crates) —
clap = { version = "4.6", features = ["derive"] }
serde = { version = "1.0", features = ["derive"] }
serde_json = "1.0"
roxmltree = "0.21"
regex = "1.12"
sha2 = "0.11"
rusqlite = { version = "0.39", features = ["bundled"] }
reqwest = { version = "0.13", default-features = false, features = ["rustls", "blocking"] }
axum = "0.8"
uuid = { version = "1", features = ["v4"] }
tokio = { version = "1", features = ["full"] }
libc = "0.2"
ratatui = "0.30"
crossterm = "0.28"

# — Phase 4 (TUI 完整视图, 无新依赖) —

# — Phase 2-3 (Hongmeng/Nuwa, 远期) —
# serde_yaml = "0.9"
# chrono = "0.4"
# blake3 = "1"
# petgraph = "0.7"
# rand = "0.8"
# rand_chacha = "0.3"
# async-openai = "0.34"
# ollama-rs = "0.3"
# anyhow = "1"
# thiserror = "2"
# tracing = "0.1"
# tracing-subscriber = "0.3"
# tabled = "0.18"

[profile.release]
opt-level = 3
lto = true
```

---

## 8. Key Reference Repos

| Repo | Use | Lang |
|------|-----|------|
| calesthio/Crucix | Delta Engine cross-run tracking + alert decay (已移植) | JS |
| agiresearch/WarAgent | Board/Stick multi-agent negotiation | Python |
| danielrosehill/Geopol-Forecaster | Two-stage simulation + Referee | Python |
| prithwis/Centaur | ZeitWorld/Centaur/Chanakya tri-component | Python |
| Peakstone-Labs/hormuz-agent-sandbox | 4-nation real-time multi-agent sim | Vue+Python |
| in6black/seldon-vault | 11 analyst Hawk/Dove dual | Python |
| dx111ge/intel-analyst | Bayesian + WASM Rust prob engine | Rust+JS |
| langchain-ai/langgraph | Checkpoint + state machine | Python |
| tachyon-beep/murk | Tick engine + deterministic replay | Rust |
| multikernel/branching | COW fork + multi-branch | Python |
| adk-rust/adk-graph | Rust LangGraph + durable resume | Rust |
| CopilotKit/aimock | LLM mock deterministic testing | TS |
| confident-ai/deepeval | LLM quality evaluation | Python |

---

## 9. Research Docs

Saved under `/home/kita/code/knowledge/projects/`:

| Doc | Lines | Topic |
|-----|-------|-------|
| tianji-research-multi-agent-negotiation.md | 467 | Agent info protocols, non-state actor modeling |
| tianji-research-orchestration-testing.md | 533 | Daemon vs on-demand, multi-provider config, checkpoint |
| tianji-research-worldline-baseline.md | 334 | Baseline definition, field dependency, causal graph |
| tianji-research-sqlite-event-pipeline.md | 153 | Recompute-vs-Persist, CQRS, Milestone 2 migration |
| tianji-design-questions.md | 129 | Original 16 open design questions |
| tianji-design-recommendations.md | 253 | Research-backed recommendations for all 16 questions |

---

## 10. Verification Criteria

Each phase must pass before moving to the next:

- `cargo build` / `cargo build --release` zero error
- `cargo test` all green
- `cargo clippy -- -D warnings` zero warning
- `tianji run --fixture ...` output field-level consistent with contracts
- `tianji delta --latest-pair` cross-run change tracking functional
- Single binary < 25MB release

Phase 2-3 additional:
- `tianji predict --field east-asia.conflict --horizon 30d` → Vec<WorldlineBranch>
- `tianji backtrack --goal "东亚稳定" --max-interventions 5` → Vec<InterventionPath>
- Manual pruning: simulation pause → TUI option → select → continue
- Checkpoint: kill process mid-sim → daemon resume → continue from breakpoint
