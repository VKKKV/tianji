from __future__ import annotations

import re
from hashlib import sha256

from .fetch import derive_canonical_content_hash, derive_canonical_entry_identity_hash
from .models import NormalizedEvent, RawItem


REGION_PATTERNS: dict[str, str] = {
    "ukraine": r"\bukraine\b",
    "russia": r"\brussia\b|\bmoscow\b",
    "middle-east": r"\biran\b|\bisrael\b|\bgaza\b|\byemen\b|\bsyria\b",
    "east-asia": r"\bchina\b|\btaiwan\b|\bkorea\b|\bjapan\b",
    "united-states": r"\bu\.s\.\b|\bunited states\b|\bwashington\b|\bwhite house\b",
    "europe": r"\beurope\b|\beu\b|\bnato\b|\bbrussels\b",
}

ACTOR_PATTERNS: dict[str, str] = {
    "nato": r"\bnato\b",
    "eu": r"\beu\b|\beuropean union\b",
    "un": r"\bunited nations\b|\bun\b",
    "usa": r"\bu\.s\.\b|\bunited states\b|\bwhite house\b",
    "china": r"\bchina\b|\bbeijing\b",
    "russia": r"\brussia\b|\bkremlin\b|\bmoscow\b",
    "iran": r"\biran\b|\btehran\b",
}

FIELD_KEYWORDS: dict[str, dict[str, float]] = {
    "conflict": {
        "attack": 3.0,
        "missile": 3.5,
        "troops": 2.5,
        "drone": 2.5,
        "ceasefire": 2.0,
        "military": 3.0,
        "strike": 3.0,
    },
    "diplomacy": {
        "talks": 2.5,
        "summit": 2.0,
        "negotiation": 3.0,
        "agreement": 2.5,
        "sanction": 2.0,
    },
    "technology": {
        "ai": 2.0,
        "chip": 2.0,
        "cyber": 2.5,
        "satellite": 2.0,
    },
    "economy": {
        "tariff": 2.0,
        "trade": 2.0,
        "oil": 2.5,
        "inflation": 1.5,
        "market": 1.5,
    },
}

TOKEN_RE = re.compile(r"[a-zA-Z][a-zA-Z0-9_-]+")


def normalize_items(items: list[RawItem]) -> list[NormalizedEvent]:
    return [normalize_item(item) for item in items]


def normalize_item(item: RawItem) -> NormalizedEvent:
    title = clean_text(item.title)
    summary = clean_text(item.summary)
    entry_identity_hash = (
        item.entry_identity_hash or derive_canonical_entry_identity_hash(item)
    )
    content_hash = item.content_hash or derive_canonical_content_hash(item)
    text = clean_text(f"{title}\n{summary}")
    keywords = extract_keywords(text)
    actors = match_patterns(text, ACTOR_PATTERNS)
    regions = match_patterns(text, REGION_PATTERNS)
    field_scores = derive_field_scores(text)
    event_id = sha256(
        f"{item.source}|{item.title}|{item.link}".encode("utf-8")
    ).hexdigest()[:16]
    return NormalizedEvent(
        event_id=event_id,
        source=item.source,
        title=title,
        summary=summary,
        link=item.link,
        published_at=item.published_at,
        keywords=keywords,
        actors=actors,
        regions=regions,
        field_scores=field_scores,
        entry_identity_hash=entry_identity_hash,
        content_hash=content_hash,
    )


def clean_text(text: str) -> str:
    return re.sub(r"\s+", " ", text).strip()


def extract_keywords(text: str, limit: int = 12) -> list[str]:
    lowered = text.lower()
    tokens = [token for token in TOKEN_RE.findall(lowered) if len(token) > 2]
    seen: list[str] = []
    for token in tokens:
        if token not in seen:
            seen.append(token)
        if len(seen) >= limit:
            break
    return seen


def match_patterns(text: str, patterns: dict[str, str]) -> list[str]:
    lowered = text.lower()
    return [name for name, pattern in patterns.items() if re.search(pattern, lowered)]


def derive_field_scores(text: str) -> dict[str, float]:
    lowered = text.lower()
    scores: dict[str, float] = {}
    for field_name, weights in FIELD_KEYWORDS.items():
        score = 0.0
        for keyword, weight in weights.items():
            if keyword in lowered:
                score += weight
        scores[field_name] = round(score, 2)
    return scores
