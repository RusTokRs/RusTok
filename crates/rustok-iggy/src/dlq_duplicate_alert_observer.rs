use iggy::prelude::{Client, IggyClient};
use thiserror::Error;

use crate::config::{ExternalConfig, IggyConfig, IggyMode};
use crate::dlq_duplicate_external_scan::{
    IggyDlqDuplicateScanError, IggyDlqDuplicateScanRequest, IggyDlqDuplicateScanWindowPolicy,
    IggyDlqDuplicateScanner,
};
use crate::dlq_duplicate_inspection::DlqDuplicateSummary;
use crate::dlq_duplicate_moving_window_scan::{
    IggyDlqDuplicateMovingWindowError, IggyDlqDuplicateMovingWindowPolicy,
    IggyDlqDuplicateMovingWindowScanner, IggyDlqDuplicateMovingWindowState,
};
use crate::dlq_duplicate_rolling_window::DlqDuplicateRollingWindowPolicy;

/// Active bounded physical DLQ scan policy for the connected observer.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum IggyDlqDuplicateAlertScanMode {
    GlobalBudget,
    FairWindow,
    MovingWindow,
}

/// Reviewed fail-closed configuration for an opt-in moving observer.
///
/// The initial offset and private per-partition cursors remain encapsulated. The
/// public debug projection includes only bounded counts and retention limits.
#[derive(Clone, PartialEq, Eq)]
pub struct IggyDlqDuplicateAlertMovingWindowConfig {
    policy: IggyDlqDuplicateMovingWindowPolicy,
}

impl IggyDlqDuplicateAlertMovingWindowConfig {
    pub fn new(
        config: &IggyConfig,
        initial_offset: u64,
        per_partition_messages: u32,
        batch_size: u32,
        rolling_max_cycles: u32,
        rolling_max_observations_per_cycle: u32,
    ) -> Result<Self, IggyDlqDuplicateAlertObserverError> {
        let partitions = configured_partitions(config)?;
        let rolling_policy = DlqDuplicateRollingWindowPolicy::new(
            rolling_max_cycles,
            rolling_max_observations_per_cycle,
        )
        .map_err(|_| IggyDlqDuplicateAlertObserverError::InvalidConfiguration)?;
        let policy = IggyDlqDuplicateMovingWindowPolicy::new(
            partitions,
            initial_offset,
            per_partition_messages,
            batch_size,
            rolling_policy,
        )
        .map_err(|_| IggyDlqDuplicateAlertObserverError::InvalidConfiguration)?;
        Ok(Self { policy })
    }

    pub fn partition_count(&self) -> usize {
        self.policy.partitions().len()
    }

    pub const fn total_message_budget(&self) -> u32 {
        self.policy.total_message_budget()
    }

    pub const fn rolling_max_cycles(&self) -> u32 {
        self.policy.rolling_policy().max_cycles()
    }

    pub const fn rolling_max_observations_per_cycle(&self) -> u32 {
        self.policy.rolling_policy().max_observations_per_cycle()
    }

    pub const fn progress_persisted(&self) -> bool {
        false
    }

    pub const fn restart_resets_to_initial_offset(&self) -> bool {
        true
    }
}

impl std::fmt::Debug for IggyDlqDuplicateAlertMovingWindowConfig {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("IggyDlqDuplicateAlertMovingWindowConfig")
            .field("partition_count", &self.partition_count())
            .field("total_message_budget", &self.total_message_budget())
            .field("rolling_max_cycles", &self.rolling_max_cycles())
            .field(
                "rolling_max_observations_per_cycle",
                &self.rolling_max_observations_per_cycle(),
            )
            .field("progress_persisted", &false)
            .field("restart_resets_to_initial_offset", &true)
            .finish_non_exhaustive()
    }
}

enum IggyDlqDuplicateAlertScan {
    Global(IggyDlqDuplicateScanRequest),
    FairWindow(IggyDlqDuplicateScanWindowPolicy),
    MovingWindow(IggyDlqDuplicateMovingWindowState),
}

impl IggyDlqDuplicateAlertScan {
    const fn mode(&self) -> IggyDlqDuplicateAlertScanMode {
        match self {
            Self::Global(_) => IggyDlqDuplicateAlertScanMode::GlobalBudget,
            Self::FairWindow(_) => IggyDlqDuplicateAlertScanMode::FairWindow,
            Self::MovingWindow(_) => IggyDlqDuplicateAlertScanMode::MovingWindow,
        }
    }

    const fn preserves_process_local_state_after_scan_error(&self) -> bool {
        matches!(self, Self::MovingWindow(_))
    }
}

