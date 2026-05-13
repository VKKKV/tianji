# PROJECT KNOWLEDGE BASE

**Updated:** 2026-05-13
**Branch:** rust-cli

## OVERVIEW

TianJi is migrating from Python to a full Rust implementation. The authoritative
architecture document is root `plan.md`, which defines four subsystems
(Cangjie, Fuxi, Hongmeng, Nuwa), the project structure, TUI design, and
phased build order.

**Current state:**
- Rust: Milestone 1A+1B complete (feed + normalization + scoring + grouping + backtracking parity). Pipeline produces field-for-field compatible `RunArtifact` with Python oracle.
- Python: shipped product surface under `tianji/` and `tests/` — preserved as the migration oracle until Rust parity gates pass.

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
| Python oracle (compatibility) | `tianji/fetch.py`, `tianji/normalize.py` | Match these for parity |
| Python scoring oracle | `tianji/scoring.py` | Match for Milestone 1B |
| Python data model | `tianji/models.py` | RawItem → NormalizedEvent → ScoredEvent → RunArtifact |
| Python persistence | `tianji/storage.py` | SQLite schema for Milestone 2 |
| Python TUI (reference) | `tianji/tui.py` | Rich TUI; target is ratatui per plan.md §9 |
| Development plan | `.trellis/spec/backend/development-plan.md` | Migration milestones and guardrails |
| TUI contract (legacy) | `.trellis/spec/backend/contracts/tui-contract.md` | Superseded by plan.md §9 |

## CODE MAP (Rust)
| Symbol | Type | Location | Role |
|--------|------|----------|------|
| `main` | function | `src/main.rs` | CLI entry and run dispatch |
| `RawItem` | struct | `src/models.rs` | Parsed feed item |
| `NormalizedEvent` | struct | `src/models.rs` | Extracted event with keywords/actors/regions |
| `RunArtifact` | struct | `src/models.rs` | Pipeline output contract |
| `parse_feed` | function | `src/fetch.rs` | RSS/Atom parsing |
| `normalize_items` | function | `src/normalize.rs` | Event extraction and field scoring |

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
- Rust build/test: `cargo build`, `cargo test`, `cargo fmt --check`.
- Python oracle verification: `.venv/bin/python -m unittest discover -s tests -v`.
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
cargo run -- run --fixture tests/fixtures/sample_feed.xml
```

## COMMANDS (Python — Oracle)
```bash
uv venv .venv
uv pip install -e .
.venv/bin/python -m tianji run --fixture tests/fixtures/sample_feed.xml
.venv/bin/python -m unittest discover -s tests -v
```

## NOTES
- The `rust-cli` branch is the active development branch for the Rust migration.
- Python TUI uses Rich; the target Rust TUI uses ratatui with Kanagawa Dark palette per `plan.md` §9.
- The local API contract is documented in `.trellis/spec/backend/contracts/local-api-contract.md`.
- Scoring model spec: `.trellis/spec/backend/scoring-spec.md`.
