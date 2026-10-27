use sea_orm::DatabaseTransaction;
use thiserror::Error;
use uuid::Uuid;

use rustok_events::DomainEvent;

use crate::{
    ControlPlaneInfrastructure, ModuleCommandContext, ModulePolicyRevisionApplyOutcome,
    ModulePolicyRevisionConsumerError, ModulePolicyRevisionCursorState, ModulePolicyRevisionGate,
    ModulePolicyRevisionGateError, ModulePolicyRevisionTransition,
    SeaOrmModulePolicyRevisionConsumer,
};

const MAX_CONSUMER_KEY_BYTES: usize = 128;

/// Owner-side publisher for an explicit effective-policy transition.
///
/// A producer calls this on the same transaction as its state mutation. The
/// event carries both revisions, so downstream consumers can apply the shared
/// predecessor gate without inferring ordering from opaque digests.
#[derive(Clone)]
pub struct ModuleEffectivePolicyTransitionPublisher {
    infrastructure: ControlPlaneInfrastructure,
}

/// Combines the lifecycle owner cursor and event append in one transaction.
/// This is used by an owner that both mutates state and maintains the local
/// effective-policy projection; stale concurrent transitions therefore abort
/// the state mutation instead of merely producing a stale outbox event.
#[derive(Clone)]
pub struct ModuleEffectivePolicyTransitionCoordinator {
    publisher: ModuleEffectivePolicyTransitionPublisher,
    consumer: SeaOrmModulePolicyRevisionConsumer,
}

impl ModuleEffectivePolicyTransitionCoordinator {
    pub fn new(
        infrastructure: ControlPlaneInfrastructure,
        consumer: SeaOrmModulePolicyRevisionConsumer,
    ) -> Self {
        Self {
            publisher: ModuleEffectivePolicyTransitionPublisher::new(infrastructure),
            consumer,
        }
    }

    /// Acquires the same tenant-scoped durable cursor lock used by transition
    /// advancement. Lifecycle writers call this before mutating tenant module
    /// state so commit guards can serialize against the complete state change.
    pub async fn lock_current_revision(
        &self,
        transaction: &DatabaseTransaction,
        tenant_id: Uuid,
        consumer_key: &str,
    ) -> Result<Option<String>, ModuleEffectivePolicyTransitionCoordinatorError> {
        Ok(self
            .lock_cursor_state(transaction, tenant_id, consumer_key)
            .await?
            .current_revision)
    }

    /// Acquires the tenant-scoped durable cursor lock and returns the full
    /// sequenced state that must supply an owner transition predecessor.
    pub async fn lock_cursor_state(
        &self,
        transaction: &DatabaseTransaction,
        tenant_id: Uuid,
        consumer_key: &str,
    ) -> Result<ModulePolicyRevisionCursorState, ModuleEffectivePolicyTransitionCoordinatorError>
    {
        self.consumer
            .lock_cursor_state_in_transaction(transaction, tenant_id, consumer_key)
            .await
            .map_err(ModuleEffectivePolicyTransitionCoordinatorError::Consumer)
    }

    pub async fn publish_and_advance(
        &self,
        transaction: &DatabaseTransaction,
        context: &ModuleCommandContext,
        consumer_key: &str,
        transition: &ModulePolicyRevisionTransition,
    ) -> Result<(), ModuleEffectivePolicyTransitionCoordinatorError> {
        if context.validate().is_err() {
            return Err(
                ModuleEffectivePolicyTransitionPublisherError::InvalidCommandContext.into(),
            );
        }
        let tenant_id = context
            .tenant_id
            .ok_or(ModuleEffectivePolicyTransitionPublisherError::InvalidCommandContext)?;
        let outcome = self
            .consumer
            .apply_in_transaction(transaction, tenant_id, consumer_key, transition)
            .await?;
        // The coordinator is used inside the owner's state transaction, so
        // only an exact locked-predecessor apply may publish. A duplicate
        // means this owner command did not create the successor, while a gap
        // means its predecessor was stale. Returning an error aborts and
        // rolls back the accompanying state mutation instead of publishing a
        // transition the owner cursor cannot prove.
        if outcome != ModulePolicyRevisionApplyOutcome::Applied {
            return Err(ModuleEffectivePolicyTransitionCoordinatorError::RevisionRejected(outcome));
        }
        self.publisher
            .publish(transaction, context, consumer_key, transition)
            .await
            .map_err(ModuleEffectivePolicyTransitionCoordinatorError::Publisher)
    }
}

