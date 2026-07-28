use iggy::prelude::{Client, IggyClient};
use thiserror::Error;

use crate::config::{ExternalConfig, IggyConfig, IggyMode};
use crate::dlq_duplicate_external_scan::{
    IggyDlqDuplicateScanError, IggyDlqDuplicateScanRequest, IggyDlqDuplicateScanner,
};
use crate::dlq_duplicate_inspection::DlqDuplicateSummary;

/// Connected read-only source for bounded physical DLQ duplicate summaries.
///
/// This adapter connects to an already-running Iggy deployment. It never starts
/// or stops a bundled broker, mutates offsets, publishes, acknowledges, deletes,
/// purges, replays, or changes broker configuration.
pub struct IggyDlqDuplicateAlertObserver {
    client: IggyClient,
    stream_name: String,
    request: IggyDlqDuplicateScanRequest,
}

impl IggyDlqDuplicateAlertObserver {
    pub async fn connect(
        config: &IggyConfig,
        start_offset: u64,
        max_messages: u32,
        batch_size: u32,
    ) -> Result<Self, IggyDlqDuplicateAlertObserverError> {
        let partitions = configured_partitions(config)?;
        let request = IggyDlqDuplicateScanRequest::new(
            partitions,
            start_offset,
            max_messages,
            batch_size,
        )
        .map_err(|_| IggyDlqDuplicateAlertObserverError::InvalidConfiguration)?;
        let external = read_only_connection_config(config)?;
        let connection_strings = connection_strings(&external)?;
        let mut connected = None;

        for connection_string in connection_strings {
            let client = IggyClient::from_connection_string(&connection_string)
                .map_err(|_| IggyDlqDuplicateAlertObserverError::InvalidConfiguration)?;
            if client.connect().await.is_ok() {
                connected = Some(client);
                break;
            }
        }

        let client = connected.ok_or(IggyDlqDuplicateAlertObserverError::ConnectionUnavailable)?;
        let stream_name = config.topology.stream_name.clone();
        IggyDlqDuplicateScanner::new(&client, &stream_name)
            .map_err(|_| IggyDlqDuplicateAlertObserverError::InvalidConfiguration)?;

        Ok(Self {
            client,
            stream_name,
            request,
        })
    }

    pub async fn summarize(
        &self,
    ) -> Result<DlqDuplicateSummary, IggyDlqDuplicateAlertObserverError> {
        IggyDlqDuplicateScanner::new(&self.client, &self.stream_name)?
            .summarize(&self.request)
            .await
            .map_err(Into::into)
    }
}

impl std::fmt::Debug for IggyDlqDuplicateAlertObserver {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("IggyDlqDuplicateAlertObserver")
            .field("partition_count", &self.request.partitions().len())
            .field("max_messages", &self.request.max_messages())
            .field("batch_size", &self.request.batch_size())
            .finish_non_exhaustive()
    }
}

#[derive(Debug, Error, Clone, Copy, PartialEq, Eq)]
pub enum IggyDlqDuplicateAlertObserverError {
    #[error("physical DLQ duplicate observer configuration is invalid")]
    InvalidConfiguration,
    #[error("physical DLQ duplicate observer connection is unavailable")]
    ConnectionUnavailable,
    #[error(transparent)]
    Scan(#[from] IggyDlqDuplicateScanError),
}

impl IggyDlqDuplicateAlertObserverError {
    pub const fn stable_code(self) -> &'static str {
        match self {
            Self::InvalidConfiguration => {
                "iggy.dlq_duplicate.alert_observer_configuration_invalid"
            }
            Self::ConnectionUnavailable => {
                "iggy.dlq_duplicate.alert_observer_connection_unavailable"
            }
            Self::Scan(error) => error.stable_code(),
        }
    }
}

fn configured_partitions(
    config: &IggyConfig,
) -> Result<Vec<u32>, IggyDlqDuplicateAlertObserverError> {
    if config.topology.domain_partitions == 0 || config.topology.domain_partitions > 128 {
        return Err(IggyDlqDuplicateAlertObserverError::InvalidConfiguration);
    }
    Ok((1..=config.topology.domain_partitions).collect())
}

