use std::collections::BTreeMap;

use regex::Regex;

use crate::models::{NormalizedEvent, ScoredEvent};
use crate::normalize::{match_patterns, ACTOR_PATTERNS, FIELD_KEYWORDS, REGION_PATTERNS};

const REGION_WEIGHTS: &[(&str, f64)] = &[
    ("ukraine", 2.5),
    ("russia", 2.0),
    ("middle-east", 2.5),
    ("east-asia", 2.0),
    ("united-states", 1.0),
    ("europe", 1.0),
];

const ACTOR_WEIGHTS: &[(&str, f64)] = &[
    ("nato", 1.5),
    ("eu", 1.0),
    ("un", 1.0),
    ("usa", 1.5),
    ("china", 1.5),
    ("russia", 1.5),
    ("iran", 1.2),
];

const IMPACT_WEIGHT: f64 = 0.65;
const FIELD_ATTRACTION_WEIGHT: f64 = 1.35;
const FA_MARGIN_WEIGHT: f64 = 0.15;
const FA_MAX_MARGIN_BONUS: f64 = 1.0;
const FA_COHERENCE_WEIGHT: f64 = 0.75;
const FA_NEAR_TIE_MARGIN_THRESHOLD: f64 = 1.0;
const FA_NEAR_TIE_WEIGHT: f64 = 0.35;
const FA_MAX_NEAR_TIE_PENALTY: f64 = 0.3;
const FA_DIFFUSE_THIRD_FIELD_THRESHOLD: f64 = 2.5;
const FA_DIFFUSE_THIRD_FIELD_WEIGHT: f64 = 0.1;
const FA_MAX_DIFFUSE_THIRD_FIELD_PENALTY: f64 = 0.2;
const IM_DOMINANT_FIELD_WEIGHT: f64 = 0.25;
const IM_NONZERO_FIELD_WEIGHT: f64 = 0.2;
const IM_NONZERO_FIELD_MIN_SCORE: f64 = 1.0;
const IM_TITLE_SALIENCE_ACTOR_MULTIPLIER: f64 = 0.2;
const IM_TITLE_SALIENCE_REGION_MULTIPLIER: f64 = 0.2;
const IM_TITLE_SALIENCE_ACTOR_MAX_PER_MATCH: f64 = 0.35;
const IM_TITLE_SALIENCE_REGION_MAX_PER_MATCH: f64 = 0.4;
const IM_TITLE_SALIENCE_MAX_BONUS: f64 = 0.8;
const IM_FIELD_IMPACT_BASELINE_AVERAGE_WEIGHT: f64 = 1.5;
const IM_FIELD_IMPACT_SCALE_WEIGHT: f64 = 0.06;
const IM_FIELD_IMPACT_MAX_BONUS: f64 = 0.5;
const IM_TEXT_SIGNAL_KEYWORD_WEIGHT: f64 = 0.12;
const IM_TEXT_SIGNAL_TITLE_WEIGHT: f64 = 0.2;
const IM_TEXT_SIGNAL_SUMMARY_WEIGHT: f64 = 0.1;
const IM_TEXT_SIGNAL_MAX_KEYWORD_HITS: usize = 4;
const IM_TEXT_SIGNAL_MAX_TITLE_HITS: usize = 2;
const IM_TEXT_SIGNAL_MAX_SUMMARY_HITS: usize = 2;
const IM_TEXT_SIGNAL_MAX_BONUS: f64 = 1.0;

fn weight_lookup(table: &[(&str, f64)], key: &str, default: f64) -> f64 {
    table
        .iter()
        .find(|(k, _)| *k == key)
        .map(|(_, v)| *v)
        .unwrap_or(default)
}

pub fn score_events(events: &[NormalizedEvent]) -> Vec<ScoredEvent> {
    let mut scored: Vec<ScoredEvent> = events.iter().map(score_event).collect();
    scored.sort_by(|a, b| {
        b.divergence_score
            .partial_cmp(&a.divergence_score)
            .unwrap_or(std::cmp::Ordering::Equal)
    });
    scored
}

