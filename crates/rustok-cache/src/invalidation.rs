use std::collections::HashMap;
use std::sync::{Arc, Mutex};

use crate::CacheInvalidationMessage;

const INVALIDATION_PAYLOAD_VERSION: &str = "v1";
const MAX_INVALIDATION_KEY_BYTES: usize = 4 * 1024;

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
        Ok(CacheInvalidationMessage::new(
            self.channel.clone(),
            format!(
                "{INVALIDATION_PAYLOAD_VERSION}:{}:{}:{}",
                self.generation,
                self.emitted_at_unix_ms,
                hex::encode(self.key.as_bytes())
            ),
        ))
    }

    pub fn from_message(
        message: &CacheInvalidationMessage,
    ) -> Result<Self, CacheInvalidationPayloadError> {
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
        let key_bytes = hex::decode(key_hex)
            .map_err(|_| CacheInvalidationPayloadError::InvalidKeyEncoding)?;
        let key = String::from_utf8(key_bytes)
            .map_err(|_| CacheInvalidationPayloadError::InvalidKeyEncoding)?;

        Self::new(
            message.channel.clone(),
            key,
            generation,
            emitted_at_unix_ms,
        )
    }

    fn validate(&self) -> Result<(), CacheInvalidationPayloadError> {
        if self.channel.trim().is_empty() {
            return Err(CacheInvalidationPayloadError::EmptyChannel);
        }
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
    EmptyKey,
    KeyTooLarge { length: usize, maximum: usize },
    ZeroGeneration,
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
            Self::EmptyKey => write!(formatter, "invalidation key must not be empty"),
            Self::KeyTooLarge { length, maximum } => write!(
                formatter,
                "invalidation key is {length} bytes; maximum is {maximum}"
            ),
            Self::ZeroGeneration => write!(formatter, "invalidation generation must be non-zero"),
            Self::MalformedPayload => write!(formatter, "malformed versioned invalidation payload"),
            Self::UnsupportedVersion(version) => {
                write!(formatter, "unsupported invalidation payload version {version:?}")
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
/// On `Gap`, the caller should clear/rebuild the affected cache namespace before trusting
/// later events. The tracker advances to the received generation so the stream can continue
/// after that recovery action.
#[derive(Clone, Default)]
pub struct CacheInvalidationGapTracker {
    last_by_channel: Arc<Mutex<HashMap<String, u64>>>,
}

impl CacheInvalidationGapTracker {
    pub fn observe(
        &self,
        event: &VersionedCacheInvalidation,
    ) -> CacheInvalidationObservation {
        let mut generations = self
            .last_by_channel
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);

        let Some(previous) = generations.get(&event.channel).copied() else {
            generations.insert(event.channel.clone(), event.generation);
            return CacheInvalidationObservation::First {
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
            Some(expected) => {
                generations.insert(event.channel.clone(), event.generation);
                CacheInvalidationObservation::Gap {
                    previous,
                    expected,
                    received: event.generation,
                }
            }
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
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CacheInvalidationObservation {
    First {
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
        matches!(self, Self::Gap { .. })
    }

    pub fn should_apply(self) -> bool {
        matches!(self, Self::First { .. } | Self::InOrder { .. } | Self::Gap { .. })
    }
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
        let decoded = VersionedCacheInvalidation::from_message(&original.to_message().unwrap())
            .unwrap();

        assert_eq!(decoded, original);
    }

    #[test]
    fn tracker_detects_gap_duplicate_and_stale_events() {
        let tracker = CacheInvalidationGapTracker::default();

        assert_eq!(
            tracker.observe(&event(10)),
            CacheInvalidationObservation::First { generation: 10 }
        );
        assert_eq!(
            tracker.observe(&event(11)),
            CacheInvalidationObservation::InOrder { generation: 11 }
        );
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
        let gap = tracker.observe(&event(14));
        assert_eq!(
            gap,
            CacheInvalidationObservation::Gap {
                previous: 11,
                expected: 12,
                received: 14,
            }
        );
        assert!(gap.requires_recovery());
        assert_eq!(tracker.last_generation("tenant.invalidate"), Some(14));
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
