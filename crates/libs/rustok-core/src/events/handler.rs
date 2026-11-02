use async_trait::async_trait;
use std::sync::Arc;
use std::time::Instant;
use tokio::sync::{Semaphore, broadcast, watch};
use tokio::task::{JoinHandle, JoinSet};
use tracing::{Instrument, debug, error, info, warn};

use super::bus::EventBus;
use super::consumer::EventConsumerRuntime;
use super::contract_delivery::{ContractListenerRegistration, ContractListenerSet};
use super::types::{DomainEvent, EventEnvelope};
use crate::Error;
use rustok_events::{ContractEventEnvelope, ContractEventPayload};

pub type HandlerResult = Result<(), Error>;

#[async_trait]
pub trait EventHandler: Send + Sync + 'static {
    fn name(&self) -> &'static str;

    fn handles(&self, event: &DomainEvent) -> bool;

    async fn handle(&self, envelope: &EventEnvelope) -> HandlerResult;

    /// Explicit opt-in to bounded typed families; root dispatch remains unchanged.
    fn handles_contract(&self, _payload: &ContractEventPayload) -> bool {
        false
    }

    /// Acknowledged by the canonical relay only after this returns success.
    /// Unsupported sealed delivery is a refusal, never a root-event conversion or noop.
    async fn handle_contract(&self, _envelope: &ContractEventEnvelope) -> HandlerResult {
        Err(Error::External(format!(
            "handler `{}` does not accept sealed contracts",
            self.name()
        )))
    }

    async fn on_error(&self, envelope: &EventEnvelope, error: &Error) {
        error!(
            handler = self.name(),
            event_type = envelope.event.event_type(),
            event_id = %envelope.id,
            error = %error,
            "Event handler error"
        );
    }
}

#[derive(Clone, Debug)]
pub struct DispatcherConfig {
    pub fail_fast: bool,
    pub max_concurrent: usize,
    pub retry_count: usize,
    pub retry_delay_ms: u64,
    pub max_queue_depth: usize,
}

impl Default for DispatcherConfig {
    fn default() -> Self {
        Self {
            fail_fast: false,
            max_concurrent: 10,
            retry_count: 0,
            retry_delay_ms: 100,
            max_queue_depth: 10000,
        }
    }
}

pub struct EventDispatcher {
    bus: EventBus,
    handlers: Vec<Arc<dyn EventHandler>>,
    config: DispatcherConfig,
}

impl EventDispatcher {
    pub fn new(bus: EventBus) -> Self {
        Self {
            bus,
            handlers: Vec::new(),
            config: DispatcherConfig::default(),
        }
    }

    pub fn with_config(bus: EventBus, config: DispatcherConfig) -> Self {
        Self {
            bus,
            handlers: Vec::new(),
            config,
        }
    }

    pub fn register<H: EventHandler>(&mut self, handler: H) -> &mut Self {
        info!(handler = handler.name(), "Registering event handler");
        self.handlers.push(Arc::new(handler));
        self
    }

    pub fn register_boxed(&mut self, handler: Arc<dyn EventHandler>) -> &mut Self {
        info!(handler = handler.name(), "Registering event handler");
        self.handlers.push(handler);
        self
    }

    pub fn handler_count(&self) -> usize {
        self.handlers.len()
    }

    pub fn start(self) -> RunningDispatcher {
        self.start_inner(None)
    }

    /// Starts the same module dispatcher with explicit acknowledged sealed delivery.
    /// Register all handlers before starting; delivery binds only the configured listener bus.
    pub fn start_with_contract_delivery(self) -> crate::Result<RunningDispatcher> {
        if self.config.max_queue_depth > Semaphore::MAX_PERMITS {
            return Err(Error::Validation(
                "module listener queue exceeds the supported semaphore bound".to_owned(),
            ));
        }
        let registration = self
            .bus
            .bind_contract_listeners(self.handlers.clone(), self.config.max_concurrent)?;
        Ok(self.start_inner(Some(registration)))
    }

