# TianJi × Crucix — Delta Engine 移植设计文档

> 参考: `/home/kita/code/Crucix/lib/delta/engine.mjs` + `memory.mjs`
> 版本: v1 | 日期: 2026-05-13
> 分支: `rust-cli` | 目标: 为 TianJi 加入跨 run 变化追踪 + 多级告警衰减

---

## 1. 背景与动机

TianJi 当前 pipeline 是**单次 run 的静态评估**：一个 feed → scoring → backtrack → 输出 artifact。每次 run 独立存在，history-compare 能对比两个 run，但缺少**持续监控视角**。

Crucix 的 Delta Engine 解决了这个问题：每次 sweep 后，自动与上一次 run 做结构化 diff，将变化分类为 new / escalated / deescalated / unchanged，按严重度分级，并输出整体方向判断。

**目标**: 将 Crucix Delta Engine 算法翻译为 Rust，作为 TianJi 的跨 run 分析层，介于 storage (已有 SQLite) 和 notification/输出层之间。

---

## 2. 核心概念映射

| Crucix 概念 | TianJi 对应 | 说明 |
|------------|------------|------|
| sweep | run | 一次完整 pipeline 执行 |
| delta | DeltaReport | 两次 run 之间的结构化变化 |
| metric (numeric) | NumericMetric | 可量化的指标 (impact_score, divergence_score, field_attraction) |
| metric (count) | CountMetric | 计数型指标 (actor_count, group_count, intervention_count) |
| signals (new/escalated/deescalated) | DeltaSignal | 分类信号向量 |
| RISK_KEYS | risk_keys | 跨域风险敏感键集合 |
| direction (risk-off/risk-on/mixed) | direction | 整体风险方向 |
| alert tier (FLASH/PRIORITY/ROUTINE) | AlertTier | 通知分级 |
| alert decay | AlertDecayModel | 告警衰减冷却 |
| memory (hot/cold) | DeltaMemory | 热/冷双层存储 |

---

## 3. Rust 数据结构

### 3.1 核心类型

