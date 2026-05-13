# TianJi — 全量 Rust 重写计划 v3

> 分支: `rust-cli` | 更新: 2026-05-13
> 目标: 智库级双向推理引擎 — 推演世界线 + 反推干预路径
> 灵感: Karpathy llm-wiki 模式 + angr 符号执行反推 + 多 Agent 博弈
> 研究参考: Geopol-Forecaster, Centaur, hormuz-agent-sandbox, WarAgent, adk-graph

> Trellis alignment: this file is a long-range Rust architecture vision, not
> shipped behavior. Current shipped TianJi remains the Python implementation
> under `tianji/` with `unittest` coverage under `tests/`. The staged migration
> gates and first implementation slice are defined in
> `.trellis/spec/backend/rust-migration-plan.md`; do not delete Python or claim
> Hongmeng/Nuwa architecture is shipped before those parity gates pass.

---

## 1. 系统架构

```
┌──────────────────────────────────────────────────────────────┐
│  Hongmeng (鸿蒙) — 编排中枢 (tokio actor 模型)               │
│  ├─ Agent 生命周期 (spawn/kill/pause/resume)                 │
│  ├─ 消息路由 + Board/Stick 分层信息公开                      │
│  ├─ 碰撞检测 + 矛盾解决                                      │
│  ├─ Checkpoint 管理 (每 round 自动 SQLite snapshot)          │
│  └─ 运行模式: CLI 手动 + daemon 常驻                         │
├──────────────────────────────────────────────────────────────┤
│                                                              │
│  ┌──────────────┐  ┌──────────────┐  ┌────────────────────┐  │
│  │ Cangjie (仓颉)│  │ Fuxi (伏羲)   │  │ Nuwa (女娲)      │  │
│  │ 信号采集      │  │ 分歧建模     │  │ 仿真沙盒          │  │
│  │              │  │              │  │                    │  │
│  │ RSS/Atom     │  │ Worldline    │  │ Forward: 多轮博弈  │  │
│  │ Web scraping │  │ 状态机       │  │ Backward: angr 反推│  │
│  │ API feeds    │  │ petgraph DAG │  │ ┌─ Board (公开)    │  │
│  │ content-hash │  │ Blake3 hash  │  │ ├─ Stick (私密)    │  │
│  │              │  │ divergence   │  │ ├─ Referee (汇总)  │  │
│  │ → signals    │  │ → alerts     │  │ └─ Market Agent    │  │
│  └──────────────┘  └──────────────┘  └────────────────────┘  │
│                                                              │
└──────────────────────────────────────────────────────────────┘
         │                  │                  │
         ▼                  ▼                  ▼
    ┌─────────────────────────────────────────────────────┐
    │  CLI / TUI / HTTP API / Web UI                       │
    │  tianji run | watch | predict | backtrack | daemon   │
    └─────────────────────────────────────────────────────┘
```

**四子系统 + 参考映射:**

| 子系统 | 做什么 | 参考项目 |
|--------|--------|---------|
| Cangjie | RSS/Atom 采集 → 归一化 → content-hash 去重 | TianJi 现有管线 |
| Fuxi | Worldline 状态机 + divergence 计算 + 阈值告警 | Centaur 的 ZeitWorld |
| Hongmeng | Agent 编排 + Board/Stick 信息公开 + Checkpoint | Centaur 的 Centaur 裁判 + WarAgent 通信模型 |
| Nuwa | 仿真沙盒: 前向多轮博弈 + 后向 angr 反推 + 人工剪枝 | Geopol-Forecaster 两阶段仿真 + hormuz-agent-sandbox |

---

## 2. Worldline 数据模型

