# PROJECT KNOWLEDGE BASE

**Updated:** 2026-05-13
**Branch:** rust-cli

## OVERVIEW

TianJi is migrating from Python to a full Rust implementation. The authoritative
architecture document is root `plan.md`, which defines four subsystems
(Cangjie, Fuxi, Hongmeng, Nuwa), the project structure, TUI design, and
phased build order.

**Current state:**
- Rust: Milestone 1A+1B+M2+M3 complete. Full pipeline, storage+history, daemon+API+webui all parity-verified against Python oracle.
- Python: shipped product surface under `tianji/` and `tests/` — preserved as the migration oracle until M6 retirement.

## STRUCTURE
```text
tianji/
├── src/                    # Rust implementation (in progress)
├── Cargo.toml              # Rust crate manifest
├── tianji/                 # Python source — migration oracle, NOT direction
├── tests/                  # Python + Rust tests
├── plan.md                 # Authoritative Rust architecture + build phases
├── profiles/               # Actor profile YAML (planned)
└── .trellis/spec/backend/  # Development guidelines and contracts
```

## WHERE TO LOOK
| Task | Location | Notes |
|------|----------|-------|
| Rust architecture | `plan.md` | Authority for all subsystems, phases, structure |
| Rust implementation | `src/main.rs`, `src/lib.rs` | Current Rust entry |
| Rust models | `src/models.rs` | Data structures |
| Rust feed parsing | `src/fetch.rs` | RSS/Atom fixture loading |
| Rust normalization | `src/normalize.rs` | Keyword/actor/region extraction |
| Rust scoring | `src/scoring.rs` | Im/Fa scoring + rationale |
| Rust grouping | `src/grouping.rs` | Event grouping + causal ordering |
| Rust backtracking | `src/backtrack.rs` | Intervention candidate generation |
| Python oracle (compatibility) | `tianji/fetch.py`, `tianji/normalize.py` | Match these for parity |
| Python scoring oracle | `tianji/scoring.py` | Match for scoring parity |
| Python data model | `tianji/models.py` | RawItem → NormalizedEvent → ScoredEvent → RunArtifact |
| Python persistence | `tianji/storage.py` | SQLite schema for Milestone 2 |
| Python TUI (reference) | `tianji/tui.py` | Rich TUI; target is ratatui per plan.md §9 |
| Development plan | `.trellis/spec/backend/development-plan.md` | Migration milestones and guardrails |

## CODE MAP (Rust)
| Symbol | Type | Location | Role |
|--------|------|----------|------|
| `main` | function | `src/main.rs` | CLI entry and run dispatch |
| `RawItem` | struct | `src/models.rs` | Parsed feed item |
| `NormalizedEvent` | struct | `src/models.rs` | Extracted event with keywords/actors/regions |
| `ScoredEvent` | struct | `src/models.rs` | Event with Im/Fa/divergence scores |
| `RunArtifact` | struct | `src/models.rs` | Pipeline output contract |
| `parse_feed` | function | `src/fetch.rs` | RSS/Atom parsing |
| `normalize_items` | function | `src/normalize.rs` | Event extraction and field scoring |
| `score_events` | function | `src/scoring.rs` | Im/Fa scoring + rationale |
| `group_events` | function | `src/grouping.rs` | Event grouping + causal ordering |
| `backtrack_candidates` | function | `src/backtrack.rs` | Intervention candidate generation |
| `persist_run` | function | `src/storage.rs` | SQLite persistence (6 tables) |
| `list_runs` | function | `src/storage.rs` | History list with filters |
| `AppState` | struct | `src/api.rs` | axum shared state (sqlite_path) |
| `build_router` | function | `src/api.rs` | axum Router (5 GET routes) |
| `DaemonState` | struct | `src/daemon.rs` | In-memory job queue (Mutex+Condvar) |
| `serve` | function | `src/daemon.rs` | tokio runtime: socket + API + worker |
| `WebUiState` | struct | `src/webui.rs` | axum state (api_base_url) |
| `serve_webui` | function | `src/webui.rs` | Static file serve + API proxy + /queue-run |

## CODE MAP (Python — Oracle)
| Symbol | Type | Location | Role |
|--------|------|----------|------|
| `run_pipeline` | function | `tianji/pipeline.py` | End-to-end pipeline coordinator |
| `parse_feed` | function | `tianji/fetch.py` | RSS/Atom parsing boundary |
| `normalize_item` | function | `tianji/normalize.py` | Event extraction and field scoring prep |
| `score_event` | function | `tianji/scoring.py` | Deterministic Im/Fa scoring |
| `backtrack_candidates` | function | `tianji/backtrack.py` | Intervention ranking |
| `RunArtifact` | dataclass | `tianji/models.py` | Serializable output contract |

## CONVENTIONS
- `plan.md` is the architecture authority. When in doubt, follow it.
- Rust implementation goes under `src/` per `plan.md` §10 project structure.
- Python code under `tianji/` and `tests/` is the oracle — match it for parity, then replace it.
- Do not delete Python code until the relevant Rust parity gate passes.
- Rust build/test: `cargo build`, `cargo test`, `cargo fmt --check`, `cargo clippy -- -D warnings`.
- Python oracle verification: `python3 -m unittest discover -s tests -v`.
- Milestone order: 1A (feed+normalize) → 1B (score+group+backtrack) → 2 (storage) → 3 (runtime) → 4 (TUI) → 5 (daemon+web) → 6 (cleanup).

## ANTI-PATTERNS
- Do not treat Python as the product direction — it is the oracle.
- Do not claim Rust parity without verifying against Python oracle output.
- Do not add async runtimes, web frameworks, TUI crates, or LLM crates before the milestone that uses them.
- Do not design for daemon/IPC/web UI before the one-shot flow stays correct.
- Do not bypass CLI input rules: no run without `--fixture` or `--fetch` plus at least one resolved source.
- Do not implement Hongmeng or Nuwa before Cangjie/Fuxi + storage parity.

## COMMANDS (Rust)
```bash
cargo build
cargo test
cargo fmt --check
cargo clippy -- -D warnings
cargo run -- run --fixture tests/fixtures/sample_feed.xml
```

## COMMANDS (Python — Oracle)
```bash
python3 -m tianji run --fixture tests/fixtures/sample_feed.xml
python3 -m unittest discover -s tests -v
```

## NOTES
- The `rust-cli` branch is the active development branch for the Rust migration.
- Python TUI uses Rich; the target Rust TUI uses ratatui with Kanagawa Dark palette per `plan.md` §9.
- The local API contract is documented in `.trellis/spec/backend/contracts/local-api-contract.md`.
- Scoring model spec: `.trellis/spec/backend/scoring-spec.md`.
