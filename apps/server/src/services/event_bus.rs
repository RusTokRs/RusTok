use std::panic::AssertUnwindSafe;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use futures_util::FutureExt;
use rustok_core::events::{
    BackpressureConfig, BackpressureController, EventEnvelope, EventTransport,
};
use rustok_core::{EventBus, EventConsumerRuntime};
use rustok_outbox::TransactionalEventBus;
use tokio::sync::{broadcast, watch};
use tokio::task::JoinHandle;

use crate::common::settings::RustokSettings;
use crate::services::app_lifecycle::StopHandle;
use crate::services::server_runtime_context::ServerRuntimeContext;

const EVENT_FORWARDER_RESTART_DELAY: Duration = Duration::from_secs(1);
const EVENT_FORWARDER_INITIAL_RETRY_DELAY: Duration = Duration::from_millis(100);
const EVENT_FORWARDER_MAX_RETRY_DELAY: Duration = Duration::from_secs(5);

#[derive(Clone)]
pub struct SharedEventBus(pub Arc<EventBus>);

#[derive(Clone, Default)]
struct EventBusStartLock(Arc<Mutex<()>>);

struct AbortOnDropEventForwarderTask {
    task: Mutex<Option<JoinHandle<()>>>,
}

impl AbortOnDropEventForwarderTask {
    fn new(task: JoinHandle<()>) -> Self {
        Self {
            task: Mutex::new(Some(task)),
        }
    }

    fn is_running(&self) -> bool {
        self.task
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .as_ref()
            .is_some_and(|task| !task.is_finished())
    }

    fn abort(&self) {
        if let Some(task) = self
            .task
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .as_ref()
        {
            task.abort();
        }
    }

    fn take_task(&self) -> Option<JoinHandle<()>> {
        self.task
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .take()
    }
}

impl Drop for AbortOnDropEventForwarderTask {
    fn drop(&mut self) {
        if let Some(task) = self
            .task
            .get_mut()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .as_ref()
        {
            task.abort();
        }
    }
}

#[derive(Clone)]
pub struct EventForwarderHandle {
    task: Arc<AbortOnDropEventForwarderTask>,
    stop_tx: watch::Sender<bool>,
}

impl EventForwarderHandle {
    #[cfg(test)]
    pub(crate) fn for_shutdown_test() -> (Self, Vec<tokio::task::AbortHandle>) {
        let (stop_tx, mut stop_rx) = watch::channel(false);
        let task = tokio::spawn(async move {
            let _ = stop_rx.changed().await;
        });
        let tasks = vec![task.abort_handle()];
        (Self::new(task, stop_tx), tasks)
    }

    fn new(task: JoinHandle<()>, stop_tx: watch::Sender<bool>) -> Self {
        Self {
            task: Arc::new(AbortOnDropEventForwarderTask::new(task)),
            stop_tx,
        }
    }

    pub fn is_running(&self) -> bool {
        self.task.is_running()
    }

    pub(crate) fn abort(&self) {
        self.task.abort();
    }

    pub(crate) fn request_stop(&self) {
        self.stop_tx.send_replace(true);
    }

    pub(crate) fn take_task(&self) -> Option<JoinHandle<()>> {
        self.task.take_task()
    }
}

pub fn event_bus_from_context(ctx: &ServerRuntimeContext) -> EventBus {
    tracing::debug!("Resolving EventBus from ServerRuntimeContext");
    ctx.shared_insert_if_absent(EventBusStartLock::default());
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
        if ctx.shared_insert_if_absent(default_transport.clone()) {
            default_transport
        } else {
            ctx.shared_get::<Arc<dyn EventTransport>>()
                .unwrap_or(default_transport)
        }
    };

    let stop = StopHandle::ensure(ctx);
    if stop.is_stopping() {
        return;
    }

    if let Some(existing) = ctx.shared_get::<EventForwarderHandle>() {
        if existing.is_running() {
            return;
        }
        tracing::warn!("Event forwarder stopped; replacing supervised runtime");
        existing.abort();
    }

    let transport = shared_transport;
    // The forwarder has a dedicated drain signal. The global StopHandle first
    // quiesces producers; shutdown requests this signal only after they have
    // joined, so a final in-flight publication cannot race an empty-queue exit.
    let (forwarder_stop_tx, forwarder_stop_rx) = watch::channel(false);
    // Subscribe before returning the bus to publishers. Spawning a task and
    // subscribing inside it leaves a startup window where broadcast::send
    // reports no receivers and the first accepted domain event is rejected.
    let initial_receiver = EventForwarderReceiver::new(bus.subscribe(), bus.backpressure());
    let task = tokio::spawn(supervise_event_forwarder(
        bus,
        transport,
        forwarder_stop_rx,
        initial_receiver,
    ));
    ctx.shared_insert(EventForwarderHandle::new(task, forwarder_stop_tx));
}

