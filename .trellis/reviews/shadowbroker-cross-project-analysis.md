# ShadowBroker → TianJi 借鉴分析

> 来源: https://github.com/BigBodyCobain/Shadowbroker.git (v0.9.7)
> 类型: 实时 OSINT 地理空间情报平台 (Next.js + MapLibre + FastAPI/Python + Rust)
> 审查日期: 2026-05-17

---

## 1. 项目概览

ShadowBroker 聚合 60+ 公开数据源（航班、船舶、卫星、GDELT 冲突、地震、火灾、
CCTV、GPS 干扰、SDR 无线电等），通过 Next.js + MapLibre GL 前端渲染到一张
地图上。后端 FastAPI 用 APScheduler 分快/慢两个 tier 拉取数据。另有一个
InfoNet 去中心化情报网格（Wormhole relay + Ed25519 签名 + 治理层 Sovereign
Shell）和一个 HMAC 签名的 AI Agent 命令通道。

代码分布:
- 前端: Next.js + TypeScript (MapLibre, 35+ 图层, 5 种视觉模式)
- 后端: FastAPI Python (未公开在 repo — Docker 镜像分发)
- 隐私核心: Rust crate (privacy-core, ~2200 行) — MLS 群组加密 + FFI
- Agent 技能: Python OpenClaw skill (HMAC 签名协议)
- Mesh 脚本: Node.js (测试网节点运维)

---

## 2. 对 TianJi 可借鉴的点

### 2.1 数据源分层 (Fast/Slow Tier)

ShadowBroker 将 60+ 数据源分为两个 tier:
- Fast tier (每 15-30s): 航班、船舶、卫星、CCTV、GPS 干扰
- Slow tier (每 5-15min): GDELT、新闻、地震、火灾、空气质量

TianJi 的 watch 模式目前所有 feed 同一刷新周期。可借鉴:
- 按 feed 类型分组，高频源短周期，低频源长周期
- 这可以降低 API 调用成本和 LLM token 消耗（Hongmeng 仿真不需要
  每 30 秒看一次地震数据）

具体实现: `DaemonConfig` 添加 `fast_interval_secs` / `slow_interval_secs`

### 2.2 Agent AI 命令通道协议

ShadowBroker 的 Agent 协议设计精湛，TianJi 的 Hongmeng Agent 通信可以借鉴:

```
POST /api/ai/channel/command   {cmd, args}          → 单个工具调用
POST /api/ai/channel/batch     [{cmd, args}, ...]    → 最多 20 个并发调用

认证: X-SB-Timestamp + X-SB-Nonce + X-SB-Signature
签名: HMAC-SHA256(secret, METHOD|path|ts|nonce|sha256(body))

Tier 控制: restricted (只读) / full (读写+注入)
Discovery: GET /api/ai/channel → {available_commands, tier, reason}
```

TianJi 的 Hongmeng Agent 目前通过内部 Rust API 调用 `pick_llm_action`。
如果要支持外部 Agent（如 Hermes 作为仿真参与者），可以:
- 实现类似的 HMAC 签名通道
- TianJi daemon 暴露 `/api/v1/agent/command` 端点
- Agent 可以读取 worldline 状态、提交 action、接收 board 消息

优先级: 中期。当前内部 Agent 调用已够用，外部集成是锦上添花。

### 2.3 告警分发系统

ShadowBroker 的 `AlertDispatcher` 支持多通道分发:
- Discord webhook (2000 字符限制，自动分块)
- Telegram bot (4096 字符限制，Markdown→纯文本 fallback)
- 通用 webhook (JSON POST)

每种告警有品牌签名 (brief/warning/threat/news/intel)。

TianJi 的 `AlertTier` (Flash/Priority/Routine) 目前只影响 HotMemory 存储。
可借鉴:
- 添加 `AlertDispatcher` 模块，支持 Discord/Telegram webhook
- Flash → 立即推送所有通道，Priority → 仅 Discord，Routine → 日志
- 配置在 `~/.tianji/config.yaml` 中

具体实现: 新建 `src/alert_dispatch.rs`，集成 reqwest (已有)
优先级: 高。这是用户明确关注的功能（TG 频道监控）。

### 2.4 快照/时间线回放

ShadowBroker 的 Time Machine:
- 每小时索引快照 (count, latest_id, latest_ts, snapshot_ids)
- 移动实体（航班/船舶）帧间插值
- 可变播放速度
- ETag 增量更新

TianJi 已有 HotMemory + Delta Engine 做跨 run 变化追踪。可借鉴:
- 为每个 persist_run 生成 snapshot 索引
- TUI 增加时间线回放模式：左右箭头切换历史 run，逐 tick 播放 field 变化
- 类似 `tianji tui --replay` 模式

优先级: 低。当前 history/compare 命令已覆盖基本需求。

### 2.5 类型化数据模型 (TypeScript → Rust)

ShadowBroker 前端有完整的 TypeScript 类型定义:

```typescript
// ~1100 行类型定义，覆盖所有实体
export interface Flight { callsign, lat, lng, alt, heading, ... }
export interface Ship { mmsi, name, type, lat, lng, sog, ... }
export interface Satellite { norad_id, name, mission, lat, lng, alt_km, ... }
export interface GDELTIncident { event_date, actors, goldstein, avg_tone, ... }
export interface Earthquake { id, mag, lat, lng, place }
// ... 40+ 类型
```

