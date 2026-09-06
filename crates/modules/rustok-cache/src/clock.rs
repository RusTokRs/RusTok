use std::time::{Duration, SystemTime, UNIX_EPOCH};

pub(crate) fn unix_time_millis() -> rustok_core::Result<u64> {
    unix_time_millis_at(SystemTime::now())
}

fn unix_time_millis_at(now: SystemTime) -> rustok_core::Result<u64> {
    let duration = now.duration_since(UNIX_EPOCH).map_err(|error| {
        rustok_core::Error::Cache(format!(
            "system clock is before the Unix epoch by {} ms",
            duration_millis_saturated(error.duration())
        ))
    })?;
    Ok(duration_millis_saturated(duration))
}

fn duration_millis_saturated(duration: Duration) -> u64 {
    duration.as_millis().min(u128::from(u64::MAX)) as u64
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn unix_time_millis_preserves_post_epoch_milliseconds() {
        assert_eq!(
            unix_time_millis_at(UNIX_EPOCH + Duration::from_millis(1_234)).unwrap(),
            1_234
        );
    }

    #[test]
    fn unix_time_millis_rejects_pre_epoch_clock() {
        let error = unix_time_millis_at(UNIX_EPOCH - Duration::from_millis(1)).unwrap_err();
        match error {
            rustok_core::Error::Cache(message) => {
                assert!(message.contains("before the Unix epoch"));
                assert!(message.contains("1 ms"));
            }
            other => panic!("unexpected clock error: {other}"),
        }
    }
}
