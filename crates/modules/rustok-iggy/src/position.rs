use std::time::{SystemTime, UNIX_EPOCH};

use iggy::prelude::{
    Client, Consumer, ConsumerKind, ConsumerOffsetClient, Identifier, IggyClient, TopicClient,
};
use serde::{Deserialize, Serialize};
use thiserror::Error;

use crate::config::{ExternalConfig, IggyConfig};

/// One broker-backed consumer-group checkpoint paired with the partition high-watermark
/// observed by the same snapshot operation.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ConsumerPartitionPosition {
    pub partition: u32,
    pub acknowledged_offset: Option<u64>,
    pub high_watermark: u64,
    pub messages_count: u64,
}

impl ConsumerPartitionPosition {
    /// Returns an exact offset lag only when the checkpoint and high-watermark are coherent.
    ///
    /// An empty partition has zero lag without requiring a stored consumer offset. A non-empty
    /// partition without a checkpoint is intentionally unknown instead of assuming offset zero.
    pub fn lag(&self) -> Option<u64> {
        if self.messages_count == 0 {
            return Some(0);
        }
        self.acknowledged_offset
            .and_then(|offset| self.high_watermark.checked_sub(offset))
    }
}

/// Partition-qualified broker snapshot for one persistent consumer group.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ConsumerPositionSnapshot {
    pub stream: String,
    pub topic: String,
    pub consumer_group: String,
    pub captured_at_unix_seconds: u64,
    pub partitions: Vec<ConsumerPartitionPosition>,
}

impl ConsumerPositionSnapshot {
    pub fn partition_count(&self) -> usize {
        self.partitions.len()
    }

    /// Whether every topic partition has a coherent lag value.
    pub fn is_complete(&self) -> bool {
        !self.partitions.is_empty()
            && self
                .partitions
                .iter()
                .all(|position| position.lag().is_some())
    }

    /// Exact total lag across all partitions, or `None` when one partition is unknown.
    pub fn total_lag(&self) -> Option<u64> {
        if !self.is_complete() {
            return None;
        }
        self.partitions
            .iter()
            .try_fold(0_u64, |total, position| total.checked_add(position.lag()?))
    }

    /// Exact maximum partition lag, or `None` when the snapshot is incomplete.
    pub fn max_lag(&self) -> Option<u64> {
        if !self.is_complete() {
            return None;
        }
        self.partitions
            .iter()
            .filter_map(ConsumerPartitionPosition::lag)
            .max()
    }
}

#[derive(Debug, Error)]
pub enum ConsumerPositionError {
    #[error("Iggy consumer-position configuration is invalid: {0}")]
    Configuration(String),
    #[error("Iggy consumer-position observer could not connect: {0}")]
    Connection(String),
    #[error("Iggy consumer-position topic is unavailable: {0}")]
    TopicUnavailable(String),
    #[error("Iggy consumer-position snapshot failed: {0}")]
    Snapshot(String),
}

impl ConsumerPositionError {
    pub const fn stable_code(&self) -> &'static str {
        match self {
            Self::Configuration(_) => "iggy.consumer_position.configuration_invalid",
            Self::Connection(_) => "iggy.consumer_position.connection_unavailable",
            Self::TopicUnavailable(_) => "iggy.consumer_position.topic_unavailable",
            Self::Snapshot(_) => "iggy.consumer_position.snapshot_failed",
        }
    }
}

/// Read-only SDK client used to query committed group offsets and current topic partitions.
///
/// This observer never creates or supervises a broker process. In bundled mode it connects to
/// the already-running loopback endpoint from `IggyConfig`; in external mode it uses the reviewed
/// external endpoints. It does not publish, consume, acknowledge, or mutate offsets.
pub struct IggyConsumerPositionObserver {
    client: IggyClient,
    stream: String,
    topic: String,
    consumer_group: String,
    stream_id: Identifier,
    topic_id: Identifier,
    consumer: Consumer,
}

