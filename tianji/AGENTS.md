# TIANJI PACKAGE

## OVERVIEW
Owned TianJi runtime: one-shot Python pipeline from feed input to JSON artifact.

## WHERE TO LOOK
| Task | Location | Notes |
|------|----------|-------|
| CLI contract | `cli.py` | Argument rules, default output path |
| End-to-end flow | `pipeline.py` | Read this first for control flow |
| Input parsing | `fetch.py` | Fixture and live feed loading |
| Optional persistence | `storage.py` | SQLite schema and per-run persistence |
| Run history reads | `storage.py`, `cli.py` | `history`, `history-show`, and `history-compare` are read-only SQLite entrypoints with filtering and grouped compare support |
| Event extraction | `normalize.py` | Keywords, actors, regions, field scores |
| Heuristic ranking | `scoring.py` | Impact, field attraction, scenario summary |
| Reverse inference | `backtrack.py` | Intervention candidate generation |
| Data contracts | `models.py` | Dataclasses define all stage boundaries |

## CONVENTIONS
- Keep the package flat until multiple files per stage justify nesting.
- New behavior should land in the stage it belongs to, not inside `cli.py` or `__main__.py`.
- `pipeline.py` orchestrates; subordinate modules stay single-purpose.
- `cli.py` currently owns source-config resolution because that logic is still operator-surface policy.
- `cli.py` also owns lightweight operator-facing history commands; keep `history`, `history-show`, and `history-compare` read-only unless a broader service layer appears.
- Preserve deterministic behavior unless a change explicitly introduces an optional model-assisted layer.
- Maintain artifact stability through `RunArtifact.to_dict()`.

## ANTI-PATTERNS
- Do not dump business logic into `cli.py` or `__main__.py`.
- Do not add a generic `utils.py` catch-all module.
- Do not import reference-repo code directly into this package.
- Do not make fetch mode depend on external network for tests.
- Do not let scoring and backtracking mutate shared input structures in place.
- Do not let SQLite persistence become mandatory for the one-shot flow.

## NOTES
- The current package has no shared `utils/` hub; stage files are the module boundaries.
- Persistence already lives in `storage.py`; continue extending it there instead of widening unrelated modules.
- History inspection, filtering, and comparison should stay in `storage.py`; keep `cli.py` as the thin operator surface.
