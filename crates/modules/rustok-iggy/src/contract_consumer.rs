use std::sync::Arc;

use rustok_core::Result;
use rustok_events::ContractEventEnvelope;
use rustok_iggy_connector::{ConsumerCursor, SubscriberMessageMetadata};
use tokio::sync::Mutex;

use crate::contract_decode_failure::{ConsumedContractDecodeFailure, ContractDecodeFailureKind};
use crate::serialization::EventSerializer;

#[derive(Debug, Clone)]
pub struct ConsumedContractEvent {
    pub stream: String,
    pub topic: String,
    pub partition: u32,
    pub envelope: ContractEventEnvelope,
    pub connector_metadata: SubscriberMessageMetadata,
    /// Exact broker payload retained for lossless DLQ publication.
    pub raw_payload: Vec<u8>,
}

/// One raw broker delivery after connector metadata validation.
///
/// Decode failures remain typed and retain exact bytes plus the same cursor metadata.
/// They are not acknowledged automatically; an owner must first publish or persist its
/// chosen terminal poison result and then call `acknowledge_decode_failure` explicitly.
#[derive(Debug, Clone)]
pub enum PersistentContractDelivery {
    Event(Box<ConsumedContractEvent>),
    DecodeFailure(Box<ConsumedContractDecodeFailure>),
}

/// Persistent consumer cursor for sealed typed event-family envelopes.
pub struct PersistentContractConsumerGroup {
    stream: String,
    topic: String,
    serializer: Arc<dyn EventSerializer>,
    cursor: Mutex<Box<dyn ConsumerCursor>>,
}

impl PersistentContractConsumerGroup {
    pub(crate) fn new(
        stream: String,
        topic: String,
        serializer: Arc<dyn EventSerializer>,
        cursor: Box<dyn ConsumerCursor>,
    ) -> Self {
        Self {
            stream,
            topic,
            serializer,
            cursor: Mutex::new(cursor),
        }
    }

    /// Receives one raw contract delivery without committing the broker offset.
    ///
    /// Connector metadata mismatches remain transport failures. Deserialization and
    /// registered-schema failures become a typed `DecodeFailure` so owner code can retain
    /// exact bytes, select a DLQ/recovery policy, and acknowledge only after its terminal
    /// result exists.
    pub async fn receive_delivery(&self) -> Result<Option<PersistentContractDelivery>> {
        let message = self
            .cursor
            .lock()
            .await
            .receive()
            .await
            .map_err(|error| rustok_core::Error::External(error.to_string()))?;
        let Some(message) = message else {
            return Ok(None);
        };
        self.validate_cursor_metadata(&message.metadata)?;

        let raw_payload = message.payload;
        let connector_metadata = message.metadata;
        let envelope = match self.serializer.deserialize_contract(&raw_payload) {
            Ok(envelope) => envelope,
            Err(_) => {
                return Ok(Some(PersistentContractDelivery::DecodeFailure(Box::new(
                    ConsumedContractDecodeFailure::new(
                        self.stream.clone(),
                        self.topic.clone(),
                        connector_metadata,
                        raw_payload,
                        ContractDecodeFailureKind::Deserialize,
                    )?,
                ))));
            }
        };
        if envelope.validate_registered_schema().is_err() {
            return Ok(Some(PersistentContractDelivery::DecodeFailure(Box::new(
                ConsumedContractDecodeFailure::new(
                    self.stream.clone(),
                    self.topic.clone(),
                    connector_metadata,
                    raw_payload,
                    ContractDecodeFailureKind::SchemaValidation,
                )?,
            ))));
        }

        Ok(Some(PersistentContractDelivery::Event(Box::new(
            ConsumedContractEvent {
                stream: self.stream.clone(),
                topic: self.topic.clone(),
                partition: connector_metadata.partition,
                envelope,
                connector_metadata,
                raw_payload,
            },
        ))))
    }

