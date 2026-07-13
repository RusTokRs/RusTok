use std::io::{self, Write};

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use uuid::Uuid;

use crate::{CacheInvalidationPayloadError, VersionedCacheInvalidation};

pub const DURABLE_CACHE_INVALIDATION_FORMAT_VERSION: u16 = 1;
pub const DEFAULT_MAX_DURABLE_INVALIDATION_BYTES: usize = 16 * 1024;
pub const MAX_DURABLE_INVALIDATION_CAUSE_BYTES: usize = 256;
pub const MAX_DURABLE_INVALIDATION_TRACE_ID_BYTES: usize = 128;

/// Versioned record suitable for transport inside a transactional outbox domain event.
///
/// The record does not invent ordering. `generation` must come from the same durable sequence as
/// the mutation (for example an outbox envelope id mapped to a monotonic stream offset or a
/// transactionally incremented namespace generation).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DurableCacheInvalidationRecord {
    format_version: u16,
    source_event_id: Uuid,
    tenant_id: Option<Uuid>,
    channel: String,
    key: String,
    generation: u64,
    emitted_at_unix_ms: u64,
    cause: String,
    trace_id: Option<String>,
}

impl DurableCacheInvalidationRecord {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        source_event_id: Uuid,
        tenant_id: Option<Uuid>,
        channel: impl Into<String>,
        key: impl Into<String>,
        generation: u64,
        emitted_at_unix_ms: u64,
        cause: impl Into<String>,
        trace_id: Option<String>,
    ) -> Result<Self, DurableCacheInvalidationError> {
        let record = Self {
            format_version: DURABLE_CACHE_INVALIDATION_FORMAT_VERSION,
            source_event_id,
            tenant_id,
            channel: channel.into(),
            key: key.into(),
            generation,
            emitted_at_unix_ms,
            cause: cause.into(),
            trace_id,
        };
        record.validate()?;
        Ok(record)
    }

    pub fn format_version(&self) -> u16 {
        self.format_version
    }

    pub fn source_event_id(&self) -> Uuid {
        self.source_event_id
    }

    pub fn tenant_id(&self) -> Option<Uuid> {
        self.tenant_id
    }

    pub fn channel(&self) -> &str {
        &self.channel
    }

    pub fn key(&self) -> &str {
        &self.key
    }

    pub fn generation(&self) -> u64 {
        self.generation
    }

    pub fn emitted_at_unix_ms(&self) -> u64 {
        self.emitted_at_unix_ms
    }

    pub fn cause(&self) -> &str {
        &self.cause
    }

    pub fn trace_id(&self) -> Option<&str> {
        self.trace_id.as_deref()
    }

    pub fn to_versioned_invalidation(
        &self,
    ) -> Result<VersionedCacheInvalidation, DurableCacheInvalidationError> {
        VersionedCacheInvalidation::new(
            self.channel.clone(),
            self.key.clone(),
            self.generation,
            self.emitted_at_unix_ms,
        )
        .map_err(DurableCacheInvalidationError::Invalidation)
    }

    /// Stable idempotency key without exposing tenant, key, cause or trace data.
    pub fn idempotency_key(&self) -> String {
        let mut digest = Sha256::new();
        digest.update(self.format_version.to_be_bytes());
        digest.update(self.source_event_id.as_bytes());
        match self.tenant_id {
            Some(tenant_id) => {
                digest.update([1]);
                digest.update(tenant_id.as_bytes());
            }
            None => digest.update([0]),
        }
        for value in [self.channel.as_bytes(), self.key.as_bytes(), self.cause.as_bytes()] {
            digest.update((value.len() as u64).to_be_bytes());
            digest.update(value);
        }
        digest.update(self.generation.to_be_bytes());
        digest.update(self.emitted_at_unix_ms.to_be_bytes());
        if let Some(trace_id) = &self.trace_id {
            digest.update((trace_id.len() as u64).to_be_bytes());
            digest.update(trace_id.as_bytes());
        } else {
            digest.update(0_u64.to_be_bytes());
        }
        format!("cache-invalidation:v1:{}", hex::encode(digest.finalize()))
    }

    pub fn encode(&self) -> Result<Vec<u8>, DurableCacheInvalidationError> {
        self.encode_with_limit(DEFAULT_MAX_DURABLE_INVALIDATION_BYTES)
    }

    pub fn encode_with_limit(
        &self,
        maximum: usize,
    ) -> Result<Vec<u8>, DurableCacheInvalidationError> {
        if maximum == 0 {
            return Err(DurableCacheInvalidationError::ZeroSizeLimit);
        }
        self.validate()?;
        let encoded_len = postcard::serialize_with_flavor(
            self,
            postcard::ser_flavors::Size::default(),
        )
        .map_err(|error| DurableCacheInvalidationError::Encode(error.to_string()))?;
        if encoded_len > maximum {
            return Err(DurableCacheInvalidationError::TooLarge {
                length: encoded_len,
                maximum,
            });
        }
        let writer = BoundedRecordWriter::new(encoded_len, maximum);
        postcard::to_io(self, writer)
            .map(BoundedRecordWriter::into_inner)
            .map_err(|error| DurableCacheInvalidationError::Encode(error.to_string()))
    }

    pub fn decode(bytes: &[u8]) -> Result<Self, DurableCacheInvalidationError> {
        Self::decode_with_limit(bytes, DEFAULT_MAX_DURABLE_INVALIDATION_BYTES)
    }

    pub fn decode_with_limit(
        bytes: &[u8],
        maximum: usize,
    ) -> Result<Self, DurableCacheInvalidationError> {
        if maximum == 0 {
            return Err(DurableCacheInvalidationError::ZeroSizeLimit);
        }
        if bytes.len() > maximum {
            return Err(DurableCacheInvalidationError::TooLarge {
                length: bytes.len(),
                maximum,
            });
        }
        let record: Self = postcard::from_bytes(bytes)
            .map_err(|error| DurableCacheInvalidationError::Decode(error.to_string()))?;
        record.validate()?;
        Ok(record)
    }

    fn validate(&self) -> Result<(), DurableCacheInvalidationError> {
        if self.format_version != DURABLE_CACHE_INVALIDATION_FORMAT_VERSION {
            return Err(DurableCacheInvalidationError::UnsupportedFormatVersion {
                found: self.format_version,
                supported: DURABLE_CACHE_INVALIDATION_FORMAT_VERSION,
            });
        }
        if self.source_event_id.is_nil() {
            return Err(DurableCacheInvalidationError::NilSourceEventId);
        }
        if self.tenant_id.is_some_and(|tenant_id| tenant_id.is_nil()) {
            return Err(DurableCacheInvalidationError::NilTenantId);
        }
        if self.emitted_at_unix_ms == 0 {
            return Err(DurableCacheInvalidationError::ZeroTimestamp);
        }
        if self.cause.trim().is_empty() {
            return Err(DurableCacheInvalidationError::EmptyCause);
        }
        if self.cause.len() > MAX_DURABLE_INVALIDATION_CAUSE_BYTES {
            return Err(DurableCacheInvalidationError::CauseTooLong {
                length: self.cause.len(),
                maximum: MAX_DURABLE_INVALIDATION_CAUSE_BYTES,
            });
        }
        if let Some(trace_id) = &self.trace_id {
            if trace_id.trim().is_empty() {
                return Err(DurableCacheInvalidationError::EmptyTraceId);
            }
            if trace_id.len() > MAX_DURABLE_INVALIDATION_TRACE_ID_BYTES {
                return Err(DurableCacheInvalidationError::TraceIdTooLong {
                    length: trace_id.len(),
                    maximum: MAX_DURABLE_INVALIDATION_TRACE_ID_BYTES,
                });
            }
        }
        self.to_versioned_invalidation()?;
        Ok(())
    }
}

