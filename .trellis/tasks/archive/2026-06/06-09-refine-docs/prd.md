# Refine Project Docs

## Goal

Refresh TianJi's project documentation so the main reader-facing docs accurately describe the current state of the project, especially the completed replay/audit work and current local-first operator workflow.

## What I Already Know

- User requested: "refine the doc of this project tianji".
- Root `README.md` is the primary operator-facing document.
- Root `plan.md` is the architecture authority per `.trellis/spec/backend/index.md`.
- Root `RELEASE_CHECKLIST.md` records Phase F4 release readiness and is older than the K3/K4 work.
- `README.md` currently states `Current State (2026-05-20)` and reports `378 unit tests + 57 integration tests`, while `plan.md` reports K3/K4 complete and `386 unit + 57 integration = 443 total`.
- `README.md` already includes replay trace and replay bundle commands, but it likely needs consistency, polish, and current-state alignment.
- Docs spec guidance says docs should be in English, local-first first, credential-free, and examples should be runnable from repo root.

## Assumptions (Temporary)

- This task should update documentation only, not implementation code.
- The highest-value target is the public-facing docs at the repo root, especially `README.md`.
- Handoff notes are session-specific and should not be treated as public docs unless explicitly requested.

## Decisions

- Scope: update root docs (`README.md`, `plan.md`, and `RELEASE_CHECKLIST.md`) so they consistently reflect the latest K3/K4 status and current counters.

## Open Questions

- None currently.

## Requirements (Evolving)

- Preserve TianJi's local-first positioning: the first runnable examples must require no credentials, no LLM, no daemon, and no network.
- Keep examples credential-free and avoid printing or embedding secrets.
- Describe shipped behavior only; do not invent or document future behavior as complete.
- Keep root documentation internally consistent with the current K3/K4 state.
- Update `README.md`, `plan.md`, and `RELEASE_CHECKLIST.md`; avoid session-only files such as `handoff.md` unless needed as context.
- Treat `plan.md` as the architecture/status authority and align `README.md` to it.
- Refresh `RELEASE_CHECKLIST.md` as the current local release/readiness record, keeping commands reproducible and transient outputs under `/tmp`.

## Acceptance Criteria (Evolving)

- [ ] `README.md` no longer contains stale current-state/test-count statements.
- [ ] `plan.md` and `RELEASE_CHECKLIST.md` align with the refreshed root-doc status and counters.
- [ ] Updated docs clearly explain the deterministic fixture path before optional provider/daemon/replay flows.
- [ ] Replay trace and replay bundle documentation is concise, accurate, and discoverable.
- [ ] Commands remain copy-pasteable from the repo root.
- [ ] Root docs do not include real secrets, private endpoints, or user-specific paths.
- [ ] Documentation changes pass `git diff --check`.

## Technical Approach

- Do a focused root-doc refresh rather than a broad rewrite.
- Update `README.md` current-state text, operator flow, and replay/audit wording for clarity and consistency.
- Update `plan.md` only where root-level metadata/status wording is stale or inconsistent.
- Update `RELEASE_CHECKLIST.md` so it is not misleadingly frozen at the older Phase F4 test counts while keeping the checklist concise and reproducible.
- Use targeted searches for stale counters/dates/status strings after editing.

## Definition of Done

- Docs updated according to confirmed scope.
- No code behavior changes unless the user expands scope.
- Relevant lightweight verification completed.
- Trellis check and spec-update judgment completed before commit.

## Out of Scope (Explicit)

- Changing CLI flags, API endpoints, schemas, or runtime behavior.
- Adding new dependencies.
- Publishing releases, creating tags, or pushing to remote.

## Technical Notes

- Inspected `README.md`, `plan.md`, `handoff.md`, `RELEASE_CHECKLIST.md`, `.trellis/spec/backend/index.md`, and `.trellis/spec/docs/phase-f3-readme-operator-quickstart-refresh.md`.
- Relevant docs guidance: `.trellis/spec/backend/index.md` and `.trellis/spec/docs/phase-f3-readme-operator-quickstart-refresh.md`.
