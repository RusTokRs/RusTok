use chrono::Utc;
use rustok_outbox::TransactionalEventBus;
use sea_orm::{
    ActiveModelTrait, ColumnTrait, DatabaseConnection, EntityTrait, QueryFilter, Set,
    TransactionTrait,
};
use uuid::Uuid;

use crate::{
    ProfileError, ProfileMutationContext, ProfileRecord, ProfileResult, ProfileService, entities,
    profile_updated_event::publish_profile_updated_in_tx,
};

pub(crate) async fn update_profile_media_with_event(
    db: &DatabaseConnection,
    event_bus: &TransactionalEventBus,
    context: ProfileMutationContext<'_>,
    avatar_media_id: Option<Uuid>,
    banner_media_id: Option<Uuid>,
) -> ProfileResult<ProfileRecord> {
    let ProfileMutationContext {
        tenant_id,
        actor_id,
        user_id,
        tenant_default_locale,
    } = context;
    let txn = db.begin().await?;
    let profile = entities::profile::Entity::find_by_id(user_id)
        .filter(entities::profile::Column::TenantId.eq(tenant_id))
        .one(&txn)
        .await?
        .ok_or(ProfileError::ProfileNotFound(user_id))?;

    let mut active: entities::profile::ActiveModel = profile.into();
    active.avatar_media_id = Set(avatar_media_id);
    active.banner_media_id = Set(banner_media_id);
    active.updated_at = Set(Utc::now().into());
    let profile = active.update(&txn).await?;

    if let Err(error) =
        publish_profile_updated_in_tx(event_bus, &txn, tenant_id, actor_id, &profile).await
    {
        tracing::error!(
            tenant_id = %tenant_id,
            user_id = %profile.user_id,
            "Profile media event publication failed; rolling back owner write"
        );
        txn.rollback().await?;
        return Err(error);
    }
    txn.commit().await?;

    ProfileService::new(db.clone())
        .get_profile(tenant_id, user_id, None, tenant_default_locale)
        .await
}
