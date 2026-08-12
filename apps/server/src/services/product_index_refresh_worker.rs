use std::env;
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::Duration;

use async_trait::async_trait;
use rustok_distribution::product_index::refresh_event::{
    PRODUCT_INDEX_LOCALE_REFRESH_EVENT_DOMAIN, PRODUCT_INDEX_VARIANT_REFRESH_EVENT_DOMAIN,
    ProductIndexRefreshDelivery, ProductIndexRefreshDeliveryWorker,
};
use rustok_events::{
    ContractEventEnvelope, ContractEventPayload, EventContractEnvelopeError,
    ProductIndexRefreshEvent,
};
use rustok_iggy::{
    ConsumedContractEvent, IggyTransport, PersistentContractConsumerGroup,
    PersistentContractDelivery,
};
use rustok_index::{
    IndexMutationAcknowledgeFailure, IndexMutationEventAcknowledger, PostgresMutationStore,
    SharedIndexMutationEventRegistry, SharedIndexSchemaRegistry, SharedIndexSourceRegistry,
};
use rustok_telemetry::runtime_consumer_metrics;
use tokio::task::JoinHandle;

use crate::common::settings::EventDeliveryProfile;
use crate::error::{Error, Result};
use crate::services::app_lifecycle::StopHandle;
use crate::services::app_runtime::module_runtime_extensions_from_ctx;
use crate::services::event_transport_factory::EventRuntime;
use crate::services::server_runtime_context::ServerRuntimeContext;

const ENABLE_ENV: &str = "RUSTOK_PRODUCT_INDEX_REFRESH_CONSUMER_ENABLED";
const IDLE_POLL_ENV: &str = "RUSTOK_PRODUCT_INDEX_REFRESH_CONSUMER_IDLE_POLL_MS";
const PRODUCT_INDEX_REFRESH_TOPIC: &str = "domain";
const PRODUCT_INDEX_REFRESH_CONSUMER_GROUP: &str = "rustok-product-index-refresh";
const DEFAULT_IDLE_POLL_MS: u64 = 500;
const MAX_IDLE_POLL_MS: u64 = 60_000;
const METRICS_CONSUMER: &str = "product_index_refresh";
const STAGE_STARTUP: &str = "startup";
const STAGE_RECEIVE: &str = "receive";
const STAGE_DECODE: &str = "decode";
const STAGE_PROCESS: &str = "process";
const STAGE_ACKNOWLEDGEMENT: &str = "acknowledgement";
const PROCESS_FAILURE_CODE: &str = "product_index.refresh.process_failed";
const ACK_FAILURE_CODE: &str = "product_index.refresh.ack_failed";
const DECODE_FAILURE_CODE: &str = "product_index.refresh.decode_failed";
static PRODUCT_INDEX_REFRESH_WORKER_INSTANCE_IDS: AtomicU64 = AtomicU64::new(1);

#[derive(Debug, Clone)]
struct ProductIndexRefreshWorkerConfig {
    max_attempts: u32,
    base_backoff: Duration,
    max_backoff: Duration,
    idle_poll: Duration,
}

impl ProductIndexRefreshWorkerConfig {
    fn from_context(ctx: &ServerRuntimeContext) -> Result<Self> {
        let retry = &ctx.settings().events.relay_retry_policy;
        let max_attempts = u32::try_from(retry.max_attempts).map_err(|_| {
            Error::Message(
                "Product Index refresh consumer max attempts must fit in u32".to_string(),
            )
        })?;
        if max_attempts == 0 {
            return Err(Error::Message(
                "Product Index refresh consumer max attempts must be greater than zero".to_string(),
            ));
        }
        let base_backoff = Duration::from_millis(retry.base_backoff_ms);
        let max_backoff = Duration::from_millis(retry.max_backoff_ms);
        if base_backoff.is_zero() || max_backoff < base_backoff {
            return Err(Error::Message(
                "Product Index refresh consumer retry backoff must be positive and bounded"
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
        })
    }
}

pub struct ProductIndexRefreshWorkerHandle {
    instance_id: u64,
    handle: JoinHandle<()>,
}

impl ProductIndexRefreshWorkerHandle {
    pub fn instance_id(&self) -> u64 {
        self.instance_id
    }