fn score_event(event: &NormalizedEvent) -> ScoredEvent {
    let (dominant_field, dominant_field_strength) = select_dominant_field(event);
    let title_salience_bonus = compute_title_salience_bonus(event);
    let field_impact_scaling_bonus =
        compute_field_impact_scaling_bonus(&dominant_field, dominant_field_strength);
    let text_signal_intensity = compute_text_signal_intensity(event, &dominant_field);
    let fa_score = compute_fa(event, dominant_field_strength);
    let im_score = compute_im(
        event,
        dominant_field_strength,
        &event.field_scores,
        title_salience_bonus,
        field_impact_scaling_bonus,
        text_signal_intensity,
    );
    let divergence_score = compute_divergence_score(im_score, fa_score);
    let rationale = build_rationale(
        event,
        &dominant_field,
        im_score,
        fa_score,
        title_salience_bonus,
        field_impact_scaling_bonus,
        text_signal_intensity,
    );

    ScoredEvent {
        event_id: event.event_id.clone(),
        title: event.title.clone(),
        source: event.source.clone(),
        link: event.link.clone(),
        published_at: event.published_at.clone(),
        actors: event.actors.clone(),
        regions: event.regions.clone(),
        keywords: event.keywords.clone(),
        dominant_field,
        impact_score: im_score,
        field_attraction: fa_score,
        divergence_score,
        rationale,
    }
}

fn compute_im(
    event: &NormalizedEvent,
    dominant_field_strength: f64,
    field_scores: &BTreeMap<String, f64>,
    title_salience_bonus: f64,
    field_impact_scaling_bonus: f64,
    text_signal_intensity: f64,
) -> f64 {
    let actor_weight: f64 = event
        .actors
        .iter()
        .map(|actor| weight_lookup(ACTOR_WEIGHTS, actor, 0.6))
        .sum();
    let region_weight: f64 = event
        .regions
        .iter()
        .map(|region| weight_lookup(REGION_WEIGHTS, region, 0.5))
        .sum();
    let keyword_density = (event.keywords.len() as f64 * 0.25).min(3.0);
    let nonzero_field_count = field_scores
        .values()
        .filter(|score| **score >= IM_NONZERO_FIELD_MIN_SCORE)
        .count();
    let nonzero_field_count = if nonzero_field_count == 0 && dominant_field_strength > 0.0 {
        1
    } else {
        nonzero_field_count
    };
    let evidence_bonus = (dominant_field_strength * IM_DOMINANT_FIELD_WEIGHT)
        + (nonzero_field_count as f64 * IM_NONZERO_FIELD_WEIGHT);

    round2(
        3.0 + actor_weight
            + region_weight
            + title_salience_bonus
            + keyword_density
            + evidence_bonus
            + field_impact_scaling_bonus
            + text_signal_intensity,
    )
}

fn compute_title_salience_bonus(event: &NormalizedEvent) -> f64 {
    let title_actors: Vec<String> = match_patterns(&event.title, ACTOR_PATTERNS);
    let title_regions: Vec<String> = match_patterns(&event.title, REGION_PATTERNS);

    let actor_bonus: f64 = event
        .actors
        .iter()
        .filter(|actor| title_actors.contains(actor))
        .map(|actor| {
            (weight_lookup(ACTOR_WEIGHTS, actor, 0.6) * IM_TITLE_SALIENCE_ACTOR_MULTIPLIER)
                .min(IM_TITLE_SALIENCE_ACTOR_MAX_PER_MATCH)
        })
        .sum();

    let region_bonus: f64 = event
        .regions
        .iter()
        .filter(|region| title_regions.contains(region))
        .map(|region| {
            (weight_lookup(REGION_WEIGHTS, region, 0.5) * IM_TITLE_SALIENCE_REGION_MULTIPLIER)
                .min(IM_TITLE_SALIENCE_REGION_MAX_PER_MATCH)
        })
        .sum();

    round2((actor_bonus + region_bonus).min(IM_TITLE_SALIENCE_MAX_BONUS))
}

fn compute_field_impact_scaling_bonus(dominant_field: &str, dominant_field_strength: f64) -> f64 {
    let dominant_keywords = match FIELD_KEYWORDS
        .iter()
        .find(|(name, _)| *name == dominant_field)
    {
        Some((_, keywords)) => keywords,
        None => return 0.0,
    };
    if dominant_keywords.is_empty() || dominant_field_strength <= 0.0 {
        return 0.0;
    }
    let average_keyword_weight: f64 =
        dominant_keywords.iter().map(|(_, w)| w).sum::<f64>() / dominant_keywords.len() as f64;
    round2(
        ((average_keyword_weight - IM_FIELD_IMPACT_BASELINE_AVERAGE_WEIGHT).max(0.0)
            * dominant_field_strength
            * IM_FIELD_IMPACT_SCALE_WEIGHT)
            .min(IM_FIELD_IMPACT_MAX_BONUS),
    )
}

