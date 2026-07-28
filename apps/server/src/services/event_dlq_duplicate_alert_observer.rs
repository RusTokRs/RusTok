use std::env;
use std::sync::Arc;
use std::time::Duration;

use rustok_iggy::{
    DlqDuplicateAlertPolicy, DlqDuplicateAlertRuntimePublisher,
    DlqDuplicateAlertRuntimeSnapshot, DlqDuplicateAlertRuntimeSubscriber,
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
const START_OFFSET_ENV: &str = "RUSTOK_EVENT_DLQ_DUPLICATE_ALERT_START_OFFSET";
const MAX_MESSAGES_ENV: &str = "RUSTOK_EVENT_DLQ_DUPLICATE_ALERT_MAX_MESSAGES";
const BATCH_SIZE_ENV: &str = "RUSTOK_EVENT_DLQ_DUPLICATE_ALERT_BATCH_SIZE";
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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EventDlqDuplicateAlertObserverMode {
    Disabled,
    NotApplicableMemory,
    NotApplicableOutboxLocal,
    IggyBundled,
    IggyExternal,
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
        self.subscriber.as_ref().map(|subscriber| subscriber.current())
    }

    pub fn is_finished(&self) -> bool {
        self.handle.as_ref().is_some_and(JoinHandle::is_finished)
    }
}

struct EventDlqDuplicateAlertObserverConfig {
    poll: Duration,
    start_offset: u64,
    max_messages: u32,
    batch_size: u32,
    policy: DlqDuplicateAlertPolicy,
}

