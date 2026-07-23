//! Iggy-based event transport for RusToK platform.
//!
//! This crate provides a streaming event transport implementation using [Iggy](https://iggy.rs),
//! a high-performance message streaming platform.
//!
//! # Architecture
//!
//! This crate implements `EventTransport` trait and handles:
//! - Event serialization (JSON/MessagePack)
//! - Topology management (streams, topics)
//! - Consumer group coordination
//! - Dead letter queue handling
//!
//! Connection management (bundled vs external mode) is delegated to `rustok-iggy-connector`.
//!
//! # Features
//!
//! - **EventTransport implementation**: Seamless integration with RusToK event system
//! - **Multiple serialization formats**: JSON (default) and MessagePack
//! - **Automatic topology management**: Streams and topics created automatically
//! - **Tenant-based partitioning**: Events from the same tenant maintain order
//! - **Consumer groups and DLQ**: Higher-level streaming primitives
//!
//! # Quick Start
//!
//! ```rust,ignore
//! use rustok_iggy::{IggyConfig, IggyTransport};
//! use rustok_core::events::{EventEnvelope, EventTransport};
//!
//! #[tokio::main]
//! async fn main() -> Result<(), Box<dyn std::error::Error>> {
//!     let config = IggyConfig::default();
//!     let transport = IggyTransport::new(config).await?;
//!     
//!     // Publish events...
//!     transport.shutdown().await?;
//!     Ok(())
//! }
//! ```
//!
//! # Configuration
//!
//! Configuration can be done via code or YAML/JSON settings:
//!
//! ```yaml
//! events:
//!   transport: iggy
//!   iggy:
//!     mode: bundled
//!     serialization: json
//!     topology:
//!       stream_name: rustok
//!       domain_partitions: 8
//!     bundled:
//!       executable: iggy-server
//!       data_dir: ./data/iggy
//!       tcp_port: 8090
//! ```

pub mod config;
pub mod consumer;
pub mod contract_consumer;
pub mod dlq;
pub mod health;
pub mod partitioning;
pub mod producer;
pub mod serialization;
pub mod topology;
pub mod transport;

pub use config::{
    BundledConfig, ExternalConfig, IggyConfig, IggyMode, RetentionConfig, SerializationFormat,
    TopologyConfig,
};
pub use consumer::{ConsumedEvent, PersistentConsumerGroup};
pub use contract_consumer::{ConsumedContractEvent, PersistentContractConsumerGroup};
pub use dlq::{DlqEntry, DlqManager};
pub use health::{HealthCheckResult, HealthStatus, health_check};
pub use partitioning::{calculate_partition, partition_key};
pub use serialization::{EventSerializer, JsonSerializer, MessagePackSerializer};
pub use topology::TopologyManager;
pub use transport::IggyTransport;

/// Dedicated Iggy topic for immutable `module.build.queued` deliveries.
pub const MODULE_BUILD_TOPIC: &str = "module-build";

#[cfg(test)]
mod contract_tests;
