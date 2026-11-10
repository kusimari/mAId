//! Dates, for the day a package was first installed: UTC, with no date
//! crate for one stamp per install.

use std::time::{SystemTime, UNIX_EPOCH};

/// Today's UTC date as `YYYY-MM-DD`.
pub fn today() -> String {
    let secs = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |d| d.as_secs());
    civil(secs / 86_400)
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
    fn civil_dates_are_right_across_leap_days() {
        assert_eq!(civil(0), "1970-01-01");
        assert_eq!(civil(11_016), "2000-02-29");
        assert_eq!(civil(20_757), "2026-10-31");
    }
}
