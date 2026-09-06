//! Event bus testing utilities
//!
//! Provides a mock event bus for testing event publishing and handling.

use rustok_core::{EventBus, EventTransport, ReliabilityLevel};
use rustok_events::{DomainEvent, EventEnvelope};
use rustok_outbox::TransactionalEventBus;
use std::any::Any;
use std::sync::{Arc, Mutex};
use uuid::Uuid;

/// Mock event transport that records events for testing.
#[derive(Debug, Clone)]
pub struct MockEventTransport {
    recorded_events: Arc<Mutex<Vec<RecordedEvent>>>,
}

#[derive(Debug, Clone)]
struct RecordedEvent {
    pub tenant_id: Uuid,
    pub event_type: String,
    pub event: DomainEvent,
}

#[async_trait::async_trait]
impl EventTransport for MockEventTransport {
    async fn publish(&self, envelope: EventEnvelope) -> rustok_core::Result<()> {
        let event_type = event_type_name(&envelope.event);
        let recorded = RecordedEvent {
            tenant_id: envelope.tenant_id,
            event_type,
            event: envelope.event.clone(),
        };
        {
            let mut events = self.recorded_events.lock().unwrap();
            events.push(recorded);
        }
        Ok(())
    }

    fn reliability_level(&self) -> ReliabilityLevel {
        ReliabilityLevel::Outbox
    }

    fn as_any(&self) -> &dyn Any {
        self
    }
}

impl MockEventTransport {
    pub fn new() -> Self {
        Self {
            recorded_events: Arc::new(Mutex::new(Vec::new())),
        }
    }

    pub fn event_count(&self) -> usize {
        self.recorded_events.lock().unwrap().len()
    }

    pub fn has_event_of_type(&self, event_type: &str) -> bool {
        self.recorded_events
            .lock()
            .unwrap()
            .iter()
            .any(|e| e.event_type == event_type)
    }

    pub fn events_of_type(&self, event_type: &str) -> Vec<DomainEvent> {
        self.recorded_events
            .lock()
            .unwrap()
            .iter()
            .filter(|e| e.event_type == event_type)
            .map(|e| e.event.clone())
            .collect()
    }

    pub fn clear(&self) {
        self.recorded_events.lock().unwrap().clear();
    }

    pub fn is_empty(&self) -> bool {
        self.recorded_events.lock().unwrap().is_empty()
    }
}

impl Default for MockEventTransport {
    fn default() -> Self {
        Self::new()
    }
}

/// Creates a new TransactionalEventBus for testing.
///
/// This is a convenience function for creating a TransactionalEventBus
/// that records events without requiring a real database.
///
/// # Example
///
/// ```rust
/// use rustok_test_utils::mock_transactional_event_bus;
/// use rustok_outbox::TransactionalEventBus;
///
/// let bus: TransactionalEventBus = mock_transactional_event_bus();
/// ```
pub fn mock_transactional_event_bus() -> TransactionalEventBus {
    let transport = Arc::new(MockEventTransport::new());
    TransactionalEventBus::new(transport)
}

/// A mock event bus that records all published events for verification.
///
/// This is useful for testing that events are published correctly without
/// actually dispatching them to real handlers.
///
/// # Example
///
/// ```rust
/// use rustok_test_utils::MockEventBus;
/// use rustok_core::{DomainEvent, EventBus};
/// use uuid::Uuid;
///
/// #[tokio::test]
/// async fn test_event_publishing() {
///     let mock_bus = MockEventBus::new();
///     let tenant_id = Uuid::new_v4();
///
///     // Publish an event
///     mock_bus.publish(tenant_id, None, DomainEvent::NodeCreated {
///         id: Uuid::new_v4(),
///         kind: "post".to_string(),
///         tenant_id,
///     }).unwrap();
///
///     // Verify event was recorded
///     assert_eq!(mock_bus.event_count(), 1);
///     assert!(mock_bus.has_event_of_type("NodeCreated"));
/// }
/// ```
#[derive(Debug, Clone)]
pub struct MockEventBus {
    inner: EventBus,
    recorded_events: Arc<Mutex<Vec<RecordedEvent>>>,
}

impl MockEventBus {
    /// Creates a new mock event bus.
    pub fn new() -> Self {
        Self {
            inner: EventBus::new(),
            recorded_events: Arc::new(Mutex::new(Vec::new())),
        }
    }

    /// Creates a new mock event bus with a specific channel capacity.
    pub fn with_capacity(capacity: usize) -> Self {
        Self {
            inner: EventBus::with_capacity(capacity),
            recorded_events: Arc::new(Mutex::new(Vec::new())),
        }
    }

