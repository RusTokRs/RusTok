use std::env;
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::Duration;

use rustok_iggy::{ConsumedContractEvent, IggyTransport};
use rustok_social_graph::index_consumer::{
    SocialGraphIndexConsumer, SocialGraphIndexConsumerError, SocialGraphIndexProcessOutcome,
};
use tokio::task::JoinHandle;

use crate::common::settings::EventDeliveryProfile;
use crate::error::{Error, Result};
use crate::services::app_lifecycle::StopHandle;
use crate::services::event_transport_factory::EventRuntime;
use crate::services::server_runtime_context::ServerRuntimeContext;

const ENABLE_ENV: &str = "RUSTOK_SOCIAL_GRAPH_INDEX_CONSUMER_ENABLED";
const IDLE_POLL_ENV: &str = "RUSTOK_SOCIAL_GRAPH_INDEX_CONSUMER_IDLE_POLL_MS";
const DEFAULT_IDLE_POLL_MS: u64 = 500;
const MAX_IDLE_POLL_MS: u64 = 60_000;
static SOCIAL_GRAPH_INDEX_WORKER_INSTANCE_IDS: AtomicU64 = AtomicU64::new(1);

#[derive(Debug, Clone)]
struct SocialGraphIndexWorkerConfig {
    max_attempts: u32,
    base_backoff: Duration,
    max_backoff: Duration,
    idle_poll: Duration,
    dlq_enabled: bool,
}

impl SocialGraphIndexWorkerConfig {
    fn from_context(ctx: &ServerRuntimeContext) -> Result<Self> {
        let settings = ctx.settings();
        let max_attempts = if settings.events.dlq.enabled {
            settings.events.dlq.max_attempts
        } else {
            settings.events.relay_retry_policy.max_attempts
        };
        let max_attempts = u32::try_from(max_attempts).map_err(|_| {
            Error::Message(
                "Social Graph Index consumer max attempts must be a positive u32".to_string(),
            )
        })?;
        if max_attempts == 0 {
            return Err(Error::Message(
                "Social Graph Index consumer max attempts must be greater than zero".to_string(),
            ));
        }

        let base_backoff =
            Duration::from_millis(settings.events.relay_retry_policy.base_backoff_ms);
        let max_backoff =
            Duration::from_millis(settings.events.relay_retry_policy.max_backoff_ms);
        if base_backoff.is_zero() || max_backoff < base_backoff {
            return Err(Error::Message(
                "Social Graph Index consumer retry backoff must be positive and bounded"
                    .to_string(),
            ));
        }

        let idle_poll_ms = optional_u64_env(IDLE_POLL_ENV, DEFAULT_IDLE_POLL_MS)?;
        if idle_poll_ms == 0 || idle_poll_ms > MAX_IDLE_POLL_MS {
            return Err(Error::Message(format!(
                "{IDLE_POLL_ENV} must be between 1 and {MAX_IDLE_POLL_MS}"
            )));
        }

        Ok(Self {
            max_attempts,
            base_backoff,
            max_backoff,
            idle_poll: Duration::from_millis(idle_poll_ms),
            dlq_enabled: settings.events.dlq.enabled,
        })
    }
}

pub struct SocialGraphIndexWorkerHandle {
    instance_id: u64,
    _handle: JoinHandle<()>,
}

impl SocialGraphIndexWorkerHandle {
    pub fn instance_id(&self) -> u64 {
        self.instance_id
    }

    pub fn is_finished(&self) -> bool {
        self._handle.is_finished()
    }

    pub fn is_ready(&self) -> bool {
        !self.is_finished()
    }
}

pub fn social_graph_index_consumer_enabled() -> Result<bool> {
    match env::var(ENABLE_ENV) {
        Ok(value) => parse_bool(ENABLE_ENV, &value).map_err(Error::Message),
        Err(env::VarError::NotPresent) => Ok(false),
        Err(error) => Err(Error::Message(format!(
            "failed to read {ENABLE_ENV}: {error}"
        ))),
    }
}

