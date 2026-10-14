use std::panic::AssertUnwindSafe;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use futures_util::FutureExt;
use rustok_core::events::{
    BackpressureConfig, BackpressureController, EventEnvelope, EventTransport,
};
use rustok_core::{EventBus, EventConsumerRuntime};
use rustok_outbox::TransactionalEventBus;
use tokio::sync::broadcast;
use tokio::task::JoinHandle;

use crate::common::settings::RustokSettings;
use crate::services::server_runtime_context::ServerRuntimeContext;

const EVENT_FORWARDER_RESTART_DELAY: Duration = Duration::from_secs(1);

#[derive(Clone)]
pub struct SharedEventBus(pub Arc<EventBus>);

#[derive(Clone, Default)]
struct EventBusStartLock(Arc<Mutex<()>>);

struct AbortOnDropEventForwarderTask {
    task: JoinHandle<()>,
}

impl AbortOnDropEventForwarderTask {
    fn new(task: JoinHandle<()>) -> Self {
        Self { task }
    }

    fn is_running(&self) -> bool {
        !self.task.is_finished()
    }

    fn abort(&self) {
        self.task.abort();
    }
}

impl Drop for AbortOnDropEventForwarderTask {
    fn drop(&mut self) {
        self.task.abort();
    }
}

#[derive(Clone)]
pub struct EventForwarderHandle(Arc<AbortOnDropEventForwarderTask>);

impl EventForwarderHandle {
    fn new(task: JoinHandle<()>) -> Self {
        Self(Arc::new(AbortOnDropEventForwarderTask::new(task)))
    }

    pub fn is_running(&self) -> bool {
        self.0.is_running()
    }

    fn abort(&self) {
        self.0.abort();
    }
}

pub fn event_bus_from_context(ctx: &ServerRuntimeContext) -> EventBus {
    tracing::debug!("Resolving EventBus from ServerRuntimeContext");
    let _ = ctx.shared_insert_if_absent(EventBusStartLock::default());
    let start_lock = ctx.shared_get::<EventBusStartLock>().unwrap_or_default();
    let _start_guard = start_lock
        .0
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);

    let bus = if let Some(shared) = ctx.shared_get::<SharedEventBus>() {
        Arc::clone(&shared.0)
    } else {
        let bus = Arc::new(build_event_bus(ctx, Some(ctx.settings())));
        ctx.shared_insert(SharedEventBus(Arc::clone(&bus)));
        bus
    };

    ensure_event_forwarder(ctx, Arc::clone(&bus));
    (*bus).clone()
}

fn ensure_event_forwarder(ctx: &ServerRuntimeContext, bus: Arc<EventBus>) {
    let shared_transport = if let Some(transport) = ctx.shared_get::<Arc<dyn EventTransport>>() {
        transport
    } else {
        let default_transport = Arc::new(rustok_outbox::OutboxTransport::new(ctx.db_clone()))
            as Arc<dyn EventTransport>;
        let _ = ctx.shared_insert_if_absent(default_transport.clone());
        ctx.shared_get::<Arc<dyn EventTransport>>()
            .unwrap_or(default_transport)
    };

    if let Some(existing) = ctx.shared_get::<EventForwarderHandle>() {
        if existing.is_running() {
            return;
        }
        tracing::warn!("Event forwarder stopped; replacing supervised runtime");
        existing.abort();
    }

    let transport = shared_transport;
    let task = tokio::spawn(supervise_event_forwarder(bus, transport));
    ctx.shared_insert(EventForwarderHandle::new(task));
}

async fn supervise_event_forwarder(bus: Arc<EventBus>, transport: Arc<dyn EventTransport>) {
    loop {
        let receiver = bus.subscribe();
        let outcome = AssertUnwindSafe(run_event_forwarder(receiver, Arc::clone(&transport)))
            .catch_unwind()
            .await;

        if outcome.is_err() {
            tracing::error!("Event forwarder panicked; restarting");
        } else {
            tracing::error!("Event forwarder exited unexpectedly; restarting");
        }
        rustok_telemetry::metrics::record_event_error("server_event_forwarder", "worker_restart");
        tokio::time::sleep(EVENT_FORWARDER_RESTART_DELAY).await;
    }
}

