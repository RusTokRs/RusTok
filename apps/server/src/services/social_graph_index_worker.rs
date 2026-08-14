use std::env;
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{Duration, Instant};

use rustok_iggy::{
    ConsumedContractDecodeFailure, ConsumedContractEvent, IggyTransport, PersistentContractDelivery,
};
use rustok_iggy_connector::migrations::{
    ConsumerPoisonIdentity, ConsumerPoisonPublishClaim, ConsumerPoisonReceiptStore,
};
use rustok_index::MutationApplyOutcome;
use rustok_social_graph::index_consumer::{
    SOCIAL_GRAPH_INDEX_CONSUMER_GROUP, SOCIAL_GRAPH_INDEX_DLQ_RECEIPT_RECOVERED_CODE,
    SocialGraphIndexConsumer, SocialGraphIndexDlqPublishOutcome, SocialGraphIndexProcessOutcome,
};
use rustok_telemetry::runtime_consumer_metrics;
use tokio::task::JoinHandle;
use uuid::Uuid;

use crate::common::settings::EventDeliveryProfile;
use crate::error::{Error, Result};
use crate::services::app_lifecycle::StopHandle;
use crate::services::event_transport_factory::EventRuntime;
use crate::services::server_runtime_context::ServerRuntimeContext;

const ENABLE_ENV: &str = "RUSTOK_SOCIAL_GRAPH_INDEX_CONSUMER_ENABLED";
const IDLE_POLL_ENV: &str = "RUSTOK_SOCIAL_GRAPH_INDEX_CONSUMER_IDLE_POLL_MS";
const DEFAULT_IDLE_POLL_MS: u64 = 500;
const MAX_IDLE_POLL_MS: u64 = 60_000;
const RAW_POISON_PUBLISH_LEASE: Duration = Duration::from_secs(30);
const METRICS_CONSUMER: &str = "social_graph_index";
const STAGE_STARTUP: &str = "startup";
const STAGE_RECEIVE: &str = "receive";
const STAGE_PROJECTION: &str = "projection";
const STAGE_DLQ_PUBLISH: &str = "dlq_publish";
const STAGE_POISON_RECEIPT: &str = "poison_receipt";
const STAGE_ACKNOWLEDGEMENT: &str = "acknowledgement";
const DLQ_RECEIPT_IN_PROGRESS_CODE: &str = "social_graph.index.dlq_publish_in_progress";
const POISON_CLAIM_BUSY_CODE: &str = "iggy.connector.poison_claim_busy";
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
        let max_backoff = Duration::from_millis(settings.events.relay_retry_policy.max_backoff_ms);
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

