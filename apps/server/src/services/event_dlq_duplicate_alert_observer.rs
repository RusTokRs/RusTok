use std::env;
use std::sync::Arc;
use std::time::Duration;

use rustok_iggy::{
    DlqDuplicateAlertPolicy, DlqDuplicateAlertRuntimePublisher, DlqDuplicateAlertRuntimeSnapshot,
    DlqDuplicateAlertRuntimeSubscriber, IggyDlqDuplicateAlertMovingWindowConfig,
    IggyDlqDuplicateAlertObserver, IggyMode, IggyTransport,
};
use tokio::task::JoinHandle;

use crate::common::settings::EventDeliveryProfile;
use crate::error::{Error, Result};
use crate::services::app_lifecycle::StopHandle;
use crate::services::event_transport_factory::EventRuntime;
use crate::services::server_runtime_context::ServerRuntimeContext;

const ENABLE_ENV: &str = "RUSTOK_EVENT_DLQ_DUPLICATE_ALERT_ENABLED";
const POLL_ENV: &str = "RUSTOK_EVENT_DLQ_DUPLICATE_ALERT_POLL_MS";
const SCAN_MODE_ENV: &str = "RUSTOK_EVENT_DLQ_DUPLICATE_ALERT_SCAN_MODE";
const START_OFFSET_ENV: &str = "RUSTOK_EVENT_DLQ_DUPLICATE_ALERT_START_OFFSET";
const MAX_MESSAGES_ENV: &str = "RUSTOK_EVENT_DLQ_DUPLICATE_ALERT_MAX_MESSAGES";
const PER_PARTITION_MESSAGES_ENV: &str = "RUSTOK_EVENT_DLQ_DUPLICATE_ALERT_PER_PARTITION_MESSAGES";
const BATCH_SIZE_ENV: &str = "RUSTOK_EVENT_DLQ_DUPLICATE_ALERT_BATCH_SIZE";
const ROLLING_MAX_CYCLES_ENV: &str = "RUSTOK_EVENT_DLQ_DUPLICATE_ALERT_ROLLING_MAX_CYCLES";
const ROLLING_MAX_OBSERVATIONS_PER_CYCLE_ENV: &str =
    "RUSTOK_EVENT_DLQ_DUPLICATE_ALERT_ROLLING_MAX_OBSERVATIONS_PER_CYCLE";
const WARNING_MESSAGES_ENV: &str = "RUSTOK_EVENT_DLQ_DUPLICATE_ALERT_WARNING_MESSAGES";
const CRITICAL_MESSAGES_ENV: &str = "RUSTOK_EVENT_DLQ_DUPLICATE_ALERT_CRITICAL_MESSAGES";
const WARNING_GROUPS_ENV: &str = "RUSTOK_EVENT_DLQ_DUPLICATE_ALERT_WARNING_GROUPS";
const CRITICAL_GROUPS_ENV: &str = "RUSTOK_EVENT_DLQ_DUPLICATE_ALERT_CRITICAL_GROUPS";
const WARNING_MAX_COPIES_ENV: &str = "RUSTOK_EVENT_DLQ_DUPLICATE_ALERT_WARNING_MAX_COPIES";
const CRITICAL_MAX_COPIES_ENV: &str = "RUSTOK_EVENT_DLQ_DUPLICATE_ALERT_CRITICAL_MAX_COPIES";
const DEFAULT_POLL_MS: u64 = 30_000;
const MAX_POLL_MS: u64 = 300_000;
const DEFAULT_MAX_MESSAGES: u32 = 1_000;
const DEFAULT_BATCH_SIZE: u32 = 100;
const STARTUP_CONFIGURATION_INVALID: &str =
    "iggy.dlq_duplicate.alert_server_observer_configuration_invalid";