```rust
struct Worldline {
    id: WorldlineId,
    fields: BTreeMap<FieldKey, f64>,   // 确定性排序, 非 HashMap
    events: Vec<EventId>,
    causal_graph: DiGraph<EventId, CausalRelation>,  // petgraph
    active_actors: HashSet<ActorId>,
    divergence: f64,                     // 与 baseline 的向量距离
    parent: Option<WorldlineId>,         // fork 来源
    diverge_tick: u64,                   // 分支点
    snapshot_hash: Blake3Hash,           // 每 tick 的 fields hash
    timestamp: DateTime,
}

struct FieldKey {
    region: String,       // "east-asia" | "europe" | "middle-east" | "global" | ...
    domain: String,       // "conflict" | "economy" | "diplomacy" | "technology" | ...
}
```

### Baseline 定义
- 操作者显式锁定: `tianji baseline --set` → 锁定当前 worldline 为 baseline
- 历史坐标: `tianji baseline --at-run 42` → 指定某次历史 run
- divergence = 逐 field 向量距离 (当前 fields vs baseline fields)
- 存储: baseline snapshot hash + field snapshot 存 SQLite
- 参考: murk-replay 的 `FieldDivergence`, git-warp 的坐标 pin

### Field 依赖图
- `petgraph::DiGraph<FieldKey, CausalRelation>` 预定义核心依赖边
- 拓扑排序决定 field 更新顺序
- 后期可选: pgmpy/DoWhy 离线因果发现 → 导入新边
- 参考: ModelingToolkit.jl 的 `varvar_dependencies`

### 分支管理
- 每个 Worldline 有唯一 id + parent_id + diverge_tick
- 分支状态存 SQLite, `history-compare` 可对比任意两分支
- 合并: OR-Set + LWW register CRDT 模式 (少见但保留可能性)
- 参考: git-warp Strand/Braid 代数, multikernel/branching COW fork

### 确定性保证
- 所有 state-affecting 路径用 BTreeMap (非 HashMap)
- 确定性 PRNG: ChaCha8 seeded
- Tick-based 时间 (非 wall clock)
- 无 `std::time::Instant` / `SystemTime` 在管线中
- 参考: murk 的 arena/ping-pong snapshot, tokio turmoil 确定性模拟

---

## 3. 管线 (Cangjie → Fuxi)

```
RSS/Atom feed
  │  quick-xml 解析 RSS 2.0 + Atom 1.0
  │  SHA256 content-hash / identity-hash (去重用)
  ▼
Vec<RawItem>
  │  regex 提取: keywords, actors, regions, field_scores
  │  patterns 从 Python normalize.py 移植
  ▼
Vec<NormalizedEvent>
  │  Im = actor_weight + region_weight + keyword_density
  │       + dominant_field_bonus + field_diversity + text_signal
  │  Fa = dominant_field_strength + dominance_margin + coherence
  │       - near_tie_penalty - diffuse_third_field_penalty
  │  divergence_score = f(Im, Fa)
  ▼
Vec<ScoredEvent>
  │  共享 keyword/actor/region + 时间窗口 24h
  │  causal ordering + evidence chain
  ▼
Vec<EventGroupSummary>
  │  dominant_field → intervention_type 映射
  │  升级为 field-aware (干预建议关联到具体 field)
  ▼
Vec<InterventionCandidate>
  │  Fuxi 更新 fields: target_field += Σ impact_score × field_attraction
  │  events 追加到因果图，重算 divergence，生成 Blake3 snapshot
  ▼
emit RunArtifact JSON + persist WorldlineSnapshot to SQLite
```

---

## 4. Actor Profile (角色约束)

三层 profile 系统 + 分层 actor 架构。

### Profile 结构

