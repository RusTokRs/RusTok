use std::env;
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{Duration, Instant};

use rustok_iggy::{ConsumedContractEvent, IggyTransport};
use rustok_index::MutationApplyOutcome;
use rustok_social_graph::index_consumer::{
    SocialGraphIndexConsumer, SocialGraphIndexProcessOutcome,
};
use rustok_telemetry::runtime_consumer_metrics;
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
const METRICS_CONSUMER: &str = "social_graph_index";
const STAGE_STARTUP: &str = "startup";
const STAGE_RECEIVE: &str = "receive";
const STAGE_PROJECTION: &str = "projection";
const STAGE_DLQ_PUBLISH: &str = "dlq_publish";
const STAGE_ACKNOWLEDGEMENT: &str = "acknowledgement";
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

    if let Err(error) = runtime_consumer_metrics::ensure_registered() {
        tracing::debug!(
            error = %error,
            worker = METRICS_CONSUMER,
            "Runtime consumer metrics are unavailable; continuing without registration"
        );
    }

    let event_runtime = ctx
        .shared_get::<Arc<EventRuntime>>()
        .ok_or_else(|| Error::Message("EventRuntime is unavailable".to_string()))?;
    if event_runtime.delivery_profile != EventDeliveryProfile::OutboxIggy {
        return Err(Error::Message(format!(
            "{ENABLE_ENV}=true requires rustok.events.delivery_profile=outbox_iggy"
        )));
    }
    let transport = ctx.shared_get::<Arc<IggyTransport>>().ok_or_else(|| {
        Error::Message(
            "outbox_iggy runtime did not publish its configured Iggy transport".to_string(),
        )
    })?;

    if !ctx.shared_contains::<StopHandle>() {
        let (stop_handle, _stop_rx) = StopHandle::new();
        ctx.shared_insert(stop_handle);
    }
    let stop_rx = ctx
        .shared_get::<StopHandle>()
        .expect("StopHandle must be registered before Social Graph Index worker startup")
        .subscribe();

    let config = SocialGraphIndexWorkerConfig::from_context(ctx)?;
    let consumer = match SocialGraphIndexConsumer::open(transport, ctx.db_clone()).await {
        Ok(consumer) => consumer,
        Err(error) => {
            runtime_consumer_metrics::record_failure(
                METRICS_CONSUMER,
                STAGE_STARTUP,
                error.stable_code(),
            );
            return Err(Error::Message(format!(
                "Social Graph Index consumer startup failed [{}]",
                error.stable_code()
            )));
        }
    };

    let instance_id = SOCIAL_GRAPH_INDEX_WORKER_INSTANCE_IDS.fetch_add(1, Ordering::Relaxed);
    runtime_consumer_metrics::record_worker_start(METRICS_CONSUMER);
    tracing::info!(
        worker = METRICS_CONSUMER,
        instance_id,
        consumer_group = rustok_social_graph::index_consumer::SOCIAL_GRAPH_INDEX_CONSUMER_GROUP,
        dlq_enabled = config.dlq_enabled,
        max_attempts = config.max_attempts,
        "Starting Social Graph Index consumer worker"
    );
    ctx.shared_insert(SocialGraphIndexWorkerHandle {
        instance_id,
        _handle: tokio::spawn(social_graph_index_worker_loop(consumer, config, stop_rx)),
    });
    Ok(())
}

