use chrono::Utc;
use rustok_outbox::TransactionalEventBus;
use sea_orm::{
    ActiveModelTrait, ColumnTrait, DatabaseConnection, EntityTrait, QueryFilter, Set,
    TransactionTrait,
};
use uuid::Uuid;

use crate::{
    ProfileError, ProfileRecord, ProfileResult, ProfileService, entities,
    profile_updated_event::publish_profile_updated_in_tx,
};

pub(crate) async fn update_profile_handle_with_event(
    db: &DatabaseConnection,
    event_bus: &TransactionalEventBus,
    tenant_id: Uuid,
    actor_id: Uuid,
    user_id: Uuid,
    handle: &str,
    tenant_default_locale: Option<&str>,
) -> ProfileResult<ProfileRecord> {
    let handle = ProfileService::normalize_handle(handle)?;
    let txn = db.begin().await?;

    let profile = entities::profile::Entity::find_by_id(user_id)
        .filter(entities::profile::Column::TenantId.eq(tenant_id))
        .one(&txn)
        .await?
        .ok_or(ProfileError::ProfileNotFound(user_id))?;
    let existing = entities::profile::Entity::find()
        .filter(entities::profile::Column::TenantId.eq(tenant_id))
        .filter(entities::profile::Column::Handle.eq(handle.clone()))
        .one(&txn)
        .await?;
    if existing.is_some_and(|candidate| candidate.user_id != user_id) {
        txn.rollback().await?;
        return Err(ProfileError::DuplicateHandle(handle));
    }

    let mut active: entities::profile::ActiveModel = profile.into();
    active.handle = Set(handle);
    active.updated_at = Set(Utc::now().into());
    let profile = active.update(&txn).await?;

    if let Err(error) =
        publish_profile_updated_in_tx(event_bus, &txn, tenant_id, actor_id, &profile).await
    {
        tracing::error!(
            tenant_id = %tenant_id,
            user_id = %profile.user_id,
            "Profile handle event publication failed; rolling back owner write"
        );
        txn.rollback().await?;
        return Err(error);
    }
    txn.commit().await?;

    ProfileService::new(db.clone())
        .get_profile(tenant_id, user_id, None, tenant_default_locale)
        .await
}
