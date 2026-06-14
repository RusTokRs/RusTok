use std::collections::HashMap;
use std::sync::Arc;

use rustok_core::Result;
use rustok_events::EventEnvelope;
use rustok_iggy_connector::IggyConnector;
use tokio::sync::RwLock;
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

        match subscriber.recv().await {
            Ok(Some(payload)) => {
                let envelope = serializer.deserialize(&payload)?;
                Ok(Some(ConsumedEvent {
                    stream: group.stream,
                    topic: group.topic,
                    partition,
                    envelope,
                }))
            }
            Ok(None) => Ok(None),
            Err(error) => Err(rustok_core::Error::External(error.to_string())),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use async_trait::async_trait;
    use rustok_core::events::DomainEvent;
    use rustok_iggy_connector::{
        ConnectorConfig, ConnectorError, MessageSubscriber, PublishRequest,
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
    }
}
