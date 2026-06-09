# TianJi handoff

Date: 2026-06-09
Repo: `/home/kita/code/tianji`
Branch: `main`
Agent workflow: Hermes plans/verifies/commits; OpenCode implements non-trivial code changes with model `kita/gpt-5.5`. Do not let OpenCode commit unless explicitly requested.

## Current status

Active task: `.trellis/tasks/06-09-k3-k4-tui-replay-audit/`

K3/K4 TUI replay/audit implementation is in dirty worktree and not committed. The implementation now includes trace-backed TUI replay loading, selected-frame rendering, structured audit display, CLI parse constraints, replay bundle integrity checks, render sanitization, and updated plan counters.

## Current dirty areas

- `src/tui/state.rs` — trace-to-TUI state mapping, replay cursor state, trace text sanitization.
- `src/tui/simulation.rs` — selected-frame worldline/agents/events/audit rendering and tests.
- `src/tui/mod.rs` — TUI replay trace/bundle loading, bundle integrity validation, render-once support, tests.
- `src/main.rs` — TUI CLI flags, conflict constraints, parse tests.
- `src/nuwa/trace.rs` / `src/nuwa.rs` — K1/K2 trace and bundle support from the current dirty stack.
- `plan.md` — current counters updated to 30,412 Rust lines / 59 files and 443 tests.
- `README.md` — existing K3/K4 documentation changes from the dirty stack.

## Implemented K3/K4 behavior

- `tianji tui --trace-jsonl <PATH> [--render-once]` loads a simulation trace into the simulation view without requiring provider execution.
- `tianji tui --replay-bundle-dir <DIR> [--render-once]` reads only `manifest.json`, `trace.jsonl`, and `outcome.json`.
- Replay bundle loading validates:
  - manifest schema version,
  - fixed file names,
  - trace/outcome byte sizes,
  - manifest frame count,
  - trace metadata frame count,
  - manifest mode/target/horizon against trace metadata.
- `Left`/`h` and `Right`/`l` change selected frame display, not just frame counters.
- Worldline, field changes, Agents, Events, and Agent audit sections now reflect the selected trace frame.
- Trace strings are sanitized before rendering: control characters/ESC are removed, whitespace/newlines/tabs collapse to spaces, and long text is truncated.
- CLI constraints now reject replay flag conflicts and reject `--simulate` with replay flags.
- Plain `tianji tui` defaults to `runs/tianji.sqlite3` again.

## Verification run by OpenCode

```text
cargo fmt: passed
cargo test tui --quiet: passed (79 passed, 364 filtered)
cargo test trace --quiet: passed (11 passed, 432 filtered)
cargo test predict --quiet: passed (7 passed, 436 filtered)
```

Counters recomputed after changes:

```text
Rust lines/files: 30,412 / 59
cargo test -- --list: 443 tests
```

## Recommended next verification

Run full gate before commit:

```bash
cargo fmt --check
cargo test --quiet tui
cargo test --quiet trace
cargo test --quiet predict
cargo test --quiet
cargo clippy -- -D warnings
git diff --check
```

Optional smoke:

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
