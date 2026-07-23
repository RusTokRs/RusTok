//! Connector layer for Iggy transports.
//!
//! This module provides two connector implementations:
//! - `ExternalConnector`: connects to an external Iggy server via TCP
//! - `BundledConnector`: manages the bundled Iggy server process
//!
//! The connector handles connection lifecycle, message publishing, and graceful shutdown.
//!
//! # Usage
//!
//! ```rust,no_run
//! use rustok_iggy_connector::{ConnectorConfig, ConnectorMode, IggyConnector, ExternalConnector};
//!
//! #[tokio::main]
//! async fn main() -> Result<(), Box<dyn std::error::Error>> {
//!     let connector = ExternalConnector::new();
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

use std::{collections::BTreeMap, sync::Arc, time::Duration};

#[cfg(all(feature = "iggy", not(target_os = "windows")))]
use std::path::Path;

use async_trait::async_trait;
use serde::{Deserialize, Serialize};
#[cfg(all(feature = "iggy", not(target_os = "windows")))]
use tokio::{net::TcpStream, process::Command, time::sleep};
use tokio::{
    process::Child,
    sync::{Mutex, RwLock},
    time::timeout,
};

mod control;
#[cfg(feature = "migrations")]
pub mod migrations;

pub use control::{
    IggyConnectorConfigurationSnapshot, IggyConnectorControl, IggyConnectorSettingsInput,
    IggyConnectorUpdateOutcome, SharedIggyConnectorControl,
};

/// Whether this build can supervise the bundled Iggy artifact.
pub const fn bundled_runtime_supported() -> bool {
    cfg!(all(feature = "iggy", not(target_os = "windows")))
}

#[cfg(feature = "iggy")]
use futures_util::StreamExt;
#[cfg(feature = "iggy")]
use iggy::prelude::{Client, IggyClient, IggyConsumer, IggyError};

/// Connection mode for Iggy connector
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ConnectorMode {
    /// Bundled mode - manages the Iggy server artifact packaged by this module.
    #[default]
    Bundled,
    /// External mode - connects to an externally managed Iggy server.
    External,
}

impl std::fmt::Display for ConnectorMode {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ConnectorMode::Bundled => write!(f, "bundled"),
            ConnectorMode::External => write!(f, "external"),
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
            "bundled" => Ok(ConnectorMode::Bundled),
            "external" => Ok(ConnectorMode::External),
            _ => Err(serde::de::Error::custom(format!("Unknown mode: {}", s))),
        }
    }
}

/// Configuration for the bundled Iggy server managed by this connector.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BundledConnectorConfig {
    /// Absolute or PATH-resolved executable for the Iggy server.
    pub executable: String,
    /// Arguments passed directly to the server process without a shell.
    #[serde(default)]
    pub arguments: Vec<String>,
    /// Explicit environment passed to the bundled server process.
    #[serde(default)]
    pub environment: BTreeMap<String, String>,
    /// Directory for durable Iggy data (streams, topics, and messages).
    pub data_dir: String,
    /// TCP port where the managed server must become reachable.
    pub tcp_port: u16,
    /// HTTP port for the managed server dashboard (0 disables it).
    pub http_port: u16,
    /// Maximum time to wait for TCP readiness.
    #[serde(default = "default_startup_timeout_ms")]
    pub startup_timeout_ms: u64,
    /// Maximum time to wait for the managed process to terminate.
    #[serde(default = "default_shutdown_timeout_ms")]
    pub shutdown_timeout_ms: u64,
}

impl Default for BundledConnectorConfig {
    fn default() -> Self {
        Self {
            executable: "iggy-server".to_string(),
            arguments: Vec::new(),
            environment: BTreeMap::new(),
            data_dir: "./data/iggy".to_string(),
            tcp_port: 8090,
            http_port: 3000,
            startup_timeout_ms: default_startup_timeout_ms(),
            shutdown_timeout_ms: default_shutdown_timeout_ms(),
        }
    }
}

const fn default_startup_timeout_ms() -> u64 {
    30_000
}

const fn default_shutdown_timeout_ms() -> u64 {
    10_000
}

/// Configuration for an external Iggy server connection.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExternalConnectorConfig {
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
    /// Optional TLS server name override.
    #[serde(default)]
    pub tls_domain: Option<String>,
    /// Optional PEM CA certificate path for TLS validation.
    #[serde(default)]
    pub tls_ca_file: Option<String>,
}

impl Default for ExternalConnectorConfig {
    fn default() -> Self {
        Self {
            addresses: vec!["127.0.0.1:8090".to_string()],
            protocol: "tcp".to_string(),
            username: "iggy".to_string(),
            password: "iggy".to_string(),
            tls_enabled: false,
            tls_domain: None,
            tls_ca_file: None,
        }
    }
}

/// Main connector configuration combining bundled and external modes.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConnectorConfig {
    /// Connection mode: Bundled or External.
    pub mode: ConnectorMode,
    /// Configuration for bundled managed mode.
    pub bundled: BundledConnectorConfig,
    /// Configuration for external mode.
    pub external: ExternalConnectorConfig,
    /// Stream name for message routing
    pub stream_name: String,
    /// Topic name for message routing
    pub topic_name: String,
    /// Number of partitions
    pub partitions: u32,
    /// Replication factor for newly created topics.
    pub replication_factor: u8,
}