pub async fn start_social_graph_index_worker_if_enabled(
    ctx: &ServerRuntimeContext,
) -> Result<()> {
    if !ctx.settings().runtime.runs_background_workers()
        || ctx.shared_contains::<SocialGraphIndexWorkerHandle>()
    {
        return Ok(());
    }
    if !social_graph_index_consumer_enabled()? {
        tracing::info!(
            env = ENABLE_ENV,
            "Social Graph Index consumer disabled by default-off runtime flag"
        );
        return Ok(());
    }

    let event_runtime = ctx
        .shared_get::<Arc<EventRuntime>>()
        .ok_or_else(|| Error::Message("EventRuntime is unavailable".to_string()))?;
    if event_runtime.delivery_profile != EventDeliveryProfile::OutboxIggy {
        return Err(Error::Message(format!(
            "{ENABLE_ENV}=true requires rustok.events.delivery_profile=outbox_iggy"
        )));
    }

    if !ctx.shared_contains::<StopHandle>() {
        let (stop_handle, _stop_rx) = StopHandle::new();
        ctx.shared_insert(stop_handle);
    }
    let stop_rx = ctx
        .shared_get::<StopHandle>()
        .expect("StopHandle must be registered before Social Graph Index worker startup")
        .subscribe();

    let config = SocialGraphIndexWorkerConfig::from_context(ctx)?;
    let iggy_config = crate::services::iggy_connector_settings_service::IggyConnectorSettingsService::resolved_config(ctx)
        .await
        .map_err(|error| {
            Error::Message(format!(
                "Social Graph Index consumer Iggy configuration failed: {error}"
            ))
        })?;
    let transport = Arc::new(IggyTransport::new(iggy_config).await.map_err(|error| {
        Error::Message(format!(
            "Social Graph Index consumer broker connection failed: {error}"
        ))
    })?);
    let consumer = SocialGraphIndexConsumer::open(Arc::clone(&transport), ctx.db_clone())
        .await
        .map_err(|error| {
            Error::Message(format!(
                "Social Graph Index consumer startup failed [{}]",
                error.stable_code()
            ))
        })?;

    let instance_id = SOCIAL_GRAPH_INDEX_WORKER_INSTANCE_IDS.fetch_add(1, Ordering::Relaxed);
    tracing::info!(
        worker = "social_graph_index",
        instance_id,
        consumer_group = rustok_social_graph::index_consumer::SOCIAL_GRAPH_INDEX_CONSUMER_GROUP,
        dlq_enabled = config.dlq_enabled,
        max_attempts = config.max_attempts,
        "Starting Social Graph Index consumer worker"
    );
    ctx.shared_insert(SocialGraphIndexWorkerHandle {
        instance_id,
        _handle: tokio::spawn(social_graph_index_worker_loop(
            consumer,
            transport,
            config,
            stop_rx,
        )),
    });
    Ok(())
}

async fn social_graph_index_worker_loop(
    mut consumer: SocialGraphIndexConsumer,
    transport: Arc<IggyTransport>,
    config: SocialGraphIndexWorkerConfig,
    mut stop_rx: tokio::sync::watch::Receiver<bool>,
) {
    loop {
        if *stop_rx.borrow() {
            tracing::info!(
                worker = "social_graph_index",
                "Worker received shutdown signal"
            );
            shutdown_transport(&transport).await;
            return;
        }

        let received = tokio::select! {
            result = consumer.receive_next() => Some(result),
            changed = stop_rx.changed() => {
                if changed.is_err() || *stop_rx.borrow() {
                    None
                } else {
                    continue;
                }
            }
        };
        let Some(received) = received else {
            tracing::info!(
                worker = "social_graph_index",
                "Worker stopped before next receive"
            );
            shutdown_transport(&transport).await;
            return;
        };

        match received {
            Ok(Some(consumed)) => {
                match process_delivery(&consumer, &config, &mut stop_rx, &consumed).await {
                    Ok(DeliveryCompletion::Completed(outcome)) => tracing::debug!(
                        worker = "social_graph_index",
                        event_id = %consumed.envelope.id(),
                        outcome = ?outcome,
                        "Social Graph Index delivery completed"
                    ),
                    Ok(DeliveryCompletion::Stopped) => {
                        tracing::info!(
                            worker = "social_graph_index",
                            event_id = %consumed.envelope.id(),
                            "Worker stopped with broker offset uncommitted"
                        );
                        shutdown_transport(&transport).await;
                        return;
                    }
                    Err(error) => {
                        tracing::error!(
                            worker = "social_graph_index",
                            event_id = %consumed.envelope.id(),
                            error = %error,
                            "Social Graph Index worker terminated with broker offset uncommitted"
                        );
                        shutdown_transport(&transport).await;
                        return;
                    }
                }
            }
            Ok(None) => {
                if wait_or_stop(config.idle_poll, &mut stop_rx).await {
                    shutdown_transport(&transport).await;
                    return;
                }
            }
            Err(error) => {
                tracing::error!(
                    worker = "social_graph_index",
                    error_code = error.stable_code(),
                    retryable = error.is_retryable(),
                    "Social Graph Index broker receive failed; persistent cursor remains uncommitted"
                );
                shutdown_transport(&transport).await;
                return;
            }
        }
    }
}

#[derive(Debug)]
enum DeliveryCompletion {
    Completed(SocialGraphIndexProcessOutcome),
    Stopped,
}

