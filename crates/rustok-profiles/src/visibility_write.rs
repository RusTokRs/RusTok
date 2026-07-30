use chrono::Utc;
use rustok_outbox::TransactionalEventBus;
use sea_orm::{
    ActiveModelTrait, ColumnTrait, DatabaseConnection, EntityTrait, QueryFilter, Set,
    TransactionTrait,
};
use uuid::Uuid;

use crate::{
    ProfileError, ProfileRecord, ProfileResult, ProfileService, ProfileVisibility, entities,
    profile_updated_event::publish_profile_updated_in_tx,
};

pub(crate) async fn update_profile_visibility_with_event(
    db: &DatabaseConnection,
    event_bus: &TransactionalEventBus,
    tenant_id: Uuid,
    actor_id: Uuid,
    user_id: Uuid,
    visibility: ProfileVisibility,
    tenant_default_locale: Option<&str>,
) -> ProfileResult<ProfileRecord> {
    let txn = db.begin().await?;
    let profile = entities::profile::Entity::find_by_id(user_id)
        .filter(entities::profile::Column::TenantId.eq(tenant_id))
        .one(&txn)
        .await?
        .ok_or(ProfileError::ProfileNotFound(user_id))?;

    let mut active: entities::profile::ActiveModel = profile.into();
    active.visibility = Set(visibility);
    active.updated_at = Set(Utc::now().into());
    let profile = active.update(&txn).await?;

    if let Err(error) =
        publish_profile_updated_in_tx(event_bus, &txn, tenant_id, actor_id, &profile).await
    {
        tracing::error!(
            tenant_id = %tenant_id,
            user_id = %profile.user_id,
            "Profile visibility event publication failed; rolling back owner write"
        );
        txn.rollback().await?;
        return Err(error);
    }
    txn.commit().await?;

    ProfileService::new(db.clone())
        .get_profile(tenant_id, user_id, None, tenant_default_locale)
        .await
}
