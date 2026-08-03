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

pub(crate) async fn update_profile_content_with_event(
    db: &DatabaseConnection,
    event_bus: &TransactionalEventBus,
    context: ProfileMutationContext<'_>,
    display_name: &str,
    bio: Option<&str>,
) -> ProfileResult<ProfileRecord> {
    let ProfileMutationContext {
        tenant_id,
        actor_id,
        user_id,
        tenant_default_locale,
    } = context;
    let display_name = ProfileService::normalize_display_name(display_name)?;
    let txn = db.begin().await?;

    let profile = entities::profile::Entity::find_by_id(user_id)
        .filter(entities::profile::Column::TenantId.eq(tenant_id))
        .one(&txn)
        .await?
        .ok_or(ProfileError::ProfileNotFound(user_id))?;
    let translation_locale = ProfileService::normalize_locale(profile.preferred_locale.as_deref())?
        .or(ProfileService::normalize_locale(tenant_default_locale)?)
        .ok_or_else(|| {
            ProfileError::InvalidLocale("effective profile locale is required".to_string())
        })?;

    let now = Utc::now();
    let mut active: entities::profile::ActiveModel = profile.into();
    active.updated_at = Set(now.into());
    let profile = active.update(&txn).await?;

    let translation = entities::profile_translation::Entity::find()
        .filter(entities::profile_translation::Column::ProfileUserId.eq(user_id))
        .filter(entities::profile_translation::Column::Locale.eq(translation_locale.clone()))
        .one(&txn)
        .await?;
    match translation {
        Some(translation) => {
            let mut active: entities::profile_translation::ActiveModel = translation.into();
            active.display_name = Set(display_name);
            active.bio = Set(bio.map(str::to_string));
            active.updated_at = Set(now.into());
            active.update(&txn).await?;
        }
        None => {
            entities::profile_translation::ActiveModel {
                id: Set(Uuid::new_v4()),
                profile_user_id: Set(user_id),
                locale: Set(translation_locale.clone()),
                display_name: Set(display_name),
                bio: Set(bio.map(str::to_string)),
                created_at: Set(now.into()),
                updated_at: Set(now.into()),
            }
            .insert(&txn)
            .await?;
        }
    }

    if let Err(error) =
        publish_profile_updated_in_tx(event_bus, &txn, tenant_id, actor_id, &profile).await
    {
        tracing::error!(
            tenant_id = %tenant_id,
            user_id = %profile.user_id,
            "Profile content event publication failed; rolling back owner write"
        );
        txn.rollback().await?;
        return Err(error);
    }
    txn.commit().await?;

    ProfileService::new(db.clone())
        .get_profile(
            tenant_id,
            user_id,
            Some(translation_locale.as_str()),
            tenant_default_locale,
        )
        .await
}
