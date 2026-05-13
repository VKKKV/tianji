# TianJi 全量 Rust 重写计划 v2

> 分支: `rust-cli` | 更新: 2026-05-13
> 目标: 智库级双向推理引擎 — 推演世界线 + 反推干预路径
> 灵感: Karpathy llm-wiki 模式 + angr 符号执行反推 + 多 Agent 博弈

---

## 1. 系统架构

```
┌──────────────────────────────────────────────────────┐
│  Hongmeng (鸿蒙) — 编排中枢                           │
│  ├─ tokio actor 模型                                  │
│  ├─ Agent 生命周期管理 (spawn/kill/pause/resume)      │
│  ├─ 消息路由 (Cangjie ↔ Fuxi ↔ Nuwa)                 │
│  └─ 碰撞检测 + 矛盾解决                               │
├──────────────────────────────────────────────────────┤
│                                                      │
│  ┌──────────────┐  ┌──────────────┐  ┌────────────┐  │
│  │ Cangjie (仓颉)│  │ Fuxi (伏羲)   │  │ Nuwa (女娲) │  │
│  │ 无头 OSINT    │  │ 分歧建模     │  │ 仿真沙盒    │  │
│  │              │  │              │  │            │  │
│  │ RSS/Atom     │  │ field 状态机  │  │ 前向推演    │  │
│  │ Web scraping │  │ 阈值监控     │  │ 后向反推    │  │
│  │ API feeds    │  │ 模式检测     │  │ 干预测试    │  │
│  │              │  │ divergence   │  │ 扰动回放    │  │
│  │ → signals    │  │ → alerts     │  │ → branches  │  │
│  └──────────────┘  └──────────────┘  └────────────┘  │
│                                                      │
└──────────────────────────────────────────────────────┘
         │                  │                  │
         ▼                  ▼                  ▼
    ┌─────────────────────────────────────────────┐
    │  CLI / TUI / HTTP API / Web UI               │
    │  tianji run | watch | predict | backtrack    │
    └─────────────────────────────────────────────┘
```

**四子系统职责：**

| 子系统 | 做什么 | Rust 实现 |
|--------|--------|-----------|
| Cangjie | 采集信号 → 归一化 → 入库 | `src/cangjie/` quick-xml + reqwest + regex |
| Fuxi | worldline 状态机 + divergence 计算 | `src/fuxi/` field 引擎 + 阈值/模式检测 |
| Hongmeng | Agent 编排 + IPC + 消息路由 | `src/hongmeng/` tokio actors + channel |
| Nuwa | 仿真沙盒：前向推演 + 后向反推 | `src/nuwa/` 沙盒环境 + Agent 执行器 |

---

## 2. Worldline 数据模型

状态机模型。worldline = 可变的 fields + 不可变的事件因果图。

```rust
struct Worldline {
    id: WorldlineId,
    fields: HashMap<FieldKey, f64>,     // "east-asia.conflict": 0.72
    events: Vec<EventId>,                // 导致当前状态的信号序列
    causal_graph: DiGraph<EventId, CausalRelation>,
    active_actors: HashSet<ActorId>,
    divergence: f64,                     // 与 baseline 的向量距离
    parent: Option<WorldlineId>,         // fork 来源 (Nuwa 沙盒用)
    timestamp: DateTime,
}

struct FieldKey {
    region: String,       // "east-asia" | "europe" | "middle-east" | "global" | ...
    domain: String,       // "conflict" | "economy" | "diplomacy" | "technology" | ...
}
```

**Field 体系：预定义核心 + LLM 补充分支**

核心 fields 人工设计，确定性评分。Cangjie 摄入信号 → regex 提取 actor/region/domain → 匹配核心 field → 加减 impact_score。

LLM 负责：
- 建议新增 fields（"检测到新的信号模式：北极航道竞争"）
- Nuwa 仿真阶段辅助 Agent 判断干预连锁影响

---

## 3. 管线 (Cangjie → Fuxi)

```
RSS/Atom feed
  │
  ▼
ingest::feed  ──→ Vec<RawItem>
  │  quick-xml 解析 RSS 2.0 + Atom 1.0
  │  SHA256 content-hash / identity-hash
  ▼
normalize     ──→ Vec<NormalizedEvent>
  │  regex 提取: keywords, actors, regions, field_scores
  │  patterns 从 Python normalize.py 移植
  ▼
score         ──→ Vec<ScoredEvent>
  │  Im = actor_weight + region_weight + keyword_density + ...
  │  Fa = dominant_field_strength + dominance_margin + coherence
  │  divergence_score = f(Im, Fa)
  ▼
group         ──→ Vec<EventGroupSummary>
  │  共享 keyword/actor/region + 时间窗口 24h
  │  causal ordering + evidence chain
  ▼
backtrack     ──→ Vec<InterventionCandidate>
  │  硬编码映射: dominant_field → intervention_type
  ▼
update worldline
  │  Fuxi 更新 fields: target_field += Σ impact_score × field_attraction
  │  events 追加到因果图
  │  重算 divergence
  ▼
emit artifact + persist SQLite
```

