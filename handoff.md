# TianJi handoff

Date: 2026-06-09
Repo: `/home/kita/code/tianji`
Branch: `main`
Agent workflow: Hermes plans/verifies/commits; OpenCode implements non-trivial Rust code changes with model `jun/gpt-5.5`. Do not let OpenCode commit unless explicitly requested.

## Current status

Active task: none. Last completed task: `.trellis/tasks/archive/2026-06/06-09-release-readiness-docs-refresh/`.

Repository state before this task was clean on `main`. K3/K4 TUI replay/audit work has already been committed and archived; do not follow older dirty-worktree notes for that stack.

## Current task scope

Documentation-only release/readiness refresh after Phase K.

Goals:

- Keep this handoff aligned with the current clean repository state.
- Ensure release/readiness documentation remains local-first, credential-free, and reproducible.
- Avoid runtime behavior changes unless a future PRD explicitly scopes them.

## Current shipped Phase K behavior

- `tianji predict --trace-jsonl <PATH>` writes `tianji.sim-trace.v1` JSONL traces.
- `tianji predict --replay-bundle-dir <DIR>` writes a local replay bundle containing `manifest.json`, `trace.jsonl`, and `outcome.json`.
- `tianji tui --trace-jsonl <PATH> [--render-once]` loads trace-backed simulation replay without provider execution.
- `tianji tui --replay-bundle-dir <DIR> [--render-once]` reads only the three replay bundle files above.
- Replay bundle validation checks schema version, fixed file names, trace/outcome sizes, frame counts, and manifest mode/target/horizon against trace metadata.
- Simulation replay scrubbing with `Left`/`h` and `Right`/`l` updates the selected frame display, including field metadata, field changes, event sequence length, and compact structured agent audit fields.
- Trace strings are sanitized before rendering.
- Replay flags conflict with each other and with `--simulate`.
- Plain `tianji tui` defaults to `runs/tianji.sqlite3`.

## Verified counters

Measured on 2026-06-09:

```text
Rust lines/files: 30,399 / 59
cargo test -- --list: 445 tests
```

## Recommended verification before commit

```bash
cargo fmt --check
cargo test --quiet
cargo clippy -- -D warnings
git diff --check
```

Optional local replay smoke:

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