impl IggyConsumerPositionObserver {
    pub async fn connect(
        config: &IggyConfig,
        consumer_group: &str,
        topic: &str,
    ) -> Result<Self, ConsumerPositionError> {
        if consumer_group.trim().is_empty() {
            return Err(ConsumerPositionError::Configuration(
                "consumer group must not be empty".to_string(),
            ));
        }
        if topic.trim().is_empty() {
            return Err(ConsumerPositionError::Configuration(
                "topic must not be empty".to_string(),
            ));
        }

        let stream = config.topology.stream_name.clone();
        let stream_id: Identifier = stream.clone().try_into().map_err(|error| {
            ConsumerPositionError::Configuration(format!("invalid stream identifier: {error}"))
        })?;
        let topic_id: Identifier = topic.to_string().try_into().map_err(|error| {
            ConsumerPositionError::Configuration(format!("invalid topic identifier: {error}"))
        })?;
        let consumer_id: Identifier = consumer_group.to_string().try_into().map_err(|error| {
            ConsumerPositionError::Configuration(format!(
                "invalid consumer-group identifier: {error}"
            ))
        })?;
        let consumer = Consumer {
            kind: ConsumerKind::ConsumerGroup,
            id: consumer_id,
        };

        let mut failures = Vec::new();
        for (address, connection_string) in config
            .external
            .addresses
            .iter()
            .zip(connection_strings(&config.external)?)
        {
            let client = match IggyClient::from_connection_string(&connection_string) {
                Ok(client) => client,
                Err(error) => {
                    failures.push(format!("{address}: {error}"));
                    continue;
                }
            };
            match client.connect().await {
                Ok(()) => {
                    return Ok(Self {
                        client,
                        stream,
                        topic: topic.to_string(),
                        consumer_group: consumer_group.to_string(),
                        stream_id,
                        topic_id,
                        consumer,
                    });
                }
                Err(error) => failures.push(format!("{address}: {error}")),
            }
        }

        Err(ConsumerPositionError::Connection(format!(
            "failed to connect to every configured Iggy address: {}",
            failures.join("; ")
        )))
    }

    /// Reads all topic partitions and the persistent group checkpoint for each partition.
    pub async fn snapshot(&self) -> Result<ConsumerPositionSnapshot, ConsumerPositionError> {
        let topic = self
            .client
            .get_topic(&self.stream_id, &self.topic_id)
            .await
            .map_err(|error| ConsumerPositionError::Snapshot(error.to_string()))?
            .ok_or_else(|| {
                ConsumerPositionError::TopicUnavailable(format!(
                    "{}/{} was not found",
                    self.stream, self.topic
                ))
            })?;

        let mut positions = Vec::with_capacity(topic.partitions.len());
        for partition in topic.partitions {
            let offset = self
                .client
                .get_consumer_offset(
                    &self.consumer,
                    &self.stream_id,
                    &self.topic_id,
                    Some(partition.id),
                )
                .await
                .map_err(|error| ConsumerPositionError::Snapshot(error.to_string()))?;
            let high_watermark = offset
                .as_ref()
                .map(|position| position.current_offset.max(partition.current_offset))
                .unwrap_or(partition.current_offset);
            positions.push(ConsumerPartitionPosition {
                partition: partition.id,
                acknowledged_offset: offset.map(|position| position.stored_offset),
                high_watermark,
                messages_count: partition.messages_count,
            });
        }
        positions.sort_by_key(|position| position.partition);

        Ok(ConsumerPositionSnapshot {
            stream: self.stream.clone(),
            topic: self.topic.clone(),
            consumer_group: self.consumer_group.clone(),
            captured_at_unix_seconds: unix_timestamp_seconds(),
            partitions: positions,
        })
    }
}

impl std::fmt::Debug for IggyConsumerPositionObserver {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("IggyConsumerPositionObserver")
            .field("stream", &self.stream)
            .field("topic", &self.topic)
            .field("consumer_group", &self.consumer_group)
            .finish_non_exhaustive()
    }
}

