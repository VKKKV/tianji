# TianJi Development Plan

## Current State

Owned TianJi source is intentionally narrow:

- `tianji/` — Python one-shot CLI pipeline
- `tests/` — fixture-first verification

Everything else at the workspace root is reference material, not the long-term product codebase.

## Product Direction

TianJi should grow in this order:

1. strengthen the owned Python core
2. add persistence and repeatable local workflows
3. formalize divergence and backtracking logic
4. complete the CLI-first operator workflow
5. add a terminal UI with Vim-style navigation on top of stable local data/contracts
6. introduce a daemon/orchestrator only after the one-shot path is solid
7. add an optional web UI only after CLI and TUI workflows are stable
8. retire embedded reference repos once their useful ideas are reimplemented in first-party code

## Roadmap Status Snapshot

The current branch has materially advanced two roadmap areas beyond what the early
plan assumed:

- **Phase 2 is partially shipped** through grouped event summaries, evidence-chain
  metadata, transitive causal clustering, admission-path causal ordering, and
  grouped backtrack reasoning.
- **Phase 4 is partially shipped** through SQLite-backed `history`,
  `history-show`, and `history-compare` workflows with score-aware and
  group-aware operator projections.

This means the next development slice should stop treating grouped analysis and
persisted operator ergonomics as the main frontier. The clearest next branch is
to deepen the first-party deterministic scoring model while preserving the now
stable CLI-first workflow.

## Phase 1 — Harden the Owned Core

Goal: turn the current MVP into a dependable local tool.

Shipped in the current branch:

- configurable source list instead of only ad hoc CLI URLs
- SQLite persistence for raw items, normalized events, and run artifacts
- stable artifact schema versioning
- more explicit error handling for malformed feeds and fetch failures
- broader deterministic tests for RSS, Atom, mixed-source runs, and core config/error branches

Still open before Phase 1 is fully closed:

- final doc/examples pass whenever the CLI surface changes again

Exit criteria:

- repeatable runs with local persistence
- no dependence on embedded reference repos at runtime
- test suite covers the core stage transitions

## Phase 2 — Formalize Divergence Logic

Goal: replace rough heuristics with a first-party TianJi scoring model.

Already shipped on this branch:

- dedicated first-party scoring vocabulary and artifact fields for
  `impact_score`, `field_attraction`, and `divergence_score`
- grouped event summaries under `scenario_summary.event_groups`
- evidence-chain metadata on grouped events
- transitive causal clustering for related events
- admission-path-based `causal_ordered_event_ids`
- `causal_span_hours` when at least two timestamps are known
- softened causal wording when span data is incomplete
- grouped backtrack reasons that reference evidence-chain and causal-cluster
  context

Deliverables:

- deepen the scoring model spec inside first-party docs/code so `impact_score`
  and `field_attraction` are more than a thin deterministic slice
- explicit definitions for TianJi versions of `Im` and `Fa`
- broaden deterministic score inputs without making the default path opaque
- keep backtracking tied to explicit evidence and grouped context

Use references from:

- `DivergenceMeter/README.md` for vocabulary and conceptual framing
- `worldmonitor/` for signal extraction and ranking patterns

Do not do:

- direct runtime dependency on DivergenceMeter code
- opaque LLM scoring as the default path

## Phase 3 — Persistence to Local Operating System

Goal: move from one-shot report generation to a durable local system.

Deliverables:

- storage module in first-party TianJi code
- run history and replayable artifacts
- source configuration file and fetch policies
- idempotent dedupe and content-hash storage

This is the point where TianJi starts owning “source code” for ingestion and state instead of leaning on nearby references for design inspiration.

## Phase 4 — CLI Completion

Goal: finish the operator workflow in the terminal before adding any richer interface layer.

Already shipped on this branch:

- persisted run history with mode / dominant-field / risk-level filters
- generated-time filtering for persisted runs
- top scored-event threshold filtering on persisted runs
- grouped-run triage fields and event-group count filters in `history`
- `history-show` navigation via explicit id, `--latest`, `--previous`, and
  `--next`
