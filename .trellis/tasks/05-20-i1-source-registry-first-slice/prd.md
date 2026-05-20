# I1 — Source Registry First Slice

## Goal

Start Phase I by adding a local-first feed source registry. Operators should be able
to load a YAML source manifest, validate source metadata, list enabled/disabled
sources as JSON, and optionally run enabled fixture sources through the existing
deterministic pipeline.

## Scope

Implement the first slice only:

- YAML registry loading and validation;
- CLI JSON summary;
- deterministic fixture fan-in for enabled fixture sources;
- examples and docs;
- tests.

## Requirements

1. Add a typed registry module.
   - Preferred path: `src/source_registry.rs`.
   - Keep types serde-friendly.
   - Expose a report schema constant such as `tianji.sources-report.v1`.

2. Registry schema
   - Read `sources:` list from YAML.
   - Required fields:
     - `id`
     - `name`
     - `enabled`
     - `tier`
     - `kind`
     - `path` for `kind: fixture`
     - `url` for `kind: rss`/`kind: atom`
   - Optional field:
     - `tags`

3. Validation
   - Duplicate IDs fail.
   - Empty IDs/names/tiers fail.
   - Unknown kind fails.
   - Fixture source without `path` fails.
   - RSS/Atom source without `url` fails.
   - Example config must not contain real private URLs or secrets.

4. CLI
   - Add:
     ```bash
     tianji sources --config examples/sources.example.yaml
     tianji sources --config examples/sources.example.yaml --run-fixtures
     ```
   - Default output: JSON summary with total/enabled/disabled/tier counts and sources.
   - `--run-fixtures` output: same report plus per-enabled-fixture run status/counts.
   - Disabled sources must be reported but not run.
   - Live rss/atom fetching is out of scope for I1.

5. Examples/docs
   - Add `examples/sources.example.yaml`.
   - Update README with a short source registry section.
   - Update `plan.md` to mark Phase I started and I1 complete only after verification.

## Non-goals

- No live HTTP feed fetching.
- No daemon scheduling changes.
- No SQLite last-success/error persistence.
- No scoring changes.
- No credential handling.

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

## Acceptance

- Commands above pass.
- JSON output is valid and includes `schema_version`.
- Example registry is credential-free and uses local fixture paths or `example.invalid` dummy URL only.
- Trellis task validates and is archived after commit.