这是 TianJi 解决 H8 (serde_json::Value 过度使用) 的最佳参考:
- 为每个实体类型定义 Rust struct with Serialize/Deserialize
- 替代 `Vec<Value>` 为 `Vec<ScoredEvent>`
- 类型安全 + 编译期检查 + IDE 自动补全

TianJi 当前最大的技术债务就是管线全程 Value → 强类型的迁移。

### 2.6 Rust 隐私核心的 FFI 模式

ShadowBroker 的 `privacy-core` 使用 handle-based FFI:
- 所有对象通过 `u64` handle 引用
- 内部用 `OnceLock<Mutex<HashMap<Handle, State>>>` 全局状态
- `ByteBuffer` 结构体传递跨语言二进制数据
- 原子计数器分配 handle

TianJi 目前没有 FFI 需求，但如果有跨语言集成场景（Python oracle 替换时
的过渡期），这个模式可以直接复用。

### 2.7 分析区域 (Analysis Zones)

ShadowBroker 的 Agent 可以在地图上放置分析区域:
- 类别: contradiction/warning/observation/hypothesis/analysis
- 严重程度: high/medium/low → 填充透明度
- cell_size_deg: 0.3-5.0 表示城市/区域/战略级别
- 包含 drivers 字段列出证据链

TianJi 的 Hongmeng Agent 仿真中，Agent 的决策理由目前只在 `AgentAction.
rationale` 字段中（自由文本）。可借鉴:
- Agent 输出结构化分析: `{assessment, category, confidence, drivers[]}`
- 下游 Nuwa 仿真可以用这些做可审计的推演路径

---

## 3. 不适合借鉴的点

### 3.1 前端架构 (Next.js + MapLibre)
TianJi 是纯 Rust CLI/TUI 工具，不需要 Web 前端。ShadowBroker 的前端不是
借鉴目标。

### 3.2 Docker 分发
ShadowBroker 用 Docker + Helm 部署。TianJi 目标单二进制 ≤25MB，
Docker 不是优先事项。

### 3.3 InfoNet 去中心化网格
ShadowBroker 的 Ed25519 + Wormhole relay + Sovereign Shell 治理系统
非常庞大，远超 TianJi 当前范围。Agent 之间通过 Board/Stick 协议通信
已足够。

### 3.4 Python 后端
ShadowBroker 的后端是 Python FastAPI（未开源，Docker 镜像分发）。
TianJi 已完全 Rust 化，不需要借鉴 Python 实现。

---

## 4. 优先级排序 (TianJi 采用建议)

1. **告警分发** (高) — AlertDispatcher 模式 → TianJi 的 AlertTier 扩展
2. **Agent 通道协议** (中) — HMAC 签名 + batch 模式 → Hongmeng 外部 Agent 支持
3. **类型化数据模型** (中) — TypeScript 类型 → Rust struct (H8 修复)
4. **数据源分层** (中) — Fast/Slow tier → DaemonConfig 扩展
5. **分析区域** (低) — 结构化 Agent 输出 → Hongmeng Agent 输出格式
6. **快照回放** (低) — Time Machine → TUI 历史回放模式
7. **Rust FFI 模式** (备用) — handle-based FFI → 跨语言集成参考

---

## 5. 具体实施建议

### 立即: 告警分发模块

```rust
// src/alert_dispatch.rs (新文件)
pub enum AlertChannel { Discord(String), Telegram(String, String) }
pub struct AlertDispatcher { channels: Vec<AlertChannel>, client: reqwest::Client }

impl AlertDispatcher {
    pub async fn send_alert(&self, tier: AlertTier, summary: &str) -> Vec<Result>
    // Flash → 所有通道, Priority → Discord only, Routine → skip
}
```

配置:
```yaml
# ~/.tianji/config.yaml
alerts:
  discord: "https://discord.com/api/webhooks/..."
  telegram:
    bot_token: "..."
    chat_id: "..."
```

### 短期: Agent 端点

在 daemon 的 axum Router 添加:
```
POST /api/v1/agent/command   (HMAC 签名)
POST /api/v1/agent/batch      (最多 10 并发)
```

### 中期: 强类型迁移

参考 ShadowBroker 的 TypeScript 类型定义风格，为 TianJi 创建:
```rust
// src/types.rs (新文件，集中所有数据类型)
// 当前分散在 models.rs, delta.rs, storage.rs, worldline/types.rs 等
```

---

## 6. 文件参考

| ShadowBroker 文件 | 行数 | TianJi 对应 |
|-------------------|------|------------|
| frontend/src/types/dashboard.ts | 1097 | src/models.rs (119行 — 需扩展) |
| openclaw-skills/sb_alerts.py | 212 | 新 src/alert_dispatch.rs |
| openclaw-skills/SKILL.md | 583 | TianJi Agent 协议文档 |
| privacy-core/src/lib.rs | 2254 | 参考 FFI 模式，暂不需要 |
| frontend/src/utils/alertSpread.ts | — | TianJi delta_memory alert clustering |
