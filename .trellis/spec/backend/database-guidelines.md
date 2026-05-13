# Database Guidelines

> **Status: Shipped (Milestone 2).** Rust uses `rusqlite` with patterns matching
> the Python oracle. The sections below document both Python oracle patterns
> (for parity verification) and Rust implementation conventions (for future
> development).

---

## Python Oracle Database Patterns (Compatibility Reference)

The sections below document the Python oracle's SQLite patterns for parity
verification. They are **not** coding standards for new Rust code.

---

## Overview

TianJi uses **raw `sqlite3` from the Python standard library** with **no ORM**. There is no third-party database library. All persistence goes through the `storage.py` hub and its sub-modules (`storage_write.py`, `storage_views.py`, `storage_filters.py`, `storage_compare.py`).

---

## Connection Management

### Pattern: `contextlib.closing()` per operation

Every database operation opens → executes → closes its own connection. There are no shared or pooled connections.

```python
# storage_write.py:30-33
from contextlib import closing
import sqlite3

with closing(sqlite3.connect(database_path)) as connection:
    connection.execute("PRAGMA foreign_keys = ON")
    initialize_schema(connection)
```

```python
# storage_views.py:155-156 — read-side connection
with closing(sqlite3.connect(sqlite_path)) as connection:
    rows = connection.execute("""SELECT ... FROM runs ORDER BY id DESC""").fetchall()
```

### Rules

- Foreign keys are enabled via `PRAGMA foreign_keys = ON` at the start of `persist_run()` (`storage_write.py:32`)
- The caller (pipeline or CLI) is responsible for providing the database path
- Each write operation opens its own connection — writes from concurrent runs don't conflict on the connection object

---

## Schema

### Tables (6 tables, `storage_write.py:48-134`)

All created with `CREATE TABLE IF NOT EXISTS` inside `connection.executescript()` called from `initialize_schema()`.

| Table | Primary Key | Purpose |
|-------|-------------|---------|
| `runs` | `id INTEGER PRIMARY KEY AUTOINCREMENT` | Pipeline run metadata |
| `source_items` | `id INTEGER PRIMARY KEY AUTOINCREMENT` | Canonical, deduplicated feed items |
| `raw_items` | `id INTEGER PRIMARY KEY AUTOINCREMENT` | Per-run raw feed items |
| `normalized_events` | `id INTEGER PRIMARY KEY AUTOINCREMENT` | Normalized events with field scores |
| `scored_events` | `id INTEGER PRIMARY KEY AUTOINCREMENT` | Scored events with divergence metrics |
| `intervention_candidates` | `id INTEGER PRIMARY KEY AUTOINCREMENT` | Backtrack intervention candidates |

### Naming Conventions

| Element | Convention | Examples |
|---------|------------|----------|
| **Table names** | `snake_case`, plural | `runs`, `raw_items`, `scored_events` |
| **Column names** | `snake_case`, descriptive | `entry_identity_hash`, `divergence_score` |
| **JSON columns** | Suffix with `_json` | `input_summary_json`, `keywords_json`, `rationale_json` |
| **Primary keys** | Always `id INTEGER PRIMARY KEY AUTOINCREMENT` | Every table |
| **Foreign keys** | `{table}_id` with `ON DELETE CASCADE` | `run_id INTEGER NOT NULL REFERENCES runs(id) ON DELETE CASCADE` |

### Foreign Key Chains

```
source_items ← raw_items.canonical_source_item_id
runs         ← raw_items.run_id (CASCADE)
             ← normalized_events.run_id (CASCADE)
             ← scored_events.run_id (CASCADE)
             ← intervention_candidates.run_id (CASCADE)
```

### JSON Serialization

List/dict columns are serialized as JSON TEXT:

```python
# storage_write.py:274-288
json.dumps(event.keywords, ensure_ascii=False)  # → TEXT column
json.dumps(event.actors, ensure_ascii=False)
json.dumps(event.regions, ensure_ascii=False)
json.dumps(event.field_scores, ensure_ascii=False)
```

On read, deserialize with:

```python
# storage_views.py — within coerce_*_row functions
json.loads(row["keywords_json"]) if row["keywords_json"] else []
```

---

## Query Patterns

### Inserts

Use `executemany()` with parameterized queries for batch inserts:

```python
# storage_write.py:226-232
connection.executemany(
    """INSERT OR IGNORE INTO raw_items
       (run_id, canonical_source_item_id, source, title, summary, link, published_at)
       VALUES (:run_id, :canonical_source_item_id, :source, :title, :summary, :link, :published_at)""",
    raw_item_rows,
)
```

### Reads

Use `execute()` with `?` placeholders:

```python
# storage_views.py:163-166
rows = connection.execute(
    """SELECT id, schema_version, mode, source_count, generated_at, ... FROM runs WHERE id = ?""",
    (run_id,),
).fetchone()
```

