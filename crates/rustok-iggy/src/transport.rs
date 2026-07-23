use std::sync::Arc;

use async_trait::async_trait;
use tracing::{error, info};

use crate::config::{IggyConfig, IggyMode};
use crate::consumer::PersistentConsumerGroup;
use crate::contract_consumer::PersistentContractConsumerGroup;
use crate::producer;
use crate::serialization::{EventSerializer, JsonSerializer, MessagePackSerializer};
use crate::topology::TopologyManager;
use rustok_core::Result;
use rustok_core::events::{EventTransport, ReliabilityLevel};
use rustok_events::{ContractEventEnvelope, EventEnvelope};
use rustok_iggy_connector::{BundledConnector, ConnectorConfig, ExternalConnector, IggyConnector};

pub struct IggyTransport {
    config: IggyConfig,
    connector: Arc<dyn IggyConnector>,
    serializer: Arc<dyn EventSerializer>,
}

impl IggyTransport {
    pub async fn new(config: IggyConfig) -> Result<Self> {
        let connector: Arc<dyn IggyConnector> = match config.mode {
            IggyMode::External => Arc::new(ExternalConnector::new()),
            IggyMode::Bundled => Arc::new(BundledConnector::new()),
        };
        let connector_config = ConnectorConfig::from(&config);

        connector
            .connect(&connector_config)
            .await
            .map_err(|error| {
                error!(error = %error, mode = %config.mode, "Failed to connect to Iggy");
                rustok_core::Error::External(error.to_string())
            })?;

        let topology = TopologyManager::new();
        topology
            .ensure_topology(&config, connector.as_ref())
            .await?;

        let serializer: Arc<dyn EventSerializer> = match config.serialization {
            crate::config::SerializationFormat::Json => Arc::new(JsonSerializer),
            crate::config::SerializationFormat::MessagePack => Arc::new(MessagePackSerializer),
        };

        info!(
            mode = %config.mode,
            serialization = %config.serialization,
            stream = %config.topology.stream_name,
            "Iggy transport initialized"
        );

        Ok(Self {
            config,
            connector,
            serializer,
        })
    }

    pub async fn shutdown(&self) -> Result<()> {
        info!(mode = %self.config.mode, "Shutting down Iggy transport");

        self.connector.shutdown().await.map_err(|error| {
            error!(error = %error, "Failed to shutdown Iggy connector");
            rustok_core::Error::External(error.to_string())
        })?;

        Ok(())
    }

    /// Opens one broker-backed consumer-group cursor for result-first work.
    ///
    /// Receive and acknowledgement operate on the same remote cursor. The
    /// caller must retain the returned value for its complete lifetime.
    pub async fn open_persistent_consumer_group(
        &self,
        group_name: &str,
        topic: &str,
    ) -> Result<PersistentConsumerGroup> {
        let stream = self.config.topology.stream_name.clone();
        let cursor = self
            .connector
            .open_consumer_group(&stream, topic, group_name)
            .await
            .map_err(|error| rustok_core::Error::External(error.to_string()))?;
        Ok(PersistentConsumerGroup::new(
            stream,
            topic.to_string(),
            Arc::clone(&self.serializer),
            cursor,
        ))
    }

    /// Opens a persistent cursor for sealed typed event-family envelopes.
    pub async fn open_persistent_contract_consumer_group(
        &self,
        group_name: &str,
        topic: &str,
    ) -> Result<PersistentContractConsumerGroup> {
        let stream = self.config.topology.stream_name.clone();
        let cursor = self
            .connector
            .open_consumer_group(&stream, topic, group_name)
            .await
            .map_err(|error| rustok_core::Error::External(error.to_string()))?;
        Ok(PersistentContractConsumerGroup::new(
            stream,
            topic.to_string(),
            Arc::clone(&self.serializer),
            cursor,
        ))
    }

    pub async fn move_to_dlq(&self, entry: crate::dlq::DlqEntry) -> Result<()> {
        crate::dlq::DlqManager::new()
            .with_stream(self.config.topology.stream_name.clone())
            .move_to_dlq(&*self.connector, entry)
            .await
    }

    pub async fn retry_dlq_entry(
        &self,
        entry: crate::dlq::DlqEntry,
        target_topic: String,
    ) -> Result<()> {
        crate::dlq::DlqManager::new()
            .with_stream(self.config.topology.stream_name.clone())
            .retry_entry(&*self.connector, entry, target_topic)
            .await
    }

    pub fn config(&self) -> &IggyConfig {
        &self.config
    }

    pub fn is_connected(&self) -> bool {
        self.connector.is_connected()
    }
}

#[async_trait]
impl EventTransport for IggyTransport {
    async fn publish(&self, envelope: EventEnvelope) -> Result<()> {
        let request = producer::build_publish_request(&self.config, &*self.serializer, envelope)?;

        self.connector.publish(request).await.map_err(|error| {
            error!(error = %error, "Failed to publish event to Iggy");
            rustok_core::Error::External(error.to_string())
        })?;

        Ok(())
    }

    async fn publish_contract(&self, envelope: ContractEventEnvelope) -> Result<()> {
        let event_type = envelope.event_type().to_string();
        let request =
            producer::build_contract_publish_request(&self.config, &*self.serializer, envelope)?;

        self.connector.publish(request).await.map_err(|error| {
            error!(event_type, error = %error, "Failed to publish contract event to Iggy");
            rustok_core::Error::External(error.to_string())
        })?;

        Ok(())
    }

    fn reliability_level(&self) -> ReliabilityLevel {
        ReliabilityLevel::Streaming
    }

    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
}

impl std::fmt::Debug for IggyTransport {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("IggyTransport")
            .field("mode", &self.config.mode)
            .field("serialization", &self.config.serialization)
            .field("stream", &self.config.topology.stream_name)
            .field("connected", &self.connector.is_connected())
            .finish()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reliability_level_is_streaming() {
        assert_eq!(ReliabilityLevel::Streaming, ReliabilityLevel::Streaming);
    }
}