async fn social_graph_index_worker_loop(
    mut consumer: SocialGraphIndexConsumer,
    config: SocialGraphIndexWorkerConfig,
    mut stop_rx: tokio::sync::watch::Receiver<bool>,
) {
    loop {
        if *stop_rx.borrow() {
            record_worker_termination("shutdown");
            tracing::info!(
                worker = METRICS_CONSUMER,
                "Worker received shutdown signal"
            );
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
            record_worker_termination("shutdown");
            tracing::info!(
                worker = METRICS_CONSUMER,
                "Worker stopped before next receive"
            );
            return;
        };

        match received {
            Ok(Some(consumed)) => {
                let delivery_started = Instant::now();
                runtime_consumer_metrics::begin_delivery(METRICS_CONSUMER, consumed.offset());
                match process_delivery(&consumer, &config, &mut stop_rx, &consumed).await {
                    Ok(DeliveryCompletion::Completed(outcome)) => {
                        let outcome_label = process_outcome_label(&outcome);
                        runtime_consumer_metrics::complete_delivery(
                            METRICS_CONSUMER,
                            outcome_label,
                            delivery_started.elapsed(),
                            consumed.offset(),
                        );
                        tracing::debug!(
                            worker = METRICS_CONSUMER,
                            event_id = %consumed.envelope.id(),
                            outcome = ?outcome,
                            "Social Graph Index delivery completed"
                        );
                    }
                    Ok(DeliveryCompletion::Stopped) => {
                        record_worker_termination("shutdown_in_flight");
                        tracing::info!(
                            worker = METRICS_CONSUMER,
                            event_id = %consumed.envelope.id(),
                            "Worker stopped with broker offset uncommitted"
                        );
                        return;
                    }
                    Err(error) => {
                        record_worker_termination("delivery_failure");
                        tracing::error!(
                            worker = METRICS_CONSUMER,
                            event_id = %consumed.envelope.id(),
                            error = %error,
                            "Social Graph Index worker terminated with broker offset uncommitted"
                        );
                        return;
                    }
                }
            }
            Ok(None) => {
                if wait_or_stop(config.idle_poll, &mut stop_rx).await {
                    record_worker_termination("shutdown");
                    return;
                }
            }
            Err(error) => {
                runtime_consumer_metrics::record_failure(
                    METRICS_CONSUMER,
                    STAGE_RECEIVE,
                    error.stable_code(),
                );
                record_worker_termination("receive_failure");
                tracing::error!(
                    worker = METRICS_CONSUMER,
                    error_code = error.stable_code(),
                    retryable = error.is_retryable(),
                    "Social Graph Index broker receive failed; persistent cursor remains uncommitted"
                );
                return;
            }
        }
    }
}

fn record_worker_termination(reason: &'static str) {
    runtime_consumer_metrics::record_worker_termination(METRICS_CONSUMER, reason);
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
                return acknowledge_terminal_result(consumer, config, stop_rx, consumed, outcome)
                    .await;
            }
            Err(error) => {
                let error_code = error.stable_code();
                let retryable = error.is_retryable();
                runtime_consumer_metrics::record_failure(
                    METRICS_CONSUMER,
                    STAGE_PROJECTION,
                    error_code,
                );
                if retryable && attempt < config.max_attempts {
                    let delay = retry_delay(config, attempt);
                    runtime_consumer_metrics::record_retry(
                        METRICS_CONSUMER,
                        STAGE_PROJECTION,
                    );
                    tracing::warn!(
                        worker = METRICS_CONSUMER,
                        event_id = %consumed.envelope.id(),
                        error_code,
                        attempt,
                        retry_delay_ms = duration_millis(delay),
                        "Social Graph Index projection failed; retrying without acknowledgement"
                    );
                    if wait_or_stop(delay, stop_rx).await {
                        return Ok(DeliveryCompletion::Stopped);
                    }
                    attempt += 1;
                    continue;
                }

                if config.dlq_enabled {
                    if let Err(dlq_error) = consumer
                        .publish_consumed_to_dlq(consumed, error_code, attempt)
                        .await
                    {
                        runtime_consumer_metrics::record_failure(
                            METRICS_CONSUMER,
                            STAGE_DLQ_PUBLISH,
                            dlq_error.stable_code(),
                        );
                        runtime_consumer_metrics::record_dlq(METRICS_CONSUMER, "failure");
                        return Err(format!(
                            "DLQ publication failed [{}]",
                            dlq_error.stable_code()
                        ));
                    }
                    runtime_consumer_metrics::record_dlq(METRICS_CONSUMER, "success");
                    tracing::warn!(
                        worker = METRICS_CONSUMER,
                        event_id = %consumed.envelope.id(),
                        error_code,
                        retryable,
                        attempts = attempt,
                        "Social Graph Index poison delivery published to DLQ; acknowledging source offset"
                    );
                    return acknowledge_terminal_result(
                        consumer,
                        config,
                        stop_rx,
                        consumed,
                        SocialGraphIndexProcessOutcome::DeadLettered { error_code },
                    )
                    .await;
                }

                return Err(format!(
                    "projection failed after {attempt} attempt(s) [{error_code}]; DLQ is disabled"
                ));
            }
        }
    }
}

