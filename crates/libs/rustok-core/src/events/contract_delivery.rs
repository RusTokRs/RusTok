//! Acknowledged sealed-family delivery for the existing module-listener bus.
//!
//! This is not a second queue: OutboxRelay retains durable work and retries until every
//! matching handler has completed. Remote cursor consumers remain transport-owned.

use std::fmt;
use std::sync::{
    Arc, RwLock, Weak,
    atomic::{AtomicBool, Ordering},
};
use std::time::Instant;

use rustok_events::{ContractEventEnvelope, ContractEventPayload};
use tokio::sync::Semaphore;

use super::{EventBus, EventHandler};
use crate::{Error, Result};

pub(super) type ContractListenerRegistry = Arc<RwLock<Option<Weak<ContractListenerSet>>>>;

pub(super) struct ContractListenerSet {
    handlers: Vec<Arc<dyn EventHandler>>,
    active: AtomicBool,
    capacity: Arc<Semaphore>,
}

impl fmt::Debug for ContractListenerSet {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("ContractListenerSet")
            .field("handler_count", &self.handlers.len())
            .field("active", &self.active.load(Ordering::Acquire))
            .finish()
    }
}

impl ContractListenerSet {
    pub(super) fn close(&self) {
        self.active.store(false, Ordering::Release);
    }

    fn require_active(&self) -> Result<()> {
        if !self.active.load(Ordering::Acquire) {
            return Err(Error::External(
                "sealed module listeners are not running".to_owned(),
            ));
        }
        Ok(())
    }

    fn handles(&self, payload: &ContractEventPayload) -> bool {
        self.handlers
            .iter()
            .any(|handler| handler.handles_contract(payload))
    }

    async fn deliver(&self, envelope: &ContractEventEnvelope) -> Result<()> {
        self.require_active()?;
        let payload = envelope.payload().map_err(invalid_envelope)?;
        let handlers = self
            .handlers
            .iter()
            .filter(|handler| handler.handles_contract(payload))
            .collect::<Vec<_>>();
        if handlers.is_empty() {
            return Err(Error::External(format!(
                "no module listener accepts sealed event `{}`",
                envelope.event_type()
            )));
        }
        // Refuse rather than creating an unbounded in-memory queue. The same canonical
        // outbox envelope remains pending and its configured relay policy owns retry.
        let _permit = Arc::clone(&self.capacity)
            .try_acquire_owned()
            .map_err(|_| {
                Error::External("sealed module listener capacity is exhausted".to_owned())
            })?;
        for handler in handlers {
            self.require_active()?;
            let started = Instant::now();
            let result = handler.handle_contract(envelope).await;
            rustok_telemetry::metrics::record_event_dispatched(
                envelope.event_type(),
                handler.name(),
            );
            rustok_telemetry::metrics::record_event_processing_duration(
                envelope.event_type(),
                handler.name(),
                started.elapsed().as_secs_f64(),
            );
            if result.is_err() {
                rustok_telemetry::metrics::record_event_error(
                    envelope.event_type(),
                    handler.name(),
                );
            }
            result?;
        }
        // If shutdown raced an in-flight idempotent projection, retain durable work instead
        // of acknowledging an ambiguous handoff. Replay refetches current state again.
        self.require_active()
    }
}

/// Owned by the dispatch task, so normal exit, abort and unwind close the binding.
pub(super) struct ContractListenerRegistration {
    pub(super) state: Arc<ContractListenerSet>,
}

impl Drop for ContractListenerRegistration {
    fn drop(&mut self) {
        self.state.close();
    }
}

impl EventBus {
    pub(super) fn bind_contract_listeners(
        &self,
        handlers: Vec<Arc<dyn EventHandler>>,
        max_concurrent: usize,
    ) -> Result<ContractListenerRegistration> {
        if max_concurrent > Semaphore::MAX_PERMITS {
            return Err(Error::Validation(
                "sealed listener concurrency exceeds the supported semaphore bound".to_owned(),
            ));
        }
        let mut registry = self
            .contract_listeners
            .write()
            .map_err(|_| poisoned_registry())?;
        if registry
            .as_ref()
            .and_then(Weak::upgrade)
            .is_some_and(|state| state.active.load(Ordering::Acquire))
        {
            return Err(Error::External(
                "sealed module listeners are already bound to this bus".to_owned(),
            ));
        }
        let state = Arc::new(ContractListenerSet {
            handlers,
            active: AtomicBool::new(true),
            capacity: Arc::new(Semaphore::new(max_concurrent.max(1))),
        });
        // The bus must not retain handlers that can themselves own an EventBus clone.
        // Only the running registration/dispatcher owns this set; avoid an Arc cycle.
        *registry = Some(Arc::downgrade(&state));
        Ok(ContractListenerRegistration { state })
    }