struct BoundedRecordWriter {
    bytes: Vec<u8>,
    maximum: usize,
}

impl BoundedRecordWriter {
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

impl Write for BoundedRecordWriter {
    fn write(&mut self, buffer: &[u8]) -> io::Result<usize> {
        let next_len = self
            .bytes
            .len()
            .checked_add(buffer.len())
            .ok_or_else(|| io::Error::other("durable invalidation output length overflow"))?;
        if next_len > self.maximum {
            return Err(io::Error::other(
                "durable invalidation output exceeds size limit",
            ));
        }
        self.bytes.extend_from_slice(buffer);
        Ok(buffer.len())
    }

    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DurableCacheInvalidationError {
    NilSourceEventId,
    NilTenantId,
    ZeroTimestamp,
    EmptyCause,
    CauseTooLong { length: usize, maximum: usize },
    EmptyTraceId,
    TraceIdTooLong { length: usize, maximum: usize },
    ZeroSizeLimit,
    UnsupportedFormatVersion { found: u16, supported: u16 },
    TooLarge { length: usize, maximum: usize },
    Invalidation(CacheInvalidationPayloadError),
    Encode(String),
    Decode(String),
}

impl std::fmt::Display for DurableCacheInvalidationError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::NilSourceEventId => write!(formatter, "source event id must not be nil"),
            Self::NilTenantId => write!(formatter, "tenant id must not be nil"),
            Self::ZeroTimestamp => write!(formatter, "invalidation timestamp must be non-zero"),
            Self::EmptyCause => write!(formatter, "invalidation cause must not be empty"),
            Self::CauseTooLong { length, maximum } => write!(
                formatter,
                "invalidation cause is {length} bytes; maximum is {maximum}"
            ),
            Self::EmptyTraceId => write!(formatter, "trace id must not be empty"),
            Self::TraceIdTooLong { length, maximum } => write!(
                formatter,
                "trace id is {length} bytes; maximum is {maximum}"
            ),
            Self::ZeroSizeLimit => write!(formatter, "durable invalidation size limit must be non-zero"),
            Self::UnsupportedFormatVersion { found, supported } => write!(
                formatter,
                "unsupported durable invalidation format version {found}; supported {supported}"
            ),
            Self::TooLarge { length, maximum } => write!(
                formatter,
                "durable invalidation record is {length} bytes; maximum is {maximum}"
            ),
            Self::Invalidation(error) => write!(formatter, "invalid cache invalidation: {error}"),
            Self::Encode(message) => write!(formatter, "durable invalidation encode failed: {message}"),
            Self::Decode(message) => write!(formatter, "durable invalidation decode failed: {message}"),
        }
    }
}

