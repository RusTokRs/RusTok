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
    let _ = ctx.shared_insert_if_absent(EventBusStartLock::default());
    let start_lock = ctx
        .shared_get::<EventBusStartLock>()
        .expect("EventBus start lock must be available after registration");
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
    let Some(shared_transport) = ctx.shared_get::<Arc<dyn EventTransport>>() else {
        tracing::warn!(
            "Event transport is not initialized; event bus will operate in local in-memory mode"
        );
        return;
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
            }
            Err(broadcast::error::RecvError::Closed) => {
                consumer_runtime.closed();
                return;
            }
        }
    }
}

pub fn transactional_event_bus_from_context(ctx: &ServerRuntimeContext) -> TransactionalEventBus {
    let transport = ctx
        .shared_get::<Arc<dyn EventTransport>>()
        .expect("Event transport must be initialized before creating TransactionalEventBus");
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
    use super::EventForwarderHandle;

    #[tokio::test]
    async fn event_forwarder_handle_reports_terminal_task() {
        let handle = EventForwarderHandle::new(tokio::spawn(std::future::pending()));
        assert!(handle.is_running());

        handle.abort();
        tokio::task::yield_now().await;

        assert!(!handle.is_running());
    }
}
