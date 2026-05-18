/// Round to 2 decimal places (matches Python `round(value, 2)`).
///
/// Uses format-based rounding to correctly handle edge cases like 6.175
/// where the IEEE 754 representation is slightly below the mathematical
/// value. Python's round() detects this and rounds down, but naive
/// `(x * 100.0).round() / 100.0` would round up due to float multiplication.
pub fn round2(value: f64) -> f64 {
    if !value.is_finite() {
        return value;
    }
    format!("{:.2}", value).parse::<f64>().unwrap_or(value)
}

/// Collapse runs of whitespace to a single space and trim boundaries.
pub fn clean_text(text: &str) -> String {
    text.split_whitespace().collect::<Vec<_>>().join(" ")
}

/// Collect string values from a JSON array field into a deterministic set.
pub fn collect_string_array(
    value: &serde_json::Value,
    key: &str,
    target: &mut std::collections::BTreeSet<String>,
) {
    if let Some(values) = value.get(key).and_then(|v| v.as_array()) {
        for value in values {
            if let Some(text) = value.as_str() {
                target.insert(text.to_string());
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn round2_basic() {
        assert_eq!(round2(0.0), 0.0);
        assert_eq!(round2(3.14159), 3.14);
        assert_eq!(round2(1.0), 1.0);
        assert_eq!(round2(15.79), 15.79);
        // Negative value
        assert_eq!(round2(-1.5), -1.5);
    }

    #[test]
    fn clean_text_collapses_whitespace_runs() {
        assert_eq!(clean_text("alpha\n\t beta   gamma"), "alpha beta gamma");
    }

    #[test]
    fn clean_text_trims_leading_and_trailing_whitespace() {
        assert_eq!(clean_text("  \n alpha beta \t "), "alpha beta");
        assert_eq!(clean_text(" \n\t "), "");
    }

    #[test]
    fn collect_string_array_inserts_only_strings() {
        let payload = serde_json::json!({
            "actors": ["usa", 42, "china", null, "usa"]
        });
        let mut values = std::collections::BTreeSet::new();

        collect_string_array(&payload, "actors", &mut values);

        assert_eq!(values.into_iter().collect::<Vec<_>>(), vec!["china", "usa"]);
    }
}
