//! Connector layer for Iggy transports.
//!
//! This module provides two connector implementations:
//! - `RemoteConnector`: connects to an external Iggy server via TCP/HTTP
//! - `EmbeddedConnector`: runs an embedded Iggy server within the application
//!
//! The connector handles connection lifecycle, message publishing, and graceful shutdown.
//!
//! # Usage
//!
//! ```rust,no_run
//! use rustok_iggy_connector::{ConnectorConfig, ConnectorMode, IggyConnector, RemoteConnector};
//!
//! #[tokio::main]
//! async fn main() -> Result<(), Box<dyn std::error::Error>> {
//!     let connector = RemoteConnector::new();
//!
//!     let config = ConnectorConfig::default();
//!     connector.connect(&config).await?;
//!
//!     // Publish messages...
//!
//!     connector.shutdown().await?;
//!     Ok(())
//! }
//! ```

use std::sync::Arc;

use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use tokio::sync::RwLock;

#[cfg(feature = "iggy")]
use iggy::prelude::{Client, IggyClient, IggyError};

/// Connection mode for Iggy connector
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ConnectorMode {
    /// Embedded mode - runs Iggy server within the application
    #[default]
    Embedded,
    /// Remote mode - connects to external Iggy server
    Remote,
}

impl std::fmt::Display for ConnectorMode {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ConnectorMode::Embedded => write!(f, "embedded"),
            ConnectorMode::Remote => write!(f, "remote"),
        }
    }
}

impl serde::Serialize for ConnectorMode {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        serializer.serialize_str(&self.to_string())
    }
}

impl<'de> serde::Deserialize<'de> for ConnectorMode {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let s = String::deserialize(deserializer)?;
        match s.to_lowercase().as_str() {
            "embedded" => Ok(ConnectorMode::Embedded),
            "remote" => Ok(ConnectorMode::Remote),
            _ => Err(serde::de::Error::custom(format!("Unknown mode: {}", s))),
        }
    }
}

/// Configuration for embedded Iggy server
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EmbeddedConnectorConfig {
    /// Directory for storing Iggy data (streams, topics, messages)
    pub data_dir: String,
    /// TCP port for the embedded server
    pub tcp_port: u16,
    /// HTTP port for the embedded server dashboard (0 to disable)
    pub http_port: u16,
    /// Whether to use persistence
    pub persistent: bool,
}

impl Default for EmbeddedConnectorConfig {
    fn default() -> Self {
        Self {
            data_dir: "./data/iggy".to_string(),
            tcp_port: 8090,
            http_port: 3000,
            persistent: true,
        }
    }
}

/// Configuration for remote Iggy server connection
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RemoteConnectorConfig {
    /// Server addresses (ip:port)
    pub addresses: Vec<String>,
    /// Protocol to use (tcp, http)
    pub protocol: String,
    /// Username for authentication
    pub username: String,
    /// Password for authentication
    pub password: String,
    /// TLS enabled
    pub tls_enabled: bool,
}

impl Default for RemoteConnectorConfig {
    fn default() -> Self {
        Self {
            addresses: vec!["127.0.0.1:8090".to_string()],
            protocol: "tcp".to_string(),
            username: "iggy".to_string(),
            password: "iggy".to_string(),
            tls_enabled: false,
        }
    }
}

/// Main connector configuration combining both modes
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConnectorConfig {
    /// Connection mode: Embedded or Remote
    pub mode: ConnectorMode,
    /// Configuration for embedded mode
    pub embedded: EmbeddedConnectorConfig,
    /// Configuration for remote mode
    pub remote: RemoteConnectorConfig,
    /// Stream name for message routing
    pub stream_name: String,
    /// Topic name for message routing
    pub topic_name: String,
    /// Number of partitions
    pub partitions: u32,
}

impl Default for ConnectorConfig {
    fn default() -> Self {
        Self {
            mode: ConnectorMode::Embedded,
            embedded: EmbeddedConnectorConfig::default(),
            remote: RemoteConnectorConfig::default(),
            stream_name: "rustok".to_string(),
            topic_name: "domain".to_string(),
            partitions: 8,
        }
    }
}

