use std::collections::BTreeMap;
use std::sync::LazyLock;

use regex::Regex;

use crate::models::{NormalizedEvent, ScoredEvent};
use crate::normalize::{match_patterns, ACTOR_PATTERNS, FIELD_KEYWORDS, REGION_PATTERNS};
use crate::scoring_params::ScoreParams;
use crate::utils::round2;

static TEXT_SIGNAL_REGEXES: LazyLock<BTreeMap<&'static str, Regex>> = LazyLock::new(|| {
    FIELD_KEYWORDS
        .iter()
        .flat_map(|(_, keywords)| keywords.iter().map(|(keyword, _)| *keyword))
        .map(|keyword| {
            (
                keyword,
                Regex::new(&format!(r"\b{}\b", regex::escape(keyword)))
                    .expect("valid text signal pattern"),
            )
        })
        .collect()
});

// ── Backward-compatible entry points ──

/// Score events using the default parameter set.
pub fn score_events(events: &[NormalizedEvent]) -> Vec<ScoredEvent> {
    score_events_with_params(events, &ScoreParams::default())
}

/// Score events using a custom `ScoreParams` configuration.
pub fn score_events_with_params(
    events: &[NormalizedEvent],
    params: &ScoreParams,
) -> Vec<ScoredEvent> {
    let mut scored: Vec<ScoredEvent> = events.iter().map(|e| score_event(e, params)).collect();
    scored.sort_by(|a, b| {
        b.divergence_score
            .partial_cmp(&a.divergence_score)
            .unwrap_or(std::cmp::Ordering::Equal)
    });
    scored
}

// ── Per-event scoring ──

fn score_event(event: &NormalizedEvent, params: &ScoreParams) -> ScoredEvent {
    let (dominant_field, dominant_field_strength) = select_dominant_field(event);
    let title_salience_bonus = compute_title_salience_bonus(event, params);
    let field_impact_scaling_bonus =
        compute_field_impact_scaling_bonus(&dominant_field, dominant_field_strength, params);
    let text_signal_intensity = compute_text_signal_intensity(event, &dominant_field, params);
    let fa_score = compute_fa(event, dominant_field_strength, params);
    let im_score = compute_im(
        event,
        dominant_field_strength,
        &event.field_scores,
        title_salience_bonus,
        field_impact_scaling_bonus,
        text_signal_intensity,
        params,
    );
    let divergence_score = compute_divergence_score(im_score, fa_score, params);
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
    params: &ScoreParams,
) -> f64 {
    let actor_weight: f64 = event
        .actors
        .iter()
        .map(|actor| params.actor_weight(actor, 0.6))
        .sum();
    let region_weight: f64 = event
        .regions
        .iter()
        .map(|region| params.region_weight(region, 0.5))
        .sum();
    let keyword_density = (event.keywords.len() as f64 * 0.25).min(3.0);
    let nonzero_field_count = field_scores
        .values()
        .filter(|score| **score >= params.im_nonzero_field_min_score)
        .count();
    let nonzero_field_count = if nonzero_field_count == 0 && dominant_field_strength > 0.0 {
        1
    } else {
        nonzero_field_count
    };
    let evidence_bonus = (dominant_field_strength * params.im_dominant_field_weight)
        + (nonzero_field_count as f64 * params.im_nonzero_field_weight);

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

fn compute_title_salience_bonus(event: &NormalizedEvent, params: &ScoreParams) -> f64 {
    let title_actors: Vec<String> = match_patterns(&event.title, &ACTOR_PATTERNS);
    let title_regions: Vec<String> = match_patterns(&event.title, &REGION_PATTERNS);

    let actor_bonus: f64 = event
        .actors
        .iter()
        .filter(|actor| title_actors.contains(actor))
        .map(|actor| {
            (params.actor_weight(actor, 0.6) * params.im_title_salience_actor_multiplier)
                .min(params.im_title_salience_actor_max_per_match)
        })
        .sum();

    let region_bonus: f64 = event
        .regions
        .iter()
        .filter(|region| title_regions.contains(region))
        .map(|region| {
            (params.region_weight(region, 0.5) * params.im_title_salience_region_multiplier)
                .min(params.im_title_salience_region_max_per_match)
        })
        .sum();

    round2((actor_bonus + region_bonus).min(params.im_title_salience_max_bonus))
}

fn compute_field_impact_scaling_bonus(
    dominant_field: &str,
    dominant_field_strength: f64,
    params: &ScoreParams,
) -> f64 {
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
        ((average_keyword_weight - params.im_field_impact_baseline_average_weight).max(0.0)
            * dominant_field_strength
            * params.im_field_impact_scale_weight)
            .min(params.im_field_impact_max_bonus),
    )
}

fn compute_text_signal_intensity(
    event: &NormalizedEvent,
    dominant_field: &str,
    params: &ScoreParams,
) -> f64 {
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
        .min(params.im_text_signal_max_keyword_hits);

    let title_hits = count_text_signal_surface_hits(
        &event.title,
        dominant_keywords,
        params.im_text_signal_max_title_hits,
    );
    let summary_hits = count_text_signal_surface_hits(
        &event.summary,
        dominant_keywords,
        params.im_text_signal_max_summary_hits,
    );

    round2(
        ((keyword_hits as f64 * params.im_text_signal_keyword_weight)
            + (title_hits as f64 * params.im_text_signal_title_weight)
            + (summary_hits as f64 * params.im_text_signal_summary_weight))
            .min(params.im_text_signal_max_bonus),
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
            TEXT_SIGNAL_REGEXES
                .get(keyword)
                .is_some_and(|pattern| pattern.is_match(&lowered))
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

fn compute_fa(event: &NormalizedEvent, dominant_field_strength: f64, params: &ScoreParams) -> f64 {
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

    let margin_bonus = (dominant_margin * params.fa_margin_weight).min(params.fa_max_margin_bonus);
    let coherence_bonus = if total_strength > 0.0 {
        (dominant_field_strength / total_strength) * params.fa_coherence_weight
    } else {
        0.0
    };
    let near_tie_penalty = ((params.fa_near_tie_margin_threshold - dominant_margin).max(0.0)
        * params.fa_near_tie_weight)
        .min(params.fa_max_near_tie_penalty);
    let diffuse_third_field_penalty = if dominant_margin >= params.fa_near_tie_margin_threshold {
        ((third_best_strength - params.fa_diffuse_third_field_threshold).max(0.0)
            * params.fa_diffuse_third_field_weight)
            .min(params.fa_max_diffuse_third_field_penalty)
    } else {
        0.0
    };

    round2(
        dominant_field_strength + margin_bonus + coherence_bonus
            - near_tie_penalty
            - diffuse_third_field_penalty,
    )
}

fn compute_divergence_score(im_score: f64, fa_score: f64, params: &ScoreParams) -> f64 {
    round2(im_score * params.impact_weight + fa_score * params.field_attraction_weight)
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
        assert_eq!(compute_fa(&event, strength, &ScoreParams::default()), 0.0);
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
        let params = ScoreParams::default();
        // divergence_score = Im * impact_weight + Fa * field_attraction_weight
        let im = 15.79;
        let fa = 7.75;
        assert_eq!(
            compute_divergence_score(im, fa, &params),
            round2(im * params.impact_weight + fa * params.field_attraction_weight)
        );
    }
}
