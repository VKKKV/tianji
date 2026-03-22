# TIANJI TESTS

## OVERVIEW
Small fixture-first verification layer for the owned Python MVP.

## WHERE TO LOOK
| Task | Location | Notes |
|------|----------|-------|
| Main coverage | `test_pipeline.py` | End-to-end owned suite |
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
