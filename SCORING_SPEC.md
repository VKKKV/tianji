# TianJi Im / Fa Scoring Slice

## Purpose

This note defines the first first-party TianJi interpretation of `Im` and `Fa` for the deterministic one-shot pipeline.

It is intentionally narrower than the long-term Phase 2 goal. The purpose of this slice is to make TianJi's scoring language explicit without changing the pipeline shape, artifact schema, or ranking behavior in a broad way.

## Definitions

- **`Im` (Impact)**
  - TianJi's deterministic estimate of how strongly an event can move the current branch.
  - In the current slice, it is a bounded additive score built from:
     - actor weight
     - region weight
     - keyword density
     - a small dominant-field evidence bonus
     - a small nonzero-field diversity bonus
     - a bounded text-signal-intensity bonus for dominant-field cue concentration across normalized text surfaces

- **`Fa` (Field Attraction)**
  - TianJi's deterministic estimate of how strongly an event belongs to its dominant attractor field.
  - In the current slice, it starts from the strongest normalized field score and then adds:
    - a small dominance-margin bonus over the second-best field
    - a small coherence bonus based on how much of the total scored field mass belongs to the dominant field
    - a small bounded near-tie ambiguity penalty when the top two fields are almost tied

- **`divergence_score`**
  - TianJi's current ranking score derived from explicit `Im` and `Fa` intermediates.
  - The current formula remains a transparent bounded blend:
    - `divergence_score = 0.65 * Im + 1.35 * Fa`

## Why This Slice Is Small

This first scoring-focused Phase 2 change does **not**:

- alter the feed parsing or normalization stages
- add clustering or multi-event causal logic
- alter SQLite persistence shape
- add model-driven or cloud-based scoring

It only makes the existing deterministic scoring language explicit and testable in first-party TianJi terms.

Later Phase 2 grouping work now adds lightweight evidence-chain metadata inside
`scenario_summary.event_groups` so grouped backtracking can cite corroborating
events. That additive nested-group metadata is separate from the scoring slice
defined here and does not change how `Im`, `Fa`, or top-level artifact versioning
work.

Current grouping work now also supports transitive causal clustering inside
`scenario_summary.event_groups`, adding admission-path `causal_ordered_event_ids`,
`causal_span_hours`, per-link relationship metadata, and `causal_summary`
without changing SQLite table shape.

## Deferred Work

Still deferred after this slice:

- richer `Fa` from corroboration and contradiction handling beyond field-score concentration
- richer `Im` from novelty/spike and baseline deviation signals
- deeper event grouping and causal clustering beyond the current transitive evidence-chain summaries
- broader persisted analysis beyond top scored-event run-history queries

## Current Text-Signal-Intensity Expansion

The current Phase 2 scoring slice now includes:

- **add one bounded text-signal-intensity factor to `Im`**

Intent:

- improve TianJi's ability to distinguish weak textual evidence from strong
  branch-relevant textual evidence inside a single normalized event
- deepen `Im` without changing the meaning of `Fa`
- preserve the current split where `Im` represents branch-moving force and `Fa`
  represents dominant-field alignment

Allowed inputs for this slice:

- normalized `keywords`
- normalized `title`
- normalized `summary`
- existing `field_scores`

Disallowed inputs for this slice:

- prior runs or SQLite history
- grouped-event or cross-event corroboration
- model-driven inference
- any hidden heuristic that cannot be described as one bounded additive term

Constraints implemented in this slice:

- the new factor is a single additive `Im` subcomponent
- it rewards **field-evidence intensity**, not mere text length
- it is bounded so actor and region weighting remain more important than raw text repetition
- `Fa` and the `divergence_score` blend remain unchanged in this branch

Current implementation shape:

- the bonus is derived from boundary-aware dominant-field cue concentration across three normalized
  text surfaces:
  - extracted `keywords`
  - normalized `title`
  - normalized `summary`
- it is intentionally smaller than the combined actor+region contribution
- it is separate from the existing keyword-density bonus because it rewards
  dominant-field-specific cue concentration rather than raw token count
- title and summary cue checks are boundary-aware so short cues like `ai` do not
  receive incidental credit inside unrelated larger words

Recommended verification for this slice:

- paired synthetic-event tests where actor, region, and dominant-field structure
  remain fixed
- stronger dominant-field textual evidence should raise `impact_score`
- the same paired tests should leave `field_attraction` unchanged or nearly so
- one exact-value scoring test should remain pinned so the additive formula stays
  inspectable

Out of scope for this slice:

- novelty or baseline-deviation scoring
- contradiction-aware or corroboration-aware `Fa`
- persistence-schema changes
- grouped-analysis changes
- CLI/history surface expansion unrelated to score validation

## Current Near-Tie Fa Refinement

The current Phase 2 scoring slice now also includes:

- **a bounded near-tie ambiguity penalty inside `Fa`**

Intent:

- reduce `field_attraction` modestly when the dominant field barely leads the
  runner-up field
- keep `Fa` aligned with “belongs to one dominant attractor field” rather than
  rewarding almost-tied top-two distributions too generously
