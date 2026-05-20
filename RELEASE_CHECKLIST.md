# TianJi Release Readiness Checklist

Date: 2026-05-20
Scope: Phase F4 local release readiness check

## Result

Status: PASS

TianJi is release-ready for a local-first Rust binary checkpoint under the checked scope below.

## Build

Command:

```bash
cargo build --release
```

Result: PASS

Release binary:

```text
target/release/tianji
```

Measured size:

```text
15338616 bytes target/release/tianji
14.63 MiB
```

Binary size target:

```text
< 25,000,000 bytes: PASS
```

## Shell completions

Commands:

```bash
cargo run --quiet -- completions bash >/tmp/tianji.bash
cargo run --quiet -- completions zsh >/tmp/_tianji
cargo run --quiet -- completions fish >/tmp/tianji.fish
wc -c /tmp/tianji.bash /tmp/_tianji /tmp/tianji.fish
```

Result: PASS

Generated output sizes:

```text
 44699 /tmp/tianji.bash
 29050 /tmp/_tianji
 27028 /tmp/tianji.fish
100777 total
```

## Fixture smoke run

Command:

```bash
cargo run --quiet -- run --fixture tests/fixtures/sample_feed.xml >/tmp/tianji-run.json
```

Validation:

```bash
python3 - <<'PY'
import json
from pathlib import Path
p = Path('/tmp/tianji-run.json')
data = json.loads(p.read_text())
assert data['schema_version'] == 'tianji.run-artifact.v1'
assert data['mode'] == 'fixture'
assert len(data.get('scored_events', [])) > 0
print(json.dumps({
    'schema_version': data['schema_version'],
    'mode': data['mode'],
    'scored_events': len(data.get('scored_events', [])),
    'intervention_candidates': len(data.get('intervention_candidates', [])),
    'dominant_field': data.get('scenario_summary', {}).get('dominant_field'),
    'risk_level': data.get('scenario_summary', {}).get('risk_level'),
}, ensure_ascii=False, sort_keys=True))
PY
```

Result: PASS

Smoke summary:

```json
{"dominant_field": "technology", "intervention_candidates": 3, "mode": "fixture", "risk_level": "high", "schema_version": "tianji.run-artifact.v1", "scored_events": 3}
```

## Regression gate

Commands:

```bash
cargo test --quiet
cargo clippy -- -D warnings
git diff --check
```

Result: PASS

Test result:

```text
341 unit passed
39 integration passed
0 failed
```

Clippy result:

```text
No warnings with -D warnings
```

Whitespace check:

```text
git diff --check: PASS
```

## Safety notes

- No live LLM/model endpoint was started or called.
- No Telegram, Discord, generic webhook, or other external alert dispatch was called.
- All smoke inputs were checked-in local fixtures.
- Transient generated files were written under `/tmp`, not the repository.
- No secrets or credentials are included in this checklist.

## Release-scope non-goals

Not performed in F4:

- Publishing a release artifact.
- Creating a git tag.
- Pushing to a remote.
- Live provider/API credential validation.