```yaml
# profiles/china.yaml
id: china
name: China
tier: nation              # nation | organization | corporation | individual

# --- Static Profile (手动维护, git 版本化) ---
interests:
  - "maintain territorial integrity in South China Sea" (salience: 0.95)
red_lines:
  - "foreign military presence in Taiwan Strait → full retaliatory posture"
capabilities:
  military: 0.85
  economic: 0.80
  technological: 0.70
  diplomatic: 0.75
  cyber: 0.82
behavior_patterns:
  - "responds to sanctions with proportional counter-sanctions"
  - "prefers economic leverage before military signaling"
historical_analogues:
  - "2016 South China Sea arbitration response"
  - "2017 THAAD deployment → economic retaliation against Lotte"

# --- Dynamic Profile (每 round 更新, LLM 提取) ---
# 由 LLM 在每轮结束后提取 Temporal Pattern
# 参考: DyTA4Rec 的 Temporal Pattern Extractor

# --- Cross-Scenario Memory (跨仿真持久, SQLite) ---
# reputation_score: 0.72
# relationship_graph: { usa: -0.3, russia: +0.4, eu: +0.1 }
# learned_strategies: ["counter-sanctions effective when target depends on rare earths"]
```

### 分层 Actor 架构

| Tier | 类型 | Profile 差异 | 示例 |
|------|------|-------------|------|
| 1 | 国家 | 完整 (military + economic + diplomatic + cyber) | China, USA, Russia |
| 2 | 组织 | 无 military, 有 influence + member_states | NATO, OPEC, EU |
| 3 | 企业 | 无 military/diplomatic, 有 market_share + supply_chain | 华为, 台积电 |
| 4 | 个人 | 简化, 侧重 decision_style + personal_network | 关键领导人 |

统一 Agent 执行器，不同 action space。同场仿真中 Tier 1-3 可共存。

参考: SwarmCast 23 persona agents, Geopol-Modeller YAML actor clusters

---

## 5. Hongmeng 编排层

### 运行模式 (混合)

```
tianji run       → 一次性管线 (同步, CLI)
tianji watch     → 持续性监控 (daemon, 默认轮询 300s)
tianji predict   → 手动前向推演 (CLI → Hongmeng → 返回)
tianji backtrack → 手动后向反推 (同上)
tianji daemon    → 常驻进程: auto triggers + Web API + checkpoint recovery
```

参考: James Carr "Seven Hosting Patterns" — Persistent Daemon + Scheduled + Event-Driven 混合

### 触发机制

- 操作者手动: `tianji predict --field east-asia.conflict --horizon 30d`
- 自动规则: field 偏离 > 阈值 或 事件模式匹配 → 自动拉起仿真
- 规则可配置: `~/.tianji/rules.yaml`
- watch 模式: 轮询 RSS → content-hash 去重 → 新事件 → pipeline → 检查触发规则

### 多轮博弈协议 (WarAgent Board/Stick 模型)

```
每轮信息流:

1. REFEREE 生成 World-State Delta (所有 Agent 可见)
   → "Iran increased military readiness to Level 3"
   → "Oil price rose 12%"

2. PUBLIC BOARD 累积 (所有 Agent)
   → 同盟声明、联合国决议、公开外交行动

3. DIRECTED MESSAGES (收件人专属)
   → 双边谈判、威胁/最后通牒

4. PRIVATE STICK (Agent 专属)
   → 内部动员状态、经济储备、国内稳定

5. MARKET AGENT 更新 (所有 Agent)
   → 油价、贸易流、制裁指数

Agent 收到: Referee Summary + Public Board + Directed Messages to self + Market
Agent 看不到: 其他 Agent 的 Private Stick、别人的 Directed Messages
```

参考: WarAgent (agiresearch/WarAgent) — 模拟 WWI/WWII, Board/Stick 是核心设计
参考: Geopol-Forecaster — Referee narrates world state, actors see only referee-authored state

### 收敛条件 (复合)

```
收敛 = 任一满足:
  1. max_rounds 达到 (默认 10)
  2. 所有 Agent 连续 2 轮预测不变
  3. Worldline fields 变化 < ε (默认 0.01)
  4. LLM token 预算耗尽 (100K tokens/仿真)

前向超时: 5min (TUI) / 30min (daemon)
后向搜索树: 最大 1000 节点 (超出返回 PartialPath)
```

---

