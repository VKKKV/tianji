# PRD — Phase D8: Fast/Slow Feed Tier Separation

> Priority: D8 | Spec: `.trellis/spec/backend/phase-d8-fast-slow-feed-tier-separation.md`

## Goal

Add deterministic fast/slow feed scheduling for TianJi watch mode, preserving the existing single-feed watch interface.

## Requirements

1. Model:
   - feed tier enum: fast / slow
   - watched feed config with URL and tier
   - scheduler config with fast and slow intervals

2. Validation:
   - at least one feed
   - non-empty URLs
   - fast interval >= 10s
   - slow interval >= fast interval

3. Execution helpers:
   - deterministic due-feed scheduling
   - injectable fetcher/sleeper for offline tests
   - single-feed compatibility helper remains intact

4. Output:
   - existing `watch.source_url`, `watch.interval`, `watch.iterations` preserved for single-feed path
   - tiered output includes feed URL, tier, interval, iteration, status and result summary/error

5. Tests:
   - old watch tests still pass
   - invalid intervals rejected
   - fast feeds run more often than slow feeds
   - injected fetcher handles multiple feeds by tier

## Allowed Files

- `src/main.rs`
- optional tiny supporting code only if needed

## Verification

Run:

```bash
cargo fmt
cargo test --quiet watch
cargo test --quiet
cargo clippy -- -D warnings
git diff --check
```

## Completion Output

```text
DEV_DONE_D8 <summary>
```

or

```text
NEED_INPUT_D8 <reason>
```