**与 Python 版的差异：**
- 管线结束不是输出 JSON 就完——是更新 worldline 状态
- `backtrack` 从硬编码映射升级为 field-aware（干预建议关联到具体 field）
- 每次 run 产生一个 worldline snapshot，SQLite 存完整历史

---

## 4. Hongmeng 编排层

**触发机制（混合）：**
- 操作者手动: `tianji predict --field east-asia.conflict --horizon 30d`
- 自动规则: field 偏离 > 阈值 或 事件模式匹配 → 自动拉起仿真
- 规则可配置: `~/.tianji/rules.yaml`

**Agent 分工：按角色分配**

Hongmeng 读取 worldline.active_actors → 为每个 actor spawn 一个 Agent → 下发角色 + worldline 状态。

Agent 之间用**多轮博弈**：
1. Round 1: 各 Agent 独立推演（不知道其他 Agent 的预测）
2. Hongmeng 汇总 → 碰撞检测 → 标记矛盾
3. Round 2: 公开部分结果（"Actor A 可能做 X"）→ Agent 调整预测
4. 迭代到收敛或最大轮数

---

## 5. Actor Profile（Agent 角色约束）

LLM 辅助 profile。骨架 YAML + LLM 推理。

```yaml
# profiles/china.yaml
id: china
name: China
interests:
  - "maintain territorial integrity in South China Sea" (salience: 0.95)
  - "secure energy supply routes through Malacca Strait" (salience: 0.85)
  - "expand semiconductor technology independence" (salience: 0.80)
  - "maintain stable trade relationships with EU" (salience: 0.70)
red_lines:
  - "foreign military presence in Taiwan Strait → full retaliatory posture"
  - "technology export ban on advanced chips → accelerate domestic R&D pipeline"
capabilities:
  military: 0.85
  economic: 0.80
  technological: 0.70
  diplomatic: 0.75
  cyber: 0.82
behavior_patterns:
  - "responds to sanctions with proportional counter-sanctions"
  - "prefers economic leverage (BRI investments, rare earth exports) before military signaling"
  - "uses state-owned enterprises as policy instruments"
  - "prioritizes stability in neighboring regions over distant interventions"
historical_analogues:
  - "2016 South China Sea arbitration response"
  - "2017 THAAD deployment in South Korea → economic retaliation against Lotte"
```

Agent 演绎时：read profile + current worldline → LLM 推理（"given constraints X/Y/Z, most likely action is..."）→ 输出 ActionProposal。

**Profile 来源：**
- 人工编写核心 actor profiles
- LLM 辅助生成次要 actor profiles（从公开信息提取）
- profile 本身可以版本化，随 worldline 演化

---

## 6. Nuwa 仿真沙盒

### 前向推演 (Forward)

```
tianji predict --field east-asia.conflict --horizon 30d

1. Hongmeng fork 当前 worldline → 创建沙盒 worldline
2. 按 worldline.active_actors spawn Agents (每个一个 tokio task)
3. 多轮博弈:
   Round 1: 各 Agent 独立推演 → ActionProposal
   Round 2: Hongmeng 碰撞检测 → 公开矛盾 → Agent 调整
   Round N: 收敛或 max_rounds
4. 每个 ActionProposal 应用到沙盒 worldline → field 变化
5. 输出: Vec<WorldlineBranch> (各分支的概率 + 关键事件序列)
```

### 后向反推 (Backward / angr 模式)

```
tianji backtrack --goal "东亚区域稳定，贸易正常化" --max-interventions 5

1. LLM 解析 goal → field 约束: east-asia.conflict < 0.3, global.trade_volume > 0.7
2. Hongmeng fork 当前 worldline → 创建反向沙盒
3. 约束前置剪枝:
   - 行动不能违反 agent profile red_lines
   - 不能超出 capabilities
   - 不符合 behavior_patterns 的降权
4. LLM 粗筛: 每个 Agent 在每轮前推演 3-5 个最可能的行动方向
5. 约束精剪: 博弈评分 + alpha-beta
6. 人工剪枝: 推演中遇歧义 → Hongmeng 暂停 → TUI 呈现选项 → 操作者选择
7. 输出: Vec<InterventionPath> (按干预步数 + 成功率排序)
```

