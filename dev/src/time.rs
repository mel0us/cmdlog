//! Shared time utilities.
//!
//! Timestamps use a configurable UTC offset from the `CMDLOG_TZ` env var
//! (e.g., "+8" for UTC+8, "-5" for UTC-5). Defaults to +8.

use std::time::SystemTime;

/// Return the current time as seconds since UNIX_EPOCH.
pub fn now_unix_secs() -> i64 {
    SystemTime::now()
        .duration_since(SystemTime::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs() as i64
}

/// Parse CMDLOG_TZ env var (e.g., "+8", "-5", "0"). Defaults to +8.
fn tz_offset_secs() -> i64 {
    std::env::var("CMDLOG_TZ")
        .ok()
        .and_then(|s| s.trim().parse::<i32>().ok())
        .unwrap_or(8) as i64
        * 3600
}

/// Break UTC epoch seconds + offset into (year, month, day, hour, min, sec).
fn epoch_to_parts(epoch_secs: i64, offset_secs: i64) -> (u32, u32, u32, u32, u32, u32) {
    let secs = epoch_secs + offset_secs;
    let days = (secs / 86400) as i64;
    let time_of_day = ((secs % 86400) + 86400) % 86400; // handle negative

    let hour = (time_of_day / 3600) as u32;
    let min = ((time_of_day % 3600) / 60) as u32;
    let sec = (time_of_day % 60) as u32;

    // Days since 1970-01-01 to (year, month, day)
    let adjusted_days = if secs < 0 { days - 1 } else { days };
    let (y, m, d) = days_to_ymd(adjusted_days);
    (y, m, d, hour, min, sec)
}

/// Convert days since 1970-01-01 to (year, month, day).
fn days_to_ymd(mut days: i64) -> (u32, u32, u32) {
    let mut y = 1970i64;
    loop {
        let year_days = if is_leap(y as u32) { 366 } else { 365 };
        if days < year_days {
            break;
        }
        days -= year_days;
        y += 1;
    }
    let leap = is_leap(y as u32);
    let months = [31, if leap { 29 } else { 28 }, 31, 30, 31, 30, 31, 31, 30, 31, 30, 31];
    let mut m = 0;
    for &md in &months {
        if days < md {
            break;
        }
        days -= md;
        m += 1;
    }
    (y as u32, m + 1, days as u32 + 1)
}

fn is_leap(y: u32) -> bool {
    y % 4 == 0 && (y % 100 != 0 || y % 400 == 0)
}

/// Format current time as "YYYY-MM-DDTHH:MM:SS" using CMDLOG_TZ offset.
pub fn iso_timestamp() -> String {
    let offset = tz_offset_secs();
    let (y, m, d, h, min, sec) = epoch_to_parts(now_unix_secs(), offset);
    format!("{:04}-{:02}-{:02}T{:02}:{:02}:{:02}", y, m, d, h, min, sec)
}

/// Return today's date as "YYYY-MM-DD" using CMDLOG_TZ offset.
pub fn today_prefix() -> String {
    let offset = tz_offset_secs();
    let (y, m, d, _, _, _) = epoch_to_parts(now_unix_secs(), offset);
    format!("{:04}-{:02}-{:02}", y, m, d)
}

/// Get current time as seconds since a common epoch (for relative comparison).
pub fn current_local_seconds() -> u64 {
    let offset = tz_offset_secs();
    let (y, m, d, h, min, sec) = epoch_to_parts(now_unix_secs(), offset);
    days_since_epoch(y, m, d) * 86400 + h as u64 * 3600 + min as u64 * 60 + sec as u64
}

/// Days since 1970-01-01 for a given date (for relative comparison only).
pub fn days_since_epoch(y: u32, m: u32, d: u32) -> u64 {
    let days_in_month = [0u32, 31, 28, 31, 30, 31, 30, 31, 31, 30, 31, 30, 31];
    let mut total: u64 = 0;
    for yr in 1970..y {
        total += if yr % 4 == 0 && (yr % 100 != 0 || yr % 400 == 0) { 366 } else { 365 };
    }
    for mo in 1..m {
        total += days_in_month[mo as usize] as u64;
        if mo == 2 && y % 4 == 0 && (y % 100 != 0 || y % 400 == 0) {
            total += 1;
        }
    }
    total += (d - 1) as u64;
    total
}
