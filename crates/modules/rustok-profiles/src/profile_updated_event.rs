use rustok_events::DomainEvent;
use rustok_outbox::TransactionalEventBus;
use sea_orm::DatabaseTransaction;
use uuid::Uuid;

use crate::{ProfileError, ProfileOperation, ProfileOperationTimer, ProfileResult, entities};

pub(crate) async fn publish_profile_updated_in_tx(
    event_bus: &TransactionalEventBus,
    txn: &DatabaseTransaction,
    tenant_id: Uuid,
    actor_id: Uuid,
    profile: &entities::profile::Model,
) -> ProfileResult<()> {
    publish_profile_updated_with_actor_in_tx(event_bus, txn, tenant_id, Some(actor_id), profile)
        .await
}

pub(crate) async fn publish_profile_updated_with_actor_in_tx(
    event_bus: &TransactionalEventBus,
    txn: &DatabaseTransaction,
    tenant_id: Uuid,
    actor_id: Option<Uuid>,
    profile: &entities::profile::Model,
) -> ProfileResult<()> {
    let timer = ProfileOperationTimer::start(
        ProfileOperation::PublishUpdatedEvent,
        tenant_id,
        profile.user_id,
    );
    let result = event_bus
        .publish_in_tx(
            txn,
            tenant_id,
            actor_id,
            DomainEvent::ProfileUpdated {
                user_id: profile.user_id,
                handle: profile.handle.clone(),
                locale: profile.preferred_locale.clone(),
            },
        )
        .await;

    match result {
        Ok(()) => {
            timer.finish_success();
            Ok(())
        }
        Err(error) => {
            timer.finish_failure(ProfileError::EventPublishUnavailable.code(), true);
            tracing::error!(
                tenant_id = %tenant_id,
                user_id = %profile.user_id,
                error = %error,
                "Profile update event publication failed"
            );
            Err(ProfileError::EventPublishUnavailable)
        }
    }
}