const STARTUP_RUNTIME_UNAVAILABLE: &str =
    "iggy.dlq_duplicate.alert_server_observer_runtime_unavailable";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EventDlqDuplicateAlertObserverMode {
    Disabled,
    Unavailable,
    NotApplicableOutboxLocal,
    IggyBundled,
    IggyExternal,
}

#[derive(Debug, Clone)]
enum EventDlqDuplicateAlertScanConfig {
    GlobalBudget {
        start_offset: u64,
        max_messages: u32,
        batch_size: u32,
    },
    FairWindow {
        start_offset: u64,
        per_partition_messages: u32,
        batch_size: u32,
    },
    MovingWindow {
        moving: IggyDlqDuplicateAlertMovingWindowConfig,
    },
}

pub struct EventDlqDuplicateAlertObserverHandle {
    mode: EventDlqDuplicateAlertObserverMode,
    subscriber: Option<DlqDuplicateAlertRuntimeSubscriber>,
    handle: Option<JoinHandle<()>>,
}

impl EventDlqDuplicateAlertObserverHandle {
    pub const fn mode(&self) -> EventDlqDuplicateAlertObserverMode {
        self.mode
    }

    pub fn current_snapshot(&self) -> Option<DlqDuplicateAlertRuntimeSnapshot> {
        self.subscriber
            .as_ref()
            .map(|subscriber| subscriber.current())
    }

    pub fn is_finished(&self) -> bool {
        self.handle.as_ref().is_some_and(JoinHandle::is_finished)
    }
}

struct EventDlqDuplicateAlertObserverConfig {
    poll: Duration,
    scan: EventDlqDuplicateAlertScanConfig,
    policy: DlqDuplicateAlertPolicy,
}

pub async fn start_event_dlq_duplicate_alert_observer(ctx: &ServerRuntimeContext) {
    if !ctx.settings().runtime.runs_background_workers()
        || ctx.shared_contains::<EventDlqDuplicateAlertObserverHandle>()
    {
        return;
    }

    let enabled = match optional_bool_env(ENABLE_ENV, false) {
        Ok(enabled) => enabled,
        Err(_) => {
            record_startup_unavailable(ctx, STARTUP_CONFIGURATION_INVALID);
            return;
        }
    };
    if !enabled {
        ctx.shared_insert(EventDlqDuplicateAlertObserverHandle {
            mode: EventDlqDuplicateAlertObserverMode::Disabled,
            subscriber: None,
            handle: None,
        });
        return;
    }

    let Some(runtime) = ctx.shared_get::<Arc<EventRuntime>>() else {
        record_startup_unavailable(ctx, STARTUP_RUNTIME_UNAVAILABLE);
        return;
    };
    let mode = match observer_mode(runtime.delivery_profile, runtime.iggy_mode.as_ref()) {
        Ok(mode) => mode,
        Err(_) => {
            record_startup_unavailable(ctx, STARTUP_CONFIGURATION_INVALID);
            return;
        }
    };
    if matches!(
        mode,
        EventDlqDuplicateAlertObserverMode::NotApplicableOutboxLocal
    ) {
        tracing::info!(
            delivery_profile = runtime.delivery_profile.as_str(),
            "Physical DLQ duplicate alert observer is not applicable to the active event delivery profile"
        );
        ctx.shared_insert(EventDlqDuplicateAlertObserverHandle {
            mode,
            subscriber: None,
            handle: None,
        });
        return;
    }

    let Some(transport) = ctx.shared_get::<Arc<IggyTransport>>() else {
        record_startup_unavailable(ctx, STARTUP_RUNTIME_UNAVAILABLE);
        return;
    };
    let iggy_config = transport.config().clone();
    let config = match EventDlqDuplicateAlertObserverConfig::from_env(&iggy_config) {
        Ok(config) => config,
        Err(_) => {
            record_startup_unavailable(ctx, STARTUP_CONFIGURATION_INVALID);
            return;
        }
    };

    if !ctx.shared_contains::<StopHandle>() {
        let (stop_handle, _stop_rx) = StopHandle::new();
        ctx.shared_insert(stop_handle);
    }
    let stop_rx = ctx
        .shared_get::<StopHandle>()
        .expect("StopHandle must exist before DLQ duplicate observer startup")
        .subscribe();

    let (publisher, subscriber) = DlqDuplicateAlertRuntimePublisher::new(config.policy);
    let handle = tokio::spawn(observer_loop(iggy_config, config, publisher, stop_rx));
    ctx.shared_insert(EventDlqDuplicateAlertObserverHandle {
        mode,
        subscriber: Some(subscriber),
        handle: Some(handle),
    });
    tracing::info!(
        mode = ?mode,
        "Starting mode-aware physical DLQ duplicate alert observer"
    );
}

