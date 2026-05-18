# TianJi v0.2.1 Code Review

Date: 2026-05-17 | Scope: 52 .rs files, ~20K lines
Build: pass | Test: 28/28 | Clippy: clean

## Critical (15)

### C1. RiskDirection semantic inversion — delta.rs:559-561
`infer_risk_direction` returns RiskOff when risk_up > risk_down+1. In finance,
RiskOff = de-risking/safety, opposite of "escalation increasing". Internally
consistent (delta_memory.rs:299 triggers Flash on RiskOff) but serialized JSON
misleads external consumers. Fix: rename enum or fix mapping.

### C2. average() no divide-by-zero — delta.rs:475-476
`fn average(values: &[f64]) -> f64` does `sum() / len()`. Empty slice panics.
Fix: `if values.is_empty() { return 0.0; }`

### C3. round2 NaN/Inf panic — utils.rs:8-10
`format!("{:.2}", value).parse::<f64>().expect(...)` — "NaN".parse() returns
Err → expect panic. Fix: `.parse().unwrap_or(value)` or `.is_finite()` guard.

### C4. insert_raw_items uses raw hash not derived — storage.rs:132-133
`ensure_canonical_source_items` derives hashes when fields are empty, but
`insert_raw_items` uses `item.entry_identity_hash.clone()` directly. Empty
hash → key mismatch → transaction rollback.
Fix: reuse derivation logic from ensure_canonical_source_items.

### C5. SQLite no WAL mode — storage.rs:43-44
Only `PRAGMA foreign_keys=ON`, no `journal_mode=WAL`. Concurrent writes block
reads with SQLITE_BUSY. Fix: `PRAGMA journal_mode=WAL` in initialize_schema.

### C6. Delta endpoint path traversal — api.rs:340
`get_latest_delta` takes user-controlled `sqlite_path` query param. Attacker
can read arbitrary files. Fix: remove query param, use AppState.sqlite_path only.

### C7. Sync I/O blocks TUI render thread — tui/state.rs:459-475
`load_detail_state` calls `get_run_summary` synchronously from key handler.
UI freezes for hundreds of ms. Fix: tokio::spawn_blocking + oneshot channel.

### C8. Terminal Resize silently dropped — tui/mod.rs:402-405
Only `Event::Key` handled; `Event::Resize` skipped. Layout breaks on resize.
Fix: call `terminal.resize()` on Resize event.

### C9. New reqwest::Client per LLM request — llm/client.rs:88,148
`reqwest::Client::new()` inside `chat_openai_compatible` and `chat_ollama` —
no connection pool reuse, TLS handshake every call.
Fix: store Client as field in LlmClient, construct once.

### C10. is_multiple_of panics on checkpoint_interval=0 — hongmeng/simulation.rs:192
`self.tick.is_multiple_of(self.config.checkpoint_interval)` — divide by zero.
Default 5 is safe but user-configurable to 0.
Fix: validate checkpoint_interval > 0 in Hongmeng::new.

### C11. parse_llm_action swallows parse errors — hongmeng/agent.rs:253-274
`serde_json::from_str` failure returns `Ok(AgentAction{action_type:"observe"})`
instead of Err. LLM format errors silently treated as valid observe.
Fix: return `Result<AgentAction, LlmError>`, let fallback chain handle.

### C12. Worldline.causal_graph #[serde(skip)] — worldline/types.rs:63-64
`causal_graph: DiGraph<EventId, CausalRelation>` skipped in serialization.
After checkpoint restore, causal graph is empty, breaking nuwa backward analysis.
Fix: implement custom Serialize/Deserialize as edge list.

### C13. Branch final_divergence uses wrong worldline — nuwa/forward.rs:179
`final_divergence: worldline.divergence + offset*0.5` uses main branch's
divergence, not `alt_worldline.divergence` (computed at line 170).
Fix: `alt_worldline.divergence + offset as f64 * 0.5`.

### C14. working_worldline.divergence never updated — nuwa/backward.rs:142
`path_probability = 1.0/(1.0+working_worldline.divergence)` — divergence
is always 0 from clone. W2_PATH_PROBABILITY weight 0.2 is dead.
Fix: call `compute_divergence_from` after each field update.

### C15. Interactive branches vector never populated — nuwa/forward.rs:407,517
`branches: Vec::new()` initialized but never pushed to. Pruning `retain`
operates on always-empty vec. TUI pruning is dead code.
Fix: push BranchSummary in branch generation logic.

## High (10)

H1. API errors all map to 500 + leak internal paths — api.rs:159-161
H2. save_baseline DELETE+INSERT not in transaction — storage.rs:1646-1650
H3. Checkpoint save error silently discarded — hongmeng/simulation.rs:210
H4. Unbounded simulation channel — tui/mod.rs:90
H5. LLM HTTP requests have no timeout — llm/client.rs:88-96,149-154
H6. check_convergence prev_fields completely unused — hongmeng/convergence.rs:88
H7. choices[0] no bounds check — llm/client.rs:115-117
H8. serde_json::Value overuse destroys type safety — multi-file
H9. No input size limits (DoS/OOM) — fetch.rs, normalize.rs
H10. O(n²) Vec::contains dedup in extract_keywords — normalize.rs:154-167

## Fix Priority Order
C5 → C9 → C4 → C13/C14/C15 → C6 → C3 → C11 → C10 → C2 → C1
