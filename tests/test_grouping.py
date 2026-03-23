from support import *


class GroupingTests(unittest.TestCase):
    def make_group_event(
        self,
        *,
        event_id: str,
        title: str,
        published_at: str | None,
        keywords: list[str],
        dominant_field: str,
        actors: list[str],
        regions: list[str],
        divergence_score: float,
    ) -> ScoredEvent:
        return ScoredEvent(
            event_id=event_id,
            title=title,
            source="fixture:test",
            link=f"https://example.com/{event_id}",
            published_at=published_at,
            actors=actors,
            regions=regions,
            keywords=keywords,
            dominant_field=dominant_field,
            impact_score=round(divergence_score - 5.0, 2),
            field_attraction=5.0,
            divergence_score=divergence_score,
            rationale=[f"Im={round(divergence_score - 5.0, 2)}", "Fa=5.0"],
        )

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
        self.assertEqual(candidates[0].target, "china")
        self.assertEqual(candidates[0].intervention_type, "capability-containment")
        self.assertIn(
            "Grouped context: 2-event technology cluster", candidates[0].reason
        )
        self.assertIn("high corroboration across causal links", candidates[0].reason)
        self.assertIn("dominant relationship=capability-race", candidates[0].reason)
        self.assertIn("signal support=7", candidates[0].reason)
        self.assertIn("link tempo=1.0h", candidates[0].reason)
        self.assertIn("shared actors=china, usa", candidates[0].reason)
        self.assertIn(
            "Quickly disrupt the linked cluster before 2 related capability moves harden into a broader race.",
            candidates[0].expected_effect,
        )
        self.assertIn("Evidence chain:", candidates[0].reason)
        self.assertIn("2 related technology events reinforce", candidates[0].reason)
        self.assertIn("Causal cluster:", candidates[0].reason)
        self.assertNotIn("Evidence chain:", candidates[1].reason)

    def test_backtrack_candidates_use_causal_chain_effect_for_larger_groups(
        self,
    ) -> None:
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
        candidates = backtrack_candidates(
            [anchor, bridge, transitive],
            event_groups=groups,
        )

        self.assertEqual([candidate.event_id for candidate in candidates], ["evt-a"])
        self.assertEqual(candidates[0].target, "china")
        self.assertEqual(candidates[0].intervention_type, "capability-freeze")
        self.assertIn("high corroboration across causal links", candidates[0].reason)
        self.assertIn("signal support range=6-7", candidates[0].reason)
        self.assertIn("link tempo range=1.0-2.0h", candidates[0].reason)
        self.assertIn("with 2 causal link(s) over 2.0h", candidates[0].reason)
        self.assertIn(
            "Urgently disrupt the reinforcing chain before 3 related capability moves harden into a broader race.",
            candidates[0].expected_effect,
        )

    def test_backtrack_candidates_keep_missing_span_effect_wording_neutral(
        self,
    ) -> None:
        unknown_time_a = self.make_group_event(
            event_id="evt-a",
            title="China and USA expand chip controls",
            published_at=None,
            keywords=["chip", "controls", "export", "dispute"],
            dominant_field="technology",
            actors=["china", "usa"],
            regions=["east-asia", "united-states"],
            divergence_score=19.58,
        )
        unknown_time_b = self.make_group_event(
            event_id="evt-b",
            title="USA and China deepen export chip restrictions",
            published_at=None,
            keywords=["chip", "restrictions", "export", "controls"],
            dominant_field="technology",
            actors=["usa", "china"],
            regions=["east-asia", "united-states"],
            divergence_score=18.31,
        )

        groups: list[EventGroupSummary] = pipeline_module.group_events(
            [unknown_time_a, unknown_time_b]
        )
        candidates = backtrack_candidates(
            [unknown_time_a, unknown_time_b],
            event_groups=groups,
        )

        self.assertEqual(candidates[0].intervention_type, "capability-containment")
        self.assertEqual(
            candidates[0].expected_effect,
            "Disrupt the linked cluster before 2 related capability moves harden into a broader race.",
        )

    def test_backtrack_candidates_keep_low_signal_three_event_group_on_weak_type(
        self,
    ) -> None:
        anchor = self.make_group_event(
            event_id="evt-a",
            title="Chip dispute review opens export track",
            published_at="2026-03-22T08:00:00Z",
            keywords=["chip", "export", "review", "alpha"],
            dominant_field="technology",
            actors=["china"],
            regions=["east-asia"],
            divergence_score=18.4,
        )
        bridge = self.make_group_event(
            event_id="evt-b",
            title="Export review widens around chip track",
            published_at="2026-03-22T09:00:00Z",
            keywords=["chip", "export", "track", "beta"],
            dominant_field="technology",
            actors=["china"],
            regions=["east-asia"],
            divergence_score=17.8,
        )
        transitive = self.make_group_event(
            event_id="evt-c",
            title="Chip track shifts into export controls review",
            published_at="2026-03-22T10:00:00Z",
            keywords=["chip", "track", "controls", "gamma"],
            dominant_field="technology",
            actors=["china"],
            regions=["east-asia"],
            divergence_score=17.1,
        )

        groups: list[EventGroupSummary] = pipeline_module.group_events(
            [anchor, bridge, transitive]
        )
        self.assertEqual(len(groups), 1)
        self.assertEqual(len(groups[0]["evidence_chain"]), 2)
        self.assertEqual(
            [link["shared_signal_count"] for link in groups[0]["evidence_chain"]],
            [4, 4],
        )

        candidates = backtrack_candidates(
            [anchor, bridge, transitive],
            event_groups=groups,
        )

        self.assertEqual([candidate.event_id for candidate in candidates], ["evt-a"])
        self.assertEqual(candidates[0].intervention_type, "capability-containment")
        self.assertIn(
            "moderate corroboration across causal links", candidates[0].reason
        )
        self.assertIn("dominant relationship=capability-race", candidates[0].reason)
        self.assertIn("signal support=4", candidates[0].reason)

    def test_backtrack_candidates_fall_back_to_shared_region_target_when_group_lacks_shared_actors(
        self,
    ) -> None:
        anchor = ScoredEvent(
            event_id="evt-a",
            title="Port disruption expands across the gulf",
            source="fixture:test",
            link="https://example.com/a",
            published_at="2026-03-22T08:00:00Z",
            actors=["shipping-ministry"],
            regions=["middle-east", "gulf"],
            keywords=["port", "shipping", "corridor", "inspection"],
            dominant_field="economy",
            impact_score=13.4,
            field_attraction=7.1,
            divergence_score=17.9,
            rationale=["Im=13.4", "Fa=7.1"],
        )
        linked = ScoredEvent(
            event_id="evt-b",
            title="Trade corridor inspection delays widen in gulf ports",
            source="fixture:test",
            link="https://example.com/b",
            published_at="2026-03-22T10:00:00Z",
            actors=["port-authority"],
            regions=["middle-east", "gulf"],
            keywords=["port", "shipping", "corridor", "delays"],
            dominant_field="economy",
            impact_score=12.8,
            field_attraction=6.8,
            divergence_score=16.2,
            rationale=["Im=12.8", "Fa=6.8"],
        )

        groups: list[EventGroupSummary] = pipeline_module.group_events([anchor, linked])
        candidates = backtrack_candidates([anchor, linked], event_groups=groups)

        self.assertEqual([candidate.event_id for candidate in candidates], ["evt-a"])
        self.assertEqual(candidates[0].target, "gulf")

    def test_backtrack_candidates_use_strong_conflict_group_type(self) -> None:
        anchor = self.make_group_event(
            event_id="evt-a",
            title="Border strike alert triggers rapid response review",
            published_at="2026-03-22T08:00:00Z",
            keywords=["border", "strike", "alert", "troops"],
            dominant_field="conflict",
            actors=["state-a", "state-b"],
            regions=["frontier"],
            divergence_score=18.9,
        )
        bridge = self.make_group_event(
            event_id="evt-b",
            title="Troop alert expands after border strike review",
            published_at="2026-03-22T09:00:00Z",
            keywords=["border", "strike", "troops", "review"],
            dominant_field="conflict",
            actors=["state-a", "state-b"],
            regions=["frontier"],
            divergence_score=18.1,
        )
        transitive = self.make_group_event(
            event_id="evt-c",
            title="Forward troop review widens after strike warning",
            published_at="2026-03-22T10:00:00Z",
            keywords=["strike", "troops", "review", "warning"],
            dominant_field="conflict",
            actors=["state-a", "state-b"],
            regions=["frontier"],
            divergence_score=17.4,
        )

        groups: list[EventGroupSummary] = pipeline_module.group_events(
            [anchor, bridge, transitive]
        )
        candidates = backtrack_candidates(
            [anchor, bridge, transitive],
            event_groups=groups,
        )

        self.assertEqual(candidates[0].intervention_type, "escalation-override")

    def test_backtrack_candidates_use_weak_conflict_group_type(self) -> None:
        anchor = self.make_group_event(
            event_id="evt-a",
            title="Border alert follows strike exchange",
            published_at="2026-03-22T08:00:00Z",
            keywords=["border", "strike", "alert", "troops"],
            dominant_field="conflict",
            actors=["state-a", "state-b"],
            regions=["frontier"],
            divergence_score=18.9,
        )
        linked = self.make_group_event(
            event_id="evt-b",
            title="Troop review follows renewed border alert",
            published_at="2026-03-22T09:00:00Z",
            keywords=["border", "alert", "troops", "review"],
            dominant_field="conflict",
            actors=["state-a", "state-b"],
            regions=["frontier"],
            divergence_score=18.1,
        )

        groups: list[EventGroupSummary] = pipeline_module.group_events([anchor, linked])
        candidates = backtrack_candidates([anchor, linked], event_groups=groups)

        self.assertEqual(candidates[0].intervention_type, "escalation-containment")

    def test_backtrack_candidates_use_strong_diplomacy_group_type(self) -> None:
        anchor = self.make_group_event(
            event_id="evt-a",
            title="Treaty channel opens after sanctions dispute",
            published_at="2026-03-22T08:00:00Z",
            keywords=["treaty", "channel", "accord", "sanctions"],
            dominant_field="diplomacy",
            actors=["bloc-a", "bloc-b"],
            regions=["europe"],
            divergence_score=17.8,
        )
        bridge = self.make_group_event(
            event_id="evt-b",
            title="Accord review deepens inside treaty channel",
            published_at="2026-03-22T09:00:00Z",
            keywords=["treaty", "accord", "review", "channel"],
            dominant_field="diplomacy",
            actors=["bloc-a", "bloc-b"],
            regions=["europe"],
            divergence_score=17.1,
        )
        transitive = self.make_group_event(
            event_id="evt-c",
            title="Accord review stalls after treaty dispute",
            published_at="2026-03-22T10:00:00Z",
            keywords=["treaty", "accord", "review", "dispute"],
            dominant_field="diplomacy",
            actors=["bloc-a", "bloc-b"],
            regions=["europe"],
            divergence_score=16.6,
        )

        groups: list[EventGroupSummary] = pipeline_module.group_events(
            [anchor, bridge, transitive]
        )
        candidates = backtrack_candidates(
            [anchor, bridge, transitive],
            event_groups=groups,
        )

        self.assertEqual(candidates[0].intervention_type, "treaty-invalidation")

    def test_backtrack_candidates_use_weak_diplomacy_group_type(self) -> None:
        anchor = self.make_group_event(
            event_id="evt-a",
            title="Treaty channel opens after sanctions dispute",
            published_at="2026-03-22T08:00:00Z",
            keywords=["treaty", "channel", "accord", "sanctions"],
            dominant_field="diplomacy",
            actors=["bloc-a", "bloc-b"],
            regions=["europe"],
            divergence_score=17.8,
        )
        linked = self.make_group_event(
            event_id="evt-b",
            title="Accord review deepens inside treaty channel",
            published_at="2026-03-22T09:00:00Z",
            keywords=["treaty", "accord", "review", "channel"],
            dominant_field="diplomacy",
            actors=["bloc-a", "bloc-b"],
            regions=["europe"],
            divergence_score=17.1,
        )

        groups: list[EventGroupSummary] = pipeline_module.group_events([anchor, linked])
        candidates = backtrack_candidates([anchor, linked], event_groups=groups)

        self.assertEqual(candidates[0].intervention_type, "channel-stabilization")

    def test_backtrack_candidates_use_strong_economy_group_type(self) -> None:
        anchor = self.make_group_event(
            event_id="evt-a",
            title="Market shock hits major supply corridor",
            published_at="2026-03-22T08:00:00Z",
            keywords=["market", "supply", "corridor", "shock"],
            dominant_field="economy",
            actors=["trade-ministry", "port-authority"],
            regions=["gulf"],
            divergence_score=18.3,
        )
        bridge = self.make_group_event(
            event_id="evt-b",
            title="Supply shock widens across market corridor",
            published_at="2026-03-22T09:00:00Z",
            keywords=["market", "supply", "shock", "corridor"],
            dominant_field="economy",
            actors=["trade-ministry", "port-authority"],
            regions=["gulf"],
            divergence_score=17.6,
        )
        transitive = self.make_group_event(
            event_id="evt-c",
            title="Market review warns of prolonged supply shock",
            published_at="2026-03-22T10:00:00Z",
            keywords=["market", "supply", "shock", "review"],
            dominant_field="economy",
            actors=["trade-ministry", "port-authority"],
            regions=["gulf"],
            divergence_score=16.9,
        )

        groups: list[EventGroupSummary] = pipeline_module.group_events(
            [anchor, bridge, transitive]
        )
        candidates = backtrack_candidates(
            [anchor, bridge, transitive],
            event_groups=groups,
        )

        self.assertEqual(candidates[0].intervention_type, "market-freeze")

    def test_backtrack_candidates_use_weak_economy_group_type(self) -> None:
        anchor = self.make_group_event(
            event_id="evt-a",
            title="Market shock hits major supply corridor",
            published_at="2026-03-22T08:00:00Z",
            keywords=["market", "supply", "corridor", "shock"],
            dominant_field="economy",
            actors=["trade-ministry", "port-authority"],
            regions=["gulf"],
            divergence_score=18.3,
        )
        linked = self.make_group_event(
            event_id="evt-b",
            title="Supply shock widens across market corridor",
            published_at="2026-03-22T09:00:00Z",
            keywords=["market", "supply", "shock", "corridor"],
            dominant_field="economy",
            actors=["trade-ministry", "port-authority"],
            regions=["gulf"],
            divergence_score=17.6,
        )

        groups: list[EventGroupSummary] = pipeline_module.group_events([anchor, linked])
        candidates = backtrack_candidates([anchor, linked], event_groups=groups)

        self.assertEqual(candidates[0].intervention_type, "market-stabilization")

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