## 6. Nuwa 仿真沙盒

### 沙盒隔离

- fork worldline = Clone WorldState + 分配新 WorldlineId(parent=当前)
- 仿真中在沙盒 worldline 上操作 (内存)
- 每 N 轮或每个 Agent 回合后 snapshot 到 SQLite (可恢复)
- Agent LLM 调用只看沙盒 worldline + 自己 profile
- 结束: commit (写入永久分支) 或 abort (丢弃)
- 参考: multikernel/branching COW, LangGraph checkpoint

### 前向推演

```
tianji predict --field east-asia.conflict --horizon 30d

1. Hongmeng fork worldline → 沙盒
2. 按 active_actors spawn Agents (每个一个 tokio task)
3. 多轮 Board/Stick 博弈:
   Round 1: 各 Agent 独立推演 → ActionProposal
   Hongmeng 碰撞检测 → 标记矛盾
   Round 2: Referee 公开 World-State Delta + Board → Agent 调整
   Round N: 收敛或 max_rounds
4. 输出: Vec<WorldlineBranch> (分支概率 + 事件序列)
```

### 后向反推 (angr 模式)

```
tianji backtrack --goal "东亚区域稳定，贸易正常化" --max-interventions 5

1. LLM 解析 goal → field 约束: east-asia.conflict < 0.3, global.trade_volume > 0.7
2. Hongmeng fork worldline → 反向沙盒
3. 约束前置剪枝: 不违反 red_lines, 不超 capabilities, 不符合 patterns 降权
4. LLM 粗筛: 每个 Agent 每轮推演 3-5 个最可能行动方向
5. 约束精剪: 博弈评分 + alpha-beta
6. 人工剪枝: 遇歧义 → Hongmeng 暂停 → TUI 选项 → 操作者选择
7. 输出: Vec<InterventionPath> (按 PathScore 降序)

PathScore = w1 × goal_proximity
          + w2 × path_probability
          - w3 × intervention_count
          - w4 × collateral_damage

默认权重: w1=1.0, w2=0.5, w3=0.3, w4=0.5
可 CLI 调整: --weights 1.0,0.5,0.3,0.5
```

### 人工剪枝协议

暂停触发条件:
- LLM 对某 Agent 行动方向分歧过大 (多选项概率接近)
- 碰撞检测发现不可调和矛盾
- 操作者预设暂停点 (`--pause-on field.east-asia.conflict > 0.7`)

暂停界面:
```
[Simulation Paused] Round 3, Agent: China
  Worldline: east-asia.conflict=0.72
  Decision: "US carrier group enters South China Sea"
  Options:
    [1] Diplomatic protest + UN appeal           (0.45)
    [2] Naval exercises in response zone         (0.35)
    [3] Economic sanctions against US allies      (0.15)
    [4] No immediate response                    (0.05)
    [p] Prune all military options
    [a] Auto-continue (pick highest prob)
> _
```

剪枝决策存为规则 — 后续仿真自动应用。

---

## 7. LLM Provider 配置

声明式 YAML + 每 Agent 分配 + 降级链。

```yaml
# ~/.tianji/config.yaml
providers:
  ollama_local:
    type: ollama
    model: qwen3:14b
    base_url: http://localhost:11434
    max_concurrency: 3

  openai_remote:
    type: openai
    model: gpt-4o
    api_key_env: OPENAI_API_KEY
    fallback: ollama_local

agent_model_map:
  forward_default: ollama_local
  backward_coarse: openai_remote
  backward_fine: ollama_local
```

参考: Astromesh declarative provider registry

---

## 8. 崩溃恢复

- 每个仿真 round 结束后自动 checkpoint 到 SQLite
- checkpoint 内容: worldline fields + Agent 状态 + 消息历史
- 恢复: `tianji daemon resume --sim-id <id>` → 从最后 checkpoint 继续
- Agent LLM 超时: 重试 2 次 → 跳过该 Agent (降级为历史行为模式)
- 进程崩溃: daemon 重启后 detect 未完成仿真 → 提示 resume/abort
- 参考: LangGraph checkpoint, adk-graph durable resume

