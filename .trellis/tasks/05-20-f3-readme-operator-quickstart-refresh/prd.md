# Phase F3 — README Operator Quickstart Refresh

## Goal

Refresh `README.md` so a new operator can run TianJi locally, validate configuration, inspect the daemon API, understand signed agent command ingress, perform alert-dispatch dry runs, and use TUI simulation replay without external services or real credentials.

## Scope

1. README operator quickstart
   - Keep the first-run path local-first: build, fixture run, optional SQLite persistence, `doctor`, daemon/API, TUI.
   - Prefer `cargo run -- ...` examples for source checkout users and mention installed `tianji` only where appropriate.
   - Keep commands copy-pasteable from repo root.

2. LLM/config documentation
   - Document `examples/config.example.yaml` and default `~/.tianji/config.yaml` usage.
   - Explain that deterministic Cangjie/Fuxi runs need no LLM, API key, or network.
   - Explain provider-backed Hongmeng/Nuwa simulation is optional and driven by configured providers.
   - Show environment-variable based credential references only; do not show raw keys.

3. Daemon/API documentation
   - Document default daemon API base URL: `http://127.0.0.1:8765/api/v1`.
   - List implemented endpoints including `POST /api/v1/agent/command`.
   - State the stable JSON envelope pattern.

4. Signed command channel documentation
   - Document required headers: `x-tianji-agent-id`, `x-tianji-agent-tier`, `x-tianji-timestamp`, `x-tianji-nonce`, `x-tianji-signature`.
   - Document HMAC message format: `timestamp + "\n" + nonce + "\n" + sha256(body)`.
   - Use dummy/test-only secret values only.
   - Do not include a live secret or imply exposing the endpoint beyond loopback.

5. Alert dispatch dry-run documentation
   - Explain dry-run planning/redaction behavior for Telegram, Discord, and generic webhook channels.
   - Keep webhook URLs and tokens dummy/redacted.
   - Do not recommend live dispatch as the first quickstart path.

6. TUI replay documentation
   - Document history navigation keybindings and simulation replay scrubbing: `Left`/`h`, `Right`/`l`.
   - Keep wording aligned with current read-only terminal surface.

## Non-goals

- No code changes unless README inspection reveals a tiny command/doc mismatch that must be fixed.
- No live LLM/model endpoint tests.
- No external network dispatch.
- No real credentials in README, fixtures, tests, or logs.

## Verification

- `cargo fmt`
- `cargo test --quiet`
- `cargo clippy -- -D warnings`
- `git diff --check`
