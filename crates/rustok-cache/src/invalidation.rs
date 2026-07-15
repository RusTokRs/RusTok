use std::collections::HashMap;
use std::sync::{Arc, Mutex};

use crate::CacheInvalidationMessage;

const INVALIDATION_PAYLOAD_VERSION: &str = "v1";
const MAX_INVALIDATION_CHANNEL_BYTES: usize = 256;
const MAX_INVALIDATION_KEY_BYTES: usize = 4 * 1024;
const MAX_ENCODED_INVALIDATION_PAYLOAD_BYTES: usize = 16 * 1024;

/// Invalidation payload carrying a monotonic generation from a durable domain sequence.
///
/// The sequence is supplied by the caller (for example an outbox/event offset). This type
/// does not invent process-local ordering and therefore remains valid across instances.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VersionedCacheInvalidation {
    pub channel: String,
    pub key: String,
    pub generation: u64,
    pub emitted_at_unix_ms: u64,
}

impl VersionedCacheInvalidation {
    pub fn new(
        channel: impl Into<String>,
        key: impl Into<String>,
        generation: u64,
        emitted_at_unix_ms: u64,
    ) -> Result<Self, CacheInvalidationPayloadError> {
        let event = Self {
            channel: channel.into(),
            key: key.into(),
            generation,
            emitted_at_unix_ms,
        };
        event.validate()?;
        Ok(event)
    }

    pub fn to_message(&self) -> Result<CacheInvalidationMessage, CacheInvalidationPayloadError> {
        self.validate()?;
        let payload = format!(
            "{INVALIDATION_PAYLOAD_VERSION}:{}:{}:{}",
            self.generation,
            self.emitted_at_unix_ms,
            hex::encode(self.key.as_bytes())
        );
        validate_payload_length(payload.len())?;
        Ok(CacheInvalidationMessage::new(self.channel.clone(), payload))
    }

    pub fn from_message(
        message: &CacheInvalidationMessage,
    ) -> Result<Self, CacheInvalidationPayloadError> {
        validate_channel(&message.channel)?;
        validate_payload_length(message.key.len())?;

        let mut parts = message.key.splitn(4, ':');
        let version = parts
            .next()
            .ok_or(CacheInvalidationPayloadError::MalformedPayload)?;
        if version != INVALIDATION_PAYLOAD_VERSION {
            return Err(CacheInvalidationPayloadError::UnsupportedVersion(
                version.to_string(),
            ));
        }
        let generation = parts
            .next()
            .ok_or(CacheInvalidationPayloadError::MalformedPayload)?
            .parse::<u64>()
            .map_err(|_| CacheInvalidationPayloadError::InvalidGeneration)?;
        let emitted_at_unix_ms = parts
            .next()
            .ok_or(CacheInvalidationPayloadError::MalformedPayload)?
            .parse::<u64>()
            .map_err(|_| CacheInvalidationPayloadError::InvalidTimestamp)?;
        let key_hex = parts
            .next()
            .ok_or(CacheInvalidationPayloadError::MalformedPayload)?;
        if key_hex.len() > MAX_INVALIDATION_KEY_BYTES.saturating_mul(2) {
            return Err(CacheInvalidationPayloadError::KeyTooLarge {
                length: key_hex.len().saturating_add(1) / 2,
                maximum: MAX_INVALIDATION_KEY_BYTES,
            });
        }
        let key_bytes =
            hex::decode(key_hex).map_err(|_| CacheInvalidationPayloadError::InvalidKeyEncoding)?;
        let key = String::from_utf8(key_bytes)
            .map_err(|_| CacheInvalidationPayloadError::InvalidKeyEncoding)?;

        Self::new(message.channel.clone(), key, generation, emitted_at_unix_ms)
    }

