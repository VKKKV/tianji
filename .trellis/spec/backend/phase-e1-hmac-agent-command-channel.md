# Phase E1 — HMAC-Signed Agent Command Channel

## Goal

Add a local daemon API endpoint that allows external AI agents to submit signed commands into TianJi without exposing an unauthenticated write surface.

Endpoint:

```text
POST /api/v1/agent/command
```

## Scope

In scope:

- Add request/response structs for agent commands.
- Add HMAC-SHA256 verification using dependencies already present or minimal cryptographic crates if necessary.
- Signature material:
  - timestamp header
  - nonce header
  - raw request body digest
- Gate command scopes by access tier:
  - `restricted`: read/query/observe style commands only
  - `full`: mutation/simulation commands
- Add nonce replay protection with bounded in-memory state.
- Keep failures generic enough to avoid leaking secrets.
- Add offline tests for valid signature, bad signature, stale timestamp, nonce replay, and tier denial.

Out of scope:

- Persisted secret management.
- Real external agent execution.
- Network calls.
- SSE push.

## Contract

Headers:

```text
x-tianji-agent-id: <id>
x-tianji-agent-tier: restricted|full
x-tianji-timestamp: <unix seconds>
x-tianji-nonce: <unique nonce>
x-tianji-signature: hex(hmac_sha256(secret, timestamp + "\n" + nonce + "\n" + sha256(body)))
```

Body:

```json
{
  "command_id": "uuid-or-client-id",
  "command_type": "observe|query|simulate|intervene",
  "payload": {}
}
```

Response:

```json
{
  "accepted": true,
  "command_id": "...",
  "agent_id": "...",
  "tier": "restricted|full",
  "command_type": "..."
}
```

For this phase the endpoint may acknowledge and validate only; it does not need to mutate simulation state.

## Verification

Run:

```bash
cargo fmt
cargo test --quiet agent_command
cargo test --quiet
cargo clippy -- -D warnings
git diff --check
```