---

## 9. TUI 设计规范

风格方向: Minimal Dashboard。与用户终端环境一致。

### 配色 — Kanagawa Dark

```
背景:       #1F1F28  (Kanagawa bg)
面板背景:   #272727  (Alacritty background — 略亮于 Kanagawa 区分层)
面板边框:   #363646  (Kanagawa dim, 细线)
前景:       #DCD7BA  (Kanagawa fg)
字段标签:   #7E9CD8  (Kanagawa blue)
数值:       #DCD7BA  (fg)
数值上升:   #98BB6C  (Kanagawa green, ↑)
数值下降:   #E46876  (Kanagawa red, ↓)
警告/偏离:   #FFA066  (Kanagawa peach, 仅关键告警)
状态栏:     #363646  (dim bg)
按键提示:   #938AA9  (Kanagawa purple)
标题:       #E6C384  (Kanagawa yellow, 温和)
```

### 字体

- 主字体: MapleMono NF CN (用户终端字体)
- Nerd Font glyphs 可选:  (warning)  (arrow)
- ASCII fallback: 用 [x] [-] [>] 代替，检测到非 Nerd Font 终端自动降级

### 布局

```
┌─ Title Bar ─────────────────────────────────────────┐
│ tianji · divergence 0.337261 · run #42               │
├─ Main Panel ────────────────────────────────────────┤
│  (当前视图: dashboard | history | detail | compare)  │
├─ Status Bar ────────────────────────────────────────┤
│ watch:active  daemon:running  [h]elp [q]uit          │
└──────────────────────────────────────────────────────┘
```

### Vim 键位

```
j/k         — 列表上下移动
h/l         — 面板焦点左右切换
gg/G        — 跳转列表首/尾
/           — 搜索/过滤
Enter       — 选择/展开
Esc         — 退出/返回
q           — 退出
?           — 帮助
数字 + G    — 跳转到第 N 行
Ctrl+d/u    — 半页滚动
```

### 动画

无。不闪烁，不过渡。panel 切换是即时 repaint。

### 视图

**Dashboard (主页)**

```
┌─ Worldline ────────────────────────────────────────┐
│ divergence   0.337261                               │
│ last run     2026-05-13 14:22:03 (run #42)          │
│ baseline     run #1 (2026-03-15)                     │
├─ Fields ───────────────────────────────────────────┤
│ east-asia.conflict     0.72  ↑0.04                   │
│ europe.stability       0.58  —                      │
│ global.trade_volume    0.63  ↓0.02                   │
│ middle-east.stability  0.31  ↓0.08                   │
│ technology.ai_race     0.81  ↑0.06                   │
├─ Top Events ───────────────────────────────────────┤
│ US carrier group enters SCS              Im:18.2    │
│ Iran nuclear talks resume in Vienna      Im:12.1    │
│ EU announces new chip export framework   Im:10.7    │
└─────────────────────────────────────────────────────┘
```

**History 列表**

```
┌─ Run History ──────────────────────────────────────┐
│  #   date        mode    divergence  dominant_field │
│  42  05-13 14:22 fetch   0.337261    conflict       │
│  41  05-13 13:25 fetch   0.332104    conflict       │
│  40  05-13 12:30 fixture 0.328773    technology     │
├─ Filters ──────────────────────────────────────────┤
│  mode:all  field:all  risk:all  limit:20            │
└─────────────────────────────────────────────────────┘
```

**仿真监控 (predict / backtrack 运行时)**

