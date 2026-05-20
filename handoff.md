# TianJi handoff

Date: 2026-05-19
Repo: /home/kita/code/tianji
Branch: main
Agent workflow: Hermes plans/verifies/commits; OpenCode implements non-trivial code changes with model `kita/gpt-5.5`.

## Current status

Clean checkpoint after Phase F2.

- Git worktree: clean before writing this handoff.
- Trellis active tasks: 0.
- Last completed target: F2 API contract fixtures.
- Next planned target: F3 README operator quickstart refresh.
- Verification baseline:
  - cargo test --quiet: 341 unit + 39 integration passed
  - cargo clippy -- -D warnings passed
  - git diff --check passed
- `plan.md` is authoritative and currently marks:
  - Phase D complete
  - Phase E complete
  - F1 complete
  - F2 complete
  - F3/F4 pending

## Recent commits

```text
dd3104b Mark F2 complete in roadmap
d3fedca chore(task): archive 05-19-f2-api-contract-fixtures
407960d Add API contract fixture coverage
ac20d91 Add F2 API contract fixtures task
79761bc chore(task): archive 05-19-f1-config-doctor-command
52e0e55 Add config doctor command
4241553 Add F1 config doctor task
467c5fb chore(task): archive 05-19-f0-roadmap-docs-refresh
28530ea Refresh roadmap and docs for Phase F
7079b8b Mark Phase E complete in roadmap
40f2103 chore(task): archive 05-19-e3-tui-snapshot-timeline-replay
6577b3c Add TUI simulation timeline replay
```

## Completed in this session

### Phase E — complete

E1. HMAC-Signed Agent Command Channel
- Added daemon API route `POST /api/v1/agent/command`.
- HMAC-SHA256 over timestamp + nonce + body digest.
- Replay protection with timestamp/nonce checks.
- Test-only deterministic secrets; no real credentials.

E2. Structured Agent Output / Auditability
- Enriched `AgentAction` with structured audit fields:
  - assessment
  - category
  - confidence
  - drivers[]
- Kept backward-compatible serde defaults.

E3. TUI Snapshot Timeline Replay
- Added replay cursor/frame metadata.
- Added left/right and h/l style scrubbing in simulation view.
- Preserved existing view keybindings.

### Phase F — partial

F0. Roadmap/docs refresh
- Refreshed `plan.md` and README state after Phase E.

F1. Config sample and doctor command
- Added `tianji doctor`:
  - `--config <PATH>`
  - `--sqlite-path <PATH>`
  - `--json`
- Checks config presence/parse, provider references, env-var presence, inline key presence without printing secret values, SQLite parent readiness.
- Added `examples/config.example.yaml`.
- README mentions `doctor` and config template.

F2. API contract fixtures
- Strengthened `/api/v1/meta` contract against `tests/fixtures/contracts/local_api_meta_v1.json`.
- Added `/api/v1/agent/command` accepted/rejected envelope contract test.
- Added alert dispatch dry-run/redaction contract tests.
- Added mocked webhook payload contract test.
- Added TUI replay frame/cursor formatting contract test.

## Pending targets

### F3. README operator quickstart refresh

Goal from `plan.md`:
- Document current LLM config.
- Document daemon API.
- Document signed command channel.
- Document alert dispatch dry-run.
- Document TUI replay keybindings.
- Keep examples local-first and credential-free.

Suggested workflow:
1. Create Trellis task:
   ```bash
   python3 ./.trellis/scripts/task.py create "F3 README operator quickstart refresh" --slug f3-readme-operator-quickstart-refresh
   ```
2. Add PRD/spec under `.trellis/tasks/.../prd.md` and `.trellis/spec/docs/phase-f3-readme-operator-quickstart-refresh.md`.
3. Update README only unless a small helper fixture is clearly needed.
4. Verify with:
   ```bash
   cargo test --quiet
   cargo clippy -- -D warnings
   git diff --check
   ```
5. Commit README changes, archive task, update `plan.md` to mark F3 complete.

Important docs to inspect before F3:
- `README.md`
- `plan.md`
- `examples/config.example.yaml`
- `src/api.rs`
- `src/alert_dispatch.rs`
- `src/tui/simulation.rs`

### F4. Release readiness check

Goal from `plan.md`:
- Verify `cargo build --release`.
- Verify binary size target: single binary < 25MB.
- Verify shell completions generation.
- Run a fixture-based smoke run.
- Produce concise release checklist in repo.

Likely output file:
- `RELEASE_CHECKLIST.md` or `docs/release-checklist.md`.

Suggested verification commands:
```bash
cargo build --release
stat -c '%s %n' target/release/tianji
cargo run --quiet -- completions fish >/tmp/tianji.fish
cargo run --quiet -- run --fixture tests/fixtures/sample_feed.xml >/tmp/tianji-run.json
cargo test --quiet
cargo clippy -- -D warnings
git diff --check
```

Note: if live LLM/model endpoint testing is needed, start local model service first:
```bash
systemctl --user start llama-server
```
F3/F4 should not need live LLM unless scope changes.

## OpenCode workflow to continue

Use interactive PTY, not a broad one-shot:
```bash
opencode --model kita/gpt-5.5 /home/kita/code/tianji
```

Hermes process pattern:
1. Start with `terminal(background=true, pty=true, notify_on_complete=true)`.
2. Submit prompt with `process.submit`.
3. Send carriage return with `process.write(data="\r")`.
4. Do not set broad watch patterns.
5. If OpenCode spins after code changed, inspect `git status --short` and logs; Ctrl-C is safe after it reports tests/diff state.
6. Hermes must independently verify before committing.
7. Hermes commits implementation and archives Trellis task.

Do not let OpenCode commit unless explicitly requested.

## Verification commands used most recently

```bash
cargo fmt
cargo test --quiet contract
cargo test --quiet api
cargo test --quiet alert_dispatch
cargo test --quiet tui
cargo test --quiet
cargo clippy -- -D warnings
git diff --check
```

Latest full pass:
```text
cargo test --quiet
- 341 unit passed
- 39 integration passed

cargo clippy -- -D warnings
- passed

git diff --check
- passed
```

## Trellis notes

- Active tasks are currently zero.
- Archived current tasks:
  - `.trellis/tasks/archive/2026-05/05-19-f1-config-doctor-command`
  - `.trellis/tasks/archive/2026-05/05-19-f2-api-contract-fixtures`
- For new work, create a new Trellis task first, then PRD/spec, then implementation.

## Safety constraints

- No real secrets in fixtures, README, tests, or logs.
- Use `[REDACTED]`, `<redacted>`, or dummy test-only values.
- Alert dispatch tests must stay mocked/dry-run.
- F3 examples should be local-first and credential-free.
- F4 release checks should not require external services.

## Known quirks

- Terminal tool may sometimes summarize output as “1 lines output”; rerun narrower commands or read files directly.
- OpenCode TUI output is ANSI-heavy and may truncate; trust `git status`, tests, clippy, and direct file reads.
- For TianJi live LLM testing only, start local model endpoint first:
  ```bash
  systemctl --user start llama-server
  ```
- F3/F4 should not require live LLM by default.

## Immediate next action

Start F3:
```bash
python3 ./.trellis/scripts/task.py create "F3 README operator quickstart refresh" --slug f3-readme-operator-quickstart-refresh
```

Then write PRD/spec and update README with operator quickstart covering:
- deterministic fixture run
- config template + `tianji doctor`
- daemon/API overview
- signed agent command channel with dummy HMAC example only
- alert dispatch dry-run
- TUI replay keybindings
