# Continue From Plan

## Goal

Execute Phase 6 Cleanup & Release per `plan.md` §4: remove the Python oracle, update documentation for Rust-only, add shell completions, and tag `v0.2.0`.

## What I Already Know

* Root `plan.md` §4 defines Phase 6 as the final cleanup milestone before release.
* All Rust milestones (M1A–M4, Crucix Delta, M3.5 housekeeping) are complete.
* Python code under `tianji/` and `tests/` is the migration oracle — now safe to retire.
* `Cargo.toml` version is already `0.2.0`.
* `clap` 4.6 with derive is already in deps; `clap_complete` needs to be added for shell completions.

## Requirements

### 4.1 Delete Python Oracle

Remove all Python artifacts:
- `tianji/*.py` (entire `tianji/` directory)
- `tests/*.py` (Python test files; keep `tests/fixtures/`)
- `pyproject.toml`
- `.venv/`
- `.ruff_cache/`
- `dummy.sqlite3`

Directories that don't exist on disk (skip silently): `.pytest_cache/`, `.agents/`, `.codex/`, `.gemini/`, `plan-crucix.md`, `uv.lock`.

### 4.2 Documentation

- Update `README.md`: remove all Python oracle references, update "Current State" to reflect M3.5 complete, update Repository Layout to remove `tianji/` and `tests/*.py`, remove "Python as Oracle" from Design Principles.
- Add `clap_complete` dependency to `Cargo.toml`.
- Implement a `completions` subcommand that generates bash/zsh/fish completions to stdout.
- Add completions usage to README CLI reference section.

### 4.3 Tag

- Git tag `v0.2.0` on the commit that completes this work.

### 4.4 Verification

- `cargo build --release` zero errors.
- `cargo test` all green.
- `cargo clippy -- -D warnings` zero warnings.
- `cargo fmt --check` passes.

## Acceptance Criteria

* [ ] No Python source files or Python project config remain in the repo.
* [ ] `README.md` has no Python oracle references; Repository Layout and Design Principles are updated.
* [ ] `tianji completions <shell>` generates shell completion scripts.
* [ ] `cargo build --release`, `cargo test`, `cargo clippy -- -D warnings`, `cargo fmt --check` all pass.
* [ ] `plan.md` §4 items marked complete.
* [ ] Git tag `v0.2.0` created.

## Out of Scope

* GitHub Actions CI (can be a follow-up).
* Binary size audit (<25MB) — informational only, not blocking.
* Hongmeng/Nuwa work.
* Cold archive rotation, external alert delivery.

## Technical Approach

1. Delete Python artifacts via `rm -rf` / `git rm`.
2. Add `clap_complete` dep, implement completions subcommand in `src/main.rs`.
3. Rewrite README sections to be Rust-only.
4. Update `plan.md` §4 status markers.
5. Run full verification suite.
6. Commit + tag.

## Decision (ADR-lite)

**Context**: All Rust milestones are complete. The Python oracle has served its purpose and can be retired.

**Decision**: Execute full Phase 6 cleanup in one task.

**Consequences**: The repo becomes a pure Rust project ready for open-source release. The Python oracle is gone and cannot be used for future parity checks — but all parity gates have already passed.