```rust
// ─── Metric Definitions ───────────────────────────────────────────

/// 数值型指标 — 检测百分比变化
pub struct NumericMetricDef {
    /// 唯一标识 (如 "impact_score", "divergence_score")
    pub key: &'static str,
    /// 人类可读标签
    pub label: &'static str,
    /// 变化阈值 (%) — 超过此值才产生信号
    pub threshold_pct: f64,
    /// 是否为风险敏感项 (影响整体方向推断)
    pub risk_sensitive: bool,
}

/// 计数型指标 — 检测绝对数量变化
pub struct CountMetricDef {
    pub key: &'static str,
    pub label: &'static str,
    /// 最小变化量 — 低于此值视为噪声
    pub threshold_abs: i64,
    pub risk_sensitive: bool,
}

// ─── Extracted Metric Values ──────────────────────────────────────

/// 一次 run 中提取的指标快照
pub struct MetricSnapshot {
    pub numerics: BTreeMap<String, f64>,
    pub counts: BTreeMap<String, i64>,
}

// ─── Delta Signals ────────────────────────────────────────────────

pub struct NumericDelta {
    pub key: String,
    pub label: String,
    pub from: f64,
    pub to: f64,
    pub pct_change: f64,
    pub direction: DeltaDirection,
    pub severity: Severity,
}

pub struct CountDelta {
    pub key: String,
    pub label: String,
    pub from: i64,
    pub to: i64,
    pub change: i64,
    pub pct_change: f64,
    pub direction: DeltaDirection,
    pub severity: Severity,
}

pub struct NewSignal {
    pub key: String,
    pub label: String,
    pub reason: String,
    pub severity: Severity,
}

pub enum DeltaDirection {
    Escalated,    // 数值上升 / 数量增加
    Deescalated,  // 数值下降 / 数量减少
}

pub enum Severity {
    Critical,     // > 3x 阈值
    High,         // > 2x 阈值
    Moderate,     // > 1x 阈值
}

// ─── Top-Level Delta Report ───────────────────────────────────────

pub struct DeltaReport {
    pub timestamp: String,              // 当前 run 时间戳
    pub previous_timestamp: Option<String>, // 上一 run 时间戳
    pub numeric_deltas: Vec<NumericDelta>,
    pub count_deltas: Vec<CountDelta>,
    pub new_signals: Vec<NewSignal>,
    pub summary: DeltaSummary,
}

pub struct DeltaSummary {
    pub total_changes: usize,
    pub critical_changes: usize,
    pub direction: RiskDirection,
    pub signal_breakdown: SignalBreakdown,
}

pub struct SignalBreakdown {
    pub new_count: usize,
    pub escalated_count: usize,
    pub deescalated_count: usize,
    pub unchanged_count: usize,
}

pub enum RiskDirection {
    RiskOff,   // 风险指标净上升
    RiskOn,    // 风险指标净下降
    Mixed,     // 信号混杂
}

// ─── Alert Tier & Decay ───────────────────────────────────────────

pub enum AlertTier {
    Flash,     // 立即行动 — cooling 5min, max 6/hr
    Priority,  // 数小时内行动 — cooling 30min, max 4/hr
    Routine,   // FYI — cooling 60min, max 2/hr
}

impl AlertTier {
    pub fn cooldown_secs(&self) -> u64 {
        match self {
            Self::Flash => 5 * 60,
            Self::Priority => 30 * 60,
            Self::Routine => 60 * 60,
        }
    }

    pub fn max_per_hour(&self) -> usize {
        match self {
            Self::Flash => 6,
            Self::Priority => 4,
            Self::Routine => 2,
        }
    }
}

/// 告警衰减模型 — 同信号重复出现时递增冷却
pub struct AlertDecayModel {
    /// 衰减阶梯 (小时): [0, 6, 12, 24]
    /// 第 1 次: 0h → 立即告警
    /// 第 2 次: 6h 冷却
    /// 第 3 次: 12h 冷却
    /// 第 4+ 次: 24h 冷却
    pub decay_tiers_hours: Vec<u64>,
    /// 修剪策略: 1 次出现的信号 N 小时后过期
    pub prune_single_hours: u64,
    /// 修剪策略: 2+ 次出现的信号 N 小时后过期
    pub prune_repeat_hours: u64,
}

impl Default for AlertDecayModel {
    fn default() -> Self {
        Self {
            decay_tiers_hours: vec![0, 6, 12, 24],
            prune_single_hours: 24,
            prune_repeat_hours: 48,
        }
    }
}

impl AlertDecayModel {
    /// 根据出现次数计算冷却时长 (秒)
    pub fn cooldown_for_count(&self, occurrence_count: usize) -> u64 {
        let idx = occurrence_count.saturating_sub(1)
            .min(self.decay_tiers_hours.len() - 1);
        self.decay_tiers_hours[idx] * 3600
    }
}
```

### 3.2 TianJi 特化指标

TianJi 的四域 scoring 体系有特化的指标定义：

```rust
/// TianJi 默认数值指标
pub const TIANJI_NUMERIC_METRICS: &[NumericMetricDef] = &[
    NumericMetricDef {
        key: "top_impact_score",
        label: "Top Impact Score",
        threshold_pct: 20.0,     // ±20% 变化才产生信号
        risk_sensitive: true,
    },
    NumericMetricDef {
        key: "top_divergence_score",
        label: "Top Divergence Score",
        threshold_pct: 15.0,
        risk_sensitive: true,
    },
    NumericMetricDef {
        key: "top_field_attraction",
        label: "Top Field Attraction",
        threshold_pct: 25.0,
        risk_sensitive: false,
    },
    NumericMetricDef {
        key: "avg_impact_score",
        label: "Avg Impact Score",
        threshold_pct: 30.0,
        risk_sensitive: true,
    },
    NumericMetricDef {
        key: "avg_divergence_score",
        label: "Avg Divergence Score",
        threshold_pct: 20.0,
        risk_sensitive: true,
    },
];

/// TianJi 默认计数指标
pub const TIANJI_COUNT_METRICS: &[CountMetricDef] = &[
    CountMetricDef {
        key: "scored_event_count",
        label: "Scored Events",
        threshold_abs: 3,    // ±3 个事件才产生信号
        risk_sensitive: true,
    },
    CountMetricDef {
        key: "intervention_candidate_count",
        label: "Intervention Candidates",
        threshold_abs: 2,
        risk_sensitive: true,
    },
    CountMetricDef {
        key: "event_group_count",
        label: "Event Groups",
        threshold_abs: 2,
        risk_sensitive: true,
    },
    CountMetricDef {
        key: "unique_actor_count",
        label: "Unique Actors",
        threshold_abs: 3,
        risk_sensitive: true,
    },
    CountMetricDef {
        key: "unique_region_count",
        label: "Unique Regions",
        threshold_abs: 2,
        risk_sensitive: false,
    },
];
```

