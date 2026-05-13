# TianJi 全量 Rust 重写计划

> 分支: `rust-cli` | 更新: 2026-05-13
> 目标: 用 Rust 完全重写 tianji，零 Python 依赖，所有功能 Rust 原生

## 架构

```
tianji (single binary)
├── CLI (clap)           ── 命令路由 + 参数校验
├── Pipeline (sync)      ── fetch → normalize → score → group → backtrack → emit
├── Storage (rusqlite)   ── SQLite 持久化 + history 查询
├── Daemon (tokio)        ── UNIX socket + loopback HTTP API (axum)
├── TUI (ratatui)         ── 终端 UI，Vim 键位
└── WebUI (axum static)   ── 可选，serve 静态 HTML/CSS/JS
```

**原则:**
- 单二进制，无外部运行时依赖
- 管线同步执行（fetch → emit 是一条直线）
- Daemon 异步（tokio），管线在 spawn_blocking 中运行
- 所有 Python 代码删除，`.venv/` `pyproject.toml` `uv.lock` 全部移除

## Crate 选型

| 用途 | Crate | 理由 |
|------|-------|------|
| CLI | clap 4 (derive) | 生态标准 |
| XML 解析 | quick-xml + serde | RSS/Atom feed 解析 |
| HTTP 客户端 | reqwest | fetch live feeds |
| HTTP 服务端 | axum | daemon loopback API |
| SQLite | rusqlite (bundled) | 持久化 + history 查询 |
| TUI | ratatui + crossterm | 替代 Python Rich |
| JSON | serde + serde_json | artifact 序列化 |
| 异步运行时 | tokio (full) | daemon + HTTP server |
| 正则 | regex | normalize 关键词提取 |
| 时间 | chrono | 时间戳/ISO 8601 |
| 哈希 | sha2 | content-hash / identity-hash |
| 错误处理 | anyhow + thiserror | |
| 日志 | tracing + tracing-subscriber | --verbose |
| 表格输出 | tabled | history 命令表格 |
| Unix socket | tokio::net::UnixListener | daemon 控制面 |

## 模块映射 (Python → Rust)

```
tianji/fetch.py        → src/ingest/mod.rs, src/ingest/feed.rs, src/ingest/fetch.rs
tianji/normalize.py    → src/normalize.rs
tianji/scoring.py      → src/scoring.rs
tianji/backtrack.py    → src/backtrack.rs
tianji/models.py       → src/models.rs
tianji/pipeline.py     → src/pipeline.rs
tianji/storage.py      → src/storage.rs
tianji/cli.py          → src/cli/run.rs, src/cli/history.rs, src/cli/daemon.rs
tianji/cli_daemon.py   → src/daemon/mod.rs, src/daemon/socket.rs, src/daemon/server.rs
tianji/cli_history.py  → src/cli/history.rs
tianji/tui.py          → src/tui/mod.rs
tianji/webui_server.py → src/webui.rs (axum static serve)
tests/                 → tests/ (Rust integration tests)
```

## 项目结构

```
tianji/
├── Cargo.toml
├── build.rs                    # 可选: build-time metadata
├── src/
│   ├── main.rs                 # clap 入口
│   ├── lib.rs                  # 库根，暴露 Pipeline、Storage 等
│   ├── models.rs               # serde structs: RawItem, NormalizedEvent, ScoredEvent,
│   │                           #   EventGroupSummary, InterventionCandidate, RunArtifact
│   ├── pipeline.rs             # run_pipeline() 编排器
│   ├── ingest/
│   │   ├── mod.rs
│   │   ├── feed.rs             # RSS/Atom XML 解析 → Vec<RawItem>
│   │   └── fetch.rs            # HTTP fetch + 文件读取
│   ├── normalize.rs            # RawItem → NormalizedEvent (regex 提取)
│   ├── scoring.rs              # NormalizedEvent → ScoredEvent (Im/Fa)
│   ├── backtrack.rs            # ScoredEvent + EventGroup → InterventionCandidate
│   ├── storage.rs              # rusqlite: schema, persist_run, history queries
│   ├── daemon/
│   │   ├── mod.rs              # daemon start/stop 逻辑
│   │   ├── socket.rs           # UNIX socket 客户端 (CLI ↔ daemon)
│   │   └── server.rs           # axum HTTP API + socket 服务端 + job queue
│   ├── cli/
│   │   ├── mod.rs              # clap 命令定义
│   │   ├── run.rs              # tianji run
│   │   ├── history.rs          # tianji history / history-show / history-compare
│   │   ├── daemon.rs           # tianji daemon start/stop/status/run/schedule
│   │   └── tui.rs              # tianji tui (启动 ratatui)
│   ├── tui/
│   │   ├── mod.rs              # ratatui 应用入口
│   │   ├── list.rs             # run 历史列表
│   │   ├── detail.rs           # 单 run 详情
│   │   └── compare.rs          # run 对比视图
│   ├── webui.rs                # axum serve tianji/webui/ 静态文件
│   ├── output.rs               # 终端输出: JSON pretty-print, 表格, 颜色
│   └── error.rs                # 错误类型定义
├── tests/
│   ├── fixtures/
│   │   └── sample_feed.xml     # 测试用 RSS fixture (从 Python 版复制)
│   ├── test_pipeline.rs        # 管线集成测试
│   ├── test_scoring.rs         # 评分单元测试
│   ├── test_storage.rs         # 持久化测试
│   ├── test_daemon.rs          # daemon 集成测试
│   └── test_cli.rs             # CLI 参数解析测试
├── tianji/webui/               # 静态 Web UI 文件 (保留现有)
│   ├── index.html
│   ├── app.js
│   └── styles.css
├── plan.md                     # 本文件
└── README.md                   # 重写后更新
```

