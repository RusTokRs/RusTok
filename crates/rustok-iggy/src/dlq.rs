use std::sync::Arc;

use rustok_core::Result;
use rustok_iggy_connector::{IggyConnector, PublishRequest, SubscriberMessageMetadata};
use tokio::sync::RwLock;
use tracing::{error, info, warn};
use uuid::Uuid;

#[derive(Debug, Clone)]
pub struct DlqEntry {
    pub event_id: Uuid,
    pub original_topic: String,
    pub payload: Vec<u8>,
    pub error: String,
    pub retry_count: u32,
    pub connector_metadata: Option<SubscriberMessageMetadata>,
}

impl DlqEntry {
    pub fn new(
        event_id: Uuid,
        original_topic: impl Into<String>,
        payload: Vec<u8>,
        error: impl Into<String>,
        retry_count: u32,
    ) -> Self {
        Self {
            event_id,
            original_topic: original_topic.into(),
            payload,
            error: error.into(),
            retry_count,
            connector_metadata: None,
        }
    }

    pub fn with_connector_metadata(mut self, metadata: SubscriberMessageMetadata) -> Self {
        self.connector_metadata = Some(metadata);
        self
    }

    pub fn source_offset(&self) -> Option<u64> {
        self.connector_metadata
            .as_ref()
            .and_then(|metadata| metadata.offset)
    }

    pub fn ack_token(&self) -> Option<&str> {
        self.connector_metadata
            .as_ref()
            .and_then(|metadata| metadata.ack_token.as_deref())
    }
}

#[derive(Debug)]
pub struct DlqManager {
    stream: Arc<RwLock<String>>,
    topic: Arc<RwLock<String>>,
    max_retries: Arc<RwLock<u32>>,
}

impl Default for DlqManager {
    fn default() -> Self {
        Self::new()
    }
}

impl DlqManager {
    pub fn new() -> Self {
        Self {
            stream: Arc::new(RwLock::new("rustok".to_string())),
            topic: Arc::new(RwLock::new("dlq".to_string())),
            max_retries: Arc::new(RwLock::new(3)),
        }
    }

    pub fn with_stream(self, stream: String) -> Self {
        *self.stream.blocking_write() = stream;
        self
    }

    pub fn with_topic(self, topic: String) -> Self {
        *self.topic.blocking_write() = topic;
        self
    }

    pub fn with_max_retries(self, max_retries: u32) -> Self {
        *self.max_retries.blocking_write() = max_retries;
        self
    }

    pub async fn move_to_dlq(&self, connector: &dyn IggyConnector, entry: DlqEntry) -> Result<()> {
        let stream = self.stream.read().await.clone();
        let topic = self.topic.read().await.clone();

        warn!(
            event_id = %entry.event_id,
            original_topic = %entry.original_topic,
            error = %entry.error,
            retry_count = entry.retry_count,
            source_offset = ?entry.source_offset(),
            has_ack_token = entry.ack_token().is_some(),
            dlq_stream = %stream,
            dlq_topic = %topic,
            "Moving event to dead letter queue"
        );

        let request = PublishRequest::new(
            stream,
            topic,
            entry.event_id.to_string(),
            entry.payload,
            format!("dlq-{}", entry.event_id),
        );

        connector.publish(request).await.map_err(|e| {
            error!(error = %e, "Failed to publish to DLQ");
            rustok_core::Error::External(e.to_string())
        })?;

        Ok(())
    }

    pub async fn retry_entry(
        &self,
        connector: &dyn IggyConnector,
        entry: DlqEntry,
        target_topic: String,
    ) -> Result<()> {
        let max_retries = *self.max_retries.read().await;
        if entry.retry_count >= max_retries {
            return Err(rustok_core::Error::External(format!(
                "DLQ entry {} exceeded retry limit {}",
                entry.event_id, max_retries
            )));
        }

        let stream = self.stream.read().await.clone();
        info!(
            event_id = %entry.event_id,
            original_topic = %entry.original_topic,
            target_topic = %target_topic,
            retry_count = entry.retry_count + 1,
            source_offset = ?entry.source_offset(),
            "Retrying DLQ entry"
        );

        let request = PublishRequest::new(
            stream,
            target_topic,
            entry.event_id.to_string(),
            entry.payload,
            format!("retry-{}-{}", entry.retry_count + 1, entry.event_id),
        );

        connector
            .publish(request)
            .await
            .map_err(|e| rustok_core::Error::External(e.to_string()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dlq_manager_defaults() {
        let manager = DlqManager::new();
        assert_eq!(*manager.stream.blocking_read(), "rustok");
        assert_eq!(*manager.topic.blocking_read(), "dlq");
        assert_eq!(*manager.max_retries.blocking_read(), 3);
    }

    #[test]
    fn dlq_entry_creation() {
        let entry = DlqEntry::new(
            Uuid::new_v4(),
            "domain",
            vec![1, 2, 3],
            "Processing failed",
            2,
        );

        assert!(!entry.payload.is_empty());
        assert_eq!(entry.retry_count, 2);
    }
}
