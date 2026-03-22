from __future__ import annotations

from dataclasses import asdict, dataclass, field
from typing import Any


@dataclass(slots=True)
class RawItem:
    source: str
    title: str
    summary: str
    link: str
    published_at: str | None


@dataclass(slots=True)
class NormalizedEvent:
    event_id: str
    source: str
    title: str
    summary: str
    link: str
    published_at: str | None
    keywords: list[str]
    actors: list[str]
    regions: list[str]
    field_scores: dict[str, float]


@dataclass(slots=True)
class ScoredEvent:
    event_id: str
    title: str
    source: str
    link: str
    published_at: str | None
    actors: list[str]
    regions: list[str]
    keywords: list[str]
    dominant_field: str
    impact_score: float
    field_attraction: float
    divergence_score: float
    rationale: list[str]


@dataclass(slots=True)
class InterventionCandidate:
    priority: int
    event_id: str
    target: str
    intervention_type: str
    reason: str
    expected_effect: str


@dataclass(slots=True)
class RunArtifact:
    mode: str
    generated_at: str
    input_summary: dict[str, Any]
    scenario_summary: dict[str, Any]
    scored_events: list[ScoredEvent] = field(default_factory=list)
    intervention_candidates: list[InterventionCandidate] = field(default_factory=list)

    def to_dict(self) -> dict[str, Any]:
        return {
            "mode": self.mode,
            "generated_at": self.generated_at,
            "input_summary": self.input_summary,
            "scenario_summary": self.scenario_summary,
            "scored_events": [asdict(item) for item in self.scored_events],
            "intervention_candidates": [
                asdict(item) for item in self.intervention_candidates
            ],
        }
