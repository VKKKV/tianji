# PRD — Phase E1: HMAC-Signed Agent Command Channel

> Priority: E1 | Spec: `.trellis/spec/backend/phase-e1-hmac-agent-command-channel.md`

## Goal

Expose a safe local API command ingress for external AI agents by adding `POST /api/v1/agent/command` with HMAC signing, timestamp freshness, nonce replay protection, and restricted/full tier gating.

## Requirements

1. API:
   - Add `POST /api/v1/agent/command` to `src/api.rs` router.
   - Preserve existing read API behavior.

2. Request validation:
   - Required headers: agent id, agent tier, timestamp, nonce, signature.
   - Raw body must be hashed with SHA-256.
   - Signature format is lower-hex HMAC-SHA256 over `timestamp + "\n" + nonce + "\n" + sha256(body)`.
   - Reject missing/malformed/stale/replayed/bad signatures.

3. Access tiers:
   - `restricted` allows `observe` and `query`.
   - `full` allows `observe`, `query`, `simulate`, `intervene`.

4. Replay protection:
   - Bounded in-memory nonce cache in `AppState` or adjacent helper.
   - Testable without wall-clock flakiness.

5. Safety:
   - Do not log or expose secret material.
   - Error responses should not include the expected signature or secret.

6. Tests:
   - valid signed command accepted
   - bad signature rejected
   - stale timestamp rejected
   - nonce replay rejected
   - restricted tier denied for mutation command

## Allowed Files

- `Cargo.toml` only if cryptographic dependency is needed
- `src/api.rs`
- `src/lib.rs` only if export needed
- focused tests in existing test module

## Verification

Run:

```bash
cargo fmt
cargo test --quiet agent_command
cargo test --quiet
cargo clippy -- -D warnings
git diff --check
```