    fn validate(&self) -> Result<(), CacheInvalidationPayloadError> {
        validate_channel(&self.channel)?;
        if self.key.trim().is_empty() {
            return Err(CacheInvalidationPayloadError::EmptyKey);
        }
        if self.key.len() > MAX_INVALIDATION_KEY_BYTES {
            return Err(CacheInvalidationPayloadError::KeyTooLarge {
                length: self.key.len(),
                maximum: MAX_INVALIDATION_KEY_BYTES,
            });
        }
        if self.generation == 0 {
            return Err(CacheInvalidationPayloadError::ZeroGeneration);
        }
        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CacheInvalidationPayloadError {
    EmptyChannel,
    ChannelTooLarge { length: usize, maximum: usize },
    EmptyKey,
    KeyTooLarge { length: usize, maximum: usize },
    PayloadTooLarge { length: usize, maximum: usize },
    ZeroGeneration,
    OffsetRegressed { current: u64, proposed: u64 },
    AcknowledgementNotContiguous { current: Option<u64>, proposed: u64 },
    MalformedPayload,
    UnsupportedVersion(String),
    InvalidGeneration,
    InvalidTimestamp,
    InvalidKeyEncoding,
}

impl std::fmt::Display for CacheInvalidationPayloadError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::EmptyChannel => write!(formatter, "invalidation channel must not be empty"),
            Self::ChannelTooLarge { length, maximum } => write!(
                formatter,
                "invalidation channel is {length} bytes; maximum is {maximum}"
            ),
            Self::EmptyKey => write!(formatter, "invalidation key must not be empty"),
            Self::KeyTooLarge { length, maximum } => write!(
                formatter,
                "invalidation key is {length} bytes; maximum is {maximum}"
            ),
            Self::PayloadTooLarge { length, maximum } => write!(
                formatter,
                "encoded invalidation payload is {length} bytes; maximum is {maximum}"
            ),
            Self::ZeroGeneration => write!(formatter, "invalidation generation must be non-zero"),
            Self::OffsetRegressed { current, proposed } => write!(
                formatter,
                "invalidation offset cannot regress from {current} to {proposed}"
            ),
            Self::AcknowledgementNotContiguous {
                current: Some(current),
                proposed,
            } => write!(
                formatter,
                "applied invalidation acknowledgement must repeat {current} or advance to the next generation; proposed {proposed}"
            ),
            Self::AcknowledgementNotContiguous {
                current: None,
                proposed,
            } => write!(
                formatter,
                "applied invalidation acknowledgement {proposed} requires a seeded offset or acknowledged recovery"
            ),
            Self::MalformedPayload => write!(formatter, "malformed versioned invalidation payload"),
            Self::UnsupportedVersion(version) => {
                write!(
                    formatter,
                    "unsupported invalidation payload version {version:?}"
                )
            }
            Self::InvalidGeneration => write!(formatter, "invalid invalidation generation"),
            Self::InvalidTimestamp => write!(formatter, "invalid invalidation timestamp"),
            Self::InvalidKeyEncoding => write!(formatter, "invalid invalidation key encoding"),
        }
    }
}

impl std::error::Error for CacheInvalidationPayloadError {}

/// Process-local observer for a durable monotonic invalidation generation.
///
/// Consumers seed the tracker from their persisted durable offset before observing live events.
/// Observation never advances the acknowledged offset. A caller must invoke `acknowledge_applied`
/// after an in-order handler succeeds, or `acknowledge_recovery` after rebuilding through an
/// unverified/gapped generation. This prevents failed work from being hidden by a later delivery.
#[derive(Clone, Default)]
pub struct CacheInvalidationGapTracker {
    last_by_channel: Arc<Mutex<HashMap<String, u64>>>,
}

impl CacheInvalidationGapTracker {
    /// Restore the last durably acknowledged generation for a channel.
    ///
    /// Seeding with zero is valid when the durable sequence is known to begin at one. A seed may
    /// advance or repeat the current offset, but never lower it.
    pub fn seed(
        &self,
        channel: impl Into<String>,
        last_generation: u64,
    ) -> Result<Option<u64>, CacheInvalidationPayloadError> {
        let channel = channel.into();
        validate_channel(&channel)?;
        self.advance_monotonically(channel, last_generation)
    }

    /// Confirm that an in-order invalidation handler completed successfully.
    ///
    /// Repeating the current offset is idempotent. Any forward jump larger than one is rejected;
    /// callers must use `acknowledge_recovery` after rebuilding through a gap.
    pub fn acknowledge_applied(
        &self,
        channel: impl Into<String>,
        applied_generation: u64,
    ) -> Result<Option<u64>, CacheInvalidationPayloadError> {
        let channel = channel.into();
        validate_channel(&channel)?;
        self.acknowledge_contiguous(channel, applied_generation)
    }

    /// Confirm that fail-safe recovery for an unverified/gapped event completed successfully.
    pub fn acknowledge_recovery(
        &self,
        channel: impl Into<String>,
        recovered_through_generation: u64,
    ) -> Result<Option<u64>, CacheInvalidationPayloadError> {
        let channel = channel.into();
        validate_channel(&channel)?;
        self.advance_monotonically(channel, recovered_through_generation)
    }