pub async fn start_social_graph_index_worker_if_enabled(ctx: &ServerRuntimeContext) -> Result<()> {
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
    let consumer =
        match SocialGraphIndexConsumer::open(Arc::clone(&transport), ctx.db_clone()).await {
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
    let poison_receipts = ConsumerPoisonReceiptStore::new(ctx.db_clone());
    let poison_publisher_id = Uuid::new_v4();

    let instance_id = SOCIAL_GRAPH_INDEX_WORKER_INSTANCE_IDS.fetch_add(1, Ordering::Relaxed);
    runtime_consumer_metrics::record_worker_start(METRICS_CONSUMER);
    tracing::info!(
        worker = METRICS_CONSUMER,
        instance_id,
        consumer_group = SOCIAL_GRAPH_INDEX_CONSUMER_GROUP,
        dlq_enabled = config.dlq_enabled,
        max_attempts = config.max_attempts,
        "Starting Social Graph Index consumer worker"
    );
    ctx.shared_insert(SocialGraphIndexWorkerHandle {
        instance_id,
        _handle: tokio::spawn(social_graph_index_worker_loop(
            consumer,
            transport,
            poison_receipts,
            poison_publisher_id,
            config,
            stop_rx,
        )),
    });
    Ok(())
}

async fn social_graph_index_worker_loop(
    consumer: SocialGraphIndexConsumer,
    transport: Arc<IggyTransport>,
    poison_receipts: ConsumerPoisonReceiptStore,
    poison_publisher_id: Uuid,
    config: SocialGraphIndexWorkerConfig,
    mut stop_rx: tokio::sync::watch::Receiver<bool>,
) {
    loop {
        if *stop_rx.borrow() {
            record_worker_termination("shutdown");
            tracing::info!(worker = METRICS_CONSUMER, "Worker received shutdown signal");
            return;
        }

        let received = tokio::select! {
            result = consumer.receive_delivery() => Some(result),
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
            Ok(Some(PersistentContractDelivery::Event(consumed))) => {
                let consumed = *consumed;
                if !handle_event_delivery(&consumer, &config, &mut stop_rx, consumed).await {
                    return;
                }
            }
            Ok(Some(PersistentContractDelivery::DecodeFailure(failure))) => {
                if !handle_decode_failure(
                    &consumer,
                    &transport,
                    &poison_receipts,
                    poison_publisher_id,
                    &config,
                    &mut stop_rx,
                    *failure,
                )
                .await
                {
                    return;
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

async fn handle_event_delivery(
    consumer: &SocialGraphIndexConsumer,
    config: &SocialGraphIndexWorkerConfig,
    stop_rx: &mut tokio::sync::watch::Receiver<bool>,
    consumed: ConsumedContractEvent,
) -> bool {
    let delivery_started = Instant::now();
    runtime_consumer_metrics::begin_delivery(METRICS_CONSUMER, consumed.offset());
    match process_delivery(consumer, config, stop_rx, &consumed).await {
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
            true
        }
        Ok(DeliveryCompletion::Stopped) => {
            record_worker_termination("shutdown_in_flight");
            tracing::info!(
                worker = METRICS_CONSUMER,
                event_id = %consumed.envelope.id(),
                "Worker stopped with broker offset uncommitted"
            );
            false
        }
        Err(error) => {
            record_worker_termination("delivery_failure");
            tracing::error!(
                worker = METRICS_CONSUMER,
                event_id = %consumed.envelope.id(),
                error = %error,
                "Social Graph Index worker terminated with broker offset uncommitted"
            );
            false
        }
    }
}

async fn handle_decode_failure(
    consumer: &SocialGraphIndexConsumer,
    transport: &Arc<IggyTransport>,
    poison_receipts: &ConsumerPoisonReceiptStore,
    poison_publisher_id: Uuid,
    config: &SocialGraphIndexWorkerConfig,
    stop_rx: &mut tokio::sync::watch::Receiver<bool>,
    failure: ConsumedContractDecodeFailure,
) -> bool {
    let delivery_started = Instant::now();
    runtime_consumer_metrics::begin_delivery(METRICS_CONSUMER, Some(failure.offset()));
    match process_decode_failure(
        consumer,
        transport,
        poison_receipts,
        poison_publisher_id,
        config,
        stop_rx,
        &failure,
    )
    .await
    {
        Ok(RawPoisonCompletion::Completed { recovered }) => {
            runtime_consumer_metrics::complete_delivery(
                METRICS_CONSUMER,
                if recovered {
                    "decode_dead_letter_recovered"
                } else {
                    "decode_dead_lettered"
                },
                delivery_started.elapsed(),
                Some(failure.offset()),
            );
            tracing::warn!(
                worker = METRICS_CONSUMER,
                error_code = failure.stable_error_code(),
                recovered,
                "Undecodable Social Graph Index delivery reached a durable neutral result and was acknowledged"
            );
            true
        }
        Ok(RawPoisonCompletion::Stopped) => {
            record_worker_termination("shutdown_in_flight");
            tracing::info!(
                worker = METRICS_CONSUMER,
                error_code = failure.stable_error_code(),
                "Worker stopped with undecodable broker offset uncommitted"
            );
            false
        }
        Err(error) => {
            record_worker_termination("decode_failure_terminalization_failed");
            tracing::error!(
                worker = METRICS_CONSUMER,
                error_code = failure.stable_error_code(),
                error = %error,
                "Social Graph Index worker terminated with undecodable broker offset uncommitted"
            );
            false
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

#[derive(Debug)]
enum DlqPublishCompletion {
    Published(SocialGraphIndexDlqPublishOutcome),
    Stopped,
}

#[derive(Debug)]
enum RawPoisonCompletion {
    Completed { recovered: bool },
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
                    runtime_consumer_metrics::record_retry(METRICS_CONSUMER, STAGE_PROJECTION);
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

                let continuing_durable_receipt = error_code == DLQ_RECEIPT_IN_PROGRESS_CODE;
                if config.dlq_enabled || continuing_durable_receipt {
                    let publish_outcome = match publish_dead_lettered_result(
                        consumer, config, stop_rx, consumed, error_code, attempt,
                    )
                    .await?
                    {
                        DlqPublishCompletion::Published(outcome) => outcome,
                        DlqPublishCompletion::Stopped => return Ok(DeliveryCompletion::Stopped),
                    };
                    let terminal_error_code = if continuing_durable_receipt
                        || matches!(
                            publish_outcome,
                            SocialGraphIndexDlqPublishOutcome::PreviouslyPublished
                        ) {
                        SOCIAL_GRAPH_INDEX_DLQ_RECEIPT_RECOVERED_CODE
                    } else {
                        error_code
                    };
                    tracing::warn!(
                        worker = METRICS_CONSUMER,
                        event_id = %consumed.envelope.id(),
                        error_code = terminal_error_code,
                        retryable,
                        projection_attempts = attempt,
                        publish_outcome = ?publish_outcome,
                        "Social Graph Index poison delivery has a durable DLQ receipt; acknowledging source offset"
                    );
                    return acknowledge_terminal_result(
                        consumer,
                        config,
                        stop_rx,
                        consumed,
                        SocialGraphIndexProcessOutcome::DeadLettered {
                            error_code: terminal_error_code,
                        },
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

async fn publish_dead_lettered_result(
    consumer: &SocialGraphIndexConsumer,
    config: &SocialGraphIndexWorkerConfig,
    stop_rx: &mut tokio::sync::watch::Receiver<bool>,
    consumed: &ConsumedContractEvent,
    stable_error_code: &'static str,
    projection_attempt_count: u32,
) -> std::result::Result<DlqPublishCompletion, String> {
    let mut attempt = 1;
    loop {
        match consumer
            .publish_consumed_to_dlq(consumed, stable_error_code, projection_attempt_count)
            .await
        {
            Ok(outcome) => {
                let result = match outcome {
                    SocialGraphIndexDlqPublishOutcome::Published => "published",
                    SocialGraphIndexDlqPublishOutcome::PreviouslyPublished => "already_published",
                };
                runtime_consumer_metrics::record_dlq(METRICS_CONSUMER, result);
                return Ok(DlqPublishCompletion::Published(outcome));
            }
            Err(error) if error.is_retryable() && attempt < config.max_attempts => {
                let delay = retry_delay(config, attempt);
                runtime_consumer_metrics::record_failure(
                    METRICS_CONSUMER,
                    STAGE_DLQ_PUBLISH,
                    error.stable_code(),
                );
                runtime_consumer_metrics::record_retry(METRICS_CONSUMER, STAGE_DLQ_PUBLISH);
                tracing::warn!(
                    worker = METRICS_CONSUMER,
                    event_id = %consumed.envelope.id(),
                    error_code = error.stable_code(),
                    attempt,
                    retry_delay_ms = duration_millis(delay),
                    "Durable DLQ receipt publication is retryable; retaining the source delivery"
                );
                if wait_or_stop(delay, stop_rx).await {
                    return Ok(DlqPublishCompletion::Stopped);
                }
                attempt += 1;
            }
            Err(error) => {
                runtime_consumer_metrics::record_failure(
                    METRICS_CONSUMER,
                    STAGE_DLQ_PUBLISH,
                    error.stable_code(),
                );
                runtime_consumer_metrics::record_dlq(METRICS_CONSUMER, "failure");
                return Err(format!(
                    "durable DLQ receipt publication failed after {attempt} attempt(s) [{}]",
                    error.stable_code()
                ));
            }
        }
    }
}

async fn process_decode_failure(
    consumer: &SocialGraphIndexConsumer,
    transport: &Arc<IggyTransport>,
    poison_receipts: &ConsumerPoisonReceiptStore,
    poison_publisher_id: Uuid,
    config: &SocialGraphIndexWorkerConfig,
    stop_rx: &mut tokio::sync::watch::Receiver<bool>,
    failure: &ConsumedContractDecodeFailure,
) -> std::result::Result<RawPoisonCompletion, String> {
    let identity = ConsumerPoisonIdentity::new(
        failure.delivery_id(),
        SOCIAL_GRAPH_INDEX_CONSUMER_GROUP,
        failure.stream(),
        failure.topic(),
        failure.partition(),
        failure.offset(),
        failure.raw_payload().to_vec(),
    )
    .map_err(|error| format!("neutral poison identity rejected [{}]", error.stable_code()))?;

    let mut lookup_attempt = 1;
    let continuing_durable_receipt = loop {
        match poison_receipts.find(&identity).await {
            Ok(existing) => break existing.is_some(),
            Err(error) if error.is_retryable() && lookup_attempt < config.max_attempts => {
                let delay = retry_delay(config, lookup_attempt);
                runtime_consumer_metrics::record_failure(
                    METRICS_CONSUMER,
                    STAGE_POISON_RECEIPT,
                    error.stable_code(),
                );
                runtime_consumer_metrics::record_retry(METRICS_CONSUMER, STAGE_POISON_RECEIPT);
                tracing::warn!(
                    worker = METRICS_CONSUMER,
                    error_code = error.stable_code(),
                    attempt = lookup_attempt,
                    retry_delay_ms = duration_millis(delay),
                    "Neutral poison receipt lookup failed; retaining source offset"
                );
                if wait_or_stop(delay, stop_rx).await {
                    return Ok(RawPoisonCompletion::Stopped);
                }
                lookup_attempt += 1;
            }
            Err(error) => {
                runtime_consumer_metrics::record_failure(
                    METRICS_CONSUMER,
                    STAGE_POISON_RECEIPT,
                    error.stable_code(),
                );
                return Err(format!(
                    "neutral poison receipt lookup failed after {lookup_attempt} attempt(s) [{}]",
                    error.stable_code()
                ));
            }
        }
    };

    if !config.dlq_enabled && !continuing_durable_receipt {
        runtime_consumer_metrics::record_failure(
            METRICS_CONSUMER,
            STAGE_DLQ_PUBLISH,
            failure.stable_error_code(),
        );
        return Err(format!(
            "undecodable contract delivery [{}] cannot choose a new terminal result while DLQ is disabled",
            failure.stable_error_code()
        ));
    }

    let mut attempt = 1;
    let recovered = loop {
        match poison_receipts
            .reserve_and_claim(
                &identity,
                failure.stable_error_code(),
                1,
                poison_publisher_id,
                RAW_POISON_PUBLISH_LEASE,
            )
            .await
        {
            Ok(ConsumerPoisonPublishClaim::AlreadyPublished)
            | Ok(ConsumerPoisonPublishClaim::AlreadyAcknowledged) => {
                runtime_consumer_metrics::record_dlq(METRICS_CONSUMER, "already_published");
                break true;
            }
            Ok(ConsumerPoisonPublishClaim::Busy) if attempt < config.max_attempts => {
                let delay = retry_delay(config, attempt);
                runtime_consumer_metrics::record_failure(
                    METRICS_CONSUMER,
                    STAGE_POISON_RECEIPT,
                    POISON_CLAIM_BUSY_CODE,
                );
                runtime_consumer_metrics::record_retry(METRICS_CONSUMER, STAGE_POISON_RECEIPT);
                tracing::warn!(
                    worker = METRICS_CONSUMER,
                    error_code = failure.stable_error_code(),
                    attempt,
                    retry_delay_ms = duration_millis(delay),
                    "Neutral poison receipt is owned by another publisher; retaining the source delivery"
                );
                if wait_or_stop(delay, stop_rx).await {
                    return Ok(RawPoisonCompletion::Stopped);
                }
                attempt += 1;
            }
            Ok(ConsumerPoisonPublishClaim::Busy) => {
                return Err(format!(
                    "neutral poison receipt remained busy after {attempt} attempt(s)"
                ));
            }
            Ok(ConsumerPoisonPublishClaim::Claimed) => {
                match transport.move_to_dlq(failure.to_dlq_entry(1)).await {
                    Ok(()) => {
                        runtime_consumer_metrics::record_dlq(METRICS_CONSUMER, "published");
                        mark_raw_poison_published(
                            poison_receipts,
                            &identity,
                            poison_publisher_id,
                            config,
                            stop_rx,
                        )
                        .await?;
                        break false;
                    }
                    Err(error) if attempt < config.max_attempts => {
                        let _ = poison_receipts
                            .release_claim(&identity, poison_publisher_id)
                            .await;
                        let delay = retry_delay(config, attempt);
                        runtime_consumer_metrics::record_failure(
                            METRICS_CONSUMER,
                            STAGE_DLQ_PUBLISH,
                            "social_graph.index.transport_unavailable",
                        );
                        runtime_consumer_metrics::record_retry(METRICS_CONSUMER, STAGE_DLQ_PUBLISH);
                        tracing::warn!(
                            worker = METRICS_CONSUMER,
                            error_code = failure.stable_error_code(),
                            attempt,
                            retry_delay_ms = duration_millis(delay),
                            error = %error,
                            "Exact-byte raw poison publication failed; released claim and retained source offset"
                        );
                        if wait_or_stop(delay, stop_rx).await {
                            return Ok(RawPoisonCompletion::Stopped);
                        }
                        attempt += 1;
                    }
                    Err(error) => {
                        let _ = poison_receipts
                            .release_claim(&identity, poison_publisher_id)
                            .await;
                        runtime_consumer_metrics::record_failure(
                            METRICS_CONSUMER,
                            STAGE_DLQ_PUBLISH,
                            "social_graph.index.transport_unavailable",
                        );
                        runtime_consumer_metrics::record_dlq(METRICS_CONSUMER, "failure");
                        return Err(format!(
                            "exact-byte raw poison publication failed after {attempt} attempt(s): {error}"
                        ));
                    }
                }
            }
            Err(error) if error.is_retryable() && attempt < config.max_attempts => {
                let delay = retry_delay(config, attempt);
                runtime_consumer_metrics::record_failure(
                    METRICS_CONSUMER,
                    STAGE_POISON_RECEIPT,
                    error.stable_code(),
                );
                runtime_consumer_metrics::record_retry(METRICS_CONSUMER, STAGE_POISON_RECEIPT);
                tracing::warn!(
                    worker = METRICS_CONSUMER,
                    error_code = error.stable_code(),
                    attempt,
                    retry_delay_ms = duration_millis(delay),
                    "Neutral poison receipt persistence failed; retaining source offset"
                );
                if wait_or_stop(delay, stop_rx).await {
                    return Ok(RawPoisonCompletion::Stopped);
                }
                attempt += 1;
            }
            Err(error) => {
                runtime_consumer_metrics::record_failure(
                    METRICS_CONSUMER,
                    STAGE_POISON_RECEIPT,
                    error.stable_code(),
                );
                return Err(format!(
                    "neutral poison receipt failed after {attempt} attempt(s) [{}]",
                    error.stable_code()
                ));
            }
        }
    };

    acknowledge_raw_poison_result(
        consumer,
        poison_receipts,
        &identity,
        config,
        stop_rx,
        failure,
        recovered,
    )
    .await
}

async fn mark_raw_poison_published(
    poison_receipts: &ConsumerPoisonReceiptStore,
    identity: &ConsumerPoisonIdentity,
    poison_publisher_id: Uuid,
    config: &SocialGraphIndexWorkerConfig,
    stop_rx: &mut tokio::sync::watch::Receiver<bool>,
) -> std::result::Result<(), String> {
    let mut attempt = 1;
    loop {
        match poison_receipts
            .mark_published(identity, poison_publisher_id)
            .await
        {
            Ok(()) => return Ok(()),
            Err(error) if error.is_retryable() && attempt < config.max_attempts => {
                let delay = retry_delay(config, attempt);
                runtime_consumer_metrics::record_failure(
                    METRICS_CONSUMER,
                    STAGE_POISON_RECEIPT,
                    error.stable_code(),
                );
                runtime_consumer_metrics::record_retry(METRICS_CONSUMER, STAGE_POISON_RECEIPT);
                tracing::warn!(
                    worker = METRICS_CONSUMER,
                    error_code = error.stable_code(),
                    attempt,
                    retry_delay_ms = duration_millis(delay),
                    "Raw poison bytes were published but durable published state failed; retrying persistence only"
                );
                if wait_or_stop(delay, stop_rx).await {
                    return Err(
                        "worker stopped after raw poison publication with source offset uncommitted"
                            .to_string(),
                    );
                }
                attempt += 1;
            }
            Err(error) => {
                runtime_consumer_metrics::record_failure(
                    METRICS_CONSUMER,
                    STAGE_POISON_RECEIPT,
                    error.stable_code(),
                );
                return Err(format!(
                    "raw poison publication succeeded but durable published state failed after {attempt} attempt(s) [{}]",
                    error.stable_code()
                ));
            }
        }
    }
}

async fn acknowledge_raw_poison_result(
    consumer: &SocialGraphIndexConsumer,
    poison_receipts: &ConsumerPoisonReceiptStore,
    identity: &ConsumerPoisonIdentity,
    config: &SocialGraphIndexWorkerConfig,
    stop_rx: &mut tokio::sync::watch::Receiver<bool>,
    failure: &ConsumedContractDecodeFailure,
    recovered: bool,
) -> std::result::Result<RawPoisonCompletion, String> {
    let mut attempt = 1;
    loop {
        match consumer.acknowledge_decode_failure(failure).await {
            Ok(()) => {
                if let Err(error) = poison_receipts.mark_acknowledged(identity).await {
                    runtime_consumer_metrics::record_failure(
                        METRICS_CONSUMER,
                        STAGE_POISON_RECEIPT,
                        error.stable_code(),
                    );
                    tracing::warn!(
                        worker = METRICS_CONSUMER,
                        error_code = error.stable_code(),
                        "Raw poison source offset committed but receipt acknowledgement bookkeeping failed"
                    );
                }
                return Ok(RawPoisonCompletion::Completed { recovered });
            }
            Err(error) if attempt < config.max_attempts => {
                let delay = retry_delay(config, attempt);
                runtime_consumer_metrics::record_failure(
                    METRICS_CONSUMER,
                    STAGE_ACKNOWLEDGEMENT,
                    error.stable_code(),
                );
                runtime_consumer_metrics::record_retry(METRICS_CONSUMER, STAGE_ACKNOWLEDGEMENT);
                tracing::warn!(
                    worker = METRICS_CONSUMER,
                    error_code = error.stable_code(),
                    attempt,
                    retry_delay_ms = duration_millis(delay),
                    "Durable neutral poison result exists but broker acknowledgement failed; retrying acknowledgement only"
                );
                if wait_or_stop(delay, stop_rx).await {
                    return Ok(RawPoisonCompletion::Stopped);
                }
                attempt += 1;
            }
            Err(error) => {
                runtime_consumer_metrics::record_failure(
                    METRICS_CONSUMER,
                    STAGE_ACKNOWLEDGEMENT,
                    error.stable_code(),
                );
                return Err(format!(
                    "raw poison broker acknowledgement failed after {attempt} attempt(s) [{}]; durable neutral receipt remains published and redelivery retries acknowledgement only",
                    error.stable_code()
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
                runtime_consumer_metrics::record_retry(METRICS_CONSUMER, STAGE_ACKNOWLEDGEMENT);
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
                        "durable DLQ receipt remains published; redelivery skips projection and DLQ publication and retries source acknowledgement only"
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
        SocialGraphIndexProcessOutcome::Projected(MutationApplyOutcome::StaleIgnored {
            ..
        }) => "stale_ignored",
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

async fn wait_or_stop(delay: Duration, stop_rx: &mut tokio::sync::watch::Receiver<bool>) -> bool {
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
        Err(error) => Err(Error::Message(format!("failed to read {name}: {error}"))),
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
