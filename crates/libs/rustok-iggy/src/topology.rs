use std::sync::Arc;

use rustok_iggy_connector::IggyConnector;
use tokio::sync::RwLock;
use tracing::info;

use crate::MODULE_BUILD_TOPIC;
use crate::config::IggyConfig;

#[derive(Debug)]
pub struct TopologyManager {
    stream_name: Arc<RwLock<String>>,
    domain_topic: Arc<RwLock<String>>,
    system_topic: Arc<RwLock<String>>,
    module_build_topic: Arc<RwLock<String>>,
    partitions: Arc<RwLock<u32>>,
    initialized: Arc<RwLock<bool>>,
}

impl Default for TopologyManager {
    fn default() -> Self {
        Self::new()
    }
}

impl TopologyManager {
    pub fn new() -> Self {
        Self {
            stream_name: Arc::new(RwLock::new(String::new())),
            domain_topic: Arc::new(RwLock::new(String::new())),
            system_topic: Arc::new(RwLock::new(String::new())),
            module_build_topic: Arc::new(RwLock::new(String::new())),
            partitions: Arc::new(RwLock::new(0)),
            initialized: Arc::new(RwLock::new(false)),
        }
    }

    pub async fn ensure_topology(
        &self,
        config: &IggyConfig,
        connector: &dyn IggyConnector,
    ) -> rustok_core::Result<()> {
        let stream_name = config.topology.stream_name.clone();
        let partitions = config.topology.domain_partitions;

        info!(
            stream = %stream_name,
            domain_partitions = partitions,
            replication_factor = config.topology.replication_factor,
            domain_retention_days = config.retention.domain_max_age_days,
            system_retention_days = config.retention.system_max_age_days,
            dlq_retention_days = config.retention.dlq_max_age_days,
            "Ensuring iggy topology"
        );

        connector
            .ensure_topology(
                &stream_name,
                &["domain", "system", MODULE_BUILD_TOPIC, "dlq"],
                partitions,
                config.topology.replication_factor,
            )
            .await
            .map_err(|error| rustok_core::Error::External(error.to_string()))?;

        *self.stream_name.write().await = stream_name.clone();
        *self.domain_topic.write().await = "domain".to_string();
        *self.system_topic.write().await = "system".to_string();
        *self.module_build_topic.write().await = MODULE_BUILD_TOPIC.to_string();
        *self.partitions.write().await = partitions;
        *self.initialized.write().await = true;

        Ok(())
    }

    pub async fn stream_name(&self) -> String {
        self.stream_name.read().await.clone()
    }

    pub async fn domain_topic(&self) -> String {
        self.domain_topic.read().await.clone()
    }

    pub async fn system_topic(&self) -> String {
        self.system_topic.read().await.clone()
    }

    /// Dedicated queue topic for immutable module build requests.
    pub async fn module_build_topic(&self) -> String {
        self.module_build_topic.read().await.clone()
    }

    pub async fn is_initialized(&self) -> bool {
        *self.initialized.read().await
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Mutex;

    #[tokio::test]
    async fn topology_manager_initializes_with_defaults() {
        let manager = TopologyManager::new();
        assert!(!manager.is_initialized().await);
    }

    #[tokio::test]
    async fn topology_manager_stores_config() {
        let manager = TopologyManager::new();
        let config = IggyConfig::default();
        let connector = MockConnector::default();

        manager.ensure_topology(&config, &connector).await.unwrap();

        assert!(manager.is_initialized().await);
        assert_eq!(manager.stream_name().await, "rustok");
        assert_eq!(manager.domain_topic().await, "domain");
        assert_eq!(manager.system_topic().await, "system");
        assert_eq!(manager.module_build_topic().await, MODULE_BUILD_TOPIC);
        assert_eq!(
            connector.topology.lock().unwrap().clone(),
            Some((
                "rustok".to_string(),
                vec![
                    "domain".to_string(),
                    "system".to_string(),
                    MODULE_BUILD_TOPIC.to_string(),
                    "dlq".to_string(),
                ],
                8,
                1,
            ))
        );
    }

    #[tokio::test]
    async fn topology_manager_does_not_mark_itself_ready_when_broker_setup_fails() {
        let manager = TopologyManager::new();
        let connector = MockConnector::failing();

        let error = manager
            .ensure_topology(&IggyConfig::default(), &connector)
            .await
            .expect_err("broker topology failure must be returned");

        assert!(error.to_string().contains("topology creation failed"));
        assert!(!manager.is_initialized().await);
        assert_eq!(manager.stream_name().await, "");
    }

    #[derive(Default)]
    struct MockConnector {
        topology: Mutex<Option<(String, Vec<String>, u32, u8)>>,
        fail_topology: bool,
    }

    impl MockConnector {
        fn failing() -> Self {
            Self {
                topology: Mutex::new(None),
                fail_topology: true,
            }
        }
    }

    #[async_trait::async_trait]
    impl rustok_iggy_connector::IggyConnector for MockConnector {
        async fn connect(
            &self,
            _config: &rustok_iggy_connector::ConnectorConfig,
        ) -> std::result::Result<(), rustok_iggy_connector::ConnectorError> {
            Ok(())
        }

        fn is_connected(&self) -> bool {
            true
        }

        async fn publish(
            &self,
            _request: rustok_iggy_connector::PublishRequest,
        ) -> std::result::Result<(), rustok_iggy_connector::ConnectorError> {
            Ok(())
        }

        async fn subscribe(
            &self,
            _stream: &str,
            _topic: &str,
            _partition: u32,
        ) -> std::result::Result<
            Box<dyn rustok_iggy_connector::MessageSubscriber>,
            rustok_iggy_connector::ConnectorError,
        > {
            Ok(Box::new(MockSubscriber))
        }

        async fn ensure_topology(
            &self,
            stream: &str,
            topics: &[&str],
            partitions: u32,
            replication_factor: u8,
        ) -> std::result::Result<(), rustok_iggy_connector::ConnectorError> {
            if self.fail_topology {
                return Err(rustok_iggy_connector::ConnectorError::Topology(
                    "topology creation failed".to_string(),
                ));
            }

            *self.topology.lock().unwrap() = Some((
                stream.to_string(),
                topics.iter().map(ToString::to_string).collect(),
                partitions,
                replication_factor,
            ));
            Ok(())
        }

        async fn shutdown(&self) -> std::result::Result<(), rustok_iggy_connector::ConnectorError> {
            Ok(())
        }
    }

    struct MockSubscriber;

    #[async_trait::async_trait]
    impl rustok_iggy_connector::MessageSubscriber for MockSubscriber {
        async fn recv(
            &mut self,
        ) -> std::result::Result<Option<Vec<u8>>, rustok_iggy_connector::ConnectorError> {
            Ok(None)
        }
    }
}
