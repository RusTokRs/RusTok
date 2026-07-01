use rustok_api::{Action, Resource};
use rustok_core::{EventBus, SecurityContext};
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
#[ignore = "Integration test requires RBAC wiring with auth context"]
async fn test_rbac_event_flow() -> TestResult<()> {
    let mut ctx = test_context().await?;

    let security = SecurityContext::system();
    assert!(matches!(
        security.get_scope(Resource::Users, Action::Read),
        rustok_api::PermissionScope::All
    ));

    let bus = ctx.bus.clone();
    bus.publish(
        ctx.tenant_id,
        security.user_id,
        DomainEvent::UserLoggedIn {
            user_id: Uuid::nil(),
        },
    )?;

    let envelope = next_event(&mut ctx.events).await?;
    assert!(matches!(envelope.event, DomainEvent::UserLoggedIn { .. }));

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