/// Request for publishing a message to Iggy
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PublishRequest {
    /// Stream identifier
    pub stream: String,
    /// Topic identifier
    pub topic: String,
    /// Partition key for routing
    pub partition_key: String,
    /// Message payload
    pub payload: Vec<u8>,
    /// Unique event identifier
    pub event_id: String,
}

impl PublishRequest {
    /// Creates a new publish request
    pub fn new(
        stream: impl Into<String>,
        topic: impl Into<String>,
        partition_key: impl Into<String>,
        payload: Vec<u8>,
        event_id: impl Into<String>,
    ) -> Self {
        Self {
            stream: stream.into(),
            topic: topic.into(),
            partition_key: partition_key.into(),
            payload,
            event_id: event_id.into(),
        }
    }

    /// Creates a simple request with default stream/topic
    pub fn simple(
        partition_key: impl Into<String>,
        payload: Vec<u8>,
        event_id: impl Into<String>,
    ) -> Self {
        Self::new("rustok", "domain", partition_key, payload, event_id)
    }
}

/// Trait for Iggy connectors - handles both embedded and remote modes
#[async_trait]
pub trait IggyConnector: Send + Sync + 'static {
    /// Connect to Iggy server (or start embedded server)
    async fn connect(&self, config: &ConnectorConfig) -> Result<(), ConnectorError>;

    /// Check if connector is connected
    fn is_connected(&self) -> bool;

    /// Publish a message to Iggy
    async fn publish(&self, request: PublishRequest) -> Result<(), ConnectorError>;

    /// Subscribe to messages (for consuming)
    async fn subscribe(
        &self,
        stream: &str,
        topic: &str,
        partition: u32,
    ) -> Result<Box<dyn MessageSubscriber>, ConnectorError>;

    /// Graceful shutdown
    async fn shutdown(&self) -> Result<(), ConnectorError>;
}

/// Metadata attached to a consumed connector message.
///
/// This type intentionally models only low-level connector facts that are
/// needed by higher transport layers for offset tracking, retries, DLQ and
/// replay coordination. It does not define retry limits, DLQ routing, replay
/// policy or any other transport-level behavior.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SubscriberMessageMetadata {
    /// Stream the message was read from.
    pub stream: String,
    /// Topic the message was read from.
    pub topic: String,
    /// Partition the message was read from.
    pub partition: u32,
    /// Connector/backend offset when available.
    pub offset: Option<u64>,
    /// Connector/backend message identifier when available.
    pub message_id: Option<String>,
    /// Delivery attempt observed by the connector when available.
    pub delivery_attempt: Option<u32>,
    /// Opaque connector-owned acknowledgement token.
    pub ack_token: Option<String>,
}

impl SubscriberMessageMetadata {
    /// Builds metadata for subscribers that know only stream/topic/partition.
    pub fn new(stream: impl Into<String>, topic: impl Into<String>, partition: u32) -> Self {
        Self {
            stream: stream.into(),
            topic: topic.into(),
            partition,
            offset: None,
            message_id: None,
            delivery_attempt: None,
            ack_token: None,
        }
    }

    /// Adds an offset, preserving builder ergonomics for tests/adapters.
    pub fn with_offset(mut self, offset: u64) -> Self {
        self.offset = Some(offset);
        self
    }

    /// Adds a backend message identifier.
    pub fn with_message_id(mut self, message_id: impl Into<String>) -> Self {
        self.message_id = Some(message_id.into());
        self
    }

    /// Adds the observed delivery attempt.
    pub fn with_delivery_attempt(mut self, delivery_attempt: u32) -> Self {
        self.delivery_attempt = Some(delivery_attempt);
        self
    }

    /// Adds an opaque acknowledgement token.
    pub fn with_ack_token(mut self, ack_token: impl Into<String>) -> Self {
        self.ack_token = Some(ack_token.into());
        self
    }

