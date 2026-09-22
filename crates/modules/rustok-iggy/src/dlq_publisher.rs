use iggy::prelude::{Client, IggyClient, IggyError, IggyMessage, Partitioning};
use thiserror::Error;
use uuid::Uuid;

use crate::config::{ExternalConfig, IggyConfig};
use crate::dlq::DlqEntry;

#[derive(Debug, Error)]
pub(crate) enum DlqPublisherError {
    #[error("Iggy DLQ publisher configuration is invalid: {0}")]
    Configuration(String),
    #[error("Iggy DLQ publisher could not connect: {0}")]
    Connection(String),
    #[error("Iggy DLQ publication failed: {0}")]
    Publish(String),
}

impl DlqPublisherError {
    pub(crate) const fn stable_code(&self) -> &'static str {
        match self {
            Self::Configuration(_) => "iggy.dlq_publisher.configuration_invalid",
            Self::Connection(_) => "iggy.dlq_publisher.connection_unavailable",
            Self::Publish(_) => "iggy.dlq_publisher.publish_failed",
        }
    }
}

/// Lazily connected SDK publisher for DLQ entries that carry an explicit broker message ID.
///
/// It is owned by `IggyTransport`, connects to the same configured broker process, and does
/// not create or supervise another transport. The dedicated connection exists because the
/// current generic connector publish API does not expose Iggy's `u128` message header ID.
pub(crate) struct IggyDlqPublisher {
    client: IggyClient,
    stream: String,
    partitions: u32,
    replication_factor: u8,
}

impl IggyDlqPublisher {
    pub(crate) async fn connect(config: &IggyConfig) -> Result<Self, DlqPublisherError> {
        if config.topology.domain_partitions == 0 {
            return Err(DlqPublisherError::Configuration(
                "topic partition count must be positive".to_string(),
            ));
        }
        if config.topology.replication_factor == 0 {
            return Err(DlqPublisherError::Configuration(
                "topic replication factor must be positive".to_string(),
            ));
        }

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
                        stream: config.topology.stream_name.clone(),
                        partitions: config.topology.domain_partitions,
                        replication_factor: config.topology.replication_factor,
                    });
                }
                Err(error) => failures.push(format!("{address}: {error}")),
            }
        }

        Err(DlqPublisherError::Connection(format!(
            "failed to connect to every configured Iggy address: {}",
            failures.join("; ")
        )))
    }

    pub(crate) async fn publish(&self, entry: &DlqEntry) -> Result<(), DlqPublisherError> {
        let message_id = entry.broker_message_id().ok_or_else(|| {
            DlqPublisherError::Configuration(
                "deterministic DLQ broker message ID is required".to_string(),
            )
        })?;
        if message_id.is_nil() {
            return Err(DlqPublisherError::Configuration(
                "deterministic DLQ broker message ID must not be nil".to_string(),
            ));
        }
        if entry.payload.is_empty() {
            return Err(DlqPublisherError::Configuration(
                "DLQ payload must not be empty".to_string(),
            ));
        }

        let partition = partition_for_message_id(message_id, self.partitions);
        let producer = self
            .client
            .producer(&self.stream, "dlq")
            .map_err(publish_error)?
            .partitioning(Partitioning::partition_id(partition))
            .create_stream_if_not_exists()
            .create_topic_if_not_exists(
                self.partitions,
                Some(self.replication_factor),
                Default::default(),
                Default::default(),
            )
            .build();
        producer.init().await.map_err(publish_error)?;

        let message = IggyMessage::builder()
            .id(message_id.as_u128())
            .payload(entry.payload.clone().into())
            .build()
            .map_err(publish_error)?;
        producer.send(vec![message]).await.map_err(publish_error)?;

        tracing::warn!(
            event_id = %entry.event_id,
            broker_message_id = %message_id,
            original_topic = %entry.original_topic,
            error = %entry.error,
            retry_count = entry.retry_count,
            source_offset = ?entry.source_offset(),
            dlq_stream = %self.stream,
            dlq_topic = "dlq",
            partition,
            "Published event to the dead letter queue with a deterministic Iggy message ID"
        );
        Ok(())
    }
}

fn publish_error(error: IggyError) -> DlqPublisherError {
    DlqPublisherError::Publish(error.to_string())
}

fn partition_for_message_id(message_id: Uuid, partitions: u32) -> u32 {
    debug_assert!(partitions > 0);
    (message_id.as_u128() % u128::from(partitions)) as u32 + 1
}

fn connection_strings(config: &ExternalConfig) -> Result<Vec<String>, DlqPublisherError> {
    if config.protocol != "tcp" {
        return Err(DlqPublisherError::Configuration(
            "deterministic DLQ publication requires the tcp protocol".to_string(),
        ));
    }
    if config.addresses.is_empty() {
        return Err(DlqPublisherError::Configuration(
            "at least one Iggy address is required".to_string(),
        ));
    }
    if config.username.is_empty() != config.password.is_empty() {
        return Err(DlqPublisherError::Configuration(
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
                return Err(DlqPublisherError::Configuration(
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
) -> Result<(), DlqPublisherError> {
    if let Some(delimiter) = value
        .chars()
        .find(|character| forbidden.contains(character))
    {
        return Err(DlqPublisherError::Configuration(format!(
            "Iggy {field} contains unsupported connection-string delimiter '{delimiter}'"
        )));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn deterministic_partition_is_stable_and_one_based() {
        let message_id = Uuid::from_u128(42);
        let first = partition_for_message_id(message_id, 8);
        let second = partition_for_message_id(message_id, 8);
        assert_eq!(first, second);
        assert!((1..=8).contains(&first));
    }

    #[test]
    fn deterministic_partition_changes_only_with_id_or_partition_count() {
        let first = partition_for_message_id(Uuid::from_u128(42), 8);
        let different_id = partition_for_message_id(Uuid::from_u128(43), 8);
        let different_count = partition_for_message_id(Uuid::from_u128(42), 7);
        assert_ne!(first, different_id);
        assert_ne!(first, different_count);
    }

    #[test]
    fn connection_strings_preserve_tls_options() {
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
            connection_strings(&config).unwrap(),
            vec![
                "iggy://service:secret@iggy.internal:8090?tls=true&tls_domain=iggy.internal&tls_ca_file=/etc/iggy-ca.pem"
            ]
        );
    }
}