async fn process_delivery(
    consumer: &SocialGraphIndexConsumer,
    config: &SocialGraphIndexWorkerConfig,
    stop_rx: &mut tokio::sync::watch::Receiver<bool>,
    consumed: &ConsumedContractEvent,
) -> std::result::Result<DeliveryCompletion, String> {
    let mut attempt = 1;
    loop {
        match consumer.project_consumed(consumed).await {
            Ok(outcome) => {
                return acknowledge_durable_result(consumer, config, stop_rx, consumed, outcome)
                    .await;
            }
            Err(error) => {
                let error_code = error.stable_code();
                let retryable = error.is_retryable();
                if retryable && attempt < config.max_attempts {
                    let delay = retry_delay(config, attempt);
                    tracing::warn!(
                        worker = "social_graph_index",
                        event_id = %consumed.envelope.id(),
                        error_code,
                        attempt,
                        retry_delay_ms = delay.as_millis() as u64,
                        "Social Graph Index projection failed; retrying without acknowledgement"
                    );
                    if wait_or_stop(delay, stop_rx).await {
                        return Ok(DeliveryCompletion::Stopped);
                    }
                    attempt += 1;
                    continue;
                }

                if config.dlq_enabled {
                    consumer
                        .move_to_dlq_and_acknowledge(consumed, error_code, attempt)
                        .await
                        .map_err(|dlq_error| {
                            format!(
                                "DLQ publication or source acknowledgement failed [{}]",
                                dlq_error.stable_code()
                            )
                        })?;
                    tracing::warn!(
                        worker = "social_graph_index",
                        event_id = %consumed.envelope.id(),
                        error_code,
                        retryable,
                        attempts = attempt,
                        "Social Graph Index poison delivery moved to DLQ and acknowledged"
                    );
                    return Ok(DeliveryCompletion::Completed(
                        SocialGraphIndexProcessOutcome::DeadLettered { error_code },
                    ));
                }

                return Err(format!(
                    "projection failed after {attempt} attempt(s) [{error_code}]; DLQ is disabled"
                ));
            }
        }
    }
}

async fn acknowledge_durable_result(
    consumer: &SocialGraphIndexConsumer,
    config: &SocialGraphIndexWorkerConfig,
    stop_rx: &mut tokio::sync::watch::Receiver<bool>,
    consumed: &ConsumedContractEvent,
    outcome: SocialGraphIndexProcessOutcome,
) -> std::result::Result<DeliveryCompletion, String> {
    let mut attempt = 1;
    loop {
        match consumer.acknowledge_consumed(consumed).await {
            Ok(()) => return Ok(DeliveryCompletion::Completed(outcome)),
            Err(error) if attempt < config.max_attempts => {
                let delay = retry_delay(config, attempt);
                tracing::warn!(
                    worker = "social_graph_index",
                    event_id = %consumed.envelope.id(),
                    error_code = error.stable_code(),
                    attempt,
                    retry_delay_ms = delay.as_millis() as u64,
                    "Durable Index result exists but broker acknowledgement failed; retrying acknowledgement only"
                );
                if wait_or_stop(delay, stop_rx).await {
                    return Ok(DeliveryCompletion::Stopped);
                }
                attempt += 1;
            }
            Err(error) => {
                return Err(format!(
                    "broker acknowledgement failed after {attempt} attempt(s) [{}]; durable Index result remains replay-safe",
                    error.stable_code()
                ));
            }
        }
    }
}

fn retry_delay(config: &SocialGraphIndexWorkerConfig, attempt: u32) -> Duration {
    let shift = attempt.saturating_sub(1).min(20);
    let multiplier = 1_u64.checked_shl(shift).unwrap_or(u64::MAX);
    let base_ms = duration_millis(config.base_backoff);
    let max_ms = duration_millis(config.max_backoff);
    Duration::from_millis(base_ms.saturating_mul(multiplier).min(max_ms))
}

fn duration_millis(duration: Duration) -> u64 {
    u64::try_from(duration.as_millis()).unwrap_or(u64::MAX)
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

async fn shutdown_transport(transport: &IggyTransport) {
    if let Err(error) = transport.shutdown().await {
        tracing::warn!(
            worker = "social_graph_index",
            error = %error,
            "Social Graph Index worker transport shutdown failed"
        );
    }
}

fn parse_bool(name: &str, value: &str) -> std::result::Result<bool, String> {
    match value.trim().to_ascii_lowercase().as_str() {
        "1" | "true" => Ok(true),
        "0" | "false" => Ok(false),
        _ => Err(format!("{name} must be true/false or 1/0")),
    }
}

fn optional_u64_env(name: &str, default: u64) -> Result<u64> {
    match env::var(name) {
        Ok(value) => value
            .parse::<u64>()
            .map_err(|error| Error::Message(format!("{name} is invalid: {error}"))),
        Err(env::VarError::NotPresent) => Ok(default),
        Err(error) => Err(Error::Message(format!(
            "failed to read {name}: {error}"
        ))),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn enable_flag_is_strict_and_default_off_compatible() {
        assert_eq!(parse_bool(ENABLE_ENV, "true"), Ok(true));
        assert_eq!(parse_bool(ENABLE_ENV, "0"), Ok(false));
        assert!(parse_bool(ENABLE_ENV, "enabled").is_err());
    }

    #[test]
    fn exponential_retry_delay_is_bounded() {
        let config = SocialGraphIndexWorkerConfig {
            max_attempts: 10,
            base_backoff: Duration::from_millis(100),
            max_backoff: Duration::from_millis(450),
            idle_poll: Duration::from_millis(500),
            dlq_enabled: true,
        };
        assert_eq!(retry_delay(&config, 1), Duration::from_millis(100));
        assert_eq!(retry_delay(&config, 2), Duration::from_millis(200));
        assert_eq!(retry_delay(&config, 3), Duration::from_millis(400));
        assert_eq!(retry_delay(&config, 4), Duration::from_millis(450));
    }
}
