use std::io::{self, Write};

use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};

pub const CACHE_ENVELOPE_FORMAT_VERSION: u16 = 1;
pub const DEFAULT_MAX_CACHE_ENVELOPE_BYTES: usize = 8 * 1024 * 1024;
const MAX_SOURCE_REVISION_BYTES: usize = 256;

/// Versioned serialized cache value with explicit freshness metadata.
///
/// The envelope separates the wire-format version from the domain schema version. A
/// deserializer must provide the expected schema version; mismatched values are treated as
/// misses and should be invalidated by the caller rather than repeatedly decoded.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CacheEnvelope<T> {
    format_version: u16,
    schema_version: u32,
    generated_at_unix_ms: u64,
    source_revision: Option<String>,
    soft_expires_at_unix_ms: Option<u64>,
    hard_expires_at_unix_ms: Option<u64>,
    payload: T,
}

impl<T> CacheEnvelope<T> {
    pub fn new(
        schema_version: u32,
        generated_at_unix_ms: u64,
        payload: T,
    ) -> Result<Self, CacheEnvelopeError> {
        if schema_version == 0 {
            return Err(CacheEnvelopeError::ZeroSchemaVersion);
        }

        Ok(Self {
            format_version: CACHE_ENVELOPE_FORMAT_VERSION,
            schema_version,
            generated_at_unix_ms,
            source_revision: None,
            soft_expires_at_unix_ms: None,
            hard_expires_at_unix_ms: None,
            payload,
        })
    }

    pub fn with_source_revision(
        mut self,
        revision: impl Into<String>,
    ) -> Result<Self, CacheEnvelopeError> {
        let revision = revision.into();
        if revision.trim().is_empty() {
            return Err(CacheEnvelopeError::EmptySourceRevision);
        }
        if revision.len() > MAX_SOURCE_REVISION_BYTES {
            return Err(CacheEnvelopeError::SourceRevisionTooLong {
                length: revision.len(),
                maximum: MAX_SOURCE_REVISION_BYTES,
            });
        }
        self.source_revision = Some(revision);
        Ok(self)
    }

    /// Configure optional stale-while-revalidate boundaries.
    ///
    /// A soft expiry is valid only together with a hard expiry. Both boundaries must be at
    /// or after generation time, and soft expiry must not exceed hard expiry.
    pub fn with_expirations(
        mut self,
        soft_expires_at_unix_ms: Option<u64>,
        hard_expires_at_unix_ms: Option<u64>,
    ) -> Result<Self, CacheEnvelopeError> {
        validate_expirations(
            self.generated_at_unix_ms,
            soft_expires_at_unix_ms,
            hard_expires_at_unix_ms,
        )?;
        self.soft_expires_at_unix_ms = soft_expires_at_unix_ms;
        self.hard_expires_at_unix_ms = hard_expires_at_unix_ms;
        Ok(self)
    }

    pub fn format_version(&self) -> u16 {
        self.format_version
    }

    pub fn schema_version(&self) -> u32 {
        self.schema_version
    }

    pub fn generated_at_unix_ms(&self) -> u64 {
        self.generated_at_unix_ms
    }

    pub fn source_revision(&self) -> Option<&str> {
        self.source_revision.as_deref()
    }

    pub fn soft_expires_at_unix_ms(&self) -> Option<u64> {
        self.soft_expires_at_unix_ms
    }

    pub fn hard_expires_at_unix_ms(&self) -> Option<u64> {
        self.hard_expires_at_unix_ms
    }

    pub fn payload(&self) -> &T {
        &self.payload
    }

    pub fn into_payload(self) -> T {
        self.payload
    }

    pub fn is_soft_expired(&self, now_unix_ms: u64) -> bool {
        self.soft_expires_at_unix_ms
            .is_some_and(|expires_at| now_unix_ms >= expires_at)
    }

    pub fn is_hard_expired(&self, now_unix_ms: u64) -> bool {
        self.hard_expires_at_unix_ms
            .is_some_and(|expires_at| now_unix_ms >= expires_at)
    }

