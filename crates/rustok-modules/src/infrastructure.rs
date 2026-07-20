use async_trait::async_trait;
use chrono::{DateTime, Utc};
use rustok_events::EventEnvelope;
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
}