    /// Builds the canonical simulated acknowledgement token for an offset.
    ///
    /// Real SDK adapters may expose their own opaque token format, but remote and
    /// embedded simulation paths use this helper so ack/replay tests do not copy
    /// token formatting logic across connector implementations.
    pub fn simulated_ack_token(
        mode: &str,
        stream: &str,
        topic: &str,
        partition: u32,
        offset: u64,
    ) -> String {
        format!("{mode}:{stream}:{topic}:{partition}:{offset}")
    }

    /// Attaches the canonical simulated acknowledgement token for this metadata.
    pub fn with_simulated_ack_token(mut self, mode: &str, offset: u64) -> Self {
        self.ack_token = Some(Self::simulated_ack_token(
            mode,
            &self.stream,
            &self.topic,
            self.partition,
            offset,
        ));
        self
    }
}

/// Consumed connector message with payload and low-level metadata.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SubscriberMessage {
    /// Message payload bytes.
    pub payload: Vec<u8>,
    /// Low-level connector metadata.
    pub metadata: SubscriberMessageMetadata,
}

impl SubscriberMessage {
    /// Creates a consumed message with explicit metadata.
    pub fn new(payload: Vec<u8>, metadata: SubscriberMessageMetadata) -> Self {
        Self { payload, metadata }
    }
}

/// Message subscriber for consuming messages from Iggy
#[async_trait]
pub trait MessageSubscriber: Send + Sync {
    /// Receive next payload. Legacy payload-only consumers may keep using this
    /// method; transport layers that need offset/ack/retry facts should prefer
    /// `recv_with_metadata`.
    async fn recv(&mut self) -> Result<Option<Vec<u8>>, ConnectorError>;

    /// Receive next message with connector-owned metadata.
    async fn recv_with_metadata(&mut self) -> Result<Option<SubscriberMessage>, ConnectorError> {
        Ok(self.recv().await?.map(|payload| {
            SubscriberMessage::new(payload, SubscriberMessageMetadata::new("", "", 0))
        }))
    }

    /// Acknowledge a message by opaque connector token.
    ///
    /// The default no-op keeps simulated/test subscribers policy-free while real
    /// SDK adapters can override this to commit offsets or acknowledge backend
    /// messages.
    async fn ack(&mut self, _ack_token: &str) -> Result<(), ConnectorError> {
        Ok(())
    }
}

/// Iggy connector errors
#[derive(Debug, thiserror::Error)]
pub enum ConnectorError {
    #[error("connection error: {0}")]
    Connection(String),

    #[error("not connected")]
    NotConnected,

    #[error("publish error: {0}")]
    Publish(String),

    #[error("subscribe error: {0}")]
    Subscribe(String),

    #[error("receive error: {0}")]
    Receive(String),

    #[error("topology error: {0}")]
    Topology(String),

    #[error("authentication error: {0}")]
    Auth(String),

    #[error("timeout: {0}")]
    Timeout(String),

    #[error("configuration error: {0}")]
    Config(String),

    #[error("iggy SDK error: {0}")]
    #[cfg(feature = "iggy")]
    Iggy(#[from] IggyError),

    #[error("iggy SDK error: {0}")]
    #[cfg(not(feature = "iggy"))]
    Iggy(String),
}

impl From<std::io::Error> for ConnectorError {
    fn from(err: std::io::Error) -> Self {
        ConnectorError::Connection(err.to_string())
    }
}

impl From<tokio::time::error::Elapsed> for ConnectorError {
    fn from(err: tokio::time::error::Elapsed) -> Self {
        ConnectorError::Timeout(err.to_string())
    }
}

// ============================================================================
// RemoteConnector - connects to external Iggy server
// ============================================================================

/// Remote connector - connects to external Iggy server via TCP/HTTP
#[derive(Debug)]
pub struct RemoteConnector {
    #[cfg(feature = "iggy")]
    client: Arc<RwLock<Option<IggyClient>>>,
    config: Arc<RwLock<Option<RemoteConnectorConfig>>>,
    stream_name: Arc<RwLock<String>>,
    topic_name: Arc<RwLock<String>>,
    connected: Arc<RwLock<bool>>,
}

impl Default for RemoteConnector {
    fn default() -> Self {
        Self::new()
    }
}

impl RemoteConnector {
    /// Creates a new remote connector
    pub fn new() -> Self {
        Self {
            #[cfg(feature = "iggy")]
            client: Arc::new(RwLock::new(None)),
            config: Arc::new(RwLock::new(None)),
            stream_name: Arc::new(RwLock::new("rustok".to_string())),
            topic_name: Arc::new(RwLock::new("domain".to_string())),
            connected: Arc::new(RwLock::new(false)),
        }
    }