- `history-show` scored-event dominant-field / threshold / limit projections
- optional intervention alignment to the visible scored-event selection
- `history-show` event-group dominant-field / limit projections
- `history-compare` pair presets for explicit ids, latest pair, against latest,
  and against previous
- grouped evidence/member/link deltas in compare output
- top scored-event score deltas in compare output
- projection-aware compare semantics matching the `history-show` read lens

Deliverables:

- complete persisted history/query ergonomics
- strong compare/navigation shortcuts for stored runs
- stable local docs and commands for day-to-day operator usage
- no hidden dependency on any future daemon or UI layer

This phase ends when TianJi feels coherent as a terminal-first tool even without any additional interface.

Still open before Phase 4 can be called complete:

- tighten parser/validation coverage for compare-projection misuse and negative
  limits
- do a doc pass so projected compare fields versus stored run-summary fields are
  explained in one place without ambiguity
- wait for a concrete operator pain point before expanding the command surface
  again

## Phase 5 — Terminal UI (Vim-Motion TUI)

Goal: add a keyboard-first terminal interface after the CLI surface is complete.

Principles:

- TUI comes **after** CLI maturity, not before
- TUI comes **before** any web GUI
- navigation should be Vim-oriented by default
- TUI should reuse the same local run/history/artifact concepts the CLI already exposes
- no duplication of business logic inside the TUI layer

Planned shape:

- browse run history
- inspect one run and its scored events/interventions
- compare runs
- navigate with Vim-style motions and shortcuts

Do not do:

- do not turn the TUI into a second orchestration runtime
- do not bypass CLI/storage contracts with ad hoc state
- do not let TUI-specific UX force premature web/API design

## Phase 6 — Hongmeng Lite

Goal: introduce a small local orchestrator only when the data path is stable.

Deliverables:

- background process or daemon entrypoint
- local command API over UNIX domain sockets or an equivalent local transport
- job execution for on-demand and scheduled runs
- status inspection from CLI

Keep it narrow:

- no distributed system
- no cloud dependency
- no mandatory web stack

## Phase 7 — Optional Web UI

Goal: add a future web UI without coupling it to the core engine.

Principles:

- UI remains optional and off by default
- UI is a separate service or process boundary
- backend contract should already exist before UI work starts
- CLI and TUI should already be mature before UI work starts
- CLI remains the source-of-truth operator surface

Planned shape:

- lightweight API layer exposing run history, current status, artifacts, and intervention candidates
- WebSocket or polling for live run progress later, not in the first UI slice
- initial UI scope limited to:
  - run a pipeline
  - inspect artifacts
  - compare historical runs
  - browse intervention candidates

TUI remains the preferred rich local interface before this phase exists.

Reference use:

- borrow workflow presentation ideas from `MiroFish/frontend/`
- borrow decoupled service thinking from `worldmonitor/` and `oh-my-openagent/`
- do not adopt any reference frontend wholesale

## Phase 8 — Reference Repo Retirement

Goal: remove the embedded local reference repositories from the long-term TianJi workspace.

Strategy:

1. classify what each reference repo contributes
2. reimplement only the useful pieces inside first-party TianJi modules
3. keep external links or notes to upstream repos for historical context
4. remove the embedded copies once TianJi no longer needs local side-by-side study

Recommended contribution map:

- `worldmonitor/`
  - keep as inspiration for ingestion, signal extraction, caching, and service boundaries
  - reimplement only the narrow server/data ideas TianJi actually needs

- `DivergenceMeter/`
  - keep as conceptual input for divergence terminology
  - reimplement the formulas and tests in owned TianJi code

- `MiroFish/`
  - keep as inspiration for simulation-stage decomposition and future web workflow ideas
  - reimplement a much smaller simulation/report boundary later

