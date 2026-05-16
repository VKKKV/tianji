# Phase 5.1: Live Feed Watch

> Part of plan.md next-step operational completion
> Target: make `tianji watch --source-url` fetch real RSS/Atom feeds instead of replaying fixture data
> Status: implemented (Phase 5.1)

## Goal

`watch` should execute the same deterministic feed pipeline used by fixture runs,
but source feed text from the provided HTTP/HTTPS URL.

## Behavior

1. `--interval` must remain at least 10 seconds.
2. `--source-url` must be HTTP or HTTPS.
3. Watch remains bounded to 3 iterations for CLI safety.
4. Each iteration fetches the feed URL, parses RSS/Atom, normalizes, scores, and
   optionally persists to SQLite.
5. Iteration JSON includes:
   - `iteration`
   - `source_url`
   - `status`
   - `raw_item_count`
   - `normalized_event_count`
   - `dominant_field`
   - `risk_level`
   - `headline`
6. Fetch/parse/pipeline errors are recorded per iteration as `{ status: "error", error: ... }`.

## Implementation Notes

- Library pipeline reuse is provided by `run_feed_text()` and
  `run_feed_text_with_alert_marking()` in `src/lib.rs`.
- `run_fixture_path_with_alert_marking()` now delegates to `run_feed_text_with_alert_marking()`.
- `handle_watch()` uses `fetch_feed_url()` with `reqwest::blocking::Client`.
- Tests use `handle_watch_with_fetcher()` dependency injection to avoid network and sleeps.
- No `reqwest` JSON feature is required.

## Verification

- `cargo fmt`
- `cargo test`
- `cargo clippy -- -D warnings`