fn record_startup_unavailable(ctx: &ServerRuntimeContext, error_code: &'static str) {
    ctx.shared_insert(EventDlqDuplicateAlertObserverHandle {
        mode: EventDlqDuplicateAlertObserverMode::Unavailable,
        subscriber: None,
        handle: None,
    });
    tracing::warn!(
        error_code,
        "Physical DLQ duplicate alert observer startup is unavailable; event delivery remains active"
    );
}

async fn observer_loop(
    iggy_config: rustok_iggy::IggyConfig,
    config: EventDlqDuplicateAlertObserverConfig,
    mut publisher: DlqDuplicateAlertRuntimePublisher,
    mut stop_rx: tokio::sync::watch::Receiver<bool>,
) {
    let mut observer = None;
    loop {
        if *stop_rx.borrow() {
            let _ = publisher.mark_unavailable();
            return;
        }

        if observer.is_none() {
            let connection = match &config.scan {
                EventDlqDuplicateAlertScanConfig::GlobalBudget {
                    start_offset,
                    max_messages,
                    batch_size,
                } => {
                    IggyDlqDuplicateAlertObserver::connect(
                        &iggy_config,
                        *start_offset,
                        *max_messages,
                        *batch_size,
                    )
                    .await
                }
                EventDlqDuplicateAlertScanConfig::FairWindow {
                    start_offset,
                    per_partition_messages,
                    batch_size,
                } => {
                    IggyDlqDuplicateAlertObserver::connect_fair_window(
                        &iggy_config,
                        *start_offset,
                        *per_partition_messages,
                        *batch_size,
                    )
                    .await
                }
                EventDlqDuplicateAlertScanConfig::MovingWindow { moving } => {
                    IggyDlqDuplicateAlertObserver::connect_moving_window(
                        &iggy_config,
                        moving.clone(),
                    )
                    .await
                }
            };
            match connection {
                Ok(connected) => observer = Some(connected),
                Err(error) => {
                    let _ = publisher.mark_unavailable();
                    tracing::warn!(
                        error_code = error.stable_code(),
                        "Physical DLQ duplicate observer could not connect; event delivery remains active"
                    );
                }
            }
        }

        if let Some(mut connected) = observer.take() {
            match connected.summarize().await {
                Ok(summary) => {
                    match publisher.publish(&summary) {
                        Ok(snapshot) => record_snapshot(snapshot),
                        Err(error) => {
                            tracing::warn!(
                                error_code = error.stable_code(),
                                "Physical DLQ duplicate alert runtime stopped publishing"
                            );
                            return;
                        }
                    }
                    observer = Some(connected);
                }
                Err(error) => {
                    let preserve_state = connected.preserves_process_local_state_after_scan_error();
                    let _ = publisher.mark_unavailable();
                    tracing::warn!(
                        error_code = error.stable_code(),
                        preserves_process_local_state = preserve_state,
                        "Physical DLQ duplicate scan failed; retry remains isolated from event delivery"
                    );
                    if preserve_state {
                        observer = Some(connected);
                    }
                }
            }
        }

        if wait_or_stop(config.poll, &mut stop_rx).await {
            let _ = publisher.mark_unavailable();
            return;
        }
    }
}

