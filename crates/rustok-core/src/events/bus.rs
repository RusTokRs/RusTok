use std::sync::{
    Arc,
    atomic::{AtomicUsize, Ordering},
};

use tokio::sync::broadcast;
use uuid::Uuid;

use super::backpressure::BackpressureController;
use super::{DomainEvent, EventEnvelope};

const DEFAULT_CHANNEL_CAPACITY: usize = 128;

#[derive(Debug)]
pub struct EventBus {
    sender: broadcast::Sender<EventEnvelope>,
    stats: Arc<EventBusStats>,
    backpressure: Option<Arc<BackpressureController>>,
}

#[derive(Debug, Default)]
pub struct EventBusStats {
    events_published: AtomicUsize,
    events_dropped: AtomicUsize,
    subscribers: AtomicUsize,
}

impl EventBusStats {
    pub fn events_published(&self) -> usize {
        self.events_published.load(Ordering::Relaxed)
    }

    pub fn events_dropped(&self) -> usize {
        self.events_dropped.load(Ordering::Relaxed)
    }

    pub fn subscribers(&self) -> usize {
        self.subscribers.load(Ordering::Relaxed)
    }
}

impl EventBus {
    pub fn new() -> Self {
        Self::with_capacity(DEFAULT_CHANNEL_CAPACITY)
    }

    pub fn with_capacity(capacity: usize) -> Self {
        let (sender, _) = broadcast::channel(capacity);
        Self {
            sender,
            stats: Arc::new(EventBusStats::default()),
            backpressure: None,
        }
    }

    /// Creates an EventBus with backpressure control enabled
    pub fn with_backpressure(capacity: usize, backpressure: BackpressureController) -> Self {
        let (sender, _) = broadcast::channel(capacity);
        Self {
            sender,
            stats: Arc::new(EventBusStats::default()),
            backpressure: Some(Arc::new(backpressure)),
        }
    }

    pub fn stats(&self) -> Arc<EventBusStats> {
        Arc::clone(&self.stats)
    }

    /// Returns the backpressure controller if enabled
    pub fn backpressure(&self) -> Option<Arc<BackpressureController>> {
        self.backpressure.as_ref().map(Arc::clone)
    }

    pub fn subscribe(&self) -> broadcast::Receiver<EventEnvelope> {
        let receiver = self.sender.subscribe();
        self.stats
            .subscribers
            .store(self.sender.receiver_count(), Ordering::Relaxed);
        receiver
    }

    #[tracing::instrument(
        name = "eventbus.publish",
        skip(self, event),
        fields(
            event.type = %event.event_type(),
            tenant_id = %tenant_id,
            actor_id = tracing::field::Empty,
            event.id = tracing::field::Empty,
            otel.kind = "producer"
        )
    )]
    pub fn publish(
        &self,
        tenant_id: Uuid,
        actor_id: Option<Uuid>,
        event: DomainEvent,
    ) -> crate::Result<()> {
        let span = tracing::Span::current();

        if let Some(actor_id) = actor_id {
            span.record("actor_id", tracing::field::display(actor_id));
        }

        let envelope = EventEnvelope::new(tenant_id, actor_id, event);
        span.record("event.id", tracing::field::display(envelope.id));

        self.publish_envelope(envelope)
    }

    #[tracing::instrument(
        name = "eventbus.publish_envelope",
        skip(self, envelope),
        fields(
            event.type = %envelope.event.event_type(),
            event.id = %envelope.id,
            tenant_id = %envelope.tenant_id,
            backpressure.enabled = self.backpressure.is_some(),
            receiver_count = self.sender.receiver_count(),
            otel.kind = "producer"
        )
    )]
    pub fn publish_envelope(&self, envelope: EventEnvelope) -> crate::Result<()> {
        // Check backpressure if enabled
        if let Some(backpressure) = &self.backpressure {
            if let Err(e) = backpressure.try_acquire() {
                tracing::warn!(
                    error = %e,
                    event_type = envelope.event.event_type(),
                    "Event rejected due to backpressure"
                );
                self.stats.events_dropped.fetch_add(1, Ordering::Relaxed);
                return Err(crate::Error::External(format!(
                    "Event rejected due to backpressure: {}",
                    e
                )));
            }
        }

        if self.sender.receiver_count() == 0 {
            tracing::debug!(event = ?envelope.event, "Event published without subscribers");
        }

        match self.sender.send(envelope) {
            Ok(_) => {
                self.stats.events_published.fetch_add(1, Ordering::Relaxed);
                tracing::debug!("Event published successfully");
                Ok(())
            }
            Err(error) => {
                // Release backpressure slot on send failure
                if let Some(backpressure) = &self.backpressure {
                    backpressure.release();
                }
                self.stats.events_dropped.fetch_add(1, Ordering::Relaxed);
                let dropped_count = self.stats.events_dropped.load(Ordering::Relaxed);
                tracing::error!(
                    event_type = error.0.event.event_type(),
                    dropped_count,
                    "Event dropped - no receivers. CQRS indexes may become inconsistent!"
                );
                Err(crate::Error::External(format!(
                    "Event dropped (no receivers): {}. Total dropped: {}",
                    error.0.event.event_type(),
                    dropped_count,
                )))
            }
        }
    }
}

impl Default for EventBus {
    fn default() -> Self {
        Self::new()
    }
}

impl Clone for EventBus {
    fn clone(&self) -> Self {
        Self {
            sender: self.sender.clone(),
            stats: Arc::clone(&self.stats),
            backpressure: self.backpressure.as_ref().map(Arc::clone),
        }
    }
}