pub async fn start_event_dlq_duplicate_alert_observer(
    ctx: &ServerRuntimeContext,
) -> Result<()> {
    if !ctx.settings().runtime.runs_background_workers()
        || ctx.shared_contains::<EventDlqDuplicateAlertObserverHandle>()
    {
        return Ok(());
    }

    let enabled = optional_bool_env(ENABLE_ENV, false)?;
    if !enabled {
        ctx.shared_insert(EventDlqDuplicateAlertObserverHandle {
            mode: EventDlqDuplicateAlertObserverMode::Disabled,
            subscriber: None,
            handle: None,
        });
        return Ok(());
    }

    let runtime = ctx
        .shared_get::<Arc<EventRuntime>>()
        .ok_or_else(|| Error::Message("EventRuntime is unavailable".to_string()))?;
    let mode = observer_mode(runtime.delivery_profile, runtime.iggy_mode.as_ref());
    if matches!(
        mode,
        EventDlqDuplicateAlertObserverMode::NotApplicableMemory
            | EventDlqDuplicateAlertObserverMode::NotApplicableOutboxLocal
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
        return Ok(());
    }

    let transport = ctx.shared_get::<Arc<IggyTransport>>().ok_or_else(|| {
        Error::Message(
            "outbox_iggy runtime did not publish its configured Iggy transport".to_string(),
        )
    })?;
    let iggy_config = transport.config().clone();
    let config = EventDlqDuplicateAlertObserverConfig::from_env()?;

    if !ctx.shared_contains::<StopHandle>() {
        let (stop_handle, _stop_rx) = StopHandle::new();
        ctx.shared_insert(stop_handle);
    }
    let stop_rx = ctx
        .shared_get::<StopHandle>()
        .expect("StopHandle must exist before DLQ duplicate observer startup")
        .subscribe();

    let (publisher, subscriber) = DlqDuplicateAlertRuntimePublisher::new(config.policy);
    let handle = tokio::spawn(observer_loop(
        iggy_config,
        config,
        publisher,
        stop_rx,
    ));
    ctx.shared_insert(EventDlqDuplicateAlertObserverHandle {
        mode,
        subscriber: Some(subscriber),
        handle: Some(handle),
    });
    tracing::info!(
        mode = ?mode,
        "Starting mode-aware physical DLQ duplicate alert observer"
    );
    Ok(())
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
            match IggyDlqDuplicateAlertObserver::connect(
                &iggy_config,
                config.start_offset,
                config.max_messages,
                config.batch_size,
            )
            .await
            {
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

        if let Some(connected) = observer.as_ref() {
            match connected.summarize().await {
                Ok(summary) => match publisher.publish(&summary) {
                    Ok(snapshot) => record_snapshot(snapshot),
                    Err(error) => {
                        tracing::warn!(
                            error_code = error.stable_code(),
                            "Physical DLQ duplicate alert runtime stopped publishing"
                        );
                        return;
                    }
                },
                Err(error) => {
                    let _ = publisher.mark_unavailable();
                    tracing::warn!(
                        error_code = error.stable_code(),
                        "Physical DLQ duplicate scan failed; reconnecting without affecting event delivery"
                    );
                    observer = None;
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
        duplicate_messages_threshold_reached = evaluation
            .duplicate_messages_threshold_reached(),
        duplicate_groups_threshold_reached = evaluation
            .duplicate_groups_threshold_reached(),
        max_copies_threshold_reached = evaluation.max_copies_threshold_reached(),
        "Recorded identifier-free physical DLQ duplicate alert snapshot"
    );
}

impl EventDlqDuplicateAlertObserverConfig {
    fn from_env() -> Result<Self> {
        let poll_ms = optional_u64_env(POLL_ENV, DEFAULT_POLL_MS)?;
        if poll_ms == 0 || poll_ms > MAX_POLL_MS {
            return Err(Error::Message(format!(
                "{POLL_ENV} must be between 1 and {MAX_POLL_MS}"
            )));
        }
        let max_messages = optional_u32_env(MAX_MESSAGES_ENV, DEFAULT_MAX_MESSAGES)?;
        let batch_size = optional_u32_env(BATCH_SIZE_ENV, DEFAULT_BATCH_SIZE)?;
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
            start_offset: optional_u64_env(START_OFFSET_ENV, 0)?,
            max_messages,
            batch_size,
            policy,
        })
    }
}

fn observer_mode(
    profile: EventDeliveryProfile,
    iggy_mode: Option<&IggyMode>,
) -> EventDlqDuplicateAlertObserverMode {
    match profile {
        EventDeliveryProfile::Memory => EventDlqDuplicateAlertObserverMode::NotApplicableMemory,
        EventDeliveryProfile::OutboxLocal => {
            EventDlqDuplicateAlertObserverMode::NotApplicableOutboxLocal
        }
        EventDeliveryProfile::OutboxIggy => match iggy_mode {
            Some(IggyMode::Bundled) => EventDlqDuplicateAlertObserverMode::IggyBundled,
            Some(IggyMode::External) => EventDlqDuplicateAlertObserverMode::IggyExternal,
            None => EventDlqDuplicateAlertObserverMode::IggyExternal,
        },
    }
}

async fn wait_or_stop(
    delay: Duration,
    stop_rx: &mut tokio::sync::watch::Receiver<bool>,
) -> bool {
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

fn required_u64_env(name: &str) -> Result<u64> {
    match env::var(name) {
        Ok(value) => value
            .parse::<u64>()
            .map_err(|error| Error::Message(format!("{name} is invalid: {error}"))),
        Err(env::VarError::NotPresent) => {
            Err(Error::Message(format!("{name} is required when {ENABLE_ENV}=true")))
        }
        Err(error) => Err(Error::Message(format!("failed to read {name}: {error}"))),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_event_delivery_profile_has_an_explicit_observer_mode() {
        assert_eq!(
            observer_mode(EventDeliveryProfile::Memory, None),
            EventDlqDuplicateAlertObserverMode::NotApplicableMemory
        );
        assert_eq!(
            observer_mode(EventDeliveryProfile::OutboxLocal, None),
            EventDlqDuplicateAlertObserverMode::NotApplicableOutboxLocal
        );
        assert_eq!(
            observer_mode(EventDeliveryProfile::OutboxIggy, Some(&IggyMode::Bundled)),
            EventDlqDuplicateAlertObserverMode::IggyBundled
        );
        assert_eq!(
            observer_mode(EventDeliveryProfile::OutboxIggy, Some(&IggyMode::External)),
            EventDlqDuplicateAlertObserverMode::IggyExternal
        );
    }

    #[test]
    fn boolean_parser_is_bounded() {
        assert_eq!(parse_bool("FLAG", "true"), Ok(true));
        assert_eq!(parse_bool("FLAG", "off"), Ok(false));
        assert!(parse_bool("FLAG", "sometimes").is_err());
    }
}