    fn start_inner(self, registration: Option<ContractListenerRegistration>) -> RunningDispatcher {
        let contract_listeners = registration
            .as_ref()
            .map(|registration| Arc::clone(&registration.state));
        let handlers = Arc::new(self.handlers);
        let config = self.config;
        let mut receiver = self.bus.subscribe();
        let bus = self.bus.clone();
        let backpressure = bus.backpressure();
        let consumer_runtime = EventConsumerRuntime::new("event_dispatcher");
        let (stop_tx, mut stop_rx) = watch::channel(false);

        let handle = tokio::spawn(
            async move {
                // Retain the RAII registration for the task lifetime, including abort/unwind.
                let _contract_registration = registration;
                consumer_runtime.restarted("startup");
                info!(handlers = handlers.len(), "Event dispatcher started");
                let queue_capacity = config.max_queue_depth.max(1);
                if config.max_queue_depth == 0 {
                    warn!(
                        "Event dispatcher max_queue_depth was zero; using the minimum queue capacity of one"
                    );
                }

                let max_concurrent = config.max_concurrent.max(1);
                let semaphore = Arc::new(Semaphore::new(max_concurrent));
                let queue = Arc::new(Semaphore::new(queue_capacity));
                let mut in_flight = JoinSet::new();

                loop {
                    // Reap completed dispatch tasks so the JoinSet does not
                    // retain finished tasks for the lifetime of the process.
                    while let Some(result) = in_flight.try_join_next() {
                        if let Err(error) = result {
                            error!(%error, "Event dispatch task failed");
                        }
                    }

                    let queue_permit = tokio::select! {
                        _ = stop_rx.changed() => break,
                        permit = Arc::clone(&queue).acquire_owned() => match permit {
                            Ok(permit) => Arc::new(permit),
                            Err(_) => {
                                error!("Event dispatcher queue semaphore closed");
                                break;
                            }
                        },
                    };

                    let envelope = tokio::select! {
                        _ = stop_rx.changed() => break,
                        received = receiver.recv() => match received {
                            Ok(envelope) => envelope,
                            Err(broadcast::error::RecvError::Lagged(skipped)) => {
                                consumer_runtime.lagged(skipped);
                                continue;
                            }
                            Err(broadcast::error::RecvError::Closed) => {
                                consumer_runtime.closed();
                                break;
                            }
                        },
                    };

                    let span = tracing::info_span!(
                        "event_dispatch",
                        event_type = envelope.event.event_type(),
                        event_id = %envelope.id,
                        tenant_id = %envelope.tenant_id
                    );

                    let bp = backpressure.clone();
                    let handlers = handlers.clone();
                    let config = config.clone();
                    let semaphore = semaphore.clone();
                    let consumer_runtime = consumer_runtime;

                    in_flight.spawn(
                        async move {
                            Self::dispatch_to_handlers(
                                envelope,
                                handlers,
                                config,
                                semaphore,
                                bp,
                                consumer_runtime,
                                queue_permit,
                            )
                            .await;
                        }
                        .instrument(span),
                    );
                }

                // Cooperative drain: stop admitting new events, then wait for
                // already admitted dispatches (and their handler tasks) to
                // finish. Host shutdown owns the outer timeout/abort fallback.
                while let Some(result) = in_flight.join_next().await {
                    if let Err(error) = result {
                        error!(%error, "Event dispatch task failed during drain");
                    }
                }
                info!("Event dispatcher drained");
            }
            .in_current_span(),
        );

        RunningDispatcher {
            handle,
            bus,
            contract_listeners,
            stop_tx,
        }
    }

