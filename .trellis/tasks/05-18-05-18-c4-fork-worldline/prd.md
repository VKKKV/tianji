# PRD — Phase C4: fork_worldline Unification

> Priority: C (medium-high risk) | Spec: .trellis/spec/backend/phase-c4-fork-worldline.md
> Trait-based decoupling of worldline forking from SQLite

## Goal

1. Extract `WorldlineStore` trait for worldline persistence
2. Make `fork_worldline` public and generic over the trait
3. Remove `rusqlite::Connection` from Nuwa signatures
4. All worldline branching uses a single `fork_worldline` entry point

## Steps

### Step 1 — Create WorldlineStore trait

NEW file: `src/worldline/store.rs`

```rust
pub trait WorldlineStore {
    fn next_id(&self) -> Result<WorldlineId, TianJiError>;
    fn save(&self, worldline: &Worldline) -> Result<(), TianJiError>;
}
```

Implementations:
- `MemoryStore` — AtomicU64 counter, in-memory Vec (for tests, no persistence)
- `SqliteStore` — wraps `rusqlite::Connection`, delegates to storage module

### Step 2 — Add module declaration

File: `src/worldline.rs` — add `pub mod store;`

### Step 3 — Refactor fork_worldline

File: `src/nuwa/sandbox.rs`

- Remove current private `fork_worldline`
- Add public `pub fn fork_worldline(base: &Worldline, store: Option<&dyn WorldlineStore>) -> Worldline`
- ID generation: `store.next_id()` with AtomicU64 fallback
- Persistence: `store.save(&forked)` when store is Some

### Step 4 — Update NuwaSandbox::new

File: `src/nuwa/sandbox.rs`

Change from `fork_worldline(&base_worldline, None)` to `fork_worldline(&base_worldline, None)`.
No behavioral change — None means no persistence, same as current.

### Step 5 — Update simulation callers

File: `src/nuwa/forward.rs`

- `run_forward` (line 35-37): replace manual clone with `fork_worldline(base, None)`
- `run_interactive_forward` (line 401-403): same
- `run_backward`: any worldline cloning in intervention paths

Verify: parent/diverge_tick set correctly, hash recomputed.

### Step 6 — Remove rusqlite from Nuwa signatures

File: `src/nuwa/sandbox.rs`

- Remove `use rusqlite::Connection;` if only used by old fork_worldline
- `NuwaSandbox` no longer imports or uses `rusqlite`

## Key Files

| Action | File | Lines |
|--------|------|-------|
| WorldlineStore trait + impls | NEW src/worldline/store.rs | +80 |
| Module declaration | src/worldline.rs | +1 |
| Refactor fork_worldline | src/nuwa/sandbox.rs | ~30 (rewrite) |
| Update callers | src/nuwa/forward.rs | ~15 |
| Update callers | src/nuwa/backward.rs | ~5 |
| Remove rusqlite import | src/nuwa/sandbox.rs | -1 |

## Commands

```bash
cargo build && cargo test && cargo clippy -- -D warnings && cargo fmt
# sandbox tests: cargo test -- nuwa::sandbox::tests
# smoke: cargo run -- predict --field global.conflict --horizon 3
```