    pub fn freshness(&self, now_unix_ms: u64) -> CacheEnvelopeFreshness {
        if self.is_hard_expired(now_unix_ms) {
            CacheEnvelopeFreshness::HardExpired
        } else if self.is_soft_expired(now_unix_ms) {
            CacheEnvelopeFreshness::Stale
        } else {
            CacheEnvelopeFreshness::Fresh
        }
    }

    fn validate_metadata(&self, expected_schema_version: u32) -> Result<(), CacheEnvelopeError> {
        if self.format_version != CACHE_ENVELOPE_FORMAT_VERSION {
            return Err(CacheEnvelopeError::UnsupportedFormatVersion {
                found: self.format_version,
                supported: CACHE_ENVELOPE_FORMAT_VERSION,
            });
        }
        if self.schema_version != expected_schema_version {
            return Err(CacheEnvelopeError::SchemaVersionMismatch {
                found: self.schema_version,
                expected: expected_schema_version,
            });
        }
        validate_expirations(
            self.generated_at_unix_ms,
            self.soft_expires_at_unix_ms,
            self.hard_expires_at_unix_ms,
        )
    }
}

impl<T> CacheEnvelope<T>
where
    T: Serialize,
{
    pub fn encode(&self) -> Result<Vec<u8>, CacheEnvelopeError> {
        self.encode_with_limit(DEFAULT_MAX_CACHE_ENVELOPE_BYTES)
    }

    pub fn encode_with_limit(
        &self,
        max_encoded_bytes: usize,
    ) -> Result<Vec<u8>, CacheEnvelopeError> {
        if max_encoded_bytes == 0 {
            return Err(CacheEnvelopeError::ZeroSizeLimit);
        }

        // Measure before allocating the output buffer. Checking a fully allocated Vec after
        // serialization defeats the size limit for large or attacker-controlled payloads.
        let encoded_len =
            postcard::serialize_with_flavor(self, postcard::ser_flavors::Size::default())
                .map_err(|error| CacheEnvelopeError::Encode(error.to_string()))?;
        if encoded_len > max_encoded_bytes {
            return Err(CacheEnvelopeError::TooLarge {
                length: encoded_len,
                maximum: max_encoded_bytes,
            });
        }

        // The writer is still bounded even after the measurement pass. This protects against a
        // custom stateful Serialize implementation producing more bytes on its second traversal.
        let writer = BoundedEnvelopeWriter::new(encoded_len, max_encoded_bytes);
        postcard::to_io(self, writer)
            .map(BoundedEnvelopeWriter::into_inner)
            .map_err(|error| CacheEnvelopeError::Encode(error.to_string()))
    }
}

impl<T> CacheEnvelope<T>
where
    T: DeserializeOwned,
{
    pub fn decode(bytes: &[u8], expected_schema_version: u32) -> Result<Self, CacheEnvelopeError> {
        Self::decode_with_limit(
            bytes,
            expected_schema_version,
            DEFAULT_MAX_CACHE_ENVELOPE_BYTES,
        )
    }

    pub fn decode_with_limit(
        bytes: &[u8],
        expected_schema_version: u32,
        max_encoded_bytes: usize,
    ) -> Result<Self, CacheEnvelopeError> {
        if expected_schema_version == 0 {
            return Err(CacheEnvelopeError::ZeroSchemaVersion);
        }
        if max_encoded_bytes == 0 {
            return Err(CacheEnvelopeError::ZeroSizeLimit);
        }
        if bytes.len() > max_encoded_bytes {
            return Err(CacheEnvelopeError::TooLarge {
                length: bytes.len(),
                maximum: max_encoded_bytes,
            });
        }

        let envelope: Self = postcard::from_bytes(bytes)
            .map_err(|error| CacheEnvelopeError::Decode(error.to_string()))?;
        envelope.validate_metadata(expected_schema_version)?;
        Ok(envelope)
    }
}

struct BoundedEnvelopeWriter {
    bytes: Vec<u8>,
    maximum: usize,
}

impl BoundedEnvelopeWriter {
    fn new(encoded_len: usize, maximum: usize) -> Self {
        Self {
            bytes: Vec::with_capacity(encoded_len.min(maximum)),
            maximum,
        }
    }