- `oh-my-openagent/`
  - keep as inspiration for orchestration, terminal integration, and modular tool boundaries
  - reimplement only if TianJi truly needs those operating patterns

Retirement trigger:

- TianJi has first-party modules for ingestion, scoring, persistence, orchestration, and optional UI planning
- architecture docs cite upstream inspiration without requiring local vendored copies

## Immediate Backlog

1. deepen the first-party deterministic scoring model in `tianji/scoring.py`
2. define a clearer first-party TianJi `Im` / `Fa` spec in docs and tests
3. add focused validation/doc cleanup for compare projections if a short slice is needed
4. define the Vim-motion TUI contract and navigation model only after scoring work lands
5. keep the future local API contract as documentation until a real process boundary is chosen

Recent progress on the now-mostly-shipped CLI workflow:

- `history-compare` now reports additive grouped evidence/member/link deltas for the top persisted event group, using already-stored `scenario_summary` data without changing the SQLite schema
- grouped compare semantics are now clearer: `top_event_group_evidence_diff.comparable` only turns true when both runs share the same top-group `headline_event_id`
- compare-side payloads now avoid redundant flattened top-group fields; nested `top_event_group` plus `top_event_group_evidence_diff` are the maintained read surface
- `history` now surfaces top scored-event `impact_score`, `field_attraction`, and `divergence_score`, with threshold filters for persisted score-aware analysis
- `history-compare` now reports additive top scored-event deltas for `impact_score`, `field_attraction`, and `divergence_score`
- `history-show` now supports dominant-field, score-threshold, and limit controls for persisted scored-event drill-down within one run
- `history-show` can now optionally align intervention candidates with the visible scored-event selection via `--only-matching-interventions`
- `history-show` now supports dominant-field and limit controls for persisted single-run event-group drill-down
- `scenario_summary.event_groups` now support transitive causal clustering, causal ordering, and richer backtrack reasoning without changing SQLite table shape
- `history` now exposes top event-group summary fields and filters for grouped-run triage across persisted runs
- `history-compare` now supports the same scored-event and event-group projection filters as `history-show` for lens-specific persisted comparison

Draft contract note now lives in `LOCAL_API_CONTRACT.md`; implementation remains future work.

## Next Recommended Milestone

### Milestone: Deterministic scoring-model expansion

Why this is next:

- grouped analysis and persisted operator workflows are no longer the weakest
  part of the owned product surface
- the current scoring slice works, but it is still thin relative to the richer
  grouping and comparison machinery now built around it
- improving the first-party scoring model sharpens both single-run output and
  persisted run comparison without changing the overall product shape

Primary files:

- `tianji/scoring.py`
- `tianji/normalize.py`
- `tianji/models.py`
- `SCORING_SPEC.md`
- `tests/test_pipeline.py`

Suggested scope for the next scoring branch:

1. inventory the current deterministic factors already available from normalized
   events
2. define how those factors should contribute to `impact_score` and
   `field_attraction`
3. keep the model additive and inspectable in code and docs
4. extend tests so score changes are asserted explicitly rather than only through
   top-level ordering
5. avoid schema churn unless a scoring explanation field truly requires it

Current code-grounded factor inventory:

- **Current `Im` inputs in `tianji/scoring.py`**
  - weighted actor presence from normalized `actors`
  - weighted region presence from normalized `regions`
  - bounded keyword density from normalized `keywords`
  - a dominant-field-strength evidence bonus from `field_scores`
  - a nonzero-field-count diversity bonus from `field_scores`
- **Current `Fa` inputs in `tianji/scoring.py`**
  - dominant field strength from `field_scores`
  - dominance margin over the second-best field
  - coherence share of dominant-field mass over total positive field mass
- **Current normalized inputs available for future deterministic use**
  - normalized `title` and `summary`
  - extracted `keywords`
  - extracted `actors`
  - extracted `regions`
  - per-field score distribution in `field_scores`
  - optional `published_at` timestamp