### 人工剪枝协议

推演中遇到以下情况时 Hongmeng 暂停:
- LLM 对某 Agent 的行动方向分歧过大（多个选项概率接近）
- 碰撞检测发现不可调和矛盾
- 操作者预设的暂停点（`--pause-on field.east-asia.conflict > 0.7`）

暂停时 TUI 呈现:
```
[Simulation Paused] Round 3, Agent: China
  Worldline: east-asia.conflict=0.72
  Decision point: "US carrier group enters South China Sea"
  Options:
    [1] Diplomatic protest + UN appeal           (概率: 0.45)
    [2] Naval exercises in response zone         (概率: 0.35)
    [3] Economic sanctions against US allies      (概率: 0.15)
    [4] No immediate response (monitor)           (概率: 0.05)
    [p] Prune all military options
    [a] Auto-continue (pick highest probability)
> _
```

**剪枝决策存为规则** — 操作者的剪枝选择可以存为全局/场景规则，后续仿真自动应用，减少重复暂停。

---

## 7. 项目结构

```
tianji/
├── Cargo.toml
├── src/
│   ├── main.rs                 # clap 入口
│   ├── lib.rs                  # 库根
│   │
│   ├── models.rs               # Worldline, NormalizedEvent, ScoredEvent, etc.
│   ├── error.rs
│   │
│   ├── cangjie/                # 仓颉: 信号采集
│   │   ├── mod.rs
│   │   ├── feed.rs             # RSS/Atom 解析 (quick-xml)
│   │   ├── fetch.rs            # HTTP fetch (reqwest)
│   │   ├── normalize.rs        # 关键词/actor/region 提取 (regex)
│   │   └── sources.rs          # source registry + fetch policy
│   │
│   ├── fuxi/                   # 伏羲: 分歧建模
│   │   ├── mod.rs
│   │   ├── worldline.rs        # Worldline 状态机 (fields + causal graph)
│   │   ├── scoring.rs          # Im/Fa 评分 + divergence 计算
│   │   ├── grouping.rs         # 事件分组 + causal ordering
│   │   ├── backtrack.rs        # 干预候选生成
│   │   └── triggers.rs         # 阈值/模式检测 → Hongmeng 告警
│   │
│   ├── hongmeng/               # 鸿蒙: 编排层
│   │   ├── mod.rs              # tokio 运行时 + 子系统启动
│   │   ├── agent_lifecycle.rs  # Agent spawn/kill/pause/resume
│   │   ├── router.rs           # 消息路由 (channel-based)
│   │   ├── collision.rs        # 多 Agent 碰撞检测 + 矛盾解决
│   │   └── rules.rs            # 自动触发规则引擎
│   │
│   ├── nuwa/                   # 女娲: 仿真沙盒
│   │   ├── mod.rs
│   │   ├── sandbox.rs          # 沙盒环境: fork worldline, 隔离变更
│   │   ├── forward.rs          # 前向推演: 多轮博弈
│   │   ├── backward.rs         # 后向反推: angr 模式 + 剪枝
│   │   ├── agent.rs            # Agent 执行器: profile + LLM 推理
│   │   ├── profile.rs          # Actor profile 加载/管理
│   │   └── pruning.rs          # 剪枝策略引擎
│   │
│   ├── storage.rs              # rusqlite: worldline snapshots, runs, profiles
│   │
│   ├── cli/                    # CLI (clap derive)
│   │   ├── mod.rs
│   │   ├── run.rs              # tianji run (管线)
│   │   ├── watch.rs            # tianji watch (持续监控)
│   │   ├── predict.rs          # tianji predict (前向推演)
│   │   ├── backtrack.rs        # tianji backtrack (后向反推)
│   │   ├── history.rs          # tianji history/show/compare
│   │   ├── daemon.rs           # tianji daemon start/stop/status
│   │   └── tui.rs              # tianji tui
│   │
│   ├── tui/                    # ratatui 终端 UI
│   │   ├── mod.rs
│   │   ├── dashboard.rs        # worldline 状态总览
│   │   ├── simulation.rs       # 仿真监控 + 暂停/人工剪枝交互
│   │   ├── history.rs          # run 历史浏览
│   │   └── profiles.rs         # Actor profile 浏览/编辑
│   │
│   ├── daemon/                 # axum HTTP API + UNIX socket 控制
│   │   ├── mod.rs
│   │   ├── server.rs           # axum HTTP 服务 (loopback)
│   │   ├── socket.rs           # UNIX socket 控制面
│   │   └── jobs.rs             # 后台 job 队列
│   │
│   ├── webui.rs                # axum serve static web UI
│   ├── llm.rs                  # LLM 调用抽象层 (local/remote, 可插拔)
│   └── output.rs               # 终端输出格式化
│
├── profiles/                   # Actor profile YAML 文件
│   ├── china.yaml
│   ├── russia.yaml
│   ├── usa.yaml
│   ├── eu.yaml
│   └── ...
│
├── rules/                      # 自动触发规则
│   └── default.yaml
│
├── tianji/webui/               # 静态 Web UI (保留现有)
├── tests/
│   ├── fixtures/sample_feed.xml
│   ├── test_pipeline.rs
│   ├── test_scoring.rs
│   ├── test_worldline.rs
│   ├── test_nuwa_forward.rs
│   ├── test_nuwa_backward.rs
│   └── test_agent_pruning.rs
├── plan.md
└── README.md
```

