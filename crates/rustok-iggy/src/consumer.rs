use std::collections::HashMap;
use std::sync::Arc;

use rustok_core::Result;
use rustok_events::EventEnvelope;
use rustok_iggy_connector::{ConsumerCursor, IggyConnector, SubscriberMessageMetadata};
use tokio::sync::{Mutex, RwLock};
use tracing::info;

use crate::serialization::EventSerializer;

#[derive(Debug, Default)]
pub struct ConsumerGroupManager {
    groups: Arc<RwLock<HashMap<String, ConsumerGroup>>>,
}

#[derive(Debug, Clone)]
pub struct ConsumerGroup {
    pub name: String,
    pub stream: String,
    pub topic: String,
    pub partitions: Vec<u32>,
}

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
/// the only transport API suitable for result-first processing: an event is
/// not acknowledged until its owner has durably persisted the terminal result.
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

impl ConsumerGroup {
    pub fn new(name: String, stream: String, topic: String) -> Self {
        Self {
            name,
            stream,
            topic,
            partitions: Vec::new(),
        }
    }

    pub fn with_partitions(mut self, partitions: Vec<u32>) -> Self {
        self.partitions = partitions;
        self
    }
}

impl ConsumerGroupManager {
    pub fn new() -> Self {
        Self {
            groups: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    pub async fn ensure_group(&self, group: ConsumerGroup) -> Result<()> {
        let name = group.name.clone();
        info!(
            group = %name,
            stream = %group.stream,
            topic = %group.topic,
            "Ensuring consumer group"
        );

        self.groups.write().await.insert(name, group);
        Ok(())
    }

    pub async fn get_group(&self, name: &str) -> Option<ConsumerGroup> {
        self.groups.read().await.get(name).cloned()
    }

    pub async fn list_groups(&self) -> Vec<String> {
        self.groups.read().await.keys().cloned().collect()
    }

    pub async fn remove_group(&self, name: &str) -> Option<ConsumerGroup> {
        self.groups.write().await.remove(name)
    }

    pub async fn consume_next(
        &self,
        connector: &dyn IggyConnector,
        serializer: &dyn EventSerializer,
        group_name: &str,
        partition: u32,
    ) -> Result<Option<ConsumedEvent>> {
        let group = self.get_group(group_name).await.ok_or_else(|| {
            rustok_core::Error::External(format!("Consumer group not registered: {group_name}"))
        })?;

        if !group.partitions.is_empty() && !group.partitions.contains(&partition) {
            return Err(rustok_core::Error::External(format!(
                "Partition {partition} is not assigned to consumer group {group_name}"
            )));
        }

        let mut subscriber = connector
            .subscribe(&group.stream, &group.topic, partition)
            .await
            .map_err(|error| rustok_core::Error::External(error.to_string()))?;

        match subscriber.recv_with_metadata().await {
            Ok(Some(message)) => {
                let envelope = serializer.deserialize(&message.payload)?;
                Ok(Some(ConsumedEvent {
                    stream: group.stream,
                    topic: group.topic,
                    partition,
                    envelope,
                    connector_metadata: message.metadata,
                }))
            }
            Ok(None) => Ok(None),
            Err(error) => Err(rustok_core::Error::External(error.to_string())),
        }
    }

    pub async fn ack_consumed(
        &self,
        connector: &dyn IggyConnector,
        consumed: &ConsumedEvent,
    ) -> Result<()> {
        consumed.validate_connector_metadata()?;
        let ack_token = consumed.ack_token().ok_or_else(|| {
            rustok_core::Error::External(format!(
                "Consumed event {} has no connector ack token",
                consumed.envelope.id
            ))
        })?;

        let mut subscriber = connector
            .subscribe(&consumed.stream, &consumed.topic, consumed.partition)
            .await
            .map_err(|error| rustok_core::Error::External(error.to_string()))?;

        subscriber
            .ack(ack_token)
            .await
            .map_err(|error| rustok_core::Error::External(error.to_string()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use async_trait::async_trait;
    use rustok_core::events::DomainEvent;
    use rustok_iggy_connector::{
        ConnectorConfig, ConnectorError, MessageSubscriber, PublishRequest, SubscriberMessage,
    };
    use uuid::Uuid;

    use crate::serialization::JsonSerializer;

    #[tokio::test]
    async fn consumer_group_manager_starts_empty() {
        let manager = ConsumerGroupManager::new();
        assert!(manager.list_groups().await.is_empty());
    }

    #[tokio::test]
    async fn consumer_group_manager_creates_group() {
        let manager = ConsumerGroupManager::new();
        let group = ConsumerGroup::new(
            "domain-consumers".to_string(),
            "rustok".to_string(),
            "domain".to_string(),
        );

        manager.ensure_group(group).await.unwrap();

        let groups = manager.list_groups().await;
        assert_eq!(groups.len(), 1);
        assert!(groups.contains(&"domain-consumers".to_string()));
    }

    #[tokio::test]
    async fn consumer_group_manager_retrieves_group() {
        let manager = ConsumerGroupManager::new();
        let group = ConsumerGroup::new(
            "test-group".to_string(),
            "test-stream".to_string(),
            "test-topic".to_string(),
        )
        .with_partitions(vec![1, 2, 3]);

        manager.ensure_group(group).await.unwrap();

        let retrieved = manager.get_group("test-group").await;
        assert!(retrieved.is_some());

        let retrieved = retrieved.unwrap();
        assert_eq!(retrieved.stream, "test-stream");
        assert_eq!(retrieved.topic, "test-topic");
        assert_eq!(retrieved.partitions, vec![1, 2, 3]);
    }

    #[tokio::test]
    async fn consumer_group_manager_removes_group() {
        let manager = ConsumerGroupManager::new();
        let group = ConsumerGroup::new("to-remove".to_string(), "s".to_string(), "t".to_string());

        manager.ensure_group(group).await.unwrap();
        let removed = manager.remove_group("to-remove").await;

        assert!(removed.is_some());
        assert!(manager.list_groups().await.is_empty());
    }

    #[tokio::test]
    async fn consume_next_deserializes_subscribed_payload() {
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
        let payload = serializer.serialize(&envelope).unwrap();
        let connector = FakeConnector::new(Some(payload));
        let manager = ConsumerGroupManager::new();
        manager
            .ensure_group(
                ConsumerGroup::new(
                    "domain-workers".to_string(),
                    "rustok".to_string(),
                    "domain".to_string(),
                )
                .with_partitions(vec![1]),
            )
            .await
            .unwrap();

        let consumed = manager
            .consume_next(&connector, &serializer, "domain-workers", 1)
            .await
            .unwrap()
            .unwrap();

        assert_eq!(consumed.stream, "rustok");
        assert_eq!(consumed.topic, "domain");
        assert_eq!(consumed.partition, 1);
        assert_eq!(consumed.envelope.id, envelope.id);
        assert_eq!(consumed.connector_metadata.offset, Some(42));
        assert_eq!(consumed.offset(), Some(42));
        assert_eq!(consumed.ack_token(), Some("fake-ack-42"));
        assert_eq!(
            consumed.connector_metadata.ack_token.as_deref(),
            Some("fake-ack-42")
        );
        assert!(consumed.validate_connector_metadata().is_ok());
    }

    #[tokio::test]
    async fn consumed_event_rejects_ack_metadata_from_different_cursor() {
        let envelope = EventEnvelope::new(
            Uuid::new_v4(),
            Some(Uuid::new_v4()),
            DomainEvent::NodeCreated {
                node_id: Uuid::new_v4(),
                kind: "post".to_string(),
                author_id: None,
            },
        );
        let consumed = ConsumedEvent {
            stream: "rustok".to_string(),
            topic: "domain".to_string(),
            partition: 1,
            envelope,
            connector_metadata: SubscriberMessageMetadata::new("rustok", "other", 1)
                .with_offset(42)
                .with_ack_token("fake-ack-42"),
        };

        let result = consumed.validate_connector_metadata();

        assert!(result.is_err());
        assert!(
            result
                .unwrap_err()
                .to_string()
                .contains("connector metadata mismatch")
        );
    }

    #[tokio::test]
    async fn ack_consumed_rejects_metadata_mismatch_before_subscribing() {
        let envelope = EventEnvelope::new(
            Uuid::new_v4(),
            Some(Uuid::new_v4()),
            DomainEvent::NodeCreated {
                node_id: Uuid::new_v4(),
                kind: "post".to_string(),
                author_id: None,
            },
        );
        let consumed = ConsumedEvent {
            stream: "rustok".to_string(),
            topic: "domain".to_string(),
            partition: 1,
            envelope,
            connector_metadata: SubscriberMessageMetadata::new("wrong", "domain", 1)
                .with_offset(42)
                .with_ack_token("fake-ack-42"),
        };
        let manager = ConsumerGroupManager::new();

        let result = manager
            .ack_consumed(&FakeConnector::new(None), &consumed)
            .await;

        assert!(result.is_err());
    }

    #[tokio::test]
    async fn consume_next_rejects_unassigned_partition() {
        let manager = ConsumerGroupManager::new();
        manager
            .ensure_group(
                ConsumerGroup::new(
                    "domain-workers".to_string(),
                    "rustok".to_string(),
                    "domain".to_string(),
                )
                .with_partitions(vec![1]),
            )
            .await
            .unwrap();

        let result = manager
            .consume_next(
                &FakeConnector::new(None),
                &JsonSerializer,
                "domain-workers",
                2,
            )
            .await;

        assert!(result.is_err());
    }

    struct FakeConnector {
        payload: Option<Vec<u8>>,
    }

    impl FakeConnector {
        fn new(payload: Option<Vec<u8>>) -> Self {
            Self { payload }
        }
    }

    #[async_trait]
    impl IggyConnector for FakeConnector {
        async fn connect(
            &self,
            _config: &ConnectorConfig,
        ) -> std::result::Result<(), ConnectorError> {
            Ok(())
        }

        fn is_connected(&self) -> bool {
            true
        }

        async fn publish(
            &self,
            _request: PublishRequest,
        ) -> std::result::Result<(), ConnectorError> {
            Ok(())
        }

        async fn subscribe(
            &self,
            _stream: &str,
            _topic: &str,
            _partition: u32,
        ) -> std::result::Result<Box<dyn MessageSubscriber>, ConnectorError> {
            Ok(Box::new(FakeSubscriber {
                payload: self.payload.clone(),
            }))
        }

        async fn shutdown(&self) -> std::result::Result<(), ConnectorError> {
            Ok(())
        }
    }

    struct FakeSubscriber {
        payload: Option<Vec<u8>>,
    }

    #[async_trait]
    impl MessageSubscriber for FakeSubscriber {
        async fn recv(&mut self) -> std::result::Result<Option<Vec<u8>>, ConnectorError> {
            Ok(self.payload.take())
        }

        async fn recv_with_metadata(
            &mut self,
        ) -> std::result::Result<Option<SubscriberMessage>, ConnectorError> {
            Ok(self.payload.take().map(|payload| {
                SubscriberMessage::new(
                    payload,
                    SubscriberMessageMetadata::new("rustok", "domain", 1)
                        .with_offset(42)
                        .with_ack_token("fake-ack-42"),
                )
            }))
        }
    }
}