/// Connected read-only source for bounded physical DLQ duplicate summaries.
///
/// This adapter connects to an already-running Iggy deployment. It never starts
/// or stops a bundled broker, mutates stored consumer offsets, publishes,
/// acknowledges, deletes, purges, replays, or changes broker configuration.
pub struct IggyDlqDuplicateAlertObserver {
    client: IggyClient,
    stream_name: String,
    scan: IggyDlqDuplicateAlertScan,
}

impl IggyDlqDuplicateAlertObserver {
    /// Connect with the compatibility global-budget scan.
    pub async fn connect(
        config: &IggyConfig,
        start_offset: u64,
        max_messages: u32,
        batch_size: u32,
    ) -> Result<Self, IggyDlqDuplicateAlertObserverError> {
        let partitions = configured_partitions(config)?;
        let request =
            IggyDlqDuplicateScanRequest::new(partitions, start_offset, max_messages, batch_size)
                .map_err(|_| IggyDlqDuplicateAlertObserverError::InvalidConfiguration)?;
        Self::connect_with_scan(config, IggyDlqDuplicateAlertScan::Global(request)).await
    }

    /// Connect with one equal per-partition fixed snapshot budget.
    pub async fn connect_fair_window(
        config: &IggyConfig,
        start_offset: u64,
        per_partition_messages: u32,
        batch_size: u32,
    ) -> Result<Self, IggyDlqDuplicateAlertObserverError> {
        let partitions = configured_partitions(config)?;
        let policy = IggyDlqDuplicateScanWindowPolicy::new(
            partitions,
            start_offset,
            per_partition_messages,
            batch_size,
        )
        .map_err(|_| IggyDlqDuplicateAlertObserverError::InvalidConfiguration)?;
        Self::connect_with_scan(config, IggyDlqDuplicateAlertScan::FairWindow(policy)).await
    }

    /// Connect with an explicit process-local moving window.
    pub async fn connect_moving_window(
        config: &IggyConfig,
        moving: IggyDlqDuplicateAlertMovingWindowConfig,
    ) -> Result<Self, IggyDlqDuplicateAlertObserverError> {
        let partitions = configured_partitions(config)?;
        if moving.policy.partitions() != partitions.as_slice() {
            return Err(IggyDlqDuplicateAlertObserverError::InvalidConfiguration);
        }
        let state = IggyDlqDuplicateMovingWindowState::new(moving.policy);
        Self::connect_with_scan(config, IggyDlqDuplicateAlertScan::MovingWindow(state)).await
    }

    async fn connect_with_scan(
        config: &IggyConfig,
        scan: IggyDlqDuplicateAlertScan,
    ) -> Result<Self, IggyDlqDuplicateAlertObserverError> {
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
        match &scan {
            IggyDlqDuplicateAlertScan::Global(_) | IggyDlqDuplicateAlertScan::FairWindow(_) => {
                IggyDlqDuplicateScanner::new(&client, &stream_name)
                    .map_err(|_| IggyDlqDuplicateAlertObserverError::InvalidConfiguration)?;
            }
            IggyDlqDuplicateAlertScan::MovingWindow(_) => {
                IggyDlqDuplicateMovingWindowScanner::new(&client, &stream_name)
                    .map_err(|_| IggyDlqDuplicateAlertObserverError::InvalidConfiguration)?;
            }
        }

        Ok(Self {
            client,
            stream_name,
            scan,
        })
    }

    pub const fn scan_mode(&self) -> IggyDlqDuplicateAlertScanMode {
        self.scan.mode()
    }

    /// Moving mode preserves its private cursors and rolling history after a
    /// failed complete-cycle attempt. Fixed modes remain reconnectable snapshots.
    pub const fn preserves_process_local_state_after_scan_error(&self) -> bool {
        self.scan.preserves_process_local_state_after_scan_error()
    }

    pub async fn summarize(
        &mut self,
    ) -> Result<DlqDuplicateSummary, IggyDlqDuplicateAlertObserverError> {
        let client = &self.client;
        let stream_name = &self.stream_name;
        match &mut self.scan {
            IggyDlqDuplicateAlertScan::Global(request) => {
                let scanner = IggyDlqDuplicateScanner::new(client, stream_name)?;
                scanner.summarize(request).await.map_err(Into::into)
            }
            IggyDlqDuplicateAlertScan::FairWindow(policy) => {
                let scanner = IggyDlqDuplicateScanner::new(client, stream_name)?;
                scanner.summarize_window(policy).await.map_err(Into::into)
            }
            IggyDlqDuplicateAlertScan::MovingWindow(state) => {
                let scanner = IggyDlqDuplicateMovingWindowScanner::new(client, stream_name)?;
                let snapshot = scanner.scan_cycle(state).await?;
                Ok(*snapshot.rolling().summary())
            }
        }
    }
}

