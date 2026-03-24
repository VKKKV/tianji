# TianJi TUI Contract

## Purpose

This document defines the shipped contract for TianJi's terminal UI.

TianJi already ships a first read-only Rich-based slice today.
The goal here is to preserve alignment between:

- the current CLI-first operator workflow
- the current SQLite-backed persisted read surface
- a later Vim-style terminal UI that reuses those existing concepts without
  inventing a second domain model

## Scope

This contract is intentionally narrow.

It covers only a **read-only persisted-run browser** built on top of the current
`history`, `history-show`, and `history-compare` semantics.

The TUI should browse:

- persisted run history
- one persisted run in detail
- two persisted runs in comparison

## Shipped Now vs Planned Later

### Shipped now

The current `tui` command already provides a read-only terminal browser over persisted SQLite-backed runs.

Current shipped behavior, as implemented in `tianji/tui.py` and dispatched from `tianji/cli.py`:

- launch from `python3 -m tianji tui --sqlite-path ...`
- browse a persisted run list
- open a persisted detail panel
- stage a left run for comparison, then browse a compare panel against a selected right run
- move through runs with keyboard-first navigation
- keep compare staging visible in TUI state
- treat storage-backed read payloads as the semantic source of truth

Representative verification already exists in `tests/test_tui.py`.

### Planned later

This contract still reserves room for a fuller Vim-style operator experience, but that later work should extend the shipped read-only browser instead of replacing it with a different domain model.

## Non-Goals

This contract does **not** define:

- live run execution screens
- progress/status polling
- daemon, scheduler, or IPC behavior
- web/API transport requirements
- new scoring logic or storage schema changes
- exact keybinding maps beyond high-level Vim-style navigation intent

## Implementation

The shipped implementation uses `rich` (`Console`, `Live`, `Layout`) plus a small stdlib raw-key loop for keyboard-first navigation. This keeps the current slice lightweight and local-first while preserving room to revisit the framework choice later if the browser grows beyond Rich's comfortable scope.

## Design Principles

1. **CLI/storage remain the source of truth**
   - The TUI should reuse current read semantics rather than inventing a new
     backend contract.

2. **Persisted truth stays distinct from projected lenses**
    - Stored run-summary fields still describe the persisted run.
    - Filters, limits, and compare projections only shape the operator view.
    - This applies equally to the shipped CLI read surface and the shipped TUI read surface.

3. **Read-only first**
   - The first TUI contract should browse persisted data only.
   - Running pipelines remains a CLI and daemon concern. The TUI does not become a write surface.

4. **Vim-style navigation without new business logic**
   - The TUI may add keyboard-first movement and selection.
   - It should not duplicate scoring, grouping, comparison, or filtering logic.

## Source-of-Truth Surfaces

The shipped and future TUI should mirror these existing owned surfaces:

- the Click-based `python3 -m tianji ...` CLI remains the semantic source of truth for command behavior

- `python3 -m tianji history --sqlite-path ...`
  - persisted run list / triage view

- `python3 -m tianji history-show --sqlite-path ...`
  - persisted single-run detail view
  - supports latest / previous / next navigation
  - supports scored-event and event-group projection lenses

- `python3 -m tianji history-compare --sqlite-path ...`
  - persisted run compare view
  - supports explicit pair and relative/latest presets
  - supports the same projection lenses as `history-show`

- `tianji/storage.py`
  - canonical read-model implementation for list/detail/compare payloads
  - persisted truth lives here, while TUI panels render projected read lenses on top

## Core Entities and IDs

The TUI should reuse current identifiers and payload vocabulary:

- `run_id` — primary persisted run identity
- `schema_version` — artifact contract version
- `event_id` — scored event identity
- `headline_event_id` — top event-group identity
- `top_scored_event` — compare/list triage anchor
- `top_event_group` — grouped-analysis triage anchor

## Operator Workflows

### 1. Run History List

Purpose:
- browse persisted runs
- triage by mode, field, risk, score, and grouped-analysis signals
- choose one run for detail or two runs for comparison

Stable row fields:

- `run_id`
- `schema_version`
- `mode`
- `generated_at`
- `raw_item_count`
- `normalized_event_count`
- `dominant_field`
- `risk_level`
- `headline`
- `event_group_count`
- `top_event_group_headline_event_id`
- `top_event_group_dominant_field`
- `top_event_group_member_count`
- `top_scored_event_id`
- `top_scored_event_dominant_field`
- `top_impact_score`
- `top_field_attraction`
- `top_divergence_score`

### 2. Run Detail

Purpose:
- inspect one stored run with optional scored-event and event-group lenses
- optionally align interventions to the visible scored-event selection

Stable root payload:

- `run_id`
- `schema_version`
- `mode`
- `generated_at`
- `input_summary`
- `scenario_summary`
- `scored_events`
- `intervention_candidates`

Stable detail behaviors:

- latest / previous / next navigation is part of the contract
- `scenario_summary` remains persisted truth even when projected lists narrow
- scored-event filtering and event-group filtering are independent
- `only_matching_interventions` aligns interventions to the final visible
  scored-event set after filters and limits

