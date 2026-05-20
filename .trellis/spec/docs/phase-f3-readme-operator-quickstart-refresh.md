# Phase F3 — README Operator Quickstart Refresh

## Purpose

Keep TianJi's README operator path current with Phase D/E/F1/F2 surfaces without making the project look cloud-first or credential-required.

## Documentation rules

1. Local-first first
   - The first runnable examples must use local fixtures and no config.
   - Any LLM/provider, daemon write, or alert dispatch path must be labeled optional.

2. Credential-free examples
   - Use env-var references such as `OPENAI_API_KEY` rather than inline keys.
   - Use dummy placeholders (`dummy-test-secret`, `<redacted>`, `https://example.invalid/...`) for examples that need secret-shaped values.
   - Never paste a real token, webhook URL, model endpoint credential, or private path.

3. Current surfaces to document
   - `examples/config.example.yaml`
   - `tianji doctor [--config <PATH>] [--sqlite-path <PATH>] [--json]`
   - daemon API at `/api/v1`, including `/api/v1/meta`, runs, compare, delta, and `POST /api/v1/agent/command`
   - signed command headers and HMAC payload construction
   - alert dispatch dry-run/redaction behavior
   - TUI simulation replay keybindings: `Left`/`h`, `Right`/`l`

4. Stability
   - README should describe shipped behavior, not future architecture.
   - Avoid changing endpoint names, schema names, or command flags without confirming in code/tests.
   - Keep shell examples runnable from repo root.

## Quality gate

Run:

```bash
cargo fmt
cargo test --quiet
cargo clippy -- -D warnings
git diff --check
```