async fn run_event_forwarder(
    mut receiver: broadcast::Receiver<EventEnvelope>,
    transport: Arc<dyn EventTransport>,
) {
    let consumer_runtime = EventConsumerRuntime::new("server_event_forwarder");
    consumer_runtime.restarted("worker_start");

    loop {
        match receiver.recv().await {
            Ok(envelope) => {
                if let Err(error) = transport.publish(envelope).await {
                    tracing::error!("Failed to publish domain event to transport: {error}");
                    rustok_telemetry::metrics::record_event_error(
                        "server_event_forwarder",
                        "publish",
                    );
                }
            }
            Err(broadcast::error::RecvError::Lagged(skipped)) => {
                consumer_runtime.lagged(skipped);
                tracing::warn!(
                    skipped,
                    "Server event forwarder lagged; {skipped} domain events dropped from broadcast queue"
                );
                rustok_telemetry::metrics::record_event_error("server_event_forwarder", "lagged");
            }
            Err(broadcast::error::RecvError::Closed) => {
                consumer_runtime.closed();
                return;
            }
        }
    }
}

pub fn transactional_event_bus_from_context(ctx: &ServerRuntimeContext) -> TransactionalEventBus {
    tracing::debug!("Resolving TransactionalEventBus from ServerRuntimeContext");
    let transport = if let Some(transport) = ctx.shared_get::<Arc<dyn EventTransport>>() {
        transport
    } else {
        let default_transport = Arc::new(rustok_outbox::OutboxTransport::new(ctx.db_clone()))
            as Arc<dyn EventTransport>;
        let _ = ctx.shared_insert_if_absent(default_transport.clone());
        default_transport
    };
    TransactionalEventBus::new(transport)
}

fn build_event_bus(ctx: &ServerRuntimeContext, settings: Option<&RustokSettings>) -> EventBus {
    let Some(runtime) =
        ctx.shared_get::<Arc<crate::services::event_transport_factory::EventRuntime>>()
    else {
        return EventBus::default();
    };

    let Some(settings) = settings else {
        tracing::warn!(
            "Rustok settings unavailable while creating EventBus; backpressure disabled"
        );
        return EventBus::with_capacity(runtime.channel_capacity);
    };

    tracing::debug!(
        capacity = runtime.channel_capacity,
        backpressure = settings.events.backpressure.enabled,
        "Building EventBus from runtime context"
    );

    if settings.events.backpressure.enabled {
        let config = &settings.events.backpressure;
        return EventBus::with_backpressure(
            runtime.channel_capacity,
            BackpressureController::new(BackpressureConfig::new(
                config.max_queue_depth,
                config.warning_threshold,
                config.critical_threshold,
            )),
        );
    }

    EventBus::with_capacity(runtime.channel_capacity)
}

#[cfg(test)]
mod tests {
    use super::{
        EventForwarderHandle, SharedEventBus, event_bus_from_context,
        transactional_event_bus_from_context,
    };
    use crate::common::settings::RustokSettings;
    use crate::services::server_runtime_context::ServerRuntimeContext;
    use rustok_core::EventBus;
    use sea_orm::Database;
    use std::sync::Arc;

    #[tokio::test]
    async fn event_forwarder_handle_reports_terminal_task() {
        let handle = EventForwarderHandle::new(tokio::spawn(std::future::pending()));
        assert!(handle.is_running());

        handle.abort();
        tokio::task::yield_now().await;

        assert!(!handle.is_running());
    }

    #[test]
    fn shared_event_bus_clone_preserves_reference() {
        let bus = Arc::new(EventBus::default());
        let shared = SharedEventBus(Arc::clone(&bus));
        let cloned = shared.clone();
        assert!(Arc::ptr_eq(&shared.0, &cloned.0));
    }

    #[tokio::test]
    async fn event_bus_from_context_is_idempotent() {
        let db = Database::connect("sqlite::memory:")
            .await
            .expect("connect sqlite");
        let ctx = ServerRuntimeContext::new(db, RustokSettings::default());

        let bus1 = event_bus_from_context(&ctx);
        let bus2 = event_bus_from_context(&ctx);

        assert!(Arc::ptr_eq(&bus1.stats(), &bus2.stats()));
        assert!(ctx.shared_get::<SharedEventBus>().is_some());
        assert!(ctx.shared_get::<EventForwarderHandle>().is_some());
    }

    #[tokio::test]
    async fn transactional_event_bus_from_context_initializes() {
        let db = Database::connect("sqlite::memory:")
            .await
            .expect("connect sqlite");
        let ctx = ServerRuntimeContext::new(db, RustokSettings::default());

        let _tx_bus = transactional_event_bus_from_context(&ctx);
        assert!(ctx.shared_get::<Arc<dyn rustok_core::events::EventTransport>>().is_some());
    }
}
