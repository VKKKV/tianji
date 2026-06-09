# TianJi Release Readiness Checklist

Date: 2026-06-09
Scope: Current local release readiness after Phase H/I/J/K replay/audit work

## Result

Status: PASS

TianJi remains release-ready for a local-first Rust binary checkpoint under the
checked scope below. The check is credential-free, uses checked-in fixtures or
deterministic local simulation, and writes transient artifacts under `/tmp`.

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
16245408 bytes target/release/tianji
15.49 MiB
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
 61405 /tmp/tianji.bash
 41131 /tmp/_tianji
 36787 /tmp/tianji.fish
139323 total
```

## Fixture smoke run

Command:

```bash
rm -f /tmp/tianji-release-run.sqlite3 /tmp/tianji-release-run.sqlite3-wal \
  /tmp/tianji-release-run.sqlite3-shm /tmp/tianji-run.json
cargo run --quiet -- run --fixture tests/fixtures/sample_feed.xml \
  --sqlite-path /tmp/tianji-release-run.sqlite3 >/tmp/tianji-run.json
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

## Replay/audit smoke run

Commands:

```bash
rm -rf /tmp/tianji-check-replay-bundle /tmp/tianji-check-trace.jsonl \
  /tmp/tianji-check-outcome.json /tmp/tianji-check-replay.txt \
  /tmp/tianji-check-bundle-replay.txt
cargo run --quiet -- predict --field east-asia.conflict --horizon 3 \
  --trace-jsonl /tmp/tianji-check-trace.jsonl \
  --replay-bundle-dir /tmp/tianji-check-replay-bundle \
  >/tmp/tianji-check-outcome.json
cargo run --quiet -- tui --trace-jsonl /tmp/tianji-check-trace.jsonl \
  --render-once >/tmp/tianji-check-replay.txt
cargo run --quiet -- tui --replay-bundle-dir /tmp/tianji-check-replay-bundle \
  --render-once >/tmp/tianji-check-bundle-replay.txt
```

Validation:

```bash
python3 - <<'PY'
import json
from pathlib import Path
out = json.loads(Path('/tmp/tianji-check-outcome.json').read_text())
trace_lines = [json.loads(line) for line in Path('/tmp/tianji-check-trace.jsonl').read_text().splitlines() if line.strip()]
manifest = json.loads(Path('/tmp/tianji-check-replay-bundle/manifest.json').read_text())
render = Path('/tmp/tianji-check-replay.txt').read_text()
bundle_render = Path('/tmp/tianji-check-bundle-replay.txt').read_text()
assert 'mode' in out and 'branches' in out
assert trace_lines[0]['schema_version'] == 'tianji.sim-trace.v1'
assert trace_lines[0]['record_type'] == 'metadata'
frames = [line for line in trace_lines if line.get('record_type') == 'frame']
assert frames
assert trace_lines[-1]['record_type'] == 'completed'
assert manifest['schema_version'] == 'tianji.replay-bundle.v1'
assert manifest['trace_file'] == 'trace.jsonl'
assert manifest['outcome_file'] == 'outcome.json'
for text in (render, bundle_render):
    assert 'status: replay loaded' in text
    assert 'Agent audit' in text
print(json.dumps({
    'trace_schema_version': trace_lines[0]['schema_version'],
    'trace_records': len(trace_lines),
    'frames': len(frames),
    'bundle_schema_version': manifest['schema_version'],
    'trace_render_bytes': len(render.encode()),
    'bundle_render_bytes': len(bundle_render.encode()),
}, sort_keys=True))
PY
```

Result: PASS

Replay summary:

```json
{"bundle_render_bytes": 3470, "bundle_schema_version": "tianji.replay-bundle.v1", "frames": 3, "trace_records": 5, "trace_render_bytes": 3470, "trace_schema_version": "tianji.sim-trace.v1"}
```

## Regression gate

Commands:

```bash
cargo fmt --check
cargo test --quiet
cargo clippy -- -D warnings
git diff --check
```

Result: PASS

Test result:

```text
445 cargo tests passed across 3 suites
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

Rust formatting:

```text
cargo fmt --check: PASS
```

## Safety notes

- No live LLM/model endpoint was started or called.
- No Telegram, Discord, generic webhook, or other external alert dispatch was called.
- Fixture smoke input was a checked-in local fixture; its SQLite smoke database
  was written under `/tmp`.
- Replay smoke used deterministic local simulation and TUI render-once only.
- Transient generated files were written under `/tmp`, not the repository.
- No secrets or credentials are included in this checklist.

## Release-scope non-goals

Not performed in this readiness check:

- Publishing a release artifact.
- Creating a git tag.
- Pushing to a remote.
- Live provider/API credential validation.
