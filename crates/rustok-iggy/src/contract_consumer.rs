use std::sync::Arc;

use rustok_core::Result;
use rustok_events::ContractEventEnvelope;
use rustok_iggy_connector::{ConsumerCursor, SubscriberMessageMetadata};
use tokio::sync::Mutex;

use crate::serialization::EventSerializer;

#[derive(Debug, Clone)]
pub struct ConsumedContractEvent {
    pub stream: String,
    pub topic: String,
    pub partition: u32,
    pub envelope: ContractEventEnvelope,
    pub connector_metadata: SubscriberMessageMetadata,
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

    /// Receives one typed contract event without committing the broker offset.
    pub async fn receive(&self) -> Result<Option<ConsumedContractEvent>> {
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
        let envelope = self.serializer.deserialize_contract(&message.payload)?;
        envelope
            .validate_registered_schema()
            .map_err(|error| rustok_core::Error::Validation(error.to_string()))?;
        if message.metadata.stream != self.stream || message.metadata.topic != self.topic {
            return Err(rustok_core::Error::External(format!(
                "Persistent contract consumer cursor returned metadata for {}/{} instead of {}/{}",
                message.metadata.stream, message.metadata.topic, self.stream, self.topic
            )));
        }

        Ok(Some(ConsumedContractEvent {
            stream: self.stream.clone(),
            topic: self.topic.clone(),
            partition: message.metadata.partition,
            envelope,
            connector_metadata: message.metadata,
        }))
    }

    /// Commits the offset for the exact contract event returned by [`Self::receive`].
    pub async fn acknowledge(&self, consumed: &ConsumedContractEvent) -> Result<()> {
        consumed.validate_connector_metadata()?;
        if consumed.stream != self.stream || consumed.topic != self.topic {
            return Err(rustok_core::Error::External(
                "Consumed contract event does not belong to this persistent consumer group"
                    .to_string(),
            ));
        }
        let ack_token = consumed.ack_token().ok_or_else(|| {
            rustok_core::Error::External(format!(
                "Consumed contract event {} has no connector ack token",
                consumed.envelope.id()
            ))
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
        crate::dlq::DlqEntry {
            event_id: self.envelope.id(),
            original_topic: self.topic,
            payload,
            error: error.into(),
            retry_count,
            connector_metadata: Some(self.connector_metadata),
        }
    }
}
