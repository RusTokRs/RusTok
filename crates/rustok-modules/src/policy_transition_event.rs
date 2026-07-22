use sea_orm::DatabaseTransaction;
use thiserror::Error;
use uuid::Uuid;

use rustok_events::DomainEvent;

use crate::{
    ControlPlaneInfrastructure, ModulePolicyRevisionApplyOutcome,
    ModulePolicyRevisionConsumerError, ModulePolicyRevisionGate, ModulePolicyRevisionGateError,
    ModulePolicyRevisionTransition, SeaOrmModulePolicyRevisionConsumer,
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

    pub async fn publish_and_advance(
        &self,
        transaction: &DatabaseTransaction,
        tenant_id: Uuid,
        actor_id: Option<Uuid>,
        consumer_key: &str,
        transition: &ModulePolicyRevisionTransition,
    ) -> Result<(), ModuleEffectivePolicyTransitionCoordinatorError> {
        let outcome = self
            .consumer
            .apply_in_transaction(transaction, tenant_id, consumer_key, transition)
            .await?;
        if outcome != ModulePolicyRevisionApplyOutcome::Applied {
            return Err(ModuleEffectivePolicyTransitionCoordinatorError::RevisionRejected(outcome));
        }
        self.publisher
            .publish(transaction, tenant_id, actor_id, consumer_key, transition)
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
        tenant_id: Uuid,
        actor_id: Option<Uuid>,
        consumer_key: &str,
        transition: &ModulePolicyRevisionTransition,
    ) -> Result<(), ModuleEffectivePolicyTransitionPublisherError> {
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
                self.infrastructure.event_envelope(
                    Some(tenant_id),
                    actor_id,
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