    /// Publishes an event and records it for later verification.
    pub fn publish(
        &self,
        tenant_id: Uuid,
        actor_id: Option<Uuid>,
        event: DomainEvent,
    ) -> rustok_core::Result<()> {
        let event_type = event_type_name(&event);

        let recorded = RecordedEvent {
            tenant_id,
            event_type,
            event: event.clone(),
        };

        {
            let mut events = self.recorded_events.lock().unwrap();
            events.push(recorded);
        }

        self.inner.publish(tenant_id, actor_id, event)
    }

    /// Returns the number of events that have been published.
    pub fn event_count(&self) -> usize {
        self.recorded_events.lock().unwrap().len()
    }

    /// Returns true if any event of the given type has been published.
    pub fn has_event_of_type(&self, event_type: &str) -> bool {
        self.recorded_events
            .lock()
            .unwrap()
            .iter()
            .any(|e| e.event_type == event_type)
    }

    /// Returns all recorded events of a specific type.
    pub fn events_of_type(&self, event_type: &str) -> Vec<DomainEvent> {
        self.recorded_events
            .lock()
            .unwrap()
            .iter()
            .filter(|e| e.event_type == event_type)
            .map(|e| e.event.clone())
            .collect()
    }

    /// Returns all recorded events for a specific tenant.
    pub fn events_for_tenant(&self, tenant_id: Uuid) -> Vec<DomainEvent> {
        self.recorded_events
            .lock()
            .unwrap()
            .iter()
            .filter(|e| e.tenant_id == tenant_id)
            .map(|e| e.event.clone())
            .collect()
    }

    /// Clears all recorded events.
    pub fn clear(&self) {
        self.recorded_events.lock().unwrap().clear();
    }

    /// Returns true if no events have been published.
    pub fn is_empty(&self) -> bool {
        self.recorded_events.lock().unwrap().is_empty()
    }

    /// Returns a copy of all recorded events.
    pub fn all_events(&self) -> Vec<DomainEvent> {
        self.recorded_events
            .lock()
            .unwrap()
            .iter()
            .map(|e| e.event.clone())
            .collect()
    }
}

impl Default for MockEventBus {
    fn default() -> Self {
        Self::new()
    }
}

/// Creates a new mock event bus.
///
/// This is a convenience function for creating a MockEventBus.
///
/// # Example
///
/// ```rust
/// use rustok_test_utils::mock_event_bus;
///
/// let bus = mock_event_bus();
/// ```
pub fn mock_event_bus() -> MockEventBus {
    MockEventBus::new()
}

/// Returns the type name of a DomainEvent as a string.
fn event_type_name(event: &DomainEvent) -> String {
    event
        .event_type()
        .split('.')
        .map(|segment| {
            let mut chars = segment.chars();
            match chars.next() {
                Some(first) => first.to_ascii_uppercase().to_string() + chars.as_str(),
                None => String::new(),
            }
        })
        .collect::<String>()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_mock_event_bus() {
        let bus = MockEventBus::new();
        let tenant_id = Uuid::new_v4();

        assert!(bus.is_empty());
        assert_eq!(bus.event_count(), 0);

        bus.publish(
            tenant_id,
            None,
            DomainEvent::NodeCreated {
                node_id: Uuid::new_v4(),
                kind: "post".to_string(),
                author_id: Some(Uuid::new_v4()),
            },
        )
        .ok();

        assert!(!bus.is_empty());
        assert_eq!(bus.event_count(), 1);
        assert!(bus.has_event_of_type("NodeCreated"));
        assert!(!bus.has_event_of_type("NodeUpdated"));
    }

    #[test]
    fn test_event_filtering() {
        let bus = MockEventBus::new();
        let tenant_id = Uuid::new_v4();

        bus.publish(
            tenant_id,
            None,
            DomainEvent::NodeCreated {
                node_id: Uuid::new_v4(),
                kind: "post".to_string(),
                author_id: Some(Uuid::new_v4()),
            },
        )
        .ok();

        bus.publish(
            tenant_id,
            None,
            DomainEvent::NodeUpdated {
                node_id: Uuid::new_v4(),
                kind: "post".to_string(),
            },
        )
        .ok();

        assert_eq!(bus.events_of_type("NodeCreated").len(), 1);
        assert_eq!(bus.events_of_type("NodeUpdated").len(), 1);
    }

    #[test]
    fn test_clear_events() {
        let bus = MockEventBus::new();
        let tenant_id = Uuid::new_v4();

        bus.publish(
            tenant_id,
            None,
            DomainEvent::NodeCreated {
                node_id: Uuid::new_v4(),
                kind: "post".to_string(),
                author_id: Some(Uuid::new_v4()),
            },
        )
        .ok();

        assert_eq!(bus.event_count(), 1);

        bus.clear();

        assert!(bus.is_empty());
        assert_eq!(bus.event_count(), 0);
    }
}
