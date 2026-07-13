use std::collections::HashMap;
use std::sync::{Arc, Mutex};

use crate::{
    CacheInvalidationObservation, CacheInvalidationPayloadError, VersionedCacheInvalidation,
    MAX_CACHE_INVALIDATION_CHANNEL_BYTES,
};

pub const DEFAULT_MAX_TRACKED_INVALIDATION_CHANNELS: usize = 4_096;

/// Fail-closed bounded tracker for durable invalidation offsets.
///
/// Existing channels are never evicted because dropping an acknowledged offset would turn the next
/// event into an unverified sequence. New channels are rejected after capacity is reached.
#[derive(Clone)]
pub struct BoundedCacheInvalidationGapTracker {
    last_by_channel: Arc<Mutex<HashMap<String, u64>>>,
    maximum_channels: usize,
}

impl Default for BoundedCacheInvalidationGapTracker {
    fn default() -> Self {
        Self {
            last_by_channel: Arc::new(Mutex::new(HashMap::new())),
            maximum_channels: DEFAULT_MAX_TRACKED_INVALIDATION_CHANNELS,
        }
    }
}

impl BoundedCacheInvalidationGapTracker {
    pub fn new(maximum_channels: usize) -> Result<Self, BoundedInvalidationTrackerError> {
        if maximum_channels == 0 {
            return Err(BoundedInvalidationTrackerError::ZeroCapacity);
        }
        Ok(Self {
            last_by_channel: Arc::new(Mutex::new(HashMap::new())),
            maximum_channels,
        })
    }

    pub fn maximum_channels(&self) -> usize {
        self.maximum_channels
    }

    pub fn channel_count(&self) -> usize {
        self.last_by_channel
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .len()
    }

    pub fn seed(
        &self,
        channel: impl Into<String>,
        last_generation: u64,
    ) -> Result<Option<u64>, BoundedInvalidationTrackerError> {
        self.advance_monotonically(channel.into(), last_generation)
    }

    pub fn acknowledge_recovery(
        &self,
        channel: impl Into<String>,
        recovered_through_generation: u64,
    ) -> Result<Option<u64>, BoundedInvalidationTrackerError> {
        self.advance_monotonically(channel.into(), recovered_through_generation)
    }

    pub fn observe(
        &self,
        event: &VersionedCacheInvalidation,
    ) -> CacheInvalidationObservation {
        let mut generations = self
            .last_by_channel
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);

        let Some(previous) = generations.get(&event.channel).copied() else {
            return CacheInvalidationObservation::UnverifiedFirst {
                generation: event.generation,
            };
        };

        if event.generation == previous {
            return CacheInvalidationObservation::Duplicate {
                generation: event.generation,
            };
        }
        if event.generation < previous {
            return CacheInvalidationObservation::Stale {
                last: previous,
                received: event.generation,
            };
        }

        match previous.checked_add(1) {
            Some(expected) if event.generation == expected => {
                generations.insert(event.channel.clone(), event.generation);
                CacheInvalidationObservation::InOrder {
                    generation: event.generation,
                }
            }
            Some(expected) => CacheInvalidationObservation::Gap {
                previous,
                expected,
                received: event.generation,
            },
            None => CacheInvalidationObservation::Stale {
                last: previous,
                received: event.generation,
            },
        }
    }

    pub fn last_generation(&self, channel: &str) -> Option<u64> {
        self.last_by_channel
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .get(channel)
            .copied()
    }

    pub fn reset(&self, channel: &str) -> Option<u64> {
        self.last_by_channel
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .remove(channel)
    }

    fn advance_monotonically(
        &self,
        channel: String,
        proposed: u64,
    ) -> Result<Option<u64>, BoundedInvalidationTrackerError> {
        validate_channel(&channel)?;
        let mut generations = self
            .last_by_channel
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);

        if let Some(current) = generations.get(&channel).copied() {
            if proposed < current {
                return Err(BoundedInvalidationTrackerError::Payload(
                    CacheInvalidationPayloadError::OffsetRegressed {
                        current,
                        proposed,
                    },
                ));
            }
            return Ok(generations.insert(channel, proposed));
        }

        if generations.len() >= self.maximum_channels {
            return Err(BoundedInvalidationTrackerError::CapacityExceeded {
                count: generations.len(),
                maximum: self.maximum_channels,
            });
        }

        Ok(generations.insert(channel, proposed))
    }
}