fn record_snapshot(snapshot: DlqDuplicateAlertRuntimeSnapshot) {
    let Some(evaluation) = snapshot.evaluation() else {
        return;
    };
    tracing::debug!(
        generation = snapshot.generation(),
        level = evaluation.level().stable_code(),
        physical_duplicates = evaluation.has_physical_duplicates(),
        identity_conflict = evaluation.has_identity_conflict(),
        duplicate_messages_threshold_reached = evaluation.duplicate_messages_threshold_reached(),
        duplicate_groups_threshold_reached = evaluation.duplicate_groups_threshold_reached(),
        max_copies_threshold_reached = evaluation.max_copies_threshold_reached(),
        "Recorded identifier-free physical DLQ duplicate alert snapshot"
    );
}

impl EventDlqDuplicateAlertObserverConfig {
    fn from_env(iggy_config: &rustok_iggy::IggyConfig) -> Result<Self> {
        let poll_ms = optional_u64_env(POLL_ENV, DEFAULT_POLL_MS)?;
        if poll_ms == 0 || poll_ms > MAX_POLL_MS {
            return Err(Error::Message(format!(
                "{POLL_ENV} must be between 1 and {MAX_POLL_MS}"
            )));
        }

        let scan = match optional_scan_mode_env()? {
            EventDlqDuplicateAlertScanMode::GlobalBudget => {
                EventDlqDuplicateAlertScanConfig::GlobalBudget {
                    start_offset: optional_u64_env(START_OFFSET_ENV, 0)?,
                    max_messages: optional_u32_env(MAX_MESSAGES_ENV, DEFAULT_MAX_MESSAGES)?,
                    batch_size: optional_u32_env(BATCH_SIZE_ENV, DEFAULT_BATCH_SIZE)?,
                }
            }
            EventDlqDuplicateAlertScanMode::FairWindow => {
                let per_partition_messages = required_u32_env(PER_PARTITION_MESSAGES_ENV)?;
                EventDlqDuplicateAlertScanConfig::FairWindow {
                    start_offset: optional_u64_env(START_OFFSET_ENV, 0)?,
                    per_partition_messages,
                    batch_size: optional_u32_env(
                        BATCH_SIZE_ENV,
                        DEFAULT_BATCH_SIZE.min(per_partition_messages),
                    )?,
                }
            }
            EventDlqDuplicateAlertScanMode::MovingWindow => moving_window_scan_config(
                iggy_config,
                required_u64_env(START_OFFSET_ENV)?,
                required_u32_env(PER_PARTITION_MESSAGES_ENV)?,
                required_u32_env(BATCH_SIZE_ENV)?,
                required_u32_env(ROLLING_MAX_CYCLES_ENV)?,
                required_u32_env(ROLLING_MAX_OBSERVATIONS_PER_CYCLE_ENV)?,
            )?,
        };

        let policy = DlqDuplicateAlertPolicy::new(
            required_u64_env(WARNING_MESSAGES_ENV)?,
            required_u64_env(CRITICAL_MESSAGES_ENV)?,
            required_u64_env(WARNING_GROUPS_ENV)?,
            required_u64_env(CRITICAL_GROUPS_ENV)?,
            required_u64_env(WARNING_MAX_COPIES_ENV)?,
            required_u64_env(CRITICAL_MAX_COPIES_ENV)?,
        )
        .map_err(|error| Error::Message(error.stable_code().to_string()))?;

        Ok(Self {
            poll: Duration::from_millis(poll_ms),
            scan,
            policy,
        })
    }
}

