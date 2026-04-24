# Quality Guidelines

> Code standards, testing requirements, and forbidden patterns for this project.

---

## Overview

TianJi is a **Python 3.12+ stdlib-first** project. The codebase is intentionally lightweight — no heavy frameworks, no ORM, no type checker. Quality is enforced through convention, review, and a deterministic test suite.

---

## Technology Stack

| Concern | Choice | Constraint |
|---------|--------|------------|
| **Language** | Python 3.12+ | No older Python versions |
| **Package manager** | uv (`uv venv .venv`) | `pyproject.toml`-based |
| **CLI framework** | Click | All CLI commands use Click decorators |
| **Database** | Raw `sqlite3` | No ORM |
| **TUI** | Rich | Terminal UI only |
| **HTTP server** | `http.server` (stdlib) | No Flask/FastAPI/Starlette |
| **XML parsing** | `xml.etree.ElementTree` (stdlib) | Feed parsing |
| **Testing** | `unittest` | Canonical test framework |
| **Type checking** | None | No mypy/pyright; types in docstrings and runtime asserts |

---

## Testing Requirements

### Framework

**`unittest` is the canonical test framework.** The operational command is:

```bash
.venv/bin/python -m unittest discover -s tests -v
```

`pyproject.toml:11-12` has a pytest stanza but it is **not** the operational test contract.

### Test Structure

| Convention | Pattern |
|------------|---------|
| **File naming** | `test_{feature}.py` — one file per feature |
| **Class naming** | `{Feature}Tests(unittest.TestCase)` |
| **Method naming** | `test_{description}` — self-documenting |
| **Imports** | All tests import from `tests/support.py` (shared import hub) |
| **Fixtures** | `tests/fixtures/` directory; contract fixtures in `tests/fixtures/contracts/` |

```python
# tests/test_pipeline.py:1,4 — test template
from support import *

class PipelineIntegrationTests(unittest.TestCase):
    def test_run_artifact_contract_fixture_freezes_v1_vocabulary(self) -> None:
        ...
```

### Test Patterns

**Integration tests with real pipeline calls** (`tests/test_pipeline.py:10-14`):
```python
artifact = run_pipeline(
    fixture_paths=[str(FIXTURE_PATH)],
    fetch=False, source_urls=[], output_path=None,
)
```
No mocking of pipeline stages — integration tests exercise real orchestration.

**Local HTTP servers for fetch testing** (`tests/test_pipeline.py:72-91`):
```python
server = ThreadingHTTPServer(("127.0.0.1", 0), Handler)
self.addCleanup(server.shutdown)
self.addCleanup(server.server_close)
```

**`TemporaryDirectory` for isolation** (`tests/test_pipeline.py:50`):
```python
with TemporaryDirectory() as tmpdir:
    sqlite_path = Path(tmpdir) / "tianji.sqlite3"
```

**Contract fixture freezing** (`tests/test_pipeline.py:5-47`):
```python
artifact_fixture = cast(dict[str, object], load_contract_fixture("run_artifact_v1.json"))
self.assertEqual(set(payload), set(artifact_fixture))
```

**Resource cleanup with `self.addCleanup()`** (`tests/test_pipeline.py:89-90`):
```python
self.addCleanup(server.shutdown)
self.addCleanup(server.server_close)
```

### Test Coverage Expectations

- Pipeline integration: every stage exercised end-to-end
- Scoring: deterministic numeric assertions on known inputs
- History: all CLI presets (`--latest`, `--previous`, `--next`, `--latest-pair`, `--against-latest`, `--against-previous`)
- Daemon: lifecycle (start → status → run → stop)
- TUI: render output and state transitions

---

## Forbidden Patterns

