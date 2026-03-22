from support import *


class ScoringTests(unittest.TestCase):
        def test_score_event_exposes_explicit_im_fa_semantics(self) -> None:
            event = NormalizedEvent(
                event_id="evt-1",
                source="fixture:test",
                title="Coordinated chip sanctions and cyber controls expand",
                summary="Officials expand coordinated chip sanctions after cyber escalation.",
                link="https://example.com/evt-1",
                published_at="2026-03-22T12:00:00Z",
                keywords=[
                    "coordinated",
                    "chip",
                    "sanctions",
                    "cyber",
                    "controls",
                    "escalation",
                ],
                actors=["usa", "china"],
                regions=["east-asia", "united-states"],
                field_scores={
                    "technology": 6.5,
                    "diplomacy": 2.0,
                    "economy": 1.5,
                    "conflict": 0.0,
                },
            )

            scored = score_event(event)

            self.assertEqual(scored.dominant_field, "technology")
            self.assertEqual(scored.impact_score, 13.56)
            self.assertEqual(scored.field_attraction, 7.66)
            self.assertEqual(scored.divergence_score, 19.16)
            self.assertIn("Im=13.56", scored.rationale)
            self.assertIn("Fa=7.66", scored.rationale)
            self.assertIn("im_text_signal_intensity=0.84", scored.rationale)
            self.assertIn("dominant_field=technology:7.66", scored.rationale)

        def test_score_event_rewards_clearer_field_alignment_in_fa(self) -> None:
            clearer_event = NormalizedEvent(
                event_id="evt-clear",
                source="fixture:test",
                title="Clear technology signal",
                summary="A strong single-field technology event.",
                link="https://example.com/clear",
                published_at="2026-03-22T12:00:00Z",
                keywords=["chip", "cyber", "controls", "sanctions"],
                actors=["usa", "china"],
                regions=["east-asia", "united-states"],
                field_scores={
                    "technology": 6.5,
                    "diplomacy": 2.0,
                    "economy": 1.5,
                    "conflict": 0.0,
                },
            )
            ambiguous_event = NormalizedEvent(
                event_id="evt-ambiguous",
                source="fixture:test",
                title="Ambiguous technology signal",
                summary="An event split across multiple attractor fields.",
                link="https://example.com/ambiguous",
                published_at="2026-03-22T12:05:00Z",
                keywords=["chip", "cyber", "talks", "trade"],
                actors=["usa", "china"],
                regions=["east-asia", "united-states"],
                field_scores={
                    "technology": 6.5,
                    "diplomacy": 5.8,
                    "economy": 5.2,
                    "conflict": 0.0,
                },
            )

            clearer_scored = score_event(clearer_event)
            ambiguous_scored = score_event(ambiguous_event)

            self.assertGreater(
                clearer_scored.field_attraction, ambiguous_scored.field_attraction
            )
            self.assertGreater(
                clearer_scored.divergence_score, ambiguous_scored.divergence_score
            )

        def test_score_event_penalizes_near_tie_field_alignment_in_fa(self) -> None:
            moderately_ambiguous_event = NormalizedEvent(
                event_id="evt-moderately-ambiguous",
                source="fixture:test",
                title="Shared field ambiguity case",
                summary="Shared event text for top-two field ambiguity checks.",
                link="https://example.com/moderately-ambiguous",
                published_at="2026-03-22T12:06:00Z",
                keywords=["chip", "cyber", "talks", "trade"],
                actors=["usa", "china"],
                regions=["east-asia", "united-states"],
                field_scores={
                    "technology": 6.5,
                    "diplomacy": 5.8,
                    "economy": 0.5,
                    "conflict": 0.0,
                },
            )
            near_tie_event = NormalizedEvent(
                event_id="evt-near-tie",
                source="fixture:test",
                title="Shared field ambiguity case",
                summary="Shared event text for top-two field ambiguity checks.",
                link="https://example.com/near-tie",
                published_at="2026-03-22T12:07:00Z",
                keywords=["chip", "cyber", "talks", "trade"],
                actors=["usa", "china"],
                regions=["east-asia", "united-states"],
                field_scores={
                    "technology": 6.5,
                    "diplomacy": 6.4,
                    "economy": 0.5,
                    "conflict": 0.0,
                },
            )

            moderately_ambiguous_scored = score_event(moderately_ambiguous_event)
            near_tie_scored = score_event(near_tie_event)

            self.assertGreater(
                moderately_ambiguous_scored.field_attraction,
                near_tie_scored.field_attraction,
            )
            self.assertEqual(
                moderately_ambiguous_scored.impact_score,
                near_tie_scored.impact_score,
            )
            self.assertGreater(
                moderately_ambiguous_scored.divergence_score,
                near_tie_scored.divergence_score,
            )

        def test_score_event_near_tie_penalty_starts_below_margin_threshold(self) -> None:
            threshold_margin_event = NormalizedEvent(
                event_id="evt-near-tie-threshold",
                source="fixture:test",
                title="Shared field ambiguity case",
                summary="Shared event text for top-two field ambiguity checks.",
                link="https://example.com/near-tie-threshold",
                published_at="2026-03-22T12:07:30Z",
                keywords=["chip", "cyber", "talks", "trade"],
                actors=["usa", "china"],
                regions=["east-asia", "united-states"],
                field_scores={
                    "technology": 6.5,
                    "diplomacy": 5.5,
                    "economy": 0.5,
                    "conflict": 0.0,
                },
            )
            below_threshold_margin_event = NormalizedEvent(
                event_id="evt-near-tie-below-threshold",
                source="fixture:test",
                title="Shared field ambiguity case",
                summary="Shared event text for top-two field ambiguity checks.",
                link="https://example.com/near-tie-below-threshold",
                published_at="2026-03-22T12:07:31Z",
                keywords=["chip", "cyber", "talks", "trade"],
                actors=["usa", "china"],
                regions=["east-asia", "united-states"],
                field_scores={
                    "technology": 6.5,
                    "diplomacy": 5.52,
                    "economy": 0.48,
                    "conflict": 0.0,
                },
            )

            threshold_scored = score_event(threshold_margin_event)
            below_threshold_scored = score_event(below_threshold_margin_event)

            self.assertEqual(
                threshold_scored.impact_score, below_threshold_scored.impact_score
            )
            self.assertGreater(
                threshold_scored.field_attraction, below_threshold_scored.field_attraction
            )
            self.assertGreater(
                threshold_scored.divergence_score, below_threshold_scored.divergence_score
            )

        def test_score_event_keeps_clear_field_alignment_semantics_stable(self) -> None:
            clear_event = NormalizedEvent(
                event_id="evt-clear-stable",
                source="fixture:test",
                title="Clear technology signal",
                summary="A strong single-field technology event.",
                link="https://example.com/clear-stable",
                published_at="2026-03-22T12:08:00Z",
                keywords=["chip", "cyber", "controls", "sanctions"],
                actors=["usa", "china"],
                regions=["east-asia", "united-states"],
                field_scores={
                    "technology": 6.5,
                    "diplomacy": 2.0,
                    "economy": 1.5,
                    "conflict": 0.0,
                },
            )

            clear_scored = score_event(clear_event)

            self.assertEqual(clear_scored.field_attraction, 7.66)

        def test_score_event_treats_zero_field_mass_as_uncategorized(self) -> None:
            event = NormalizedEvent(
                event_id="evt-zero-field-mass",
                source="fixture:test",
                title="Neutral update without field cues",
                summary="Officials issue a general process update without branch-specific cues.",
                link="https://example.com/zero-field-mass",
                published_at="2026-03-22T12:08:15Z",
                keywords=["neutral", "update", "officials", "process"],
                actors=["observer"],
                regions=["unknown-region"],
                field_scores={
                    "technology": 0.0,
                    "diplomacy": 0.0,
                    "economy": 0.0,
                    "conflict": 0.0,
                },
            )

            scored = score_event(event)

            self.assertEqual(scored.dominant_field, "uncategorized")
            self.assertEqual(scored.field_attraction, 0.0)
            self.assertEqual(scored.impact_score, 5.1)
            self.assertEqual(scored.divergence_score, 3.31)
            self.assertNotIn("im_text_signal_intensity=0.0", scored.rationale)
            self.assertIn("dominant_field=uncategorized:0", scored.rationale)

        def test_score_event_zero_field_mass_does_not_produce_negative_fa(self) -> None:
            uncategorized_event = NormalizedEvent(
                event_id="evt-zero-field-fa",
                source="fixture:test",
                title="Neutral update without field cues",
                summary="Officials issue a general process update without branch-specific cues.",
                link="https://example.com/zero-field-fa",
                published_at="2026-03-22T12:08:16Z",
                keywords=["neutral", "update", "brief"],
                actors=["observer"],
                regions=["unknown-region"],
                field_scores={
                    "technology": 0.0,
                    "diplomacy": 0.0,
                    "economy": 0.0,
                    "conflict": 0.0,
                },
            )
            weakly_categorized_event = NormalizedEvent(
                event_id="evt-weak-field-fa",
                source="fixture:test",
                title="Weak technology cue",
                summary="A light chip-related update.",
                link="https://example.com/weak-field-fa",
                published_at="2026-03-22T12:08:17Z",
                keywords=["neutral", "update", "brief"],
                actors=["observer"],
                regions=["unknown-region"],
                field_scores={
                    "technology": 0.2,
                    "diplomacy": 0.0,
                    "economy": 0.0,
                    "conflict": 0.0,
                },
            )

            uncategorized_scored = score_event(uncategorized_event)
            weakly_categorized_scored = score_event(weakly_categorized_event)

            self.assertGreaterEqual(uncategorized_scored.field_attraction, 0.0)
            self.assertGreater(
                weakly_categorized_scored.field_attraction,
                uncategorized_scored.field_attraction,
            )

        def test_score_event_exact_top_field_tie_is_order_independent(self) -> None:
            technology_first_event = NormalizedEvent(
                event_id="evt-tie-tech-first",
                source="fixture:test",
                title="Chip controls and talks advance together",
                summary="Officials discuss chip controls while talks continue.",
                link="https://example.com/tie-tech-first",
                published_at="2026-03-22T12:08:18Z",
                keywords=["chip", "controls", "talks", "officials"],
                actors=["usa"],
                regions=["east-asia"],
                field_scores={
                    "technology": 6.0,
                    "diplomacy": 6.0,
                    "economy": 1.0,
                    "conflict": 0.0,
                },
            )
            diplomacy_first_event = NormalizedEvent(
                event_id="evt-tie-diplomacy-first",
                source="fixture:test",
                title="Chip controls and talks advance together",
                summary="Officials discuss chip controls while talks continue.",
                link="https://example.com/tie-diplomacy-first",
                published_at="2026-03-22T12:08:19Z",
                keywords=["chip", "controls", "talks", "officials"],
                actors=["usa"],
                regions=["east-asia"],
                field_scores={
                    "diplomacy": 6.0,
                    "technology": 6.0,
                    "economy": 1.0,
                    "conflict": 0.0,
                },
            )

            technology_first_scored = score_event(technology_first_event)
            diplomacy_first_scored = score_event(diplomacy_first_event)

            self.assertEqual(technology_first_scored.dominant_field, "diplomacy")
            self.assertEqual(
                technology_first_scored.dominant_field,
                diplomacy_first_scored.dominant_field,
            )
            self.assertEqual(
                technology_first_scored.impact_score,
                diplomacy_first_scored.impact_score,
            )
            self.assertEqual(
                technology_first_scored.field_attraction,
                diplomacy_first_scored.field_attraction,
            )
            self.assertEqual(
                technology_first_scored.divergence_score,
                diplomacy_first_scored.divergence_score,
            )
            self.assertIn(
                "dominant_field=diplomacy:6.05", technology_first_scored.rationale
            )
            self.assertIn("dominant_field=diplomacy:6.05", diplomacy_first_scored.rationale)
            self.assertIn(
                "im_text_signal_intensity=0.42", technology_first_scored.rationale
            )
            self.assertIn("im_text_signal_intensity=0.42", diplomacy_first_scored.rationale)

        def test_summarize_scenario_resolves_dominant_field_ties_independently_of_event_order(
            self,
        ) -> None:
            diplomacy_event = ScoredEvent(
                event_id="evt-summary-diplomacy",
                title="Diplomatic channel hardens",
                source="fixture:test",
                link="https://example.com/summary-diplomacy",
                published_at="2026-03-22T12:08:20Z",
                actors=["usa"],
                regions=["east-asia"],
                keywords=["talks", "sanction"],
                dominant_field="diplomacy",
                impact_score=10.0,
                field_attraction=5.0,
                divergence_score=13.25,
                rationale=["Im=10.0", "Fa=5.0", "dominant_field=diplomacy:5.0"],
            )
            technology_event = ScoredEvent(
                event_id="evt-summary-technology",
                title="Technology channel hardens",
                source="fixture:test",
                link="https://example.com/summary-technology",
                published_at="2026-03-22T12:08:21Z",
                actors=["china"],
                regions=["united-states"],
                keywords=["chip", "cyber"],
                dominant_field="technology",
                impact_score=10.0,
                field_attraction=5.0,
                divergence_score=13.25,
                rationale=["Im=10.0", "Fa=5.0", "dominant_field=technology:5.0"],
            )

            diplomacy_first_summary = summarize_scenario(
                [diplomacy_event, technology_event]
            )
            technology_first_summary = summarize_scenario(
                [technology_event, diplomacy_event]
            )

            self.assertEqual(
                diplomacy_first_summary["dominant_field"],
                technology_first_summary["dominant_field"],
            )
            self.assertEqual(diplomacy_first_summary["dominant_field"], "diplomacy")

        def test_score_event_applies_dominance_margin_inside_fa(self) -> None:
            narrower_margin_event = NormalizedEvent(
                event_id="evt-fa-margin-narrow",
                source="fixture:test",
                title="Shared field structure",
                summary="Shared field structure summary.",
                link="https://example.com/fa-margin-narrow",
                published_at="2026-03-22T12:08:30Z",
                keywords=["chip", "cyber", "controls", "trade"],
                actors=["usa", "china"],
                regions=["east-asia", "united-states"],
                field_scores={
                    "technology": 6.0,
                    "diplomacy": 3.0,
                    "economy": 1.0,
                    "conflict": 0.0,
                },
            )
            wider_margin_event = NormalizedEvent(
                event_id="evt-fa-margin-wide",
                source="fixture:test",
                title="Shared field structure",
                summary="Shared field structure summary.",
                link="https://example.com/fa-margin-wide",
                published_at="2026-03-22T12:08:31Z",
                keywords=["chip", "cyber", "controls", "trade"],
                actors=["usa", "china"],
                regions=["east-asia", "united-states"],
                field_scores={
                    "technology": 6.0,
                    "diplomacy": 2.0,
                    "economy": 2.0,
                    "conflict": 0.0,
                },
            )

            narrower_scored = score_event(narrower_margin_event)
            wider_scored = score_event(wider_margin_event)

            self.assertEqual(narrower_scored.impact_score, wider_scored.impact_score)
            self.assertGreater(
                wider_scored.field_attraction, narrower_scored.field_attraction
            )

        def test_score_event_applies_coherence_inside_fa(self) -> None:
            tighter_coherence_event = NormalizedEvent(
                event_id="evt-fa-coherence-tight",
                source="fixture:test",
                title="Shared field structure",
                summary="Shared field structure summary.",
                link="https://example.com/fa-coherence-tight",
                published_at="2026-03-22T12:08:32Z",
                keywords=["chip", "cyber", "controls", "trade"],
                actors=["usa", "china"],
                regions=["east-asia", "united-states"],
                field_scores={
                    "technology": 6.0,
                    "diplomacy": 2.0,
                    "economy": 1.0,
                    "conflict": 0.5,
                },
            )
            more_diffuse_event = NormalizedEvent(
                event_id="evt-fa-coherence-diffuse",
                source="fixture:test",
                title="Shared field structure",
                summary="Shared field structure summary.",
                link="https://example.com/fa-coherence-diffuse",
                published_at="2026-03-22T12:08:33Z",
                keywords=["chip", "cyber", "controls", "trade"],
                actors=["usa", "china"],
                regions=["east-asia", "united-states"],
                field_scores={
                    "technology": 6.0,
                    "diplomacy": 2.0,
                    "economy": 1.5,
                    "conflict": 0.5,
                },
            )

            tighter_scored = score_event(tighter_coherence_event)
            diffuse_scored = score_event(more_diffuse_event)

            self.assertEqual(tighter_scored.impact_score, diffuse_scored.impact_score)
            self.assertGreater(
                tighter_scored.field_attraction, diffuse_scored.field_attraction
            )

        def test_score_event_penalizes_diffuse_mixed_field_support_in_fa(self) -> None:
            clearer_two_field_event = NormalizedEvent(
                event_id="evt-fa-diffuse-clearer",
                source="fixture:test",
                title="Shared field structure",
                summary="Shared field structure summary.",
                link="https://example.com/fa-diffuse-clearer",
                published_at="2026-03-22T12:08:34Z",
                keywords=["chip", "cyber", "controls", "trade"],
                actors=["usa", "china"],
                regions=["east-asia", "united-states"],
                field_scores={
                    "technology": 6.5,
                    "diplomacy": 4.9,
                    "economy": 1.0,
                    "conflict": 0.0,
                },
            )
            diffuse_three_field_event = NormalizedEvent(
                event_id="evt-fa-diffuse-three-field",
                source="fixture:test",
                title="Shared field structure",
                summary="Shared field structure summary.",
                link="https://example.com/fa-diffuse-three-field",
                published_at="2026-03-22T12:08:35Z",
                keywords=["chip", "cyber", "controls", "trade"],
                actors=["usa", "china"],
                regions=["east-asia", "united-states"],
                field_scores={
                    "technology": 6.5,
                    "diplomacy": 4.9,
                    "economy": 4.8,
                    "conflict": 0.0,
                },
            )

            clearer_scored = score_event(clearer_two_field_event)
            diffuse_scored = score_event(diffuse_three_field_event)

            self.assertEqual(clearer_scored.impact_score, diffuse_scored.impact_score)
            self.assertGreaterEqual(
                clearer_scored.field_attraction - diffuse_scored.field_attraction,
                0.25,
            )
            self.assertGreater(
                clearer_scored.divergence_score, diffuse_scored.divergence_score
            )

        def test_score_event_diffuse_third_field_penalty_starts_above_threshold(
            self,
        ) -> None:
            threshold_third_field_event = NormalizedEvent(
                event_id="evt-diffuse-third-threshold",
                source="fixture:test",
                title="Shared field structure",
                summary="Shared field structure summary.",
                link="https://example.com/diffuse-third-threshold",
                published_at="2026-03-22T12:08:36Z",
                keywords=["chip", "cyber", "controls", "trade"],
                actors=["usa", "china"],
                regions=["east-asia", "united-states"],
                field_scores={
                    "technology": 6.5,
                    "diplomacy": 5.4,
                    "economy": 2.5,
                    "conflict": 0.13,
                },
            )
            above_threshold_third_field_event = NormalizedEvent(
                event_id="evt-diffuse-third-above-threshold",
                source="fixture:test",
                title="Shared field structure",
                summary="Shared field structure summary.",
                link="https://example.com/diffuse-third-above-threshold",
                published_at="2026-03-22T12:08:37Z",
                keywords=["chip", "cyber", "controls", "trade"],
                actors=["usa", "china"],
                regions=["east-asia", "united-states"],
                field_scores={
                    "technology": 6.5,
                    "diplomacy": 5.4,
                    "economy": 2.62,
                    "conflict": 0.01,
                },
            )

            threshold_scored = score_event(threshold_third_field_event)
            above_threshold_scored = score_event(above_threshold_third_field_event)

            self.assertEqual(
                threshold_scored.impact_score, above_threshold_scored.impact_score
            )
            self.assertGreater(
                threshold_scored.field_attraction, above_threshold_scored.field_attraction
            )
            self.assertGreater(
                threshold_scored.divergence_score, above_threshold_scored.divergence_score
            )

        def test_score_event_applies_actor_weight_inside_im(self) -> None:
            baseline_event = NormalizedEvent(
                event_id="evt-actor-baseline",
                source="fixture:test",
                title="Neutral update",
                summary="Neutral summary language.",
                link="https://example.com/actor-baseline",
                published_at="2026-03-22T12:09:00Z",
                keywords=["neutral", "update", "brief"],
                actors=["observer"],
                regions=["unknown-region"],
                field_scores={
                    "technology": 4.0,
                    "diplomacy": 0.0,
                    "economy": 0.0,
                    "conflict": 0.0,
                },
            )
            weighted_actor_event = NormalizedEvent(
                event_id="evt-actor-weighted",
                source="fixture:test",
                title="Neutral update",
                summary="Neutral summary language.",
                link="https://example.com/actor-weighted",
                published_at="2026-03-22T12:10:00Z",
                keywords=["neutral", "update", "brief"],
                actors=["usa"],
                regions=["unknown-region"],
                field_scores={
                    "technology": 4.0,
                    "diplomacy": 0.0,
                    "economy": 0.0,
                    "conflict": 0.0,
                },
            )

            baseline_scored = score_event(baseline_event)
            weighted_scored = score_event(weighted_actor_event)

            self.assertEqual(
                baseline_scored.field_attraction, weighted_scored.field_attraction
            )
            self.assertAlmostEqual(
                weighted_scored.impact_score - baseline_scored.impact_score, 0.9, places=2
            )

        def test_score_event_applies_region_weight_inside_im(self) -> None:
            baseline_event = NormalizedEvent(
                event_id="evt-region-baseline",
                source="fixture:test",
                title="Neutral update",
                summary="Neutral summary language.",
                link="https://example.com/region-baseline",
                published_at="2026-03-22T12:11:00Z",
                keywords=["neutral", "update", "brief"],
                actors=["observer"],
                regions=["unknown-region"],
                field_scores={
                    "technology": 4.0,
                    "diplomacy": 0.0,
                    "economy": 0.0,
                    "conflict": 0.0,
                },
            )
            weighted_region_event = NormalizedEvent(
                event_id="evt-region-weighted",
                source="fixture:test",
                title="Neutral update",
                summary="Neutral summary language.",
                link="https://example.com/region-weighted",
                published_at="2026-03-22T12:12:00Z",
                keywords=["neutral", "update", "brief"],
                actors=["observer"],
                regions=["east-asia"],
                field_scores={
                    "technology": 4.0,
                    "diplomacy": 0.0,
                    "economy": 0.0,
                    "conflict": 0.0,
                },
            )

            baseline_scored = score_event(baseline_event)
            weighted_scored = score_event(weighted_region_event)

            self.assertEqual(
                baseline_scored.field_attraction, weighted_scored.field_attraction
            )
            self.assertAlmostEqual(
                weighted_scored.impact_score - baseline_scored.impact_score, 1.5, places=2
            )

        def test_score_event_caps_raw_keyword_density_inside_im(self) -> None:
            over_cap_keywords = [
                "alpha",
                "beta",
                "gamma",
                "delta",
                "epsilon",
                "zeta",
                "eta",
                "theta",
                "iota",
                "kappa",
                "lambda",
                "mu",
            ]
            much_more_keywords = over_cap_keywords + [
                "nu",
                "xi",
                "omicron",
                "pi",
                "rho",
                "sigma",
            ]
            capped_event = NormalizedEvent(
                event_id="evt-keyword-cap-1",
                source="fixture:test",
                title="Neutral update",
                summary="Neutral summary language.",
                link="https://example.com/keyword-cap-1",
                published_at="2026-03-22T12:13:00Z",
                keywords=over_cap_keywords,
                actors=["observer"],
                regions=["unknown-region"],
                field_scores={
                    "technology": 4.0,
                    "diplomacy": 0.0,
                    "economy": 0.0,
                    "conflict": 0.0,
                },
            )
            still_capped_event = NormalizedEvent(
                event_id="evt-keyword-cap-2",
                source="fixture:test",
                title="Neutral update",
                summary="Neutral summary language.",
                link="https://example.com/keyword-cap-2",
                published_at="2026-03-22T12:14:00Z",
                keywords=much_more_keywords,
                actors=["observer"],
                regions=["unknown-region"],
                field_scores={
                    "technology": 4.0,
                    "diplomacy": 0.0,
                    "economy": 0.0,
                    "conflict": 0.0,
                },
            )

            capped_scored = score_event(capped_event)
            still_capped_scored = score_event(still_capped_event)

            self.assertEqual(
                capped_scored.field_attraction, still_capped_scored.field_attraction
            )
            self.assertEqual(capped_scored.impact_score, still_capped_scored.impact_score)

        def test_score_event_applies_dominant_field_strength_inside_im(self) -> None:
            lower_strength_event = NormalizedEvent(
                event_id="evt-strength-low",
                source="fixture:test",
                title="Neutral update",
                summary="Neutral summary language.",
                link="https://example.com/strength-low",
                published_at="2026-03-22T12:15:00Z",
                keywords=["neutral", "update", "brief"],
                actors=["observer"],
                regions=["unknown-region"],
                field_scores={
                    "technology": 4.0,
                    "diplomacy": 0.5,
                    "economy": 0.0,
                    "conflict": 0.0,
                },
            )
            higher_strength_event = NormalizedEvent(
                event_id="evt-strength-high",
                source="fixture:test",
                title="Neutral update",
                summary="Neutral summary language.",
                link="https://example.com/strength-high",
                published_at="2026-03-22T12:16:00Z",
                keywords=["neutral", "update", "brief"],
                actors=["observer"],
                regions=["unknown-region"],
                field_scores={
                    "technology": 6.0,
                    "diplomacy": 0.5,
                    "economy": 0.0,
                    "conflict": 0.0,
                },
            )

            lower_scored = score_event(lower_strength_event)
            higher_scored = score_event(higher_strength_event)

            self.assertAlmostEqual(
                higher_scored.impact_score - lower_scored.impact_score,
                0.5,
                places=2,
            )

        def test_score_event_ignores_subthreshold_field_noise_inside_im(self) -> None:
            baseline_event = NormalizedEvent(
                event_id="evt-nonzero-baseline",
                source="fixture:test",
                title="Neutral update",
                summary="Neutral summary language.",
                link="https://example.com/nonzero-baseline",
                published_at="2026-03-22T12:17:00Z",
                keywords=["neutral", "update", "brief"],
                actors=["observer"],
                regions=["unknown-region"],
                field_scores={
                    "technology": 4.0,
                    "diplomacy": 0.0,
                    "economy": 0.0,
                    "conflict": 0.0,
                },
            )
            noisy_extra_field_event = NormalizedEvent(
                event_id="evt-nonzero-noisy",
                source="fixture:test",
                title="Neutral update",
                summary="Neutral summary language.",
                link="https://example.com/nonzero-noisy",
                published_at="2026-03-22T12:18:00Z",
                keywords=["neutral", "update", "brief"],
                actors=["observer"],
                regions=["unknown-region"],
                field_scores={
                    "technology": 4.0,
                    "diplomacy": 0.5,
                    "economy": 0.3,
                    "conflict": 0.0,
                },
            )

            baseline_scored = score_event(baseline_event)
            noisy_scored = score_event(noisy_extra_field_event)

            self.assertEqual(baseline_scored.impact_score, noisy_scored.impact_score)

        def test_score_event_rewards_meaningful_field_diversity_inside_im(self) -> None:
            baseline_event = NormalizedEvent(
                event_id="evt-nonzero-threshold-baseline",
                source="fixture:test",
                title="Neutral update",
                summary="Neutral summary language.",
                link="https://example.com/nonzero-threshold-baseline",
                published_at="2026-03-22T12:18:30Z",
                keywords=["neutral", "update", "brief"],
                actors=["observer"],
                regions=["unknown-region"],
                field_scores={
                    "technology": 4.0,
                    "diplomacy": 0.0,
                    "economy": 0.0,
                    "conflict": 0.0,
                },
            )
            meaningful_extra_field_event = NormalizedEvent(
                event_id="evt-nonzero-threshold-meaningful",
                source="fixture:test",
                title="Neutral update",
                summary="Neutral summary language.",
                link="https://example.com/nonzero-threshold-meaningful",
                published_at="2026-03-22T12:19:00Z",
                keywords=["neutral", "update", "brief"],
                actors=["observer"],
                regions=["unknown-region"],
                field_scores={
                    "technology": 4.0,
                    "diplomacy": 1.0,
                    "economy": 1.0,
                    "conflict": 0.0,
                },
            )

            baseline_scored = score_event(baseline_event)
            meaningful_scored = score_event(meaningful_extra_field_event)

            self.assertAlmostEqual(
                meaningful_scored.impact_score - baseline_scored.impact_score,
                0.4,
                places=2,
            )

        def test_score_event_applies_text_signal_surface_contributions_inside_im(
            self,
        ) -> None:
            baseline_event = NormalizedEvent(
                event_id="evt-text-signal-baseline",
                source="fixture:test",
                title="Neutral update",
                summary="Neutral summary language.",
                link="https://example.com/text-signal-baseline",
                published_at="2026-03-22T12:19:00Z",
                keywords=["neutral", "update", "brief"],
                actors=["observer"],
                regions=["unknown-region"],
                field_scores={
                    "technology": 4.0,
                    "diplomacy": 0.0,
                    "economy": 0.0,
                    "conflict": 0.0,
                },
            )
            baseline_scored = score_event(baseline_event)

            surface_cases = [
                (
                    "keywords",
                    NormalizedEvent(
                        event_id="evt-text-signal-keywords",
                        source="fixture:test",
                        title="Neutral update",
                        summary="Neutral summary language.",
                        link="https://example.com/text-signal-keywords",
                        published_at="2026-03-22T12:20:00Z",
                        keywords=["neutral", "update", "brief", "chip"],
                        actors=["observer"],
                        regions=["unknown-region"],
                        field_scores={
                            "technology": 4.0,
                            "diplomacy": 0.0,
                            "economy": 0.0,
                            "conflict": 0.0,
                        },
                    ),
                    0.37,
                ),
                (
                    "title",
                    NormalizedEvent(
                        event_id="evt-text-signal-title",
                        source="fixture:test",
                        title="Chip update",
                        summary="Neutral summary language.",
                        link="https://example.com/text-signal-title",
                        published_at="2026-03-22T12:21:00Z",
                        keywords=["neutral", "update", "brief"],
                        actors=["observer"],
                        regions=["unknown-region"],
                        field_scores={
                            "technology": 4.0,
                            "diplomacy": 0.0,
                            "economy": 0.0,
                            "conflict": 0.0,
                        },
                    ),
                    0.2,
                ),
                (
                    "summary",
                    NormalizedEvent(
                        event_id="evt-text-signal-summary",
                        source="fixture:test",
                        title="Neutral update",
                        summary="Chip summary language.",
                        link="https://example.com/text-signal-summary",
                        published_at="2026-03-22T12:22:00Z",
                        keywords=["neutral", "update", "brief"],
                        actors=["observer"],
                        regions=["unknown-region"],
                        field_scores={
                            "technology": 4.0,
                            "diplomacy": 0.0,
                            "economy": 0.0,
                            "conflict": 0.0,
                        },
                    ),
                    0.1,
                ),
            ]

            for surface_name, event, expected_delta in surface_cases:
                with self.subTest(surface=surface_name):
                    scored = score_event(event)
                    self.assertEqual(
                        baseline_scored.field_attraction, scored.field_attraction
                    )
                    self.assertAlmostEqual(
                        scored.impact_score - baseline_scored.impact_score,
                        expected_delta,
                        places=2,
                    )

        def test_score_event_combines_text_signal_surface_contributions_inside_im(
            self,
        ) -> None:
            baseline_event = NormalizedEvent(
                event_id="evt-text-signal-combined-baseline",
                source="fixture:test",
                title="Neutral update",
                summary="Neutral summary language.",
                link="https://example.com/text-signal-combined-baseline",
                published_at="2026-03-22T12:23:00Z",
                keywords=["neutral", "update", "brief"],
                actors=["observer"],
                regions=["unknown-region"],
                field_scores={
                    "technology": 4.0,
                    "diplomacy": 0.0,
                    "economy": 0.0,
                    "conflict": 0.0,
                },
            )
            combined_signal_event = NormalizedEvent(
                event_id="evt-text-signal-combined",
                source="fixture:test",
                title="Chip update",
                summary="Chip summary language.",
                link="https://example.com/text-signal-combined",
                published_at="2026-03-22T12:24:00Z",
                keywords=["neutral", "update", "brief", "chip"],
                actors=["observer"],
                regions=["unknown-region"],
                field_scores={
                    "technology": 4.0,
                    "diplomacy": 0.0,
                    "economy": 0.0,
                    "conflict": 0.0,
                },
            )

            baseline_scored = score_event(baseline_event)
            combined_scored = score_event(combined_signal_event)

            self.assertEqual(
                baseline_scored.field_attraction, combined_scored.field_attraction
            )
            self.assertAlmostEqual(
                combined_scored.impact_score - baseline_scored.impact_score,
                0.67,
                places=2,
            )

        def test_score_event_rewards_stronger_weighted_field_intensity_in_im(self) -> None:
            lower_intensity_event = NormalizedEvent(
                event_id="evt-low-im",
                source="fixture:test",
                title="Moderate technology escalation",
                summary="Moderate event with limited weighted field intensity.",
                link="https://example.com/low-im",
                published_at="2026-03-22T12:10:00Z",
                keywords=["chip", "cyber", "talks", "tariff"],
                actors=["usa", "china"],
                regions=["east-asia", "united-states"],
                field_scores={
                    "technology": 4.0,
                    "diplomacy": 0.0,
                    "economy": 0.0,
                    "conflict": 0.0,
                },
            )
            higher_intensity_event = NormalizedEvent(
                event_id="evt-high-im",
                source="fixture:test",
                title="Severe technology escalation",
                summary="Severe event with stronger weighted field intensity.",
                link="https://example.com/high-im",
                published_at="2026-03-22T12:15:00Z",
                keywords=["chip", "cyber", "talks", "tariff"],
                actors=["usa", "china"],
                regions=["east-asia", "united-states"],
                field_scores={
                    "technology": 4.0,
                    "diplomacy": 3.5,
                    "economy": 3.0,
                    "conflict": 0.0,
                },
            )

            lower_scored = score_event(lower_intensity_event)
            higher_scored = score_event(higher_intensity_event)

            self.assertGreater(higher_scored.impact_score, lower_scored.impact_score)

        def test_score_event_rewards_stronger_text_signal_intensity_in_im(self) -> None:
            weaker_text_event = NormalizedEvent(
                event_id="evt-weak-text",
                source="fixture:test",
                title="Technology policy update",
                summary="Officials discuss export policy changes and regional planning.",
                link="https://example.com/weak-text",
                published_at="2026-03-22T12:20:00Z",
                keywords=[
                    "technology",
                    "policy",
                    "export",
                    "changes",
                    "regional",
                    "planning",
                ],
                actors=["usa", "china"],
                regions=["east-asia", "united-states"],
                field_scores={
                    "technology": 6.5,
                    "diplomacy": 2.0,
                    "economy": 1.5,
                    "conflict": 0.0,
                },
            )
            stronger_text_event = NormalizedEvent(
                event_id="evt-strong-text",
                source="fixture:test",
                title="Chip and cyber controls tighten in technology dispute",
                summary="Officials expand chip controls and cyber restrictions after satellite concerns.",
                link="https://example.com/strong-text",
                published_at="2026-03-22T12:25:00Z",
                keywords=[
                    "chip",
                    "cyber",
                    "controls",
                    "satellite",
                    "restrictions",
                    "dispute",
                ],
                actors=["usa", "china"],
                regions=["east-asia", "united-states"],
                field_scores={
                    "technology": 6.5,
                    "diplomacy": 2.0,
                    "economy": 1.5,
                    "conflict": 0.0,
                },
            )

            weaker_scored = score_event(weaker_text_event)
            stronger_scored = score_event(stronger_text_event)

            self.assertGreater(stronger_scored.impact_score, weaker_scored.impact_score)
            self.assertEqual(
                stronger_scored.field_attraction, weaker_scored.field_attraction
            )

        def test_score_event_text_signal_intensity_does_not_reward_generic_keyword_mass(
            self,
        ) -> None:
            generic_token_event = NormalizedEvent(
                event_id="evt-generic-text",
                source="fixture:test",
                title="International policy developments remain under discussion",
                summary="Officials review committee process updates, planning notes, and general strategy language.",
                link="https://example.com/generic-text",
                published_at="2026-03-22T12:30:00Z",
                keywords=[
                    "international",
                    "policy",
                    "developments",
                    "discussion",
                    "committee",
                    "strategy",
                ],
                actors=["usa", "china"],
                regions=["east-asia", "united-states"],
                field_scores={
                    "technology": 6.5,
                    "diplomacy": 2.0,
                    "economy": 1.5,
                    "conflict": 0.0,
                },
            )
            branch_relevant_event = NormalizedEvent(
                event_id="evt-branch-text",
                source="fixture:test",
                title="AI chip and cyber dispute intensifies",
                summary="Officials review chip controls, cyber restrictions, and satellite exposure.",
                link="https://example.com/branch-text",
                published_at="2026-03-22T12:35:00Z",
                keywords=["ai", "chip", "cyber", "satellite", "controls", "restrictions"],
                actors=["usa", "china"],
                regions=["east-asia", "united-states"],
                field_scores={
                    "technology": 6.5,
                    "diplomacy": 2.0,
                    "economy": 1.5,
                    "conflict": 0.0,
                },
            )

            generic_scored = score_event(generic_token_event)
            branch_scored = score_event(branch_relevant_event)

            self.assertGreater(branch_scored.impact_score, generic_scored.impact_score)
            self.assertEqual(
                branch_scored.field_attraction, generic_scored.field_attraction
            )

        def test_score_event_text_signal_intensity_respects_cap(self) -> None:
            strong_text_event = NormalizedEvent(
                event_id="evt-strong-cap",
                source="fixture:test",
                title="AI chip cyber satellite controls escalate",
                summary="Officials review ai chip cyber satellite controls after new alerts.",
                link="https://example.com/strong-cap",
                published_at="2026-03-22T12:40:00Z",
                keywords=["ai", "chip", "cyber", "satellite", "controls", "alerts"],
                actors=["usa", "china"],
                regions=["east-asia", "united-states"],
                field_scores={
                    "technology": 6.5,
                    "diplomacy": 2.0,
                    "economy": 1.5,
                    "conflict": 0.0,
                },
            )
            exaggerated_text_event = NormalizedEvent(
                event_id="evt-exaggerated-cap",
                source="fixture:test",
                title="AI chip cyber satellite controls escalate with ai chip cyber satellite focus",
                summary="Officials review ai chip cyber satellite controls after ai chip cyber satellite alerts and ai chip cyber satellite exposure.",
                link="https://example.com/exaggerated-cap",
                published_at="2026-03-22T12:45:00Z",
                keywords=["ai", "chip", "cyber", "satellite", "controls", "exposure"],
                actors=["usa", "china"],
                regions=["east-asia", "united-states"],
                field_scores={
                    "technology": 6.5,
                    "diplomacy": 2.0,
                    "economy": 1.5,
                    "conflict": 0.0,
                },
            )

            strong_scored = score_event(strong_text_event)
            exaggerated_scored = score_event(exaggerated_text_event)

            self.assertEqual(strong_scored.impact_score, exaggerated_scored.impact_score)
            self.assertEqual(
                strong_scored.field_attraction, exaggerated_scored.field_attraction
            )

        def test_score_event_text_signal_intensity_ignores_incidental_substrings(
            self,
        ) -> None:
            neutral_text_event = NormalizedEvent(
                event_id="evt-neutral-text",
                source="fixture:test",
                title="Regional relief planning continues",
                summary="Officials discuss corridor planning and funding updates.",
                link="https://example.com/neutral-text",
                published_at="2026-03-22T12:50:00Z",
                keywords=[
                    "regional",
                    "relief",
                    "planning",
                    "corridor",
                    "funding",
                    "updates",
                ],
                actors=["usa", "china"],
                regions=["east-asia", "united-states"],
                field_scores={
                    "technology": 6.5,
                    "diplomacy": 2.0,
                    "economy": 1.5,
                    "conflict": 0.0,
                },
            )
            incidental_substring_event = NormalizedEvent(
                event_id="evt-incidental-text",
                source="fixture:test",
                title="Air aid planning continues",
                summary="Officials discuss air aid corridors and fair funding updates.",
                link="https://example.com/incidental-text",
                published_at="2026-03-22T12:55:00Z",
                keywords=[
                    "regional",
                    "relief",
                    "planning",
                    "corridor",
                    "funding",
                    "updates",
                ],
                actors=["usa", "china"],
                regions=["east-asia", "united-states"],
                field_scores={
                    "technology": 6.5,
                    "diplomacy": 2.0,
                    "economy": 1.5,
                    "conflict": 0.0,
                },
            )

            neutral_scored = score_event(neutral_text_event)
            incidental_scored = score_event(incidental_substring_event)

            self.assertEqual(incidental_scored.impact_score, neutral_scored.impact_score)
            self.assertEqual(
                incidental_scored.field_attraction, neutral_scored.field_attraction
            )

        def test_score_event_text_signal_intensity_matches_punctuation_adjacent_cues(
            self,
        ) -> None:
            plain_text_event = NormalizedEvent(
                event_id="evt-plain-text",
                source="fixture:test",
                title="Policy update remains under review",
                summary="Officials discuss restrictions and oversight planning.",
                link="https://example.com/plain-text",
                published_at="2026-03-22T13:00:00Z",
                keywords=[
                    "policy",
                    "update",
                    "review",
                    "restrictions",
                    "oversight",
                    "planning",
                ],
                actors=["usa", "china"],
                regions=["east-asia", "united-states"],
                field_scores={
                    "technology": 6.5,
                    "diplomacy": 2.0,
                    "economy": 1.5,
                    "conflict": 0.0,
                },
            )
            punctuated_cue_event = NormalizedEvent(
                event_id="evt-punctuated-text",
                source="fixture:test",
                title="AI-driven chip, cyber, and satellite controls tighten",
                summary="Officials review chip, cyber, and satellite restrictions after new alerts.",
                link="https://example.com/punctuated-text",
                published_at="2026-03-22T13:05:00Z",
                keywords=[
                    "policy",
                    "update",
                    "review",
                    "restrictions",
                    "oversight",
                    "planning",
                ],
                actors=["usa", "china"],
                regions=["east-asia", "united-states"],
                field_scores={
                    "technology": 6.5,
                    "diplomacy": 2.0,
                    "economy": 1.5,
                    "conflict": 0.0,
                },
            )

            plain_scored = score_event(plain_text_event)
            punctuated_scored = score_event(punctuated_cue_event)

            self.assertGreater(punctuated_scored.impact_score, plain_scored.impact_score)
            self.assertEqual(
                punctuated_scored.field_attraction, plain_scored.field_attraction
            )
