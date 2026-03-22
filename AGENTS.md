# PROJECT KNOWLEDGE BASE

**Generated:** 2026-03-22
**Commit:** c45b73b
**Branch:** feat/one-shot-mvp

## OVERVIEW
TianJi currently ships as a small Python CLI for one-shot `fetch -> normalize -> score -> backtrack -> emit` runs. This workspace is unusual because it also contains four large local reference repositories that inform future work but are not first-party TianJi source.

## STRUCTURE
```text
tianji/
├── tianji/                # Owned Python source; current product surface
├── tests/                 # Owned verification surface; fixture-first unittest suite
├── worldmonitor/          # Reference repo; existing AGENTS.md present
├── oh-my-openagent/       # Reference repo; existing AGENTS.md present
├── MiroFish/              # Reference repo; no prior AGENTS.md in this snapshot
├── DivergenceMeter/       # Reference repo; concept-heavy divergence vocabulary
├── README.md              # Product-facing status and roadmap
└── DEV_PLAN.md            # TianJi build and extraction roadmap
```

## WHERE TO LOOK
| Task | Location | Notes |
|------|----------|-------|
| Run the product | `tianji/__main__.py`, `tianji/cli.py` | Canonical entry is `python3 -m tianji run ...` |
| Understand orchestration | `tianji/pipeline.py` | Stage spine for the owned MVP |
| Understand data model | `tianji/models.py` | Raw item -> normalized event -> scored event -> artifact |
| Change ingestion | `tianji/fetch.py` | RSS/Atom fixture loading and one-time URL fetch |
| Change source selection | `tianji/cli.py` | Source registry parsing and URL selection live here today |
| Change persistence | `tianji/storage.py` | Optional SQLite persistence boundary |
| Inspect persisted runs | `tianji/cli.py`, `tianji/storage.py` | `history` lists/filters runs; `history-show` supports run id, latest, previous, or next; `history-compare` supports explicit ids and latest/relative presets |
| Change heuristics | `tianji/normalize.py`, `tianji/scoring.py`, `tianji/backtrack.py` | Deterministic first |
| Verify changes | `tests/test_pipeline.py`, `tests/fixtures/sample_feed.xml` | Local fixture + local HTTP server |
| Plan future divergence ideas | `DivergenceMeter/`, `DEV_PLAN.md` | Use as concept source, not owned runtime |
| Plan future orchestration/UI ideas | `worldmonitor/`, `MiroFish/`, `oh-my-openagent/` | Reference only unless explicitly reimplemented |

## CODE MAP
| Symbol | Type | Location | Role |
|--------|------|----------|------|
| `main` | function | `tianji/cli.py` | CLI entry and input guardrails |
| `run_pipeline` | function | `tianji/pipeline.py` | End-to-end pipeline coordinator |
| `parse_feed` | function | `tianji/fetch.py` | RSS/Atom parsing boundary |
| `normalize_item` | function | `tianji/normalize.py` | Event extraction and field scoring prep |
| `score_event` | function | `tianji/scoring.py` | Deterministic divergence-style heuristic |
| `backtrack_candidates` | function | `tianji/backtrack.py` | Intervention ranking |
| `RunArtifact` | dataclass | `tianji/models.py` | Serializable output contract |
| `PipelineTests` | test class | `tests/test_pipeline.py` | Fixture, fetch, and CLI validation |

## CONVENTIONS
- First-party TianJi source is only `tianji/` and `tests/`.
- Prefer the repo-local uv environment: `uv venv .venv` and `.venv/bin/python -m tianji`.
- Python 3.12+; stdlib-first; no heavy framework implied by current code.
- Verification is `unittest`-based even though `pyproject.toml` includes a minimal pytest stanza.
- Current CLI also supports `--source-config`, `--source-name`, `--sqlite-path`, `history`, `history-show`, and `history-compare`.
- `runs/` contains generated artifacts and is not source.
- `worldmonitor/`, `oh-my-openagent/`, `MiroFish/`, and `DivergenceMeter/` are workspace references, not the default edit target.

## ANTI-PATTERNS (THIS PROJECT)
- Do not treat reference repos as first-party TianJi implementation.
- Do not design for daemon/IPC/web UI before the one-shot flow stays correct.
- Do not replace deterministic logic with opaque model-driven behavior prematurely.
- Do not add cloud-required dependencies to the owned MVP.
- Do not bypass CLI input rules: no run without `--fixture` or `--fetch` plus at least one resolved source.

## UNIQUE STYLES
- Flat owned package: `fetch.py`, `normalize.py`, `scoring.py`, `backtrack.py`, `pipeline.py` stay as explicit stages.
- Reference repos are kept nearby for extraction-by-reimplementation, not by tight coupling.
- Root docs must distinguish current reality from future architecture.

## REFERENCE REPO EXIT PLAN
- Extract concepts into TianJi specs, tests, and first-party modules.
- Reimplement useful ideas inside `tianji/` rather than cross-importing from sibling repos.
- Keep upstream names and links in docs if historical context still matters.
- Remove local embedded references once TianJi no longer needs side-by-side study.

## COMMANDS
```bash
uv venv .venv
.venv/bin/python -m tianji run --fixture tests/fixtures/sample_feed.xml
.venv/bin/python -m tianji run --fixture tests/fixtures/sample_feed.xml --output runs/latest-run.json
.venv/bin/python -m tianji run --fixture tests/fixtures/sample_feed.xml --sqlite-path runs/tianji.sqlite3
.venv/bin/python -m tianji run --fetch --source-url https://example.com/feed.xml
.venv/bin/python -m tianji history --sqlite-path runs/tianji.sqlite3
.venv/bin/python -m tianji history --sqlite-path runs/tianji.sqlite3 --since 2026-03-22T10:00:00+00:00 --until 2026-03-22T12:00:00+00:00
.venv/bin/python -m tianji history-show --sqlite-path runs/tianji.sqlite3 --run-id 1
.venv/bin/python -m tianji history-show --sqlite-path runs/tianji.sqlite3 --latest
.venv/bin/python -m tianji history-show --sqlite-path runs/tianji.sqlite3 --run-id 3 --previous
.venv/bin/python -m tianji history-show --sqlite-path runs/tianji.sqlite3 --run-id 1 --next
.venv/bin/python -m tianji history-compare --sqlite-path runs/tianji.sqlite3 --left-run-id 1 --right-run-id 2
.venv/bin/python -m tianji history-compare --sqlite-path runs/tianji.sqlite3 --latest-pair
.venv/bin/python -m tianji history-compare --sqlite-path runs/tianji.sqlite3 --run-id 3 --against-latest
.venv/bin/python -m tianji history-compare --sqlite-path runs/tianji.sqlite3 --run-id 3 --against-previous
.venv/bin/python -m unittest discover -s tests -v
```

## NOTES
- Workspace scale is dominated by embedded repos, not by TianJi itself.
- Future local API contract is documented in `LOCAL_API_CONTRACT.md`; no HTTP server ships in this repo yet.
- Existing AGENTS already live in `worldmonitor/` and `oh-my-openagent/`; prefer those if you intentionally work there.
- Missing AGENTS coverage existed mainly in `MiroFish/` and `DivergenceMeter/`; local files below cover only the highest-value boundaries.
