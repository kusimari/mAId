//! UTC stamps, with no date crate for a few stamps a day.

use std::time::{SystemTime, UNIX_EPOCH};

fn secs() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |d| d.as_secs())
}

/// Today's UTC date as `YYYY-MM-DD`.
pub fn today() -> String {
    civil(secs() / 86_400)
}

/// Now, as `YYYY-MM-DDTHH:MM:SSZ`.
pub fn now() -> String {
    stamp(secs())
}

pub fn stamp(secs: u64) -> String {
    let t = secs % 86_400;
    format!(
        "{}T{:02}:{:02}:{:02}Z",
        civil(secs / 86_400),
        t / 3600,
        t / 60 % 60,
        t % 60
    )
}

/// Days since 1970 to a civil date, after Howard Hinnant's algorithm.
pub fn civil(days: u64) -> String {
    let z = days as i64 + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z - era * 146_097;
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    let y = yoe + era * 400 + i64::from(m <= 2);
    format!("{y:04}-{m:02}-{d:02}")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn stamps_are_utc_and_right_across_leap_days() {
        assert_eq!(civil(0), "1970-01-01");
        assert_eq!(civil(11_016), "2000-02-29");
        assert_eq!(stamp(20_757 * 86_400 + 3_723), "2026-10-31T01:02:03Z");
    }
}