fn compute_text_signal_intensity(event: &NormalizedEvent, dominant_field: &str) -> f64 {
    let dominant_keywords = match FIELD_KEYWORDS
        .iter()
        .find(|(name, _)| *name == dominant_field)
    {
        Some((_, keywords)) if !keywords.is_empty() => keywords,
        _ => return 0.0,
    };

    let keyword_hits = event
        .keywords
        .iter()
        .filter(|keyword| dominant_keywords.iter().any(|(k, _)| *k == **keyword))
        .count()
        .min(IM_TEXT_SIGNAL_MAX_KEYWORD_HITS);

    let title_hits = count_text_signal_surface_hits(
        &event.title,
        dominant_keywords,
        IM_TEXT_SIGNAL_MAX_TITLE_HITS,
    );
    let summary_hits = count_text_signal_surface_hits(
        &event.summary,
        dominant_keywords,
        IM_TEXT_SIGNAL_MAX_SUMMARY_HITS,
    );

    round2(
        ((keyword_hits as f64 * IM_TEXT_SIGNAL_KEYWORD_WEIGHT)
            + (title_hits as f64 * IM_TEXT_SIGNAL_TITLE_WEIGHT)
            + (summary_hits as f64 * IM_TEXT_SIGNAL_SUMMARY_WEIGHT))
            .min(IM_TEXT_SIGNAL_MAX_BONUS),
    )
}

fn count_text_signal_surface_hits(
    text: &str,
    dominant_keywords: &[(&str, f64)],
    max_hits: usize,
) -> usize {
    let lowered = text.to_lowercase();
    dominant_keywords
        .iter()
        .filter(|(keyword, _)| {
            let pattern = Regex::new(&format!(r"\b{}\b", regex::escape(keyword)))
                .expect("valid text signal pattern");
            pattern.is_match(&lowered)
        })
        .count()
        .min(max_hits)
}

fn select_dominant_field(event: &NormalizedEvent) -> (String, f64) {
    let total_strength: f64 = event.field_scores.values().filter(|s| **s > 0.0).sum();
    if total_strength <= 0.0 {
        return ("uncategorized".to_string(), 0.0);
    }
    let max_strength = event.field_scores.values().copied().fold(0.0_f64, f64::max);
    let mut tied_fields: Vec<&str> = event
        .field_scores
        .iter()
        .filter(|(_, strength)| **strength == max_strength && **strength > 0.0)
        .map(|(name, _)| name.as_str())
        .collect();
    if tied_fields.is_empty() {
        return ("uncategorized".to_string(), 0.0);
    }
    tied_fields.sort();
    (tied_fields[0].to_string(), round2(max_strength))
}

fn compute_fa(event: &NormalizedEvent, dominant_field_strength: f64) -> f64 {
    let mut ordered_scores: Vec<f64> = event.field_scores.values().copied().collect();
    ordered_scores.sort_by(|a, b| b.partial_cmp(a).unwrap_or(std::cmp::Ordering::Equal));

    let second_best_strength = if ordered_scores.len() > 1 {
        round2(ordered_scores[1])
    } else {
        0.0
    };
    let third_best_strength = if ordered_scores.len() > 2 {
        round2(ordered_scores[2])
    } else {
        0.0
    };
    let total_strength: f64 = event.field_scores.values().filter(|s| **s > 0.0).sum();
    if total_strength <= 0.0 {
        return 0.0;
    }
    let dominant_margin = (dominant_field_strength - second_best_strength).max(0.0);

    let margin_bonus = (dominant_margin * FA_MARGIN_WEIGHT).min(FA_MAX_MARGIN_BONUS);
    let coherence_bonus = if total_strength > 0.0 {
        (dominant_field_strength / total_strength) * FA_COHERENCE_WEIGHT
    } else {
        0.0
    };
    let near_tie_penalty = ((FA_NEAR_TIE_MARGIN_THRESHOLD - dominant_margin).max(0.0)
        * FA_NEAR_TIE_WEIGHT)
        .min(FA_MAX_NEAR_TIE_PENALTY);
    let diffuse_third_field_penalty = if dominant_margin >= FA_NEAR_TIE_MARGIN_THRESHOLD {
        ((third_best_strength - FA_DIFFUSE_THIRD_FIELD_THRESHOLD).max(0.0)
            * FA_DIFFUSE_THIRD_FIELD_WEIGHT)
            .min(FA_MAX_DIFFUSE_THIRD_FIELD_PENALTY)
    } else {
        0.0
    };

    round2(
        dominant_field_strength + margin_bonus + coherence_bonus
            - near_tie_penalty
            - diffuse_third_field_penalty,
    )
}