## 数据模型 (src/models.rs)

```rust
// --- Pipeline stages ---

struct RawItem {
    source: String,
    title: String,
    summary: String,
    link: Option<String>,
    published_at: Option<String>,
    entry_identity_hash: String,   // SHA256(source + id)
    content_hash: String,           // SHA256(title + summary)
}

struct NormalizedEvent {
    event_id: String,
    title: String,
    summary: String,
    source: String,
    published_at: Option<String>,
    keywords: Vec<String>,
    actors: Vec<String>,
    regions: Vec<String>,
    field_scores: HashMap<String, f64>,
    dominant_field: String,
}

struct ScoredEvent {
    event_id: String,
    title: String,
    summary: String,
    source: String,
    published_at: Option<String>,
    keywords: Vec<String>,
    actors: Vec<String>,
    regions: Vec<String>,
    dominant_field: String,
    impact_score: f64,         // Im
    field_attraction: f64,     // Fa
    divergence_score: f64,     // combined
    rationale: Rationale,
}

struct EventGroupSummary {
    group_id: String,
    headline_event_id: String,
    headline_title: String,
    member_event_ids: Vec<String>,
    member_count: usize,
    dominant_field: String,
    shared_keywords: Vec<String>,
    shared_actors: Vec<String>,
    shared_regions: Vec<String>,
    group_score: f64,
    causal_ordered_event_ids: Vec<String>,
    causal_span_hours: Option<f64>,
    evidence_chain: Vec<EventChainLink>,
    chain_summary: String,
    causal_summary: String,
}

struct InterventionCandidate {
    event_id: String,
    title: String,
    intervention_type: String,
    target: String,
    reason: String,
    priority: u8,
}

struct RunArtifact {
    schema_version: String,
    mode: String,              // "fixture" | "fetch" | "fetch+fixture"
    generated_at: String,
    input_summary: InputSummary,
    scenario_summary: ScenarioSummary,
    scored_events: Vec<ScoredEvent>,
    intervention_candidates: Vec<InterventionCandidate>,
}
```

## 开发阶段

### Phase 1: 管线核心 (主要工作量)

**目标**: `tianji run --fixture tests/fixtures/sample_feed.xml` 输出与 Python 版一致的 JSON

**文件**: `models.rs` `ingest/` `normalize.rs` `scoring.rs` `backtrack.rs` `pipeline.rs` `error.rs`

**要点:**
- quick-xml 解析 RSS 2.0 + Atom 1.0
- regex patterns 从 `normalize.py` 逐字移植 (ACTOR_PATTERNS, REGION_PATTERNS, FIELD_KEYWORDS)
- scoring 公式移植: Im = actor_weight + region_weight + keyword_density + dominant_field_bonus + ...
- Fa = dominant_field_strength + dominance_margin + coherence
- 分组逻辑移植 (link_score_between_events, group_events)
- 输出 JSON 与 Python 版 artifact 字段级对齐

**验证:** `diff <(python -m tianji run --fixture ...) <(cargo run -- run --fixture ...)`

### Phase 2: SQLite 持久化 + History

**目标**: `tianji run --sqlite-path ...` 写入，`history/show/compare` 读取

**文件**: `storage.rs` `cli/history.rs`

**要点:**
- rusqlite schema: runs, source_items, raw_items, normalized_events, scored_events, interventions (与 Python storage.py schema 兼容或重设计)
- run 去重: entry_identity_hash + content_hash
- history 过滤: mode, dominant_field, risk_level, since/until, score thresholds
- history-show: run detail + scored events + interventions + event groups
- history-compare: 双 run diff + comparable 标记

### Phase 3: CLI (clap)

**目标**: 完整命令面，与 Python CLI 行为一致

**文件**: `main.rs` `cli/mod.rs` `cli/run.rs` `output.rs`