impl std::error::Error for DurableCacheInvalidationError {}

#[cfg(test)]
mod tests {
    use super::*;

    fn record() -> DurableCacheInvalidationRecord {
        DurableCacheInvalidationRecord::new(
            Uuid::from_u128(7),
            Some(Uuid::from_u128(9)),
            "tenant.invalidate",
            "tenant:42",
            11,
            1_000,
            "tenant.settings.updated",
            Some("0123456789abcdef".to_string()),
        )
        .unwrap()
    }

    #[test]
    fn durable_record_round_trip_preserves_recovery_fields() {
        let record = record();
        let bytes = record.encode().unwrap();
        let decoded = DurableCacheInvalidationRecord::decode(&bytes).unwrap();
        assert_eq!(decoded, record);
        assert_eq!(
            decoded.to_versioned_invalidation().unwrap(),
            VersionedCacheInvalidation::new("tenant.invalidate", "tenant:42", 11, 1_000)
                .unwrap()
        );
    }

    #[test]
    fn idempotency_key_is_stable_bounded_and_sensitive_to_generation() {
        let first = record();
        let second = DurableCacheInvalidationRecord::new(
            first.source_event_id(),
            first.tenant_id(),
            first.channel(),
            first.key(),
            first.generation() + 1,
            first.emitted_at_unix_ms(),
            first.cause(),
            first.trace_id().map(ToOwned::to_owned),
        )
        .unwrap();
        assert_eq!(first.idempotency_key(), record().idempotency_key());
        assert_ne!(first.idempotency_key(), second.idempotency_key());
        assert_eq!(first.idempotency_key().len(), "cache-invalidation:v1:".len() + 64);
    }

    #[test]
    fn durable_record_rejects_unbounded_metadata_before_encoding() {
        let error = DurableCacheInvalidationRecord::new(
            Uuid::from_u128(1),
            None,
            "tenant.invalidate",
            "tenant:42",
            1,
            1,
            "x".repeat(MAX_DURABLE_INVALIDATION_CAUSE_BYTES + 1),
            None,
        )
        .unwrap_err();
        assert!(matches!(
            error,
            DurableCacheInvalidationError::CauseTooLong { .. }
        ));
    }

    #[test]
    fn decoder_rejects_unsupported_format_version() {
        let mut record = record();
        record.format_version = DURABLE_CACHE_INVALIDATION_FORMAT_VERSION + 1;
        let bytes = postcard::to_stdvec(&record).unwrap();
        assert!(matches!(
            DurableCacheInvalidationRecord::decode(&bytes),
            Err(DurableCacheInvalidationError::UnsupportedFormatVersion { .. })
        ));
    }
}
