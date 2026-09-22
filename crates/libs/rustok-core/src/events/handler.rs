use async_trait::async_trait;
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::Instant;
use tokio::sync::{Semaphore, broadcast};
use tokio::task::JoinHandle;
use tracing::{Instrument, debug, error, info, warn};

use super::bus::EventBus;
use super::consumer::EventConsumerRuntime;
use super::types::{DomainEvent, EventEnvelope};
use crate::Error;

pub type HandlerResult = Result<(), Error>;

#[async_trait]
pub trait EventHandler: Send + Sync + 'static {
    fn name(&self) -> &'static str;

    fn handles(&self, event: &DomainEvent) -> bool;

    async fn handle(&self, envelope: &EventEnvelope) -> HandlerResult;

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
        let handlers = Arc::new(self.handlers);
        let config = self.config;
        let mut receiver = self.bus.subscribe();
        let bus = self.bus.clone();
        let backpressure = bus.backpressure();
        let consumer_runtime = EventConsumerRuntime::new("event_dispatcher");

        let handle = tokio::spawn(
            async move {
                consumer_runtime.restarted("startup");
                info!(handlers = handlers.len(), "Event dispatcher started");
                let max_concurrent = config.max_concurrent.max(1);
                let semaphore = Arc::new(Semaphore::new(max_concurrent));

                loop {
                    match receiver.recv().await {
                        Ok(envelope) => {
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

                            tokio::spawn(
                                async move {
                                    Self::dispatch_to_handlers(
                                        envelope,
                                        handlers,
                                        config,
                                        semaphore,
                                        bp,
                                        consumer_runtime,
                                    )
                                    .await;
                                }
                                .instrument(span),
                            );
                        }
                        Err(broadcast::error::RecvError::Lagged(skipped)) => {
                            consumer_runtime.lagged(skipped);
                        }
                        Err(broadcast::error::RecvError::Closed) => {
                            consumer_runtime.closed();
                            break;
                        }
                    }
                }
            }
            .in_current_span(),
        );

        RunningDispatcher { handle, bus }
    }

    async fn dispatch_to_handlers(
        envelope: EventEnvelope,
        handlers: Arc<Vec<Arc<dyn EventHandler>>>,
        config: DispatcherConfig,
        semaphore: Arc<Semaphore>,
        backpressure: Option<Arc<super::backpressure::BackpressureController>>,
        consumer_runtime: EventConsumerRuntime,
    ) {
        let dispatch_started_at = Instant::now();
        let event_type = envelope.event.event_type().to_string();
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

        let handler_count = matching_handlers.len();

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

        // For concurrent execution, track handler completion
        let completion_count = Arc::new(AtomicUsize::new(0));

        for handler in matching_handlers {
            let envelope = envelope.clone();
            let config = config.clone();
            let permit = semaphore.clone().acquire_owned().await;
            let bp = backpressure.clone();
            let count = Arc::clone(&completion_count);
            let event_type = event_type.clone();

            tokio::spawn(async move {
                let _permit = permit;

                struct CompletionGuard {
                    count: Arc<AtomicUsize>,
                    limit: usize,
                    bp: Option<Arc<super::backpressure::BackpressureController>>,
                    consumer_runtime: EventConsumerRuntime,
                    event_type: String,
                    dispatch_started_at: Instant,
                }

                impl Drop for CompletionGuard {
                    fn drop(&mut self) {
                        let completed = self.count.fetch_add(1, Ordering::Relaxed) + 1;
                        if completed == self.limit {
                            if let Some(bp) = &self.bp {
                                bp.release();
                            }
                            self.consumer_runtime.record_dispatch_latency(
                                &self.event_type,
                                self.dispatch_started_at,
                            );
                        }
                    }
                }

                let _guard = CompletionGuard {
                    count,
                    limit: handler_count,
                    bp,
                    consumer_runtime,
                    event_type,
                    dispatch_started_at,
                };

                let _ = Self::handle_with_retry(handler, envelope, &config).await;
            });
        }
    }

    async fn handle_with_retry(
        handler: Arc<dyn EventHandler>,
        envelope: EventEnvelope,
        config: &DispatcherConfig,
    ) -> Result<(), Error> {
        let mut attempts = 0;
        let max_attempts = config.retry_count + 1;

        loop {
            attempts += 1;
            match handler.handle(&envelope).await {
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
    handle: JoinHandle<()>,
    bus: EventBus,
}

impl RunningDispatcher {
    pub fn bus(&self) -> &EventBus {
        &self.bus
    }

    pub fn stop(self) {
        self.handle.abort();
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

        EventDispatcher::dispatch_to_handlers(
            test_envelope(),
            Arc::new(Vec::new()),
            DispatcherConfig::default(),
            Arc::new(Semaphore::new(1)),
            Some(Arc::clone(&controller)),
            EventConsumerRuntime::new("test_dispatcher"),
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
}