### 3. Run Compare

Purpose:
- compare two persisted runs under one operator lens
- inspect both side summaries plus explicit diff fields
- surface a compact top-group evidence diff subsection inside the compare panel

Stable compare payload:

- `left_run_id`
- `right_run_id`
- `left`
- `right`
- `diff`

Stable compare selection modes:

- explicit `left_run_id` / `right_run_id`
- latest pair
- run against latest
- run against previous

These presets remain mutually exclusive.

## Lens and Projection Semantics

The shipped TUI already exposes shared in-TUI projection controls for the
read-only detail and compare panes while keeping the history list on persisted
truth.

The remaining Phase 5 work should treat those shared projection rules as shipped
behavior, then focus on persisted-navigation parity, input/render separation,
and stronger verification rather than re-inventing a separate TUI-only
projection model.

### Scored-event lens

Shipped shared lens controls:

- `dominant_field`
- `limit_scored_events`

Explicitly deferred from this slice:

- numeric threshold entry for `min_impact_score` / `max_impact_score`
- numeric threshold entry for `min_field_attraction` / `max_field_attraction`
- numeric threshold entry for `min_divergence_score` / `max_divergence_score`

### Event-group lens

Shipped shared lens controls:

- `group_dominant_field`
- `limit_event_groups`

### Intervention alignment lens

Shipped shared lens control:

- `only_matching_interventions=false`
  - keep full persisted intervention list

- `only_matching_interventions=true`
  - keep only interventions whose `event_id` remains in the final visible
    scored-event set

This Phase 5 work should keep those shipped controls shared across detail and
compare panes, preserve the list as persisted truth, and avoid adding list-pane
filtering, text entry, or any new storage semantics.

### Persisted truth vs projected view

- `history-show`
  - run-level summary fields remain persisted truth
  - projected `scored_events`, `event_groups`, and aligned interventions are
    read-time lenses

- `history-compare`
  - compare-side projected fields such as `top_scored_event`,
    `intervention_event_ids`, `event_group_count`, and `top_event_group` depend
    on the active lens
  - stored run-summary fields like `headline`, `dominant_field`, and
    `risk_level` still describe the persisted run itself

Those projected lenses stay confined to detail and compare rendering. The
persisted history list should continue to show storage-backed truth rather than
a separately filtered list pane.

## Navigation and State Model

The first shipped slice already has concrete navigation behavior, but this
contract only locks the high-level navigation nouns, not every final keybinding
detail.

Minimum state:

- selected `run_id` in history list
- selected compare pair, including a visible staged left run before compare activates
- active scored-event lens
- active event-group lens
- intervention alignment toggle
- active panel/view (`list`, `detail`, `compare`)

The current implementation already carries these ideas in state, including a visible staged left compare run before compare activation.

The remaining Phase 5 work is to make previous/next run movement and compare
target stepping fully honor persisted-run semantics beyond the currently loaded
list window, keep input handling separate from render formatting, and harden
verification around the shipped shared lenses instead of adding those lenses.

Minimum Vim-style navigation intent:

- move selection within a list/table
- open selected run detail
- switch back to history list
- move to previous / next persisted run from detail view
- stage a left/right compare pair from list selection with visible staged-pair feedback
- switch focus between compare left/right/diff panes
- apply or clear filter lenses without mutating stored data

Shipped navigation parity requirement:

- previous / next stepping from a persisted run from detail view should resolve
  against SQLite-backed previous/next run semantics, even when that adjacent run
  falls outside the currently loaded list window
- compare-target stepping should follow the same persisted previous/next run
  semantics, skipping the staged left run when needed rather than falling back
  to local row-only stepping
- first/last persisted boundaries should surface explicit navigation errors
  instead of silently stopping at the current list window edge

## Empty and Error States

The TUI should treat current CLI/storage semantics as authoritative.

Valid empty states:

- no persisted runs available
- no scored events remain after projection
- no event groups remain after projection
- no intervention candidates remain after alignment

Validation/error states to preserve:

- negative limits are invalid
- inverted score windows are invalid
- compare preset mixes are invalid
- non-positive explicit run IDs are invalid
- missing previous/next/latest runs should surface as navigation errors, not as
  silent fallback behavior

## Relationship to Local API Planning

This document is intentionally separate from `LOCAL_API_CONTRACT.md`.

- `TUI_CONTRACT.md` defines a shipped read-only terminal surface plus later terminal planning over current in-process storage/artifact semantics.
- `LOCAL_API_CONTRACT.md` defines a future process boundary for an optional local
  service/API layer.

The shipped TUI does not require a local API, and later TUI work should not require one either.

## Recommended Later Implementation Order

When TianJi extends the terminal UI further, keep the next slices in this order:

1. history list view
2. single-run detail view
3. run compare view
4. only then consider run-triggering or live-runtime concerns

This keeps Phase 5 aligned with current product reality: local-first, deterministic, persisted-analysis-first, and read-only by default.
