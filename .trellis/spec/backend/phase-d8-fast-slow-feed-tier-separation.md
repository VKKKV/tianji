# Phase D8 — Fast/Slow Feed Tier Separation

## Goal

Extend TianJi watch scheduling so multiple watched feeds can be grouped by urgency, with separate fast/slow intervals. This reduces unnecessary fetch/API/LLM cost while preserving the existing single-feed `watch --source-url --interval` compatibility path.

## Scope

In scope:

- Add feed tier model in `src/daemon.rs` or `src/main.rs` near watch code.
- Support fast/slow watched feeds and interval validation.
- Preserve existing `Cli::Watch { source_url, interval, sqlite_path, config }` behavior.
- Add testable helper functions for scheduling and injected fetcher execution.
- JSON watch output should expose configured tiers/intervals and per-feed results.

Out of scope:

- Infinite production daemon scheduler refactor.
- Async runtime migration for watch.
- Real network integration beyond existing `fetch_feed_url`.
- Full config-file parsing if it would sprawl; CLI/testable structs are enough for this phase.

## Terms

Feed tier:

- `fast`: urgent feeds, lower interval.
- `slow`: background feeds, higher interval.

Suggested defaults:

- fast interval: 30 seconds
- slow interval: 300 seconds

Validation:

- fast interval >= 10 seconds
- slow interval >= fast interval
- at least one watched feed
- each feed URL must be non-empty

## Compatibility

Existing single-feed watch must keep working:

```bash
tianji watch --source-url https://example.com/feed.xml --interval 60
```

Its output contract may gain optional fields, but current tests checking `watch.source_url`, `watch.interval`, and `watch.iterations` must continue to pass.

## Acceptance Criteria

- Fast/slow feed tier data structures exist and are deterministic.
- Scheduling helper returns due feeds by tier for each tick/iteration.
- Existing single-feed watch tests still pass.
- New tests cover:
  - invalid intervals rejected
  - slow interval cannot be below fast interval
  - fast feeds run more frequently than slow feeds
  - injected fetcher processes multiple tiered feeds without real network
  - JSON output includes tier metadata
- `cargo fmt` passes.
- `cargo test --quiet watch` or focused equivalent passes.
- `cargo test --quiet` passes.
- `cargo clippy -- -D warnings` passes.
- `git diff --check` passes.