    pub fn observe(&self, event: &VersionedCacheInvalidation) -> CacheInvalidationObservation {
        let generations = self
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

    fn acknowledge_contiguous(
        &self,
        channel: String,
        proposed: u64,
    ) -> Result<Option<u64>, CacheInvalidationPayloadError> {
        let mut generations = self
            .last_by_channel
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        let Some(current) = generations.get(&channel).copied() else {
            return Err(
                CacheInvalidationPayloadError::AcknowledgementNotContiguous {
                    current: None,
                    proposed,
                },
            );
        };
        if proposed == current {
            return Ok(Some(current));
        }
        if current.checked_add(1) != Some(proposed) {
            return Err(
                CacheInvalidationPayloadError::AcknowledgementNotContiguous {
                    current: Some(current),
                    proposed,
                },
            );
        }
        Ok(generations.insert(channel, proposed))
    }

    fn advance_monotonically(
        &self,
        channel: String,
        proposed: u64,
    ) -> Result<Option<u64>, CacheInvalidationPayloadError> {
        let mut generations = self
            .last_by_channel
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        if let Some(current) = generations.get(&channel).copied() {
            if proposed < current {
                return Err(CacheInvalidationPayloadError::OffsetRegressed { current, proposed });
            }
        }
        Ok(generations.insert(channel, proposed))
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CacheInvalidationObservation {
    UnverifiedFirst {
        generation: u64,
    },
    InOrder {
        generation: u64,
    },
    Duplicate {
        generation: u64,
    },
    Stale {
        last: u64,
        received: u64,
    },
    Gap {
        previous: u64,
        expected: u64,
        received: u64,
    },
}

impl CacheInvalidationObservation {
    pub fn requires_recovery(self) -> bool {
        matches!(self, Self::UnverifiedFirst { .. } | Self::Gap { .. })
    }

    /// Only a proven contiguous event is safe to apply directly.
    pub fn should_apply(self) -> bool {
        matches!(self, Self::InOrder { .. })
    }

    pub fn recovery_generation(self) -> Option<u64> {
        match self {
            Self::UnverifiedFirst { generation } => Some(generation),
            Self::Gap { received, .. } => Some(received),
            _ => None,
        }
    }
}

fn validate_channel(channel: &str) -> Result<(), CacheInvalidationPayloadError> {
    if channel.trim().is_empty() {
        return Err(CacheInvalidationPayloadError::EmptyChannel);
    }
    if channel.len() > MAX_INVALIDATION_CHANNEL_BYTES {
        return Err(CacheInvalidationPayloadError::ChannelTooLarge {
            length: channel.len(),
            maximum: MAX_INVALIDATION_CHANNEL_BYTES,
        });
    }
    Ok(())
}

fn validate_payload_length(length: usize) -> Result<(), CacheInvalidationPayloadError> {
    if length > MAX_ENCODED_INVALIDATION_PAYLOAD_BYTES {
        return Err(CacheInvalidationPayloadError::PayloadTooLarge {
            length,
            maximum: MAX_ENCODED_INVALIDATION_PAYLOAD_BYTES,
        });
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn event(generation: u64) -> VersionedCacheInvalidation {
        VersionedCacheInvalidation::new("tenant.invalidate", "tenant:42", generation, 1_000)
            .unwrap()
    }

    #[test]
    fn payload_round_trip_preserves_delimiter_characters_in_key() {
        let original = VersionedCacheInvalidation::new(
            "tenant.invalidate",
            "tenant:v2:id:42|negative:key",
            7,
            1_234,
        )
        .unwrap();
        let decoded =
            VersionedCacheInvalidation::from_message(&original.to_message().unwrap()).unwrap();

        assert_eq!(decoded, original);
    }

    #[test]
    fn unseeded_first_event_requires_acknowledged_recovery() {
        let tracker = CacheInvalidationGapTracker::default();
        let observation = tracker.observe(&event(10));

        assert_eq!(
            observation,
            CacheInvalidationObservation::UnverifiedFirst { generation: 10 }
        );
        assert!(observation.requires_recovery());
        assert!(!observation.should_apply());
        assert_eq!(observation.recovery_generation(), Some(10));
        assert_eq!(tracker.last_generation("tenant.invalidate"), None);

        tracker
            .acknowledge_recovery("tenant.invalidate", 10)
            .unwrap();
        assert_eq!(
            tracker.observe(&event(11)),
            CacheInvalidationObservation::InOrder { generation: 11 }
        );
        assert_eq!(tracker.last_generation("tenant.invalidate"), Some(10));
        tracker
            .acknowledge_applied("tenant.invalidate", 11)
            .unwrap();
        assert_eq!(tracker.last_generation("tenant.invalidate"), Some(11));
    }

    #[test]
    fn in_order_event_remains_retryable_until_acknowledged() {
        let tracker = CacheInvalidationGapTracker::default();
        tracker.seed("tenant.invalidate", 9).unwrap();

        let expected = CacheInvalidationObservation::InOrder { generation: 10 };
        assert_eq!(tracker.observe(&event(10)), expected);
        assert_eq!(tracker.observe(&event(10)), expected);
        assert_eq!(tracker.last_generation("tenant.invalidate"), Some(9));

        tracker
            .acknowledge_applied("tenant.invalidate", 10)
            .unwrap();
        assert_eq!(
            tracker.observe(&event(10)),
            CacheInvalidationObservation::Duplicate { generation: 10 }
        );
    }

    #[test]
    fn applied_acknowledgement_rejects_unseeded_or_skipped_offsets() {
        let tracker = CacheInvalidationGapTracker::default();
        assert_eq!(
            tracker
                .acknowledge_applied("tenant.invalidate", 1)
                .unwrap_err(),
            CacheInvalidationPayloadError::AcknowledgementNotContiguous {
                current: None,
                proposed: 1,
            }
        );

        tracker.seed("tenant.invalidate", 10).unwrap();
        assert_eq!(
            tracker
                .acknowledge_applied("tenant.invalidate", 12)
                .unwrap_err(),
            CacheInvalidationPayloadError::AcknowledgementNotContiguous {
                current: Some(10),
                proposed: 12,
            }
        );
        assert_eq!(tracker.last_generation("tenant.invalidate"), Some(10));
        assert_eq!(
            tracker
                .acknowledge_applied("tenant.invalidate", 10)
                .unwrap(),
            Some(10)
        );
        assert_eq!(
            tracker
                .acknowledge_applied("tenant.invalidate", 11)
                .unwrap(),
            Some(10)
        );
        assert_eq!(tracker.last_generation("tenant.invalidate"), Some(11));
    }

    #[test]
    fn gap_does_not_advance_until_recovery_is_acknowledged() {
        let tracker = CacheInvalidationGapTracker::default();
        tracker.seed("tenant.invalidate", 9).unwrap();
        assert_eq!(
            tracker.observe(&event(10)),
            CacheInvalidationObservation::InOrder { generation: 10 }
        );
        assert_eq!(tracker.last_generation("tenant.invalidate"), Some(9));
        tracker
            .acknowledge_applied("tenant.invalidate", 10)
            .unwrap();

        let gap = tracker.observe(&event(14));
        assert_eq!(
            gap,
            CacheInvalidationObservation::Gap {
                previous: 10,
                expected: 11,
                received: 14,
            }
        );
        assert!(gap.requires_recovery());
        assert_eq!(tracker.last_generation("tenant.invalidate"), Some(10));
        assert_eq!(tracker.observe(&event(14)), gap);

        tracker
            .acknowledge_recovery("tenant.invalidate", 14)
            .unwrap();
        assert_eq!(
            tracker.observe(&event(15)),
            CacheInvalidationObservation::InOrder { generation: 15 }
        );
        assert_eq!(tracker.last_generation("tenant.invalidate"), Some(14));
    }

    #[test]
    fn duplicate_stale_and_offset_regression_are_rejected_safely() {
        let tracker = CacheInvalidationGapTracker::default();
        tracker.seed("tenant.invalidate", 11).unwrap();
        assert_eq!(
            tracker.observe(&event(11)),
            CacheInvalidationObservation::Duplicate { generation: 11 }
        );
        assert_eq!(
            tracker.observe(&event(9)),
            CacheInvalidationObservation::Stale {
                last: 11,
                received: 9,
            }
        );
        assert_eq!(
            tracker.seed("tenant.invalidate", 10).unwrap_err(),
            CacheInvalidationPayloadError::OffsetRegressed {
                current: 11,
                proposed: 10,
            }
        );
    }

    #[test]
    fn rejects_oversized_channel_payload_and_key_before_allocation() {
        let oversized_channel = "c".repeat(MAX_INVALIDATION_CHANNEL_BYTES + 1);
        assert!(matches!(
            VersionedCacheInvalidation::new(oversized_channel, "key", 1, 1).unwrap_err(),
            CacheInvalidationPayloadError::ChannelTooLarge { .. }
        ));

        let oversized_payload = CacheInvalidationMessage::new(
            "tenant.invalidate",
            "x".repeat(MAX_ENCODED_INVALIDATION_PAYLOAD_BYTES + 1),
        );
        assert!(matches!(
            VersionedCacheInvalidation::from_message(&oversized_payload).unwrap_err(),
            CacheInvalidationPayloadError::PayloadTooLarge { .. }
        ));

        assert!(matches!(
            VersionedCacheInvalidation::new(
                "tenant.invalidate",
                "k".repeat(MAX_INVALIDATION_KEY_BYTES + 1),
                1,
                1,
            )
            .unwrap_err(),
            CacheInvalidationPayloadError::KeyTooLarge { .. }
        ));
    }

    #[test]
    fn rejects_zero_generation_and_unknown_payload_version() {
        assert_eq!(
            VersionedCacheInvalidation::new("tenant.invalidate", "key", 0, 1).unwrap_err(),
            CacheInvalidationPayloadError::ZeroGeneration
        );
        assert_eq!(
            VersionedCacheInvalidation::from_message(&CacheInvalidationMessage::new(
                "tenant.invalidate",
                "v2:1:1:6b6579",
            ))
            .unwrap_err(),
            CacheInvalidationPayloadError::UnsupportedVersion("v2".to_string())
        );
    }
}