    pub fn is_ready(&self) -> bool {
        !self.handle.is_finished()
    }
}

#[derive(Clone)]
struct ProductIndexRefreshAcknowledger {
    group: Arc<PersistentContractConsumerGroup>,
}

#[async_trait]
impl IndexMutationEventAcknowledger for ProductIndexRefreshAcknowledger {
    type Token = ConsumedContractEvent;

    async fn acknowledge(
        &self,
        token: &Self::Token,
    ) -> std::result::Result<(), IndexMutationAcknowledgeFailure> {
        self.group.acknowledge(token).await.map_err(|_| {
            IndexMutationAcknowledgeFailure::retryable(ACK_FAILURE_CODE)
                .expect("static Product Index acknowledgement failure code is valid")
        })
    }
}

struct ProductIndexRefreshRuntime {
    group: Arc<PersistentContractConsumerGroup>,
    worker:
        ProductIndexRefreshDeliveryWorker<PostgresMutationStore, ProductIndexRefreshAcknowledger>,
    schemas: SharedIndexSchemaRegistry,
    sources: SharedIndexSourceRegistry,
    events: SharedIndexMutationEventRegistry,
}

pub fn product_index_refresh_consumer_enabled() -> Result<bool> {
    match env::var(ENABLE_ENV) {
        Ok(value) => parse_bool(ENABLE_ENV, &value).map_err(Error::Message),
        Err(env::VarError::NotPresent) => Ok(false),
        Err(error) => Err(Error::Message(format!(
            "failed to read {ENABLE_ENV}: {error}"
        ))),
    }
}

pub async fn start_product_index_refresh_worker_if_enabled(
    ctx: &ServerRuntimeContext,
) -> Result<()> {
    if !ctx.settings().runtime.runs_background_workers()
        || ctx.shared_contains::<ProductIndexRefreshWorkerHandle>()
    {
        return Ok(());
    }
    if !product_index_refresh_consumer_enabled()? {
        tracing::info!(
            env = ENABLE_ENV,
            "Product Index refresh consumer disabled by default-off runtime flag"
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

    let extensions = module_runtime_extensions_from_ctx(ctx);
    let schemas = extensions
        .get::<SharedIndexSchemaRegistry>()
        .cloned()
        .ok_or_else(|| {
            Error::Message("Product Index schema registry is unavailable".to_string())
        })?;
    let sources = extensions
        .get::<SharedIndexSourceRegistry>()
        .cloned()
        .ok_or_else(|| {
            Error::Message("Product Index source registry is unavailable".to_string())
        })?;
    let events = extensions
        .get::<SharedIndexMutationEventRegistry>()
        .cloned()
        .ok_or_else(|| Error::Message("Product Index event registry is unavailable".to_string()))?;
    for event_domain in [
        PRODUCT_INDEX_LOCALE_REFRESH_EVENT_DOMAIN,
        PRODUCT_INDEX_VARIANT_REFRESH_EVENT_DOMAIN,
    ] {
        if events.get(event_domain).is_none() {
            runtime_consumer_metrics::record_failure(
                METRICS_CONSUMER,
                STAGE_STARTUP,
                "product_index.refresh.route_missing",
            );
            return Err(Error::Message(format!(
                "Product Index refresh route is unavailable: {event_domain}"
            )));
        }
    }

    let group = Arc::new(
        transport
            .open_persistent_contract_consumer_group(
                PRODUCT_INDEX_REFRESH_CONSUMER_GROUP,
                PRODUCT_INDEX_REFRESH_TOPIC,
            )
            .await
            .map_err(|error| {
                runtime_consumer_metrics::record_failure(
                    METRICS_CONSUMER,
                    STAGE_STARTUP,
                    "product_index.refresh.consumer_open_failed",
                );
                Error::Message(format!(
                    "Product Index refresh consumer startup failed: {error}"
                ))
            })?,
    );
    let acknowledger = ProductIndexRefreshAcknowledger {
        group: Arc::clone(&group),
    };
    let runtime = ProductIndexRefreshRuntime {
        group,
        worker: ProductIndexRefreshDeliveryWorker::new(
            PostgresMutationStore::new(ctx.db_clone()),
            acknowledger,
        ),
        schemas,
        sources,
        events,
    };

    if !ctx.shared_contains::<StopHandle>() {
        let (stop_handle, _stop_rx) = StopHandle::new();
        ctx.shared_insert(stop_handle);
    }
    let stop_rx = ctx
        .shared_get::<StopHandle>()
        .expect("StopHandle must be registered before Product Index refresh worker startup")
        .subscribe();
    let config = ProductIndexRefreshWorkerConfig::from_context(ctx)?;
    let instance_id = PRODUCT_INDEX_REFRESH_WORKER_INSTANCE_IDS.fetch_add(1, Ordering::Relaxed);
    runtime_consumer_metrics::record_worker_start(METRICS_CONSUMER);
    tracing::info!(
        worker = METRICS_CONSUMER,
        instance_id,
        consumer_group = PRODUCT_INDEX_REFRESH_CONSUMER_GROUP,
        topic = PRODUCT_INDEX_REFRESH_TOPIC,
        max_attempts = config.max_attempts,
        "Starting Product Index refresh consumer worker"
    );
    ctx.shared_insert(ProductIndexRefreshWorkerHandle {
        instance_id,
        handle: tokio::spawn(product_index_refresh_worker_loop(runtime, config, stop_rx)),
    });
    Ok(())
}

async fn product_index_refresh_worker_loop(
    runtime: ProductIndexRefreshRuntime,
    config: ProductIndexRefreshWorkerConfig,
    mut stop_rx: tokio::sync::watch::Receiver<bool>,
) {
    loop {
        if *stop_rx.borrow() {
            record_worker_termination("shutdown");
            return;
        }
        let received = tokio::select! {
            result = runtime.group.receive_delivery() => Some(result),
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
            return;
        };
        match received {
            Ok(Some(PersistentContractDelivery::Event(consumed))) => {
                let consumed = *consumed;
                if !process_consumed_delivery(&runtime, &config, &mut stop_rx, consumed).await {
                    return;
                }
            }
            Ok(Some(PersistentContractDelivery::DecodeFailure(failure))) => {
                runtime_consumer_metrics::record_failure(
                    METRICS_CONSUMER,
                    STAGE_DECODE,
                    failure.stable_error_code(),
                );
                record_worker_termination("decode_failure");
                tracing::error!(
                    worker = METRICS_CONSUMER,
                    error_code = failure.stable_error_code(),
                    "Product Index refresh consumer received undecodable contract bytes; source offset remains uncommitted"
                );
                return;
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
                    "product_index.refresh.receive_failed",
                );
                record_worker_termination("receive_failure");
                tracing::error!(
                    worker = METRICS_CONSUMER,
                    error = %error,
                    "Product Index refresh broker receive failed; source offset remains uncommitted"
                );
                return;
            }
        }
    }
}

async fn process_consumed_delivery(
    runtime: &ProductIndexRefreshRuntime,
    config: &ProductIndexRefreshWorkerConfig,
    stop_rx: &mut tokio::sync::watch::Receiver<bool>,
    consumed: ConsumedContractEvent,
) -> bool {
    let delivery = match product_index_refresh_delivery_from_envelope(
        &consumed.envelope,
        consumed.clone(),
    ) {
        Ok(delivery) => delivery,
        Err(error) => {
            runtime_consumer_metrics::record_failure(
                METRICS_CONSUMER,
                STAGE_DECODE,
                DECODE_FAILURE_CODE,
            );
            record_worker_termination("semantic_decode_failure");
            tracing::error!(
                worker = METRICS_CONSUMER,
                event_id = %consumed.envelope.id(),
                error = %error,
                "Product Index refresh envelope projection failed; source offset remains uncommitted"
            );
            return false;
        }
    };

    let Some(delivery) = delivery else {
        return acknowledge_unrelated(runtime, config, stop_rx, &consumed).await;
    };

    let mut attempt = 1;
    loop {
        match runtime
            .worker
            .process(
                runtime.schemas.registry(),
                &runtime.sources,
                &runtime.events,
                if attempt == 1 {
                    delivery_from_consumed(&consumed).expect("validated Product refresh delivery")
                } else {
                    delivery_from_consumed(&consumed).expect("validated Product refresh redelivery")
                },
            )
            .await
        {
            Ok(outcome) => {
                tracing::debug!(
                    worker = METRICS_CONSUMER,
                    event_id = %outcome.event_id(),
                    source_name = outcome.source_name(),
                    source_version = outcome.source_version(),
                    mutation_outcome = ?outcome.mutation_outcome(),
                    attempt,
                    "Product Index refresh delivery durably applied and acknowledged"
                );
                return true;
            }
            Err(error) if attempt < config.max_attempts => {
                let delay = retry_delay(config, attempt);
                runtime_consumer_metrics::record_failure(
                    METRICS_CONSUMER,
                    STAGE_PROCESS,
                    PROCESS_FAILURE_CODE,
                );
                runtime_consumer_metrics::record_retry(METRICS_CONSUMER, STAGE_PROCESS);
                tracing::warn!(
                    worker = METRICS_CONSUMER,
                    event_id = %consumed.envelope.id(),
                    error = %error,
                    attempt,
                    retry_delay_ms = duration_millis(delay),
                    "Product Index refresh processing failed; retrying without committing the broker offset"
                );
                if wait_or_stop(delay, stop_rx).await {
                    record_worker_termination("shutdown_in_flight");
                    return false;
                }
                attempt += 1;
            }
            Err(error) => {
                runtime_consumer_metrics::record_failure(
                    METRICS_CONSUMER,
                    STAGE_PROCESS,
                    PROCESS_FAILURE_CODE,
                );
                record_worker_termination("delivery_failure");
                tracing::error!(
                    worker = METRICS_CONSUMER,
                    event_id = %consumed.envelope.id(),
                    error = %error,
                    attempt,
                    "Product Index refresh processing exhausted bounded retries; source offset remains uncommitted for broker redelivery"
                );
                return false;
            }
        }
    }
}

fn delivery_from_consumed(
    consumed: &ConsumedContractEvent,
) -> std::result::Result<
    ProductIndexRefreshDelivery<ConsumedContractEvent>,
    EventContractEnvelopeError,
> {
    product_index_refresh_delivery_from_envelope(&consumed.envelope, consumed.clone())
        .map(|value| value.expect("caller established canonical Product Index refresh payload"))
}

fn product_index_refresh_delivery_from_envelope<T>(
    envelope: &ContractEventEnvelope,
    acknowledgement_token: T,
) -> std::result::Result<Option<ProductIndexRefreshDelivery<T>>, EventContractEnvelopeError> {
    let delivery = match envelope.payload()? {
        ContractEventPayload::ProductIndexRefresh(
            ProductIndexRefreshEvent::LocaleRefreshRequested {
                product_id,
                locale,
                source_version,
            },
        ) => ProductIndexRefreshDelivery::locale(
            envelope.id(),
            envelope.tenant_id(),
            *product_id,
            locale.clone(),
            *source_version,
            acknowledgement_token,
        ),
        ContractEventPayload::ProductIndexRefresh(
            ProductIndexRefreshEvent::VariantRefreshRequested {
                product_id,
                variant_id,
                source_version,
            },
        ) => ProductIndexRefreshDelivery::variant(
            envelope.id(),
            envelope.tenant_id(),
            *product_id,
            *variant_id,
            *source_version,
            acknowledgement_token,
        ),
        _ => return Ok(None),
    };
    Ok(Some(delivery))
}

async fn acknowledge_unrelated(
    runtime: &ProductIndexRefreshRuntime,
    config: &ProductIndexRefreshWorkerConfig,
    stop_rx: &mut tokio::sync::watch::Receiver<bool>,
    consumed: &ConsumedContractEvent,
) -> bool {
    let mut attempt = 1;
    loop {
        match runtime.group.acknowledge(consumed).await {
            Ok(()) => return true,
            Err(error) if attempt < config.max_attempts => {
                let delay = retry_delay(config, attempt);
                runtime_consumer_metrics::record_failure(
                    METRICS_CONSUMER,
                    STAGE_ACKNOWLEDGEMENT,
                    ACK_FAILURE_CODE,
                );
                runtime_consumer_metrics::record_retry(METRICS_CONSUMER, STAGE_ACKNOWLEDGEMENT);
                tracing::warn!(
                    worker = METRICS_CONSUMER,
                    event_id = %consumed.envelope.id(),
                    error = %error,
                    attempt,
                    retry_delay_ms = duration_millis(delay),
                    "Unrelated sealed domain event acknowledgement failed; retrying acknowledgement only"
                );
                if wait_or_stop(delay, stop_rx).await {
                    record_worker_termination("shutdown_in_flight");
                    return false;
                }
                attempt += 1;
            }
            Err(error) => {
                runtime_consumer_metrics::record_failure(
                    METRICS_CONSUMER,
                    STAGE_ACKNOWLEDGEMENT,
                    ACK_FAILURE_CODE,
                );
                record_worker_termination("unrelated_ack_failure");
                tracing::error!(
                    worker = METRICS_CONSUMER,
                    event_id = %consumed.envelope.id(),
                    error = %error,
                    attempt,
                    "Unrelated sealed domain event acknowledgement exhausted bounded retries; source offset remains uncommitted"
                );
                return false;
            }
        }
    }
}

fn retry_delay(config: &ProductIndexRefreshWorkerConfig, attempt: u32) -> Duration {
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
        _ = tokio::time::sleep(delay) => *stop_rx.borrow(),
        changed = stop_rx.changed() => changed.is_err() || *stop_rx.borrow(),
    }
}

fn record_worker_termination(reason: &'static str) {
    runtime_consumer_metrics::record_worker_termination(METRICS_CONSUMER, reason);
}

fn parse_bool(name: &str, value: &str) -> std::result::Result<bool, String> {
    match value.trim().to_ascii_lowercase().as_str() {
        "1" | "true" | "yes" | "on" => Ok(true),
        "0" | "false" | "no" | "off" => Ok(false),
        _ => Err(format!("{name} must be a boolean value")),
    }
}

fn optional_u64_env(name: &str, default: u64) -> Result<u64> {
    match env::var(name) {
        Ok(value) => value.trim().parse::<u64>().map_err(|error| {
            Error::Message(format!("{name} must be an unsigned integer: {error}"))
        }),
        Err(env::VarError::NotPresent) => Ok(default),
        Err(error) => Err(Error::Message(format!("failed to read {name}: {error}"))),
    }
}

#[cfg(test)]
mod tests {
    use rustok_events::ContractEventEnvelope;
    use uuid::Uuid;

