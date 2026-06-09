# Credential-free replay smoke corpus

## Goal

Finish the Post-K candidate "Expand the credential-free evaluation corpus and replay smoke checks" with a small local-first slice.

## Scope

Add a credential-free smoke script/gate that exercises replay bundle export and trace-backed TUI render-once without provider config, network, daemon, or secrets.

Expected behavior:

- The smoke path creates transient files only under `/tmp`.
- It runs `tianji predict --field <field> --horizon <small N> --replay-bundle-dir <tmp-dir>` and captures stdout outcome JSON.
- It runs `tianji tui --replay-bundle-dir <tmp-dir> --render-once` and captures text output.
- It validates the bundle contains only `manifest.json`, `trace.jsonl`, and `outcome.json`.
- It validates JSON schemas/record kinds enough to catch broken trace/bundle generation.
- It validates TUI render text includes selected frame/audit signal text.
- Add focused test coverage or script coverage consistent with existing repository patterns.
- Document the smoke gate in README and plan.

## Non-goals

- No provider-backed simulation.
- No live feed fetch.
- No daemon/API/web UI changes.
- No schema or CLI flag changes unless required by a bug discovered during implementation.

## Verification

Run:

```bash
cargo fmt --check
bash scripts/check-eval.sh
bash scripts/check-replay-smoke.sh
cargo test --quiet
cargo clippy -- -D warnings
git diff --check
```
