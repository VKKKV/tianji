# I2-I4 — Finish Source/feed Management

## Goal

Finish Phase I by extending the I1 local source registry into an operator-ready
source/feed management layer. The final Phase I shape should support:

- I2: source health summaries with last-success/error style status in JSON reports;
- I3: registry-driven live feed fetching for explicitly enabled rss/atom sources;
- I4: docs, CI-friendly smoke commands, and roadmap completion.

Phase I must remain safe by default: examples/tests use local fixtures and dummy
URLs only. Live fetching is opt-in from operator-provided registries.

## Requirements

1. Source report health
   - Extend `tianji sources --config <PATH>` JSON with per-source health/status fields.
   - Include enough metadata for operators to see whether a source is enabled, runnable, skipped, or failing.
   - Add aggregate counters for ready/skipped/error if practical.
   - Do not persist health to SQLite in this phase unless it is small and well-tested; in-memory command report is acceptable for Phase I completion.

2. Registry-driven fetching/running
   - Add an explicit flag for network fetching, e.g. `--fetch-live` or equivalent.
   - `--run-fixtures` continues to run enabled fixture sources only.
   - Live rss/atom fetching must be opt-in; plain `tianji sources --config ...` must not make network calls.
   - Disabled sources must never be fetched or run.
   - Tests must use mocked/local HTTP where needed, or avoid network entirely.
   - Example file must keep `example.invalid` dummy URL disabled.

3. Run output
   - Fixture and live runs should use the same concise per-source report shape:
     - source id
     - kind
     - status
     - item/event/candidate counts on success
     - dominant field/risk level on success
     - redacted/non-secret error string on failure
   - Full artifacts should not be printed in source management by default.

4. CLI/docs
   - README documents:
     - source registry validation/listing;
     - fixture fan-in;
     - explicit live fetch opt-in and its safety constraints;
     - local smoke command(s).
   - `plan.md` marks Phase I complete when verified.

5. Tests
   - Add unit/integration tests for:
     - report health/aggregate fields;
     - disabled source remains skipped;
     - live fetching flag is parsed;
     - live fetch path succeeds against a deterministic local mock or injected fetcher;
     - live fetch path is never triggered by default listing.

## Non-goals

- No credentialed feed examples.
- No background scheduling redesign.
- No daemon API expansion unless small and necessary.
- No scoring changes.
- No dependency on external internet in tests.

## Verification commands

```bash
cargo fmt
cargo test --quiet source
cargo test --quiet
cargo clippy -- -D warnings
git diff --check
cargo run --quiet -- sources --config examples/sources.example.yaml
cargo run --quiet -- sources --config examples/sources.example.yaml --run-fixtures
```

If a live-fetch smoke command is added, it must use a local/mock source or be documented as optional and not required for CI.
