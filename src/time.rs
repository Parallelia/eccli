//! Time-window helpers for election scheduling.

use anyhow::{bail, Result};

/// Values below this threshold are relative offsets (seconds from now); values
/// at or above it are absolute unix timestamps.
pub const ABSOLUTE_THRESHOLD: i64 = 1_000_000_000;

/// Resolve a user-supplied time value into an absolute unix timestamp.
///
/// Relative values are added to `now` with overflow checking so that
/// user-controlled input cannot panic or wrap.
pub fn resolve_ts(value: i64, now: i64) -> Result<i64> {
    if value < ABSOLUTE_THRESHOLD {
        now.checked_add(value)
            .ok_or_else(|| anyhow::anyhow!("time offset {value} overflows when added to {now}"))
    } else {
        Ok(value)
    }
}

/// Compute `(start, end)` unix timestamps from CLI inputs.
///
/// Exactly one of `duration` (seconds) or `end_time` must be provided.
pub fn compute_window(
    start: i64,
    duration: Option<i64>,
    end_time: Option<i64>,
    now: i64,
) -> Result<(i64, i64)> {
    let start_ts = resolve_ts(start, now)?;
    let end_ts = match (duration, end_time) {
        (Some(_), Some(_)) => bail!("--duration and --end-time are mutually exclusive"),
        (None, None) => bail!("provide one of --duration or --end-time"),
        (Some(d), None) => {
            if d <= 0 {
                bail!("--duration must be a positive number of seconds");
            }
            start_ts.checked_add(d).ok_or_else(|| {
                anyhow::anyhow!("duration {d} overflows the start time {start_ts}")
            })?
        }
        (None, Some(e)) => resolve_ts(e, now)?,
    };
    if end_ts <= start_ts {
        bail!("end time ({end_ts}) must be after start time ({start_ts})");
    }
    Ok((start_ts, end_ts))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn relative_values_add_to_now() {
        assert_eq!(resolve_ts(60, 1_000).unwrap(), 1_060);
    }

    #[test]
    fn absolute_values_pass_through() {
        assert_eq!(resolve_ts(2_000_000_000, 1_000).unwrap(), 2_000_000_000);
    }

    #[test]
    fn duration_computes_end() {
        let (s, e) = compute_window(60, Some(3600), None, 1_000).unwrap();
        assert_eq!(s, 1_060);
        assert_eq!(e, 1_060 + 3600);
    }

    #[test]
    fn end_time_absolute_is_used() {
        let (_s, e) = compute_window(0, None, Some(2_000_000_000), 1_000).unwrap();
        assert_eq!(e, 2_000_000_000);
    }

    #[test]
    fn both_duration_and_end_is_error() {
        assert!(compute_window(0, Some(10), Some(2_000_000_000), 1_000).is_err());
    }

    #[test]
    fn neither_duration_nor_end_is_error() {
        assert!(compute_window(0, None, None, 1_000).is_err());
    }

    #[test]
    fn end_before_start_is_error() {
        // duration resolves end before start is impossible with positive duration,
        // but an absolute end before start must be rejected.
        assert!(compute_window(2_000_000_000, None, Some(1_500_000_000), 1_000).is_err());
    }

    #[test]
    fn non_positive_duration_is_error() {
        assert!(compute_window(0, Some(0), None, 1_000).is_err());
        assert!(compute_window(0, Some(-5), None, 1_000).is_err());
    }

    #[test]
    fn relative_offset_overflow_is_error() {
        assert!(resolve_ts(1, i64::MAX).is_err());
    }

    #[test]
    fn duration_overflow_is_error() {
        // start resolves to i64::MAX (absolute), duration addition overflows.
        assert!(compute_window(i64::MAX, Some(1), None, 1_000).is_err());
    }
}
