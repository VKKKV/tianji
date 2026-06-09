#!/usr/bin/env bash
set -euo pipefail

tmp_root="${TMPDIR:-/tmp}"
if [[ "$tmp_root" != "/tmp" && "$tmp_root" != /tmp/* ]]; then
  echo "TMPDIR must resolve under /tmp for replay smoke output: $tmp_root" >&2
  exit 1
fi

workdir="$(mktemp -d "$tmp_root/tianji-replay-smoke.XXXXXX")"
cleanup() {
  rm -rf "$workdir"
}
trap cleanup EXIT

bundle_dir="$workdir/replay-bundle"
outcome_json="$workdir/outcome.stdout.json"
tui_text="$workdir/tui-render.txt"

cargo run --quiet -- predict \
  --field global.conflict \
  --horizon 2 \
  --replay-bundle-dir "$bundle_dir" \
  >"$outcome_json"

cargo run --quiet -- tui \
  --replay-bundle-dir "$bundle_dir" \
  --render-once \
  >"$tui_text"

python3 - "$workdir" "$bundle_dir" "$outcome_json" "$tui_text" <<'PY'
import json
import sys
from pathlib import Path

workdir = Path(sys.argv[1])
bundle_dir = Path(sys.argv[2])
outcome_path = Path(sys.argv[3])
tui_path = Path(sys.argv[4])

if not workdir.is_relative_to(Path('/tmp')):
    raise SystemExit(f'smoke workdir escaped /tmp: {workdir}')

bundle_files = sorted(path.name for path in bundle_dir.iterdir() if path.is_file())
expected_files = ['manifest.json', 'outcome.json', 'trace.jsonl']
if bundle_files != expected_files:
    raise SystemExit(f'unexpected replay bundle files: {bundle_files}')

stdout_outcome = json.loads(outcome_path.read_text())
bundle_outcome = json.loads((bundle_dir / 'outcome.json').read_text())
if stdout_outcome != bundle_outcome:
    raise SystemExit('predict stdout outcome does not match bundle outcome.json')
if stdout_outcome.get('mode', {}).get('forward', {}).get('horizon_ticks') != 2:
    raise SystemExit('predict outcome missing expected forward horizon')
if stdout_outcome.get('mode', {}).get('forward', {}).get('target_field') != 'global:conflict':
    raise SystemExit('predict outcome missing expected forward target field')
if 'branches' not in stdout_outcome or not isinstance(stdout_outcome['branches'], list):
    raise SystemExit('predict outcome missing branches array')

manifest = json.loads((bundle_dir / 'manifest.json').read_text())
if manifest.get('schema_version') != 'tianji.replay-bundle.v1':
    raise SystemExit(f"bad bundle schema_version: {manifest.get('schema_version')}")
if manifest.get('trace_file') != 'trace.jsonl' or manifest.get('outcome_file') != 'outcome.json':
    raise SystemExit('manifest must reference trace.jsonl and outcome.json')
if manifest.get('target_field') != 'global:conflict':
    raise SystemExit(f"bad target_field: {manifest.get('target_field')}")
if manifest.get('horizon_ticks') != 2:
    raise SystemExit(f"bad horizon_ticks: {manifest.get('horizon_ticks')}")
if manifest.get('trace_bytes') != (bundle_dir / 'trace.jsonl').stat().st_size:
    raise SystemExit('manifest trace_bytes mismatch')
if manifest.get('outcome_bytes') != (bundle_dir / 'outcome.json').stat().st_size:
    raise SystemExit('manifest outcome_bytes mismatch')

trace_records = [
    json.loads(line)
    for line in (bundle_dir / 'trace.jsonl').read_text().splitlines()
    if line.strip()
]
if len(trace_records) != manifest.get('frame_count', 0) + 2:
    raise SystemExit('trace record count does not match manifest frame_count')
if trace_records[0].get('record_type') != 'metadata':
    raise SystemExit('first trace record must be metadata')
if trace_records[0].get('schema_version') != 'tianji.sim-trace.v1':
    raise SystemExit('trace metadata schema mismatch')
if trace_records[0].get('frame_count') != manifest.get('frame_count'):
    raise SystemExit('trace metadata frame_count mismatch')
frames = [record for record in trace_records if record.get('record_type') == 'frame']
if not frames:
    raise SystemExit('trace must contain at least one frame record')
if any('tick' not in frame or 'agent_actions' not in frame for frame in frames):
    raise SystemExit('frame records must include tick and agent_actions')
if trace_records[-1].get('record_type') != 'completed':
    raise SystemExit('last trace record must be completed')
if trace_records[-1].get('outcome') != bundle_outcome:
    raise SystemExit('trace completed outcome does not match bundle outcome.json')
if [record.get('record_type') for record in trace_records].count('metadata') != 1:
    raise SystemExit('trace must contain exactly one metadata record')
if [record.get('record_type') for record in trace_records].count('completed') != 1:
    raise SystemExit('trace must contain exactly one completed record')

tui_text = tui_path.read_text()
required_text = [
    'status: replay loaded',
    'frame metadata:',
    'Field changes',
    'event sequence length',
    'Agent audit',
]
missing = [text for text in required_text if text not in tui_text]
if missing:
    raise SystemExit(f'TUI render missing expected text: {missing}')

print(json.dumps({
    'bundle_files': bundle_files,
    'bundle_schema_version': manifest['schema_version'],
    'frame_count': manifest['frame_count'],
    'trace_record_count': len(trace_records),
    'tui_render_bytes': len(tui_text.encode()),
}, sort_keys=True))
PY
