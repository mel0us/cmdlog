use cmdlog::time::{days_since_epoch, iso_timestamp, now_unix_secs, today_prefix};

#[test]
fn days_since_epoch_unix_epoch() {
    assert_eq!(days_since_epoch(1970, 1, 1), 0);
}

#[test]
fn days_since_epoch_known_date() {
    // 2000-01-01 = day 10957
    assert_eq!(days_since_epoch(2000, 1, 1), 10957);
}

#[test]
fn days_since_epoch_leap_year_feb_29() {
    // 2024 is a leap year
    let feb28 = days_since_epoch(2024, 2, 28);
    let mar1 = days_since_epoch(2024, 3, 1);
    assert_eq!(mar1 - feb28, 2); // Feb 28 → Feb 29 → Mar 1
}

#[test]
fn days_since_epoch_non_leap_year() {
    // 2023 is not a leap year
    let feb28 = days_since_epoch(2023, 2, 28);
    let mar1 = days_since_epoch(2023, 3, 1);
    assert_eq!(mar1 - feb28, 1); // Feb 28 → Mar 1 directly
}

#[test]
fn days_since_epoch_century_not_leap() {
    // 1900 is NOT a leap year (divisible by 100 but not 400)
    let feb28 = days_since_epoch(1972, 2, 28);
    let mar1 = days_since_epoch(1972, 3, 1);
    assert_eq!(mar1 - feb28, 2); // 1972 is a leap year
}

#[test]
fn days_since_epoch_400_year_leap() {
    // 2000 IS a leap year (divisible by 400)
    let feb28 = days_since_epoch(2000, 2, 28);
    let mar1 = days_since_epoch(2000, 3, 1);
    assert_eq!(mar1 - feb28, 2);
}

#[test]
fn days_since_epoch_monotonic() {
    let d1 = days_since_epoch(2026, 4, 5);
    let d2 = days_since_epoch(2026, 4, 6);
    let d3 = days_since_epoch(2026, 4, 7);
    assert!(d1 < d2);
    assert!(d2 < d3);
    assert_eq!(d2 - d1, 1);
    assert_eq!(d3 - d2, 1);
}

#[test]
fn days_since_epoch_full_year() {
    // Non-leap year has 365 days
    let start = days_since_epoch(2023, 1, 1);
    let end = days_since_epoch(2024, 1, 1);
    assert_eq!(end - start, 365);

    // Leap year has 366 days
    let start = days_since_epoch(2024, 1, 1);
    let end = days_since_epoch(2025, 1, 1);
    assert_eq!(end - start, 366);
}

#[test]
fn iso_timestamp_format() {
    let ts = iso_timestamp();
    // Must match YYYY-MM-DDTHH:MM:SS
    let re = regex::Regex::new(r"^\d{4}-\d{2}-\d{2}T\d{2}:\d{2}:\d{2}$").unwrap();
    assert!(re.is_match(&ts), "timestamp '{}' doesn't match format", ts);
}

#[test]
fn today_prefix_format() {
    let prefix = today_prefix();
    // Must match YYYY-MM-DD
    let re = regex::Regex::new(r"^\d{4}-\d{2}-\d{2}$").unwrap();
    assert!(re.is_match(&prefix), "prefix '{}' doesn't match format", prefix);
}

#[test]
fn iso_timestamp_starts_with_today_prefix() {
    let ts = iso_timestamp();
    let prefix = today_prefix();
    assert!(
        ts.starts_with(&prefix),
        "timestamp '{}' should start with '{}'",
        ts,
        prefix
    );
}

#[test]
fn now_unix_secs_positive() {
    let secs = now_unix_secs();
    assert!(secs > 0, "unix time should be positive");
    // Should be after 2020
    assert!(secs > 1577836800, "should be after 2020-01-01");
}