fn moving_window_scan_config(
    iggy_config: &rustok_iggy::IggyConfig,
    initial_offset: u64,
    per_partition_messages: u32,
    batch_size: u32,
    rolling_max_cycles: u32,
    rolling_max_observations_per_cycle: u32,
) -> Result<EventDlqDuplicateAlertScanConfig> {
    let moving = IggyDlqDuplicateAlertMovingWindowConfig::new(
        iggy_config,
        initial_offset,
        per_partition_messages,
        batch_size,
        rolling_max_cycles,
        rolling_max_observations_per_cycle,
    )
    .map_err(|error| Error::Message(error.stable_code().to_string()))?;
    Ok(EventDlqDuplicateAlertScanConfig::MovingWindow { moving })
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum EventDlqDuplicateAlertScanMode {
    GlobalBudget,
    FairWindow,
    MovingWindow,
}

fn optional_scan_mode_env() -> Result<EventDlqDuplicateAlertScanMode> {
    match env::var(SCAN_MODE_ENV) {
        Ok(value) => parse_scan_mode(&value).map_err(Error::Message),
        Err(env::VarError::NotPresent) => Ok(EventDlqDuplicateAlertScanMode::GlobalBudget),
        Err(error) => Err(Error::Message(format!(
            "failed to read {SCAN_MODE_ENV}: {error}"
        ))),
    }
}

fn parse_scan_mode(value: &str) -> std::result::Result<EventDlqDuplicateAlertScanMode, String> {
    match value.trim().to_ascii_lowercase().as_str() {
        "global" | "global_budget" => Ok(EventDlqDuplicateAlertScanMode::GlobalBudget),
        "fair" | "fair_window" => Ok(EventDlqDuplicateAlertScanMode::FairWindow),
        "moving" | "moving_window" => Ok(EventDlqDuplicateAlertScanMode::MovingWindow),
        _ => Err(format!(
            "{SCAN_MODE_ENV} must be global_budget, fair_window, or moving_window"
        )),
    }
}

fn observer_mode(
    profile: EventDeliveryProfile,
    iggy_mode: Option<&IggyMode>,
) -> Result<EventDlqDuplicateAlertObserverMode> {
    match profile {
        EventDeliveryProfile::OutboxLocal => {
            Ok(EventDlqDuplicateAlertObserverMode::NotApplicableOutboxLocal)
        }
        EventDeliveryProfile::OutboxIggy => match iggy_mode {
            Some(IggyMode::Bundled) => Ok(EventDlqDuplicateAlertObserverMode::IggyBundled),
            Some(IggyMode::External) => Ok(EventDlqDuplicateAlertObserverMode::IggyExternal),
            None => Err(Error::Message(
                "outbox_iggy runtime is missing its active Iggy mode".to_string(),
            )),
        },
    }
}

async fn wait_or_stop(delay: Duration, stop_rx: &mut tokio::sync::watch::Receiver<bool>) -> bool {
    tokio::select! {
        _ = tokio::time::sleep(delay) => false,
        changed = stop_rx.changed() => changed.is_err() || *stop_rx.borrow(),
    }
}

fn optional_bool_env(name: &str, default: bool) -> Result<bool> {
    match env::var(name) {
        Ok(value) => parse_bool(name, &value).map_err(Error::Message),
        Err(env::VarError::NotPresent) => Ok(default),
        Err(error) => Err(Error::Message(format!("failed to read {name}: {error}"))),
    }
}

fn parse_bool(name: &str, value: &str) -> std::result::Result<bool, String> {
    match value.trim().to_ascii_lowercase().as_str() {
        "1" | "true" | "yes" | "on" => Ok(true),
        "0" | "false" | "no" | "off" => Ok(false),
        _ => Err(format!("{name} must be a boolean")),
    }
}

fn optional_u64_env(name: &str, default: u64) -> Result<u64> {
    match env::var(name) {
        Ok(value) => value
            .parse::<u64>()
            .map_err(|error| Error::Message(format!("{name} is invalid: {error}"))),
        Err(env::VarError::NotPresent) => Ok(default),
        Err(error) => Err(Error::Message(format!("failed to read {name}: {error}"))),
    }
}

fn optional_u32_env(name: &str, default: u32) -> Result<u32> {
    let value = optional_u64_env(name, u64::from(default))?;
    u32::try_from(value).map_err(|_| Error::Message(format!("{name} exceeds u32")))
}

fn required_u32_env(name: &str) -> Result<u32> {
    let value = required_u64_env(name)?;
    u32::try_from(value).map_err(|_| Error::Message(format!("{name} exceeds u32")))
}

fn required_u64_env(name: &str) -> Result<u64> {
    match env::var(name) {
        Ok(value) => value
            .parse::<u64>()
            .map_err(|error| Error::Message(format!("{name} is invalid: {error}"))),
        Err(env::VarError::NotPresent) => Err(Error::Message(format!(
            "{name} is required when {ENABLE_ENV}=true"
        ))),
        Err(error) => Err(Error::Message(format!("failed to read {name}: {error}"))),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_event_delivery_profile_has_an_explicit_observer_mode() {
        assert_eq!(
            observer_mode(EventDeliveryProfile::OutboxLocal, None).unwrap(),
            EventDlqDuplicateAlertObserverMode::NotApplicableOutboxLocal
        );
        assert_eq!(
            observer_mode(EventDeliveryProfile::OutboxIggy, Some(&IggyMode::Bundled)).unwrap(),
            EventDlqDuplicateAlertObserverMode::IggyBundled
        );
        assert_eq!(
            observer_mode(EventDeliveryProfile::OutboxIggy, Some(&IggyMode::External)).unwrap(),
            EventDlqDuplicateAlertObserverMode::IggyExternal
        );
        assert!(observer_mode(EventDeliveryProfile::OutboxIggy, None).is_err());
    }

    #[test]
    fn startup_unavailable_state_has_no_task_or_snapshot() {
        let handle = EventDlqDuplicateAlertObserverHandle {
            mode: EventDlqDuplicateAlertObserverMode::Unavailable,
            subscriber: None,
            handle: None,
        };
        assert_eq!(
            handle.mode(),
            EventDlqDuplicateAlertObserverMode::Unavailable
        );
        assert_eq!(handle.current_snapshot(), None);
        assert!(!handle.is_finished());
    }

    #[test]
    fn scan_mode_parser_preserves_global_default_and_explicit_opt_in_modes() {
        assert_eq!(
            parse_scan_mode("global_budget").unwrap(),
            EventDlqDuplicateAlertScanMode::GlobalBudget
        );
        assert_eq!(
            parse_scan_mode("fair_window").unwrap(),
            EventDlqDuplicateAlertScanMode::FairWindow
        );
        assert_eq!(
            parse_scan_mode("moving_window").unwrap(),
            EventDlqDuplicateAlertScanMode::MovingWindow
        );
        assert!(parse_scan_mode("moving_cursor").is_err());
    }

    #[test]
    fn moving_window_configuration_is_explicit_and_fail_closed() {
        let mut iggy_config = rustok_iggy::IggyConfig::default();
        iggy_config.topology.domain_partitions = 2;
        assert!(moving_window_scan_config(&iggy_config, 10, 2, 1, 3, 3).is_err());
        let valid = moving_window_scan_config(&iggy_config, 10, 2, 1, 3, 4).unwrap();
        let EventDlqDuplicateAlertScanConfig::MovingWindow { moving } = valid else {
            panic!("expected moving-window configuration");
        };
        assert_eq!(moving.partition_count(), 2);
        assert_eq!(moving.total_message_budget(), 4);
        assert!(!moving.progress_persisted());
        assert!(moving.restart_resets_to_initial_offset());
    }

    #[test]
    fn boolean_parser_is_bounded() {
        assert_eq!(parse_bool("FLAG", "true"), Ok(true));
        assert_eq!(parse_bool("FLAG", "off"), Ok(false));
        assert!(parse_bool("FLAG", "sometimes").is_err());
    }
}
