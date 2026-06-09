# PRD: K3/K4 TUI replay trace and audit viewer

## Goal

Complete remaining Phase K simulation replay/export work by making the Rust TUI simulation view consume replay trace frames and expose structured agent audit details.

## Current state

- K1 JSONL trace export exists via `tianji.sim-trace.v1` and `src/nuwa/trace.rs`.
- K2 replay bundles exist via `tianji.replay-bundle.v1` containing `manifest.json`, `trace.jsonl`, and `outcome.json`.
- TUI simulation view already has a replay cursor, but displayed fields/events remain current-state only.
- `SimAgent` currently only renders actor/status/last_action, while trace frames carry structured audit fields: action type, target, confidence, rationale, assessment, category, drivers.

## Requirements

### K3: real trace-backed frame navigation

1. Add a TUI-facing trace replay state that can be built from `SimulationTrace` frames.
2. Preserve existing live simulation behavior and pruning controls.
3. In simulation view, `Left`/`h` and `Right`/`l` must change the displayed frame, not only the frame counter.
4. Selected frame display must include at least:
   - frame/tick position
   - field values for that frame
   - field changes for that frame
   - event sequence length or equivalent frame metadata
5. Support loading a replay bundle or trace file into the simulation view from CLI if a small, coherent flag can be added without breaking existing `tianji tui --sqlite-path` behavior. Prefer minimal flags such as:
   - `tianji tui --replay-bundle-dir <DIR>`
   - and/or `tianji tui --trace-jsonl <PATH>`
6. Keep replay bundle parsing secret-safe: read only `manifest.json`, `trace.jsonl`, and `outcome.json`; never read config/env/API keys.

### K4: structured agent audit viewer

1. Render structured agent action audit fields in the simulation view for the selected frame:
   - actor id
   - action type
   - target if present
   - confidence
   - category
   - assessment
   - drivers
   - rationale
2. Keep output compact and terminal-friendly.
3. Add tests proving audit fields appear in formatted/rendered simulation text.

## Non-goals

- No network calls.
- No provider/model execution.
- No new persistence schema unless absolutely necessary.
- No destructive file operations.
- Do not let OpenCode commit; Hermes will verify and commit.

## Relevant files

- `src/tui/state.rs`
- `src/tui/simulation.rs`
- `src/tui/mod.rs`
- `src/main.rs`
- `src/nuwa/trace.rs`
- `src/nuwa.rs`
- `README.md`
- `plan.md`
- `.trellis/spec/backend/phase-e3-tui-snapshot-timeline-replay.md`
- `.trellis/spec/backend/contracts/tui-contract.md`

## Verification commands

Run at minimum:

```bash
cargo fmt --check
cargo test tui
cargo test trace
cargo test predict
cargo clippy -- -D warnings
git diff --check
cargo test
```

If CLI replay loading is implemented, add smoke checks like:

```bash
rm -rf /tmp/tianji-k3-bundle
cargo run --quiet -- predict --field global.conflict --horizon 2 --replay-bundle-dir /tmp/tianji-k3-bundle >/tmp/tianji-k3-outcome.json
cargo run --quiet -- tui --replay-bundle-dir /tmp/tianji-k3-bundle --render-once >/tmp/tianji-k3-tui.txt
python3 - <<'PY'
from pathlib import Path
text = Path('/tmp/tianji-k3-tui.txt').read_text()
assert 'frame' in text.lower()
assert 'audit' in text.lower() or 'assessment' in text.lower()
print('ok')
PY
```

Only add `--render-once` if it fits current TUI architecture; otherwise verify with unit tests and explain why an interactive smoke is not practical.
