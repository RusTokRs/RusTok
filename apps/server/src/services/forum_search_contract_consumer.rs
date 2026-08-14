use std::env;
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::Duration;

use rustok_iggy::{
    ConsumedContractDecodeFailure, ConsumedContractEvent, ContractDecodeFailureKind, DlqEntry,
    IggyTransport, PersistentContractConsumerGroup, PersistentContractDelivery,
};
use rustok_iggy_connector::migrations::{
    ConsumerPoisonIdentity, ConsumerPoisonPublishClaim, ConsumerPoisonReceiptStore,
};
use rustok_search::{
    FORUM_SEARCH_CONTRACT_CONSUMER_GROUP, FORUM_SEARCH_CONTRACT_TOPIC, ForumSearchContractIngress,
    ForumSearchContractIngressOutcome,
};
use tokio::task::JoinHandle;
use uuid::Uuid;

use crate::common::settings::EventDeliveryProfile;
use crate::error::{Error, Result};
use crate::services::app_lifecycle::StopHandle;
use crate::services::event_transport_factory::EventRuntime;
use crate::services::server_runtime_context::ServerRuntimeContext;

const ENABLE_ENV: &str = "RUSTOK_FORUM_SEARCH_CONTRACT_CONSUMER_ENABLED";
const IDLE_POLL_ENV: &str = "RUSTOK_FORUM_SEARCH_CONTRACT_CONSUMER_IDLE_POLL_MS";
const DEFAULT_IDLE_POLL_MS: u64 = 500;
const MAX_IDLE_POLL_MS: u64 = 60_000;
const POISON_PUBLISH_LEASE: Duration = Duration::from_secs(30);
const POISON_CLAIM_BUSY_CODE: &str = "iggy.connector.poison_claim_busy";
static FORUM_SEARCH_CONTRACT_WORKER_INSTANCE_IDS: AtomicU64 = AtomicU64::new(1);

#[derive(Clone, Debug)]
struct ForumSearchContractWorkerConfig {
    max_attempts: u32,
    base_backoff: Duration,
    max_backoff: Duration,
    idle_poll: Duration,
    dlq_enabled: bool,
}