async fn acknowledge_terminal_result(
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
                runtime_consumer_metrics::record_failure(
                    METRICS_CONSUMER,
                    STAGE_ACKNOWLEDGEMENT,
                    error.stable_code(),
                );
                runtime_consumer_metrics::record_retry(
                    METRICS_CONSUMER,
                    STAGE_ACKNOWLEDGEMENT,
                );
                tracing::warn!(
                    worker = METRICS_CONSUMER,
                    event_id = %consumed.envelope.id(),
                    error_code = error.stable_code(),
                    attempt,
                    retry_delay_ms = duration_millis(delay),
                    "Terminal durable result exists but broker acknowledgement failed; retrying acknowledgement only"
                );
                if wait_or_stop(delay, stop_rx).await {
                    return Ok(DeliveryCompletion::Stopped);
                }
                attempt += 1;
            }
            Err(error) => {
                runtime_consumer_metrics::record_failure(
                    METRICS_CONSUMER,
                    STAGE_ACKNOWLEDGEMENT,
                    error.stable_code(),
                );
                let recovery = match &outcome {
                    SocialGraphIndexProcessOutcome::DeadLettered { .. } => {
                        "DLQ publication succeeded but the source offset remains uncommitted; redelivery may republish until a durable DLQ identity exists"
                    }
                    _ => "terminal handling remains replay-safe",
                };
                return Err(format!(
                    "broker acknowledgement failed after {attempt} attempt(s) [{}]; {recovery}",
                    error.stable_code()
                ));
            }
        }
    }
}

fn process_outcome_label(outcome: &SocialGraphIndexProcessOutcome) -> &'static str {
    match outcome {
        SocialGraphIndexProcessOutcome::Projected(MutationApplyOutcome::Applied { .. }) => {
            "applied"
        }
        SocialGraphIndexProcessOutcome::Projected(MutationApplyOutcome::Duplicate { .. }) => {
            "duplicate"
        }
        SocialGraphIndexProcessOutcome::Projected(MutationApplyOutcome::StaleIgnored { .. }) => {
            "stale_ignored"
        }
        SocialGraphIndexProcessOutcome::IgnoredUnrelated { .. } => "ignored_unrelated",
        SocialGraphIndexProcessOutcome::DeadLettered { .. } => "dead_lettered",
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

    #[test]
    fn process_outcome_labels_are_bounded() {
        assert_eq!(
            process_outcome_label(&SocialGraphIndexProcessOutcome::Projected(
                MutationApplyOutcome::Applied { source_version: 1 }
            )),
            "applied"
        );
        assert_eq!(
            process_outcome_label(&SocialGraphIndexProcessOutcome::Projected(
                MutationApplyOutcome::Duplicate { source_version: 1 }
            )),
            "duplicate"
        );
        assert_eq!(
            process_outcome_label(&SocialGraphIndexProcessOutcome::Projected(
                MutationApplyOutcome::StaleIgnored {
                    incoming_source_version: 1,
                    current_source_version: 2,
                }
            )),
            "stale_ignored"
        );
        assert_eq!(
            process_outcome_label(&SocialGraphIndexProcessOutcome::IgnoredUnrelated {
                event_type: "other.event.v1".to_string(),
            }),
            "ignored_unrelated"
        );
        assert_eq!(
            process_outcome_label(&SocialGraphIndexProcessOutcome::DeadLettered {
                error_code: "social_graph.index.envelope_invalid",
            }),
            "dead_lettered"
        );
    }
}