- **Important current constraint**
  - the scoring stage currently has only single-event inputs; anything requiring
    historical baselines, cross-run novelty, or multi-event corroboration should
    stay deferred unless the scoring contract is intentionally widened

Recommended execution order for that branch:

#### Step 1 — factor inventory and scoring gap review

- read `tianji/normalize.py`, `tianji/models.py`, and `tianji/scoring.py`
- list which normalized signals already exist and are stable enough to score
- separate currently used factors from deferred-but-already-available factors

Expected outcome:

- one written inventory of available deterministic inputs
- one explicit note describing why each candidate factor belongs in `Im`, `Fa`,
  or neither

#### Step 2 — spec expansion before code changes

- update `SCORING_SPEC.md` before deep code edits
- define the next bounded deterministic expansion of TianJi `Im` and `Fa`
- preserve the rule that the default scoring path stays inspectable and local

Expected outcome:

- a revised scoring spec that explains each additive factor, its intent, and its
  scoring constraints
- a short deferred-work list for anything that would require baseline history,
  multi-run context, or non-deterministic inference

#### Step 3 — narrow implementation in `tianji/scoring.py`

- implement only the factors already justified in the spec
- keep score computation bounded and easy to trace from input event to output
  artifact fields
- avoid coupling scoring changes to persistence or CLI read-surface changes

Expected outcome:

- richer but still deterministic `impact_score` and `field_attraction`
- unchanged pipeline shape and unchanged storage contract unless a clearly
  justified additive explanation field is needed

#### Step 4 — score-specific verification

- add focused tests in `tests/test_pipeline.py`
- assert component-level scoring behavior where possible, not only ranked order
- verify the full suite still passes after scoring changes

Expected outcome:

- tests that make future scoring regressions legible
- preserved confidence in existing history/grouping/backtracking behavior

Recommended verification additions:

- these `Im` factor-isolation tests are now shipped:
  - actor-weight changes inside `Im`
  - region-weight changes inside `Im`
  - keyword-density cap behavior
  - nonzero-field-count effects inside `Im`
  - dominant-field-strength effects inside `Im`
  - direct keyword/title/summary text-signal surface contributions
- add factor-isolation tests for dominance-margin and coherence effects inside
  `Fa`
- keep at least one exact-value test for a representative scored event so the
  documented formula remains pinned
- keep persisted CLI score-filter tests focused on read behavior, not on formula
  internals

#### Step 5 — doc and operator pass

- update README/plan/spec wording only if scoring semantics materially changed
- keep CLI operator docs aligned with shipped reality, not aspirational formulas

Expected outcome:

- shipped docs explain the new scoring slice accurately
- no accidental roadmap drift toward daemon/API work during scoring iteration

Out of scope for that branch:

- multi-run baseline deviation scoring
- dynamic novelty models that require persisted historical distributions
- opaque model-driven scoring
- TUI, daemon, or API implementation work
- broad CLI expansion unrelated to scoring validation

Smaller fallback slice if the next session stays in CLI/docs work:

- this fallback is now largely shipped:
  - `history-compare` rejects negative compare limits and mixed preset misuse
  - README/hand-off text now explains projected compare fields versus stored
    run-summary fields
  - inverted score windows are now rejected consistently across `history`,
    `history-show`, and `history-compare`
- remaining CLI/docs fallback work should now be limited to small wording polish
  only if operator confusion still appears in practice

## Next Session Checklist

If the next session starts the scoring branch directly, the fastest disciplined
sequence is:

1. read `tianji/scoring.py`, `tianji/normalize.py`, and the scoring tests near
   `test_score_event_exposes_explicit_im_fa_semantics`
2. write a short factor inventory note inside `SCORING_SPEC.md`
3. choose one narrow scoring expansion and explicitly mark what stays deferred
4. add or update score-specific tests before broad pipeline adjustments
5. change `tianji/scoring.py` only after the intended deterministic semantics are
   written down