### Row Type Safety

All query results go through `coerce_*_row()` functions that validate column types and raise `RuntimeError` for unexpected values:

```python
# storage_views.py:360-475 — 12+ coerce_*_row functions
def coerce_run_summary_row(row: sqlite3.Row) -> dict[str, object]:
    run_id = row["id"]
    if not isinstance(run_id, int | str):
        raise RuntimeError("Unexpected run id type in storage row")
    return { ... }
```

### Deduplication

Source items use `UNIQUE(entry_identity_hash, content_hash)` for dedup, with `INSERT OR IGNORE` to silently skip duplicates:

```python
# storage_write.py:196
INSERT OR IGNORE INTO source_items (entry_identity_hash, content_hash, ...)
```

---

## Migrations

### Pattern: Additive Schema Drift

No formal migration framework. Use `ensure_column()` for additive changes:

```python
# storage_write.py:149-162
def ensure_column(
    connection: sqlite3.Connection,
    *,
    table_name: str,
    column_name: str,
    column_definition: str,
) -> None:
    rows = connection.execute(f"PRAGMA table_info({table_name})").fetchall()
    existing_column_names = {str(row[1]) for row in rows}
    if column_name in existing_column_names:
        return
    connection.execute(
        f"ALTER TABLE {table_name} ADD COLUMN {column_name} {column_definition}"
    )
```

Call from `initialize_schema()` for columns added after initial creation (`storage_write.py:135-146`):

```python
# storage_write.py:135-136
ensure_column(connection, table_name="raw_items",
              column_name="canonical_source_item_id", column_definition="INTEGER")
```

### Rules
- Only additive changes (no DROP COLUMN)
- Each new column gets its own `ensure_column()` call
- Columns are always nullable to avoid breaking existing data

---

## Anti-Patterns

- **No ORM** — do not introduce SQLAlchemy, Peewee, or any ORM
- **No connection pooling** — do not introduce `sqlite3` connection pooling
- **No raw string formatting for queries** — always use `?` placeholders (`storage_write.py` uses `:named` style with `executemany`)
- **No shared mutable connection** — every operation opens and closes its own connection
- **No destructive migrations** — only ADD COLUMN, never DROP or ALTER existing columns

---

## Common Mistakes

- Forgetting to `commit()` after writes — always call `connection.commit()` explicitly (or use a context manager that auto-commits)
- Not wrapping connection in `closing()` — leads to leaked connections
- Using inconsistent placeholder styles — `storage_write.py` uses `:named` params; `storage_views.py` uses `?` positional params. Match the existing style in the file you're editing.

---

## Rust Database Conventions (Milestone 2+)

The Rust implementation in `src/storage.rs` mirrors the Python oracle's SQLite patterns using `rusqlite`.

### Connection Management

- Every operation opens its own `rusqlite::Connection::open(path)`.
- `PRAGMA foreign_keys = ON` executed on every connection immediately after opening.
- No connection pooling, no shared mutable connections.
- `persist_run()` wraps all inserts in `connection.transaction()` with explicit `tx.commit()`.

### Error Handling

- `rusqlite::Error::QueryReturnedNoRows` is matched specifically for "not found" cases (e.g., `get_run_summary`, `get_latest_run_id`). Other rusqlite errors are propagated via `TianJiError::Storage`.
- Never use `.ok()` on `query_row()` — it swallows real errors like corrupt DB or missing table.

### Schema

- 6 tables with exact same DDL as Python (see Python Oracle section above).
- `ensure_column()` migration helper replicated in Rust for additive schema drift.
- No explicit indexes beyond PKs and `source_items UNIQUE(entry_identity_hash, content_hash)`.

### Query Patterns

- Batch inserts use `prepare()` + loop with `execute()` (rusqlite has no `executemany` with named params).
- Reads use positional `?` placeholders with `query_row()` or `prepare()` + `query_map()`.
- Scored events always sorted by `divergence_score DESC, id ASC`.
- Intervention candidates always sorted by `priority ASC, id ASC`.

### Event Groups (Design Decision)

Event groups are **recomputed on read** from scored_events — never persisted. Rationale:
- scored_events is the source of truth; event_groups is a derived value.
- scored_events are immutable post-write, so recomputation is always current.
- Per-run cost is O(1) on 3-10 scored_events.
- Follows LiveStore event-sourcing principle: never include derived/computed values in event payloads.

### Gotchas

- `rusqlite::Connection::open()` creates the file if it doesn't exist — same as Python `sqlite3.connect()`.
- `INSERT OR IGNORE` for source_items dedup requires the UNIQUE constraint to be present.
- JSON columns are TEXT — serialize with `serde_json::to_string()`, deserialize with `serde_json::from_str()`.
- `format_evidence_chain_link` must produce deterministic output with sorted components for compare diff parity.

---

**Language**: English
