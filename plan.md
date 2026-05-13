# Rust CLI 重写计划

> 分支: `rust-cli` | 创建: 2026-05-13
> 目标: 用 Rust 重写 tianji CLI 层，Python 管线保留为引擎

## 架构决策

```
┌─────────────────────────────────────────┐
│  Rust CLI binary (`tianji`)             │
│  ├─ clap: 参数解析 + 校验              │
│  ├─ 命令路由 + 输出格式化               │
│  ├─ rusqlite: history 直读 SQLite       │
│  └─ 子进程: 调用 Python 管线            │
└──────────┬──────────────────────────────┘
           │ subprocess / PyO3
┌──────────▼──────────────────────────────┐
│  Python 管线 (保持不变)                  │
│  tianji/pipeline.py  scoring.py  etc.   │
└─────────────────────────────────────────┘
```

**为什么不直接全量移植到 Rust:**
- 管线核心逻辑（scoring/normalize/backtrack）是纯算法，已稳定
- 改动频繁的是 CLI 参数、输出格式、daemon 交互
- Python 管线可以后续逐步迁移，CLI 先独立

**Python 调用方式:**
- 优先 `subprocess`（零依赖、简单、可独立测试）
- 后续可换 PyO3（减少进程启动开销、类型安全）
- history/show/compare 不调 Python — rusqlite 直读 SQLite

## Crate 选型

| 用途 | Crate | 理由 |
|------|-------|------|
| CLI 框架 | clap 4 (derive) | 生态标准，类型安全 |
| JSON 序列化 | serde + serde_json | 管线 artifact 读写 |
| SQLite 读取 | rusqlite | history/show/compare 直读 |
| HTTP 客户端 | reqwest | daemon loopback API |
| 终端输出 | crossterm + ansi_term | 彩色/表格输出 |
| 进度条 | indicatif | run 命令进度 |
| 错误处理 | anyhow + thiserror | 用户友好错误 |
| 子进程 | std::process::Command | 调用 Python 管线 |
| 日志 | tracing + tracing-subscriber | --verbose 模式 |
| UNIX socket | std::os::unix::net | daemon 通信 |

## 命令映射

### `tianji run`
```
tianji run --fixture tests/fixtures/sample_feed.xml
tianji run --fixture a.xml --fixture b.xml
tianji run --fetch --source-url https://example.com/feed.xml
tianji run --fetch --source-config sources.json --source-name example-feed
tianji run --fetch --source-config sources.json --source-name example-feed --fetch-policy if-changed
tianji run --fixture ... --output runs/custom.json
tianji run --fixture ... --sqlite-path runs/tianji.sqlite3
```
实现: 序列化参数为 JSON → `subprocess` 调用 `.venv/bin/python -m tianji run` → 捕获 stdout JSON → 格式化输出

### `tianji history`
```
tianji history --sqlite-path runs/tianji.sqlite3
tianji history --sqlite-path runs/tianji.sqlite3 --dominant-field technology --risk-level high
tianji history --sqlite-path runs/tianji.sqlite3 --since ... --until ...
tianji history --sqlite-path runs/tianji.sqlite3 --min-top-divergence-score 18
```
实现: rusqlite 直读 → 格式化表格输出（彩色列、对齐、截断长字段）

### `tianji history-show`
```
tianji history-show --sqlite-path runs/tianji.sqlite3 --run-id 1
tianji history-show --sqlite-path runs/tianji.sqlite3 --latest
tianji history-show --sqlite-path runs/tianji.sqlite3 --run-id 3 --previous
```
实现: rusqlite 直读 → 结构化输出（scored events、interventions、event groups）

### `tianji history-compare`
```
tianji history-compare --sqlite-path runs/tianji.sqlite3 --left-run-id 1 --right-run-id 2
tianji history-compare --sqlite-path runs/tianji.sqlite3 --latest-pair
```
实现: rusqlite 双查询 → diff 格式化输出

### `tianji daemon`
```
tianji daemon start  [--socket-path ...] [--sqlite-path ...] [--host 127.0.0.1] [--port 8765]
tianji daemon stop   [--socket-path ...]
tianji daemon status [--socket-path ...] [--job-id ...]
tianji daemon run    [--socket-path ...] --fixture ...
tianji daemon schedule [--socket-path ...] --every-seconds N --count M --fixture ...
```
实现: UNIX socket 通信协议（JSON 消息），`start` 子进程启动 Python daemon

