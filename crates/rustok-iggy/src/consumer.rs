use std::sync::Arc;

use rustok_core::Result;
use rustok_events::EventEnvelope;
use rustok_iggy_connector::{ConsumerCursor, SubscriberMessageMetadata};
use tokio::sync::Mutex;

use crate::serialization::EventSerializer;

#[derive(Debug, Clone)]
pub struct ConsumedEvent {
    pub stream: String,
    pub topic: String,
    pub partition: u32,
    pub envelope: EventEnvelope,
    pub connector_metadata: SubscriberMessageMetadata,
}

/// One persistent external consumer-group cursor.
///
/// The broker cursor is retained across receive and acknowledgement. This is
/// the only root-event transport API suitable for result-first processing: an
/// event is not acknowledged until its owner has durably persisted the terminal
/// result. Per-partition subscribers that opened a different cursor to
/// acknowledge a delivery are intentionally not supported.
pub struct PersistentConsumerGroup {
    stream: String,
    topic: String,
    serializer: Arc<dyn EventSerializer>,
    cursor: Mutex<Box<dyn ConsumerCursor>>,
}

impl PersistentConsumerGroup {
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

    /// Receives one event without committing the broker offset.
    pub async fn receive(&self) -> Result<Option<ConsumedEvent>> {
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
        let envelope = self.serializer.deserialize(&message.payload)?;
        envelope
            .validate_registered_schema()
            .map_err(|error| rustok_core::Error::Validation(error.to_string()))?;
        if message.metadata.stream != self.stream || message.metadata.topic != self.topic {
            return Err(rustok_core::Error::External(format!(
                "Persistent consumer cursor returned metadata for {}/{} instead of {}/{}",
                message.metadata.stream, message.metadata.topic, self.stream, self.topic
            )));
        }

        Ok(Some(ConsumedEvent {
            stream: self.stream.clone(),
            topic: self.topic.clone(),
            partition: message.metadata.partition,
            envelope,
            connector_metadata: message.metadata,
        }))
    }

    /// Commits the offset for the exact event returned by [`Self::receive`].
    pub async fn acknowledge(&self, consumed: &ConsumedEvent) -> Result<()> {
        consumed.validate_connector_metadata()?;
        if consumed.stream != self.stream || consumed.topic != self.topic {
            return Err(rustok_core::Error::External(
                "Consumed event does not belong to this persistent consumer group".to_string(),
            ));
        }
        let ack_token = consumed.ack_token().ok_or_else(|| {
            rustok_core::Error::External(format!(
                "Consumed event {} has no connector ack token",
                consumed.envelope.id
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

impl ConsumedEvent {
    /// Returns the connector-owned offset when the backend exposed one.
    pub fn offset(&self) -> Option<u64> {
        self.connector_metadata.offset
    }

    /// Validates that connector metadata belongs to the consumed stream/topic/partition.
    ///
    /// Real SDK ack implementations commit offsets by opaque connector token. This
    /// guard prevents transport code from acknowledging a token captured from a
    /// different backend cursor after DLQ/replay movement or manual tests mutate
    /// metadata.
    pub fn validate_connector_metadata(&self) -> Result<()> {
        if self.connector_metadata.stream != self.stream
            || self.connector_metadata.topic != self.topic
            || self.connector_metadata.partition != self.partition
        {
            return Err(rustok_core::Error::External(format!(
                "Consumed event {} connector metadata mismatch: expected {}/{}/{} got {}/{}/{}",
                self.envelope.id,
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

    /// Returns the opaque connector acknowledgement token when one is available.
    pub fn ack_token(&self) -> Option<&str> {
        self.connector_metadata.ack_token.as_deref()
    }

    /// Builds a DLQ entry preserving the connector metadata observed at consume time.
    pub fn into_dlq_entry(
        self,
        payload: Vec<u8>,
        error: impl Into<String>,
        retry_count: u32,
    ) -> crate::dlq::DlqEntry {
        crate::dlq::DlqEntry {
            event_id: self.envelope.id,
            original_topic: self.topic,
            payload,
            error: error.into(),
            retry_count,
            connector_metadata: Some(self.connector_metadata),
        }
    }
}

#[cfg(test)]
mod tests {
    use std::collections::VecDeque;
    use std::sync::{Arc, Mutex as StdMutex};

    use async_trait::async_trait;
    use rustok_events::DomainEvent;
    use rustok_iggy_connector::{ConnectorError, SubscriberMessage};
    use uuid::Uuid;

    use super::*;
    use crate::serialization::JsonSerializer;

    #[tokio::test]
    async fn persistent_consumer_uses_one_cursor_for_receive_and_acknowledgement() {
        let envelope = EventEnvelope::new(
            Uuid::new_v4(),
            Some(Uuid::new_v4()),
            DomainEvent::NodeCreated {
                node_id: Uuid::new_v4(),
                kind: "post".to_string(),
                author_id: None,
            },
        );
        let serializer = JsonSerializer;
        let payload = serializer.serialize(&envelope).expect("serialize event");
        let acknowledged = Arc::new(StdMutex::new(Vec::new()));
        let cursor = FakeCursor {
            messages: VecDeque::from([SubscriberMessage::new(
                payload,
                SubscriberMessageMetadata::new("rustok", "domain", 1)
                    .with_offset(42)
                    .with_ack_token("ack-42"),
            )]),
            acknowledged: Arc::clone(&acknowledged),
        };
        let consumer = PersistentConsumerGroup::new(
            "rustok".to_string(),
            "domain".to_string(),
            Arc::new(serializer),
            Box::new(cursor),
        );

        let consumed = consumer
            .receive()
            .await
            .expect("receive event")
            .expect("one event");
        assert_eq!(consumed.envelope.id, envelope.id);
        assert_eq!(consumed.offset(), Some(42));
        consumer
            .acknowledge(&consumed)
            .await
            .expect("acknowledge exact delivery");

        assert_eq!(
            acknowledged.lock().expect("ack lock").as_slice(),
            ["ack-42"]
        );
    }

    #[tokio::test]
    async fn persistent_consumer_rejects_metadata_from_another_cursor() {
        let envelope = EventEnvelope::new(
            Uuid::new_v4(),
            None,
            DomainEvent::NodeCreated {
                node_id: Uuid::new_v4(),
                kind: "post".to_string(),
                author_id: None,
            },
        );
        let serializer = JsonSerializer;
        let payload = serializer.serialize(&envelope).expect("serialize event");
        let cursor = FakeCursor {
            messages: VecDeque::from([SubscriberMessage::new(
                payload,
                SubscriberMessageMetadata::new("rustok", "other", 1)
                    .with_offset(42)
                    .with_ack_token("ack-42"),
            )]),
            acknowledged: Arc::new(StdMutex::new(Vec::new())),
        };
        let consumer = PersistentConsumerGroup::new(
            "rustok".to_string(),
            "domain".to_string(),
            Arc::new(serializer),
            Box::new(cursor),
        );

        assert!(consumer.receive().await.is_err());
    }

    struct FakeCursor {
        messages: VecDeque<SubscriberMessage>,
        acknowledged: Arc<StdMutex<Vec<String>>>,
    }

    #[async_trait]
    impl ConsumerCursor for FakeCursor {
        async fn receive(
            &mut self,
        ) -> std::result::Result<Option<SubscriberMessage>, ConnectorError> {
            Ok(self.messages.pop_front())
        }

        async fn acknowledge(
            &mut self,
            ack_token: &str,
        ) -> std::result::Result<(), ConnectorError> {
            self.acknowledged
                .lock()
                .expect("ack lock")
                .push(ack_token.to_string());
            Ok(())
        }
    }
}
