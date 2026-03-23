# Milestone: Deterministic scoring-model expansion

This milestone deepens the first-party TianJi scoring model by introducing actor/region title-salience and field-specific impact scaling. It also includes a final documentation pass for the recently shipped CLI comparison and projection semantics.

## TODOs
- [x] Update `SCORING_SPEC.md` to define actor/region title-salience and field-specific impact scaling; verify by rereading the edited sections and confirming each new additive factor maps to a named computation path in `tianji/scoring.py` — expect an implementation-matching scoring spec
- [x] Implement actor/region title-salience bonus in `tianji/scoring.py`; verify with `.venv/bin/python -m unittest tests.test_scoring.ScoringTests.test_score_event_rewards_actor_and_region_title_salience` — expect title mentions to raise `impact_score` without changing `field_attraction`
- [x] Implement field-specific impact scaling in `tianji/scoring.py`; verify with `.venv/bin/python -m unittest tests.test_scoring.ScoringTests.test_score_event_applies_field_specific_impact_scaling` — expect otherwise-similar events in different dominant fields to produce the documented `Im` spread
- [x] Expand `rationale` in `tianji/scoring.py` to expose the new additive components; verify with `.venv/bin/python -m unittest tests.test_scoring.ScoringTests.test_score_event_exposes_explicit_im_fa_semantics` and inspect the rationale list in the assertion payload — expect transparent factor names and values
- [x] Add factor-isolation tests for title-salience and field-scaling in `tests/test_scoring.py`; verify by running `.venv/bin/python -m unittest tests.test_scoring -v` — expect the new isolation cases to pass alongside the existing deterministic scoring suite
- [x] Update `test_score_event_exposes_explicit_im_fa_semantics` in `tests/test_scoring.py` for the final pinned formula; verify with `.venv/bin/python -m unittest tests.test_scoring.ScoringTests.test_score_event_exposes_explicit_im_fa_semantics` — expect exact-value assertions and rationale fragments to match the final implementation
- [x] Perform a doc pass on `README.md` and `DEV_PLAN.md` to capture Phase 2.3 semantics and projected compare-field wording; verify by rereading the edited sections and confirming they describe the new scoring factors plus persisted-truth-versus-projected-view semantics without contradiction — expect repo docs to match shipped behavior

## Final Verification Wave
- [x] Run `.venv/bin/python -m unittest tests.test_scoring -v` — expect the full scoring suite to pass with the final deterministic formulas and rationale output
- [x] Run `.venv/bin/python -m tianji history-show --sqlite-path <temp-sqlite-path> --run-id 1` against a freshly generated persisted fixture run — expect visible rationale entries for the new scoring factors in the returned scored-event payload
- [x] Reread `SCORING_SPEC.md` and `tianji/scoring.py` side by side — expect every named additive factor, bound, and rationale term in the spec to match the final code
- [x] Run `.venv/bin/python -m unittest discover -s tests -v` — expect no regressions across pipeline, history, grouping, CLI, and TUI coverage
