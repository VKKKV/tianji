# Quality Guidelines

> Code standards, testing requirements, and forbidden patterns for this project.

---

## Overview

TianJi is a **Rust project** migrating from Python. The Rust implementation under
`src/` is the product direction. The Python codebase under `tianji/` and `tests/`
is the migration oracle — it defines the compatibility contract that Rust must
match gate-by-gate before replacing any Python surface.

---

## Rust Technology Stack

| Concern | Choice | Constraint |
|---------|--------|------------|
| **Language** | Rust 2021 edition | `Cargo.toml`-based |
| **CLI framework** | `clap` (derive) | Phase 1 (replacing manual arg parse) |
| **XML parsing** | `roxmltree` | Feed parsing (Cangjie) |
### Rust Dependencies (Current)

Only dependencies needed for implemented milestones:

```toml
regex = "1.10"
roxmltree = "0.20"
serde = { version = "1.0", features = ["derive"] }
serde_json = "1.0"
sha2 = "0.10"
```

### Rust Dependencies (Planned, per `plan.md` §11)

Add only when the milestone requires them:

- Milestone 2: `rusqlite` (SQLite persistence)
- Milestone 4: `ratatui`, `crossterm` (TUI)
- Milestone 5: `tokio`, `axum`, `reqwest` (daemon, HTTP)
- Phase 2+: `blake3`, `petgraph`, `chrono` (worldline, field DAG)
- Phase 3+: `async-openai`, `ollama-rs` (LLM providers)

---

## Rust Testing Requirements

### Framework

**`cargo test` is the canonical test framework.** Verification commands:

```bash
cargo test
cargo fmt --check
cargo clippy -- -D warnings
```

### Test Structure

| Convention | Pattern |
|------------|---------|
| **Unit tests** | `#[cfg(test)] mod tests` inside each module |
| **Integration tests** | Tests in `src/lib.rs` exercising full pipeline |
| **Fixture path** | `tests/fixtures/sample_feed.xml` |
| **Contract fixture** | `tests/fixtures/contracts/run_artifact_v1.json` |

### Test Coverage Expectations

- Pipeline integration: every stage exercised end-to-end
- Scoring: deterministic numeric assertions on known inputs (exact-value + factor isolation)
- Grouping: event group structure and causal ordering
- Backtracking: intervention candidate generation
- Contract: top-level and nested artifact keys match Python oracle
- Hash parity: canonical hashes match Python SHA-256 expectations

### Python Oracle Verification

The Python test suite must still pass to confirm the oracle is intact:

```bash
uv venv .venv && uv pip install -e .
.venv/bin/python -m unittest discover -s tests -v
```

---

## Rust Code Standards

### Structs for Data

All structured data uses typed Rust structs:

```rust
// src/models.rs — all model types
#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct NormalizedEvent {
    pub event_id: String,
    pub source: String,
    pub title: String,
    ...
}

#[derive(Debug, Serialize)]
pub struct RunArtifact {
    pub schema_version: String,
    pub mode: String,
    ...
}
```

### JSON Serialization

Use `serde_json::to_string_pretty()` for artifact output:

```rust
pub fn artifact_json(artifact: &RunArtifact) -> Result<String, TianJiError> {
    Ok(serde_json::to_string_pretty(artifact)?)
}
```

### Error Handling

Use the `TianJiError` enum with `Result<T, TianJiError>` returns:

```rust
#[derive(Debug)]
pub enum TianJiError {
    Usage(String),
    Input(String),
    Io(std::io::Error),
    Json(serde_json::Error),
}
```

See `error-handling.md` for full details.

### Naming

| Element | Convention | Example |
|---------|------------|---------|
| Modules | `snake_case.rs` | `scoring.rs`, `backtrack.rs` |
| Structs | `PascalCase` | `RunArtifact`, `ScoredEvent` |
| Functions | `snake_case` | `run_fixture_path`, `normalize_item` |
| Constants | `UPPER_SNAKE` | `ATOM_NS`, `FIELD_KEYWORDS` |
| Private helpers | `snake_case` (module-local) | `clean_text`, `sha256_hex` |

### Spec Document Naming

Specification documents under `.trellis/spec/` use **lowercase kebab-case** filenames.

---

## File Size Guidelines

- If a Rust module exceeds ~500 lines, consider splitting into submodules
- If a module exceeds ~800 lines, it should definitely be split
- Test modules within `src/` can be larger due to many test cases

---

## Forbidden Patterns

| Pattern | Why |
|---------|-----|
| **Adding dependencies before the milestone that needs them** | Each milestone adds only what it uses |
| **Async runtimes before Phase 2+** | No `tokio` until daemon/simulation needs it |
| **Web frameworks before Milestone 5** | No `axum` until daemon/API milestone |
| **TUI crates before Milestone 4** | No `ratatui` until TUI milestone |
| **LLM crates before Phase 3+** | No `async-openai`/`ollama-rs` until simulation |
| **Tests depending on public network** | All tests use local fixtures |
| **Asserting against incidental formatting** | Assert on artifact semantics, not whitespace |
| **Bypassing CLI input rules** | No run without `--fixture` plus at least one resolved source |
| **Deleting Python code before parity gates pass** | Python is the oracle |
| **Extending Python code as product direction** | New features go in Rust |

---

## Anti-Patterns to Avoid

- **Don't treat Python as the product direction** — it is the oracle
- **Don't design for daemon/IPC/web UI before one-shot flow is correct** — CLI first
- **Don't replace deterministic logic with opaque model-driven behavior** — deterministic first
- **Don't add cloud-required dependencies** — local-only MVP
- **Don't claim Rust parity without verifying against Python oracle output**

---

## Pre-Commit Checklist

Before committing any Rust change:

- [ ] `cargo test` passes
- [ ] `cargo fmt --check` passes
- [ ] `cargo clippy -- -D warnings` passes
- [ ] New code follows existing patterns (look at neighboring modules)
- [ ] No new dependencies in `Cargo.toml` without milestone justification
- [ ] Tests don't depend on public network
- [ ] Python oracle tests still pass (if Python venv available)

---

## Python Oracle Reference (Migration Compatibility)

The sections below describe the Python oracle codebase for parity verification.
These are **not** coding standards for new code — they document the existing
Python behavior that Rust must match.

### Python Technology Stack

| Concern | Choice | Constraint |
|---------|--------|------------|
| **Language** | Python 3.12+ | Oracle only |
| **Package manager** | uv (`uv venv .venv`) | `pyproject.toml`-based |
| **CLI framework** | Click | Oracle only |
| **Database** | Raw `sqlite3` | No ORM |
| **TUI** | Rich | Oracle only |
| **Testing** | `unittest` | Oracle verification |

### Python Data Model

- All structured data uses `dataclasses.dataclass`
- Serializable dataclasses implement `to_dict()` returning `dict[str, object]`
- JSON output uses `json.dumps(data, ensure_ascii=False, indent=2)`

### Python Pre-Commit Checklist

- [ ] All existing `unittest` tests pass
- [ ] No new dependencies added to `pyproject.toml` without justification
- [ ] Database schema changes use `ensure_column()` for additive changes only
- [ ] CLI output uses `click.echo()` with JSON format

---

**Language**: English
