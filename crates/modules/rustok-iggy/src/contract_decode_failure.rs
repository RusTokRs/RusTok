use rustok_core::Result;
use rustok_iggy_connector::SubscriberMessageMetadata;
use sha2::{Digest, Sha256};
use uuid::Uuid;

use crate::dlq::DlqEntry;

const CONTRACT_DECODE_FAILURE_ID_DOMAIN: &[u8] =
    b"rustok.iggy.contract.decode_failure.delivery_id.v1";

/// Stable connector-owned failure classification for raw contract deliveries that
/// cannot become a validated [`rustok_events::ContractEventEnvelope`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ContractDecodeFailureKind {
    Deserialize,
    SchemaValidation,
}

impl ContractDecodeFailureKind {
    pub const fn stable_code(self) -> &'static str {
        match self {
            Self::Deserialize => "iggy.contract.decode_invalid",
            Self::SchemaValidation => "iggy.contract.schema_invalid",
        }
    }
}

/// Exact broker delivery retained when contract decoding or canonical schema
/// validation fails before a domain event id or tenant id can be trusted.
///
/// The connector delivery id is derived only from immutable source coordinates and
/// exact broker bytes. It is suitable as a stable DLQ broker identity, but it is not
/// a domain event id, not a tenant identity, and not a durable exactly-once receipt.
#[derive(Debug, Clone)]
pub struct ConsumedContractDecodeFailure {
    stream: String,
    topic: String,
    partition: u32,
    source_offset: u64,
    connector_metadata: SubscriberMessageMetadata,
    raw_payload: Vec<u8>,
    kind: ContractDecodeFailureKind,
}

impl ConsumedContractDecodeFailure {
    pub fn new(
        stream: String,
        topic: String,
        connector_metadata: SubscriberMessageMetadata,
        raw_payload: Vec<u8>,
        kind: ContractDecodeFailureKind,
    ) -> Result<Self> {
        let source_offset = connector_metadata.offset.ok_or_else(|| {
            rustok_core::Error::External(
                "Undecodable contract delivery has no connector offset".to_string(),
            )
        })?;
        let failure = Self {
            partition: connector_metadata.partition,
            source_offset,
            stream,
            topic,
            connector_metadata,
            raw_payload,
            kind,
        };
        failure.validate_connector_metadata()?;
        Ok(failure)
    }

    pub fn stream(&self) -> &str {
        &self.stream
    }

    pub fn topic(&self) -> &str {
        &self.topic
    }

    pub const fn partition(&self) -> u32 {
        self.partition
    }

    pub const fn offset(&self) -> u64 {
        self.source_offset
    }

    pub const fn kind(&self) -> ContractDecodeFailureKind {
        self.kind
    }

    pub const fn stable_error_code(&self) -> &'static str {
        self.kind.stable_code()
    }

    pub fn connector_metadata(&self) -> &SubscriberMessageMetadata {
        &self.connector_metadata
    }

    pub fn ack_token(&self) -> Option<&str> {
        self.connector_metadata.ack_token.as_deref()
    }

    pub fn raw_payload(&self) -> &[u8] {
        &self.raw_payload
    }

    pub fn validate_connector_metadata(&self) -> Result<()> {
        if self.connector_metadata.stream != self.stream
            || self.connector_metadata.topic != self.topic
            || self.connector_metadata.partition != self.partition
            || self.connector_metadata.offset != Some(self.source_offset)
        {
            return Err(rustok_core::Error::External(format!(
                "Undecodable contract delivery connector metadata mismatch: expected {}/{}/{}/{} got {}/{}/{}/{:?}",
                self.stream,
                self.topic,
                self.partition,
                self.source_offset,
                self.connector_metadata.stream,
                self.connector_metadata.topic,
                self.connector_metadata.partition,
                self.connector_metadata.offset
            )));
        }
        Ok(())
    }

    /// Stable RFC 9562 UUIDv8 for the immutable broker delivery.
    ///
    /// Failure kind, retry count, time, process identity, and random values are excluded
    /// so decoder changes and retries retain one connector delivery identity.
    pub fn delivery_id(&self) -> Uuid {
        let mut hasher = Sha256::new();
        hash_part(&mut hasher, CONTRACT_DECODE_FAILURE_ID_DOMAIN);
        hash_part(&mut hasher, self.stream.as_bytes());
        hash_part(&mut hasher, self.topic.as_bytes());
        hash_part(&mut hasher, &self.partition.to_be_bytes());
        hash_part(&mut hasher, &self.source_offset.to_be_bytes());
        hash_part(&mut hasher, &self.raw_payload);

        let digest = hasher.finalize();
        let mut bytes = [0_u8; 16];
        bytes.copy_from_slice(&digest[..16]);
        bytes[6] = (bytes[6] & 0x0f) | 0x80;
        bytes[8] = (bytes[8] & 0x3f) | 0x80;
        Uuid::from_bytes(bytes)
    }

    /// Builds a lossless DLQ entry without inventing decoded tenant or event facts.
    ///
    /// `DlqEntry` requires an event-shaped UUID, so the connector delivery UUID occupies
    /// that transport field and is also attached as the explicit broker message id. Owner
    /// code must continue to distinguish it from a decoded domain event id.
    pub fn to_dlq_entry(&self, retry_count: u32) -> DlqEntry {
        let delivery_id = self.delivery_id();
        DlqEntry::new(
            delivery_id,
            self.topic.clone(),
            self.raw_payload.clone(),
            self.stable_error_code(),
            retry_count,
        )
        .with_connector_metadata(self.connector_metadata.clone())
        .with_broker_message_id(delivery_id)
    }
}

