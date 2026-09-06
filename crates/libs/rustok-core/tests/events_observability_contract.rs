use rustok_core::events::{
    BackpressureConfig, BackpressureController, BackpressureError, BackpressureState, DomainEvent,
    EventBus, EventTransport, MemoryTransport, ReliabilityLevel,
};
use uuid::Uuid;

fn test_event() -> DomainEvent {
    DomainEvent::IndexUpdated {
        index_name: "products".to_string(),
        target_id: Uuid::new_v4(),
    }
}

#[test]
fn backpressure_release_is_saturating_and_keeps_metrics_stable() {
    let controller = BackpressureController::new(BackpressureConfig::new(10, 0.5, 0.8));

    controller.release();
    let metrics = controller.metrics();
    assert_eq!(metrics.current_depth, 0);
    assert_eq!(metrics.events_accepted, 0);
    assert_eq!(metrics.events_rejected, 0);
    assert_eq!(metrics.state, BackpressureState::Normal);

    controller.try_acquire().unwrap();
    controller.release();
    controller.release();

    assert_eq!(controller.current_depth(), 0);
    assert_eq!(controller.metrics().events_accepted, 1);
}

#[test]
fn backpressure_reports_warning_and_critical_observability_metrics() {
    let controller = BackpressureController::new(BackpressureConfig::new(10, 0.5, 0.8));

    for _ in 0..5 {
        controller.try_acquire().unwrap();
    }

    let warning = controller.metrics();
    assert_eq!(warning.current_depth, 5);
    assert_eq!(warning.state, BackpressureState::Warning);
    assert_eq!(warning.warning_count, 0);
    assert_eq!(warning.events_accepted, 5);

    for _ in 0..3 {
        controller.try_acquire().unwrap();
    }

    let critical_rejection = controller.try_acquire().unwrap_err();
    assert!(matches!(
        critical_rejection,
        BackpressureError::QueueFull {
            current: 8,
            max: 10
        }
    ));

    let critical = controller.metrics();
    assert_eq!(critical.current_depth, 8);
    assert_eq!(critical.state, BackpressureState::Critical);
    assert_eq!(critical.events_accepted, 8);
    assert_eq!(critical.events_rejected, 1);
    assert_eq!(critical.warning_count, 3);
    assert_eq!(critical.critical_count, 1);
}

#[test]
fn event_bus_stats_track_successful_publish_and_backpressure_rejection() {
    let tenant_id = Uuid::new_v4();
    let actor_id = Some(Uuid::new_v4());
    let backpressure = BackpressureController::new(BackpressureConfig::new(2, 0.5, 1.0));
    let bus = EventBus::with_backpressure(8, backpressure);
    let mut subscriber = bus.subscribe();

    bus.publish(tenant_id, actor_id, test_event()).unwrap();

    let received = subscriber.try_recv().unwrap();
    assert_eq!(received.tenant_id, tenant_id);
    assert_eq!(received.actor_id, actor_id);
    assert_eq!(received.event_type, "index.updated");
    assert_eq!(bus.stats().events_published(), 1);
    assert_eq!(bus.stats().events_dropped(), 0);
    assert_eq!(bus.stats().subscribers(), 1);

    bus.publish(tenant_id, None, test_event()).unwrap();
    assert_eq!(subscriber.try_recv().unwrap().event_type, "index.updated");

    let rejected = bus.publish(tenant_id, None, test_event()).unwrap_err();
    assert!(rejected.to_string().contains("backpressure"));
    assert_eq!(bus.stats().events_published(), 2);
    assert_eq!(bus.stats().events_dropped(), 1);

    let bp_metrics = bus.backpressure().unwrap().metrics();
    assert_eq!(bp_metrics.current_depth, 2);
    assert_eq!(bp_metrics.events_accepted, 2);
    assert_eq!(bp_metrics.events_rejected, 1);
}

#[tokio::test]
async fn memory_transport_exposes_in_memory_reliability_and_batch_stats() {
    let tenant_id = Uuid::new_v4();
    let transport = MemoryTransport::with_capacity(8);
    let mut subscriber = transport.subscribe();

    assert_eq!(transport.reliability_level(), ReliabilityLevel::InMemory);

    let first = rustok_core::events::EventEnvelope::new(tenant_id, None, test_event());
    let second = rustok_core::events::EventEnvelope::new(tenant_id, None, test_event());
    let first_id = first.id;
    let second_id = second.id;

    transport.publish_batch(vec![first, second]).await.unwrap();

    assert_eq!(subscriber.recv().await.unwrap().id, first_id);
    assert_eq!(subscriber.recv().await.unwrap().id, second_id);
    assert_eq!(transport.stats().events_published(), 2);
    assert_eq!(transport.stats().events_dropped(), 0);
}