```
┌─ Simulation ───────────────────────────────────────┐
│ mode: forward  field: east-asia.conflict  round 3/10│
│ progress  ████████░░  30%                           │
├─ Worldline ────────────────────────────────────────┤
│ east-asia.conflict   0.84  ↑0.12                    │
│ global.trade_volume  0.55  ↓0.08                    │
├─ Agents ───────────────────────────────────────────┤
│ China      done      (naval exercise)               │
│ USA        running…                                 │
│ Russia     pending                                  │
└─────────────────────────────────────────────────────┘
```

人工剪枝暂停时，暂停提示覆盖底部区域。

### 实现

- ratatui `Block::bordered()` 细线边框
- Style 硬编码 Kanagawa 色值 (不使用 ratatui 内置 Color enum)
- 配色集中定义在 `tui/theme.rs`
- 事件循环: `crossterm::event::poll(Duration::from_millis(100))` 非阻塞
- 窗口 resize: `Constraint::Percentage` + `Constraint::Min`
- 列表: `List::new()` + `highlight_style` 标记选中行
- 状态栏: `Paragraph::new()` 右对齐快捷键

```rust
// tui/theme.rs
pub const KANAGAWA: Theme = Theme {
    bg:        Color::Rgb(0x1F, 0x1F, 0x28),
    panel_bg:  Color::Rgb(0x27, 0x27, 0x27),
    border:    Color::Rgb(0x36, 0x36, 0x46),
    fg:        Color::Rgb(0xDC, 0xD7, 0xBA),
    label:     Color::Rgb(0x7E, 0x9C, 0xD8),
    value:     Color::Rgb(0xDC, 0xD7, 0xBA),
    up:        Color::Rgb(0x98, 0xBB, 0x6C),
    down:      Color::Rgb(0xE4, 0x68, 0x76),
    warn:      Color::Rgb(0xFF, 0xA0, 0x66),
    status_bg: Color::Rgb(0x36, 0x36, 0x46),
    key_hint:  Color::Rgb(0x93, 0x8A, 0xA9),
    title:     Color::Rgb(0xE6, 0xC3, 0x84),
};
```

---

## 10. 测试策略 (四层)

```
Layer 1: aimock / llmreplay — 录制真实 LLM 响应, CI 中 replay 确定性测试
Layer 2: Snapshot tests — 仿真第 N 轮 world state snapshot, 代码变更后对比
Layer 3: DeepEval — LLM-as-Judge 周期性质量评估
Layer 4: 管线确定性测试 — 无 LLM 路径全量单元 + 集成测试
```

参考: aimock (CopilotKit), DeepEval (confident-ai), llmreplay

---

## 10. 项目结构

