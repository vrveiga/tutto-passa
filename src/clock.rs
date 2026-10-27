use anyhow::{Context, Result};
use jiff::{ToSpan, Zoned, civil};

use crate::config::Config;

/// Local wall-clock time minus `day_starts_at` hours. Civil arithmetic, so DST
/// transitions never shift which day an instant belongs to.
pub fn effective_today(now: civil::DateTime, day_starts_at: u8) -> civil::Date {
    now.saturating_sub(i64::from(day_starts_at).hours()).date()
}

/// Reads `TP_TODAY` or the system clock. Only called from `main`.
pub fn today(config: &Config) -> Result<civil::Date> {
    match std::env::var("TP_TODAY") {
        Ok(s) if !s.is_empty() => s.parse().with_context(|| format!("invalid TP_TODAY '{s}'")),
        _ => Ok(effective_today(Zoned::now().datetime(), config.day_starts_at)),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use jiff::civil::{date, datetime};
    use jiff::tz::TimeZone;

    #[test]
    fn around_midnight() {
        for (now, starts, want) in [
            (datetime(2026, 10, 28, 1, 30, 0, 0), 4, date(2026, 10, 27)),
            (datetime(2026, 10, 28, 3, 59, 59, 0), 4, date(2026, 10, 27)),
            (datetime(2026, 10, 28, 4, 0, 0, 0), 4, date(2026, 10, 28)),
            (datetime(2026, 10, 28, 0, 0, 0, 0), 0, date(2026, 10, 28)),
            (datetime(2026, 10, 27, 23, 59, 0, 0), 0, date(2026, 10, 27)),
            (datetime(2026, 1, 1, 2, 0, 0, 0), 4, date(2025, 12, 31)),
        ] {
            assert_eq!(effective_today(now, starts), want, "{now}");
        }
    }

    #[test]
    fn dst_transitions() {
        let tz = TimeZone::get("America/New_York").unwrap();
        // Spring forward 2026-03-08 02:00 -> 03:00. 03:30 local is only 1.5h
        // after midnight in real time, but the wall clock says 03:30.
        let z = datetime(2026, 3, 8, 3, 30, 0, 0).to_zoned(tz.clone()).unwrap();
        assert_eq!(effective_today(z.datetime(), 4), date(2026, 3, 7));
        let z = datetime(2026, 3, 8, 4, 0, 0, 0).to_zoned(tz.clone()).unwrap();
        assert_eq!(effective_today(z.datetime(), 4), date(2026, 3, 8));
        // Fall back 2026-11-01 02:00 -> 01:00: both 01:30s belong to Oct 31.
        let first = datetime(2026, 11, 1, 1, 30, 0, 0).to_zoned(tz.clone()).unwrap();
        let second = first.checked_add(1.hour()).unwrap();
        assert_eq!(second.datetime(), first.datetime());
        assert_eq!(effective_today(first.datetime(), 4), date(2026, 10, 31));
        assert_eq!(effective_today(second.datetime(), 4), date(2026, 10, 31));
    }
}