impl std::fmt::Debug for IggyDlqDuplicateAlertObserver {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let mut debug = formatter.debug_struct("IggyDlqDuplicateAlertObserver");
        debug.field("scan_mode", &self.scan.mode());
        match &self.scan {
            IggyDlqDuplicateAlertScan::Global(request) => {
                debug
                    .field("partition_count", &request.partitions().len())
                    .field("max_messages", &request.max_messages())
                    .field("batch_size", &request.batch_size());
            }
            IggyDlqDuplicateAlertScan::FairWindow(policy) => {
                debug
                    .field("partition_count", &policy.partitions().len())
                    .field("per_partition_messages", &policy.per_partition_messages())
                    .field("total_message_budget", &policy.total_message_budget())
                    .field("batch_size", &policy.batch_size());
            }
            IggyDlqDuplicateAlertScan::MovingWindow(state) => {
                let policy = state.policy();
                debug
                    .field("partition_count", &policy.partitions().len())
                    .field("total_message_budget", &policy.total_message_budget())
                    .field("rolling_max_cycles", &policy.rolling_policy().max_cycles())
                    .field(
                        "rolling_max_observations_per_cycle",
                        &policy.rolling_policy().max_observations_per_cycle(),
                    )
                    .field("progress_persisted", &false)
                    .field("restart_resets_to_initial_offset", &true);
            }
        }
        debug.finish_non_exhaustive()
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
    #[error(transparent)]
    MovingScan(#[from] IggyDlqDuplicateMovingWindowError),
}

impl IggyDlqDuplicateAlertObserverError {
    pub const fn stable_code(self) -> &'static str {
        match self {
            Self::InvalidConfiguration => "iggy.dlq_duplicate.alert_observer_configuration_invalid",
            Self::ConnectionUnavailable => {
                "iggy.dlq_duplicate.alert_observer_connection_unavailable"
            }
            Self::Scan(error) => error.stable_code(),
            Self::MovingScan(error) => error.stable_code(),
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
            || !is_bundled_loopback_address(&external.addresses[0], config.bundled.tcp_port)
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
    if let Some(domain) = config
        .tls_domain
        .as_deref()
        .filter(|value| !value.is_empty())
    {
        options.push(format!("tls_domain={domain}"));
    }
    if let Some(ca_file) = config
        .tls_ca_file
        .as_deref()
        .filter(|value| !value.is_empty())
    {
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
    if value
        .chars()
        .any(|character| forbidden.contains(&character))
    {
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
    fn all_configured_partitions_are_included_in_request() {
        let mut config = IggyConfig::default();
        config.topology.domain_partitions = 3;
        assert_eq!(configured_partitions(&config).unwrap(), vec![1, 2, 3]);
    }

    #[test]
    fn global_fair_and_moving_scan_modes_remain_explicit() {
        let global = IggyDlqDuplicateAlertScan::Global(
            IggyDlqDuplicateScanRequest::new(vec![1, 2], 0, 100, 25).unwrap(),
        );
        let fair = IggyDlqDuplicateAlertScan::FairWindow(
            IggyDlqDuplicateScanWindowPolicy::new(vec![1, 2], 0, 50, 25).unwrap(),
        );
        let mut config = IggyConfig::default();
        config.topology.domain_partitions = 2;
        let moving = IggyDlqDuplicateAlertMovingWindowConfig::new(&config, 0, 2, 1, 3, 4).unwrap();
        let moving = IggyDlqDuplicateAlertScan::MovingWindow(
            IggyDlqDuplicateMovingWindowState::new(moving.policy),
        );

        assert_eq!(global.mode(), IggyDlqDuplicateAlertScanMode::GlobalBudget);
        assert_eq!(fair.mode(), IggyDlqDuplicateAlertScanMode::FairWindow);
        assert_eq!(moving.mode(), IggyDlqDuplicateAlertScanMode::MovingWindow);
        assert!(!global.preserves_process_local_state_after_scan_error());
        assert!(moving.preserves_process_local_state_after_scan_error());
    }

    #[test]
    fn moving_window_requires_complete_cycle_capacity() {
        let mut config = IggyConfig::default();
        config.topology.domain_partitions = 2;
        assert_eq!(
            IggyDlqDuplicateAlertMovingWindowConfig::new(&config, 0, 2, 1, 3, 3).unwrap_err(),
            IggyDlqDuplicateAlertObserverError::InvalidConfiguration
        );
        let valid = IggyDlqDuplicateAlertMovingWindowConfig::new(&config, 0, 2, 1, 3, 4).unwrap();
        assert_eq!(valid.partition_count(), 2);
        assert_eq!(valid.total_message_budget(), 4);
        assert!(!valid.progress_persisted());
        assert!(valid.restart_resets_to_initial_offset());
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