    /// Compatibility receive path for callers that have not yet adopted typed raw
    /// decode failures. A malformed delivery remains unacknowledged and returns only a
    /// bounded stable classification; exact bytes remain available through
    /// [`Self::receive_delivery`].
    pub async fn receive(&self) -> Result<Option<ConsumedContractEvent>> {
        match self.receive_delivery().await? {
            Some(PersistentContractDelivery::Event(consumed)) => Ok(Some(*consumed)),
            Some(PersistentContractDelivery::DecodeFailure(failure)) => {
                Err(rustok_core::Error::Validation(format!(
                    "Persistent contract delivery rejected [{}]",
                    failure.stable_error_code()
                )))
            }
            None => Ok(None),
        }
    }

    /// Commits the offset for the exact contract event returned by [`Self::receive`] or
    /// [`Self::receive_delivery`].
    pub async fn acknowledge(&self, consumed: &ConsumedContractEvent) -> Result<()> {
        consumed.validate_connector_metadata()?;
        self.acknowledge_metadata(
            &consumed.stream,
            &consumed.topic,
            consumed.partition,
            &consumed.connector_metadata,
            "Consumed contract event",
        )
        .await
    }

    /// Commits an undecodable delivery only after owner code established its terminal
    /// poison result. This method never publishes a DLQ entry or chooses retry policy.
    pub async fn acknowledge_decode_failure(
        &self,
        consumed: &ConsumedContractDecodeFailure,
    ) -> Result<()> {
        consumed.validate_connector_metadata()?;
        self.acknowledge_metadata(
            consumed.stream(),
            consumed.topic(),
            consumed.partition(),
            consumed.connector_metadata(),
            "Undecodable contract delivery",
        )
        .await
    }

    fn validate_cursor_metadata(&self, metadata: &SubscriberMessageMetadata) -> Result<()> {
        if metadata.stream != self.stream || metadata.topic != self.topic {
            return Err(rustok_core::Error::External(format!(
                "Persistent contract consumer cursor returned metadata for {}/{} instead of {}/{}",
                metadata.stream, metadata.topic, self.stream, self.topic
            )));
        }
        Ok(())
    }

    async fn acknowledge_metadata(
        &self,
        stream: &str,
        topic: &str,
        partition: u32,
        metadata: &SubscriberMessageMetadata,
        subject: &str,
    ) -> Result<()> {
        if stream != self.stream
            || topic != self.topic
            || metadata.stream != stream
            || metadata.topic != topic
            || metadata.partition != partition
        {
            return Err(rustok_core::Error::External(format!(
                "{subject} does not belong to this persistent consumer group"
            )));
        }
        let ack_token = metadata.ack_token.as_deref().ok_or_else(|| {
            rustok_core::Error::External(format!("{subject} has no connector ack token"))
        })?;
        self.cursor
            .lock()
            .await
            .acknowledge(ack_token)
            .await
            .map_err(|error| rustok_core::Error::External(error.to_string()))
    }
}

impl ConsumedContractEvent {
    pub fn offset(&self) -> Option<u64> {
        self.connector_metadata.offset
    }

    pub fn raw_payload(&self) -> &[u8] {
        &self.raw_payload
    }

    pub fn validate_connector_metadata(&self) -> Result<()> {
        if self.connector_metadata.stream != self.stream
            || self.connector_metadata.topic != self.topic
            || self.connector_metadata.partition != self.partition
        {
            return Err(rustok_core::Error::External(format!(
                "Consumed contract event {} connector metadata mismatch: expected {}/{}/{} got {}/{}/{}",
                self.envelope.id(),
                self.stream,
                self.topic,
                self.partition,
                self.connector_metadata.stream,
                self.connector_metadata.topic,
                self.connector_metadata.partition
            )));
        }
        Ok(())
    }

    pub fn ack_token(&self) -> Option<&str> {
        self.connector_metadata.ack_token.as_deref()
    }

    pub fn into_dlq_entry(
        self,
        payload: Vec<u8>,
        error: impl Into<String>,
        retry_count: u32,
    ) -> crate::dlq::DlqEntry {
        crate::dlq::DlqEntry::new(self.envelope.id(), self.topic, payload, error, retry_count)
            .with_connector_metadata(self.connector_metadata)
    }
}
