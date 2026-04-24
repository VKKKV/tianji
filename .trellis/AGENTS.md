# PROJECT KNOWLEDGE BASE

**Generated:** 2026-03-26
**Commit:** 155633a
**Branch:** main

## OVERVIEW
TianJi currently ships a terminal-first local stack: synchronous CLI writes for one-shot `fetch -> normalize -> score -> backtrack -> emit` runs, SQLite-backed history reads, a read-only Rich TUI, a thin local daemon for bounded queueing and status, a loopback read-first HTTP API, and an optional separate web UI. Upstream inspiration still matters historically, but the active workspace now centers on first-party TianJi source, tests, and docs only.

## STRUCTURE
```text
tianji/
├── tianji/                # Owned Python source; current product surface
├── tests/                 # Owned verification surface; fixture-first unittest suite
├── README.md              # Product-facing status and roadmap
└── .trellis/spec/backend/development-plan.md  # TianJi build and extraction roadmap
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
| Plan future divergence ideas | `.trellis/spec/backend/development-plan.md` | Keep concept notes first-party and cite upstream work only as history |
| Plan future orchestration/UI ideas | `README.md`, `.trellis/spec/backend/development-plan.md` | Keep planning first-party and cite upstream work only as history |

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
- Current CLI also supports `--source-config`, `--source-name`, `--sqlite-path`, `history`, `history-show`, `history-compare`, `tui`, and `daemon`.
- `runs/` contains generated artifacts and is not source.
- Earlier upstream projects may still be cited in docs, but they are not part of the checked-out TianJi workspace.

## ANTI-PATTERNS (THIS PROJECT)
- Do not treat upstream inspiration as first-party TianJi implementation.
- Do not design for daemon/IPC/web UI before the one-shot flow stays correct.
- Do not replace deterministic logic with opaque model-driven behavior prematurely.
- Do not add cloud-required dependencies to the owned MVP.
- Do not bypass CLI input rules: no run without `--fixture` or `--fetch` plus at least one resolved source.

## UNIQUE STYLES
- Flat owned package: `fetch.py`, `normalize.py`, `scoring.py`, `backtrack.py`, `pipeline.py` stay as explicit stages.
- Historical upstream ideas should be cited briefly in docs, then reimplemented inside TianJi rather than mirrored locally.
- Root docs must distinguish current reality from future architecture.

## REFERENCE REPO EXIT PLAN
- Extract concepts into TianJi specs, tests, and first-party modules.
- Reimplement useful ideas inside `tianji/` rather than depending on external local checkouts.
- Keep upstream names or links in docs if historical context still matters.
- Avoid rebuilding a side-by-side embedded reference workspace.

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
.venv/bin/python -m tianji tui --sqlite-path runs/tianji.sqlite3
.venv/bin/python -m tianji daemon start --sqlite-path runs/tianji.sqlite3 --socket-path runs/tianji.sock --host 127.0.0.1 --port 8765
.venv/bin/python -m tianji daemon status --socket-path runs/tianji.sock
.venv/bin/python -m tianji daemon run --socket-path runs/tianji.sock --fixture tests/fixtures/sample_feed.xml
.venv/bin/python -m tianji daemon schedule --socket-path runs/tianji.sock --every-seconds 300 --count 3 --fixture tests/fixtures/sample_feed.xml
.venv/bin/python -m tianji daemon stop --socket-path runs/tianji.sock
.venv/bin/python -m tianji.webui_server --api-base-url http://127.0.0.1:8765 --host 127.0.0.1 --port 8766
.venv/bin/python -m unittest discover -s tests -v
```

## NOTES
- Workspace scale is now centered on first-party TianJi code, tests, and docs.
- The shipped local API contract is documented in `.trellis/spec/backend/contracts/local-api-contract.md`; the daemon hosts the loopback HTTP server at `127.0.0.1:8765` by default, and the optional web UI is served separately at `127.0.0.1:8766`.
- Historical upstream repos may still matter for attribution, but this AGENTS file covers the active TianJi workspace itself.
