# TianJi Session Handoff

## Current State

TianJi is still a **CLI-first, local-first Python tool**. The owned surface is in
`tianji/` and `tests/`, with deterministic pipeline stages and optional SQLite
persistence.

Primary verification command:

```bash
.venv/bin/python -m unittest discover -s tests -v
```

Latest verified state in this session:

- full unittest suite passes
- current count: `117` tests
- local `main` branch now exists and already includes the merged scoring-refinement work
- active continuation branch is `feat/scoring-fa-threshold-boundaries`
- history/history-show/history-compare operator workflows are substantially richer than at the start of the branch
- Candidate A scoring slice is now shipped: `Im` includes a bounded text-signal-intensity bonus while `Fa` remains field-alignment-only
- Candidate B has now advanced through two narrow shipped refinements: `Fa` includes a bounded near-tie ambiguity penalty when the top two fields nearly tie and a bounded diffuse mixed-field penalty when a strong third field remains after the top-two margin is already clear
- scoring determinism has now been tightened in three more narrow shipped slices:
  - zero positive field-mass events stay `uncategorized` instead of inheriting a dominant field from dict order, and `Fa` floors at `0.0` for that case
  - exact positive-score dominant-field ties are now resolved canonically by field-name order so `Im` text-signal matching and rationale output do not depend on `field_scores` insertion order
  - scenario-summary dominant-field count ties are now deterministic and prefer the strongest tied event by `divergence_score`, with field-name order only as the final fallback
- `Im` field-diversity credit is now thresholded: only fields with `field_score >= 1.0` count toward the diversity bonus, while the dominant positive field still preserves the baseline one-field credit
- history-compare parser coverage now includes negative compare limits and mixed-preset misuse, and the CLI rejects explicit-pair ids mixed with `--against-latest` / `--against-previous`
- inverted score windows are now rejected consistently across `history`, `history-show`, and `history-compare`
- negative `history --limit` values are now parser-rejected instead of silently changing slice behavior
- non-positive explicit persisted run ids are now parser-rejected for `history-show` and explicit/preset `history-compare` paths
- scoring coverage now includes isolated `Im` tests for actor weighting, region weighting, raw keyword-density cap behavior, dominant-field-strength bonus behavior, thresholded field-diversity behavior, direct text-signal surface contributions, and isolated `Fa` tests for dominance-margin and coherence behavior
- `Fa` threshold-boundary coverage now explicitly pins both ambiguity gates: the near-tie penalty starts below the `1.0` top-two margin threshold and the diffuse-third-field penalty starts above the `2.5` third-field threshold
- scoring coverage now also explicitly pins zero-field uncategorized behavior, exact dominant-field tie determinism, and scenario-summary tie determinism

## What Ships Now

### Core pipeline

- fixture-first and one-time fetch execution
- deterministic normalization into keywords / actors / regions / field scores
- explicit `impact_score` / `field_attraction` / `divergence_score`
- `impact_score` now includes a bounded dominant-field text-signal-intensity bonus derived from normalized keywords/title/summary
- deterministic backtracking with grouped-event dedupe
- schema-versioned JSON artifacts
- optional SQLite persistence for run inspection

### Scoring semantics

- `Im` remains deterministic and additive
- current `Im` inputs now include:
  - actor weight
  - region weight
  - keyword density
  - dominant-field evidence bonus
  - thresholded field-diversity bonus (`field_score >= 1.0`, with the dominant positive field still preserving baseline one-field credit)
  - bounded text-signal-intensity bonus
- `Fa` still comes only from dominant-field concentration semantics:
  - dominant field strength
  - dominance margin over the second-best field
  - coherence share of dominant-field mass
  - bounded near-tie ambiguity penalty on the top-two margin when the rounded top-two gap is below `1.0`
  - bounded diffuse mixed-field penalty when the rounded top-two gap is at least `1.0` but third-field support remains unusually strong
- current text-signal-intensity behavior is **boundary-aware cross-surface dominant-field cue concentration**, not historical novelty and not grouped corroboration
- title/summary cue matching now uses cached compiled regexes, so the boundary-aware rule stays cheap in the scoring loop
- `score_event()` now computes dominant field and text-signal intensity once and reuses them for both `impact_score` and rationale construction
- score rationale now includes `im_text_signal_intensity=...` so the new `Im` factor stays inspectable
- dominant-field selection is now deterministic across both scored events and scenario summaries; no field winner should depend on Python dict insertion order

### Grouping and causal logic

- grouped event summaries in `scenario_summary.event_groups`
- evidence-chain metadata on grouped events
- transitive causal clustering
- admission-path-based `causal_ordered_event_ids`
- `causal_span_hours` from earliest/latest known timestamps when at least two exist
- softened causal wording when span is unknown
- grouped backtrack reasons include evidence-chain and causal-cluster context

