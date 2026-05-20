# Phase I — Source/feed Management

## Purpose

Phase I turns TianJi's single-input feed path into an explicit source registry
without making the product depend on live network access. The operator should be
able to define source metadata in YAML, enable/disable sources, group them by tier,
and inspect registry health/status locally.

The registry is an operator control plane for feed inputs. It is not a scoring
change and not an LLM/simulation feature.

## Principles

1. Local-first default
   - Checked-in examples must use local fixture paths or dummy URLs.
   - Tests must not require network, daemon, LLM provider, or credentials.

2. Explicit source identity
   - Each source has a stable `id` used in reports/status and future persistence.
   - Display names and URLs/paths are metadata, not identity.

3. Safe config
   - No tokens, API keys, passwords, cookies, or private feed URLs in examples/tests.
   - If credentialed feeds are ever added, credentials must be referenced through env vars, never printed.

4. Bounded first slice
   - Start with manifest loading, validation, listing, and fixture-run fan-in.
   - Defer live polling metadata persistence to later Phase I tasks.

## Registry schema

Canonical sample path:

```text
examples/sources.example.yaml
```

Suggested YAML shape:

```yaml
sources:
  - id: sample_technology
    name: Sample technology fixture
    enabled: true
    tier: primary
    kind: fixture
    path: tests/fixtures/sample_feed.xml
    tags: [technology, demo]

  - id: economy_fixture
    name: Economy fixture
    enabled: true
    tier: secondary
    kind: fixture
    path: tests/fixtures/economy_feed.xml
    tags: [economy, demo]

  - id: disabled_dummy_remote
    name: Disabled dummy remote feed
    enabled: false
    tier: watchlist
    kind: rss
    url: https://example.invalid/feed.xml
    tags: [dummy]
```

Required fields:

- `id`: non-empty stable slug; unique within the file.
- `name`: non-empty operator label.
- `enabled`: boolean.
- `tier`: non-empty operator tier such as `primary`, `secondary`, `watchlist`.
- `kind`: one of `fixture`, `rss`, `atom`.
- `path`: required for `fixture`.
- `url`: required for `rss`/`atom`.
- `tags`: optional list of labels.

Validation rules:

- Duplicate IDs fail validation.
- Missing `path` for fixture fails validation.
- Missing `url` for rss/atom fails validation.
- Unknown kind fails validation.
- Disabled sources are loaded and reported but not selected for fan-in runs.

## First-slice CLI

Preferred command:

```bash
tianji sources --config examples/sources.example.yaml
```

Default behavior prints JSON summary:

```json
{
  "schema_version": "tianji.sources-report.v1",
  "config": "examples/sources.example.yaml",
  "total": 3,
  "enabled": 2,
  "disabled": 1,
  "tiers": {"primary": 1, "secondary": 1, "watchlist": 1},
  "sources": []
}
```

Optional first-slice run fan-in:

```bash
tianji sources --config examples/sources.example.yaml --run-fixtures
```

`--run-fixtures` should:

- run enabled `kind: fixture` sources through the deterministic pipeline;
- ignore disabled sources;
- reject `rss`/`atom` live network fetching in first slice unless explicitly designed later;
- return non-zero if any selected fixture cannot be read/parsed;
- emit JSON with per-source status and artifact counts, not full artifacts by default.

## Acceptance for I1

1. Add typed Rust registry support, preferably `src/source_registry.rs`.
2. Add `examples/sources.example.yaml` with only fixture/dummy-example sources.
3. Add `tianji sources --config <PATH>` JSON summary.
4. Add `--run-fixtures` deterministic local fan-in for enabled fixture sources.
5. Add tests for valid load, duplicate ID rejection, missing fixture path rejection, disabled source exclusion, and CLI parsing.
6. Update README/plan with Phase I start state.

## Verification

```bash
cargo fmt
cargo test --quiet source
cargo test --quiet
cargo clippy -- -D warnings
git diff --check
cargo run --quiet -- sources --config examples/sources.example.yaml
cargo run --quiet -- sources --config examples/sources.example.yaml --run-fixtures
```
