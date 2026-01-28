use std::time::{SystemTime, UNIX_EPOCH};

pub(crate) fn unix_seconds(time: SystemTime) -> Option<u64> {
    time.duration_since(UNIX_EPOCH).ok().map(|d| d.as_secs())
}

pub(crate) fn unix_seconds_opt(time: Option<SystemTime>) -> Option<u64> {
    time.and_then(unix_seconds)
}

pub(crate) fn unix_seconds_or_zero(time: Option<SystemTime>) -> u64 {
    unix_seconds_opt(time).unwrap_or(0)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;

    #[test]
    fn unix_seconds_handles_before_and_after_epoch() {
        assert_eq!(unix_seconds(UNIX_EPOCH), Some(0));
        assert_eq!(unix_seconds(UNIX_EPOCH + Duration::from_secs(1)), Some(1));
        assert_eq!(unix_seconds(UNIX_EPOCH - Duration::from_secs(1)), None);
    }

    #[test]
    fn unix_seconds_helpers_handle_optional_values() {
        assert_eq!(unix_seconds_opt(None), None);
        assert_eq!(unix_seconds_opt(Some(UNIX_EPOCH)), Some(0));
        assert_eq!(unix_seconds_or_zero(None), 0);
        assert_eq!(
            unix_seconds_or_zero(Some(UNIX_EPOCH + Duration::from_secs(3))),
            3
        );
    }
}