6. run the full unittest suite after scoring changes, not only targeted tests

Practical recommendation:

- prefer one additive scoring improvement with strong tests over a broad scoring
  rewrite that also touches persistence, grouping, and CLI behavior in the same
  branch

## First Scoring-Branch Candidates

To reduce startup ambiguity for the next implementation session, prefer choosing
one of these bounded branches instead of reopening scoring design from scratch.

### Candidate A — enrich `Im` with text-signal intensity (recommended)

Shape:

- keep current actor, region, keyword, dominant-field, and nonzero-field inputs
- add one or two bounded text-derived intensity signals from already-normalized
  single-event data
- keep `Fa` unchanged except for any necessary rationale wording updates

Why this is the best first branch:

- `Im` is currently the shallower side of the model
- this uses inputs TianJi already has without widening pipeline scope
- it should improve single-event ranking without changing persistence or CLI read
  semantics
- it keeps the meaning split clean: `Im` measures branch-moving force, `Fa`
  measures field alignment

Risks:

- text-derived factors can become noisy if they duplicate keyword density rather
  than adding a genuinely distinct signal
- too many micro-bonuses would make the score harder to reason about

Success criteria:

- one clearly explained new `Im` factor lands in spec, code, and tests
- score changes are observable in exact-value or factor-isolation tests
- no schema, persistence, or CLI contract changes are required

### Candidate B — refine `Fa` ambiguity handling

Shape:

- leave `Im` alone
- make `Fa` more sensitive to mixed-field ambiguity, contradiction, or weak field
  concentration using only the current `field_scores` distribution

Why it could be good:

- `Fa` is already conceptually crisp, so this branch could stay mathematically
  simple
- it would sharpen how TianJi distinguishes clear-field signals from mixed ones

Why it is not the first recommendation:

- the current `Fa` formula already has coherent margin and concentration logic
- improvements here are likely to be incremental rather than clearly unlocking a
  richer Phase 2 slice

Success criteria:

- ambiguous mixed-field events lose rank relative to clearer same-domain events
- the formula remains easy to explain in one short section of `SCORING_SPEC.md`

### Candidate C — rationale transparency only

Shape:

- keep the formulas mostly intact
- expand `rationale` so each score exposes more of its additive components

Why it could be useful:

- improves inspectability immediately
- lowers future debugging and operator-explanation cost

Why it should not be first on its own:

- Phase 2 currently needs better scoring semantics more than better formatting of
  existing semantics
- rationale-only work does not meaningfully deepen the model

Success criteria:

- score explanations become more legible without changing persisted contract shape
  in a breaking way
- at least one exact-value test also pins rationale structure for the chosen
  branch

### Candidate D — weight-table recalibration only

Shape:

- retune `ACTOR_WEIGHTS`, `REGION_WEIGHTS`, or coefficient constants without
  adding new factor types

Why it is weakest as a first branch:

- this can improve outputs, but it does not really advance the model structure
- it invites subjective tuning without a strong new conceptual contract

Use only if:

- testing shows the current model shape is right but one or two constants are
  clearly distorting rankings

Recommended choice:

- start with **Candidate A** unless fresh code reading reveals an obviously more
  urgent `Fa` ambiguity flaw
- if the team wants the smallest possible slice, combine **Candidate A** with the
  test additions from the verification plan and defer all other scoring changes

Current status:

- **Candidate A is now shipped** in first-party code, tests, and docs
- title/summary cue matching is now boundary-aware, so short cues like `ai` do
  not gain accidental credit inside unrelated larger words
- text-signal cue matching now uses cached compiled regexes to avoid repeated
  pattern compilation in the scoring loop
- `score_event()` now computes dominant field and text-signal intensity once and
  reuses those values for both `impact_score` and rationale construction

What this means for the next scoring branch:

- do not reopen the original Candidate A design work unless a concrete bug shows
  the current boundary-aware cue model is still wrong