---

## 8. 依赖清单

```toml
[package]
name = "tianji"
version = "0.2.0"
edition = "2024"

[dependencies]
# CLI
clap = { version = "4", features = ["derive"] }

# 序列化
serde = { version = "1", features = ["derive"] }
serde_json = "1"
serde_yaml = "0.9"

# 管线
quick-xml = { version = "0.37", features = ["serialize"] }
regex = "1"
chrono = { version = "0.4", features = ["serde"] }
sha2 = "0.10"

# HTTP
reqwest = { version = "0.12", features = ["rustls-tls"], default-features = false }
axum = "0.8"

# 异步
tokio = { version = "1", features = ["full"] }

# 持久化
rusqlite = { version = "0.32", features = ["bundled"] }

# TUI
ratatui = "0.29"
crossterm = "0.28"

# 输出
tabled = "0.18"

# LLM
async-openai = "0.27"          # OpenAI-compatible API
ollama-rs = "0.2"              # local Ollama

# 图 (causal graph)
petgraph = "0.7"

# 错误/日志
anyhow = "1"
thiserror = "2"
tracing = "0.1"
tracing-subscriber = "0.3"

[dev-dependencies]
tempfile = "3"
assert-json-diff = "2"

[profile.release]
opt-level = 3
lto = true
```

---

## 9. 开发阶段

### Phase 1: Worldline 核心 + 管线 (最大工作量)
- models.rs: 所有数据结构
- cangjie/: feed 解析, normalize, fetch
- fuxi/: worldline 状态机, scoring, grouping, backtrack, triggers
- storage.rs: SQLite schema
- CLI: `tianji run`
- 验证: 输出与 Python 版 JSON 字段级对齐

### Phase 2: Hongmeng 编排层
- tokio actor 模型
- Agent 生命周期管理
- 消息路由
- 碰撞检测
- 自动触发规则引擎
- CLI: `tianji watch` (持续运行)

### Phase 3: Nuwa 仿真沙盒
- sandbox.rs: worldline fork + 隔离
- agent.rs: Agent 执行器 + profile 加载
- forward.rs: 多轮博弈前向推演
- backward.rs: 后向反推 + 剪枝引擎
- pruning.rs: LLM粗筛 + 约束精剪 + 人工暂停
- CLI: `tianji predict`, `tianji backtrack`

### Phase 4: TUI
- dashboard: worldline 状态总览
- simulation: 仿真监控 + 人工剪枝交互
- history: run 历史浏览
- profiles: Actor profile 管理

### Phase 5: Daemon + Web UI
- axum HTTP API + UNIX socket
- 后台 job 队列
- static web UI serve

### Phase 6: 清理 + 文档
- 删除所有 Python 代码
- 删除 `.venv/` `.agents/` `.codex/` `.gemini/`
- 更新 README
- shell completions

---

## 10. 删除清单

- 所有 Python 代码: `tianji/*.py` `tests/*.py` `pyproject.toml` `uv.lock`
- `.venv/` `.pytest_cache/` `__pycache__/`
- `.agents/` `.codex/` `.gemini/`（保留需要的 `.opencode/` 配置）
- `node_modules/`（`.opencode/` 内需要的保留）
- `dummy.sqlite3`

---

## 11. 验证标准

- `cargo build --release` 零 warning
- `cargo test` 全绿
- `tianji run --fixture ...` 输出与 Python 版字段级一致
- `tianji predict --field east-asia.conflict --horizon 30d` 产出一组 WorldlineBranch
- `tianji backtrack --goal "东亚稳定" --max-interventions 5` 产出一组 InterventionPath
- 人工剪枝: 仿真中暂停 → TUI 呈现选项 → 选择后继续 → 仿真完成
- 单二进制 < 25MB release