---

## 4. 算法流程

### 4.1 主入口: `compute_delta()`

```
compute_delta(current_run, previous_run) → DeltaReport | None

如果 previous_run 为 None (首次运行): 返回 None

Step 1: 提取数值指标快照
  for each NumericMetricDef:
    cur = extract(current_run, metric_def)  // 从 RunArtifact 提取
    prev = extract(previous_run, metric_def)
    if cur 或 prev 为 null: skip
    pct_change = (cur - prev) / |prev| * 100
    if |pct_change| > threshold:
      严重度 = 判定严重度(|pct_change|, threshold)
      方向 = pct_change > 0 ? Escalated : Deescalated
      推入 numeric_deltas 或 escalated/deescalated

Step 2: 提取计数指标快照
  for each CountMetricDef:
    cur = extract(current_run, metric_def)
    prev = extract(previous_run, metric_def)
    diff = cur - prev
    if |diff| >= threshold:
      严重度 = 判定严重度(|diff|, threshold)
      方向 = diff > 0 ? Escalated : Deescalated
      推入 count_deltas

Step 3: 检测新事件
  - 跨 run ID 集合比对: new_event_ids = current_ids - previous_ids
  - 新事件组 (上一个 run 中不存在的 event_group): 推入 new_signals
  - 新干预候选 (上一个 run 中不存在的 intervention): 推入 new_signals
  - 主导域变化 (dominant_field 从 A 变为 B): 推入 new_signals

Step 4: 计算总览
  total_changes = new_signals.len() + escalated.len() + deescalated.len()
  critical_changes = count of severity=Critical 的信号
  direction = 推断方向(escalated, deescalated, risk_keys)
    - risk_up = escalated 中 risk_sensitive 的数量
    - risk_down = deescalated 中 risk_sensitive 的数量
    - risk_up > risk_down + 1 → RiskOff
    - risk_down > risk_up + 1 → RiskOn
    - 否则 → Mixed
```

### 4.2 严重度判定

```rust
fn severity_for_numeric(pct_change_abs: f64, threshold: f64) -> Severity {
    let ratio = pct_change_abs / threshold;
    if ratio > 3.0 { Severity::Critical }
    else if ratio > 2.0 { Severity::High }
    else { Severity::Moderate }
}

fn severity_for_count(change_abs: i64, threshold: i64) -> Severity {
    let ratio = change_abs as f64 / threshold as f64;
    if ratio > 5.0 { Severity::Critical }
    else if ratio > 2.0 { Severity::High }
    else { Severity::Moderate }
}
```

### 4.3 语义去重 (信号哈希)

类似 Crucix 的 `contentHash` + `stablePostKey`。

TianJi 的事件/干预信号更结构化，去重基于以下层次：