```
tianji/
├── Cargo.toml
├── src/
│   ├── main.rs
│   ├── lib.rs
│   ├── models.rs               # Worldline, Event, Profile, ActionProposal...
│   ├── error.rs
│   │
│   ├── cangjie/
│   │   ├── mod.rs
│   │   ├── feed.rs             # RSS/Atom (quick-xml)
│   │   ├── fetch.rs            # HTTP (reqwest)
│   │   ├── normalize.rs        # regex 关键词/actor/region 提取
│   │   └── sources.rs          # source registry + fetch policy
│   │
│   ├── fuxi/
│   │   ├── mod.rs
│   │   ├── worldline.rs        # Worldline 状态机 + Blake3 snapshot
│   │   ├── scoring.rs          # Im/Fa + divergence
│   │   ├── grouping.rs         # 事件分组 + causal ordering
│   │   ├── backtrack.rs        # 干预候选
│   │   ├── triggers.rs         # 阈值/模式检测
│   │   └── dependency.rs       # petgraph field DAG
│   │
│   ├── hongmeng/
│   │   ├── mod.rs              # tokio 运行时 + 子系统启动
│   │   ├── agent_lifecycle.rs  # spawn/kill/pause/resume
│   │   ├── router.rs           # Board/Stick 消息路由
│   │   ├── referee.rs          # World-State Delta 生成
│   │   ├── collision.rs        # 碰撞检测 + 矛盾
│   │   ├── rules.rs            # 自动触发规则
│   │   ├── checkpoint.rs       # SQLite checkpoint 管理
│   │   └── config.rs           # ~/.tianji/config.yaml 加载
│   │
│   ├── nuwa/
│   │   ├── mod.rs
│   │   ├── sandbox.rs          # fork worldline + 隔离
│   │   ├── forward.rs          # 前向多轮 Board/Stick 博弈
│   │   ├── backward.rs         # 后向反推 + 剪枝
│   │   ├── agent.rs            # Agent 执行器: profile + LLM 推理
│   │   ├── profile.rs          # Profile 加载/三层管理
│   │   ├── market.rs           # Market Agent (油价/贸易流)
│   │   └── pruning.rs          # LLM粗筛 + 约束精剪 + 人工暂停
│   │
│   ├── storage.rs              # rusqlite: worldlines, runs, profiles, checkpoints
│   ├── llm.rs                  # LLM 抽象层 (async-openai + ollama-rs + 降级)
│   │
│   ├── cli/                    # clap derive
│   │   ├── mod.rs
│   │   ├── run.rs              # tianji run
│   │   ├── watch.rs            # tianji watch
│   │   ├── predict.rs          # tianji predict
│   │   ├── backtrack.rs        # tianji backtrack
│   │   ├── history.rs          # tianji history/show/compare
│   │   ├── baseline.rs         # tianji baseline --set/--at-run
│   │   ├── daemon.rs           # tianji daemon start/stop/status/resume
│   │   └── tui.rs              # tianji tui
│   │
│   ├── tui/                    # ratatui
│   │   ├── mod.rs
│   │   ├── dashboard.rs        # worldline 总览
│   │   ├── simulation.rs       # 仿真监控 + 人工剪枝
│   │   ├── history.rs          # run 历史
│   │   └── profiles.rs         # profile 浏览
│   │
│   ├── daemon/                 # axum + UNIX socket
│   │   ├── mod.rs
│   │   ├── server.rs           # axum HTTP (loopback)
│   │   ├── socket.rs           # UNIX socket 控制面
│   │   └── jobs.rs             # 后台 job 队列
│   │
│   ├── webui.rs                # axum serve static
│   └── output.rs               # 终端格式化 (tabled + JSON)
│
├── profiles/                   # Actor profile YAML
│   ├── nations/
│   │   ├── china.yaml
│   │   ├── usa.yaml
│   │   ├── russia.yaml
│   │   └── ...
│   ├── organizations/
│   │   ├── nato.yaml
│   │   └── eu.yaml
│   └── corporations/
│       └── tsmc.yaml
│
├── rules/                      # 自动触发规则
│   └── default.yaml
│
├── tianji/webui/               # 静态 Web UI (保留)
├── tests/
│   ├── fixtures/sample_feed.xml
│   ├── test_pipeline.rs
│   ├── test_scoring.rs
│   ├── test_worldline.rs
│   ├── test_nuwa_forward.rs
│   ├── test_nuwa_backward.rs
│   ├── test_agent_pruning.rs
│   └── test_checkpoint.rs
├── plan.md
└── README.md
```

---

## 11. 依赖清单 (版本已对齐最新)

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
blake3 = "1"                    # 新增: worldline snapshot hash

# HTTP
reqwest = { version = "0.12", features = ["rustls-tls"], default-features = false }
axum = "0.8"

# 异步
tokio = { version = "1", features = ["full"] }

# 持久化
rusqlite = { version = "0.32", features = ["bundled"] }

# TUI
ratatui = "0.30"                # 更正: 0.29 → 0.30
crossterm = "0.28"

# 输出
tabled = "0.18"

# LLM
async-openai = "0.34"           # 更正: 0.27 → 0.34
ollama-rs = "0.3"               # 确认: 优于 ollama-rest

# 图
petgraph = "0.7"

# 确定性 PRNG
rand = "0.8"
rand_chacha = "0.3"             # 新增: 确定性仿真

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

