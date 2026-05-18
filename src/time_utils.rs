use std::sync::LazyLock;

static ISO_RFC3339_TIME_RE: LazyLock<regex::Regex> = LazyLock::new(|| {
    regex::Regex::new(r"^(\d{4})-(\d{2})-(\d{2})T(\d{2}):(\d{2}):(\d{2})")
        .expect("valid ISO/RFC3339 time regex")
});

static STRICT_UTC_RFC3339_TIME_RE: LazyLock<regex::Regex> = LazyLock::new(|| {
    regex::Regex::new(r"^(\d{4})-(\d{2})-(\d{2})T(\d{2}):(\d{2}):(\d{2})(?:Z|\+00:00)?$")
        .expect("valid strict UTC RFC3339 time regex")
});

static RFC2822_TIME_RE: LazyLock<regex::Regex> = LazyLock::new(|| {
    regex::Regex::new(r"^(?:\w{3}, )?(\d{1,2}) (\w{3}) (\d{4}) (\d{2}):(\d{2}):(\d{2})")
        .expect("valid RFC2822 time regex")
});

/// Compute days since Unix epoch (1970-01-01) using Howard Hinnant's
/// `days_from_civil` algorithm.
pub fn days_from_civil(year: i64, month: i64, day: i64) -> i64 {
    let year = year - i64::from(month <= 2);
    let era = if year >= 0 { year } else { year - 399 } / 400;
    let yoe = year - era * 400;
    let doy = (153 * (month + if month > 2 { -3 } else { 9 }) + 2) / 5 + day - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    era * 146_097 + doe - 719_468
}

pub fn datetime_to_unix_seconds(
    year: i64,
    month: i64,
    day: i64,
    hour: i64,
    minute: i64,
    second: i64,
) -> i64 {
    days_from_civil(year, month, day) * 86_400 + hour * 3_600 + minute * 60 + second
}

/// Parse a narrow ISO/RFC3339 UTC timestamp prefix to Unix seconds.
///
/// This intentionally preserves the previous project behavior: the parser
/// accepts `Z`, `+00:00`, and any suffix after the `YYYY-MM-DDTHH:MM:SS`
/// prefix without applying non-UTC offset arithmetic.
pub fn parse_iso_rfc3339_timestamp_seconds(timestamp: &str) -> Option<i64> {
    let normalized = timestamp.replace('Z', "+00:00");
    let caps = ISO_RFC3339_TIME_RE.captures(&normalized)?;
    Some(datetime_to_unix_seconds(
        caps[1].parse().ok()?,
        caps[2].parse().ok()?,
        caps[3].parse().ok()?,
        caps[4].parse().ok()?,
        caps[5].parse().ok()?,
        caps[6].parse().ok()?,
    ))
}

/// Parse integer Unix seconds or a narrow ISO/RFC3339 UTC timestamp.
pub fn parse_unix_or_rfc3339_timestamp_seconds(timestamp: &str) -> Option<i64> {
    timestamp
        .parse::<i64>()
        .ok()
        .or_else(|| parse_strict_utc_rfc3339_timestamp_seconds(timestamp))
}

fn parse_strict_utc_rfc3339_timestamp_seconds(timestamp: &str) -> Option<i64> {
    let caps = STRICT_UTC_RFC3339_TIME_RE.captures(timestamp)?;
    Some(datetime_to_unix_seconds(
        caps[1].parse().ok()?,
        caps[2].parse().ok()?,
        caps[3].parse().ok()?,
        caps[4].parse().ok()?,
        caps[5].parse().ok()?,
        caps[6].parse().ok()?,
    ))
}

/// Parse RSS-style RFC2822-ish timestamps like `Sun, 22 Mar 2026 07:00:00 GMT`.
pub fn parse_rfc2822_timestamp_seconds(timestamp: &str) -> Option<i64> {
    let caps = RFC2822_TIME_RE.captures(timestamp)?;
    let month = month_from_str(&caps[2])?;
    Some(datetime_to_unix_seconds(
        caps[3].parse().ok()?,
        month,
        caps[1].parse().ok()?,
        caps[4].parse().ok()?,
        caps[5].parse().ok()?,
        caps[6].parse().ok()?,
    ))
}

