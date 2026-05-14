/// Round to 2 decimal places (matches Python `round(value, 2)`).
///
/// Uses format-based rounding to correctly handle edge cases like 6.175
/// where the IEEE 754 representation is slightly below the mathematical
/// value. Python's round() detects this and rounds down, but naive
/// `(x * 100.0).round() / 100.0` would round up due to float multiplication.
pub fn round2(value: f64) -> f64 {
    format!("{:.2}", value)
        .parse::<f64>()
        .expect("round2: formatted f64 should parse")
}

/// Compute the number of days since Unix epoch (1970-01-01) for the given
/// calendar date. Used by timestamp parsing in grouping and storage.
pub fn days_since_epoch(year: i64, month: i64, day: i64) -> i64 {
    let y = year - 1;
    let leap_years = y / 4 - y / 100 + y / 400;
    let days_from_years = y * 365 + leap_years;

    let cumulative_days_before_month: [i64; 12] =
        [0, 31, 59, 90, 120, 151, 181, 212, 243, 273, 304, 334];
    let is_leap = year % 4 == 0 && (year % 100 != 0 || year % 400 == 0);
    let month_offset = if month >= 3 && is_leap {
        cumulative_days_before_month[month as usize - 1] + 1
    } else {
        cumulative_days_before_month[month as usize - 1]
    };

    days_from_years + month_offset + day - 719528 // offset to unix epoch (1970-01-01)
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
    fn days_since_epoch_consistency_with_grouping_and_storage() {
        // The same function is used in grouping.rs parse_iso_time and storage.rs
        // parse_history_timestamp. Verify basic monotonicity and known offsets.
        let d1 = days_since_epoch(2026, 3, 22);
        let d2 = days_since_epoch(2026, 3, 23);
        assert_eq!(d2 - d1, 1);

        // March 1 vs Feb 29 in a leap year
        let feb29 = days_since_epoch(2024, 2, 29);
        let mar1 = days_since_epoch(2024, 3, 1);
        assert_eq!(mar1 - feb29, 1);

        // Non-leap year: Feb 28 to Mar 1 is 1 day
        let feb28 = days_since_epoch(2023, 2, 28);
        let mar1_nl = days_since_epoch(2023, 3, 1);
        assert_eq!(mar1_nl - feb28, 1);
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
