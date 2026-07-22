use async_trait::async_trait;
use chrono::{DateTime, Utc};
use rustok_events::{DomainEvent, EventEnvelope};
use rustok_outbox::{OutboxTransport, TransactionalEventWriter};
use sea_orm::{DatabaseConnection, DatabaseTransaction};
use std::sync::Arc;
use uuid::Uuid;

/// Owner clock used for identities and evidence created outside a database
/// expression. Transactional database timestamps remain owned by the storage
/// adapter so one commit uses one database clock.
pub trait ControlPlaneClock: Send + Sync {
    fn now(&self) -> DateTime<Utc>;
}

/// Owner identity source for aggregate, operation, and outbox identities.
pub trait ControlPlaneIdGenerator: Send + Sync {
    fn new_id(&self) -> Uuid;
}

/// Shared infrastructure context supplied to owner services. Keeping clock and
/// identity generation together prevents individual services from reaching for
/// process globals and makes command/evidence fixtures deterministic.
#[derive(Clone)]
pub struct ControlPlaneInfrastructure {
    clock: Arc<dyn ControlPlaneClock>,
    ids: Arc<dyn ControlPlaneIdGenerator>,
    events: Arc<dyn TransactionalEventWriter>,
}

impl ControlPlaneInfrastructure {
    pub fn new(clock: Arc<dyn ControlPlaneClock>, ids: Arc<dyn ControlPlaneIdGenerator>) -> Self {
        Self {
            clock,
            ids,
            events: Arc::new(UnavailableTransactionalEventWriter),
        }
    }

    /// Creates production infrastructure for services that append events inside
    /// their owner-opened SeaORM transaction.
    pub fn for_database(db: DatabaseConnection) -> Self {
        Self::default().with_transactional_event_writer(Arc::new(OutboxTransport::new(db)))
    }

    pub fn with_transactional_event_writer(
        mut self,
        events: Arc<dyn TransactionalEventWriter>,
    ) -> Self {
        self.events = events;
        self
    }

    pub fn now(&self) -> DateTime<Utc> {
        self.clock.now()
    }

    pub fn new_id(&self) -> Uuid {
        self.ids.new_id()
    }

    pub(crate) fn prefixed_id(&self, prefix: &str) -> String {
        format!("{prefix}_{}", self.new_id().simple())
    }

    pub(crate) async fn write_event(
        &self,
        transaction: &DatabaseTransaction,
        envelope: EventEnvelope,
    ) -> rustok_core::Result<()> {
        self.events.write_event(transaction, envelope).await
    }

    /// Builds a root event envelope with owner-injected identity and time.
    /// `None` is the canonical platform scope and is encoded as the nil tenant
    /// identity used by the root event contract.
    pub(crate) fn event_envelope(
        &self,
        tenant_id: Option<Uuid>,
        actor_id: Option<Uuid>,
        event: DomainEvent,
    ) -> EventEnvelope {
        let mut envelope = EventEnvelope::new(tenant_id.unwrap_or_default(), actor_id, event);
        let event_id = self.new_id();
        envelope.id = event_id;
        envelope.correlation_id = event_id;
        envelope.timestamp = self.now();
        envelope
    }
}

impl Default for ControlPlaneInfrastructure {
    fn default() -> Self {
        Self::new(Arc::new(SystemClock), Arc::new(RandomIdGenerator))
    }
}

struct SystemClock;

impl ControlPlaneClock for SystemClock {
    fn now(&self) -> DateTime<Utc> {
        Utc::now()
    }
}

struct RandomIdGenerator;

impl ControlPlaneIdGenerator for RandomIdGenerator {
    fn new_id(&self) -> Uuid {
        Uuid::new_v4()
    }
}

struct UnavailableTransactionalEventWriter;

#[async_trait]
impl TransactionalEventWriter for UnavailableTransactionalEventWriter {
    async fn write_event(
        &self,
        _transaction: &DatabaseTransaction,
        _envelope: EventEnvelope,
    ) -> rustok_core::Result<()> {
        Err(rustok_core::Error::External(
            "transactional event writer is not configured".to_string(),
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    struct FixedClock(DateTime<Utc>);

    impl ControlPlaneClock for FixedClock {
        fn now(&self) -> DateTime<Utc> {
            self.0.to_owned()
        }
    }

    struct FixedId(Uuid);

    impl ControlPlaneIdGenerator for FixedId {
        fn new_id(&self) -> Uuid {
            self.0
        }
    }

    #[test]
    fn infrastructure_uses_injected_clock_and_identity_source() {
        let now = DateTime::parse_from_rfc3339("2026-07-20T12:00:00Z")
            .expect("fixed time")
            .with_timezone(&Utc);
        let id = Uuid::parse_str("aaaaaaaa-aaaa-4aaa-8aaa-aaaaaaaaaaaa").expect("fixed UUID");
        let infrastructure = ControlPlaneInfrastructure::new(
            Arc::new(FixedClock(now.to_owned())),
            Arc::new(FixedId(id)),
        );

        assert_eq!(infrastructure.now(), now);
        assert_eq!(infrastructure.new_id(), id);
    }

    #[test]
    fn event_envelope_uses_owner_identity_time_and_scope() {
        let now = DateTime::parse_from_rfc3339("2026-07-20T12:00:00Z")
            .expect("fixed time")
            .with_timezone(&Utc);
        let event_id = Uuid::parse_str("aaaaaaaa-aaaa-4aaa-8aaa-aaaaaaaaaaaa").expect("fixed UUID");
        let tenant_id =
            Uuid::parse_str("bbbbbbbb-bbbb-4bbb-8bbb-bbbbbbbbbbbb").expect("tenant UUID");
        let actor_id = Uuid::parse_str("cccccccc-cccc-4ccc-8ccc-cccccccccccc").expect("actor UUID");
        let infrastructure = ControlPlaneInfrastructure::new(
            Arc::new(FixedClock(now.to_owned())),
            Arc::new(FixedId(event_id)),
        );

        let envelope = infrastructure.event_envelope(
            Some(tenant_id),
            Some(actor_id),
            DomainEvent::ModuleBuildQueued {
                request_id: Uuid::parse_str("dddddddd-dddd-4ddd-8ddd-dddddddddddd")
                    .expect("request UUID"),
                tenant_id,
                project_id: "fixture".to_string(),
                attempt: 1,
            },
        );

        assert_eq!(envelope.id, event_id);
        assert_eq!(envelope.correlation_id, event_id);
        assert_eq!(envelope.tenant_id, tenant_id);
        assert_eq!(envelope.actor_id, Some(actor_id));
        assert_eq!(envelope.timestamp, now);
    }
}