/// Parse event timestamps accepted by grouping: ISO/RFC3339 first, then RSS
/// RFC2822-ish strings.
pub fn parse_event_timestamp_seconds(value: &Option<String>) -> Option<i64> {
    let value = value.as_ref()?;
    parse_iso_rfc3339_timestamp_seconds(value).or_else(|| parse_rfc2822_timestamp_seconds(value))
}

fn month_from_str(s: &str) -> Option<i64> {
    match s {
        "Jan" => Some(1),
        "Feb" => Some(2),
        "Mar" => Some(3),
        "Apr" => Some(4),
        "May" => Some(5),
        "Jun" => Some(6),
        "Jul" => Some(7),
        "Aug" => Some(8),
        "Sep" => Some(9),
        "Oct" => Some(10),
        "Nov" => Some(11),
        "Dec" => Some(12),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn days_from_civil_known_epoch_offsets() {
        assert_eq!(days_from_civil(1970, 1, 1), 0);
        assert_eq!(days_from_civil(1970, 1, 2), 1);
        assert_eq!(days_from_civil(1969, 12, 31), -1);
        assert_eq!(
            days_from_civil(2024, 3, 1) - days_from_civil(2024, 2, 29),
            1
        );
        assert_eq!(
            days_from_civil(2023, 3, 1) - days_from_civil(2023, 2, 28),
            1
        );
    }

    #[test]
    fn datetime_to_unix_seconds_combines_date_and_time() {
        assert_eq!(datetime_to_unix_seconds(1970, 1, 1, 0, 0, 0), 0);
        assert_eq!(datetime_to_unix_seconds(1970, 1, 1, 1, 2, 3), 3_723);
    }

    #[test]
    fn parses_iso_rfc3339_utc_forms() {
        let zulu = parse_iso_rfc3339_timestamp_seconds("2026-03-22T07:00:00Z");
        let utc = parse_iso_rfc3339_timestamp_seconds("2026-03-22T07:00:00+00:00");
        assert_eq!(zulu, utc);
        assert_eq!(
            parse_iso_rfc3339_timestamp_seconds("1970-01-01T00:00:01+00:00"),
            Some(1)
        );
    }

    #[test]
    fn parses_unix_seconds_or_rfc3339() {
        assert_eq!(parse_unix_or_rfc3339_timestamp_seconds("42"), Some(42));
        assert_eq!(
            parse_unix_or_rfc3339_timestamp_seconds("1970-01-01T00:00:42Z"),
            Some(42)
        );
    }

    #[test]
    fn unix_or_rfc3339_parser_preserves_delta_memory_strictness() {
        assert_eq!(
            parse_unix_or_rfc3339_timestamp_seconds("1970-01-01T00:00:42+00:00"),
            Some(42)
        );
        assert_eq!(
            parse_unix_or_rfc3339_timestamp_seconds("1970-01-01T00:00:42"),
            Some(42)
        );
        assert_eq!(
            parse_unix_or_rfc3339_timestamp_seconds("1970-01-01T00:00:42+05:00"),
            None
        );
        assert_eq!(
            parse_unix_or_rfc3339_timestamp_seconds("1970-01-01T00:00:42Zjunk"),
            None
        );
    }

    #[test]
    fn parses_rfc2822ish_rss_timestamp() {
        let first = parse_rfc2822_timestamp_seconds("Sun, 22 Mar 2026 07:00:00 GMT");
        let second = parse_rfc2822_timestamp_seconds("Sun, 22 Mar 2026 08:00:00 GMT");
        assert_eq!(second.zip(first).map(|(b, a)| b - a), Some(3_600));
    }

    #[test]
    fn event_parser_accepts_iso_and_rfc2822() {
        assert_eq!(
            parse_event_timestamp_seconds(&Some("1970-01-01T00:00:05Z".to_string())),
            Some(5)
        );
        assert!(
            parse_event_timestamp_seconds(&Some("Sun, 22 Mar 2026 07:00:00 GMT".to_string()))
                .is_some()
        );
        assert_eq!(parse_event_timestamp_seconds(&None), None);
    }
}