pub(crate) fn take_forwarder_task_for_shutdown(
    ctx: &ServerRuntimeContext,
) -> Option<JoinHandle<()>> {
    let start_lock = ctx.shared_get_or_insert_with(EventBusStartLock::default);
    let _start_guard = start_lock
        .0
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    ctx.shared_take::<EventForwarderHandle>()
        .and_then(|forwarder| {
            forwarder.request_stop();
            forwarder.take_task()
        })
}

async fn supervise_event_forwarder(
    bus: Arc<EventBus>,
    transport: Arc<dyn EventTransport>,
    mut stop_rx: watch::Receiver<bool>,
    initial_receiver: EventForwarderReceiver,
) {
    let mut next_receiver = Some(initial_receiver);
    loop {
        let receiver = next_receiver
            .take()
            .unwrap_or_else(|| EventForwarderReceiver::new(bus.subscribe(), bus.backpressure()));
        let outcome = AssertUnwindSafe(run_event_forwarder(
            receiver,
            Arc::clone(&transport),
            stop_rx.clone(),
        ))
        .catch_unwind()
        .await;

        // run_event_forwarder owns the drain even when stop was requested
        // before this supervisor was first polled. Do not return before the
        // initial receiver has persisted its already-accepted queue.
        if *stop_rx.borrow() {
            return;
        }

        if outcome.is_err() {
            tracing::error!("Event forwarder panicked; restarting");
        } else {
            tracing::error!("Event forwarder exited unexpectedly; restarting");
        }
        rustok_telemetry::metrics::record_event_error("server_event_forwarder", "worker_restart");

        next_receiver = Some(
            subscribe_for_forwarder_restart(
                &bus,
                &mut stop_rx,
                EVENT_FORWARDER_RESTART_DELAY,
            )
            .await,
        );
    }
}

/// Keep a bounded receiver subscribed throughout restart backoff. Publishers
/// can fill the broadcast ring up to its hard capacity instead of losing every
/// event to a temporary "no receivers" error. If shutdown arrives during the
/// delay, the next supervisor iteration drains this receiver before exiting.
async fn subscribe_for_forwarder_restart(
    bus: &Arc<EventBus>,
    stop_rx: &mut watch::Receiver<bool>,
    delay: Duration,
) -> EventForwarderReceiver {
    let receiver = EventForwarderReceiver::new(bus.subscribe(), bus.backpressure());
    tokio::select! {
        biased;
        _ = stop_rx.changed() => {}
        _ = tokio::time::sleep(delay) => {}
    }
    receiver
}

struct EventForwarderReceiver {
    receiver: broadcast::Receiver<EventEnvelope>,
    backpressure: Option<Arc<BackpressureController>>,
}

impl EventForwarderReceiver {
    fn new(
        receiver: broadcast::Receiver<EventEnvelope>,
        backpressure: Option<Arc<BackpressureController>>,
    ) -> Self {
        Self {
            receiver,
            backpressure,
        }
    }

    async fn recv(
        &mut self,
    ) -> std::result::Result<EventEnvelope, broadcast::error::RecvError> {
        self.receiver.recv().await
    }

    fn try_recv(
        &mut self,
    ) -> std::result::Result<EventEnvelope, broadcast::error::TryRecvError> {
        self.receiver.try_recv()
    }
}

impl Drop for EventForwarderReceiver {
    fn drop(&mut self) {
        release_backpressure_slots(&self.backpressure, self.receiver.len());
    }
}

struct BackpressureSlotGuard(Option<Arc<BackpressureController>>);

impl Drop for BackpressureSlotGuard {
    fn drop(&mut self) {
        if let Some(backpressure) = &self.0 {
            backpressure.release();
        }
    }
}

