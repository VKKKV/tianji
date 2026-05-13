# TIANJI PACKAGE

## OVERVIEW

This directory contains the **Python migration oracle** — the shipped TianJi runtime
that Rust implementations must match for parity before replacing. The product
direction is the full Rust rewrite defined in root `plan.md`.

**Status: Migration Oracle.** Code here is preserved for compatibility verification,
not extended as the product direction.

## WHERE TO LOOK
| Task | Location | Notes |
|------|----------|-------|
| CLI contract | `cli.py` | Argument rules, default output path |
| End-to-end flow | `pipeline.py` | Read this first for control flow |
| Input parsing | `fetch.py` | Fixture and live feed loading — match in Rust `src/fetch.rs` |
| Optional persistence | `storage.py` | SQLite schema and per-run persistence — match in Milestone 2 |
| Run history reads | `storage.py`, `cli.py` | `history`, `history-show`, `history-compare` |
| Event extraction | `normalize.py` | Keywords, actors, regions, field scores — match in Rust `src/normalize.rs` |
| Heuristic ranking | `scoring.py` | Impact, field attraction, scenario summary — match in Milestone 1B |
| Reverse inference | `backtrack.py` | Intervention candidate generation — match in Milestone 1B |
| Data contracts | `models.py` | Dataclasses define all stage boundaries — match in Rust `src/models.rs` |

## CONVENTIONS
- This package is the oracle: Rust must match its output, not extend it.
- Keep the package flat until Rust parity is verified.
- `pipeline.py` orchestrates; subordinate modules stay single-purpose.
- Preserve deterministic behavior unless a change explicitly introduces an optional model-assisted layer.
- Maintain artifact stability through `RunArtifact.to_dict()`.
- Do not add new features here — add them in Rust.

## ANTI-PATTERNS
- Do not extend this package as the product direction.
- Do not dump business logic into `cli.py` or `__main__.py`.
- Do not add a generic `utils.py` catch-all module.
- Do not let scoring and backtracking mutate shared input structures in place.
- Do not let SQLite persistence become mandatory for the one-shot flow.

## NOTES
- Root `plan.md` defines the target Rust architecture.
- `.trellis/spec/backend/development-plan.md` tracks migration milestones.
- Once a Rust parity gate passes, the corresponding Python code is retired per `plan.md` §13.