```rust
/// 信号的稳定标识键 — 三层 fallback
pub enum SignalIdentity {
    /// 首选: run_id + event_id (精确唯一)
    EventKey { run_id: i64, event_id: String },
    /// 次选: hash(event_id + dominant_field + actors)
    ContentHash(String),
    /// 兜底: hash(title + dominant_field)
    SemanticHash(String),
}

impl SignalIdentity {
    pub fn for_scored_event(run_id: i64, event: &ScoredEvent) -> Self {
        SignalIdentity::EventKey {
            run_id,
            event_id: event.event_id.clone(),
        }
    }

    /// 跨 run 去重: 构建 identity hash，用于阻止同一信号重复告警
    pub fn cross_run_dedup_key(event: &ScoredEvent) -> String {
        use sha2::{Digest, Sha256};
        let payload = format!(
            "{}|{}|{}|{:?}",
            event.event_id,
            event.dominant_field,
            event.title.chars().take(80).collect::<String>(),
            event.actors.iter().take(3).cloned().collect::<Vec<_>>(),
        );
        hex::encode(Sha256::digest(payload.as_bytes()))
    }
}
```

---

## 5. DeltaMemory — 热/冷双层存储

### 5.1 设计

```
runs/
├── memory/
│   ├── hot.json          # 最近 3 个 run 的 compacted data + delta
│   ├── hot.json.bak      # 崩溃恢复备份
│   └── cold/
│       ├── 2026-05-10.json
│       ├── 2026-05-11.json
│       └── ...
```

### 5.2 Hot memory 结构

```rust
/// Hot memory — 最近 3 个 run 的快照
pub struct HotMemory {
    pub runs: VecDeque<HotRunEntry>,  // 最新在前, max 3
    pub alerted_signals: BTreeMap<String, AlertedSignalEntry>,
}

pub struct HotRunEntry {
    pub timestamp: String,
    pub run_id: i64,
    /// Compacted 数据 (完整 RunArtifact 的子集)
    pub compact: CompactRunData,
    /// 本次 run 相对前一次的 delta (第一个 run 的 delta 为 None)
    pub delta: Option<DeltaReport>,
}

/// 已告警信号追踪
pub struct AlertedSignalEntry {
    pub first_seen: String,       // ISO 时间戳
    pub last_alerted: String,     // ISO 时间戳
    pub count: usize,             // 出现次数
}

/// 压缩的 run 数据 — 只保留 delta 计算所需的字段
pub struct CompactRunData {
    pub meta: CompactMeta,
    pub field_summary: BTreeMap<String, FieldCompact>,
    pub top_event_ids: Vec<String>,
    pub top_actor_ids: Vec<String>,
    pub top_region_ids: Vec<String>,
    pub group_ids: Vec<String>,
}

pub struct CompactMeta {
    pub run_id: i64,
    pub mode: String,
    pub generated_at: String,
    pub dominant_field: String,
    pub risk_level: String,
}

pub struct FieldCompact {
    pub dominant_field: String,
    pub top_impact_score: f64,
    pub top_divergence_score: f64,
    pub top_field_attraction: f64,
    pub event_count: usize,
}
```

### 5.3 原子写入

```rust
impl HotMemory {
    /// 原子写入: write to .tmp → rename to target, keep .bak
    pub fn save_atomic(&self, path: &Path) -> Result<(), std::io::Error> {
        let tmp_path = path.with_extension("json.tmp");
        let bak_path = path.with_extension("json.bak");

        // 1. 写临时文件
        let json = serde_json::to_string_pretty(self)?;
        std::fs::write(&tmp_path, &json)?;

        // 2. 备份当前文件 (如果存在)
        if path.exists() {
            let _ = std::fs::rename(path, &bak_path);
        }

        // 3. 原子 rename: .tmp → hot.json
        std::fs::rename(&tmp_path, path)?;

        Ok(())
    }

    /// 加载 hot memory，尝试主文件 + 备份
    pub fn load(path: &Path) -> Self {
        for candidate in [path, &path.with_extension("json.bak")] {
            if let Ok(raw) = std::fs::read_to_string(candidate) {
                if let Ok(hot) = serde_json::from_str::<Self>(&raw) {
                    return hot;
                }
            }
        }
        // 无有效文件 → 返回空结构
        Self::default()
    }
}
```