    fn into_inner(self) -> Vec<u8> {
        self.bytes
    }
}

impl Write for BoundedEnvelopeWriter {
    fn write(&mut self, buffer: &[u8]) -> io::Result<usize> {
        let next_len = self
            .bytes
            .len()
            .checked_add(buffer.len())
            .ok_or_else(|| io::Error::other("cache envelope output length overflow"))?;
        if next_len > self.maximum {
            return Err(io::Error::other("cache envelope output exceeds size limit"));
        }
        self.bytes.extend_from_slice(buffer);
        Ok(buffer.len())
    }

    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CacheEnvelopeFreshness {
    Fresh,
    Stale,
    HardExpired,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CacheEnvelopeError {
    ZeroSchemaVersion,
    ZeroSizeLimit,
    EmptySourceRevision,
    SourceRevisionTooLong {
        length: usize,
        maximum: usize,
    },
    SoftExpiryRequiresHardExpiry,
    ExpiryBeforeGeneration {
        boundary: &'static str,
        generated_at_unix_ms: u64,
        expires_at_unix_ms: u64,
    },
    SoftExpiryAfterHardExpiry {
        soft_unix_ms: u64,
        hard_unix_ms: u64,
    },
    UnsupportedFormatVersion {
        found: u16,
        supported: u16,
    },
    SchemaVersionMismatch {
        found: u32,
        expected: u32,
    },
    TooLarge {
        length: usize,
        maximum: usize,
    },
    Encode(String),
    Decode(String),
}

impl std::fmt::Display for CacheEnvelopeError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::ZeroSchemaVersion => write!(formatter, "cache schema version must be non-zero"),
            Self::ZeroSizeLimit => write!(formatter, "cache envelope size limit must be non-zero"),
            Self::EmptySourceRevision => {
                write!(formatter, "cache source revision must not be empty")
            }
            Self::SourceRevisionTooLong { length, maximum } => write!(
                formatter,
                "cache source revision is {length} bytes; maximum is {maximum}"
            ),
            Self::SoftExpiryRequiresHardExpiry => {
                write!(formatter, "soft cache expiry requires a hard expiry")
            }
            Self::ExpiryBeforeGeneration {
                boundary,
                generated_at_unix_ms,
                expires_at_unix_ms,
            } => write!(
                formatter,
                "cache {boundary} expiry {expires_at_unix_ms} precedes generation {generated_at_unix_ms}"
            ),
            Self::SoftExpiryAfterHardExpiry {
                soft_unix_ms,
                hard_unix_ms,
            } => write!(
                formatter,
                "soft cache expiry {soft_unix_ms} exceeds hard expiry {hard_unix_ms}"
            ),
            Self::UnsupportedFormatVersion { found, supported } => write!(
                formatter,
                "unsupported cache envelope format version {found}; supported {supported}"
            ),
            Self::SchemaVersionMismatch { found, expected } => write!(
                formatter,
                "cache schema version {found} does not match expected {expected}"
            ),
            Self::TooLarge { length, maximum } => write!(
                formatter,
                "cache envelope is {length} bytes; maximum is {maximum}"
            ),
            Self::Encode(message) => write!(formatter, "cache envelope encode failed: {message}"),
            Self::Decode(message) => write!(formatter, "cache envelope decode failed: {message}"),
        }
    }
}

impl std::error::Error for CacheEnvelopeError {}