fn connection_strings(config: &ExternalConfig) -> Result<Vec<String>, ConsumerPositionError> {
    if config.protocol != "tcp" {
        return Err(ConsumerPositionError::Configuration(
            "consumer-position observation requires the tcp protocol".to_string(),
        ));
    }
    if config.addresses.is_empty() {
        return Err(ConsumerPositionError::Configuration(
            "at least one Iggy address is required".to_string(),
        ));
    }
    if config.username.is_empty() != config.password.is_empty() {
        return Err(ConsumerPositionError::Configuration(
            "username and password must either both be set or both be empty".to_string(),
        ));
    }
    validate_component(&config.username, "username", &[':', '@'])?;
    validate_component(&config.password, "password", &[':', '@'])?;
    if let Some(domain) = config.tls_domain.as_deref() {
        validate_component(domain, "tls_domain", &['?', '&', '='])?;
    }
    if let Some(ca_file) = config.tls_ca_file.as_deref() {
        validate_component(ca_file, "tls_ca_file", &['?', '&', '='])?;
    }

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

    config
        .addresses
        .iter()
        .map(|address| {
            if address.is_empty() {
                return Err(ConsumerPositionError::Configuration(
                    "Iggy addresses must not contain an empty value".to_string(),
                ));
            }
            validate_component(address, "address", &['@', '?', '#'])?;
            let mut connection_string = if config.username.is_empty() {
                format!("iggy://{address}")
            } else {
                format!("iggy://{}:{}@{address}", config.username, config.password)
            };
            if !options.is_empty() {
                connection_string.push('?');
                connection_string.push_str(&options.join("&"));
            }
            Ok(connection_string)
        })
        .collect()
}

fn validate_component(
    value: &str,
    field: &str,
    forbidden: &[char],
) -> Result<(), ConsumerPositionError> {
    if let Some(delimiter) = value
        .chars()
        .find(|character| forbidden.contains(character))
    {
        return Err(ConsumerPositionError::Configuration(format!(
            "Iggy {field} contains unsupported connection-string delimiter '{delimiter}'"
        )));
    }
    Ok(())
}

fn unix_timestamp_seconds() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_secs())
        .unwrap_or(0)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn snapshot(partitions: Vec<ConsumerPartitionPosition>) -> ConsumerPositionSnapshot {
        ConsumerPositionSnapshot {
            stream: "rustok".to_string(),
            topic: "domain".to_string(),
            consumer_group: "group".to_string(),
            captured_at_unix_seconds: 1,
            partitions,
        }
    }

    #[test]
    fn complete_snapshot_calculates_exact_total_and_max_lag() {
        let snapshot = snapshot(vec![
            ConsumerPartitionPosition {
                partition: 1,
                acknowledged_offset: Some(7),
                high_watermark: 10,
                messages_count: 11,
            },
            ConsumerPartitionPosition {
                partition: 2,
                acknowledged_offset: Some(4),
                high_watermark: 9,
                messages_count: 10,
            },
            ConsumerPartitionPosition {
                partition: 3,
                acknowledged_offset: None,
                high_watermark: 0,
                messages_count: 0,
            },
        ]);

        assert!(snapshot.is_complete());
        assert_eq!(snapshot.total_lag(), Some(8));
        assert_eq!(snapshot.max_lag(), Some(5));
    }

    #[test]
    fn non_empty_partition_without_checkpoint_is_incomplete() {
        let snapshot = snapshot(vec![ConsumerPartitionPosition {
            partition: 1,
            acknowledged_offset: None,
            high_watermark: 4,
            messages_count: 5,
        }]);

        assert!(!snapshot.is_complete());
        assert_eq!(snapshot.total_lag(), None);
        assert_eq!(snapshot.max_lag(), None);
    }

    #[test]
    fn checkpoint_ahead_of_high_watermark_fails_closed() {
        let position = ConsumerPartitionPosition {
            partition: 1,
            acknowledged_offset: Some(5),
            high_watermark: 4,
            messages_count: 5,
        };
        assert_eq!(position.lag(), None);
    }

    #[test]
    fn connection_strings_preserve_reviewed_tls_options() {
        let config = ExternalConfig {
            addresses: vec!["iggy.internal:8090".to_string()],
            protocol: "tcp".to_string(),
            username: "service".to_string(),
            password: "secret".to_string(),
            tls_enabled: true,
            tls_domain: Some("iggy.internal".to_string()),
            tls_ca_file: Some("/etc/iggy-ca.pem".to_string()),
        };
        assert_eq!(
            connection_strings(&config).expect("valid config"),
            vec![
                "iggy://service:secret@iggy.internal:8090?tls=true&tls_domain=iggy.internal&tls_ca_file=/etc/iggy-ca.pem"
            ]
        );
    }
}