### Persisted operator workflow

- `history` supports:
  - mode / dominant-field / risk-level filters
  - since / until filters
  - top scored-event score filters
  - top event-group dominant-field and event-group count filters
  - grouped-run triage fields in list output
  - parser rejection for negative list limits
  - parser rejection for inverted top-score windows
- `history-show` supports:
  - latest / previous / next navigation
  - scored-event dominant-field and score-threshold filters
  - scored-event limits
  - optional intervention alignment to visible scored events
  - event-group dominant-field and limit filters
  - parser rejection for non-positive explicit `--run-id`
  - parser rejection for inverted score windows
- `history-compare` supports:
  - explicit pair / latest pair / against latest / against previous presets
  - grouped evidence/member/link deltas
  - top scored-event score deltas
  - the same scored-event and event-group projection filters as `history-show`
  - compare-side summaries remain summaries, not full `history-show` payloads
  - parser rejection for non-positive explicit run ids, inverted compare score windows, and mixed preset misuse

## Important Contracts

### Mixed compare semantics

In `history-compare`, projection filters affect compare-side projected fields such
as:

- `top_scored_event`
- `intervention_event_ids`
- `event_group_count`
- `top_event_group`

But full-run stored summary fields like `dominant_field`, `risk_level`, and
`headline` still describe the stored run, not the filtered lens.

### No-group history semantics

For runs with no grouped scenarios:

- `event_group_count = 0`
- `top_event_group_headline_event_id = null`
- `top_event_group_dominant_field = null`
- `top_event_group_member_count = null`

### Causal-group semantics

- `evidence_chain` reflects actual admission edges used to attach members to a group
- `causal_ordered_event_ids` follows that admission-path tree, not guaranteed strict timestamp order
- `causal_span_hours` is `null` unless at least two timestamps are known

## Most Relevant Files

- `README.md` — current reality, commands, shipped surface
- `DEV_PLAN.md` — roadmap and recent progress
- `SCORING_SPEC.md` — deterministic scoring + current grouping semantics
- `tianji/scoring.py` — explicit `Im` / `Fa` math including text-signal-intensity bonus, thresholded field-diversity credit, and deterministic field selection rules
- `tianji/cli.py` — Click-based operator surface, validation rules, and `main(argv)` entry compatibility
- `tianji/storage.py` — persistence + all history/history-show/history-compare read logic
- `tianji/pipeline.py` — orchestration, grouping, causal clustering
- `tianji/backtrack.py` — intervention generation and grouped reasoning text
- `tests/test_pipeline.py` — authoritative fixture-first and end-to-end verification suite

## Branch / Resume Context

- `main` now exists locally and includes the merged scoring-refinement stack at:
  - `c01aaa8` — `merge scoring refinement work into main`
- current continuation branch for new work:
  - `feat/scoring-fa-threshold-boundaries`
- latest commits on the active branch:
  - `226671c` — `document field attraction threshold coverage`
  - `46e0876` — `cover field attraction threshold boundaries`
- latest branch before that:
  - `feat/scoring-fa-mixed-field-case`
  - `31dfac5` — `document diffuse field attraction refinement`
  - `342b0bb` — `refine field attraction for diffuse mixed fields`
- working tree was clean at handoff time

## Roadmap Position

The just-completed slices pushed far into:

- Phase 2: richer grouping / causal clustering / backtracking semantics plus the first scoring-model expansion beyond the original thin `Im` / `Fa` slice
- Phase 4: CLI-first persisted-analysis ergonomics

At this point, the biggest obvious CLI gap was already closed by making
`history-compare` projection-aware, and the next several narrow scoring cleanup
slices have also landed. The next session should default to a **small doc/spec
cleanup or a fresh scoring issue only if a new concrete weakness is found**, not
to reopening already-shipped tie-handling or residual-noise work.

The CLI implementation now uses Click while preserving the existing operator command surface and `main(argv)` entry semantics used by the unittest suite.

Phase 5 is now underway beyond the contract layer:

- `TUI_CONTRACT.md` defines the read-only persisted-analysis TUI
- a first Rich-based implementation now exists for history list and detail views
- it reuses current `history`, `history-show`, and `history-compare` semantics
- it does **not** imply any daemon/API dependency
- current Rich detail browsing already includes compact scored-event and intervention previews

Given the current branch stack, the default resume path should be:

1. start from `feat/scoring-fa-threshold-boundaries` if continuing scoring work
2. start from `main` only if beginning a new unrelated feature branch

## Recommended Next Work

Best next milestone:

1. **Refine the first-party scoring model from the now-shipped Candidate A base**
   - only if a fresh concrete scoring weakness appears after the now-shipped determinism cleanup
   - keep the next step narrow and deterministic
   - strongest likely next target is either:
     - a small follow-up to the shipped `Fa` ambiguity refinements only if a new mixed-field weakness remains after the pinned threshold tests, or
     - one more tightly bounded text-signal edge-case pass only if a real cue-boundary bug appears,
   - likely files: `tianji/scoring.py`, `SCORING_SPEC.md`, `tests/test_pipeline.py`
   - preserve current branch guardrails:
     - no persistence or schema changes
     - no CLI/history surface expansion unless a score-driven test truly requires it
     - no novelty/baseline scoring yet

Best smaller default next step:

2. **Refresh handoff/spec/docs around shipped scoring semantics**
   - capture the now-shipped determinism stack in `SESSION_HANDOFF.md`, `DEV_PLAN.md`, and `SCORING_SPEC.md`
   - keep the next session from reopening already-shipped tie-handling or raw nonzero-field assumptions

Best larger next branch after scoring:

3. **Continue Rich-based TUI implementation**
   - `TUI_CONTRACT.md` now exists and the first Rich-based list/detail implementation has started from it
   - next work here should keep refining the read-only list/detail browser and only then expand toward compare mode
   - continue reusing CLI/storage semantics rather than inventing a daemon/API boundary

## Suggested Commands For The Next Session

```bash
.venv/bin/python -m unittest discover -s tests -v
.venv/bin/python -m tianji history --sqlite-path runs/tianji.sqlite3
.venv/bin/python -m tianji history-show --sqlite-path runs/tianji.sqlite3 --run-id 1
.venv/bin/python -m tianji history-compare --sqlite-path runs/tianji.sqlite3 --left-run-id 1 --right-run-id 2
```

Useful projection examples:

```bash
.venv/bin/python -m tianji history --sqlite-path runs/tianji.sqlite3 --top-group-dominant-field technology --min-event-group-count 1
.venv/bin/python -m tianji history-show --sqlite-path runs/tianji.sqlite3 --run-id 1 --dominant-field diplomacy --limit-scored-events 1 --only-matching-interventions
.venv/bin/python -m tianji history-compare --sqlite-path runs/tianji.sqlite3 --left-run-id 1 --right-run-id 2 --dominant-field diplomacy --group-dominant-field diplomacy --limit-scored-events 1 --limit-event-groups 1
```

Useful scoring verification command:

```bash
.venv/bin/python -m unittest discover -s tests -v
```

Current scoring-focused tests worth reading first:

- `test_score_event_exposes_explicit_im_fa_semantics`
- `test_score_event_applies_actor_weight_inside_im`
- `test_score_event_applies_dominant_field_strength_inside_im`
- `test_score_event_ignores_subthreshold_field_noise_inside_im`
- `test_score_event_rewards_meaningful_field_diversity_inside_im`
- `test_score_event_applies_region_weight_inside_im`
- `test_score_event_applies_text_signal_surface_contributions_inside_im`
- `test_score_event_caps_raw_keyword_density_inside_im`
- `test_score_event_combines_text_signal_surface_contributions_inside_im`
- `test_score_event_applies_dominance_margin_inside_fa`
- `test_score_event_applies_coherence_inside_fa`
- `test_score_event_penalizes_diffuse_mixed_field_support_in_fa`
- `test_score_event_near_tie_penalty_starts_below_margin_threshold`
- `test_score_event_diffuse_third_field_penalty_starts_above_threshold`
- `test_score_event_rewards_clearer_field_alignment_in_fa`
- `test_score_event_penalizes_near_tie_field_alignment_in_fa`
- `test_score_event_keeps_clear_field_alignment_semantics_stable`
- `test_score_event_rewards_stronger_text_signal_intensity_in_im`
- `test_score_event_text_signal_intensity_does_not_reward_generic_keyword_mass`
- `test_score_event_text_signal_intensity_ignores_incidental_substrings`
- `test_score_event_text_signal_intensity_matches_punctuation_adjacent_cues`
- `test_score_event_text_signal_intensity_respects_cap`
- `test_score_event_treats_zero_field_mass_as_uncategorized`
- `test_score_event_exact_top_field_tie_is_order_independent`
- `test_summarize_scenario_resolves_dominant_field_ties_independently_of_event_order`

## Guardrails For The Next Session

- keep first-party changes in `tianji/` and `tests/`
- keep `cli.py` thin; put persisted read behavior in `storage.py`
- do not add schema migrations unless the task truly requires them
- prefer additive nested metadata over top-level contract churn
- do not jump to daemon/web work yet
- preserve deterministic behavior by default
- for the next scoring slice, keep `Fa` isolated unless a concrete field-alignment bug is demonstrated
- do not reopen dominant-field tie handling or raw nonzero-field `Im` diversity assumptions unless a new regression proves current deterministic rules wrong
- avoid widening text-signal scoring into history-aware novelty, grouped corroboration, or opaque heuristics