fn validate_expirations(
    generated_at_unix_ms: u64,
    soft_expires_at_unix_ms: Option<u64>,
    hard_expires_at_unix_ms: Option<u64>,
) -> Result<(), CacheEnvelopeError> {
    if soft_expires_at_unix_ms.is_some() && hard_expires_at_unix_ms.is_none() {
        return Err(CacheEnvelopeError::SoftExpiryRequiresHardExpiry);
    }

    if let Some(soft) = soft_expires_at_unix_ms {
        if soft < generated_at_unix_ms {
            return Err(CacheEnvelopeError::ExpiryBeforeGeneration {
                boundary: "soft",
                generated_at_unix_ms,
                expires_at_unix_ms: soft,
            });
        }
    }

    if let Some(hard) = hard_expires_at_unix_ms {
        if hard < generated_at_unix_ms {
            return Err(CacheEnvelopeError::ExpiryBeforeGeneration {
                boundary: "hard",
                generated_at_unix_ms,
                expires_at_unix_ms: hard,
            });
        }
    }

    if let (Some(soft), Some(hard)) = (soft_expires_at_unix_ms, hard_expires_at_unix_ms) {
        if soft > hard {
            return Err(CacheEnvelopeError::SoftExpiryAfterHardExpiry {
                soft_unix_ms: soft,
                hard_unix_ms: hard,
            });
        }
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn envelope_round_trip_preserves_version_and_revision() {
        let envelope = CacheEnvelope::new(3, 1_000, vec![1_u8, 2, 3])
            .unwrap()
            .with_source_revision("db-row-version-42")
            .unwrap()
            .with_expirations(Some(2_000), Some(3_000))
            .unwrap();

        let encoded = envelope.encode().unwrap();
        let decoded = CacheEnvelope::<Vec<u8>>::decode(&encoded, 3).unwrap();

        assert_eq!(decoded, envelope);
        assert_eq!(decoded.source_revision(), Some("db-row-version-42"));
    }

    #[test]
    fn schema_mismatch_is_not_silently_deserialized() {
        let encoded = CacheEnvelope::new(2, 1_000, "value".to_string())
            .unwrap()
            .encode()
            .unwrap();

        assert_eq!(
            CacheEnvelope::<String>::decode(&encoded, 3).unwrap_err(),
            CacheEnvelopeError::SchemaVersionMismatch {
                found: 2,
                expected: 3,
            }
        );
    }

    #[test]
    fn encode_rejects_oversized_output_before_allocating_it() {
        let envelope = CacheEnvelope::new(1, 1_000, vec![7_u8; 256]).unwrap();
        let measured =
            postcard::serialize_with_flavor(&envelope, postcard::ser_flavors::Size::default())
                .unwrap();

        assert_eq!(
            envelope.encode_with_limit(64).unwrap_err(),
            CacheEnvelopeError::TooLarge {
                length: measured,
                maximum: 64,
            }
        );
    }

    #[test]
    fn bounded_writer_never_accepts_output_past_limit() {
        let mut writer = BoundedEnvelopeWriter::new(4, 4);
        writer.write_all(b"1234").unwrap();
        assert!(writer.write_all(b"5").is_err());
        assert_eq!(writer.into_inner(), b"1234");
    }

    #[test]
    fn decode_rejects_oversized_input_before_deserialization() {
        let bytes = vec![0_u8; 65];
        assert_eq!(
            CacheEnvelope::<Vec<u8>>::decode_with_limit(&bytes, 1, 64).unwrap_err(),
            CacheEnvelopeError::TooLarge {
                length: 65,
                maximum: 64,
            }
        );
    }

    #[test]
    fn validates_soft_and_hard_expiration_order() {
        assert_eq!(
            CacheEnvelope::new(1, 1_000, "value")
                .unwrap()
                .with_expirations(Some(2_000), None)
                .unwrap_err(),
            CacheEnvelopeError::SoftExpiryRequiresHardExpiry
        );
        assert_eq!(
            CacheEnvelope::new(1, 1_000, "value")
                .unwrap()
                .with_expirations(Some(3_000), Some(2_000))
                .unwrap_err(),
            CacheEnvelopeError::SoftExpiryAfterHardExpiry {
                soft_unix_ms: 3_000,
                hard_unix_ms: 2_000,
            }
        );
    }

    #[test]
    fn reports_fresh_stale_and_hard_expired_states() {
        let envelope = CacheEnvelope::new(1, 1_000, "value")
            .unwrap()
            .with_expirations(Some(2_000), Some(3_000))
            .unwrap();

        assert_eq!(envelope.freshness(1_999), CacheEnvelopeFreshness::Fresh);
        assert_eq!(envelope.freshness(2_000), CacheEnvelopeFreshness::Stale);
        assert_eq!(
            envelope.freshness(3_000),
            CacheEnvelopeFreshness::HardExpired
        );
    }
}