- preserve the existing split where `Fa` remains field-alignment-only and `Im`
  remains branch-moving force

Constraints implemented in this slice:

- dominant field strength remains the base of `Fa`
- the existing margin bonus and coherence bonus remain in place
- the new adjustment is a small bounded subtraction driven only by the margin
  between the top two fields
- in the current implementation, that bounded subtraction applies when the
  rounded top-two margin is below `1.0`
- broad contradiction/corroboration logic is still deferred

Current implementation shape:

- when the top-two field margin is comfortably clear, `Fa` behaves like the
  prior shipped formula
- when that margin becomes a near tie, `Fa` subtracts a small bounded ambiguity
  penalty
- the penalty is intentionally smaller than the dominant-field base and remains a
  surgical ambiguity adjustment rather than a formula rewrite

Recommended verification for this slice:

- paired synthetic-event tests where `Im` inputs stay fixed and only the top-two
  field spread changes
- a near-tie case such as `6.5 vs 6.4` should score lower `Fa` than a more
  clearly dominant case such as `6.5 vs 5.8`
- a clearly dominant case such as `6.5 vs 2.0` should remain stable

## Current Diffuse Mixed-Field Fa Refinement

The current Phase 2 scoring slice now also includes:

- **a bounded diffuse mixed-field penalty inside `Fa` when the top-two margin is already clear**

Intent:

- reduce `field_attraction` modestly for broadly split three-field cases that are
  not near ties but still retain unusually strong third-field support
- keep cleaner two-field structures from scoring too close to materially more
  diffuse mixed-field distributions with the same `Im`
- preserve the current split where `Fa` remains field-alignment-only and `Im`
  remains branch-moving force

Constraints implemented in this slice:

- the near-tie penalty remains the primary top-two ambiguity adjustment
- the new diffuse penalty only applies when the rounded top-two margin is already
  at least `1.0`
- the new diffuse penalty is driven by unusually strong third-field support, not
  by any persistence, history, or grouped context
- the penalty stays bounded and smaller than the dominant-field base

Current implementation shape:

- cleaner two-field cases with a clear dominant margin still behave like the
  prior shipped formula
- when a strong third field remains after the top-two margin is already clear,
  `Fa` subtracts a small bounded diffuse-support penalty
- this remains a surgical ambiguity adjustment rather than a broad contradiction
  model

Recommended verification for this slice:

- paired synthetic-event tests where `Im` inputs stay fixed and the top-two
  margin remains clear
- a diffuse three-field case such as `6.5 / 4.9 / 4.8` should score lower `Fa`
  than a cleaner two-field case such as `6.5 / 4.9 / 0.2`

## Current Implementation Boundary

- `tianji/scoring.py` owns the explicit `Im` and `Fa` computations for now.
- `impact_score` in the artifact corresponds to current TianJi `Im`.
- `field_attraction` in the artifact corresponds to current TianJi `Fa`.

This preserves backward compatibility while making the Phase 2 vocabulary real inside first-party code.

Current persisted operator workflow now also exposes top scored-event `impact_score`,
`field_attraction`, and `divergence_score` in `history`, along with threshold filters
over those top-event values. Those thresholds apply only to the single persisted
top scored event for each run, and runs without scored events expose `null` top
metrics that will not match numeric score filters. `history-compare` now also
reports additive deltas for those same top-event score metrics, while broader
per-event or aggregate scoring queries remain future work.

`history` now also exposes grouped-run summary fields such as `event_group_count`
and the top event group's dominant field, so persisted run triage can use grouped
scenario signals without opening `history-show` or `history-compare` first.

For compare semantics, `top_scored_event_comparable=true` means both runs share
the same top scored `event_id`, so the score deltas can be read as one persisted
leading signal evolving over time. When `top_scored_event_comparable=false`, the
same delta fields remain populated but represent contrast between different top
events rather than longitudinal change of one event.

Single-run persisted analysis now also supports score-aware `history-show`
filtering over stored `scored_events`, including dominant-field, `impact_score`,
`field_attraction`, and `divergence_score` selectors plus per-run event limits.
That drill-down still operates on persisted scored events for one chosen run,
not on broader cross-run aggregate scoring queries. When operators want the
intervention list to stay aligned with the visible scored-event selection,
`history-show` can now optionally keep only intervention candidates whose
`event_id` remains in that final visible scored-event set after filters and
limits.

Single-run grouped analysis now also supports read-time `history-show`
filtering over persisted `scenario_summary.event_groups`, including dominant-field
selection and per-run event-group limits. This is still a view over the stored
scenario summary, not a new grouping or persistence contract.

That same read-time projection model now also applies to `history-compare`, so
paired run comparison can be scoped to one scored-event dominant field, score
window, visible intervention subset, or event-group dominant field/limit without
changing the persisted runs themselves.

Within grouped summaries, `causal_ordered_event_ids` follows the admission-path
tree used to attach members to a cluster, not guaranteed timestamp order.
`causal_span_hours` uses the earliest and latest known timestamps when at least
two are present; otherwise it remains `null`, and `causal_summary` falls back to
non-span wording instead of implying a known timeline.