### `tianji tui`
```
tianji tui --sqlite-path runs/tianji.sqlite3
```
实现: `subprocess` 调用 Python TUI（Rich 太复杂，暂不移植）

## 项目结构

```
tianji/
├── Cargo.toml              # Rust workspace
├── src/
│   ├── main.rs             # clap 入口，命令路由
│   ├── cli/
│   │   ├── mod.rs
│   │   ├── run.rs          # run 命令 + 子进程管线调用
│   │   ├── history.rs      # history/history-show/history-compare
│   │   ├── daemon.rs       # daemon 子命令 + socket 通信
│   │   └── tui.rs          # tui 启动器
│   ├── pipeline.rs         # 调用 Python 管线的抽象层
│   ├── storage.rs          # rusqlite 直读封装
│   ├── daemon_socket.rs    # UNIX socket 客户端
│   ├── output.rs           # 终端输出格式化 (表格/颜色/JSON)
│   └── error.rs            # 错误类型
├── tianji/                  # Python 管线 (现有，不变)
├── tests/                   # Python 测试 (现有，不变)
├── tests_rust/              # Rust 集成测试
└── plan.md                  # 本文件
```

## 开发阶段

### Phase 1: 骨架 (1-2 sessions)
- `cargo init`，配置 Cargo.toml 依赖
- clap 命令结构 + 参数校验
- `run` 命令：subprocess 调 Python 管线，捕获 JSON
- 基本输出：JSON pretty-print 或 --json 原始输出
- 验证：现有 Python 测试全绿

### Phase 2: History 直读 (1-2 sessions)
- rusqlite 打开现有 tianji.sqlite3
- `history` 命令：列表 + 过滤 + 彩色表格
- `history-show`：单 run 详情
- `history-compare`：双 run diff
- 验证：与 Python `history` 输出对比一致

### Phase 3: Daemon (1 session)
- UNIX socket 客户端实现
- `daemon start` 子进程启动 Python daemon
- `daemon stop/status/run/schedule` socket 通信
- 健康检查 + 超时处理

### Phase 4: 润色 (1 session)
- `tui` 启动器
- `--verbose` / `--quiet` 全局 flag
- 错误信息人性化（文件不存在 / socket 断开 / Python 环境缺失）
- shell completions (clap generate)
- README 更新

### Phase 5: Python 管线 Rust 重写 (可选，后期)
- 逐模块迁移：normalize → scoring → backtrack → pipeline
- 目标：消除 Python 运行时依赖
- 前提：CLI 层稳定后，按需启动

## 不变的部分

以下完全不碰：
- `tianji/*.py` 管线核心代码
- `tests/*.py` Python 测试
- `pyproject.toml` / `.venv/` / uv 环境
- `.trellis/` 规约文档（暂留）
- `tianji/webui/` / `tianji/webui_server.py`

## 要清理的部分（本分支任务）

- 删除 `.agents/` `.codex/` `.gemini/` — 只保留 `.opencode/`（或有用的 agent config 迁移到项目根 AGENTS.md）
- 删除 `.venv/` `node_modules/` — 加 `.gitignore`
- 删除 `.pytest_cache/`
- 删除无用的 `.trellis/workspace/kita/journal-1.md`（空文件）
- `.gitignore` 添加:
  ```
  .venv/
  node_modules/
  target/
  runs/
  *.sqlite3
  ```

## 风险与注意事项

1. **SQLite schema 耦合**: rusqlite 直读依赖现有 schema。Python `storage.py` 改 schema 时需同步更新 Rust 端
2. **Python 环境**: CLI 需要能找到 `.venv/bin/python`，优先用项目内 venv，fallback 到系统 python3
3. **daemon socket 协议**: 需确认现有 socket 协议是 JSON line-delimited，方便 Rust 端实现
4. **subprocess 性能**: `run` 命令的进程启动开销 ~50ms，可接受。如果后续频繁调用，换 PyO3 或全量 Rust 移植
5. **保留 Python CLI 可运行**: Rust CLI 不删除现有 `tianji/cli.py`，两者共存，Rust 为主入口，Python 作 fallback

## 验证标准

- `cargo build --release` 成功
- `./target/release/tianji run --fixture tests/fixtures/sample_feed.xml` 输出与 Python 版一致
- 现有 Python 测试套件全绿（管线未被改动）
- `cargo test` 覆盖所有 CLI 命令参数解析
- Rust history 输出与 Python history 输出逐字段一致
