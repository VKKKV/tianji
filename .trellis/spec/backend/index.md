# Backend Development Guidelines

> Best practices for backend development in this project.

---

## Overview

This directory contains guidelines, contracts, and specs for backend development.

- **Guidelines**: How to write code (conventions, patterns, anti-patterns)
- **Contracts**: What subsystems do (shipped behavior boundaries, frozen vocabularies)
- **Specs**: Technical designs for specific domains (scoring model, development roadmap)

---

## Pre-Development Checklist

Before writing backend code:

1. Read the relevant **guideline files** below for coding standards
2. Read the relevant **contract** in `contracts/` if your change touches a shipped subsystem
3. Read the **scoring spec** if modifying heuristic/analysis behavior
4. Read the **thinking guides** in `spec/guides/` for cross-layer and code-reuse considerations

---

## Guidelines Index

| Guide | Description | Status |
|-------|-------------|--------|
| [Directory Structure](./directory-structure.md) | Module organization and file layout | Filled |
| [Database Guidelines](./database-guidelines.md) | ORM patterns, queries, migrations | Filled |
| [Error Handling](./error-handling.md) | Error types, handling strategies | Filled |
| [Quality Guidelines](./quality-guidelines.md) | Code standards, forbidden patterns | Filled |
| [Logging Guidelines](./logging-guidelines.md) | Structured logging, log levels | Filled |

---

## Contracts Index

| Contract | Description |
|----------|-------------|
| [Daemon Contract](./contracts/daemon-contract.md) | Local daemon runtime contract |
| [Local API Contract](./contracts/local-api-contract.md) | Read-first loopback HTTP API contract |
| [TUI Contract](./contracts/tui-contract.md) | Read-only Rich terminal UI contract |
| [Web UI Contract](./contracts/web-ui-contract.md) | Optional browser UI contract |

---

## Specs Index

| Spec | Description |
|------|-------------|
| [Scoring Spec](./scoring-spec.md) | Deterministic `Im`/`Fa` scoring model: formulas, rationale, deferred work |
| [Development Plan](./development-plan.md) | Long-range product roadmap and Rust migration alignment |
| [Rust Rewrite Plan](../../plan.md) | Full Rust rewrite vision: Cangjie/Fuxi/Hongmeng/Nuwa architecture, TUI spec, dependency list |

---

**Language**: All documentation should be written in **English**.