**要点:**
- clap derive 定义所有命令/参数/默认值
- `--fixture` 多值, `--fetch` flag, `--source-url` 多值
- `--source-config` JSON 文件解析
- `--fetch-policy` (always/if-missing/if-changed)
- `--output` 默认 `runs/latest-run.json`
- 输出格式: 默认 JSON pretty-print, `--json` 原始 JSON

### Phase 4: Daemon + HTTP API

**目标**: `tianji daemon start` 启动后台进程，HTTP API 可查询

**文件**: `daemon/socket.rs` `daemon/server.rs` `daemon/mod.rs` `cli/daemon.rs`

**要点:**
- tokio 异步 daemon 进程
- UNIX socket 控制面 (JSON 协议)
- axum HTTP API: GET /api/v1/meta, /runs, /runs/{id}, /runs/latest, /compare
- job queue: daemon run / schedule (bounded --every-seconds + --count)
- job lifecycle: queued → running → succeeded | failed
- 管线执行在 spawn_blocking 中运行

### Phase 5: TUI (ratatui)

**目标**: `tianji tui --sqlite-path ...` 启动终端 UI

**文件**: `tui/mod.rs` `tui/list.rs` `tui/detail.rs` `tui/compare.rs` `cli/tui.rs`

**要点:**
- Vim 键位 (j/k/h/l, gg/G, / 搜索)
- 三面板: 左侧 run 列表, 中央详情, 右侧对比
- 只读, 数据来自 SQLite
- 复用 CLI history 的查询逻辑
- 彩色输出, 与 CLI 风格一致

### Phase 6: Web UI (axum static)

**目标**: `tianji daemon start` 同时 serve 静态 Web UI

**文件**: `webui.rs`

**要点:**
- axum serve `tianji/webui/` 目录
- 保留现有 index.html / app.js / styles.css
- app.js 通过 `/api/v1/*` 读数据 (daemon HTTP API 已存在)
- Web UI 默认关闭, `--webui` flag 开启

### Phase 7: 清理 + 文档

- 删除所有 Python 代码: `tianji/*.py` `tests/*.py` `pyproject.toml` `uv.lock`
- 删除 `.venv/` `.pytest_cache/`
- 删除 `.agents/` `.codex/` `.gemini/` (保留 `.opencode/` 中仍需要的 agent config)
- 更新 README.md 为新 Rust CLI 用法
- 更新 `.gitignore`
- `cargo build --release` 验证

## 依赖清单 (Cargo.toml)

```toml
[package]
name = "tianji"
version = "0.2.0"
edition = "2024"

[dependencies]
clap = { version = "4", features = ["derive"] }
serde = { version = "1", features = ["derive"] }
serde_json = "1"
quick-xml = { version = "0.37", features = ["serialize"] }
reqwest = { version = "0.12", features = ["blocking", "rustls-tls"], default-features = false }
axum = "0.8"
tokio = { version = "1", features = ["full"] }
rusqlite = { version = "0.32", features = ["bundled"] }
ratatui = "0.29"
crossterm = "0.28"
regex = "1"
chrono = { version = "0.4", features = ["serde"] }
sha2 = "0.10"
anyhow = "1"
thiserror = "2"
tracing = "0.1"
tracing-subscriber = "0.3"
tabled = "0.18"

[dev-dependencies]
tempfile = "3"
assert-json-diff = "2"

[profile.release]
opt-level = 3
lto = true
```

## 不变的部分

以下保留不动:
- `tianji/webui/index.html` `app.js` `styles.css` — 静态 Web UI，由 axum serve
- `tests/fixtures/sample_feed.xml` — 复制到 Rust tests/ 目录
- `.trellis/` 规约文档（可选保留或删除）
- `LICENSE` `README.md`（重写后更新内容）

## 要删除的部分

- 所有 Python 代码: `tianji/*.py` `tests/*.py` `pyproject.toml` `uv.lock`
- `.venv/` `.pytest_cache/` `__pycache__/`
- `.agents/` `.codex/` `.gemini/`（多套 agent 框架残留）
- `node_modules/`（`.opencode/node_modules/` 保留或清理）
- `dummy.sqlite3` 空测试文件

## 验证标准

- `cargo build --release` 零 warning
- `cargo test` 全绿
- `./target/release/tianji run --fixture tests/fixtures/sample_feed.xml` JSON 输出与 Python 版逐字段一致
- `tianji run --sqlite-path /tmp/test.sqlite3` → `tianji history --sqlite-path /tmp/test.sqlite3` 表头正确
- `tianji daemon start` → `curl http://127.0.0.1:8765/api/v1/meta` 返回 200
- `tianji tui --sqlite-path ...` 启动无 panic
- 单二进制 (< 20MB release) 可直接分发
