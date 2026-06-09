# Release readiness docs refresh

## Goal

Refresh TianJi's post-K release/readiness documentation so a future operator or agent can continue from the current clean `main` state without following stale dirty-worktree handoff notes.

## Scope

Documentation-only.

Update or verify:

- `handoff.md` reflects the current clean repository state, not the pre-commit K3/K4 dirty stack.
- `README.md` and `plan.md` do not contradict current shipped replay/audit behavior or verification counters.
- Release/readiness instructions stay local-first, credential-free, and reproducible.
- Any validation outputs described in docs are backed by real commands.

## Non-goals

- No Rust code changes.
- No API/schema/CLI flag changes.
- No provider, webhook, daemon write-path, or network-dependent checks.
- No secret-bearing examples.

## Verification

Run at minimum:

```bash
rg -n "dirty worktree|not committed|443 tests|445 tests|30,412|30,413|Phase K|K3|K4|replay-bundle|trace-jsonl" handoff.md README.md plan.md .trellis/spec
cargo fmt --check
cargo test --quiet
cargo clippy -- -D warnings
git diff --check
```

If documentation records line/test counts, recompute them after final edits before committing.
