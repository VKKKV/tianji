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
    - the event's current field-attraction strength

- **`Fa` (Field Attraction)**
  - TianJi's deterministic estimate of how strongly an event belongs to its dominant attractor field.
  - In the current slice, it is the strongest normalized field score already produced by `normalize.py`.

- **`divergence_score`**
  - TianJi's current ranking score derived from explicit `Im` and `Fa` intermediates.
  - The current formula remains a transparent bounded blend:
    - `divergence_score = 0.65 * Im + 1.35 * Fa`

## Why This Slice Is Small

This first Phase 2 change does **not**:

- alter the feed parsing or normalization stages
- change the artifact schema
- add clustering or multi-event causal logic
- alter SQLite persistence shape
- add model-driven or cloud-based scoring

It only makes the existing deterministic scoring language explicit and testable in first-party TianJi terms.

## Deferred Work

Still deferred after this slice:

- richer `Fa` from dominance margin, corroboration, and contradiction handling
- richer `Im` from novelty/spike and baseline deviation signals
- event grouping and causal clustering
- evidence-chain backtracking
- persisted run-history queries over scoring components

## Current Implementation Boundary

- `tianji/scoring.py` owns the explicit `Im` and `Fa` computations for now.
- `impact_score` in the artifact corresponds to current TianJi `Im`.
- `field_attraction` in the artifact corresponds to current TianJi `Fa`.

This preserves backward compatibility while making the Phase 2 vocabulary real inside first-party code.