## 12. 开发阶段

### Phase 1: Worldline 核心 + 管线
- models.rs: 所有数据结构
- cangjie/: feed, normalize, fetch
- fuxi/: worldline 状态机, scoring, grouping, backtrack, triggers, dependency DAG
- storage.rs: SQLite schema + Blake3 snapshot
- CLI: `tianji run`, `tianji history`
- 验证: 输出与 Python 版 JSON 字段级对齐

### Phase 2: Hongmeng 编排层
- tokio actor 模型 + Board/Stick 消息路由
- Agent 生命周期 + Referee 生成
- 碰撞检测 + 收敛条件
- Checkpoint 管理 + 崩溃恢复
- 自动触发规则引擎
- CLI: `tianji watch`, `tianji baseline`

### Phase 3: Nuwa 仿真沙盒
- sandbox: worldline fork + 隔离
- agent: profile 加载 + LLM 推理 (三层 profile)
- forward: 多轮 Board/Stick 博弈
- market: Market Agent (油价/贸易流)
- backward: 后向反推 + 剪枝引擎
- CLI: `tianji predict`, `tianji backtrack`

### Phase 4: TUI
- dashboard: worldline 状态总览
- simulation: 仿真监控 + 人工剪枝交互
- history: run 历史 + 分支对比
- profiles: Actor profile 三层浏览

### Phase 5: Daemon + Web UI
- axum HTTP API + UNIX socket
- 后台 job 队列 + 自动恢复
- LLM provider 配置加载
- static Web UI serve

### Phase 6: 清理 + 文档
- 删除所有 Python 代码
- 删除 `.venv/` `.agents/` `.codex/` `.gemini/`
- 更新 README
- shell completions (clap generate)

---

## 13. 删除清单

- 所有 Python 代码: `tianji/*.py` `tests/*.py` `pyproject.toml` `uv.lock`
- `.venv/` `.pytest_cache/` `__pycache__/`
- `.agents/` `.codex/` `.gemini/` (保留 `.opencode/` 中有用的)
- `node_modules/` (`.opencode/` 内需要的保留)
- `dummy.sqlite3`

---

## 14. 关键参考仓库

| 仓库 | 用于 | 语言 |
|------|------|------|
| agiresearch/WarAgent | Board/Stick 分层信息公开 | Python |
| danielrosehill/Geopol-Forecaster | 两阶段仿真 + Referee 模式 | Python |
| prithwis/Centaur | ZeitWorld/Centaur/Chanakya 三组件 | Python |
| Peakstone-Labs/hormuz-agent-sandbox | 4 国 multi-agent 实时仿真 | Vue+Python |
| in6black/seldon-vault | 11 分析师 Hawk/Dove 对偶 | Python |
| dx111ge/intel-analyst | Bayesian + WASM Rust 概率引擎 | Rust+JS |
| langchain-ai/langgraph | Checkpoint + state machine | Python |
| tachyon-beep/murk | Tick 引擎 + 确定性 replay | Rust |
| multikernel/branching | COW fork + 多分支管理 | Python |
| adk-rust/adk-graph | Rust LangGraph + durable resume | Rust |
| CopilotKit/aimock | LLM mock 确定性测试 | TS |
| confident-ai/deepeval | LLM 质量评估 | Python |

---

## 15. 验证标准

- `cargo build --release` 零 warning
- `cargo test` 全绿 (含四层测试策略)
- `tianji run --fixture ...` 输出与 Python 版字段级一致
- `tianji predict --field east-asia.conflict --horizon 30d` → Vec<WorldlineBranch>
- `tianji backtrack --goal "东亚稳定" --max-interventions 5` → Vec<InterventionPath>
- 人工剪枝: 仿真暂停 → TUI 选项 → 选择继续 → 完成
- Checkpoint: 仿真中 kill 进程 → daemon resume → 从断点继续
- 单二进制 < 25MB release