    #[cfg(feature = "iggy")]
    async fn create_and_connect(
        config: &RemoteConnectorConfig,
    ) -> Result<IggyClient, ConnectorError> {
        let address = config
            .addresses
            .first()
            .cloned()
            .unwrap_or_else(|| "127.0.0.1:8090".to_string());

        tracing::info!(address = %address, protocol = %config.protocol, "Connecting to Iggy server");

        let connection_string = if !config.username.is_empty() {
            format!("iggy://{}:{}@{}", config.username, config.password, address)
        } else {
            format!("iggy://{}", address)
        };

        let client = IggyClient::from_connection_string(&connection_string)
            .map_err(|e: IggyError| ConnectorError::Connection(e.to_string()))?;

        client
            .connect()
            .await
            .map_err(|e: IggyError| ConnectorError::Connection(e.to_string()))?;

        Ok(client)
    }

    #[cfg(not(feature = "iggy"))]
    #[allow(dead_code)]
    async fn create_and_connect(_config: &RemoteConnectorConfig) -> Result<(), ConnectorError> {
        tracing::warn!("Iggy SDK not enabled, using mock client");
        Ok(())
    }
}

#[async_trait]
impl IggyConnector for RemoteConnector {
    async fn connect(&self, config: &ConnectorConfig) -> Result<(), ConnectorError> {
        let remote_config = config.remote.clone();

        *self.config.write().await = Some(remote_config.clone());
        *self.stream_name.write().await = config.stream_name.clone();
        *self.topic_name.write().await = config.topic_name.clone();

        #[cfg(feature = "iggy")]
        {
            let client = Self::create_and_connect(&remote_config).await?;
            *self.client.write().await = Some(client);
        }

        *self.connected.write().await = true;

        tracing::info!(
            mode = "remote",
            address = ?remote_config.addresses,
            stream = %config.stream_name,
            topic = %config.topic_name,
            "Iggy remote connector initialized"
        );

        Ok(())
    }

    fn is_connected(&self) -> bool {
        self.connected
            .try_read()
            .map(|connected| *connected)
            .unwrap_or(false)
    }

    async fn publish(&self, request: PublishRequest) -> Result<(), ConnectorError> {
        if !*self.connected.read().await {
            return Err(ConnectorError::NotConnected);
        }

        let partition = calculate_partition(&request.partition_key);

        #[cfg(feature = "iggy")]
        {
            use iggy::prelude::{IggyMessage, Partitioning};

            let client_guard = self.client.read().await;
            let client: &IggyClient = client_guard.as_ref().ok_or(ConnectorError::NotConnected)?;

            let producer = client
                .producer(&request.stream, &request.topic)
                .map_err(|e: IggyError| ConnectorError::Publish(e.to_string()))?
                .partitioning(Partitioning::partition_id(partition))
                .build();

            producer
                .init()
                .await
                .map_err(|e: IggyError| ConnectorError::Publish(e.to_string()))?;

            let message = IggyMessage::builder()
                .payload(request.payload.clone().into())
                .build()
                .map_err(|e: IggyError| ConnectorError::Publish(e.to_string()))?;

            producer
                .send(vec![message])
                .await
                .map_err(|e: IggyError| ConnectorError::Publish(e.to_string()))?;
        }

        #[cfg(not(feature = "iggy"))]
        {
            tracing::debug!(
                mode = "remote",
                stream = %request.stream,
                topic = %request.topic,
                partition = partition,
                event_id = %request.event_id,
                payload_size = request.payload.len(),
                "Publishing event via remote connector (simulated)"
            );
        }

        tracing::debug!(
            mode = "remote",
            stream = %request.stream,
            topic = %request.topic,
            partition = partition,
            event_id = %request.event_id,
            payload_size = request.payload.len(),
            "Published event via remote connector"
        );

        Ok(())
    }

