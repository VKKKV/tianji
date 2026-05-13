# TIANJI TESTS

## OVERVIEW

Small fixture-first verification layer for the Python migration oracle. These
tests verify the Python pipeline output that Rust must match for parity.

**Status: Migration Oracle Tests.** These tests define the compatibility contract.
Do not remove them until the corresponding Rust parity gate passes.

## WHERE TO LOOK
| Task | Location | Notes |
|------|----------|-------|
| Pipeline integration | `test_pipeline.py` | Core end-to-end pipeline and persistence checks |
| TUI coverage | `test_tui.py` | Read-only terminal browser state (Rich TUI — oracle only) |
| History list coverage | `test_history_list.py` | `history` list/read filtering surface |
| History show coverage | `test_history_show.py` | `history-show` detail and projection surface |
| History compare coverage | `test_history_compare.py` | `history-compare` presets, projections, and diff |
| Scoring coverage | `test_scoring.py` | `Im` / `Fa` scoring semantics and summary tie rules |
| Grouping coverage | `test_grouping.py` | grouping, causal clustering, and grouped backtracking |
| CLI input coverage | `test_cli_inputs.py` | source-config and operator-facing failure paths |
| Shared test helpers | `support.py` | test-only fixture/constants/import hub |
| Stable input | `fixtures/sample_feed.xml` | Canonical deterministic feed sample |

## CONVENTIONS
- Use `unittest` style to match the existing suite.
- Prefer end-to-end assertions over mocking internal stage functions.
- Keep fixtures local and deterministic.
- Do not remove tests until Rust parity is verified against the same fixture.

## ANTI-PATTERNS
- Do not add tests that depend on public network availability.
- Do not assert against incidental formatting when artifact semantics are what matter.
- Do not remove Python tests before Rust parity gates pass.

## NOTES
- `python3 -m unittest discover -s tests -v` is the oracle test command.
- Rust tests under `src/` and `tests/` (Rust) should use the same fixtures.
- Root `plan.md` §10 defines the target Rust test structure.
- Once Rust parity passes, Python tests are retired per `plan.md` §13.