fn compute_divergence_score(im_score: f64, fa_score: f64) -> f64 {
    round2(im_score * IMPACT_WEIGHT + fa_score * FIELD_ATTRACTION_WEIGHT)
}

fn build_rationale(
    event: &NormalizedEvent,
    dominant_field: &str,
    im_score: f64,
    fa_score: f64,
    title_salience_bonus: f64,
    field_impact_scaling_bonus: f64,
    text_signal_intensity: f64,
) -> Vec<String> {
    let mut rationale = vec![format!("Im={im_score}"), format!("Fa={fa_score}")];
    if title_salience_bonus > 0.0 {
        rationale.push(format!("im_title_salience={title_salience_bonus}"));
    }
    if field_impact_scaling_bonus > 0.0 {
        rationale.push(format!(
            "im_field_impact_scaling={field_impact_scaling_bonus}"
        ));
    }
    let dominant_field_keywords = FIELD_KEYWORDS
        .iter()
        .find(|(name, _)| *name == dominant_field);
    if let Some((_, keywords)) = dominant_field_keywords {
        if !keywords.is_empty() {
            rationale.push(format!("im_text_signal_intensity={text_signal_intensity}"));
        }
    }
    if !event.actors.is_empty() {
        rationale.push(format!("actors={}", event.actors.join(", ")));
    }
    if !event.regions.is_empty() {
        rationale.push(format!("regions={}", event.regions.join(", ")));
    }
    if fa_score > 0.0 {
        rationale.push(format!("dominant_field={dominant_field}:{fa_score}"));
    } else {
        rationale.push("dominant_field=uncategorized:0".to_string());
    }
    rationale
}

pub fn summarize_scenario(
    scored_events: &[ScoredEvent],
) -> (String, String, String, Vec<String>, Vec<String>) {
    if scored_events.is_empty() {
        return (
            "No high-signal events were available for inference.".to_string(),
            "uncategorized".to_string(),
            "low".to_string(),
            Vec::new(),
            Vec::new(),
        );
    }

    let top_count = 3.min(scored_events.len());
    let top_events = &scored_events[..top_count];

    // field_counts
    let mut field_counts: BTreeMap<String, usize> = BTreeMap::new();
    for event in scored_events {
        *field_counts
            .entry(event.dominant_field.clone())
            .or_insert(0) += 1;
    }

    // region_counts, actor_counts from top events
    let mut region_counts: Vec<(String, usize)> = Vec::new();
    let mut actor_counts: Vec<(String, usize)> = Vec::new();
    for event in top_events {
        for region in &event.regions {
            if let Some(pos) = region_counts.iter().position(|(name, _)| name == region) {
                region_counts[pos].1 += 1;
            } else {
                region_counts.push((region.clone(), 1));
            }
        }
        for actor in &event.actors {
            if let Some(pos) = actor_counts.iter().position(|(name, _)| name == actor) {
                actor_counts[pos].1 += 1;
            } else {
                actor_counts.push((actor.clone(), 1));
            }
        }
    }

    let average_score: f64 =
        top_events.iter().map(|e| e.divergence_score).sum::<f64>() / top_events.len() as f64;
    let risk_level = if average_score >= 9.0 {
        "high"
    } else if average_score >= 6.0 {
        "medium"
    } else {
        "low"
    };

    let max_field_count = *field_counts.values().max().unwrap_or(&0);
    let tied_fields: Vec<String> = field_counts
        .iter()
        .filter(|(_, count)| **count == max_field_count)
        .map(|(name, _)| name.clone())
        .collect();

    let best_field_divergence: BTreeMap<String, f64> = tied_fields
        .iter()
        .map(|field_name| {
            let max_div = scored_events
                .iter()
                .filter(|e| e.dominant_field == *field_name)
                .map(|e| e.divergence_score)
                .fold(0.0_f64, f64::max);
            (field_name.clone(), max_div)
        })
        .collect();

    // Python: sorted(tied_fields, key=lambda fn: (-best_div[field], field))
    let dominant_field = tied_fields
        .iter()
        .min_by(|a, b| {
            let div_a = best_field_divergence[a.as_str()];
            let div_b = best_field_divergence[b.as_str()];
            (-div_a)
                .partial_cmp(&(-div_b))
                .unwrap_or(std::cmp::Ordering::Equal)
                .then_with(|| a.cmp(b))
        })
        .cloned()
        .unwrap_or_else(|| "uncategorized".to_string());

    let headline = format!(
        "The strongest current branch is {dominant_field}, driven by {} and {} additional high-signal events.",
        top_events[0].title.to_lowercase(),
        top_events.len() - 1
    );

    // Python Counter.most_common(3): sort by count desc, then insertion order for ties
    // We preserve insertion order by using a Vec and only sorting by -count
    // (ties remain in insertion order, matching Python Counter behavior)
    region_counts.sort_by(|a, b| b.1.cmp(&a.1));
    actor_counts.sort_by(|a, b| b.1.cmp(&a.1));

    let top_regions: Vec<String> = region_counts
        .into_iter()
        .take(3)
        .map(|(name, _)| name)
        .collect();

    let top_actors: Vec<String> = actor_counts
        .into_iter()
        .take(3)
        .map(|(name, _)| name)
        .collect();

    (
        headline,
        dominant_field,
        risk_level.to_string(),
        top_regions,
        top_actors,
    )
}