fn release_backpressure_slots(
    backpressure: &Option<Arc<BackpressureController>>,
    count: usize,
) {
    if let Some(backpressure) = backpressure {
        for _ in 0..count {
            backpressure.release();
        }
    }
}

async fn publish_event_with_retry(
    envelope: EventEnvelope,
    transport: Arc<dyn EventTransport>,
    backpressure: Option<Arc<BackpressureController>>,
) {
    let _slot = BackpressureSlotGuard(backpressure);
    let event_id = envelope.id;
    let event_type = envelope.event.event_type();
    let mut attempt = 0_u64;
    let mut retry_delay = EVENT_FORWARDER_INITIAL_RETRY_DELAY;

    loop {
        attempt += 1;
        let outcome = AssertUnwindSafe(transport.publish(envelope.clone()))
            .catch_unwind()
            .await;
        match outcome {
            Ok(Ok(())) => {
                if attempt > 1 {
                    tracing::info!(
                        event_id = %event_id,
                        event_type,
                        attempts = attempt,
                        "Domain event persisted after forwarder retries"
                    );
                }
                return;
            }
            Ok(Err(error)) => {
                tracing::warn!(
                    event_id = %event_id,
                    event_type,
                    attempts = attempt,
                    error = %error,
                    "Failed to persist domain event; retrying the same envelope"
                );
            }
            Err(_) => {
                tracing::error!(
                    event_id = %event_id,
                    event_type,
                    attempts = attempt,
                    "Event transport panicked; retrying the same envelope"
                );
            }
        }
        rustok_telemetry::metrics::record_event_error("server_event_forwarder", "publish");
        tokio::time::sleep(retry_delay).await;
        retry_delay = retry_delay
            .saturating_mul(2)
            .min(EVENT_FORWARDER_MAX_RETRY_DELAY);
    }
}

