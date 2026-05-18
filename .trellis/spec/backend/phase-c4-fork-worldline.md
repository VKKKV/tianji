# Phase C4 — fork_worldline Unification

> Status: spec | Risk: medium-high | Files: ~3 | Trait-based decoupling

## Current State

`fork_worldline` is a private `fn` in `src/nuwa/sandbox.rs:60-81`:

```rust
fn fork_worldline(base: &Worldline, conn: Option<&rusqlite::Connection>) -> Worldline
```

Problems:
1. Coupled to `rusqlite::Connection` — makes testing hard, ties Nuwa to SQLite
2. `pub(crate)` visibility not possible as private fn — callers in other modules can't use it
3. ID generation fallback (`AtomicU64` static) duplicates `next_worldline_id` logic
4. No trait abstraction — can't swap storage backend (in-memory for tests, SQLite for prod)

## Design — `WorldlineStore` trait + public `fork_worldline`

### Step 1: Extract `WorldlineStore` trait

```rust
// src/worldline/store.rs (NEW)
pub trait WorldlineStore {
    fn next_id(&self) -> Result<WorldlineId, TianJiError>;
    fn save(&self, worldline: &Worldline) -> Result<(), TianJiError>;
}

// In-memory implementation for tests
pub struct MemoryStore {
    counter: AtomicU64,
}

// SQLite implementation
pub struct SqliteStore {
    conn: rusqlite::Connection,
}
```

### Step 2: Make `fork_worldline` public + generic

```rust
// src/nuwa/sandbox.rs
pub fn fork_worldline(
    base: &Worldline,
    store: Option<&dyn WorldlineStore>,
) -> Worldline {
    let new_id = match store {
        Some(s) => s.next_id().unwrap_or_else(|_| {
            static COUNTER: AtomicU64 = AtomicU64::new(1);
            COUNTER.fetch_add(1, Ordering::Relaxed)
        }),
        None => {
            static COUNTER: AtomicU64 = AtomicU64::new(1);
            COUNTER.fetch_add(1, Ordering::Relaxed)
        }
    };
    let mut forked = base.clone();
    forked.id = new_id;
    forked.parent = Some(base.id);
    forked.diverge_tick = 0;
    forked.snapshot_hash = Worldline::compute_snapshot_hash(&forked.fields);
    forked.created_at = chrono::Utc::now();

    if let Some(s) = store {
        let _ = s.save(&forked);
    }
    forked
}
```

### Step 3: Update callers

- `NuwaSandbox::new` (sandbox.rs:46) → `fork_worldline(&base_worldline, None)` → unchanged
- `run_forward` (forward.rs:35-37) → currently clones manually; replace with `fork_worldline(base, None)`
- `run_interactive_forward` (forward.rs:401-403) → same
- `run_backward` (backward.rs) → any worldline cloning for intervention paths

### Step 4: Remove `rusqlite::Connection` from Nuwa signatures

- `save_worldline(db, &forked)` call in fork_worldline → `store.save(&forked)`
- `NuwaSandbox` no longer needs to know about SQLite

## Files Changed

- NEW `src/worldline/store.rs` — `WorldlineStore` trait, `MemoryStore`, `SqliteStore`
- `src/worldline.rs` — `pub mod store;`
- `src/nuwa/sandbox.rs` — extract `fork_worldline`, make public, generic over trait
- `src/nuwa/forward.rs` — replace manual clone+setup with `fork_worldline`
- `src/nuwa/backward.rs` — same for intervention path worldlines

## Verification

```bash
cargo build && cargo test && cargo clippy -- -D warnings
# All 310 tests pass
# cargo test -- nuwa::sandbox::tests — existing fork tests still pass
# cargo run -- predict --field global.conflict --horizon 3 — identical output
```

## Pitfall

- `dyn WorldlineStore` requires the trait to be object-safe — avoid generic methods with type params
- `rusqlite::Connection` is not `Send` — if `SqliteStore` wraps it, spawning with `Arc<Mutex<>>` may be needed for async contexts