    async fn dispatch_to_handlers(
        envelope: EventEnvelope,
        handlers: Arc<Vec<Arc<dyn EventHandler>>>,
        config: DispatcherConfig,
        semaphore: Arc<Semaphore>,
        backpressure: Option<Arc<super::backpressure::BackpressureController>>,
        consumer_runtime: EventConsumerRuntime,
        queue_permit: Arc<tokio::sync::OwnedSemaphorePermit>,
    ) {
        let dispatch_started_at = Instant::now();
        let event_type = envelope.event.event_type().to_string();
        consumer_runtime.record_publish_lag(&event_type, envelope.timestamp);
        let matching_handlers: Vec<_> = handlers
            .iter()
            .filter(|handler| handler.handles(&envelope.event))
            .cloned()
            .collect();

        if matching_handlers.is_empty() {
            debug!(event_type = event_type.as_str(), "No handlers for event");
            // Release backpressure slot if no handlers
            if let Some(bp) = backpressure {
                bp.release();
            }
            consumer_runtime.record_dispatch_latency(&event_type, dispatch_started_at);
            return;
        }

        debug!(
            event_type = event_type.as_str(),
            handler_count = matching_handlers.len(),
            "Dispatching to handlers"
        );

        if config.fail_fast {
            for handler in matching_handlers {
                let envelope = envelope.clone();
                if let Err(error) = Self::handle_with_retry(handler, envelope, &config).await {
                    error!(
                        event_type = event_type.as_str(),
                        error = %error,
                        "Fail fast enabled, stopping dispatch after handler error"
                    );
                    break;
                }
            }
            // Release backpressure slot after all handlers complete
            if let Some(bp) = backpressure {
                bp.release();
            }
            consumer_runtime.record_dispatch_latency(&event_type, dispatch_started_at);
            return;
        }

        // Concurrent handlers are owned by this dispatch task. Joining them
        // here makes the dispatcher's in-flight JoinSet a real drain boundary
        // instead of leaking detached handler tasks past shutdown.
        let mut handler_tasks = JoinSet::new();
        for handler in matching_handlers {
            let envelope = envelope.clone();
            let config = config.clone();
            let semaphore = Arc::clone(&semaphore);
            let event_type = event_type.clone();
            let queue_permit = Arc::clone(&queue_permit);

            handler_tasks.spawn(async move {
                let Ok(_permit) = semaphore.acquire_owned().await else {
                    error!("Event dispatcher handler semaphore closed");
                    return;
                };
                let _queue_permit = queue_permit;
                if let Err(error) = Self::handle_with_retry(handler, envelope, &config).await {
                    debug!(
                        event_type = event_type.as_str(),
                        %error,
                        "Event handler finished with an error after retry policy"
                    );
                }
            });
        }

        while let Some(result) = handler_tasks.join_next().await {
            if let Err(error) = result {
                error!(%error, "Event handler task failed");
            }
        }

        if let Some(bp) = backpressure {
            bp.release();
        }
        consumer_runtime.record_dispatch_latency(&event_type, dispatch_started_at);
    }

    async fn handle_with_retry(
        handler: Arc<dyn EventHandler>,
        envelope: EventEnvelope,
        config: &DispatcherConfig,
    ) -> Result<(), Error> {
        let mut attempts = 0;
        let max_attempts = config.retry_count + 1;
        let event_type = envelope.event.event_type();
        let handler_name = handler.name();

        loop {
            attempts += 1;
            let attempt_started_at = Instant::now();
            let outcome = handler.handle(&envelope).await;
            // Every invocation of a named handler is one dispatch, so retries are
            // counted as separate dispatches and the duration histogram observes the
            // same population the dispatched counter counts.
            rustok_telemetry::metrics::record_event_dispatched(event_type, handler_name);
            rustok_telemetry::metrics::record_event_processing_duration(
                event_type,
                handler_name,
                attempt_started_at.elapsed().as_secs_f64(),
            );
            match outcome {
                Ok(()) => {
                    debug!(
                        handler = handler.name(),
                        event_type = envelope.event.event_type(),
                        "Handler completed successfully"
                    );
                    return Ok(());
                }
                Err(error) => {
                    if attempts < max_attempts {
                        warn!(
                            handler = handler.name(),
                            attempt = attempts,
                            max_attempts = max_attempts,
                            error = %error,
                            "Handler failed, retrying"
                        );
                        tokio::time::sleep(tokio::time::Duration::from_millis(
                            config.retry_delay_ms,
                        ))
                        .await;
                    } else {
                        handler.on_error(&envelope, &error).await;
                        return Err(error);
                    }
                }
            }
        }
    }
}

pub struct RunningDispatcher {
    contract_listeners: Option<Arc<ContractListenerSet>>,
    handle: JoinHandle<()>,
    bus: EventBus,
    stop_tx: watch::Sender<bool>,
}

impl RunningDispatcher {
    pub fn bus(&self) -> &EventBus {
        &self.bus
    }

    /// Stop admitting root and sealed events before draining root dispatches.
    pub fn request_stop(&self) {
        if let Some(listeners) = &self.contract_listeners {
            listeners.close();
        }
        self.stop_tx.send_replace(true);
    }

    /// Emergency abort. Prefer `request_stop` + `join` for graceful shutdown.
    pub fn abort(&self) {
        if let Some(listeners) = &self.contract_listeners {
            listeners.close();
        }
        self.handle.abort();
    }

    /// Abort handle usable after `join` has been moved into a timeout future.
    pub fn abort_handle(&self) -> tokio::task::AbortHandle {
        self.handle.abort_handle()
    }

    /// Request cooperative stop and wait for the dispatcher to drain.
    pub async fn stop(self) -> Result<(), tokio::task::JoinError> {
        self.request_stop();
        self.join().await
    }

