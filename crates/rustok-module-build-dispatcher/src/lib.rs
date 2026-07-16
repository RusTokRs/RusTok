//! Broker-neutral delivery handling for isolated module builds.

pub mod host;

use async_trait::async_trait;
use rustok_events::DomainEvent;
use rustok_iggy::{ConsumedEvent, IggyTransport, PersistentConsumerGroup, MODULE_BUILD_TOPIC};
use rustok_modules::{
    ModuleBuildProtocolError, ModuleBuildResultRecord, ModuleBuildWorker, SeaOrmModuleBuildService,
};
use std::sync::Arc;
use tokio::sync::Mutex;
use uuid::Uuid;

pub use host::{run_dispatcher, ModuleBuildDispatcherConfig};

/// Dedicated external consumer group for immutable module-build queue events.
pub const MODULE_BUILD_CONSUMER_GROUP: &str = "rustok-module-build-dispatcher";

/// Broker message already filtered to `module.build.queued`. The broker adapter
/// owns decoding and offset/ack-token handling; the owner delivery component
/// receives only the immutable correlation facts it needs.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ModuleBuildDelivery {
    pub delivery_id: Uuid,
    pub request_id: Uuid,
    pub tenant_id: Uuid,
}

/// External broker adapter. An implementation must acknowledge a delivery only
/// after [`ModuleBuildDeliveryConsumer::process`] succeeds. It deliberately
/// has no direct database or build-tool API.
#[async_trait]
pub trait ModuleBuildDeliverySource: Send + Sync {
    async fn acknowledge(&self, delivery_id: Uuid) -> Result<(), String>;
}

/// Iggy adapter for the dedicated `module-build` topic.
///
/// This adapter retains one persistent remote cursor. It exposes a delivery
/// only after validating its event payload, and commits its offset exclusively
/// after the owner service persisted a result or recognized an idempotent
/// redelivery. Keeping the transport alive in the adapter also keeps the
/// connector session alive for the cursor's full lifetime.
pub struct IggyModuleBuildDeliverySource {
    _transport: Arc<IggyTransport>,
    consumer: PersistentConsumerGroup,
    pending: Mutex<Option<(Uuid, ConsumedEvent)>>,
}

impl IggyModuleBuildDeliverySource {
    /// Opens the module-build consumer group on an already initialized remote
    /// Iggy transport. The configured broker topology must provision the
    /// `module-build` topic before the dispatcher starts.
    pub async fn open(transport: Arc<IggyTransport>) -> Result<Self, String> {
        let consumer = transport
            .open_persistent_consumer_group(MODULE_BUILD_CONSUMER_GROUP, MODULE_BUILD_TOPIC)
            .await
            .map_err(|error| error.to_string())?;
        Ok(Self {
            _transport: transport,
            consumer,
            pending: Mutex::new(None),
        })
    }

    /// Receives one validated queue delivery without committing its Iggy
    /// offset. An outstanding delivery must be acknowledged before calling
    /// this method again.
    pub async fn receive(&self) -> Result<Option<ModuleBuildDelivery>, String> {
        let mut pending = self.pending.lock().await;
        if pending.is_some() {
            return Err(
                "acknowledge the outstanding module-build delivery before receiving another"
                    .to_string(),
            );
        }
        let Some(consumed) = self
            .consumer
            .receive()
            .await
            .map_err(|error| error.to_string())?
        else {
            return Ok(None);
        };

        let DomainEvent::ModuleBuildQueued {
            request_id,
            tenant_id,
            ..
        } = &consumed.envelope.event
        else {
            return Err(format!(
                "module-build topic contained unexpected event type {}",
                consumed.envelope.event_type
            ));
        };
        if *tenant_id != consumed.envelope.tenant_id {
            return Err(format!(
                "module-build event {} has mismatched envelope and payload tenant IDs",
                consumed.envelope.id
            ));
        }

        let delivery = ModuleBuildDelivery {
            delivery_id: consumed.envelope.id,
            request_id: *request_id,
            tenant_id: *tenant_id,
        };
        *pending = Some((delivery.delivery_id, consumed));
        Ok(Some(delivery))
    }
}

#[async_trait]
impl ModuleBuildDeliverySource for IggyModuleBuildDeliverySource {
    async fn acknowledge(&self, delivery_id: Uuid) -> Result<(), String> {
        let mut pending = self.pending.lock().await;
        let (pending_id, consumed) = pending.as_ref().ok_or_else(|| {
            "module-build delivery acknowledgement has no outstanding broker message".to_string()
        })?;
        if *pending_id != delivery_id {
            return Err("module-build delivery acknowledgement does not match the outstanding broker message"
                .to_string());
        }
        self.consumer
            .acknowledge(consumed)
            .await
            .map_err(|error| error.to_string())?;
        *pending = None;
        Ok(())
    }
}

/// Result of one delivery attempt. A terminal result may already have been
/// persisted when a broker redelivers an older queue notification.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ModuleBuildDeliveryResult {
    Persisted(ModuleBuildResultRecord),
    AlreadySettled,
}

/// Coordinates an external broker delivery with the owner-side queue/result
/// contract. It cannot execute Cargo and does not expose the worker process.
pub struct ModuleBuildDeliveryConsumer<'a, W: ?Sized> {
    service: &'a SeaOrmModuleBuildService,
    worker: &'a W,
}

impl<'a, W> ModuleBuildDeliveryConsumer<'a, W>
where
    W: ModuleBuildWorker + ?Sized,
{
    pub fn new(service: &'a SeaOrmModuleBuildService, worker: &'a W) -> Self {
        Self { service, worker }
    }

    /// Performs owner-validated remote delivery. Callers acknowledge the
    /// broker delivery only after this returns `Ok`.
    pub async fn process(
        &self,
        delivery: &ModuleBuildDelivery,
    ) -> Result<ModuleBuildDeliveryResult, ModuleBuildProtocolError> {
        match self
            .service
            .dispatch_queued(delivery.tenant_id, delivery.request_id, self.worker)
            .await
        {
            Ok(record) => Ok(ModuleBuildDeliveryResult::Persisted(record)),
            Err(ModuleBuildProtocolError::NotQueued) => {
                Ok(ModuleBuildDeliveryResult::AlreadySettled)
            }
            Err(error) => Err(error),
        }
    }

    /// Executes and acknowledges one broker delivery. A source failure is
    /// surfaced after result persistence so the broker can redeliver safely.
    pub async fn process_and_acknowledge<S>(
        &self,
        source: &S,
        delivery: ModuleBuildDelivery,
    ) -> Result<ModuleBuildDeliveryResult, ModuleBuildProtocolError>
    where
        S: ModuleBuildDeliverySource + ?Sized,
    {
        let result = self.process(&delivery).await?;
        source
            .acknowledge(delivery.delivery_id)
            .await
            .map_err(ModuleBuildProtocolError::Transport)?;
        Ok(result)
    }
}
