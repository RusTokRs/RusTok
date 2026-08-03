use crate::{
    ProfileError, ProfileMutationContext, ProfileRecord, ProfileResult, ProfileService, entities,
    profile_updated_event::publish_profile_updated_in_tx,
};
use chrono::Utc;
use rustok_outbox::TransactionalEventBus;
use sea_orm::{
    ActiveModelTrait, ColumnTrait, DatabaseConnection, EntityTrait, QueryFilter, Set,
    TransactionTrait,
};

pub(crate) async fn update_profile_locale_with_event(
    db: &DatabaseConnection,
    event_bus: &TransactionalEventBus,
    context: ProfileMutationContext<'_>,
    preferred_locale: Option<&str>,
) -> ProfileResult<ProfileRecord> {
    let ProfileMutationContext {
        tenant_id,
        actor_id,
        user_id,
        tenant_default_locale,
    } = context;
    let preferred_locale = ProfileService::normalize_locale(preferred_locale)?;
    let requested_locale = preferred_locale
        .clone()
        .or(ProfileService::normalize_locale(tenant_default_locale)?)
        .ok_or_else(|| {
            ProfileError::InvalidLocale("effective profile locale is required".to_string())
        })?;

    let txn = db.begin().await?;
    let profile = entities::profile::Entity::find_by_id(user_id)
        .filter(entities::profile::Column::TenantId.eq(tenant_id))
        .one(&txn)
        .await?
        .ok_or(ProfileError::ProfileNotFound(user_id))?;

    let mut active: entities::profile::ActiveModel = profile.into();
    active.preferred_locale = Set(preferred_locale);
    active.updated_at = Set(Utc::now().into());
    let profile = active.update(&txn).await?;

    // Locale preference changes selection policy only. They must never copy
    // localized display copy into a different locale or create a translation.
    if let Err(error) =
        publish_profile_updated_in_tx(event_bus, &txn, tenant_id, actor_id, &profile).await
    {
        tracing::error!(
            tenant_id = %tenant_id,
            user_id = %profile.user_id,
            "Profile locale event publication failed; rolling back owner write"
        );
        txn.rollback().await?;
        return Err(error);
    }
    txn.commit().await?;

    ProfileService::new(db.clone())
        .get_profile(
            tenant_id,
            user_id,
            Some(requested_locale.as_str()),
            tenant_default_locale,
        )
        .await
}
