from support import *


class GroupingTests(unittest.TestCase):
        def test_group_events_clusters_obviously_related_events(self) -> None:
            related_a = ScoredEvent(
                event_id="evt-a",
                title="China and USA expand chip controls",
                source="fixture:test",
                link="https://example.com/a",
                published_at="2026-03-22T08:00:00Z",
                actors=["china", "usa"],
                regions=["east-asia", "united-states"],
                keywords=["chip", "controls", "export", "dispute"],
                dominant_field="technology",
                impact_score=14.03,
                field_attraction=7.75,
                divergence_score=19.58,
                rationale=["Im=14.03", "Fa=7.75"],
            )
            related_b = ScoredEvent(
                event_id="evt-b",
                title="USA and China deepen export chip restrictions",
                source="fixture:test",
                link="https://example.com/b",
                published_at="2026-03-22T09:00:00Z",
                actors=["usa", "china"],
                regions=["east-asia", "united-states"],
                keywords=["chip", "restrictions", "export", "controls"],
                dominant_field="technology",
                impact_score=13.5,
                field_attraction=7.1,
                divergence_score=18.31,
                rationale=["Im=13.5", "Fa=7.1"],
            )
            unrelated = ScoredEvent(
                event_id="evt-c",
                title="Iran diplomacy channel reopens",
                source="fixture:test",
                link="https://example.com/c",
                published_at="2026-03-22T10:00:00Z",
                actors=["iran"],
                regions=["middle-east"],
                keywords=["talks", "diplomacy", "channel", "iran"],
                dominant_field="diplomacy",
                impact_score=11.67,
                field_attraction=6.17,
                divergence_score=15.92,
                rationale=["Im=11.67", "Fa=6.17"],
            )

            groups: list[EventGroupSummary] = pipeline_module.group_events(
                [related_a, related_b, unrelated]
            )

            self.assertEqual(len(groups), 1)
            self.assertEqual(groups[0]["headline_event_id"], "evt-a")
            self.assertEqual(
                groups[0]["headline_title"], "China and USA expand chip controls"
            )
            self.assertEqual(groups[0]["member_event_ids"], ["evt-a", "evt-b"])
            self.assertEqual(groups[0]["shared_keywords"], ["chip", "controls", "export"])
            self.assertEqual(groups[0]["dominant_field"], "technology")
            self.assertEqual(groups[0]["shared_actors"], ["china", "usa"])
            self.assertEqual(groups[0]["shared_regions"], ["east-asia", "united-states"])
            self.assertEqual(len(groups[0]["evidence_chain"]), 1)
            self.assertEqual(groups[0]["evidence_chain"][0]["from_event_id"], "evt-a")
            self.assertEqual(groups[0]["evidence_chain"][0]["to_event_id"], "evt-b")
            self.assertEqual(
                groups[0]["evidence_chain"][0]["relationship"], "capability-race"
            )
            self.assertEqual(groups[0]["evidence_chain"][0]["shared_signal_count"], 7)
            self.assertEqual(groups[0]["evidence_chain"][0]["time_delta_hours"], 1.0)
            self.assertEqual(groups[0]["causal_ordered_event_ids"], ["evt-a", "evt-b"])
            self.assertEqual(groups[0]["causal_span_hours"], 1.0)
            self.assertIn(
                "2 related technology events reinforce", groups[0]["chain_summary"]
            )
            self.assertIn("chip, controls, export", groups[0]["chain_summary"])
            self.assertIn("capability-race cluster", groups[0]["causal_summary"])

        def test_group_events_do_not_cluster_distant_related_events(self) -> None:
            early_event = ScoredEvent(
                event_id="evt-early",
                title="China and USA expand chip controls",
                source="fixture:test",
                link="https://example.com/early",
                published_at="2026-03-22T08:00:00Z",
                actors=["china", "usa"],
                regions=["east-asia", "united-states"],
                keywords=["chip", "controls", "export", "dispute"],
                dominant_field="technology",
                impact_score=14.03,
                field_attraction=7.75,
                divergence_score=19.58,
                rationale=["Im=14.03", "Fa=7.75"],
            )
            late_event = ScoredEvent(
                event_id="evt-late",
                title="USA and China deepen export chip restrictions",
                source="fixture:test",
                link="https://example.com/late",
                published_at="2026-03-25T08:00:00Z",
                actors=["usa", "china"],
                regions=["east-asia", "united-states"],
                keywords=["chip", "restrictions", "export", "controls"],
                dominant_field="technology",
                impact_score=13.5,
                field_attraction=7.1,
                divergence_score=18.31,
                rationale=["Im=13.5", "Fa=7.1"],
            )

            groups: list[EventGroupSummary] = pipeline_module.group_events(
                [early_event, late_event]
            )

            self.assertEqual(groups, [])

        def test_group_events_allow_missing_timestamp_fallback(self) -> None:
            unknown_time_a = ScoredEvent(
                event_id="evt-a",
                title="China and USA expand chip controls",
                source="fixture:test",
                link="https://example.com/a",
                published_at=None,
                actors=["china", "usa"],
                regions=["east-asia", "united-states"],
                keywords=["chip", "controls", "export", "dispute"],
                dominant_field="technology",
                impact_score=14.03,
                field_attraction=7.75,
                divergence_score=19.58,
                rationale=["Im=14.03", "Fa=7.75"],
            )
            unknown_time_b = ScoredEvent(
                event_id="evt-b",
                title="USA and China deepen export chip restrictions",
                source="fixture:test",
                link="https://example.com/b",
                published_at=None,
                actors=["usa", "china"],
                regions=["east-asia", "united-states"],
                keywords=["chip", "restrictions", "export", "controls"],
                dominant_field="technology",
                impact_score=13.5,
                field_attraction=7.1,
                divergence_score=18.31,
                rationale=["Im=13.5", "Fa=7.1"],
            )

            groups: list[EventGroupSummary] = pipeline_module.group_events(
                [unknown_time_a, unknown_time_b]
            )

            self.assertEqual(len(groups), 1)
            self.assertIsNone(groups[0]["causal_span_hours"])
            self.assertIn("across 2 events.", groups[0]["causal_summary"])
            self.assertNotIn(" over ", groups[0]["causal_summary"])

        def test_group_events_compute_causal_span_from_known_timestamps(self) -> None:
            known_early = ScoredEvent(
                event_id="evt-a",
                title="China and USA expand chip controls",
                source="fixture:test",
                link="https://example.com/a",
                published_at="2026-03-22T08:00:00Z",
                actors=["china", "usa"],
                regions=["east-asia", "united-states"],
                keywords=["chip", "controls", "export", "dispute"],
                dominant_field="technology",
                impact_score=14.03,
                field_attraction=7.75,
                divergence_score=19.58,
                rationale=["Im=14.03", "Fa=7.75"],
            )
            unknown_time = ScoredEvent(
                event_id="evt-b",
                title="USA broadens export controls after chip dispute",
                source="fixture:test",
                link="https://example.com/b",
                published_at=None,
                actors=["usa", "china"],
                regions=["east-asia", "united-states"],
                keywords=["chip", "controls", "tariff", "sanctions"],
                dominant_field="technology",
                impact_score=13.5,
                field_attraction=7.1,
                divergence_score=18.31,
                rationale=["Im=13.5", "Fa=7.1"],
            )
            known_late = ScoredEvent(
                event_id="evt-c",
                title="USA widens chip tariff controls after export review",
                source="fixture:test",
                link="https://example.com/c",
                published_at="2026-03-22T10:00:00Z",
                actors=["usa", "china"],
                regions=["east-asia", "united-states"],
                keywords=["tariff", "sanctions", "controls", "review"],
                dominant_field="technology",
                impact_score=12.9,
                field_attraction=6.95,
                divergence_score=17.82,
                rationale=["Im=12.9", "Fa=6.95"],
            )

            groups: list[EventGroupSummary] = pipeline_module.group_events(
                [known_early, unknown_time, known_late]
            )

            self.assertEqual(len(groups), 1)
            self.assertEqual(groups[0]["causal_span_hours"], 2.0)
            self.assertIn(" over 2.0h", groups[0]["causal_summary"])

        def test_group_events_support_transitive_causal_clustering(self) -> None:
            anchor = ScoredEvent(
                event_id="evt-a",
                title="China expands chip controls",
                source="fixture:test",
                link="https://example.com/a",
                published_at="2026-03-22T08:00:00Z",
                actors=["china", "usa"],
                regions=["east-asia", "united-states"],
                keywords=["chip", "controls", "export", "dispute"],
                dominant_field="technology",
                impact_score=14.03,
                field_attraction=7.75,
                divergence_score=19.58,
                rationale=["Im=14.03", "Fa=7.75"],
            )
            bridge = ScoredEvent(
                event_id="evt-b",
                title="USA broadens export controls after chip dispute",
                source="fixture:test",
                link="https://example.com/b",
                published_at="2026-03-22T10:00:00Z",
                actors=["usa", "china"],
                regions=["east-asia", "united-states"],
                keywords=["chip", "controls", "tariff", "sanctions"],
                dominant_field="technology",
                impact_score=13.5,
                field_attraction=7.1,
                divergence_score=18.31,
                rationale=["Im=13.5", "Fa=7.1"],
            )
            transitive = ScoredEvent(
                event_id="evt-c",
                title="USA widens chip tariff controls after export review",
                source="fixture:test",
                link="https://example.com/c",
                published_at="2026-03-22T09:00:00Z",
                actors=["usa", "china"],
                regions=["east-asia", "united-states"],
                keywords=["tariff", "sanctions", "controls", "review"],
                dominant_field="technology",
                impact_score=12.9,
                field_attraction=6.95,
                divergence_score=17.82,
                rationale=["Im=12.9", "Fa=6.95"],
            )

            groups: list[EventGroupSummary] = pipeline_module.group_events(
                [anchor, bridge, transitive]
            )

            self.assertEqual(len(groups), 1)
            self.assertEqual(groups[0]["member_event_ids"], ["evt-a", "evt-b", "evt-c"])
            self.assertEqual(
                groups[0]["causal_ordered_event_ids"], ["evt-a", "evt-b", "evt-c"]
            )
            self.assertEqual(len(groups[0]["evidence_chain"]), 2)
            self.assertEqual(groups[0]["evidence_chain"][0]["from_event_id"], "evt-a")
            self.assertEqual(groups[0]["evidence_chain"][0]["to_event_id"], "evt-b")
            self.assertEqual(groups[0]["evidence_chain"][1]["from_event_id"], "evt-b")
            self.assertEqual(groups[0]["evidence_chain"][1]["to_event_id"], "evt-c")
            self.assertEqual(groups[0]["causal_span_hours"], 2.0)
            self.assertEqual(groups[0]["evidence_chain"][1]["time_delta_hours"], 1.0)
            self.assertIn("across 3 events", groups[0]["causal_summary"])

        def test_pipeline_surfaces_event_groups_in_scenario_summary(self) -> None:
            artifact = run_pipeline(
                fixture_paths=[str(FIXTURE_PATH)],
                fetch=False,
                source_urls=[],
                output_path=None,
            )

            self.assertIn("event_groups", artifact.scenario_summary)
            self.assertIsInstance(artifact.scenario_summary["event_groups"], list)
            for group in artifact.scenario_summary["event_groups"]:
                self.assertIn("headline_title", group)
                self.assertIn("shared_keywords", group)

        def test_backtrack_candidates_collapse_grouped_duplicate_events(self) -> None:
            grouped_a = ScoredEvent(
                event_id="evt-a",
                title="China and USA expand chip controls",
                source="fixture:test",
                link="https://example.com/a",
                published_at="2026-03-22T08:00:00Z",
                actors=["china", "usa"],
                regions=["east-asia", "united-states"],
                keywords=["chip", "controls", "export", "dispute"],
                dominant_field="technology",
                impact_score=14.03,
                field_attraction=7.75,
                divergence_score=19.58,
                rationale=["Im=14.03", "Fa=7.75"],
            )
            grouped_b = ScoredEvent(
                event_id="evt-b",
                title="USA and China deepen export chip restrictions",
                source="fixture:test",
                link="https://example.com/b",
                published_at="2026-03-22T09:00:00Z",
                actors=["usa", "china"],
                regions=["east-asia", "united-states"],
                keywords=["chip", "restrictions", "export", "controls"],
                dominant_field="technology",
                impact_score=13.5,
                field_attraction=7.1,
                divergence_score=18.31,
                rationale=["Im=13.5", "Fa=7.1"],
            )
            unrelated = ScoredEvent(
                event_id="evt-c",
                title="Iran diplomacy channel reopens",
                source="fixture:test",
                link="https://example.com/c",
                published_at="2026-03-22T10:00:00Z",
                actors=["iran"],
                regions=["middle-east"],
                keywords=["talks", "diplomacy", "channel", "iran"],
                dominant_field="diplomacy",
                impact_score=11.67,
                field_attraction=6.17,
                divergence_score=15.92,
                rationale=["Im=11.67", "Fa=6.17"],
            )
            groups: list[EventGroupSummary] = pipeline_module.group_events(
                [grouped_a, grouped_b, unrelated]
            )

            candidates = backtrack_candidates(
                [grouped_a, grouped_b, unrelated],
                event_groups=groups,
            )

            self.assertEqual(len(candidates), 2)
            self.assertEqual(candidates[0].event_id, "evt-a")
            self.assertEqual(candidates[1].event_id, "evt-c")
            self.assertIn("Evidence chain:", candidates[0].reason)
            self.assertIn("2 related technology events reinforce", candidates[0].reason)
            self.assertIn("Causal cluster:", candidates[0].reason)
            self.assertNotIn("Evidence chain:", candidates[1].reason)

        def test_pipeline_reduces_duplicate_interventions_for_grouped_events(self) -> None:
            fixture_a = ScoredEvent(
                event_id="evt-a",
                title="China and USA expand chip controls",
                source="fixture:test",
                link="https://example.com/a",
                published_at="2026-03-22T08:00:00Z",
                actors=["china", "usa"],
                regions=["east-asia", "united-states"],
                keywords=["chip", "controls", "export", "dispute"],
                dominant_field="technology",
                impact_score=14.03,
                field_attraction=7.75,
                divergence_score=19.58,
                rationale=["Im=14.03", "Fa=7.75"],
            )
            fixture_b = ScoredEvent(
                event_id="evt-b",
                title="USA and China deepen export chip restrictions",
                source="fixture:test",
                link="https://example.com/b",
                published_at="2026-03-22T09:00:00Z",
                actors=["usa", "china"],
                regions=["east-asia", "united-states"],
                keywords=["chip", "restrictions", "export", "controls"],
                dominant_field="technology",
                impact_score=13.5,
                field_attraction=7.1,
                divergence_score=18.31,
                rationale=["Im=13.5", "Fa=7.1"],
            )
            unrelated = ScoredEvent(
                event_id="evt-c",
                title="Iran diplomacy channel reopens",
                source="fixture:test",
                link="https://example.com/c",
                published_at="2026-03-22T10:00:00Z",
                actors=["iran"],
                regions=["middle-east"],
                keywords=["talks", "diplomacy", "channel", "iran"],
                dominant_field="diplomacy",
                impact_score=11.67,
                field_attraction=6.17,
                divergence_score=15.92,
                rationale=["Im=11.67", "Fa=6.17"],
            )
            groups: list[EventGroupSummary] = pipeline_module.group_events(
                [fixture_a, fixture_b, unrelated]
            )

            candidates = backtrack_candidates(
                [fixture_a, fixture_b, unrelated],
                event_groups=groups,
            )

            self.assertEqual(
                [candidate.event_id for candidate in candidates], ["evt-a", "evt-c"]
            )