### 5.4 衰减冷却判定

```rust
impl HotMemory {
    /// 检查信号是否在冷却期内
    /// 返回 true = 应抑制 (仍在冷却)
    pub fn is_signal_suppressed(
        &self,
        signal_key: &str,
        decay: &AlertDecayModel,
    ) -> bool {
        let entry = match self.alerted_signals.get(signal_key) {
            Some(e) => e,
            None => return false, // 从未告警 → 不抑制
        };

        let cooldown_secs = decay.cooldown_for_count(entry.count);
        let last_alerted = chrono::DateTime::parse_from_rfc3339(&entry.last_alerted)
            .ok()
            .map(|dt| dt.timestamp())
            .unwrap_or(0);

        let now = chrono::Utc::now().timestamp();
        (now - last_alerted) < cooldown_secs as i64
    }

    /// 标记信号已告警
    pub fn mark_alerted(&mut self, signal_key: &str) {
        let now = chrono::Utc::now().to_rfc3339();
        self.alerted_signals
            .entry(signal_key.to_string())
            .and_modify(|e| {
                e.count += 1;
                e.last_alerted = now.clone();
            })
            .or_insert(AlertedSignalEntry {
                first_seen: now.clone(),
                last_alerted: now,
                count: 1,
            });
    }

    /// 修剪过期告警信号
    pub fn prune_stale_signals(&mut self, decay: &AlertDecayModel) {
        let now = chrono::Utc::now().timestamp();
        self.alerted_signals.retain(|_, entry| {
            let last = chrono::DateTime::parse_from_rfc3339(&entry.last_alerted)
                .ok()
                .map(|dt| dt.timestamp())
                .unwrap_or(0);
            let max_age_hours = if entry.count >= 2 {
                decay.prune_repeat_hours
            } else {
                decay.prune_single_hours
            };
            (now - last) < (max_age_hours * 3600) as i64
        });
    }
}
```

---

## 6. 与 TianJi 现有代码的整合点

### 6.1 整合位置

```
pipeline (lib.rs)
  │
  ├─ run_fixture_path()         ← 现有 entry point
  │   ├─ parse_feed → normalize → score → group → backtrack
  │   ├─ persist_run()           ← 已有 SQLite 持久化
  │   └─ [NEW] compute_and_store_delta()  ← 新增
  │
  ├─ DeltaEngine (新模块 src/delta.rs)
  │   ├─ compute_delta()
  │   ├─ MetricSnapshot::from_artifact()
  │   └─ DeltaReport
  │
  ├─ DeltaMemory (新模块 src/delta_memory.rs)
  │   ├─ HotMemory::load/save
  │   ├─ ColdStorage::archive
  │   └─ AlertDecayModel
  │
  └─ Notification tiering (src/daemon.rs 或新模块)
      ├─ classify_delta_tier(DeltaReport) → AlertTier
      └─ send_notification(AlertTier, message)
```

### 6.2 Cargo.toml 新增依赖

```toml
[dependencies]
# ... existing deps ...
chrono = "0.4"       # 时间处理 (已有 serde feature)
hex = "0.4"          # SHA256 hex encoding
```

不需要额外的大型依赖。`sha2`、`serde`、`serde_json`、`rusqlite` 已在项目中。

### 6.3 lib.rs 模块声明

```rust
// src/lib.rs — 新增
pub mod delta;
pub mod delta_memory;

// 新增导出
pub use delta::{compute_delta, DeltaReport, DeltaSummary, RiskDirection, Severity};
pub use delta_memory::{AlertDecayModel, HotMemory, AlertedSignalEntry};
```

### 6.4 集成到 run_fixture_path

在 `lib.rs` 的 `run_fixture_path()` 末尾 (persist_run 之后) 插入：

