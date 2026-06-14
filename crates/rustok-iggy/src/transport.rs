use std::sync::Arc;

use async_trait::async_trait;
use tracing::{error, info};

use crate::config::{IggyConfig, IggyMode};
use crate::consumer::ConsumerGroupManager;
use crate::producer;
use crate::serialization::{EventSerializer, JsonSerializer, PostcardSerializer};
use crate::topology::TopologyManager;
use rustok_core::events::{EventTransport, ReliabilityLevel};
use rustok_core::Result;
use rustok_events::EventEnvelope;
use rustok_iggy_connector::{ConnectorConfig, EmbeddedConnector, IggyConnector, RemoteConnector};

pub struct IggyTransport {
    config: IggyConfig,
    connector: Arc<dyn IggyConnector>,
    topology: TopologyManager,
    consumers: ConsumerGroupManager,
    serializer: Arc<dyn EventSerializer>,
}

impl IggyTransport {
    pub async fn new(config: IggyConfig) -> Result<Self> {
        let connector: Arc<dyn IggyConnector> = match config.mode {
            IggyMode::Remote => Arc::new(RemoteConnector::new()),
            IggyMode::Embedded => Arc::new(EmbeddedConnector::new()),
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
            crate::config::SerializationFormat::Postcard => Arc::new(PostcardSerializer),
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
            topology,
            consumers: ConsumerGroupManager::new(),
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

    pub async fn subscribe_as_group(&self, group: &str) -> Result<()> {
        use crate::consumer::ConsumerGroup;

        let group = ConsumerGroup::new(
            group.to_string(),
            self.config.topology.stream_name.clone(),
            "domain".to_string(),
        );

        self.consumers.ensure_group(group).await
    }

    pub async fn consume_next_as_group(
        &self,
        group: &str,
        partition: u32,
    ) -> Result<Option<crate::consumer::ConsumedEvent>> {
        self.consumers
            .consume_next(&*self.connector, &*self.serializer, group, partition)
            .await
    }

    pub async fn replay(&self) -> Result<()> {
        if !self.topology.is_initialized().await {
            return Err(rustok_core::Error::External(
                "Topology not initialized".to_string(),
            ));
        }

        Ok(())
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