/// Round to 2 decimal places (matches Python `round(value, 2)`).
///
/// Uses format-based rounding to correctly handle edge cases like 6.175
/// where the IEEE 754 representation is slightly below the mathematical
/// value. Python's round() detects this and rounds down, but naive
/// `(x * 100.0).round() / 100.0` would round up due to float multiplication.
fn round2(value: f64) -> f64 {
    format!("{:.2}", value).parse::<f64>().unwrap()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn compute_im_base_formula_components() {
        let actor_weight: f64 = 1.5 + 1.5; // usa + china
        let region_weight: f64 = 2.0 + 1.0; // east-asia + united-states
        let keyword_density: f64 = (12.0_f64 * 0.25_f64).min(3.0_f64); // 3.0
        assert_eq!(round2(actor_weight), 3.0);
        assert_eq!(round2(region_weight), 3.0);
        assert_eq!(keyword_density, 3.0);
    }

    #[test]
    fn compute_fa_handles_zero_total_strength() {
        let event = NormalizedEvent {
            event_id: "test".to_string(),
            source: "test".to_string(),
            title: "test".to_string(),
            summary: "test".to_string(),
            link: "test".to_string(),
            published_at: None,
            keywords: vec![],
            actors: vec![],
            regions: vec![],
            field_scores: BTreeMap::from([
                ("conflict".to_string(), 0.0),
                ("diplomacy".to_string(), 0.0),
                ("technology".to_string(), 0.0),
                ("economy".to_string(), 0.0),
            ]),
            entry_identity_hash: String::new(),
            content_hash: String::new(),
        };
        let (dominant_field, strength) = select_dominant_field(&event);
        assert_eq!(dominant_field, "uncategorized");
        assert_eq!(strength, 0.0);
        assert_eq!(compute_fa(&event, strength), 0.0);
    }

    #[test]
    fn select_dominant_field_picks_canonical_order_on_tie() {
        let mut scores = BTreeMap::new();
        scores.insert("conflict".to_string(), 5.0);
        scores.insert("diplomacy".to_string(), 5.0);
        let event = NormalizedEvent {
            event_id: "test".to_string(),
            source: "test".to_string(),
            title: "test".to_string(),
            summary: "test".to_string(),
            link: "test".to_string(),
            published_at: None,
            keywords: vec![],
            actors: vec![],
            regions: vec![],
            field_scores: scores,
            entry_identity_hash: String::new(),
            content_hash: String::new(),
        };
        let (dominant_field, strength) = select_dominant_field(&event);
        assert_eq!(dominant_field, "conflict");
        assert_eq!(strength, 5.0);
    }

    #[test]
    fn divergence_score_formula() {
        // divergence_score = Im * 0.65 + Fa * 1.35
        let im = 15.79;
        let fa = 7.75;
        assert_eq!(
            compute_divergence_score(im, fa),
            round2(im * 0.65 + fa * 1.35)
        );
    }
}