fn read_only_connection_config(
    config: &IggyConfig,
) -> Result<ExternalConfig, IggyDlqDuplicateAlertObserverError> {
    let external = config.external.clone();
    if config.mode == IggyMode::Bundled {
        if external.protocol != "tcp"
            || external.tls_enabled
            || external.addresses.len() != 1
            || !is_bundled_loopback_address(
                &external.addresses[0],
                config.bundled.tcp_port,
            )
        {
            return Err(IggyDlqDuplicateAlertObserverError::InvalidConfiguration);
        }
    }
    Ok(external)
}

fn is_bundled_loopback_address(address: &str, expected_port: u16) -> bool {
    let Some((host, port)) = address.rsplit_once(':') else {
        return false;
    };
    let host = host.trim_matches(['[', ']']);
    matches!(host, "127.0.0.1" | "localhost" | "::1")
        && port.parse::<u16>().ok() == Some(expected_port)
}

fn connection_strings(
    config: &ExternalConfig,
) -> Result<Vec<String>, IggyDlqDuplicateAlertObserverError> {
    if config.protocol != "tcp"
        || config.addresses.is_empty()
        || config.username.is_empty() != config.password.is_empty()
    {
        return Err(IggyDlqDuplicateAlertObserverError::InvalidConfiguration);
    }
    validate_component(&config.username, &[':', '@'])?;
    validate_component(&config.password, &[':', '@'])?;
    if let Some(domain) = config.tls_domain.as_deref() {
        validate_component(domain, &['?', '&', '='])?;
    }
    if let Some(ca_file) = config.tls_ca_file.as_deref() {
        validate_component(ca_file, &['?', '&', '='])?;
    }

    let mut options = Vec::new();
    if config.tls_enabled {
        options.push("tls=true".to_string());
    }
    if let Some(domain) = config.tls_domain.as_deref().filter(|value| !value.is_empty()) {
        options.push(format!("tls_domain={domain}"));
    }
    if let Some(ca_file) = config.tls_ca_file.as_deref().filter(|value| !value.is_empty()) {
        options.push(format!("tls_ca_file={ca_file}"));
    }

    config
        .addresses
        .iter()
        .map(|address| {
            if address.is_empty() {
                return Err(IggyDlqDuplicateAlertObserverError::InvalidConfiguration);
            }
            validate_component(address, &['@', '?', '#'])?;
            let mut value = if config.username.is_empty() {
                format!("iggy://{address}")
            } else {
                format!("iggy://{}:{}@{address}", config.username, config.password)
            };
            if !options.is_empty() {
                value.push('?');
                value.push_str(&options.join("&"));
            }
            Ok(value)
        })
        .collect()
}

fn validate_component(
    value: &str,
    forbidden: &[char],
) -> Result<(), IggyDlqDuplicateAlertObserverError> {
    if value.chars().any(|character| forbidden.contains(&character)) {
        return Err(IggyDlqDuplicateAlertObserverError::InvalidConfiguration);
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bundled_mode_requires_matching_loopback_address() {
        let mut config = IggyConfig::default();
        config.mode = IggyMode::Bundled;
        config.bundled.tcp_port = 8091;
        config.external.addresses = vec!["127.0.0.1:8091".to_string()];
        assert!(read_only_connection_config(&config).is_ok());

        config.external.addresses = vec!["127.0.0.1:8090".to_string()];
        assert_eq!(
            read_only_connection_config(&config).unwrap_err(),
            IggyDlqDuplicateAlertObserverError::InvalidConfiguration
        );
    }

    #[test]
    fn all_configured_partitions_are_scanned_once() {
        let mut config = IggyConfig::default();
        config.topology.domain_partitions = 3;
        assert_eq!(configured_partitions(&config).unwrap(), vec![1, 2, 3]);
    }

    #[test]
    fn invalid_partition_count_fails_closed() {
        for partitions in [0, 129] {
            let mut config = IggyConfig::default();
            config.topology.domain_partitions = partitions;
            assert_eq!(
                configured_partitions(&config).unwrap_err(),
                IggyDlqDuplicateAlertObserverError::InvalidConfiguration
            );
        }
    }

    #[test]
    fn stable_errors_expose_no_connection_details() {
        assert_eq!(
            IggyDlqDuplicateAlertObserverError::InvalidConfiguration.stable_code(),
            "iggy.dlq_duplicate.alert_observer_configuration_invalid"
        );
        assert_eq!(
            IggyDlqDuplicateAlertObserverError::ConnectionUnavailable.stable_code(),
            "iggy.dlq_duplicate.alert_observer_connection_unavailable"
        );
    }
}
