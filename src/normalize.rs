use std::collections::BTreeMap;

use regex::Regex;
use sha2::{Digest, Sha256};

use crate::fetch::{derive_canonical_content_hash, derive_canonical_entry_identity_hash};
use crate::models::{NormalizedEvent, RawItem};

pub const REGION_PATTERNS: &[(&str, &str)] = &[
    ("ukraine", r"\bukraine\b"),
    ("russia", r"\brussia\b|\bmoscow\b"),
    (
        "middle-east",
        r"\biran\b|\bisrael\b|\bgaza\b|\byemen\b|\bsyria\b",
    ),
    ("east-asia", r"\bchina\b|\btaiwan\b|\bkorea\b|\bjapan\b"),
    (
        "united-states",
        r"\bu\.s\.\b|\bunited states\b|\bwashington\b|\bwhite house\b",
    ),
    ("europe", r"\beurope\b|\beu\b|\bnato\b|\bbrussels\b"),
];

pub const ACTOR_PATTERNS: &[(&str, &str)] = &[
    ("nato", r"\bnato\b"),
    ("eu", r"\beu\b|\beuropean union\b"),
    ("un", r"\bunited nations\b|\bun\b"),
    ("usa", r"\bu\.s\.\b|\bunited states\b|\bwhite house\b"),
    ("china", r"\bchina\b|\bbeijing\b"),
    ("russia", r"\brussia\b|\bkremlin\b|\bmoscow\b"),
    ("iran", r"\biran\b|\btehran\b"),
];

pub const FIELD_KEYWORDS: &[(&str, &[(&str, f64)])] = &[
    (
        "conflict",
        &[
            ("attack", 3.0),
            ("missile", 3.5),
            ("troops", 2.5),
            ("drone", 2.5),
            ("ceasefire", 2.0),
            ("military", 3.0),
            ("strike", 3.0),
        ],
    ),
    (
        "diplomacy",
        &[
            ("talks", 2.5),
            ("summit", 2.0),
            ("negotiation", 3.0),
            ("agreement", 2.5),
            ("sanction", 2.0),
        ],
    ),
    (
        "technology",
        &[
            ("ai", 2.0),
            ("chip", 2.0),
            ("cyber", 2.5),
            ("satellite", 2.0),
        ],
    ),
    (
        "economy",
        &[
            ("tariff", 2.0),
            ("trade", 2.0),
            ("oil", 2.5),
            ("inflation", 1.5),
            ("market", 1.5),
        ],
    ),
];

pub fn normalize_items(items: &[RawItem]) -> Vec<NormalizedEvent> {
    items.iter().map(normalize_item).collect()
}

pub fn normalize_item(item: &RawItem) -> NormalizedEvent {
    let title = clean_text(&item.title);
    let summary = clean_text(&item.summary);
    let entry_identity_hash = if item.entry_identity_hash.is_empty() {
        derive_canonical_entry_identity_hash(item)
    } else {
        item.entry_identity_hash.clone()
    };
    let content_hash = if item.content_hash.is_empty() {
        derive_canonical_content_hash(item)
    } else {
        item.content_hash.clone()
    };
    let text = clean_text(&format!("{title}\n{summary}"));
    let event_id = format!(
        "{:x}",
        Sha256::digest(format!("{}|{}|{}", item.source, item.title, item.link).as_bytes())
    )[..16]
        .to_string();

    NormalizedEvent {
        event_id,
        source: item.source.clone(),
        title,
        summary,
        link: item.link.clone(),
        published_at: item.published_at.clone(),
        keywords: extract_keywords(&text, 12),
        actors: match_patterns(&text, ACTOR_PATTERNS),
        regions: match_patterns(&text, REGION_PATTERNS),
        field_scores: derive_field_scores(&text),
        entry_identity_hash,
        content_hash,
    }
}

pub fn clean_text(text: &str) -> String {
    Regex::new(r"\s+")
        .expect("valid whitespace regex")
        .replace_all(text, " ")
        .trim()
        .to_string()
}

pub fn extract_keywords(text: &str, limit: usize) -> Vec<String> {
    let token_re = Regex::new(r"[a-zA-Z][a-zA-Z0-9_-]+").expect("valid token regex");
    let lowered = text.to_lowercase();
    let mut seen = Vec::new();
    for matched in token_re.find_iter(&lowered) {
        let token = matched.as_str().to_string();
        if token.len() > 2 && !seen.contains(&token) {
            seen.push(token);
        }
        if seen.len() >= limit {
            break;
        }
    }
    seen
}

pub fn match_patterns(text: &str, patterns: &[(&str, &str)]) -> Vec<String> {
    let lowered = text.to_lowercase();
    patterns
        .iter()
        .filter(|&(_, pattern)| {
            Regex::new(pattern)
                .expect("valid oracle pattern")
                .is_match(&lowered)
        })
        .map(|(name, _)| (*name).to_string())
        .collect()
}

pub fn derive_field_scores(text: &str) -> BTreeMap<String, f64> {
    let lowered = text.to_lowercase();
    let mut scores = BTreeMap::new();
    for (field_name, weights) in FIELD_KEYWORDS {
        let score: f64 = weights
            .iter()
            .filter_map(|(keyword, weight)| lowered.contains(keyword).then_some(weight))
            .sum();
        let rounded = (score * 100.0).round() / 100.0;
        scores.insert(
            (*field_name).to_string(),
            if rounded == 0.0 { 0.0 } else { rounded },
        );
    }
    scores
}
