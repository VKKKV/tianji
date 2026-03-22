# Project TianJi (天机) - AI Development & Execution Plan

## 🎯 核心目标 (Core Objective)
你是一个顶级的极客开发者（Arch Linux 拥趸）。你需要为我开发一个名为“天机 (TianJi)”的系统。这是一个融合了全球情报抓取、多智能体沙盒推演、以及世界线收束（Divergence）反推(重点)的“上帝引擎”。

模块化、极简主义、底层由 python/Rust/Go 驱动的 Daemon 组成，通过 IPC (UNIX Domain Sockets) 通信。坚决抵制过度封装和不必要的框架。UI 层必须与核心引擎完全解耦。

## 📂 本地知识库参考 (Local Context)
请先利用你的本地文件读取能力，深入分析当前工作目录 `~/code/tianji` 下的四个开源参考项目，汲取它们的灵感
1. `./worldmonitor/`：提取其 OSINT 情报抓取逻辑和新闻源节点，但摒弃其前端代码。我们需要的是无头（Headless）的数据流。
2. `./DivergenceMeter/`：深入研究其数学模型和代码逻辑，特别是关于 $Im$ (Impact) 和 $Fa$ (Field attraction) 的世界线变动率算法。
3. `./MiroFish/`：参考其 Swarm Intelligence (群体智能) 和多 Agent 沙盒推演的逻辑，将其改造为适配大模型的轻量级实现。
4. `./oh-my-openagent/`：参考其终端集成（Tmux/LSP/AST-Grep）和 Agent 编排能力，将其思想融入“天机”的内部调度中。

*注：如果遇到缺失的依赖、算法细节或系统级 API (如 systemd/dbus)，请随时使用 Web Search (网络搜索) 获取最新文档。*

## 🐉 架构与模块拆解 (The Mythological Architecture)

请按照以下模块化步骤推进开发：

### Phase 1: Cangjie (仓颉) - OSINT Fetcher & RAG
* **职责：** 核心数据抓取与检索。
* **任务：** 编写一个轻量级服务（推荐 Rust 或 Go or python），定时抓取全球新闻 RSS/API。数据清洗后存入本地 SQLite。实现极速的基于 Hash 的内容检索机制（参考 oh-my-openagent 的 Hashline 机制）。

### Phase 2: Hongmeng (鸿蒙) - The Core Orchestrator
* **职责：** 系统总线与 IPC 调度守护进程 (Daemon)。
* **任务：** 建立基于 UNIX Domain Sockets 的通信总线。它负责接收来自 CLI 的目标变动率（Target Divergence），并管理其他所有 Agent 的生命周期。

### Phase 3: Fuxi (伏羲) & Nuwa (女娲) - Divergence & Execution
* **职责：** 伏羲负责战略推演（计算 Divergence 并反推需要干涉的关键事件）；女娲负责将干涉事件注入到模拟沙盒（MiroFish 的轻量化变体）中观察蝴蝶效应。
* **任务：** 实现 Divergence 核心算法（参考 DivergenceMeter）。编写基于本地大模型 API（如 Ollama/llama.cpp 接口）的 Prompt 链路。

### Phase 4: CLI & Chooseable UI (终端与可选可视化层)
* **终端优先 (CLI First)：** 开发 `tianji-cli` 工具，支持诸如 `tianji-cli trigger --target-divergence 1.048596` 的硬核命令。
* **解耦的 Web UI：** 提供一个**可选的 (Chooseable)** 轻量级后端 API（如 FastAPI 或 Go Fiber）和前端页面（Vue/React）。前端通过 WebSocket 订阅鸿蒙的 IPC 状态。强调：Web UI 必须作为一个独立的服务（`tianji-web.service`），默认关闭，仅在用户需要时启动。

## 🛠️ 开发规范与限制
1. **No Bloatware:** 不要引入庞大的全家桶框架。依赖越少越好。
2. **Local First:** 默认所有大模型调用指向本地 `localhost` 的接口（如 OpenAI 兼容的本地 API），不强制依赖外部闭源云服务。
3. **输出要求:** 给出清晰的代码结构、关键功能的源码、以及在 Arch Linux 上的 systemd 部署脚本。