    fn contract_listener_set(&self) -> Result<Arc<ContractListenerSet>> {
        self.contract_listeners
            .read()
            .map_err(|_| poisoned_registry())?
            .as_ref()
            .and_then(Weak::upgrade)
            .ok_or_else(|| {
                Error::External(
                    "sealed module listeners are not bound to the delivery bus".to_owned(),
                )
            })
    }

    /// Queries a running explicit registry, not root broadcast subscriber counts.
    /// An unavailable registry is an error even for a remotely consumed family, so startup
    /// cannot accidentally skip a required local handler before registration completes.
    pub fn has_contract_handler(&self, envelope: &ContractEventEnvelope) -> Result<bool> {
        let payload = envelope.payload().map_err(invalid_envelope)?;
        let state = self.contract_listener_set()?;
        state.require_active()?;
        Ok(state.handles(payload))
    }

    /// Delivers the unchanged sealed envelope and awaits all matching module handlers.
    /// Root envelopes retain the established root-only broadcast path.
    pub async fn publish_contract_envelope(&self, envelope: ContractEventEnvelope) -> Result<()> {
        let payload = envelope.payload().map_err(invalid_envelope)?;
        if matches!(payload, ContractEventPayload::Root(_)) {
            return self.publish_envelope(envelope.into_root_envelope().map_err(invalid_envelope)?);
        }
        let state = self.contract_listener_set()?;
        state.deliver(&envelope).await?;
        self.record_contract_published(&envelope);
        Ok(())
    }
}

fn invalid_envelope(error: rustok_events::EventContractEnvelopeError) -> Error {
    Error::Validation(error.to_string())
}