```rust
// 计算并存储 delta (如果有前一次 run)
if let Some(db_path) = sqlite_path {
    // 已有: persist_run(...)

    // 新增: delta 计算
    let previous_id = get_previous_run_id(db_path, current_run_id)?;
    if let Some(prev_id) = previous_id {
        let prev_artifact = load_run_artifact(db_path, prev_id)?;
        let current_snapshot = MetricSnapshot::from_artifact(&artifact);
        let prev_snapshot = MetricSnapshot::from_artifact(&prev_artifact);
        let delta = compute_delta(
            &current_snapshot,
            &prev_snapshot,
            &TIANJI_NUMERIC_METRICS,
            &TIANJI_COUNT_METRICS,
            &artifact,
            &prev_artifact,
        );
        if let Some(report) = delta {
            // 写入 DeltaReport 到 SQLite 或 JSON 文件
            store_delta_report(db_path, report)?;

            // 更新 hot memory
            let memory_path = delta_memory_path(db_path);
            let mut hot = HotMemory::load(&memory_path);
            hot.push_run(compact_run_data(&artifact), Some(report));
            hot.prune_stale_signals(&AlertDecayModel::default());
            hot.save_atomic(&memory_path)?;
        }
    }
}
```

### 6.5 CLI 新增命令

```rust
// main.rs Cli enum 新增
/// Show delta between latest runs (or specific run pair)
Delta {
    #[arg(long = "sqlite-path")]
    sqlite_path: String,
    /// Show delta for the latest N runs
    #[arg(long = "latest", default_value_t = 2)]
    latest: usize,
    /// Specific left run ID
    #[arg(long = "left-run-id")]
    left_run_id: Option<i64>,
    /// Specific right run ID
    #[arg(long = "right-run-id")]
    right_run_id: Option<i64>,
},
```

对应的 handler 复用 DeltaReport 的 JSON 序列化输出。

---

## 7. 配置设计

### 7.1 配置源优先级

```
1. CLI flags (最高优先级)
2. 环境变量 (TIANJI_DELTA_*)
3. crucix.config.mjs 风格的配置文件 (可选, 未来)
4. 代码中的默认常量 (最低优先级)
```

### 7.2 配置结构

```rust
pub struct DeltaConfig {
    /// 数值指标阈值覆盖
    pub numeric_thresholds: BTreeMap<String, f64>,
    /// 计数指标阈值覆盖
    pub count_thresholds: BTreeMap<String, i64>,
    /// 告警衰减模型
    pub alert_decay: AlertDecayModel,
    /// hot memory 保 run 数
    pub hot_run_count: usize,
    /// 是否在 daemon 模式下自动推送 delta 通知
    pub auto_notify: bool,
}

impl Default for DeltaConfig {
    fn default() -> Self {
        Self {
            numeric_thresholds: BTreeMap::new(),
            count_thresholds: BTreeMap::new(),
            alert_decay: AlertDecayModel::default(),
            hot_run_count: 3,
            auto_notify: true,
        }
    }
}
```

### 7.3 默认阈值

TianJi 的默认阈值基于 Crucix 的经验值，调整为地缘政治分析场景：

| 指标 | 类型 | 阈值 | 说明 |
|------|------|------|------|
| top_impact_score | numeric | ±20% | 单次 run 的最高 impact 变化 |
| top_divergence_score | numeric | ±15% | divergence 的显著变化不需要超过 20% |
| top_field_attraction | numeric | ±25% | field attraction 波动较大 |
| avg_impact_score | numeric | ±30% | 均值更平滑，阈值放宽 |
| avg_divergence_score | numeric | ±20% | |
| scored_event_count | count | ±3 | 少于 3 个事件变化忽略 |
| intervention_candidate_count | count | ±2 | 干预建议的变化值得注意 |
| event_group_count | count | ±2 | |
| unique_actor_count | count | ±3 | |
| unique_region_count | count | ±2 | |

---

## 8. AlertTier 分级逻辑

将 DeltaReport 映射到告警层级：