impl Default for ConnectorConfig {
    fn default() -> Self {
        Self {
            mode: ConnectorMode::Bundled,
            bundled: BundledConnectorConfig::default(),
            external: ExternalConnectorConfig::default(),
            stream_name: "rustok".to_string(),
            topic_name: "domain".to_string(),
            partitions: 8,
            replication_factor: 1,
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

/// Trait for Iggy connectors - handles managed bundled and external modes.
#[async_trait]
pub trait IggyConnector: Send + Sync + 'static {
    /// Connect to Iggy server (or start the managed bundled server).
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

    /// Opens a persistent consumer-group cursor. A cursor owns both message
    /// receipt and acknowledgement so an offset can only be committed by the
    /// exact backend consumer that observed it.
    ///
    /// This is intentionally separate from the legacy per-partition subscriber
    /// API. A real broker consumer group must not create one cursor to receive
    /// an event and another cursor to commit its offset.
    async fn open_consumer_group(
        &self,
        stream: &str,
        topic: &str,
        group_name: &str,
    ) -> Result<Box<dyn ConsumerCursor>, ConnectorError> {
        Err(ConnectorError::Config(format!(
            "persistent consumer groups are not supported for {stream}/{topic} ({group_name})"
        )))
    }

    /// Ensures that the stream and all required topics exist before producers
    /// or consumer groups are opened.
    async fn ensure_topology(
        &self,
        stream: &str,
        topics: &[&str],
        partitions: u32,
        replication_factor: u8,
    ) -> Result<(), ConnectorError> {
        let _ = (stream, topics, partitions, replication_factor);
        Err(ConnectorError::Config(
            "broker topology management is not supported by this connector".to_string(),
        ))
    }

    /// Graceful shutdown
    async fn shutdown(&self) -> Result<(), ConnectorError>;
}

/// Persistent external-broker cursor with explicit acknowledgement.
///
/// Callers must acknowledge a returned message before requesting another one.
/// This preserves at-least-once delivery when owner-side processing fails or a
/// worker becomes unavailable before its result is persisted.
#[async_trait]
pub trait ConsumerCursor: Send {
    /// Receives one message without committing its broker offset.
    async fn receive(&mut self) -> Result<Option<SubscriberMessage>, ConnectorError>;

    /// Commits the opaque token from the most recently received message.
    async fn acknowledge(&mut self, ack_token: &str) -> Result<(), ConnectorError>;
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
    /// Real SDK adapters expose their own opaque token format; legacy
    /// subscriber test doubles use this helper so ack/replay tests do not copy
    /// token formatting logic.
    pub fn simulated_ack_token(
        mode: &str,
        stream: &str,
        topic: &str,
        partition: u32,
        offset: u64,
    ) -> String {
        ConnectorAckToken::simulated(mode, stream, topic, partition, offset).encode()
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

/// Connector-owned acknowledgement token scope.
///
/// Tokens stay opaque to transport users, but connector implementations keep a
/// structured builder/parser internally so simulated and real SDK subscribers
/// validate stream/topic/partition/offset scope before acknowledging.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ConnectorAckToken {
    /// Canonical no-SDK simulation token used by legacy subscriber test doubles.
    Simulated {
        mode: String,
        stream: String,
        topic: String,
        partition: u32,
        offset: u64,
    },
    /// Canonical real Iggy SDK cursor token.
    ///
    /// The consumer id is intentionally opaque connector state; higher layers must
    /// not inspect it for retry/DLQ/replay policy decisions.
    IggySdk {
        stream: String,
        topic: String,
        partition: u32,
        offset: u64,
        consumer_id: String,
    },
}

impl ConnectorAckToken {
    const SIMULATED_PREFIX: &'static str = "sim";
    const IGGY_SDK_PREFIX: &'static str = "iggy-sdk";

    pub fn simulated(mode: &str, stream: &str, topic: &str, partition: u32, offset: u64) -> Self {
        Self::Simulated {
            mode: mode.to_string(),
            stream: stream.to_string(),
            topic: topic.to_string(),
            partition,
            offset,
        }
    }

    pub fn iggy_sdk(
        stream: &str,
        topic: &str,
        partition: u32,
        offset: u64,
        consumer_id: &str,
    ) -> Self {
        Self::IggySdk {
            stream: stream.to_string(),
            topic: topic.to_string(),
            partition,
            offset,
            consumer_id: consumer_id.to_string(),
        }
    }

    pub fn encode(&self) -> String {
        match self {
            Self::Simulated {
                mode,
                stream,
                topic,
                partition,
                offset,
            } => {
                format!(
                    "{}:{mode}:{stream}:{topic}:{partition}:{offset}",
                    Self::SIMULATED_PREFIX
                )
            }
            Self::IggySdk {
                stream,
                topic,
                partition,
                offset,
                consumer_id,
            } => {
                format!(
                    "{}:{stream}:{topic}:{partition}:{offset}:{consumer_id}",
                    Self::IGGY_SDK_PREFIX
                )
            }
        }
    }

    pub fn decode(token: &str) -> Result<Self, ConnectorError> {
        let parts: Vec<&str> = token.split(':').collect();
        match parts.as_slice() {
            [
                Self::SIMULATED_PREFIX,
                mode,
                stream,
                topic,
                partition,
                offset,
            ] => Ok(Self::simulated(
                mode,
                stream,
                topic,
                partition.parse().map_err(|_| {
                    ConnectorError::Config("invalid simulated ack partition".to_string())
                })?,
                offset.parse().map_err(|_| {
                    ConnectorError::Config("invalid simulated ack offset".to_string())
                })?,
            )),
            [
                Self::IGGY_SDK_PREFIX,
                stream,
                topic,
                partition,
                offset,
                consumer_id,
            ] => Ok(Self::iggy_sdk(
                stream,
                topic,
                partition
                    .parse()
                    .map_err(|_| ConnectorError::Config("invalid SDK ack partition".to_string()))?,
                offset
                    .parse()
                    .map_err(|_| ConnectorError::Config("invalid SDK ack offset".to_string()))?,
                consumer_id,
            )),
            _ => Err(ConnectorError::Config(
                "unsupported connector ack token".to_string(),
            )),
        }
    }

    pub fn matches_scope(&self, stream: &str, topic: &str, partition: u32) -> bool {
        match self {
            Self::Simulated {
                stream: token_stream,
                topic: token_topic,
                partition: token_partition,
                ..
            }
            | Self::IggySdk {
                stream: token_stream,
                topic: token_topic,
                partition: token_partition,
                ..
            } => token_stream == stream && token_topic == topic && *token_partition == partition,
        }
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
// ExternalConnector - connects to external Iggy server
// ============================================================================

/// External connector - connects to an external Iggy server via TCP.
#[derive(Debug)]
pub struct ExternalConnector {
    #[cfg(feature = "iggy")]
    client: Arc<RwLock<Option<IggyClient>>>,
    config: Arc<RwLock<Option<ExternalConnectorConfig>>>,
    stream_name: Arc<RwLock<String>>,
    topic_name: Arc<RwLock<String>>,
    partitions: Arc<RwLock<u32>>,
    replication_factor: Arc<RwLock<u8>>,
    connected: Arc<RwLock<bool>>,
}

impl Default for ExternalConnector {
    fn default() -> Self {
        Self::new()
    }
}

impl ExternalConnector {
    /// Creates a new external connector
    pub fn new() -> Self {
        Self {
            #[cfg(feature = "iggy")]
            client: Arc::new(RwLock::new(None)),
            config: Arc::new(RwLock::new(None)),
            stream_name: Arc::new(RwLock::new("rustok".to_string())),
            topic_name: Arc::new(RwLock::new("domain".to_string())),
            partitions: Arc::new(RwLock::new(8)),
            replication_factor: Arc::new(RwLock::new(1)),
            connected: Arc::new(RwLock::new(false)),
        }
    }

    #[cfg_attr(not(any(feature = "iggy", test)), allow(dead_code))]
    fn connection_string(config: &ExternalConnectorConfig) -> Result<String, ConnectorError> {
        if config.protocol != "tcp" {
            return Err(ConnectorError::Config(
                "persistent Iggy consumer groups require the tcp protocol".to_string(),
            ));
        }
        let address = config
            .addresses
            .first()
            .cloned()
            .unwrap_or_else(|| "127.0.0.1:8090".to_string());

        if config.username.is_empty() != config.password.is_empty() {
            return Err(ConnectorError::Config(
                "Iggy username and password must either both be set or both be empty".to_string(),
            ));
        }
        validate_connection_string_component(&config.username, "username", &[':', '@'])?;
        validate_connection_string_component(&config.password, "password", &[':', '@'])?;
        if let Some(domain) = config.tls_domain.as_deref() {
            validate_connection_string_component(domain, "tls_domain", &['?', '&', '='])?;
        }
        if let Some(ca_file) = config.tls_ca_file.as_deref() {
            validate_connection_string_component(ca_file, "tls_ca_file", &['?', '&', '='])?;
        }

        tracing::info!(address = %address, protocol = %config.protocol, "Connecting to Iggy server");

        let mut connection_string = if !config.username.is_empty() {
            format!("iggy://{}:{}@{}", config.username, config.password, address)
        } else {
            format!("iggy://{}", address)
        };
        let mut options = Vec::new();
        if config.tls_enabled {
            options.push("tls=true".to_string());
        }
        if let Some(domain) = config
            .tls_domain
            .as_deref()
            .filter(|domain| !domain.is_empty())
        {
            options.push(format!("tls_domain={domain}"));
        }
        if let Some(ca_file) = config
            .tls_ca_file
            .as_deref()
            .filter(|ca_file| !ca_file.is_empty())
        {
            options.push(format!("tls_ca_file={ca_file}"));
        }
        if !options.is_empty() {
            connection_string.push('?');
            connection_string.push_str(&options.join("&"));
        }

        Ok(connection_string)
    }

    #[cfg(feature = "iggy")]
    async fn create_and_connect(
        config: &ExternalConnectorConfig,
    ) -> Result<IggyClient, ConnectorError> {
        let connection_string = Self::connection_string(config)?;
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
    async fn create_and_connect(_config: &ExternalConnectorConfig) -> Result<(), ConnectorError> {
        Err(ConnectorError::Config(
            "external Iggy requires the `iggy` connector feature".to_string(),
        ))
    }
}

#[async_trait]
impl IggyConnector for ExternalConnector {
    async fn connect(&self, config: &ConnectorConfig) -> Result<(), ConnectorError> {
        if config.partitions == 0 {
            return Err(ConnectorError::Config(
                "Iggy topic partitions must be greater than zero".to_string(),
            ));
        }
        if config.replication_factor == 0 {
            return Err(ConnectorError::Config(
                "Iggy topic replication_factor must be greater than zero".to_string(),
            ));
        }
        let remote_config = config.external.clone();

        *self.config.write().await = Some(remote_config.clone());
        *self.stream_name.write().await = config.stream_name.clone();
        *self.topic_name.write().await = config.topic_name.clone();
        *self.partitions.write().await = config.partitions;
        *self.replication_factor.write().await = config.replication_factor;

        #[cfg(feature = "iggy")]
        {
            let client = Self::create_and_connect(&remote_config).await?;
            *self.client.write().await = Some(client);
        }

        *self.connected.write().await = true;

        tracing::info!(
            mode = "external",
            address = ?remote_config.addresses,
            stream = %config.stream_name,
            topic = %config.topic_name,
            "Iggy external connector initialized"
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

        let partitions = *self.partitions.read().await;
        let replication_factor = *self.replication_factor.read().await;
        let partition = calculate_partition(&request.partition_key, partitions);

        #[cfg(feature = "iggy")]
        {
            use iggy::prelude::{IggyMessage, Partitioning};

            let client_guard = self.client.read().await;
            let client: &IggyClient = client_guard.as_ref().ok_or(ConnectorError::NotConnected)?;

            let producer = client
                .producer(&request.stream, &request.topic)
                .map_err(|e: IggyError| ConnectorError::Publish(e.to_string()))?
                .partitioning(Partitioning::partition_id(partition))
                .create_stream_if_not_exists()
                .create_topic_if_not_exists(
                    partitions,
                    Some(replication_factor),
                    Default::default(),
                    Default::default(),
                )
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
                mode = "external",
                stream = %request.stream,
                topic = %request.topic,
                partition = partition,
                event_id = %request.event_id,
                payload_size = request.payload.len(),
                "Publishing event via external connector (simulated)"
            );
        }

        tracing::debug!(
            mode = "external",
            stream = %request.stream,
            topic = %request.topic,
            partition = partition,
            event_id = %request.event_id,
            payload_size = request.payload.len(),
            "Published event via external connector"
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
            mode = "external",
            stream = stream,
            topic = topic,
            partition = partition,
            "Subscribed to messages"
        );

        Ok(Box::new(ExternalMessageSubscriber::new(
            stream.to_string(),
            topic.to_string(),
            partition,
        )))
    }

    async fn open_consumer_group(
        &self,
        stream: &str,
        topic: &str,
        group_name: &str,
    ) -> Result<Box<dyn ConsumerCursor>, ConnectorError> {
        if !*self.connected.read().await {
            return Err(ConnectorError::NotConnected);
        }

        #[cfg(feature = "iggy")]
        {
            let client_guard = self.client.read().await;
            let client: &IggyClient = client_guard.as_ref().ok_or(ConnectorError::NotConnected)?;
            let mut consumer = client
                .consumer_group(group_name, stream, topic)
                .map_err(|error: IggyError| ConnectorError::Subscribe(error.to_string()))?
                .commit_failed_messages()
                .build();
            consumer
                .init()
                .await
                .map_err(|error: IggyError| ConnectorError::Subscribe(error.to_string()))?;

            tracing::info!(
                mode = "external",
                stream,
                topic,
                consumer_group = group_name,
                "Opened persistent Iggy consumer-group cursor"
            );

            return Ok(Box::new(ExternalConsumerGroupCursor::new(
                consumer, stream, topic, group_name,
            )));
        }

        #[cfg(not(feature = "iggy"))]
        {
            let _ = (stream, topic, group_name);
            Err(ConnectorError::Config(
                "external Iggy consumer groups require the `iggy` feature".to_string(),
            ))
        }
    }

    async fn ensure_topology(
        &self,
        stream: &str,
        topics: &[&str],
        partitions: u32,
        replication_factor: u8,
    ) -> Result<(), ConnectorError> {
        if !*self.connected.read().await {
            return Err(ConnectorError::NotConnected);
        }
        if partitions == 0 || replication_factor == 0 {
            return Err(ConnectorError::Config(
                "Iggy topology requires non-zero partitions and replication_factor".to_string(),
            ));
        }

        #[cfg(feature = "iggy")]
        {
            let client_guard = self.client.read().await;
            let client: &IggyClient = client_guard.as_ref().ok_or(ConnectorError::NotConnected)?;
            for topic in topics {
                let producer = client
                    .producer(stream, topic)
                    .map_err(|error: IggyError| ConnectorError::Topology(error.to_string()))?
                    .create_stream_if_not_exists()
                    .create_topic_if_not_exists(
                        partitions,
                        Some(replication_factor),
                        Default::default(),
                        Default::default(),
                    )
                    .build();
                producer
                    .init()
                    .await
                    .map_err(|error: IggyError| ConnectorError::Topology(error.to_string()))?;
            }
            return Ok(());
        }

        #[cfg(not(feature = "iggy"))]
        {
            let _ = (stream, topics, partitions, replication_factor);
            Err(ConnectorError::Config(
                "broker topology management requires the `iggy` feature".to_string(),
            ))
        }
    }

    async fn shutdown(&self) -> Result<(), ConnectorError> {
        #[cfg(feature = "iggy")]
        {
            *self.client.write().await = None;
        }
        *self.connected.write().await = false;

        tracing::info!(mode = "external", "Iggy external connector shutdown");
        Ok(())
    }
}

/// Real external Iggy consumer-group cursor. It permits only one outstanding
/// delivery: Iggy SDK offsets are cursor-scoped, so receiving again before an
/// acknowledgement could commit the wrong partition or skip a redelivery.
#[cfg(feature = "iggy")]
pub struct ExternalConsumerGroupCursor {
    consumer: IggyConsumer,
    stream: String,
    topic: String,
    group_name: String,
    pending: Option<(u32, u64)>,
}

#[cfg(feature = "iggy")]
impl ExternalConsumerGroupCursor {
    fn new(consumer: IggyConsumer, stream: &str, topic: &str, group_name: &str) -> Self {
        Self {
            consumer,
            stream: stream.to_string(),
            topic: topic.to_string(),
            group_name: group_name.to_string(),
            pending: None,
        }
    }
}

#[cfg(feature = "iggy")]
#[async_trait]
impl ConsumerCursor for ExternalConsumerGroupCursor {
    async fn receive(&mut self) -> Result<Option<SubscriberMessage>, ConnectorError> {
        if self.pending.is_some() {
            return Err(ConnectorError::Receive(
                "acknowledge the outstanding consumer-group delivery before receiving another"
                    .to_string(),
            ));
        }

        let received = match self.consumer.next().await {
            Some(Ok(message)) => message,
            Some(Err(error)) => return Err(ConnectorError::Receive(error.to_string())),
            None => return Ok(None),
        };
        // `current_offset` tracks the cursor's polling state. Commit the
        // message header offset instead, because that is the exact delivery
        // acknowledged by this cursor.
        let offset = received.message.header.offset;
        let partition = received.partition_id;
        self.pending = Some((partition, offset));
        let ack_token = ConnectorAckToken::iggy_sdk(
            &self.stream,
            &self.topic,
            partition,
            offset,
            &self.group_name,
        )
        .encode();

        Ok(Some(SubscriberMessage::new(
            received.message.payload.to_vec(),
            SubscriberMessageMetadata::new(&self.stream, &self.topic, partition)
                .with_offset(offset)
                .with_ack_token(ack_token),
        )))
    }

    async fn acknowledge(&mut self, ack_token: &str) -> Result<(), ConnectorError> {
        let token = ConnectorAckToken::decode(ack_token)?;
        let (partition, offset) = self.pending.ok_or_else(|| {
            ConnectorError::Config("consumer-group cursor has no outstanding delivery".to_string())
        })?;
        let ConnectorAckToken::IggySdk {
            stream,
            topic,
            partition: token_partition,
            offset: token_offset,
            consumer_id,
        } = token
        else {
            return Err(ConnectorError::Config(
                "real Iggy cursor requires an Iggy SDK acknowledgement token".to_string(),
            ));
        };

        if stream != self.stream
            || topic != self.topic
            || token_partition != partition
            || token_offset != offset
            || consumer_id != self.group_name
        {
            return Err(ConnectorError::Config(
                "ack token does not match the outstanding Iggy consumer-group delivery".to_string(),
            ));
        }

        self.consumer
            .store_offset(offset, Some(partition))
            .await
            .map_err(|error: IggyError| ConnectorError::Receive(error.to_string()))?;
        self.pending = None;
        Ok(())
    }
}

/// External message subscriber implementation
#[derive(Debug)]
#[allow(dead_code)]
pub struct ExternalMessageSubscriber {
    stream: String,
    topic: String,
    partition: u32,
}

impl ExternalMessageSubscriber {
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
            .with_simulated_ack_token("external", offset)
    }
}

#[async_trait]
impl MessageSubscriber for ExternalMessageSubscriber {
    async fn recv(&mut self) -> Result<Option<Vec<u8>>, ConnectorError> {
        Ok(None)
    }

    async fn recv_with_metadata(&mut self) -> Result<Option<SubscriberMessage>, ConnectorError> {
        Ok(None)
    }

    async fn ack(&mut self, ack_token: &str) -> Result<(), ConnectorError> {
        let token = ConnectorAckToken::decode(ack_token)?;
        if !token.matches_scope(&self.stream, &self.topic, self.partition) {
            return Err(ConnectorError::Config(
                "ack token scope does not match external subscriber".to_string(),
            ));
        }
        tracing::debug!(
            mode = "external",
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
// BundledConnector - manages a native Iggy server process
// ============================================================================

/// A durable single-node Iggy deployment managed by the host application.
///
/// The server itself remains a separate native process. This connector owns
/// only its lifecycle and delegates all broker I/O to `ExternalConnector`, so
/// bundled and external deployments share the same real SDK behaviour.
#[derive(Debug)]
pub struct BundledConnector {
    external: ExternalConnector,
    child: Arc<Mutex<Option<Child>>>,
    config: Arc<RwLock<Option<BundledConnectorConfig>>>,
}

impl Default for BundledConnector {
    fn default() -> Self {
        Self::new()
    }
}

impl BundledConnector {
    /// Creates a connector that starts one configured native Iggy server.
    pub fn new() -> Self {
        Self {
            external: ExternalConnector::new(),
            child: Arc::new(Mutex::new(None)),
            config: Arc::new(RwLock::new(None)),
        }
    }

    #[cfg(any(test, not(target_os = "windows")))]
    fn bundled_address(config: &ConnectorConfig) -> Result<String, ConnectorError> {
        if config.external.protocol != "tcp" {
            return Err(ConnectorError::Config(
                "bundled Iggy mode requires the tcp protocol".to_string(),
            ));
        }
        if config.external.tls_enabled {
            return Err(ConnectorError::Config(
                "bundled Iggy mode uses a loopback plaintext TCP connection; configure TLS with external mode"
                    .to_string(),
            ));
        }
        if config.external.addresses.len() != 1 {
            return Err(ConnectorError::Config(
                "bundled Iggy mode requires exactly one loopback external address".to_string(),
            ));
        }

        let address = &config.external.addresses[0];
        let (host, port) = address.rsplit_once(':').ok_or_else(|| {
            ConnectorError::Config("bundled Iggy address must be host:port".to_string())
        })?;
        let host = host.trim_matches(['[', ']']);
        let port: u16 = port.parse().map_err(|_| {
            ConnectorError::Config("bundled Iggy address has an invalid port".to_string())
        })?;

        if !matches!(host, "127.0.0.1" | "localhost" | "::1") || port != config.bundled.tcp_port {
            return Err(ConnectorError::Config(format!(
                "bundled Iggy address must be a loopback address on TCP port {}",
                config.bundled.tcp_port
            )));
        }

        Ok(address.clone())
    }

    #[cfg(all(feature = "iggy", not(target_os = "windows")))]
    fn start_process(config: &ConnectorConfig) -> Result<Child, ConnectorError> {
        if config.bundled.executable.trim().is_empty() {
            return Err(ConnectorError::Config(
                "bundled Iggy executable must not be empty".to_string(),
            ));
        }
        if config.external.username.trim().is_empty() || config.external.password.is_empty() {
            return Err(ConnectorError::Config(
                "bundled Iggy mode requires non-empty root credentials".to_string(),
            ));
        }

        let data_dir = Path::new(&config.bundled.data_dir);
        std::fs::create_dir_all(data_dir).map_err(|error| {
            ConnectorError::Config(format!(
                "failed to create bundled Iggy data directory: {error}"
            ))
        })?;
        let data_dir = std::fs::canonicalize(data_dir).map_err(|error| {
            ConnectorError::Config(format!(
                "failed to resolve bundled Iggy data directory: {error}"
            ))
        })?;

        let mut command = Command::new(&config.bundled.executable);
        command
            .args(&config.bundled.arguments)
            .current_dir(&data_dir)
            .kill_on_drop(true)
            .envs(&config.bundled.environment)
            .env("IGGY_SYSTEM_PATH", &data_dir)
            .env("IGGY_TCP_ENABLED", "true")
            .env(
                "IGGY_TCP_ADDRESS",
                format!("127.0.0.1:{}", config.bundled.tcp_port),
            )
            .env("IGGY_ROOT_USERNAME", &config.external.username)
            .env("IGGY_ROOT_PASSWORD", &config.external.password);

        if config.bundled.http_port == 0 {
            command.env("IGGY_HTTP_ENABLED", "false");
        } else {
            command.env("IGGY_HTTP_ENABLED", "true").env(
                "IGGY_HTTP_ADDRESS",
                format!("127.0.0.1:{}", config.bundled.http_port),
            );
        }

        command.spawn().map_err(|error| {
            ConnectorError::Connection(format!(
                "failed to start bundled Iggy executable '{}': {error}",
                config.bundled.executable
            ))
        })
    }

    async fn stop_child(&self, shutdown_timeout_ms: u64) -> Result<(), ConnectorError> {
        let Some(mut child) = self.child.lock().await.take() else {
            return Ok(());
        };

        let pid = child.id();
        let _ = child.start_kill();
        match timeout(Duration::from_millis(shutdown_timeout_ms), child.wait()).await {
            Ok(Ok(status)) => {
                tracing::info!(
                    ?pid,
                    ?status,
                    mode = "bundled",
                    "Bundled Iggy process stopped"
                );
                Ok(())
            }
            Ok(Err(error)) => Err(ConnectorError::Connection(format!(
                "failed to wait for bundled Iggy process: {error}"
            ))),
            Err(_) => {
                child.kill().await.map_err(|error| {
                    ConnectorError::Timeout(format!(
                        "bundled Iggy process did not stop and could not be terminated: {error}"
                    ))
                })?;
                let _ = child.wait().await;
                Err(ConnectorError::Timeout(
                    "bundled Iggy process did not stop within the configured timeout".to_string(),
                ))
            }
        }
    }

    #[cfg(all(feature = "iggy", not(target_os = "windows")))]
    async fn wait_until_ready(
        &self,
        config: &ConnectorConfig,
        address: &str,
    ) -> Result<(), ConnectorError> {
        let deadline =
            tokio::time::Instant::now() + Duration::from_millis(config.bundled.startup_timeout_ms);
        let mut last_error = None;

        loop {
            if let Some(status) = self
                .child
                .lock()
                .await
                .as_mut()
                .ok_or_else(|| ConnectorError::NotConnected)?
                .try_wait()
                .map_err(|error| ConnectorError::Connection(error.to_string()))?
            {
                return Err(ConnectorError::Connection(format!(
                    "bundled Iggy process exited before readiness with status {status}"
                )));
            }

            if TcpStream::connect(address).await.is_ok() {
                match self.external.connect(config).await {
                    Ok(()) => return Ok(()),
                    Err(error) => last_error = Some(error),
                }
            }

            if tokio::time::Instant::now() >= deadline {
                break;
            }
            sleep(Duration::from_millis(100)).await;
        }

        Err(ConnectorError::Timeout(format!(
            "bundled Iggy was not ready at {address} within {} ms{}",
            config.bundled.startup_timeout_ms,
            last_error
                .map(|error| format!("; last connection error: {error}"))
                .unwrap_or_default()
        )))
    }
}

#[async_trait]
impl IggyConnector for BundledConnector {
    async fn connect(&self, config: &ConnectorConfig) -> Result<(), ConnectorError> {
        #[cfg(target_os = "windows")]
        {
            let _ = config;
            return Err(ConnectorError::Config(
                "bundled Iggy mode is unavailable on Windows because upstream iggy-server does not support Windows; use external mode with an Iggy host on a supported platform"
                    .to_string(),
            ));
        }

        #[cfg(all(not(target_os = "windows"), not(feature = "iggy")))]
        {
            let _ = config;
            return Err(ConnectorError::Config(
                "bundled Iggy mode requires the `iggy` connector feature".to_string(),
            ));
        }

        #[cfg(all(not(target_os = "windows"), feature = "iggy"))]
        {
            let address = Self::bundled_address(config)?;
            if self.child.lock().await.is_some() {
                return Err(ConnectorError::Config(
                    "bundled Iggy connector is already managing a process".to_string(),
                ));
            }

            let child = Self::start_process(config)?;
            let pid = child.id();
            *self.child.lock().await = Some(child);
            *self.config.write().await = Some(config.bundled.clone());

            if let Err(error) = self.wait_until_ready(config, &address).await {
                let _ = self.stop_child(config.bundled.shutdown_timeout_ms).await;
                *self.config.write().await = None;
                return Err(error);
            }

            tracing::info!(
                ?pid,
                mode = "bundled",
                address = %address,
                data_dir = %config.bundled.data_dir,
                "Bundled Iggy connector initialized"
            );
            Ok(())
        }
    }

    fn is_connected(&self) -> bool {
        self.external.is_connected()
    }

    async fn publish(&self, request: PublishRequest) -> Result<(), ConnectorError> {
        self.external.publish(request).await
    }

    async fn subscribe(
        &self,
        stream: &str,
        topic: &str,
        partition: u32,
    ) -> Result<Box<dyn MessageSubscriber>, ConnectorError> {
        self.external.subscribe(stream, topic, partition).await
    }

    async fn open_consumer_group(
        &self,
        stream: &str,
        topic: &str,
        group_name: &str,
    ) -> Result<Box<dyn ConsumerCursor>, ConnectorError> {
        self.external
            .open_consumer_group(stream, topic, group_name)
            .await
    }

    async fn ensure_topology(
        &self,
        stream: &str,
        topics: &[&str],
        partitions: u32,
        replication_factor: u8,
    ) -> Result<(), ConnectorError> {
        self.external
            .ensure_topology(stream, topics, partitions, replication_factor)
            .await
    }

    async fn shutdown(&self) -> Result<(), ConnectorError> {
        let remote_result = self.external.shutdown().await;
        let shutdown_timeout_ms = self
            .config
            .write()
            .await
            .take()
            .map(|config| config.shutdown_timeout_ms)
            .unwrap_or_else(default_shutdown_timeout_ms);
        let process_result = self.stop_child(shutdown_timeout_ms).await;
        remote_result.and(process_result)
    }
}

// ============================================================================
// Helper functions
// ============================================================================

/// Validate a component interpolated into Iggy's SDK connection-string parser.
///
/// The SDK parser does not URL-decode values. Reject its structural delimiters
/// instead of generating a string that authenticates with unintended values.
#[cfg_attr(not(any(feature = "iggy", test)), allow(dead_code))]
fn validate_connection_string_component(
    value: &str,
    field: &str,
    forbidden: &[char],
) -> Result<(), ConnectorError> {
    if let Some(delimiter) = value
        .chars()
        .find(|character| forbidden.contains(character))
    {
        return Err(ConnectorError::Config(format!(
            "Iggy {field} contains unsupported connection-string delimiter '{delimiter}'"
        )));
    }

    Ok(())
}

/// Calculate partition number based on key
fn calculate_partition(key: &str, partitions: u32) -> u32 {
    use std::collections::hash_map::DefaultHasher;
    use std::hash::{Hash, Hasher};

    let mut hasher = DefaultHasher::new();
    key.hash(&mut hasher);
    let hash = hasher.finish();

    (hash % u64::from(partitions)) as u32 + 1
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
        assert_eq!(ConnectorMode::Bundled.to_string(), "bundled");
        assert_eq!(ConnectorMode::External.to_string(), "external");
    }

    #[test]
    fn test_connector_mode_serialization() {
        let bundled = ConnectorMode::Bundled;
        let external = ConnectorMode::External;

        assert_eq!(serde_json::to_string(&bundled).unwrap(), "\"bundled\"");
        assert_eq!(serde_json::to_string(&external).unwrap(), "\"external\"");

        assert_eq!(
            serde_json::from_str::<ConnectorMode>("\"bundled\"").unwrap(),
            ConnectorMode::Bundled
        );
        assert_eq!(
            serde_json::from_str::<ConnectorMode>("\"external\"").unwrap(),
            ConnectorMode::External
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
        let connector = ExternalConnector::new();
        assert!(!connector.is_connected());
    }

    #[tokio::test]
    async fn test_local_connector_default() {
        let connector = BundledConnector::new();
        assert!(!connector.is_connected());
    }

    #[cfg(target_os = "windows")]
    #[tokio::test]
    async fn local_connector_explains_the_upstream_windows_constraint() {
        let connector = BundledConnector::new();
        let error = connector
            .connect(&ConnectorConfig::default())
            .await
            .expect_err("upstream iggy-server does not support Windows");

        assert!(matches!(error, ConnectorError::Config(message) if message.contains("Windows")));
    }

    #[tokio::test]
    async fn test_remote_connector_connect() {
        let connector = ExternalConnector::new();
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
                    | Ok(Err(ConnectorError::Config(_)))
            ),
            "unexpected external connect result: {:?}",
            result
        );
        tracing::debug!("Connect result (bounded by timeout): {:?}", result);
    }

    #[test]
    fn test_local_connector_rejects_a_non_loopback_broker_address() {
        let config = ConnectorConfig {
            mode: ConnectorMode::Bundled,
            external: ExternalConnectorConfig {
                addresses: vec!["192.0.2.1:8090".to_string()],
                ..ExternalConnectorConfig::default()
            },
            ..Default::default()
        };

        let result = BundledConnector::bundled_address(&config);
        assert!(matches!(result, Err(ConnectorError::Config(_))));
    }

    #[test]
    fn test_local_connector_rejects_tls_or_non_tcp_configuration() {
        let mut config = ConnectorConfig::default();
        config.external.tls_enabled = true;
        assert!(matches!(
            BundledConnector::bundled_address(&config),
            Err(ConnectorError::Config(_))
        ));

        config.external.tls_enabled = false;
        config.external.protocol = "http".to_string();
        assert!(matches!(
            BundledConnector::bundled_address(&config),
            Err(ConnectorError::Config(_))
        ));
    }

    #[tokio::test]
    async fn test_publish_not_connected() {
        let connector = ExternalConnector::new();
        let request = PublishRequest::simple("key1", vec![1, 2, 3], "event1");

        let result = connector.publish(request).await;
        assert!(matches!(result, Err(ConnectorError::NotConnected)));
    }

    #[tokio::test]
    async fn test_remote_subscriber() {
        let mut subscriber =
            ExternalMessageSubscriber::new("stream1".to_string(), "topic1".to_string(), 1);
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
            SubscriberMessageMetadata::simulated_ack_token("external", "stream1", "topic1", 3, 99);
        let metadata = SubscriberMessageMetadata::new("stream1", "topic1", 3)
            .with_offset(99)
            .with_simulated_ack_token("external", 99);

        assert_eq!(token, "sim:external:stream1:topic1:3:99");
        assert_eq!(metadata.ack_token.as_deref(), Some(token.as_str()));
    }

    #[test]
    fn test_remote_metadata_uses_canonical_simulated_ack_tokens() {
        let external =
            ExternalMessageSubscriber::new("stream1".to_string(), "topic1".to_string(), 3)
                .metadata_for_offset(99);

        assert_eq!(
            external.ack_token.as_deref(),
            Some("sim:external:stream1:topic1:3:99")
        );
    }

    #[test]
    fn remote_connection_string_preserves_tls_configuration() {
        let connection = ExternalConnector::connection_string(&ExternalConnectorConfig {
            addresses: vec!["iggy.internal:8090".to_string()],
            protocol: "tcp".to_string(),
            username: "service".to_string(),
            password: "secret".to_string(),
            tls_enabled: true,
            tls_domain: Some("iggy.internal".to_string()),
            tls_ca_file: Some("C:/certs/iggy-ca.pem".to_string()),
        })
        .expect("valid TCP connection configuration");

        assert_eq!(
            connection,
            "iggy://service:secret@iggy.internal:8090?tls=true&tls_domain=iggy.internal&tls_ca_file=C:/certs/iggy-ca.pem"
        );
    }

    #[test]
    fn remote_connection_rejects_a_non_tcp_protocol() {
        let error = ExternalConnector::connection_string(&ExternalConnectorConfig {
            protocol: "http".to_string(),
            ..ExternalConnectorConfig::default()
        })
        .expect_err("persistent consumer groups require TCP");

        assert!(matches!(error, ConnectorError::Config(_)));
    }

    #[test]
    fn remote_connection_rejects_sdk_connection_string_delimiters() {
        let error = ExternalConnector::connection_string(&ExternalConnectorConfig {
            password: "contains@delimiter".to_string(),
            ..ExternalConnectorConfig::default()
        })
        .expect_err("the Iggy SDK parser cannot safely parse this password");

        assert!(matches!(error, ConnectorError::Config(_)));
    }

    #[test]
    fn test_subscriber_message_carries_payload_and_metadata() {
        let metadata = SubscriberMessageMetadata::new("stream1", "topic1", 1);
        let message = SubscriberMessage::new(vec![1, 2, 3], metadata.clone());

        assert_eq!(message.payload, vec![1, 2, 3]);
        assert_eq!(message.metadata, metadata);
    }

    #[test]
    fn test_connector_ack_token_roundtrip_and_scope() {
        let simulated = ConnectorAckToken::simulated("external", "stream1", "topic1", 3, 99);
        let encoded = simulated.encode();
        assert_eq!(encoded, "sim:external:stream1:topic1:3:99");
        assert_eq!(ConnectorAckToken::decode(&encoded).unwrap(), simulated);
        assert!(simulated.matches_scope("stream1", "topic1", 3));
        assert!(!simulated.matches_scope("stream2", "topic1", 3));

        let sdk = ConnectorAckToken::iggy_sdk("stream1", "topic1", 3, 100, "consumer-a");
        let sdk_encoded = sdk.encode();
        assert_eq!(sdk_encoded, "iggy-sdk:stream1:topic1:3:100:consumer-a");
        assert_eq!(ConnectorAckToken::decode(&sdk_encoded).unwrap(), sdk);
        assert!(sdk.matches_scope("stream1", "topic1", 3));
    }

    #[tokio::test]
    async fn test_subscriber_ack_rejects_wrong_scope() {
        let mut subscriber =
            ExternalMessageSubscriber::new("stream1".to_string(), "topic1".to_string(), 1);
        let wrong_scope =
            ConnectorAckToken::simulated("external", "stream2", "topic1", 1, 99).encode();

        assert!(matches!(
            subscriber.ack(&wrong_scope).await,
            Err(ConnectorError::Config(_))
        ));
    }

    #[test]
    fn test_config_defaults() {
        let config = ConnectorConfig::default();

        assert_eq!(config.mode, ConnectorMode::Bundled);
        assert_eq!(config.stream_name, "rustok");
        assert_eq!(config.topic_name, "domain");
        assert_eq!(config.partitions, 8);

        let bundled = BundledConnectorConfig::default();
        assert_eq!(bundled.executable, "iggy-server");
        assert_eq!(bundled.data_dir, "./data/iggy");
        assert_eq!(bundled.tcp_port, 8090);

        let external = ExternalConnectorConfig::default();
        assert_eq!(external.addresses, vec!["127.0.0.1:8090"]);
        assert_eq!(external.protocol, "tcp");
    }
}

#[cfg(test)]
mod contract_tests;