async fn run_event_forwarder(
    mut receiver: EventForwarderReceiver,
    transport: Arc<dyn EventTransport>,
    mut stop_rx: watch::Receiver<bool>,
) {
    let consumer_runtime = EventConsumerRuntime::new("server_event_forwarder");
    consumer_runtime.restarted("worker_start");
    let mut stopping = *stop_rx.borrow();

    loop {
        if stopping {
            match receiver.try_recv() {
                Ok(envelope) => {
                    publish_event_with_retry(
                        envelope,
                        Arc::clone(&transport),
                        receiver.backpressure.clone(),
                    )
                    .await;
                }
                Err(broadcast::error::TryRecvError::Lagged(skipped)) => {
                    release_backpressure_slots(&receiver.backpressure, skipped as usize);
                    consumer_runtime.lagged(skipped);
                    tracing::warn!(
                        skipped,
                        "Server event forwarder lagged while draining its shutdown queue"
                    );
                    rustok_telemetry::metrics::record_event_error(
                        "server_event_forwarder",
                        "lagged",
                    );
                }
                Err(
                    broadcast::error::TryRecvError::Empty
                    | broadcast::error::TryRecvError::Closed,
                ) => {
                    consumer_runtime.closed();
                    return;
                }
            }
            continue;
        }

        tokio::select! {
            biased;
            changed = stop_rx.changed() => {
                if changed.is_err() || *stop_rx.borrow() {
                    stopping = true;
                }
            }
            received = receiver.recv() => {
                match received {
                    Ok(envelope) => {
                        publish_event_with_retry(
                            envelope,
                            Arc::clone(&transport),
                            receiver.backpressure.clone(),
                        )
                        .await;
                    }
                    Err(broadcast::error::RecvError::Lagged(skipped)) => {
                        release_backpressure_slots(&receiver.backpressure, skipped as usize);
                        consumer_runtime.lagged(skipped);
                        tracing::warn!(
                            skipped,
                            "Server event forwarder lagged; {skipped} domain events dropped from broadcast queue"
                        );
                        rustok_telemetry::metrics::record_event_error(
                            "server_event_forwarder",
                            "lagged",
                        );
                    }
                    Err(broadcast::error::RecvError::Closed) => {
                        consumer_runtime.closed();
                        return;
                    }
                }
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
        ctx.shared_insert_if_absent(default_transport.clone());
        default_transport
    };
    TransactionalEventBus::new(transport)
}

fn build_event_bus(ctx: &ServerRuntimeContext, settings: Option<&RustokSettings>) -> EventBus {
    const DEFAULT_WARNING_THRESHOLD: f64 = 0.999;
    const HARD_CAPACITY_THRESHOLD: f64 = 1.0;

    let runtime_capacity = ctx
        .shared_get::<Arc<crate::services::event_transport_factory::EventRuntime>>()
        .map(|runtime| runtime.channel_capacity);
    // EventBus can be requested before EventRuntime is composed. Still honor the
    // validated bootstrap capacity instead of silently falling back to the core
    // default, and always guard the durable forwarder's broadcast ring.
    let channel_capacity = runtime_capacity
        .or_else(|| settings.map(|settings| settings.events.channel_capacity))
        .unwrap_or(128)
        .max(1);

    let backpressure = settings.map(|settings| &settings.events.backpressure);
    let (max_queue_depth, warning_threshold, critical_threshold) =
        match backpressure.filter(|config| config.enabled) {
            Some(config) => {
                let configured_max_depth = config.max_queue_depth.max(1);
                let max_queue_depth = configured_max_depth.min(channel_capacity);
                if configured_max_depth > channel_capacity {
                    tracing::warn!(
                        configured_max_depth,
                        channel_capacity,
                        "Clamping EventBus backpressure depth to broadcast capacity to prevent lagged event loss"
                    );
                }
                (
                    max_queue_depth,
                    config.warning_threshold,
                    config.critical_threshold,
                )
            }
            None => {
                if settings.is_none() {
                    tracing::warn!(
                        capacity = channel_capacity,
                        "Rustok settings unavailable while creating EventBus; enforcing the broadcast hard-capacity guard"
                    );
                }
                // Disabling configurable backpressure must not disable the hard
                // admission bound: otherwise a slow/unavailable outbox writer can
                // let Tokio overwrite events before the forwarder persists them.
                (
                    channel_capacity,
                    DEFAULT_WARNING_THRESHOLD,
                    HARD_CAPACITY_THRESHOLD,
                )
            }
        };

    tracing::debug!(
        capacity = channel_capacity,
        max_queue_depth,
        configurable_backpressure = backpressure.is_some_and(|config| config.enabled),
        "Building bounded EventBus from runtime context"
    );

    EventBus::with_backpressure(
        channel_capacity,
        BackpressureController::new(BackpressureConfig::new(
            max_queue_depth,
            warning_threshold,
            critical_threshold,
        )),
    )
}

#[cfg(test)]
mod tests {
    use super::{
        EventForwarderHandle, EventForwarderReceiver, SharedEventBus, event_bus_from_context,
        run_event_forwarder, transactional_event_bus_from_context,
    };
    use crate::common::settings::RustokSettings;
    use crate::services::server_runtime_context::ServerRuntimeContext;
    use rustok_core::EventBus;
    use rustok_core::events::{
        BackpressureConfig, BackpressureController, EventTransport, ReliabilityLevel,
    };
    use sea_orm::Database;
    use std::sync::{
        Arc, Mutex,
        atomic::{AtomicUsize, Ordering},
    };
    use std::time::Duration;
    use tokio::sync::watch;

    #[tokio::test]
    async fn event_forwarder_handle_reports_terminal_task() {
        let (stop_tx, _stop_rx) = watch::channel(false);
        let handle =
            EventForwarderHandle::new(tokio::spawn(std::future::pending()), stop_tx);
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
    async fn event_forwarder_retries_and_drains_before_releasing_backpressure() {
        use async_trait::async_trait;
        use rustok_events::{DomainEvent, EventEnvelope};
        use uuid::Uuid;

        struct FlakyTransport {
            attempts: AtomicUsize,
            accepted: Mutex<Vec<EventEnvelope>>,
        }

        #[async_trait]
        impl EventTransport for FlakyTransport {
            async fn publish(&self, envelope: EventEnvelope) -> rustok_core::Result<()> {
                let attempt = self.attempts.fetch_add(1, Ordering::SeqCst);
                if attempt == 0 {
                    return Err(rustok_core::Error::External(
                        "temporary outbox failure".to_owned(),
                    ));
                }
                self.accepted.lock().unwrap().push(envelope);
                Ok(())
            }

            fn reliability_level(&self) -> ReliabilityLevel {
                ReliabilityLevel::InMemory
            }

            fn as_any(&self) -> &dyn std::any::Any {
                self
            }
        }

        let backpressure = BackpressureController::new(BackpressureConfig::new(8, 0.5, 0.75));
        let bus = EventBus::with_backpressure(8, backpressure);
        let receiver =
            EventForwarderReceiver::new(bus.subscribe(), bus.backpressure());
        let transport = Arc::new(FlakyTransport {
            attempts: AtomicUsize::new(0),
            accepted: Mutex::new(vec![]),
        });
        let (stop_tx, stop_rx) = watch::channel(false);
        let task = tokio::spawn(run_event_forwarder(
            receiver,
            transport.clone(),
            stop_rx,
        ));

        let tenant_id = Uuid::new_v4();
        let envelope = EventEnvelope::new(
            tenant_id,
            None,
            DomainEvent::TenantUpdated { tenant_id },
        );
        bus.publish_envelope(envelope.clone())
            .expect("event should enter the in-memory bus");

        // Stop immediately: the forwarder must still retry and persist the
        // already-accepted envelope before it exits.
        stop_tx.send_replace(true);
        tokio::time::timeout(Duration::from_secs(3), task)
            .await
            .expect("forwarder should drain its accepted queue")
            .expect("forwarder task should join");

        assert_eq!(transport.attempts.load(Ordering::SeqCst), 2);
        assert_eq!(
            transport.accepted.lock().unwrap().as_slice(),
            &[envelope]
        );
        assert_eq!(
            bus.backpressure().expect("backpressure enabled").current_depth(),
            0
        );
    }

    #[tokio::test]
    async fn default_event_bus_enforces_the_broadcast_hard_capacity_bound() {
        use rustok_events::{DomainEvent, EventEnvelope};
        use uuid::Uuid;

        let db = Database::connect("sqlite::memory:")
            .await
            .expect("connect sqlite");
        let mut settings = RustokSettings::default();
        settings.events.channel_capacity = 2;
        let ctx = ServerRuntimeContext::new(db, settings);
        let bus = super::build_event_bus(&ctx, Some(ctx.settings()));
        let mut receiver = bus.subscribe();

        let first_tenant = Uuid::new_v4();
        let first = EventEnvelope::new(
            first_tenant,
            None,
            DomainEvent::TenantUpdated {
                tenant_id: first_tenant,
            },
        );
        let second_tenant = Uuid::new_v4();
        let second = EventEnvelope::new(
            second_tenant,
            None,
            DomainEvent::TenantUpdated {
                tenant_id: second_tenant,
            },
        );
        let third_tenant = Uuid::new_v4();
        let third = EventEnvelope::new(
            third_tenant,
            None,
            DomainEvent::TenantUpdated {
                tenant_id: third_tenant,
            },
        );

        bus.publish_envelope(first.clone()).unwrap();
        bus.publish_envelope(second.clone()).unwrap();
        assert!(
            bus.publish_envelope(third).is_err(),
            "disabled soft backpressure must still reject before the broadcast ring overwrites"
        );
        assert_eq!(receiver.recv().await.unwrap().id, first.id);
        assert_eq!(receiver.recv().await.unwrap().id, second.id);
        assert_eq!(
            bus.backpressure()
                .expect("hard capacity guard is always installed")
                .current_depth(),
            2
        );
    }

    #[tokio::test]
    async fn forwarder_stays_live_until_in_flight_producers_finish() {
        use async_trait::async_trait;
        use rustok_events::{DomainEvent, EventEnvelope};
        use uuid::Uuid;

        struct RecordingTransport {
            accepted: Mutex<Vec<EventEnvelope>>,
        }

        #[async_trait]
        impl EventTransport for RecordingTransport {
            async fn publish(&self, envelope: EventEnvelope) -> rustok_core::Result<()> {
                self.accepted.lock().unwrap().push(envelope);
                Ok(())
            }

            fn reliability_level(&self) -> ReliabilityLevel {
                ReliabilityLevel::InMemory
            }

            fn as_any(&self) -> &dyn std::any::Any {
                self
            }
        }

        let backpressure = BackpressureController::new(BackpressureConfig::new(8, 0.7, 1.0));
        let bus = Arc::new(EventBus::with_backpressure(8, backpressure));
        let receiver = EventForwarderReceiver::new(bus.subscribe(), bus.backpressure());
        let transport = Arc::new(RecordingTransport {
            accepted: Mutex::new(vec![]),
        });
        let (forwarder_stop_tx, forwarder_stop_rx) = watch::channel(false);
        let forwarder = tokio::spawn(run_event_forwarder(
            receiver,
            transport.clone(),
            forwarder_stop_rx,
        ));

        // Simulate the global worker stop arriving while a producer still owns
        // an in-flight operation. The forwarder must not consume that signal;
        // it is stopped only after the producer has published and joined.
        let (producer_stop_tx, mut producer_stop_rx) = watch::channel(false);
        let producer_bus = Arc::clone(&bus);
        let producer = tokio::spawn(async move {
            producer_stop_rx.changed().await.unwrap();
            let tenant_id = Uuid::new_v4();
            let envelope = EventEnvelope::new(
                tenant_id,
                None,
                DomainEvent::TenantUpdated { tenant_id },
            );
            producer_bus.publish_envelope(envelope.clone()).unwrap();
            envelope
        });

        producer_stop_tx.send_replace(true);
        let envelope = producer.await.unwrap();
        forwarder_stop_tx.send_replace(true);
        tokio::time::timeout(Duration::from_secs(3), forwarder)
            .await
            .expect("forwarder should drain after producer completion")
            .expect("forwarder task should join");

        assert_eq!(transport.accepted.lock().unwrap().as_slice(), &[envelope]);
        assert_eq!(
            bus.backpressure().expect("hard capacity guard").current_depth(),
            0
        );
    }

    #[tokio::test]
    async fn supervisor_drains_initial_queue_when_stop_precedes_first_poll() {
        use async_trait::async_trait;
        use rustok_events::{DomainEvent, EventEnvelope};
        use uuid::Uuid;

        struct RecordingTransport {
            accepted: Mutex<Vec<EventEnvelope>>,
        }

        #[async_trait]
        impl EventTransport for RecordingTransport {
            async fn publish(&self, envelope: EventEnvelope) -> rustok_core::Result<()> {
                self.accepted.lock().unwrap().push(envelope);
                Ok(())
            }

            fn reliability_level(&self) -> ReliabilityLevel {
                ReliabilityLevel::InMemory
            }

            fn as_any(&self) -> &dyn std::any::Any {
                self
            }
        }

        let backpressure = BackpressureController::new(BackpressureConfig::new(8, 0.7, 1.0));
        let bus = Arc::new(EventBus::with_backpressure(8, backpressure));
        let initial_receiver = EventForwarderReceiver::new(bus.subscribe(), bus.backpressure());
        let tenant_id = Uuid::new_v4();
        let envelope = EventEnvelope::new(
            tenant_id,
            None,
            DomainEvent::TenantUpdated { tenant_id },
        );
        bus.publish_envelope(envelope.clone())
            .expect("the pre-subscribed receiver accepts the event");

        let transport = Arc::new(RecordingTransport {
            accepted: Mutex::new(vec![]),
        });
        let (stop_tx, stop_rx) = watch::channel(true);
        let task = tokio::spawn(super::supervise_event_forwarder(
            bus.clone(),
            transport.clone(),
            stop_rx,
            initial_receiver,
        ));

        tokio::time::timeout(Duration::from_secs(3), task)
            .await
            .expect("supervisor should drain before exiting")
            .expect("supervisor task should join");
        assert_eq!(
            transport.accepted.lock().unwrap().as_slice(),
            std::slice::from_ref(&envelope)
        );
        assert_eq!(
            bus.backpressure().expect("hard capacity guard").current_depth(),
            0
        );
        drop(stop_tx);
    }

    #[tokio::test]
    async fn restart_backoff_keeps_a_receiver_and_drains_when_shutdown_arrives() {
        use async_trait::async_trait;
        use rustok_events::{DomainEvent, EventEnvelope};
        use uuid::Uuid;

        struct RecordingTransport {
            accepted: Mutex<Vec<EventEnvelope>>,
        }

        #[async_trait]
        impl EventTransport for RecordingTransport {
            async fn publish(&self, envelope: EventEnvelope) -> rustok_core::Result<()> {
                self.accepted.lock().unwrap().push(envelope);
                Ok(())
            }

            fn reliability_level(&self) -> ReliabilityLevel {
                ReliabilityLevel::InMemory
            }

            fn as_any(&self) -> &dyn std::any::Any {
                self
            }
        }

        let bus = Arc::new(EventBus::with_backpressure(
            8,
            BackpressureController::new(BackpressureConfig::new(8, 0.7, 1.0)),
        ));
        let (stop_tx, mut stop_rx) = watch::channel(false);
        let mut restart = Box::pin(super::subscribe_for_forwarder_restart(
            &bus,
            &mut stop_rx,
            Duration::from_secs(5),
        ));

        // Poll once so the helper installs its receiver before the producer
        // publishes into the otherwise-empty broadcast bus.
        tokio::select! {
            biased;
            _ = &mut restart => panic!("restart backoff should still be pending"),
            _ = tokio::task::yield_now() => {}
        }

        let tenant_id = Uuid::new_v4();
        let envelope = EventEnvelope::new(
            tenant_id,
            None,
            DomainEvent::TenantUpdated { tenant_id },
        );
        bus.publish_envelope(envelope.clone())
            .expect("restart backoff must retain a bounded receiver");
        stop_tx.send_replace(true);

        let receiver = restart.await;
        let transport = Arc::new(RecordingTransport {
            accepted: Mutex::new(vec![]),
        });
        let (_drain_stop_tx, drain_stop_rx) = watch::channel(true);
        tokio::time::timeout(
            Duration::from_secs(3),
            super::run_event_forwarder(receiver, transport.clone(), drain_stop_rx),
        )
        .await
        .expect("shutdown iteration should drain the restart receiver");

        assert_eq!(
            transport.accepted.lock().unwrap().as_slice(),
            std::slice::from_ref(&envelope)
        );
        assert_eq!(
            bus.backpressure().expect("hard capacity guard").current_depth(),
            0
        );
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
    async fn event_bus_from_context_subscribes_before_returning_to_publishers() {
        use async_trait::async_trait;
        use rustok_core::events::{EventEnvelope, EventTransport, ReliabilityLevel};
        use rustok_events::DomainEvent;
        use uuid::Uuid;

        struct RecordingTransport {
            accepted: Mutex<Vec<EventEnvelope>>,
        }

        #[async_trait]
        impl EventTransport for RecordingTransport {
            async fn publish(&self, envelope: EventEnvelope) -> rustok_core::Result<()> {
                self.accepted.lock().unwrap().push(envelope);
                Ok(())
            }

            fn reliability_level(&self) -> ReliabilityLevel {
                ReliabilityLevel::InMemory
            }

            fn as_any(&self) -> &dyn std::any::Any {
                self
            }
        }

        let db = Database::connect("sqlite::memory:")
            .await
            .expect("connect sqlite");
        let ctx = ServerRuntimeContext::new(db, RustokSettings::default());
        let transport = Arc::new(RecordingTransport {
            accepted: Mutex::new(vec![]),
        });
        let transport_trait: Arc<dyn EventTransport> = transport.clone();
        ctx.shared_insert(transport_trait);

        let bus = event_bus_from_context(&ctx);
        let tenant_id = Uuid::new_v4();
        let envelope = EventEnvelope::new(
            tenant_id,
            None,
            DomainEvent::TenantUpdated { tenant_id },
        );
        bus.publish_envelope(envelope.clone())
            .expect("forwarder receiver must be installed before the bus is returned");

        tokio::time::timeout(Duration::from_secs(3), async {
            loop {
                if transport.accepted.lock().unwrap().as_slice() == std::slice::from_ref(&envelope) {
                    break;
                }
                tokio::task::yield_now().await;
            }
        })
        .await
        .expect("forwarder should persist the first event");

        let task = super::take_forwarder_task_for_shutdown(&ctx)
            .expect("forwarder should be registered");
        tokio::time::timeout(Duration::from_secs(3), task)
            .await
            .expect("forwarder should drain and stop")
            .expect("forwarder task should join");
    }

    #[tokio::test]
    async fn transactional_event_bus_from_context_initializes() {
        let db = Database::connect("sqlite::memory:")
            .await
            .expect("connect sqlite");
        let ctx = ServerRuntimeContext::new(db, RustokSettings::default());

        let _tx_bus = transactional_event_bus_from_context(&ctx);
        assert!(
            ctx.shared_get::<Arc<dyn rustok_core::events::EventTransport>>()
                .is_some()
        );
    }
}
