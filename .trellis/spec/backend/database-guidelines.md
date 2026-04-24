# Database Guidelines

> Database patterns and conventions for this project.

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

**Language**: English