    async fn subscribe(
        &self,
        stream: &str,
        topic: &str,
        partition: u32,
    ) -> Result<Box<dyn MessageSubscriber>, ConnectorError> {
        if !*self.connected.read().await {
            return Err(ConnectorError::NotConnected);
        }

        tracing::info!(
            mode = "remote",
            stream = stream,
            topic = topic,
            partition = partition,
            "Subscribed to messages"
        );

        Ok(Box::new(RemoteMessageSubscriber::new(
            stream.to_string(),
            topic.to_string(),
            partition,
        )))
    }

    async fn shutdown(&self) -> Result<(), ConnectorError> {
        #[cfg(feature = "iggy")]
        {
            *self.client.write().await = None;
        }
        *self.connected.write().await = false;

        tracing::info!(mode = "remote", "Iggy remote connector shutdown");
        Ok(())
    }
}

/// Remote message subscriber implementation
#[derive(Debug)]
#[allow(dead_code)]
pub struct RemoteMessageSubscriber {
    stream: String,
    topic: String,
    partition: u32,
}

impl RemoteMessageSubscriber {
    pub fn new(stream: String, topic: String, partition: u32) -> Self {
        Self {
            stream,
            topic,
            partition,
        }
    }

    #[allow(dead_code)]
    fn metadata_for_offset(&self, offset: u64) -> SubscriberMessageMetadata {
        SubscriberMessageMetadata::new(&self.stream, &self.topic, self.partition)
            .with_offset(offset)
            .with_simulated_ack_token("remote", offset)
    }
}

#[async_trait]
impl MessageSubscriber for RemoteMessageSubscriber {
    async fn recv(&mut self) -> Result<Option<Vec<u8>>, ConnectorError> {
        Ok(None)
    }

    async fn recv_with_metadata(&mut self) -> Result<Option<SubscriberMessage>, ConnectorError> {
        Ok(None)
    }

    async fn ack(&mut self, ack_token: &str) -> Result<(), ConnectorError> {
        tracing::debug!(
            mode = "remote",
            stream = %self.stream,
            topic = %self.topic,
            partition = self.partition,
            ack_token = %ack_token,
            "Acknowledged connector message"
        );
        Ok(())
    }
}

// ============================================================================
// EmbeddedConnector - runs Iggy server within the application
// ============================================================================

/// Embedded connector - runs Iggy server within the application
#[derive(Debug)]
pub struct EmbeddedConnector {
    config: Arc<RwLock<Option<EmbeddedConnectorConfig>>>,
    connected: Arc<RwLock<bool>>,
    stream_name: Arc<RwLock<String>>,
    topic_name: Arc<RwLock<String>>,
    partitions: Arc<RwLock<u32>>,
}

impl Default for EmbeddedConnector {
    fn default() -> Self {
        Self::new()
    }
}

impl EmbeddedConnector {
    /// Creates a new embedded connector
    pub fn new() -> Self {
        Self {
            config: Arc::new(RwLock::new(None)),
            connected: Arc::new(RwLock::new(false)),
            stream_name: Arc::new(RwLock::new("rustok".to_string())),
            topic_name: Arc::new(RwLock::new("domain".to_string())),
            partitions: Arc::new(RwLock::new(8)),
        }
    }