fn hash_part(hasher: &mut Sha256, bytes: &[u8]) {
    let length = u64::try_from(bytes.len()).unwrap_or(u64::MAX);
    hasher.update(length.to_be_bytes());
    hasher.update(bytes);
}

#[cfg(test)]
mod tests {
    use super::*;

    fn metadata(offset: u64) -> SubscriberMessageMetadata {
        SubscriberMessageMetadata::new("rustok", "domain", 2)
            .with_offset(offset)
            .with_ack_token(format!("ack-{offset}"))
    }

    fn failure(
        payload: Vec<u8>,
        offset: u64,
        kind: ContractDecodeFailureKind,
    ) -> ConsumedContractDecodeFailure {
        ConsumedContractDecodeFailure::new(
            "rustok".to_string(),
            "domain".to_string(),
            metadata(offset),
            payload,
            kind,
        )
        .unwrap()
    }

    #[test]
    fn delivery_id_is_stable_custom_versioned_and_kind_independent() {
        let first = failure(vec![1, 2, 3], 42, ContractDecodeFailureKind::Deserialize);
        let second = failure(
            vec![1, 2, 3],
            42,
            ContractDecodeFailureKind::SchemaValidation,
        );

        assert_eq!(first.delivery_id(), second.delivery_id());
        assert_eq!(first.delivery_id().as_bytes()[6] >> 4, 8);
        assert_eq!(first.delivery_id().as_bytes()[8] & 0xc0, 0x80);
    }

    #[test]
    fn delivery_id_changes_with_exact_payload_or_source_position() {
        assert_ne!(
            failure(vec![1, 2, 3], 42, ContractDecodeFailureKind::Deserialize).delivery_id(),
            failure(vec![1, 2, 4], 42, ContractDecodeFailureKind::Deserialize).delivery_id()
        );
        assert_ne!(
            failure(vec![1, 2, 3], 42, ContractDecodeFailureKind::Deserialize).delivery_id(),
            failure(vec![1, 2, 3], 43, ContractDecodeFailureKind::Deserialize).delivery_id()
        );
    }

    #[test]
    fn dlq_entry_keeps_exact_bytes_and_stable_connector_identity() {
        let failure = failure(
            vec![0xff, 0x00, 0x7f],
            42,
            ContractDecodeFailureKind::Deserialize,
        );
        let entry = failure.to_dlq_entry(3);

        assert_eq!(entry.event_id, failure.delivery_id());
        assert_eq!(entry.broker_message_id(), Some(failure.delivery_id()));
        assert_eq!(entry.payload, failure.raw_payload());
        assert_eq!(entry.error, "iggy.contract.decode_invalid");
        assert_eq!(entry.retry_count, 3);
    }
}