| Pattern | Why |
|---------|-----|
| **Adding an ORM** | SQLite is stdlib-only by design |
| **Adding a web framework** | HTTP is served via stdlib `http.server` |
| **Adding a type checker** | Types are documented in dataclasses and runtime asserts |
| **Introducing a second test runner** | Stay with `unittest` |
| **Tests depending on public network** | All fetch tests use local HTTP servers |
| **Asserting against incidental formatting** | Assert on artifact semantics, not whitespace |
| **Nesting test directories** | `tests/` stays flat |
| **Bypassing CLI input rules** | No run without `--fixture` or `--fetch` plus at least one resolved source |
| **Cloud-required dependencies** | Local-first; no cloud SDKs |
| **Opaque model-driven behavior** | Deterministic heuristics first |

---

## Code Standards

### Dataclasses for Data

All structured data uses `dataclasses.dataclass`:

```python
# tianji/models.py — all model types
@dataclass
class RawItem:
    source: str
    title: str
    ...

@dataclass
class RunArtifact:
    ...
    def to_dict(self) -> dict[str, object]:
        ...
```

### Dataclass Contract: `to_dict()`

Serializable dataclasses implement `to_dict()` returning `dict[str, object]`:

```python
# tianji/models.py — RunArtifact.to_dict()
def to_dict(self) -> dict[str, object]:
    return {
        "schema_version": 1,
        "generated_at": self.generated_at.isoformat(),
        "input_summary": ...,
        "scored_events": [e.to_dict() for e in self.scored_events],
        ...
    }
```

### JSON Serialization

Consistent `json.dumps()` pattern:
```python
json.dumps(data, ensure_ascii=False, indent=2)
```

### Naming

| Element | Convention | Example |
|---------|------------|---------|
| Files | `snake_case.py` | `storage_write.py` |
| Classes | `PascalCase` | `PipelineIntegrationTests` |
| Functions | `snake_case` | `run_pipeline`, `normalize_item` |
| Constants | `UPPER_SNAKE` | `FIXTURE_PATH` |
| Private helpers | `_leading_underscore` | `_error_envelope` |

### Convention: Separate Code Naming from Spec Naming

**What**: Python code and Trellis spec documents use different filename conventions.

**Why**: Source files follow Python module rules, while spec files are documentation artifacts that read better and link more consistently in lowercase kebab-case.

**Correct split**:

```text
# Python source
tianji/storage_write.py
tianji/cli_history.py

# Trellis specs
.trellis/spec/backend/scoring-spec.md
.trellis/spec/backend/contracts/local-api-contract.md
```

**Wrong split**:

```text
# Don't mix styles
.trellis/spec/backend/SCORING_SPEC.md
.trellis/spec/backend/contracts/LOCAL_API_CONTRACT.md
```

Use this rule when moving older root docs into `.trellis/spec/`: rename them to lowercase kebab-case as part of the move, then update all references in indexes, AGENTS files, and product docs.

---

## File Size Guidelines

- If a file exceeds ~400 lines, consider splitting it (like `storage.py` → `storage_write.py` + `storage_views.py` + etc.)
- If a file exceeds ~800 lines, it should definitely be split
- Exceptions: test files for scoring (`test_scoring.py:1457`) can be large due to many test cases

---

## Anti-Patterns to Avoid

- **Don't treat upstream inspiration as first-party implementation** — reimplement inside TianJi
- **Don't design for daemon/IPC/web UI before one-shot flow is correct** — CLI first
- **Don't replace deterministic logic with opaque model-driven behavior** — deterministic first
- **Don't add cloud-required dependencies** — local-only MVP
- **Don't bypass CLI input rules** — `--fixture` or `--fetch` + resolved source required

---

## Pre-Commit Checklist

Before committing any change:

- [ ] All existing `unittest` tests pass
- [ ] New code follows existing patterns (look at neighboring files)
- [ ] No new dependencies added to `pyproject.toml` without justification
- [ ] No `print()` debug statements left in
- [ ] No `logging` module imports
- [ ] Database schema changes use `ensure_column()` for additive changes only
- [ ] CLI output uses `click.echo()` with JSON format
- [ ] Tests don't depend on public network

---

**Language**: English