- the next most likely worthwhile scoring branch is now **Candidate B** or
  another similarly narrow deterministic refinement

### Candidate A concrete proposal

The first implementation-ready version of Candidate A should stay narrow:

- add **text-signal intensity** to `Im`, not a broad new event-understanding layer
- derive it only from information already present on one normalized event
- keep the contribution bounded and additive
- keep `Fa` and `divergence_score` weighting unchanged for this first branch

Recommended interpretation of text-signal intensity:

- reward events whose normalized text contains a denser concentration of
  high-salience field evidence than the current plain keyword-count cap can
  express
- distinguish “many generic extracted tokens” from “strong repeated branch-relevant
  signals” without requiring multi-run baselines or cross-event corroboration

Allowed signal sources for this first branch:

- normalized `keywords`
- normalized `title`
- normalized `summary`
- existing per-field score distribution in `field_scores`

Disallowed signal sources for this first branch:

- any historical baseline from SQLite or prior runs
- grouped-event or cross-event corroboration
- external models or remote services
- fuzzy heuristics that cannot be explained as one bounded additive term

Recommended shape of the new factor:

- one new bounded `Im` subcomponent only
- it should reward **field-evidence intensity**, not simply raw length
- it should be explainable in one sentence and testable in isolation
- it should have a lower ceiling than the existing actor+region contribution so
  TianJi does not collapse into text-only ranking

What this factor should not become:

- not a second copy of keyword density
- not a hidden reweighting of `Fa`
- not a latent contradiction detector
- not a surrogate for future novelty or baseline-deviation work

Preferred verification target for Candidate A:

- create paired synthetic events where actor, region, and dominant field are held
  constant
- vary only the strength of branch-relevant textual evidence
- assert that the stronger-text event gains `impact_score` while `field_attraction`
  stays unchanged or near-unchanged
- keep one exact-value test updated so the intended additive formula remains
  pinned in docs and code

Decision rule for whether Candidate A succeeded:

- if the new factor improves `Im` discrimination between weak-text and
  strong-text events without forcing storage, CLI, or grouping changes, the
  branch was scoped correctly
- if the implementation requires multiple new bonuses, explanation churn, or
  compare-surface updates, the branch is too broad and should be narrowed again

### Candidate A implementation checklist

Use this as the concrete execution order when the next session starts coding.

#### `SCORING_SPEC.md`

- confirm the existing shipped slice still describes current code accurately
- convert the planned text-signal-intensity factor into one short explicit rule
- name the new `Im` subcomponent and state its cap
- keep deferred items listed explicitly so novelty, contradiction, and baseline
  work do not leak into the branch

#### `tests/test_pipeline.py`

- add one paired-event test where only textual field-evidence intensity changes
- keep actor, region, and dominant-field structure fixed in that paired test
- assert `impact_score` increases for the stronger-text event
- assert `field_attraction` stays the same or within the expected unchanged range
- update one exact-value test if the additive `Im` formula changes numerically
- leave persisted history/history-show/history-compare tests alone unless the
  score outputs they already inspect legitimately change

Suggested exact Candidate A test cases:

- **`test_score_event_rewards_stronger_text_signal_intensity_in_im`**
  - build two `NormalizedEvent` values with the same actors, regions, and
    `field_scores`
  - keep the same dominant field on both events
  - give the stronger event denser branch-relevant text in `title`, `summary`,
    and/or `keywords`
  - assert higher `impact_score` for the stronger-text event
  - assert equal or near-equal `field_attraction`

- **`test_score_event_text_signal_intensity_does_not_reward_generic_keyword_mass`**
  - compare a branch-relevant technology-heavy text against a generic long text
    with many extracted tokens but weaker field-evidence concentration
  - keep actor/region structure fixed
  - assert the generic-token event does not gain the same `Im` benefit purely
    from token volume

- **`test_score_event_text_signal_intensity_respects_cap`**
  - compare a strong-text event against an exaggerated version with repeated
    branch-relevant cues
  - assert the new `Im` subcomponent saturates rather than growing unbounded