#[derive(Debug, Error)]
pub enum ModuleEffectivePolicyTransitionCoordinatorError {
    #[error(transparent)]
    Consumer(#[from] ModulePolicyRevisionConsumerError),
    #[error("effective-policy transition was rejected by the durable cursor: {0:?}")]
    RevisionRejected(ModulePolicyRevisionApplyOutcome),
    #[error(transparent)]
    Publisher(#[from] ModuleEffectivePolicyTransitionPublisherError),
}

impl ModuleEffectivePolicyTransitionPublisher {
    pub fn new(infrastructure: ControlPlaneInfrastructure) -> Self {
        Self { infrastructure }
    }

    pub async fn publish(
        &self,
        transaction: &DatabaseTransaction,
        context: &ModuleCommandContext,
        consumer_key: &str,
        transition: &ModulePolicyRevisionTransition,
    ) -> Result<(), ModuleEffectivePolicyTransitionPublisherError> {
        let tenant_id = context
            .tenant_id
            .ok_or(ModuleEffectivePolicyTransitionPublisherError::InvalidCommandContext)?;
        if context.validate().is_err() {
            return Err(ModuleEffectivePolicyTransitionPublisherError::InvalidCommandContext);
        }
        validate_request(tenant_id, consumer_key)?;
        let mut gate = ModulePolicyRevisionGate::new(transition.previous_revision.clone())?;
        if !matches!(
            gate.apply(transition)?,
            crate::ModulePolicyRevisionApplyOutcome::Applied
        ) {
            return Err(ModuleEffectivePolicyTransitionPublisherError::InvalidTransition);
        }
        self.infrastructure
            .write_event(
                transaction,
                self.infrastructure.event_envelope_for_command(
                    context,
                    DomainEvent::ModuleEffectivePolicyRevisionChanged {
                        consumer_key: consumer_key.to_string(),
                        previous_revision: transition.previous_revision.clone(),
                        next_revision: transition.next_revision.clone(),
                    },
                ),
            )
            .await
            .map_err(|error| {
                ModuleEffectivePolicyTransitionPublisherError::Storage(error.to_string())
            })
    }
}

#[derive(Debug, Error)]
pub enum ModuleEffectivePolicyTransitionPublisherError {
    #[error("effective-policy transition publisher requires a valid tenant-scoped command context")]
    InvalidCommandContext,
    #[error("effective-policy transition publisher tenant must be a non-nil UUID")]
    InvalidTenant,
    #[error("effective-policy transition publisher consumer key is invalid")]
    InvalidConsumerKey,
    #[error(transparent)]
    Revision(#[from] ModulePolicyRevisionGateError),
    #[error("effective-policy transition must be an applied successor")]
    InvalidTransition,
    #[error("effective-policy transition event storage failed: {0}")]
    Storage(String),
}

fn validate_request(
    tenant_id: Uuid,
    consumer_key: &str,
) -> Result<(), ModuleEffectivePolicyTransitionPublisherError> {
    if tenant_id.is_nil() {
        return Err(ModuleEffectivePolicyTransitionPublisherError::InvalidTenant);
    }
    if consumer_key.is_empty()
        || consumer_key.trim() != consumer_key
        || consumer_key.len() > MAX_CONSUMER_KEY_BYTES
        || consumer_key.chars().any(char::is_control)
    {
        return Err(ModuleEffectivePolicyTransitionPublisherError::InvalidConsumerKey);
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use std::sync::{Arc, Mutex};

    use async_trait::async_trait;
    use sea_orm::{ConnectOptions, ConnectionTrait, Database, TransactionTrait};

    use super::*;

    #[derive(Clone, Default)]
    struct CapturingEventWriter(Arc<Mutex<Vec<rustok_events::EventEnvelope>>>);

    #[async_trait]
    impl rustok_outbox::TransactionalEventWriter for CapturingEventWriter {
        async fn write_event(
            &self,
            _transaction: &DatabaseTransaction,
            envelope: rustok_events::EventEnvelope,
        ) -> rustok_core::Result<()> {
            self.0
                .lock()
                .expect("capturing event writer lock")
                .push(envelope);
            Ok(())
        }
    }

    fn digest(marker: char) -> String {
        format!("sha256:{}", marker.to_string().repeat(64))
    }

    #[tokio::test]
    async fn publisher_retains_the_command_context_in_the_transition_event() {
        let database = Database::connect("sqlite::memory:")
            .await
            .expect("database");
        let event_writer = CapturingEventWriter::default();
        let publisher = ModuleEffectivePolicyTransitionPublisher::new(
            ControlPlaneInfrastructure::default()
                .with_transactional_event_writer(Arc::new(event_writer.clone())),
        );
        let context = ModuleCommandContext {
            actor_id: Uuid::new_v4(),
            tenant_id: Some(Uuid::new_v4()),
            trace_id: "test:effective-policy-transition".to_string(),
            correlation_id: Uuid::new_v4(),
            idempotency_key: Uuid::new_v4(),
        };
        let transition = ModulePolicyRevisionTransition {
            previous_revision: Some(digest('a')),
            next_revision: digest('b'),
        };
        let transaction = database.begin().await.expect("transaction");

        publisher
            .publish(&transaction, &context, "module.lifecycle", &transition)
            .await
            .expect("publish transition");
        transaction.commit().await.expect("commit");

        let events = event_writer
            .0
            .lock()
            .expect("capturing event writer lock")
            .clone();
        assert_eq!(events.len(), 1);
        let event = &events[0];
        assert_eq!(event.tenant_id, context.tenant_id.expect("tenant"));
        assert_eq!(event.actor_id, Some(context.actor_id));
        assert_eq!(event.correlation_id, context.correlation_id);
        assert_eq!(event.trace_id.as_deref(), Some(context.trace_id.as_str()));
    }

    #[tokio::test]
    async fn coordinator_rejects_a_gap_without_publishing_or_moving_the_cursor() {
        let url = format!(
            "sqlite:file:module_policy_coordinator_{}?mode=memory&cache=shared",
            Uuid::new_v4()
        );
        let mut options = ConnectOptions::new(url);
        options
            .max_connections(1)
            .min_connections(1)
            .sqlx_logging(false);
        let database = Database::connect(options).await.expect("database");
        database
            .execute_unprepared(
                "CREATE TABLE module_policy_revision_cursors (\
                    tenant_id TEXT NOT NULL, \
                    consumer_key TEXT NOT NULL, \
                    current_revision TEXT NULL, \
                    current_sequence INTEGER NOT NULL DEFAULT 0, \
                    updated_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP, \
                    PRIMARY KEY (tenant_id, consumer_key)\
                 )",
            )
            .await
            .expect("cursor table");

        let tenant_id = Uuid::new_v4();
        let consumer = SeaOrmModulePolicyRevisionConsumer::new(database.clone());
        let first_revision = digest('a');
        assert_eq!(
            consumer
                .apply(
                    tenant_id,
                    "module.lifecycle",
                    &ModulePolicyRevisionTransition {
                        previous_revision: None,
                        next_revision: first_revision.clone(),
                    },
                )
                .await
                .expect("initial cursor advance"),
            ModulePolicyRevisionApplyOutcome::Applied
        );

        let event_writer = CapturingEventWriter::default();
        let coordinator = ModuleEffectivePolicyTransitionCoordinator::new(
            ControlPlaneInfrastructure::default()
                .with_transactional_event_writer(Arc::new(event_writer.clone())),
            SeaOrmModulePolicyRevisionConsumer::new(database.clone()),
        );
        let context = ModuleCommandContext {
            actor_id: Uuid::new_v4(),
            tenant_id: Some(tenant_id),
            trace_id: "test:effective-policy-gap".to_string(),
            correlation_id: Uuid::new_v4(),
            idempotency_key: Uuid::new_v4(),
        };
        let transaction = database.begin().await.expect("transaction");
        let rejected = coordinator
            .publish_and_advance(
                &transaction,
                &context,
                "module.lifecycle",
                &ModulePolicyRevisionTransition {
                    previous_revision: Some(digest('c')),
                    next_revision: digest('b'),
                },
            )
            .await;
        assert!(matches!(
            rejected,
            Err(
                ModuleEffectivePolicyTransitionCoordinatorError::RevisionRejected(
                    ModulePolicyRevisionApplyOutcome::ResyncRequired
                )
            )
        ));
        transaction.rollback().await.expect("rollback");

        assert!(
            event_writer
                .0
                .lock()
                .expect("capturing event writer lock")
                .is_empty()
        );
        assert_eq!(
            consumer
                .cursor_state(tenant_id, "module.lifecycle")
                .await
                .expect("cursor state after rejected gap"),
            ModulePolicyRevisionCursorState {
                current_revision: Some(first_revision),
                current_sequence: 1,
            }
        );
    }
}