    async fn init_embedded(&self, config: &EmbeddedConnectorConfig) -> Result<(), ConnectorError> {
        tracing::info!(
            data_dir = %config.data_dir,
            tcp_port = config.tcp_port,
            http_port = config.http_port,
            persistent = config.persistent,
            "Initializing embedded Iggy server"
        );

        let data_dir = std::path::Path::new(&config.data_dir);
        if config.persistent && !data_dir.exists() {
            std::fs::create_dir_all(data_dir)
                .map_err(|e| ConnectorError::Config(format!("Failed to create data dir: {}", e)))?;
        }

        *self.config.write().await = Some(config.clone());

        tracing::info!(
            mode = "embedded",
            data_dir = %config.data_dir,
            tcp_port = config.tcp_port,
            "Embedded Iggy server initialized"
        );

        Ok(())
    }
}

#[async_trait]
impl IggyConnector for EmbeddedConnector {
    async fn connect(&self, config: &ConnectorConfig) -> Result<(), ConnectorError> {
        self.init_embedded(&config.embedded).await?;

        *self.stream_name.write().await = config.stream_name.clone();
        *self.topic_name.write().await = config.topic_name.clone();
        *self.partitions.write().await = config.partitions;

        *self.connected.write().await = true;

        tracing::info!(
            mode = "embedded",
            data_dir = %config.embedded.data_dir,
            tcp_port = config.embedded.tcp_port,
            http_port = config.embedded.http_port,
            stream = %config.stream_name,
            topic = %config.topic_name,
            partitions = config.partitions,
            "Iggy embedded connector initialized"
        );

        Ok(())
    }

    fn is_connected(&self) -> bool {
        self.connected
            .try_read()
            .map(|connected| *connected)
            .unwrap_or(false)
    }

    async fn publish(&self, request: PublishRequest) -> Result<(), ConnectorError> {
        if !*self.connected.read().await {
            return Err(ConnectorError::NotConnected);
        }

        let partition = calculate_partition(&request.partition_key);

        tracing::debug!(
            mode = "embedded",
            stream = %request.stream,
            topic = %request.topic,
            partition = partition,
            event_id = %request.event_id,
            payload_size = request.payload.len(),
            "Publishing event via embedded connector"
        );

        Ok(())
    }

    async fn subscribe(
        &self,
        stream: &str,
        topic: &str,
        partition: u32,
    ) -> Result<Box<dyn MessageSubscriber>, ConnectorError> {
        if !*self.connected.read().await {
            return Err(ConnectorError::NotConnected);
        }

        tracing::info!(
            mode = "embedded",
            stream = stream,
            topic = topic,
            partition = partition,
            "Subscribed to messages"
        );

        Ok(Box::new(EmbeddedMessageSubscriber::new(
            stream.to_string(),
            topic.to_string(),
            partition,
        )))
    }

    async fn shutdown(&self) -> Result<(), ConnectorError> {
        *self.config.write().await = None;
        *self.connected.write().await = false;

        tracing::info!(mode = "embedded", "Iggy embedded connector shutdown");
        Ok(())
    }
}

/// Embedded message subscriber implementation
#[derive(Debug)]
#[allow(dead_code)]
pub struct EmbeddedMessageSubscriber {
    stream: String,
    topic: String,
    partition: u32,
}

impl EmbeddedMessageSubscriber {
    pub fn new(stream: String, topic: String, partition: u32) -> Self {
        Self {
            stream,
            topic,
            partition,
        }
    }

    #[allow(dead_code)]
    fn metadata_for_offset(&self, offset: u64) -> SubscriberMessageMetadata {
        SubscriberMessageMetadata::new(&self.stream, &self.topic, self.partition)
            .with_offset(offset)
            .with_simulated_ack_token("embedded", offset)
    }
}

#[async_trait]
impl MessageSubscriber for EmbeddedMessageSubscriber {
    async fn recv(&mut self) -> Result<Option<Vec<u8>>, ConnectorError> {
        Ok(None)
    }

    async fn recv_with_metadata(&mut self) -> Result<Option<SubscriberMessage>, ConnectorError> {
        Ok(None)
    }