- **`test_score_event_text_signal_intensity_ignores_incidental_substrings`**
  - prove short cues like `ai` do not receive credit inside unrelated larger
    words such as `air` or `fair`
  - keep actor/region/field structure fixed so only boundary behavior is tested

- **`test_score_event_text_signal_intensity_matches_punctuation_adjacent_cues`**
  - prove punctuation-adjacent cues like `chip,` and `cyber,` still count as
    real dominant-field evidence
  - keep `field_attraction` unchanged so the test isolates `Im`

- **update `test_score_event_exposes_explicit_im_fa_semantics`**
  - keep it as the pinned exact-value formula test
  - update expected `impact_score`, `divergence_score`, and rationale entries if
    the new bounded `Im` term changes them
  - keep `dominant_field` and `field_attraction` expectations explicit so `Fa`
    drift is caught immediately

#### `tianji/scoring.py`

- introduce the new bounded `Im` subcomponent in `compute_im`
- keep the current actor, region, keyword-density, dominant-field, and nonzero-
  field logic readable rather than folding everything into one opaque expression
- avoid changing `compute_fa` in this branch unless a test proves the new `Im`
  factor accidentally forced an `Fa` adjustment
- keep `compute_divergence_score` unchanged for the first Candidate A slice
- update rationale text only enough to keep the score legible after the new `Im`
  factor lands

#### Verification commands

- run `.venv/bin/python -m unittest discover -s tests -v`
- if exact-value score assertions change, inspect whether the delta is explained
  by the new bounded `Im` term rather than unintended collateral changes

Abort conditions for the branch:

- if the new factor cannot be described in one sentence
- if the tests require touching persistence or CLI parser behavior
- if `Fa` starts changing for reasons unrelated to field alignment
- if the rationale becomes less interpretable after the change instead of more
  interpretable

Recommended next branch after Candidate A hardening:

- prefer a narrow **Candidate B** exploration over more Candidate A churn
  unless a real text-boundary bug is observed
- if pursuing Candidate B, start with one mixed-field synthetic-event pair that
  exposes a concrete `Fa` ambiguity before changing formula constants
- keep the same discipline used for Candidate A:
  - spec/document the intended semantics first
  - add one or two factor-isolation tests
  - keep persistence, CLI, grouping, and schema unchanged

Current status after the first Candidate B slice:

- a bounded near-tie `Fa` ambiguity penalty is now shipped
- a bounded diffuse mixed-field `Fa` penalty is now also shipped for cases where
  the top-two margin is already clear but third-field support remains unusually
  strong
- compare preset misuse, negative compare limits, and inverted persisted score
  windows are now parser-rejected across the read-only operator surface
- additive `Im` terms and direct text-signal bonus surfaces now have explicit
  isolation coverage in `tests/test_pipeline.py`
- the next likely useful work is now either a further narrow `Fa` refinement only
  if a new concrete mixed-field case still escapes the current ambiguity rules,
  or a return to pure scoring-model depth inside `Im`

## Phase Boundary Notes

To keep future sessions from reopening already-mostly-solved branches, use these
working boundaries:

- **Phase 2 next work** = deepen first-party deterministic scoring semantics
  inside the existing pipeline shape
- **Phase 4 remaining work** = only small operator-wording polish unless a new
  persisted-analysis gap appears
- **Phase 5 start condition** = scoring semantics and CLI operator workflows feel
  stable enough that a TUI can reuse them without forcing contract churn
- **Local API work** stays contract-only until a real process boundary exists;
  `LOCAL_API_CONTRACT.md` is a planning input, not a near-term implementation
  trigger

## Guardrails

- keep first-party source under `tianji/` and `tests/`
- prefer reimplementation over cross-importing from references
- avoid framework-first expansion
- keep web UI future-compatible but not current-scope
- every new layer should preserve local-first, deterministic-first behavior