impl ForumSearchContractWorkerConfig {
    fn from_context(ctx: &ServerRuntimeContext) -> Result<Self> {
        let settings = ctx.settings();
        let configured_attempts = if settings.events.dlq.enabled {
            settings.events.dlq.max_attempts
        } else {
            settings.events.relay_retry_policy.max_attempts
        };
        let max_attempts = u32::try_from(configured_attempts).map_err(|_| {
            Error::Message(
                "Forum Search contract consumer max attempts must fit a positive u32".to_string(),
            )
        })?;
        if max_attempts == 0 {
            return Err(Error::Message(
                "Forum Search contract consumer max attempts must be greater than zero".to_string(),
            ));
        }

        let base_backoff =
            Duration::from_millis(settings.events.relay_retry_policy.base_backoff_ms);
        let max_backoff = Duration::from_millis(settings.events.relay_retry_policy.max_backoff_ms);
        if base_backoff.is_zero() || max_backoff < base_backoff {
            return Err(Error::Message(
                "Forum Search contract consumer retry backoff must be positive and bounded"
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

pub struct ForumSearchContractConsumerWorkerHandle {
    instance_id: u64,
    _handle: JoinHandle<()>,
}

impl ForumSearchContractConsumerWorkerHandle {
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

pub fn forum_search_contract_consumer_enabled() -> Result<bool> {
    match env::var(ENABLE_ENV) {
        Ok(value) => parse_bool(ENABLE_ENV, &value).map_err(Error::Message),
        Err(env::VarError::NotPresent) => Ok(false),
        Err(error) => Err(Error::Message(format!(
            "failed to read {ENABLE_ENV}: {error}"
        ))),
    }
}

pub async fn start_forum_search_contract_consumer_if_enabled(
    ctx: &ServerRuntimeContext,
) -> Result<()> {
    if !ctx.settings().runtime.runs_background_workers()
        || ctx.shared_contains::<ForumSearchContractConsumerWorkerHandle>()
    {
        return Ok(());
    }
    if !forum_search_contract_consumer_enabled()? {
        tracing::info!(
            env = ENABLE_ENV,
            "Forum Search typed invalidation consumer disabled by default-off runtime flag"
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
    let transport = ctx.shared_get::<Arc<IggyTransport>>().ok_or_else(|| {
        Error::Message(
            "outbox_iggy runtime did not publish its configured Iggy transport".to_string(),
        )
    })?;
    let ingress = ForumSearchContractIngress::new(ctx.db_clone());
    if !ingress.supports_persistent_ingress() {
        return Err(Error::Message(
            "Forum Search typed invalidation consumer requires PostgreSQL".to_string(),
        ));
    }
    let group = transport
        .open_persistent_contract_consumer_group(
            FORUM_SEARCH_CONTRACT_CONSUMER_GROUP,
            FORUM_SEARCH_CONTRACT_TOPIC,
        )
        .await
        .map_err(|error| {
            Error::Message(format!(
                "Forum Search typed invalidation consumer startup failed: {error}"
            ))
        })?;

    if !ctx.shared_contains::<StopHandle>() {
        let (stop_handle, _stop_rx) = StopHandle::new();
        ctx.shared_insert(stop_handle);
    }
    let stop_rx = ctx
        .shared_get::<StopHandle>()
        .expect("StopHandle must be registered before Forum Search contract consumer startup")
        .subscribe();
    let config = ForumSearchContractWorkerConfig::from_context(ctx)?;
    let poison_receipts = ConsumerPoisonReceiptStore::new(ctx.db_clone());
    let poison_publisher_id = Uuid::new_v4();

    let instance_id = FORUM_SEARCH_CONTRACT_WORKER_INSTANCE_IDS.fetch_add(1, Ordering::Relaxed);
    tracing::info!(
        instance_id,
        consumer_group = FORUM_SEARCH_CONTRACT_CONSUMER_GROUP,
        topic = FORUM_SEARCH_CONTRACT_TOPIC,
        dlq_enabled = config.dlq_enabled,
        max_attempts = config.max_attempts,
        "Starting Forum Search typed invalidation consumer"
    );
    ctx.shared_insert(ForumSearchContractConsumerWorkerHandle {
        instance_id,
        _handle: tokio::spawn(forum_search_contract_consumer_loop(
            group,
            ingress,
            transport,
            poison_receipts,
            poison_publisher_id,
            config,
            stop_rx,
        )),
    });
    Ok(())
}

async fn forum_search_contract_consumer_loop(
    group: PersistentContractConsumerGroup,
    ingress: ForumSearchContractIngress,
    transport: Arc<IggyTransport>,
    poison_receipts: ConsumerPoisonReceiptStore,
    poison_publisher_id: Uuid,
    config: ForumSearchContractWorkerConfig,
    mut stop_rx: tokio::sync::watch::Receiver<bool>,
) {
    loop {
        if *stop_rx.borrow() {
            tracing::info!("Forum Search typed invalidation consumer stopped");
            return;
        }

        let received = tokio::select! {
            result = group.receive_delivery() => Some(result),
            changed = stop_rx.changed() => {
                if changed.is_err() || *stop_rx.borrow() {
                    None
                } else {
                    continue;
                }
            }
        };
        let Some(received) = received else {
            tracing::info!("Forum Search typed invalidation consumer stopped before receive");
            return;
        };

        let keep_running = match received {
            Ok(Some(PersistentContractDelivery::Event(consumed))) => {
                let consumed = *consumed;
                process_contract_event(
                    &group,
                    &ingress,
                    &transport,
                    &poison_receipts,
                    poison_publisher_id,
                    &config,
                    &mut stop_rx,
                    consumed,
                )
                .await
            }
            Ok(Some(PersistentContractDelivery::DecodeFailure(failure))) => {
                process_decode_failure(
                    &group,
                    &transport,
                    &poison_receipts,
                    poison_publisher_id,
                    &config,
                    &mut stop_rx,
                    *failure,
                )
                .await
            }
            Ok(None) => !wait_or_stop(config.idle_poll, &mut stop_rx).await,
            Err(error) => {
                tracing::error!(
                    error = %error,
                    "Forum Search typed invalidation receive failed; broker offset remains uncommitted"
                );
                false
            }
        };
        if !keep_running {
            return;
        }
    }
}

#[allow(clippy::too_many_arguments)]
async fn process_contract_event(
    group: &PersistentContractConsumerGroup,
    ingress: &ForumSearchContractIngress,
    transport: &Arc<IggyTransport>,
    poison_receipts: &ConsumerPoisonReceiptStore,
    poison_publisher_id: Uuid,
    config: &ForumSearchContractWorkerConfig,
    stop_rx: &mut tokio::sync::watch::Receiver<bool>,
    consumed: ConsumedContractEvent,
) -> bool {
    let mut attempt = 1;
    loop {
        match ingress.ingest(&consumed.envelope).await {
            Ok(outcome) => {
                if !acknowledge_event(group, config, stop_rx, &consumed).await {
                    return false;
                }
                match outcome {
                    ForumSearchContractIngressOutcome::DurablyAccepted {
                        root_event_id,
                        owner_revision,
                    } => tracing::debug!(
                        typed_envelope_id = %consumed.envelope.id(),
                        root_event_id = %root_event_id,
                        owner_revision,
                        "Forum Search typed invalidation reached the shared durable inbox"
                    ),
                    ForumSearchContractIngressOutcome::IgnoredUnrelated { event_type } => {
                        tracing::trace!(
                            event_type,
                            "Forum Search typed consumer acknowledged an unrelated sealed event"
                        )
                    }
                }
                return true;
            }
            Err(error) if error.is_retryable() && attempt < config.max_attempts => {
                let delay = retry_delay(config, attempt);
                tracing::warn!(
                    typed_envelope_id = %consumed.envelope.id(),
                    error_code = error.stable_code(),
                    attempt,
                    retry_delay_ms = duration_millis(delay),
                    "Forum Search durable inbox admission failed; retaining broker offset"
                );
                if wait_or_stop(delay, stop_rx).await {
                    return false;
                }
                attempt += 1;
            }
            Err(error) if error.is_retryable() => {
                tracing::error!(
                    typed_envelope_id = %consumed.envelope.id(),
                    error_code = error.stable_code(),
                    attempts = attempt,
                    "Forum Search durable inbox admission exhausted retries; broker offset remains uncommitted"
                );
                return false;
            }
            Err(error) => {
                tracing::warn!(
                    typed_envelope_id = %consumed.envelope.id(),
                    error_code = error.stable_code(),
                    "Forum Search typed invalidation is semantic poison"
                );
                return terminalize_semantic_poison(
                    group,
                    transport,
                    poison_receipts,
                    poison_publisher_id,
                    config,
                    stop_rx,
                    &consumed,
                    error.stable_code(),
                    attempt,
                )
                .await;
            }
        }
    }
}

#[allow(clippy::too_many_arguments)]
async fn terminalize_semantic_poison(
    group: &PersistentContractConsumerGroup,
    transport: &Arc<IggyTransport>,
    poison_receipts: &ConsumerPoisonReceiptStore,
    poison_publisher_id: Uuid,
    config: &ForumSearchContractWorkerConfig,
    stop_rx: &mut tokio::sync::watch::Receiver<bool>,
    consumed: &ConsumedContractEvent,
    stable_error_code: &'static str,
    observed_attempts: u32,
) -> bool {
    let Some((identity, entry)) = semantic_poison_descriptor(
        consumed,
        stable_error_code,
        observed_attempts,
    )
    .map_err(|error| {
        tracing::error!(
            typed_envelope_id = %consumed.envelope.id(),
            error = %error,
            "Forum Search semantic poison identity failed; broker offset remains uncommitted"
        );
    })
    .ok() else {
        return false;
    };

    if establish_poison_result(
        transport,
        poison_receipts,
        poison_publisher_id,
        config,
        stop_rx,
        &identity,
        entry,
        stable_error_code,
        observed_attempts,
    )
    .await
    .is_err()
    {
        return false;
    }
    acknowledge_event_with_receipt(group, poison_receipts, &identity, config, stop_rx, consumed)
        .await
}

#[allow(clippy::too_many_arguments)]
async fn process_decode_failure(
    group: &PersistentContractConsumerGroup,
    transport: &Arc<IggyTransport>,
    poison_receipts: &ConsumerPoisonReceiptStore,
    poison_publisher_id: Uuid,
    config: &ForumSearchContractWorkerConfig,
    stop_rx: &mut tokio::sync::watch::Receiver<bool>,
    failure: ConsumedContractDecodeFailure,
) -> bool {
    let identity = match ConsumerPoisonIdentity::new(
        failure.delivery_id(),
        FORUM_SEARCH_CONTRACT_CONSUMER_GROUP,
        failure.stream(),
        failure.topic(),
        failure.partition(),
        failure.offset(),
        failure.raw_payload().to_vec(),
    ) {
        Ok(identity) => identity,
        Err(error) => {
            tracing::error!(
                error_code = error.stable_code(),
                "Forum Search raw poison identity failed; broker offset remains uncommitted"
            );
            return false;
        }
    };
    let stable_error_code = failure.stable_error_code();
    if establish_poison_result(
        transport,
        poison_receipts,
        poison_publisher_id,
        config,
        stop_rx,
        &identity,
        failure.to_dlq_entry(1),
        stable_error_code,
        1,
    )
    .await
    .is_err()
    {
        return false;
    }
    acknowledge_decode_failure_with_receipt(
        group,
        poison_receipts,
        &identity,
        config,
        stop_rx,
        &failure,
    )
    .await
}

fn semantic_poison_descriptor(
    consumed: &ConsumedContractEvent,
    stable_error_code: &'static str,
    observed_attempts: u32,
) -> std::result::Result<(ConsumerPoisonIdentity, DlqEntry), String> {
    let offset = consumed
        .offset()
        .ok_or_else(|| "validated contract delivery has no connector offset".to_string())?;
    let delivery_identity = ConsumedContractDecodeFailure::new(
        consumed.stream.clone(),
        consumed.topic.clone(),
        consumed.connector_metadata.clone(),
        consumed.raw_payload().to_vec(),
        ContractDecodeFailureKind::SchemaValidation,
    )
    .map_err(|error| error.to_string())?;
    let delivery_id = delivery_identity.delivery_id();
    let identity = ConsumerPoisonIdentity::new(
        delivery_id,
        FORUM_SEARCH_CONTRACT_CONSUMER_GROUP,
        &consumed.stream,
        &consumed.topic,
        consumed.partition,
        offset,
        consumed.raw_payload().to_vec(),
    )
    .map_err(|error| error.to_string())?;
    let entry = DlqEntry::new(
        delivery_id,
        consumed.topic.clone(),
        consumed.raw_payload().to_vec(),
        stable_error_code,
        observed_attempts,
    )
    .with_connector_metadata(consumed.connector_metadata.clone())
    .with_broker_message_id(delivery_id);
    Ok((identity, entry))
}

#[allow(clippy::too_many_arguments)]
async fn establish_poison_result(
    transport: &Arc<IggyTransport>,
    poison_receipts: &ConsumerPoisonReceiptStore,
    poison_publisher_id: Uuid,
    config: &ForumSearchContractWorkerConfig,
    stop_rx: &mut tokio::sync::watch::Receiver<bool>,
    identity: &ConsumerPoisonIdentity,
    entry: DlqEntry,
    stable_error_code: &'static str,
    observed_attempts: u32,
) -> std::result::Result<(), ()> {
    let continuing_receipt =
        match lookup_poison_receipt(poison_receipts, identity, config, stop_rx).await {
            Ok(value) => value,
            Err(()) => return Err(()),
        };
    if !config.dlq_enabled && !continuing_receipt {
        tracing::error!(
            error_code = stable_error_code,
            "Forum Search poison cannot choose a new terminal result while DLQ is disabled; broker offset remains uncommitted"
        );
        return Err(());
    }

    let mut attempt = 1;
    loop {
        match poison_receipts
            .reserve_and_claim(
                identity,
                stable_error_code,
                observed_attempts.max(1),
                poison_publisher_id,
                POISON_PUBLISH_LEASE,
            )
            .await
        {
            Ok(ConsumerPoisonPublishClaim::AlreadyPublished)
            | Ok(ConsumerPoisonPublishClaim::AlreadyAcknowledged) => return Ok(()),
            Ok(ConsumerPoisonPublishClaim::Busy) if attempt < config.max_attempts => {
                let delay = retry_delay(config, attempt);
                tracing::warn!(
                    error_code = POISON_CLAIM_BUSY_CODE,
                    attempt,
                    retry_delay_ms = duration_millis(delay),
                    "Forum Search poison receipt is owned by another publisher"
                );
                if wait_or_stop(delay, stop_rx).await {
                    return Err(());
                }
                attempt += 1;
            }
            Ok(ConsumerPoisonPublishClaim::Busy) => {
                tracing::error!(
                    error_code = POISON_CLAIM_BUSY_CODE,
                    attempts = attempt,
                    "Forum Search poison receipt remained busy; broker offset remains uncommitted"
                );
                return Err(());
            }
            Ok(ConsumerPoisonPublishClaim::Claimed) => {
                match transport.move_to_dlq(entry.clone()).await {
                    Ok(()) => {
                        return mark_poison_published(
                            poison_receipts,
                            identity,
                            poison_publisher_id,
                            config,
                            stop_rx,
                        )
                        .await;
                    }
                    Err(error) if attempt < config.max_attempts => {
                        let _ = poison_receipts
                            .release_claim(identity, poison_publisher_id)
                            .await;
                        let delay = retry_delay(config, attempt);
                        tracing::warn!(
                            error = %error,
                            error_code = stable_error_code,
                            attempt,
                            retry_delay_ms = duration_millis(delay),
                            "Forum Search poison DLQ publication failed; released claim and retained broker offset"
                        );
                        if wait_or_stop(delay, stop_rx).await {
                            return Err(());
                        }
                        attempt += 1;
                    }
                    Err(error) => {
                        let _ = poison_receipts
                            .release_claim(identity, poison_publisher_id)
                            .await;
                        tracing::error!(
                            error = %error,
                            error_code = stable_error_code,
                            attempts = attempt,
                            "Forum Search poison DLQ publication failed; broker offset remains uncommitted"
                        );
                        return Err(());
                    }
                }
            }
            Err(error) if error.is_retryable() && attempt < config.max_attempts => {
                let delay = retry_delay(config, attempt);
                tracing::warn!(
                    error_code = error.stable_code(),
                    attempt,
                    retry_delay_ms = duration_millis(delay),
                    "Forum Search poison receipt persistence failed; retaining broker offset"
                );
                if wait_or_stop(delay, stop_rx).await {
                    return Err(());
                }
                attempt += 1;
            }
            Err(error) => {
                tracing::error!(
                    error_code = error.stable_code(),
                    attempts = attempt,
                    "Forum Search poison receipt failed; broker offset remains uncommitted"
                );
                return Err(());
            }
        }
    }
}

async fn lookup_poison_receipt(
    poison_receipts: &ConsumerPoisonReceiptStore,
    identity: &ConsumerPoisonIdentity,
    config: &ForumSearchContractWorkerConfig,
    stop_rx: &mut tokio::sync::watch::Receiver<bool>,
) -> std::result::Result<bool, ()> {
    let mut attempt = 1;
    loop {
        match poison_receipts.find(identity).await {
            Ok(receipt) => return Ok(receipt.is_some()),
            Err(error) if error.is_retryable() && attempt < config.max_attempts => {
                let delay = retry_delay(config, attempt);
                tracing::warn!(
                    error_code = error.stable_code(),
                    attempt,
                    retry_delay_ms = duration_millis(delay),
                    "Forum Search poison receipt lookup failed; retaining broker offset"
                );
                if wait_or_stop(delay, stop_rx).await {
                    return Err(());
                }
                attempt += 1;
            }
            Err(error) => {
                tracing::error!(
                    error_code = error.stable_code(),
                    attempts = attempt,
                    "Forum Search poison receipt lookup failed; broker offset remains uncommitted"
                );
                return Err(());
            }
        }
    }
}

async fn mark_poison_published(
    poison_receipts: &ConsumerPoisonReceiptStore,
    identity: &ConsumerPoisonIdentity,
    poison_publisher_id: Uuid,
    config: &ForumSearchContractWorkerConfig,
    stop_rx: &mut tokio::sync::watch::Receiver<bool>,
) -> std::result::Result<(), ()> {
    let mut attempt = 1;
    loop {
        match poison_receipts
            .mark_published(identity, poison_publisher_id)
            .await
        {
            Ok(()) => return Ok(()),
            Err(error) if error.is_retryable() && attempt < config.max_attempts => {
                let delay = retry_delay(config, attempt);
                tracing::warn!(
                    error_code = error.stable_code(),
                    attempt,
                    retry_delay_ms = duration_millis(delay),
                    "Forum Search poison bytes were published; retrying durable published state only"
                );
                if wait_or_stop(delay, stop_rx).await {
                    return Err(());
                }
                attempt += 1;
            }
            Err(error) => {
                tracing::error!(
                    error_code = error.stable_code(),
                    attempts = attempt,
                    "Forum Search poison publication succeeded but durable state failed; broker offset remains uncommitted"
                );
                return Err(());
            }
        }
    }
}

async fn acknowledge_event(
    group: &PersistentContractConsumerGroup,
    config: &ForumSearchContractWorkerConfig,
    stop_rx: &mut tokio::sync::watch::Receiver<bool>,
    consumed: &ConsumedContractEvent,
) -> bool {
    let mut attempt = 1;
    loop {
        match group.acknowledge(consumed).await {
            Ok(()) => return true,
            Err(error) if attempt < config.max_attempts => {
                let delay = retry_delay(config, attempt);
                tracing::warn!(
                    typed_envelope_id = %consumed.envelope.id(),
                    error = %error,
                    attempt,
                    retry_delay_ms = duration_millis(delay),
                    "Forum Search durable result exists but broker acknowledgement failed"
                );
                if wait_or_stop(delay, stop_rx).await {
                    return false;
                }
                attempt += 1;
            }
            Err(error) => {
                tracing::error!(
                    typed_envelope_id = %consumed.envelope.id(),
                    error = %error,
                    attempts = attempt,
                    "Forum Search broker acknowledgement failed; redelivery will recognize the durable inbox row"
                );
                return false;
            }
        }
    }
}

async fn acknowledge_event_with_receipt(
    group: &PersistentContractConsumerGroup,
    poison_receipts: &ConsumerPoisonReceiptStore,
    identity: &ConsumerPoisonIdentity,
    config: &ForumSearchContractWorkerConfig,
    stop_rx: &mut tokio::sync::watch::Receiver<bool>,
    consumed: &ConsumedContractEvent,
) -> bool {
    if !acknowledge_event(group, config, stop_rx, consumed).await {
        return false;
    }
    if let Err(error) = poison_receipts.mark_acknowledged(identity).await {
        tracing::warn!(
            error_code = error.stable_code(),
            "Forum Search semantic poison offset committed but receipt acknowledgement bookkeeping failed"
        );
    }
    true
}

async fn acknowledge_decode_failure_with_receipt(
    group: &PersistentContractConsumerGroup,
    poison_receipts: &ConsumerPoisonReceiptStore,
    identity: &ConsumerPoisonIdentity,
    config: &ForumSearchContractWorkerConfig,
    stop_rx: &mut tokio::sync::watch::Receiver<bool>,
    failure: &ConsumedContractDecodeFailure,
) -> bool {
    let mut attempt = 1;
    loop {
        match group.acknowledge_decode_failure(failure).await {
            Ok(()) => {
                if let Err(error) = poison_receipts.mark_acknowledged(identity).await {
                    tracing::warn!(
                        error_code = error.stable_code(),
                        "Forum Search raw poison offset committed but receipt acknowledgement bookkeeping failed"
                    );
                }
                return true;
            }
            Err(error) if attempt < config.max_attempts => {
                let delay = retry_delay(config, attempt);
                tracing::warn!(
                    error = %error,
                    error_code = failure.stable_error_code(),
                    attempt,
                    retry_delay_ms = duration_millis(delay),
                    "Forum Search raw poison result exists but broker acknowledgement failed"
                );
                if wait_or_stop(delay, stop_rx).await {
                    return false;
                }
                attempt += 1;
            }
            Err(error) => {
                tracing::error!(
                    error = %error,
                    error_code = failure.stable_error_code(),
                    attempts = attempt,
                    "Forum Search raw poison acknowledgement failed; redelivery will recognize the durable receipt"
                );
                return false;
            }
        }
    }
}

fn retry_delay(config: &ForumSearchContractWorkerConfig, attempt: u32) -> Duration {
    let shift = attempt.saturating_sub(1).min(20);
    let multiplier = 1_u64.checked_shl(shift).unwrap_or(u64::MAX);
    Duration::from_millis(
        duration_millis(config.base_backoff)
            .saturating_mul(multiplier)
            .min(duration_millis(config.max_backoff)),
    )
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
        let config = ForumSearchContractWorkerConfig {
            max_attempts: 10,
            base_backoff: Duration::from_millis(100),
            max_backoff: Duration::from_millis(450),
            idle_poll: Duration::from_millis(500),
            dlq_enabled: true,
        };
        assert_eq!(retry_delay(&config, 1), Duration::from_millis(100));
        assert_eq!(retry_delay(&config, 2), Duration::from_millis(200));
        assert_eq!(retry_delay(&config, 4), Duration::from_millis(450));
    }
}
