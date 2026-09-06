use chrono::Utc;
use rustok_outbox::TransactionalEventBus;
use rustok_taxonomy::{TaxonomyService, TaxonomyTermKind};
use sea_orm::{
    ActiveModelTrait, ColumnTrait, DatabaseConnection, EntityTrait, QueryFilter, Set,
    TransactionTrait,
};
use uuid::Uuid;

use crate::{
    ProfileBackfillRequest, ProfileBackfillResult, ProfileError, ProfileMutationContext,
    ProfileRecord, ProfileResult, ProfileService, ProfileStatus, UpsertProfileInput, entities,
    profile_updated_event::{
        publish_profile_updated_in_tx, publish_profile_updated_with_actor_in_tx,
    },
};

const PROFILE_SCOPE_VALUE: &str = "profiles";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ExistingProfilePolicy {
    Update,
    Skip,
}

struct ProfileWriteOutcome {
    profile: ProfileRecord,
    created: bool,
}

struct ProfileWriteRequest<'a> {
    tenant_id: Uuid,
    actor_id: Option<Uuid>,
    user_id: Uuid,
    input: UpsertProfileInput,
    tenant_default_locale: Option<&'a str>,
    existing_policy: ExistingProfilePolicy,
}

pub(crate) async fn upsert_profile_with_event(
    db: &DatabaseConnection,
    event_bus: &TransactionalEventBus,
    context: ProfileMutationContext<'_>,
    input: UpsertProfileInput,
) -> ProfileResult<ProfileRecord> {
    let ProfileMutationContext {
        tenant_id,
        actor_id,
        user_id,
        tenant_default_locale,
    } = context;
    let outcome = write_profile_with_event(
        db,
        event_bus,
        ProfileWriteRequest {
            tenant_id,
            actor_id: Some(actor_id),
            user_id,
            input,
            tenant_default_locale,
            existing_policy: ExistingProfilePolicy::Update,
        },
    )
    .await?;
    Ok(outcome.profile)
}

pub(crate) async fn backfill_profile_with_event(
    db: &DatabaseConnection,
    event_bus: &TransactionalEventBus,
    request: ProfileBackfillRequest<'_>,
) -> ProfileResult<ProfileBackfillResult> {
    let ProfileBackfillRequest {
        tenant_id,
        user_id,
        email,
        display_name,
        preferred_locale,
        visibility,
        tenant_default_locale,
    } = request;
    let service = ProfileService::new(db.clone());
    match service
        .get_profile(tenant_id, user_id, preferred_locale, tenant_default_locale)
        .await
    {
        Ok(profile) => {
            return Ok(ProfileBackfillResult {
                profile,
                created: false,
            });
        }
        Err(ProfileError::ProfileNotFound(_)) => {}
        Err(error) => return Err(error),
    }

    let input = service
        .plan_backfill_profile(
            tenant_id,
            user_id,
            email,
            display_name,
            preferred_locale,
            visibility,
        )
        .await?;
    let outcome = write_profile_with_event(
        db,
        event_bus,
        ProfileWriteRequest {
            tenant_id,
            actor_id: None,
            user_id,
            input,
            tenant_default_locale,
            existing_policy: ExistingProfilePolicy::Skip,
        },
    )
    .await?;

    Ok(ProfileBackfillResult {
        profile: outcome.profile,
        created: outcome.created,
    })
}