    use super::*;

    #[test]
    fn canonical_locale_envelope_maps_to_product_delivery_with_same_identity() {
        let tenant_id = Uuid::from_u128(20);
        let product_id = Uuid::from_u128(30);
        let event_id = Uuid::from_u128(40);
        let envelope = ContractEventEnvelope::new_with_envelope_id(
            event_id,
            tenant_id,
            None,
            ProductIndexRefreshEvent::LocaleRefreshRequested {
                product_id,
                locale: "en-us".to_string(),
                source_version: 7,
            },
        )
        .unwrap();

        let delivery = product_index_refresh_delivery_from_envelope(&envelope, "opaque-ack")
            .unwrap()
            .unwrap()
            .into_index_delivery()
            .unwrap();

        assert_eq!(delivery.event_id(), event_id);
        assert_eq!(delivery.key().tenant_id, tenant_id);
        assert_eq!(delivery.key().entity_id, product_id);
        assert_eq!(delivery.key().locale.as_ref().unwrap().as_str(), "en-US");
        assert_eq!(delivery.minimum_source_version(), 7);
        assert_eq!(delivery.acknowledgement_token(), &"opaque-ack");
    }

    #[test]
    fn canonical_variant_envelope_maps_to_variant_delivery_without_locale() {
        let tenant_id = Uuid::from_u128(21);
        let product_id = Uuid::from_u128(31);
        let variant_id = Uuid::from_u128(41);
        let event_id = Uuid::from_u128(51);
        let envelope = ContractEventEnvelope::new_with_envelope_id(
            event_id,
            tenant_id,
            None,
            ProductIndexRefreshEvent::VariantRefreshRequested {
                product_id,
                variant_id,
                source_version: 9,
            },
        )
        .unwrap();

        let delivery = product_index_refresh_delivery_from_envelope(&envelope, "opaque-ack")
            .unwrap()
            .unwrap()
            .into_index_delivery()
            .unwrap();

        assert_eq!(delivery.event_id(), event_id);
        assert_eq!(delivery.key().tenant_id, tenant_id);
        assert_eq!(delivery.key().entity_id, variant_id);
        assert!(delivery.key().locale.is_none());
        assert_eq!(delivery.minimum_source_version(), 9);
    }

    #[test]
    fn retry_backoff_is_exponential_and_bounded() {
        let config = ProductIndexRefreshWorkerConfig {
            max_attempts: 5,
            base_backoff: Duration::from_millis(100),
            max_backoff: Duration::from_millis(250),
            idle_poll: Duration::from_millis(10),
        };

        assert_eq!(retry_delay(&config, 1), Duration::from_millis(100));
        assert_eq!(retry_delay(&config, 2), Duration::from_millis(200));
        assert_eq!(retry_delay(&config, 3), Duration::from_millis(250));
        assert_eq!(retry_delay(&config, 20), Duration::from_millis(250));
    }
}