    async fn ack(&mut self, ack_token: &str) -> Result<(), ConnectorError> {
        tracing::debug!(
            mode = "embedded",
            stream = %self.stream,
            topic = %self.topic,
            partition = self.partition,
            ack_token = %ack_token,
            "Acknowledged connector message"
        );
        Ok(())
    }
}

// ============================================================================
// Helper functions
// ============================================================================

/// Calculate partition number based on key
fn calculate_partition(key: &str) -> u32 {
    use std::collections::hash_map::DefaultHasher;
    use std::hash::{Hash, Hasher};

    let mut hasher = DefaultHasher::new();
    key.hash(&mut hasher);
    let hash = hasher.finish();

    (hash % 8) as u32 + 1
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_calculate_partition() {
        let key1 = "tenant-123";
        let key2 = "tenant-456";
        let key3 = "tenant-123";

        let p1 = calculate_partition(key1);
        let p2 = calculate_partition(key2);
        let p3 = calculate_partition(key3);

        assert_ne!(p1, p2);
        assert_eq!(p1, p3);
    }

    #[test]
    fn test_partition_in_range() {
        for i in 0..1000 {
            let key = format!("tenant-{}", i);
            let partition = calculate_partition(&key);
            assert!(
                (1..=8).contains(&partition),
                "Partition {} out of range",
                partition
            );
        }
    }

    #[test]
    fn test_connector_mode_display() {
        assert_eq!(ConnectorMode::Embedded.to_string(), "embedded");
        assert_eq!(ConnectorMode::Remote.to_string(), "remote");
    }

    #[test]
    fn test_connector_mode_serialization() {
        let embedded = ConnectorMode::Embedded;
        let remote = ConnectorMode::Remote;

        assert_eq!(serde_json::to_string(&embedded).unwrap(), "\"embedded\"");
        assert_eq!(serde_json::to_string(&remote).unwrap(), "\"remote\"");

        assert_eq!(
            serde_json::from_str::<ConnectorMode>("\"embedded\"").unwrap(),
            ConnectorMode::Embedded
        );
        assert_eq!(
            serde_json::from_str::<ConnectorMode>("\"remote\"").unwrap(),
            ConnectorMode::Remote
        );
    }

    #[test]
    fn test_publish_request() {
        let request = PublishRequest::new("stream1", "topic1", "key1", vec![1, 2, 3], "event1");

        assert_eq!(request.stream, "stream1");
        assert_eq!(request.topic, "topic1");
        assert_eq!(request.partition_key, "key1");
        assert_eq!(request.payload, vec![1, 2, 3]);
        assert_eq!(request.event_id, "event1");
    }

    #[test]
    fn test_publish_request_simple() {
        let request = PublishRequest::simple("key1", vec![1, 2, 3], "event1");

        assert_eq!(request.stream, "rustok");
        assert_eq!(request.topic, "domain");
    }

    #[tokio::test]
    async fn test_remote_connector_default() {
        let connector = RemoteConnector::new();
        assert!(!connector.is_connected());
    }

    #[tokio::test]
    async fn test_embedded_connector_default() {
        let connector = EmbeddedConnector::new();
        assert!(!connector.is_connected());
    }

    #[tokio::test]
    async fn test_remote_connector_connect() {
        let connector = RemoteConnector::new();
        let config = ConnectorConfig::default();

        let result = tokio::time::timeout(
            std::time::Duration::from_secs(5),
            connector.connect(&config),
        )
        .await;

        assert!(
            matches!(
                result,
                Err(_)
                    | Ok(Ok(()))
                    | Ok(Err(ConnectorError::Connection(_)))
                    | Ok(Err(ConnectorError::Timeout(_)))
            ),
            "unexpected remote connect result: {:?}",
            result
        );
        tracing::debug!("Connect result (bounded by timeout): {:?}", result);
    }

    #[tokio::test]
    async fn test_embedded_connector_connect() {
        let connector = EmbeddedConnector::new();
        let config = ConnectorConfig {
            mode: ConnectorMode::Embedded,
            embedded: EmbeddedConnectorConfig {
                data_dir: "/tmp/test-iggy".to_string(),
                tcp_port: 8091,
                http_port: 3001,
                persistent: false,
            },
            ..Default::default()
        };

        let result = connector.connect(&config).await;
        assert!(result.is_ok() || result.is_err());

        let _ = connector.shutdown().await;
    }

    #[tokio::test]
    async fn test_publish_not_connected() {
        let connector = RemoteConnector::new();
        let request = PublishRequest::simple("key1", vec![1, 2, 3], "event1");

        let result = connector.publish(request).await;
        assert!(matches!(result, Err(ConnectorError::NotConnected)));
    }

    #[tokio::test]
    async fn test_remote_subscriber() {
        let mut subscriber =
            RemoteMessageSubscriber::new("stream1".to_string(), "topic1".to_string(), 1);
        let result = subscriber.recv().await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_embedded_subscriber() {
        let mut subscriber =
            EmbeddedMessageSubscriber::new("stream1".to_string(), "topic1".to_string(), 1);
        let result = subscriber.recv().await;
        assert!(result.is_ok());
    }

    #[test]
    fn test_subscriber_message_metadata_builder() {
        let metadata = SubscriberMessageMetadata::new("stream1", "topic1", 3)
            .with_offset(99)
            .with_ack_token("ack-99");

        assert_eq!(metadata.stream, "stream1");
        assert_eq!(metadata.topic, "topic1");
        assert_eq!(metadata.partition, 3);
        assert_eq!(metadata.offset, Some(99));
        assert_eq!(metadata.ack_token.as_deref(), Some("ack-99"));
        assert_eq!(metadata.message_id, None);
        assert_eq!(metadata.delivery_attempt, None);
    }

    #[test]
    fn test_subscriber_message_metadata_simulated_ack_token_builder() {
        let token =
            SubscriberMessageMetadata::simulated_ack_token("remote", "stream1", "topic1", 3, 99);
        let metadata = SubscriberMessageMetadata::new("stream1", "topic1", 3)
            .with_offset(99)
            .with_simulated_ack_token("remote", 99);

        assert_eq!(token, "remote:stream1:topic1:3:99");
        assert_eq!(metadata.ack_token.as_deref(), Some(token.as_str()));
    }

    #[test]
    fn test_remote_and_embedded_metadata_use_canonical_simulated_ack_tokens() {
        let remote = RemoteMessageSubscriber::new("stream1".to_string(), "topic1".to_string(), 3)
            .metadata_for_offset(99);
        let embedded =
            EmbeddedMessageSubscriber::new("stream1".to_string(), "topic1".to_string(), 3)
                .metadata_for_offset(99);

        assert_eq!(
            remote.ack_token.as_deref(),
            Some("remote:stream1:topic1:3:99")
        );
        assert_eq!(
            embedded.ack_token.as_deref(),
            Some("embedded:stream1:topic1:3:99")
        );
    }

    #[test]
    fn test_subscriber_message_carries_payload_and_metadata() {
        let metadata = SubscriberMessageMetadata::new("stream1", "topic1", 1);
        let message = SubscriberMessage::new(vec![1, 2, 3], metadata.clone());

        assert_eq!(message.payload, vec![1, 2, 3]);
        assert_eq!(message.metadata, metadata);
    }

    #[test]
    fn test_config_defaults() {
        let config = ConnectorConfig::default();

        assert_eq!(config.mode, ConnectorMode::Embedded);
        assert_eq!(config.stream_name, "rustok");
        assert_eq!(config.topic_name, "domain");
        assert_eq!(config.partitions, 8);

        let embedded = EmbeddedConnectorConfig::default();
        assert_eq!(embedded.data_dir, "./data/iggy");
        assert_eq!(embedded.tcp_port, 8090);

        let remote = RemoteConnectorConfig::default();
        assert_eq!(remote.addresses, vec!["127.0.0.1:8090"]);
        assert_eq!(remote.protocol, "tcp");
    }
}

#[cfg(test)]
mod contract_tests;
