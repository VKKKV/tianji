# TIANJI TESTS

## OVERVIEW
Small fixture-first verification layer for the owned Python MVP.

## WHERE TO LOOK
| Task | Location | Notes |
|------|----------|-------|
| Pipeline integration | `test_pipeline.py` | Core end-to-end pipeline and persistence checks |
| TUI coverage | `test_tui.py` | Read-only terminal browser state, formatting, and launch flow |
| History list coverage | `test_history_list.py` | `history` list/read filtering surface |
| History show coverage | `test_history_show.py` | `history-show` detail and projection surface |
| History compare coverage | `test_history_compare.py` | `history-compare` presets, projections, and diff semantics |
| Scoring coverage | `test_scoring.py` | `Im` / `Fa` scoring semantics and summary tie rules |
| Grouping coverage | `test_grouping.py` | grouping, causal clustering, and grouped backtracking |
| CLI input coverage | `test_cli_inputs.py` | source-config and operator-facing failure paths |
| Shared test helpers | `support.py` | test-only fixture/constants/import hub; not a test module |
| Stable input | `fixtures/sample_feed.xml` | Canonical deterministic feed sample |

## CONVENTIONS
- Use `unittest` style to match the existing suite.
- Prefer end-to-end assertions over mocking internal stage functions.
- Keep fixtures local and deterministic.
- For fetch-path coverage, use a local HTTP server like the existing test rather than external URLs.

## ANTI-PATTERNS
- Do not add tests that depend on public network availability.
- Do not assert against incidental formatting when artifact semantics are what matter.
- Do not introduce a second test runner unless the repo standard changes globally.

## NOTES
- `python3 -m unittest discover -s tests -v` is the current source-of-truth command.
- The pytest stanza in `pyproject.toml` is not the operational test contract yet.
- Keep the `tests/` layout flat unless unittest discovery/package rules are revisited explicitly.
- Import shared helpers from `support.py`, not from sibling test modules.
