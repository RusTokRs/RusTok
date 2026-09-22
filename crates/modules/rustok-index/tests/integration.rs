use rustok_core::EventBus;
use rustok_events::{DomainEvent, EventEnvelope};
use tokio::sync::broadcast;
use uuid::Uuid;

type TestResult<T> = Result<T, Box<dyn std::error::Error + Send + Sync>>;

struct TestContext {
    bus: EventBus,
    events: broadcast::Receiver<EventEnvelope>,
    tenant_id: Uuid,
}

#[tokio::test]
#[ignore = "Integration test requires database/index wiring"]
async fn test_index_rebuild_flow() -> TestResult<()> {
    let mut ctx = test_context().await?;

    let event = DomainEvent::ReindexRequested {
        target_type: "content".to_string(),
        target_id: None,
    };

    let bus = ctx.bus.clone();
    bus.publish(ctx.tenant_id, None, event)?;

    let envelope = next_event(&mut ctx.events).await?;
    assert!(matches!(
        envelope.event,
        DomainEvent::ReindexRequested { .. }
    ));

    let indexed = wait_for_index(&ctx).await?;
    assert_eq!(indexed.status, "ready");

    Ok(())
}

async fn test_context() -> TestResult<TestContext> {
    let bus = EventBus::new();
    let events = bus.subscribe();
    let tenant_id = Uuid::nil();

    Ok(TestContext {
        bus,
        events,
        tenant_id,
    })
}

async fn next_event(
    receiver: &mut broadcast::Receiver<EventEnvelope>,
) -> TestResult<EventEnvelope> {
    let envelope = tokio::time::timeout(std::time::Duration::from_secs(5), receiver.recv())
        .await
        .map_err(|_| "timed out waiting for event")??;
    Ok(envelope)
}

struct IndexStatus {
    status: String,
}

async fn wait_for_index(_ctx: &TestContext) -> TestResult<IndexStatus> {
    todo!("wire index rebuild status lookup for integration tests")
}
