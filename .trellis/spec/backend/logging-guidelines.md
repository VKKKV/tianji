# Logging Guidelines

> **Status: Oracle-only.** This document describes the Python oracle's output
> conventions. The Rust target uses `tracing` (Milestone 5+). Until then, Rust
> output goes to stdout (artifact JSON) and stderr (errors).

---

## Python Oracle Logging (Compatibility Reference)

The sections below document the Python oracle's output patterns for parity
verification. They are **not** coding standards for new Rust code.

---

## Overview

TianJi does **not** use Python's `logging` module. There are **zero** `import logging` statements in the codebase. Output is handled through purpose-specific channels:

| Channel | Purpose | Location |
|---------|---------|-----------|
| `click.echo()` | Primary CLI output (JSON results, status messages) | All CLI handlers |
| `print()` | Fallback messages only (TUI unavailable, server startup) | `tui.py:44`, `webui_server.py:251` |
| `sys.stderr` | Click error display | `cli.py:782` |
| `JobRecord.error` | Daemon worker failure tracking | `daemon.py:204` |

---

## Output Patterns

### CLI Output: `click.echo()`

All CLI commands return results via `click.echo()`. JSON is the expected output format:

```python
# tianji/cli.py:76-77 — artifact output
artifact_json = json.dumps(artifact.to_dict(), ensure_ascii=False, indent=2)
click.echo(artifact_json)
```

```python
# tianji/cli_history.py:95 — history listing
click.echo(json.dumps(result, ensure_ascii=False, indent=2))
```

```python
# tianji/cli_daemon.py:213 — daemon status
click.echo(json.dumps(response, ensure_ascii=False, indent=2))
```

### Fallback Messages: `print()`

Only used when the primary channel is unavailable:

```python
# tianji/tui.py:44 — TUI fallback when no runs exist
print("No persisted runs are available for the TUI browser.")
```

```python
# tianji/webui_server.py:251 — server startup
print(f"WebUI server starting on {args.host}:{args.port}")
```

### Error Display: `sys.stderr`

Click errors are displayed via the framework's own mechanism:

```python
# tianji/cli.py:782
except click.ClickException as error:
    error.show(file=sys.stderr)
```

### Daemon Error Tracking: `JobRecord.error`

The daemon has no runtime log output. Worker failures are stored in `JobRecord.error` strings:

```python
# tianji/daemon.py:202-204
except Exception as exc:
    error_message = f"{exc.__class__.__name__}: {exc}"
    self.state.set_job_failed(record.job_id, error=error_message)
```

### API HTTP Logging: Explicitly Disabled

The API server's `log_message` is overridden to a no-op:

```python
# tianji/api.py:70-71
def log_message(self, format: str, *args: Any) -> None:
    return  # No-op: suppress HTTP request logging
```

---

## TUI Output

The Rich TUI uses `Console` with `Live(screen=True)` — it writes directly to the terminal and is not part of any logging system. No log messages should be printed while the TUI is active (they would corrupt the terminal display).

---

## Rules

- **Use `click.echo()` for all CLI output** — it respects Click's output streams and testing infrastructure
- **Use `print()` only for fallback messages** — when Click is not available (TUI fallback, raw server startup)
- **Use `json.dumps(ensure_ascii=False, indent=2)` for structured output** — consistent formatting across all commands
- **Never use `logging` module** — not part of this project's conventions
- **Never print to stdout during TUI sessions** — would corrupt the terminal display

---

## Anti-Patterns

- **No `logging` module** — do not introduce `logging.getLogger()`, `logging.basicConfig()`, or any logging framework
- **No `print()` for normal CLI output** — use `click.echo()` in CLI commands
- **No runtime log files** — all state is in SQLite; there is no log file to tail
- **No `stderr` for normal output** — `stderr` is only for Click error display

---

## Common Mistakes

- Adding `print()` debug statements and forgetting to remove them — use `click.echo()` or write to the database instead
- Introducing `logging` because "every project needs logging" — TianJi intentionally uses SQLite for all persistent state
- Printing formatted text during TUI sessions — it will corrupt the Rich terminal display

---

**Language**: English