```rust
pub fn classify_delta_tier(delta: &DeltaReport, config: &DeltaConfig) -> Option<AlertTier> {
    let summary = &delta.summary;

    // FLASH: 有 critical_changes 且方向明确
    if summary.critical_changes >= 2 && summary.direction == RiskDirection::RiskOff {
        return Some(AlertTier::Flash);
    }
    if summary.critical_changes >= 3 {
        return Some(AlertTier::Flash);
    }

    // PRIORITY: 有意义的变化 (至少 1 个 critical 或 3+ 个变化)
    if summary.critical_changes >= 1 {
        return Some(AlertTier::Priority);
    }
    if summary.total_changes >= 3 {
        return Some(AlertTier::Priority);
    }

    // ROUTINE: 有变化但不紧急
    if summary.total_changes >= 1 {
        return Some(AlertTier::Routine);
    }

    // 零变化 → 不推送
    None
}
```

---

## 9. 与现有 storage 的关系

Delta 计算**复用但不侵入**现有 SQLite storage。

**读取路径**:
- `get_run_summary()` — 已有，用于提取 scorered_events / groups / interventions
- 新增 `load_run_artifact_full()` — 如果需要完整 RunArtifact (当前 storage 存储 JSON blob)

**写入路径**:
- `persist_run()` — 不变，继续写 6 张表
- Delta 数据**单独存储**:
  - Hot memory: 文件系统 `runs/memory/hot.json` (JSON, 原子写入)
  - Cold archive: 文件系统 `runs/memory/cold/YYYY-MM-DD.json`
  - 也可以扩展 SQLite 加一张 `deltas` 表，但 JSON 文件更简单且符合 Crucix 模式

**为什么不用 SQLite 存 delta**:
- Delta 是「最近 3 个 run」的快速访问层，JSON 文件直接加载比 SQL 查询更快
- 冷归档按日期拆分，文件系统天然索引
- 不改动现有 6 表 schema

---

## 10. 迁移路径

### Phase 1: 核心算法 + 无侵入集成 (本周可完成)

1. 创建 `src/delta.rs` — `compute_delta()` 纯函数
2. 创建 `src/delta_memory.rs` — `HotMemory` + 原子 I/O
3. 在 `lib.rs` 的 `run_fixture_path()` 末尾插入 delta 计算 (不改变现有返回)
4. 添加 CLI `delta` 子命令
5. 添加 `lib.rs` 测试: 两轮 fixture run → 验证 delta 输出

### Phase 2: daemon 自动 delta (下周)

6. daemon worker 在每次 run 后自动计算 delta
7. 将 delta 嵌入 daemon 的 run status 响应中
8. 添加 `AlertTier` 分级 + webui 显示

### Phase 3: 告警通知 (按需)

9. Telegram/Discord webhook 推送 (如果 tianji 需要)
10. AlertDecay 衰减 + 信号修剪

---

## 11. 参考

- Crucix Delta Engine: `/home/kita/code/Crucix/lib/delta/engine.mjs` (251 行)
- Crucix Memory Manager: `/home/kita/code/Crucix/lib/delta/memory.mjs` (244 行)
- Crucix Alert Tier: `/home/kita/code/Crucix/lib/alerts/telegram.mjs` (Tier config 部分)
- TianJi plan.md: `/home/kita/code/tianji/plan.md` (876 行, 四子系统架构)
- TianJi storage.rs: `/home/kita/code/tianji/src/storage.rs` (1437 行, 6 表 SQLite)

---

## 12. 风险与注意事项

- **Delta 不是 Worldline divergence**: 现有 `divergence_score` 是单 run 内事件偏离基线的度量。Delta 是跨 run 的变化追踪。两者互补，不替换。
- **指标选择需迭代**: 初始的 `TIANJI_NUMERIC_METRICS` / `TIANJI_COUNT_METRICS` 是经验猜测，实际使用后应根据噪声水平调整阈值。
- **Hot memory 损坏**: `hot.json` 如果损坏，`hot.json.bak` 是恢复路径。最坏情况是丢失 delta 历史但不影响核心 pipeline。
- **不需要新增 Rust 依赖**: 除了 `chrono` (如果时间处理的 serde 不够用) 或 `hex` (更干净的 hex encode)，当前依赖完全够用。