fn poisoned_registry() -> Error {
    Error::External(
        "sealed module listener registry is unavailable; restart the runtime".to_owned(),
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use async_trait::async_trait;
    use rustok_events::{DomainEvent, EventEnvelope, InventoryReservationEvent};
    use std::sync::atomic::AtomicUsize;
    use tokio::sync::Notify;
    use uuid::Uuid;

    struct Listener {
        calls: Arc<AtomicUsize>,
        failure: bool,
        entered: Option<Arc<Notify>>,
        release: Option<Arc<Notify>>,
    }

    #[async_trait]
    impl EventHandler for Listener {
        fn name(&self) -> &'static str {
            "contract_delivery_regression"
        }
        fn handles(&self, _event: &DomainEvent) -> bool {
            false
        }
        async fn handle(&self, _envelope: &EventEnvelope) -> Result<()> {
            Err(Error::Validation(
                "bounded facts must not enter root handlers".to_owned(),
            ))
        }
        fn handles_contract(&self, payload: &ContractEventPayload) -> bool {
            matches!(payload, ContractEventPayload::InventoryReservation(_))
        }
        async fn handle_contract(&self, _envelope: &ContractEventEnvelope) -> Result<()> {
            self.calls.fetch_add(1, Ordering::SeqCst);
            if let Some(entered) = &self.entered {
                entered.notify_one();
            }
            if let Some(release) = &self.release {
                release.notified().await;
            }
            if self.failure {
                return Err(Error::External("projection failed".to_owned()));
            }
            Ok(())
        }
    }

    fn envelope() -> ContractEventEnvelope {
        ContractEventEnvelope::new(
            Uuid::new_v4(),
            Some(Uuid::new_v4()),
            InventoryReservationEvent::Changed {
                product_id: Uuid::new_v4(),
                variant_id: Uuid::new_v4(),
            },
        )
        .unwrap()
    }

    fn listener(calls: &Arc<AtomicUsize>, failure: bool) -> Arc<dyn EventHandler> {
        Arc::new(Listener {
            calls: Arc::clone(calls),
            failure,
            entered: None,
            release: None,
        })
    }

    #[tokio::test]
    async fn unbound_and_unmatched_sealed_delivery_are_not_success() {
        let bus = EventBus::new();
        // Invalid configuration refuses before acquiring/poisoning the registry lock.
        assert!(matches!(
            bus.bind_contract_listeners(vec![], usize::MAX),
            Err(Error::Validation(_))
        ));
        let oversized_queue = super::super::DispatcherConfig {
            max_queue_depth: usize::MAX,
            ..Default::default()
        };
        assert!(matches!(
            super::super::EventDispatcher::with_config(bus.clone(), oversized_queue)
                .start_with_contract_delivery(),
            Err(Error::Validation(_))
        ));
        assert!(bus.publish_contract_envelope(envelope()).await.is_err());
        let _binding = bus.bind_contract_listeners(vec![], 1).unwrap();
        assert!(!bus.has_contract_handler(&envelope()).unwrap());
        assert!(bus.publish_contract_envelope(envelope()).await.is_err());
    }

    #[tokio::test]
    async fn sealed_delivery_waits_for_handler_completion_and_preserves_root_isolation() {
        let bus = EventBus::new();
        let mut root = bus.subscribe();
        let calls = Arc::new(AtomicUsize::new(0));
        let entered = Arc::new(Notify::new());
        let release = Arc::new(Notify::new());
        let _binding = bus
            .bind_contract_listeners(
                vec![Arc::new(Listener {
                    calls: Arc::clone(&calls),
                    failure: false,
                    entered: Some(entered.clone()),
                    release: Some(release.clone()),
                })],
                1,
            )
            .unwrap();
        let task = tokio::spawn({
            let bus = bus.clone();
            async move { bus.publish_contract_envelope(envelope()).await }
        });
        entered.notified().await;
        assert!(!task.is_finished());
        assert!(root.try_recv().is_err());
        release.notify_one();
        task.await.unwrap().unwrap();
        assert_eq!(calls.load(Ordering::SeqCst), 1);
        assert_eq!(bus.stats().events_published(), 1);
    }

    #[tokio::test]
    async fn handler_failure_propagates_without_acknowledging_or_changing_envelope_identity() {
        let bus = EventBus::new();
        let calls = Arc::new(AtomicUsize::new(0));
        let _binding = bus
            .bind_contract_listeners(vec![listener(&calls, true)], 1)
            .unwrap();
        let envelope = envelope();
        for _ in 0..2 {
            assert!(
                bus.publish_contract_envelope(envelope.clone())
                    .await
                    .is_err()
            );
        }
        assert_eq!(calls.load(Ordering::SeqCst), 2);
        assert_eq!(bus.stats().events_published(), 0);
    }

    #[tokio::test]
    async fn closing_listener_binding_refuses_delivery_and_allows_explicit_rebind() {
        let bus = EventBus::new();
        let calls = Arc::new(AtomicUsize::new(0));
        let binding = bus
            .bind_contract_listeners(vec![listener(&calls, false)], 1)
            .unwrap();
        assert!(bus.bind_contract_listeners(vec![], 1).is_err());
        binding.state.close();
        assert!(bus.has_contract_handler(&envelope()).is_err());
        assert!(bus.publish_contract_envelope(envelope()).await.is_err());
        let _replacement = bus
            .bind_contract_listeners(vec![listener(&calls, false)], 1)
            .unwrap();
        drop(binding);
        bus.publish_contract_envelope(envelope()).await.unwrap();
        assert_eq!(calls.load(Ordering::SeqCst), 1);
    }

    #[tokio::test]
    async fn bounded_capacity_refuses_instead_of_queuing_another_fact() {
        let bus = EventBus::new();
        let calls = Arc::new(AtomicUsize::new(0));
        let entered = Arc::new(Notify::new());
        let release = Arc::new(Notify::new());
        let _binding = bus
            .bind_contract_listeners(
                vec![Arc::new(Listener {
                    calls,
                    failure: false,
                    entered: Some(entered.clone()),
                    release: Some(release.clone()),
                })],
                1,
            )
            .unwrap();
        let task = tokio::spawn({
            let bus = bus.clone();
            async move { bus.publish_contract_envelope(envelope()).await }
        });
        entered.notified().await;
        assert!(bus.publish_contract_envelope(envelope()).await.is_err());
        release.notify_one();
        task.await.unwrap().unwrap();
    }

    #[tokio::test]
    async fn dispatcher_abort_closes_contract_delivery_not_only_root_receiver() {
        let bus = EventBus::new();
        let calls = Arc::new(AtomicUsize::new(0));
        let mut dispatcher = super::super::EventDispatcher::new(bus.clone());
        dispatcher.register_boxed(listener(&calls, false));
        let running = dispatcher.start_with_contract_delivery().unwrap();
        bus.publish_contract_envelope(envelope()).await.unwrap();
        running.abort();
        assert!(bus.publish_contract_envelope(envelope()).await.is_err());
        assert_eq!(calls.load(Ordering::SeqCst), 1);
    }
}