fn validate_channel(channel: &str) -> Result<(), BoundedInvalidationTrackerError> {
    if channel.trim().is_empty() {
        return Err(BoundedInvalidationTrackerError::Payload(
            CacheInvalidationPayloadError::EmptyChannel,
        ));
    }
    if channel.len() > MAX_CACHE_INVALIDATION_CHANNEL_BYTES {
        return Err(BoundedInvalidationTrackerError::Payload(
            CacheInvalidationPayloadError::ChannelTooLarge {
                length: channel.len(),
                maximum: MAX_CACHE_INVALIDATION_CHANNEL_BYTES,
            },
        ));
    }
    Ok(())
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum BoundedInvalidationTrackerError {
    ZeroCapacity,
    CapacityExceeded { count: usize, maximum: usize },
    Payload(CacheInvalidationPayloadError),
}

impl std::fmt::Display for BoundedInvalidationTrackerError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::ZeroCapacity => write!(
                formatter,
                "tracked invalidation channel capacity must be greater than zero"
            ),
            Self::CapacityExceeded { count, maximum } => write!(
                formatter,
                "tracked invalidation channels reached capacity {maximum}; current count {count}"
            ),
            Self::Payload(error) => write!(formatter, "invalid invalidation offset: {error}"),
        }
    }
}

impl std::error::Error for BoundedInvalidationTrackerError {}

#[cfg(test)]
mod tests {
    use super::*;

    fn event(channel: &str, generation: u64) -> VersionedCacheInvalidation {
        VersionedCacheInvalidation::new(channel, "tenant:42", generation, 1_000).unwrap()
    }

    #[test]
    fn new_channels_fail_closed_after_capacity_without_evicting_offsets() {
        let tracker = BoundedCacheInvalidationGapTracker::new(2).unwrap();
        tracker.seed("first", 4).unwrap();
        tracker.seed("second", 8).unwrap();

        assert!(matches!(
            tracker.seed("third", 1),
            Err(BoundedInvalidationTrackerError::CapacityExceeded {
                count: 2,
                maximum: 2
            })
        ));
        assert_eq!(tracker.last_generation("first"), Some(4));
        assert_eq!(tracker.last_generation("second"), Some(8));
        assert_eq!(tracker.channel_count(), 2);
    }

    #[test]
    fn existing_offsets_remain_monotonic_at_capacity() {
        let tracker = BoundedCacheInvalidationGapTracker::new(1).unwrap();
        tracker.seed("tenant.invalidate", 10).unwrap();
        tracker.seed("tenant.invalidate", 11).unwrap();
        assert_eq!(tracker.last_generation("tenant.invalidate"), Some(11));
        assert!(matches!(
            tracker.seed("tenant.invalidate", 9),
            Err(BoundedInvalidationTrackerError::Payload(
                CacheInvalidationPayloadError::OffsetRegressed { .. }
            ))
        ));
    }

    #[test]
    fn gap_does_not_advance_until_recovery_is_acknowledged() {
        let tracker = BoundedCacheInvalidationGapTracker::new(1).unwrap();
        tracker.seed("tenant.invalidate", 3).unwrap();
        assert_eq!(
            tracker.observe(&event("tenant.invalidate", 5)),
            CacheInvalidationObservation::Gap {
                previous: 3,
                expected: 4,
                received: 5,
            }
        );
        assert_eq!(tracker.last_generation("tenant.invalidate"), Some(3));
        tracker
            .acknowledge_recovery("tenant.invalidate", 5)
            .unwrap();
        assert_eq!(tracker.last_generation("tenant.invalidate"), Some(5));
    }
}