    pub fn is_finished(&self) -> bool {
        self.handle.is_finished()
    }

    pub async fn join(self) -> Result<(), tokio::task::JoinError> {
        self.handle.await
    }
}

pub struct HandlerBuilder<F, Fut, P>
where
    F: Fn(EventEnvelope) -> Fut + Send + Sync + 'static,
    Fut: std::future::Future<Output = HandlerResult> + Send + Sync + 'static,
    P: Fn(&DomainEvent) -> bool + Send + Sync + 'static,
{
    name: &'static str,
    predicate: P,
    handler: F,
    _phantom: std::marker::PhantomData<Fut>,
}

impl<F, Fut, P> HandlerBuilder<F, Fut, P>
where
    F: Fn(EventEnvelope) -> Fut + Send + Sync + 'static,
    Fut: std::future::Future<Output = HandlerResult> + Send + Sync + 'static,
    P: Fn(&DomainEvent) -> bool + Send + Sync + 'static,
{
    pub fn new(name: &'static str, predicate: P, handler: F) -> Self {
        Self {
            name,
            predicate,
            handler,
            _phantom: std::marker::PhantomData,
        }
    }
}

#[async_trait]
impl<F, Fut, P> EventHandler for HandlerBuilder<F, Fut, P>
where
    F: Fn(EventEnvelope) -> Fut + Send + Sync + 'static,
    Fut: std::future::Future<Output = HandlerResult> + Send + Sync + 'static,
    P: Fn(&DomainEvent) -> bool + Send + Sync + 'static,
{
    fn name(&self) -> &'static str {
        self.name
    }

    fn handles(&self, event: &DomainEvent) -> bool {
        (self.predicate)(event)
    }

    async fn handle(&self, envelope: &EventEnvelope) -> HandlerResult {
        (self.handler)(envelope.clone()).await
    }
}

#[macro_export]
macro_rules! event_handler {
    ($name:expr, $predicate:expr, $handler:expr) => {
        $crate::events::handler::HandlerBuilder::new($name, $predicate, $handler)
    };
}

#[cfg(test)]
mod tests {
    use super::*;
    use async_trait::async_trait;
    use std::sync::atomic::{AtomicUsize, Ordering};
    use tokio::sync::Notify;
    use uuid::Uuid;

    fn test_envelope() -> EventEnvelope {
        EventEnvelope::new(
            Uuid::new_v4(),
            None,
            DomainEvent::IndexUpdated {
                index_name: "products".to_string(),
                target_id: Uuid::new_v4(),
            },
        )
    }

    #[derive(Debug)]
    struct CountingHandler {
        attempts: Arc<AtomicUsize>,
        fail_until_attempt: usize,
        error_notifications: Arc<AtomicUsize>,
    }

    #[async_trait]
    impl EventHandler for CountingHandler {
        fn name(&self) -> &'static str {
            "counting_handler"
        }

        fn handles(&self, event: &DomainEvent) -> bool {
            matches!(event, DomainEvent::IndexUpdated { .. })
        }

        async fn handle(&self, _envelope: &EventEnvelope) -> HandlerResult {
            let attempt = self.attempts.fetch_add(1, Ordering::SeqCst) + 1;
            if attempt <= self.fail_until_attempt {
                Err(Error::External(format!("attempt {attempt} failed")))
            } else {
                Ok(())
            }
        }