async fn write_profile_with_event(
    db: &DatabaseConnection,
    event_bus: &TransactionalEventBus,
    request: ProfileWriteRequest<'_>,
) -> ProfileResult<ProfileWriteOutcome> {
    let ProfileWriteRequest {
        tenant_id,
        actor_id,
        user_id,
        input,
        tenant_default_locale,
        existing_policy,
    } = request;
    let UpsertProfileInput {
        handle,
        display_name,
        bio,
        tags,
        avatar_media_id,
        banner_media_id,
        preferred_locale,
        visibility,
    } = input;
    let handle = ProfileService::normalize_handle(&handle)?;
    let display_name = ProfileService::normalize_display_name(&display_name)?;
    let preferred_locale = ProfileService::normalize_locale(preferred_locale.as_deref())?;
    let translation_locale = preferred_locale
        .clone()
        .or(ProfileService::normalize_locale(tenant_default_locale)?)
        .ok_or_else(|| {
            ProfileError::InvalidLocale("effective profile locale is required".to_string())
        })?;

    let txn = db.begin().await?;
    let existing = entities::profile::Entity::find_by_id(user_id)
        .filter(entities::profile::Column::TenantId.eq(tenant_id))
        .one(&txn)
        .await?;
    if existing_policy == ExistingProfilePolicy::Skip && existing.is_some() {
        txn.rollback().await?;
        let profile = ProfileService::new(db.clone())
            .get_profile(tenant_id, user_id, None, tenant_default_locale)
            .await?;
        return Ok(ProfileWriteOutcome {
            profile,
            created: false,
        });
    }

    let handle_owner = entities::profile::Entity::find()
        .filter(entities::profile::Column::TenantId.eq(tenant_id))
        .filter(entities::profile::Column::Handle.eq(handle.clone()))
        .one(&txn)
        .await?;
    if let Some(handle_owner) = handle_owner
        && handle_owner.user_id != user_id
    {
        return Err(ProfileError::DuplicateHandle(handle));
    }

    let created = existing.is_none();
    let now = Utc::now();
    let profile = match existing {
        Some(profile) => {
            let mut active: entities::profile::ActiveModel = profile.into();
            active.handle = Set(handle);
            active.avatar_media_id = Set(avatar_media_id);
            active.banner_media_id = Set(banner_media_id);
            active.preferred_locale = Set(preferred_locale);
            active.visibility = Set(visibility);
            active.status = Set(ProfileStatus::Active);
            active.updated_at = Set(now.into());
            active.update(&txn).await?
        }
        None => {
            entities::profile::ActiveModel {
                user_id: Set(user_id),
                tenant_id: Set(tenant_id),
                handle: Set(handle),
                avatar_media_id: Set(avatar_media_id),
                banner_media_id: Set(banner_media_id),
                preferred_locale: Set(preferred_locale),
                visibility: Set(visibility),
                status: Set(ProfileStatus::Active),
                created_at: Set(now.into()),
                updated_at: Set(now.into()),
            }
            .insert(&txn)
            .await?
        }
    };

    let translation = entities::profile_translation::Entity::find()
        .filter(entities::profile_translation::Column::ProfileUserId.eq(user_id))
        .filter(entities::profile_translation::Column::Locale.eq(translation_locale.clone()))
        .one(&txn)
        .await?;
    match translation {
        Some(translation) => {
            let mut active: entities::profile_translation::ActiveModel = translation.into();
            active.display_name = Set(display_name.clone());
            active.bio = Set(bio.clone());
            active.updated_at = Set(now.into());
            active.update(&txn).await?;
        }
        None => {
            entities::profile_translation::ActiveModel {
                id: Set(Uuid::new_v4()),
                profile_user_id: Set(user_id),
                locale: Set(translation_locale.clone()),
                display_name: Set(display_name),
                bio: Set(bio),
                created_at: Set(now.into()),
                updated_at: Set(now.into()),
            }
            .insert(&txn)
            .await?;
        }
    }

    entities::profile_tag::Entity::delete_many()
        .filter(entities::profile_tag::Column::ProfileUserId.eq(user_id))
        .exec(&txn)
        .await?;
    let tags = normalize_tag_names(&tags);
    if !tags.is_empty() {
        let term_ids = TaxonomyService::new(db.clone())
            .ensure_terms_for_module_in_tx(
                &txn,
                tenant_id,
                TaxonomyTermKind::Tag,
                PROFILE_SCOPE_VALUE,
                &translation_locale,
                &tags,
            )
            .await?;
        for (index, term_id) in term_ids.into_iter().enumerate() {
            entities::profile_tag::ActiveModel {
                profile_user_id: Set(user_id),
                term_id: Set(term_id),
                tenant_id: Set(tenant_id),
                created_at: Set((now + chrono::Duration::microseconds(index as i64)).into()),
            }
            .insert(&txn)
            .await?;
        }
    }

    let publish_result = match actor_id {
        Some(actor_id) => {
            publish_profile_updated_in_tx(event_bus, &txn, tenant_id, actor_id, &profile).await
        }
        None => {
            publish_profile_updated_with_actor_in_tx(event_bus, &txn, tenant_id, None, &profile)
                .await
        }
    };
    if let Err(error) = publish_result {
        tracing::error!(
            tenant_id = %tenant_id,
            user_id = %profile.user_id,
            "Profile upsert event publication failed; rolling back owner write"
        );
        txn.rollback().await?;
        return Err(error);
    }
    txn.commit().await?;

    let profile = ProfileService::new(db.clone())
        .get_profile(tenant_id, user_id, None, tenant_default_locale)
        .await?;
    Ok(ProfileWriteOutcome { profile, created })
}

fn normalize_tag_names(tag_names: &[String]) -> Vec<String> {
    let mut normalized = Vec::new();
    for tag_name in tag_names {
        let trimmed = tag_name.trim();
        if trimmed.is_empty() {
            continue;
        }
        if !normalized
            .iter()
            .any(|existing: &String| existing.eq_ignore_ascii_case(trimmed))
        {
            normalized.push(trimmed.to_string());
        }
    }
    normalized
}
