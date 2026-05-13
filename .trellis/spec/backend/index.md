# Backend Development Guidelines

> Best practices for backend development in this project.

---

## Overview

This directory contains guidelines, contracts, and specs for backend development.

**Authority**: Root `plan.md` is the authoritative architecture document. It defines
the Rust project structure (§10), dependency list (§11), and phased build order (§12).

**Migration state**: Milestone 1A+1B complete. Python code under `tianji/` is the
migration oracle — not the product direction. Python-specific guidelines below
describe the oracle codebase and are preserved for parity verification during
the Rust migration.

- **Guidelines**: How to write code (conventions, patterns, anti-patterns)
- **Contracts**: What subsystems do (shipped behavior boundaries, frozen vocabularies)
- **Specs**: Technical designs for specific domains (scoring model, development roadmap)

---

## Pre-Development Checklist

Before writing Rust code:

1. Read `plan.md` for architecture, project structure, and build phases
2. Read the **scoring spec** if modifying heuristic/analysis behavior
3. Read the relevant **contract** in `contracts/` if your change touches a shipped subsystem
4. Read the **thinking guides** in `spec/guides/` for cross-layer and code-reuse considerations
5. Read relevant Python oracle modules under `tianji/` for parity verification

Before touching Python oracle code:

1. Treat `tianji/` and `tests/` as the compatibility oracle, not the product direction
2. Do not extend Python code — add features in Rust
3. Do not delete Python code until the corresponding Rust parity gate passes

---

## Guidelines Index

| Guide | Description | Status |
|-------|-------------|--------|
| [Directory Structure](./directory-structure.md) | Project layout — Rust target structure + Python oracle reference | Rust-primary |
| [Quality Guidelines](./quality-guidelines.md) | Code standards, forbidden patterns — Rust + Python oracle | Rust-primary |
| [Error Handling](./error-handling.md) | Error types and handling — Rust `TianJiError` enum + Python oracle reference | Rust-primary |
| [Database Guidelines](./database-guidelines.md) | SQLite patterns — Python oracle reference (Rust: rusqlite, Milestone 2) | Oracle-only |
| [Logging Guidelines](./logging-guidelines.md) | Output conventions — Python oracle reference (Rust: tracing, later milestone) | Oracle-only |

---

## Contracts Index

| Contract | Description |
|----------|-------------|
| [Daemon Contract](./contracts/daemon-contract.md) | Local daemon runtime contract |
| [Local API Contract](./contracts/local-api-contract.md) | Read-first loopback HTTP API contract |
| [TUI Contract](./contracts/tui-contract.md) | Terminal UI contract (superseded by plan.md §9 for Rust TUI) |
| [Web UI Contract](./contracts/web-ui-contract.md) | Optional browser UI contract |

---

## Specs Index

| Spec | Description |
|------|-------------|
| [Scoring Spec](./scoring-spec.md) | Deterministic `Im`/`Fa` scoring model: formulas, rationale, deferred work |
| [Development Plan](./development-plan.md) | Rust migration milestones and guardrails |
| [Rust Rewrite Plan](../../plan.md) | Full Rust rewrite vision: Cangjie/Fuxi/Hongmeng/Nuwa architecture, TUI spec, dependency list |

---

**Language**: All documentation should be written in **English**.