        async fn on_error(&self, _envelope: &EventEnvelope, _error: &Error) {
            self.error_notifications.fetch_add(1, Ordering::SeqCst);
        }
    }

    #[tokio::test]
    async fn handle_with_retry_stops_after_successful_retry_without_on_error() {
        let attempts = Arc::new(AtomicUsize::new(0));
        let error_notifications = Arc::new(AtomicUsize::new(0));
        let handler = Arc::new(CountingHandler {
            attempts: Arc::clone(&attempts),
            fail_until_attempt: 1,
            error_notifications: Arc::clone(&error_notifications),
        });
        let config = DispatcherConfig {
            retry_count: 2,
            retry_delay_ms: 0,
            ..DispatcherConfig::default()
        };

        EventDispatcher::handle_with_retry(handler, test_envelope(), &config)
            .await
            .unwrap();

        assert_eq!(attempts.load(Ordering::SeqCst), 2);
        assert_eq!(error_notifications.load(Ordering::SeqCst), 0);
    }

    #[tokio::test]
    async fn dispatcher_respects_max_queue_depth_before_receiving_more_events() {
        use std::sync::atomic::{AtomicUsize, Ordering};
        use tokio::sync::Notify;

        struct BlockingHandler {
            started: Arc<AtomicUsize>,
            release: Arc<Notify>,
        }

        #[async_trait]
        impl EventHandler for BlockingHandler {
            fn name(&self) -> &'static str {
                "blocking_queue_test"
            }

            fn handles(&self, _event: &DomainEvent) -> bool {
                true
            }

            async fn handle(&self, _envelope: &EventEnvelope) -> HandlerResult {
                self.started.fetch_add(1, Ordering::SeqCst);
                self.release.notified().await;
                Ok(())
            }
        }

        let bus = EventBus::new();
        let started = Arc::new(AtomicUsize::new(0));
        let release = Arc::new(Notify::new());
        let mut dispatcher = EventDispatcher::with_config(
            bus.clone(),
            DispatcherConfig {
                max_concurrent: 1,
                max_queue_depth: 1,
                ..DispatcherConfig::default()
            },
        );
        dispatcher.register(BlockingHandler {
            started: Arc::clone(&started),
            release: Arc::clone(&release),
        });
        let running = dispatcher.start();

        bus.publish(
            Uuid::new_v4(),
            None,
            DomainEvent::IndexUpdated {
                index_name: "products".to_string(),
                target_id: Uuid::new_v4(),
            },
        )
        .expect("first event publish");
        while started.load(Ordering::SeqCst) == 0 {
            tokio::task::yield_now().await;
        }

        bus.publish(
            Uuid::new_v4(),
            None,
            DomainEvent::IndexUpdated {
                index_name: "products".to_string(),
                target_id: Uuid::new_v4(),
            },
        )
        .expect("second event publish");
        for _ in 0..10 {
            tokio::task::yield_now().await;
        }
        assert_eq!(started.load(Ordering::SeqCst), 1);

        release.notify_one();
        for _ in 0..100 {
            if started.load(Ordering::SeqCst) == 2 {
                break;
            }
            tokio::task::yield_now().await;
        }
        assert_eq!(started.load(Ordering::SeqCst), 2);
        // This test intentionally leaves the second handler blocked; use the
        // emergency abort path instead of cooperative drain.
        running.abort();
        assert!(running.join().await.is_err());
    }

    #[tokio::test]
    async fn handle_with_retry_calls_on_error_after_retry_budget_is_exhausted() {
        let attempts = Arc::new(AtomicUsize::new(0));
        let error_notifications = Arc::new(AtomicUsize::new(0));
        let handler = Arc::new(CountingHandler {
            attempts: Arc::clone(&attempts),
            fail_until_attempt: usize::MAX,
            error_notifications: Arc::clone(&error_notifications),
        });
        let config = DispatcherConfig {
            retry_count: 2,
            retry_delay_ms: 0,
            ..DispatcherConfig::default()
        };

        let error = EventDispatcher::handle_with_retry(handler, test_envelope(), &config)
            .await
            .unwrap_err();

        assert!(error.to_string().contains("attempt 3 failed"));
        assert_eq!(attempts.load(Ordering::SeqCst), 3);
        assert_eq!(error_notifications.load(Ordering::SeqCst), 1);
    }

    #[tokio::test]
    async fn dispatch_releases_backpressure_when_no_handlers_match() {
        let controller = Arc::new(super::super::backpressure::BackpressureController::new(
            super::super::backpressure::BackpressureConfig::new(4, 0.5, 1.0),
        ));
        controller.try_acquire().unwrap();

        let queue_permit = Arc::new(Arc::new(Semaphore::new(1)).acquire_owned().await.unwrap());

        EventDispatcher::dispatch_to_handlers(
            test_envelope(),
            Arc::new(Vec::new()),
            DispatcherConfig::default(),
            Arc::new(Semaphore::new(1)),
            Some(Arc::clone(&controller)),
            EventConsumerRuntime::new("test_dispatcher"),
            queue_permit,
        )
        .await;

        assert_eq!(controller.current_depth(), 0);
    }

    #[tokio::test]
    async fn concurrent_dispatch_releases_backpressure_after_all_handlers_finish() {
        let controller = Arc::new(super::super::backpressure::BackpressureController::new(
            super::super::backpressure::BackpressureConfig::new(4, 0.5, 1.0),
        ));
        controller.try_acquire().unwrap();

        let first_started = Arc::new(Notify::new());
        let allow_first_finish = Arc::new(Notify::new());
        let handled = Arc::new(AtomicUsize::new(0));

        let first_started_for_handler = Arc::clone(&first_started);
        let allow_first_finish_for_handler = Arc::clone(&allow_first_finish);
        let handled_for_first = Arc::clone(&handled);
        let first = HandlerBuilder::new(
            "first",
            |_| true,
            move |_| {
                let first_started = Arc::clone(&first_started_for_handler);
                let allow_first_finish = Arc::clone(&allow_first_finish_for_handler);
                let handled = Arc::clone(&handled_for_first);
                async move {
                    first_started.notify_one();
                    allow_first_finish.notified().await;
                    handled.fetch_add(1, Ordering::SeqCst);
                    Ok(())
                }
            },
        );

        let handled_for_second = Arc::clone(&handled);
        let second = HandlerBuilder::new(
            "second",
            |_| true,
            move |_| {
                let handled = Arc::clone(&handled_for_second);
                async move {
                    handled.fetch_add(1, Ordering::SeqCst);
                    Ok(())
                }
            },
        );

        let queue_permit = Arc::new(Arc::new(Semaphore::new(1)).acquire_owned().await.unwrap());

        let dispatch = tokio::spawn(EventDispatcher::dispatch_to_handlers(
            test_envelope(),
            Arc::new(vec![
                Arc::new(first) as Arc<dyn EventHandler>,
                Arc::new(second) as Arc<dyn EventHandler>,
            ]),
            DispatcherConfig {
                max_concurrent: 2,
                retry_delay_ms: 0,
                ..DispatcherConfig::default()
            },
            Arc::new(Semaphore::new(2)),
            Some(Arc::clone(&controller)),
            EventConsumerRuntime::new("test_dispatcher"),
            queue_permit,
        ));

        first_started.notified().await;
        assert_eq!(controller.current_depth(), 1);

        allow_first_finish.notify_one();
        dispatch.await.unwrap();

        for _ in 0..10 {
            if controller.current_depth() == 0 && handled.load(Ordering::SeqCst) == 2 {
                break;
            }
            tokio::task::yield_now().await;
        }

        assert_eq!(handled.load(Ordering::SeqCst), 2);
        assert_eq!(controller.current_depth(), 0);
    }

    #[tokio::test]
    async fn running_dispatcher_lifecycle_abort_and_is_finished() {
        let bus = EventBus::new();
        let dispatcher = EventDispatcher::new(bus);
        let running = dispatcher.start();

        assert!(!running.is_finished());
        running.abort();

        // After abort, join yields cancelled/join error
        let join_result = running.join().await;
        assert!(join_result.is_err());
    }

    #[tokio::test]
    async fn cooperative_stop_drains_in_flight_handlers_before_join_returns() {
        struct BlockingHandler {
            started: Arc<Notify>,
            release: Arc<Notify>,
            completed: Arc<AtomicUsize>,
        }

        #[async_trait]
        impl EventHandler for BlockingHandler {
            fn name(&self) -> &'static str {
                "cooperative_drain_handler"
            }

            fn handles(&self, _event: &DomainEvent) -> bool {
                true
            }

            async fn handle(&self, _envelope: &EventEnvelope) -> HandlerResult {
                self.started.notify_one();
                self.release.notified().await;
                self.completed.fetch_add(1, Ordering::SeqCst);
                Ok(())
            }
        }

        let bus = EventBus::new();
        let started = Arc::new(Notify::new());
        let release = Arc::new(Notify::new());
        let completed = Arc::new(AtomicUsize::new(0));
        let mut dispatcher = EventDispatcher::new(bus.clone());
        dispatcher.register(BlockingHandler {
            started: Arc::clone(&started),
            release: Arc::clone(&release),
            completed: Arc::clone(&completed),
        });
        let running = dispatcher.start();

        bus.publish(
            Uuid::new_v4(),
            None,
            DomainEvent::IndexUpdated {
                index_name: "products".to_string(),
                target_id: Uuid::new_v4(),
            },
        )
        .expect("publish drain test event");
        started.notified().await;

        running.request_stop();
        assert!(
            !running.is_finished(),
            "stop must wait for the admitted handler instead of aborting it"
        );
        release.notify_one();

        running.stop().await.expect("cooperative dispatcher stop");
        assert_eq!(completed.load(Ordering::SeqCst), 1);
    }
}
