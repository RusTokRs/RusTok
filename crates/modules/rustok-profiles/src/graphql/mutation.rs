use std::{future::Future, time::Duration};

use async_graphql::{Context, FieldError, Object, Result};
use rustok_api::{
    AuthContext, PortContext, PortError, PortErrorKind, TenantContext,
    graphql::{GraphQLError, require_module_enabled},
};
use rustok_media::{MediaAssetReadPort, MediaService};
use rustok_outbox::TransactionalEventBus;
use rustok_storage::StorageRuntime;
use sea_orm::DatabaseConnection;
use uuid::Uuid;

use crate::{
    ProfileError, ProfileMediaSlot, ProfileMutationContext, ProfileMutationService,
    ProfileOperation, ProfileOperationTimer, ProfileRecord, ProfileResult,
    validate_profile_media_asset,
};

use super::{MODULE_SLUG, types::*};

const PROFILE_MEDIA_READ_DEADLINE: Duration = Duration::from_secs(2);

#[derive(Default)]
pub struct ProfilesMutation;

#[Object]
impl ProfilesMutation {
    async fn upsert_my_profile(
        &self,
        ctx: &Context<'_>,
        input: GqlUpsertProfileInput,
    ) -> Result<GqlProfile> {
        require_module_enabled(ctx, MODULE_SLUG).await?;
        let auth = require_human_user(ctx)?;
        let db = ctx.data::<DatabaseConnection>()?;
        let event_bus = ctx.data::<TransactionalEventBus>()?;
        let mutations = ProfileMutationService::new(db, event_bus);
        let tenant = ctx.data::<TenantContext>()?;

        validate_profile_media_references(
            ctx,
            &auth,
            tenant.id,
            input.avatar_media_id,
            input.banner_media_id,
        )
        .await?;

        let profile = observe_profile_write(
            ProfileOperation::Upsert,
            tenant.id,
            auth.user_id,
            mutations.upsert_profile_with_event(
                self_service_mutation_context(tenant, auth.user_id),
                input.into(),
            ),
        )
        .await?;

        Ok(profile.into())
    }

    async fn update_my_profile_handle(
        &self,
        ctx: &Context<'_>,
        handle: String,
    ) -> Result<GqlProfile> {
        require_module_enabled(ctx, MODULE_SLUG).await?;
        let auth = require_human_user(ctx)?;
        let db = ctx.data::<DatabaseConnection>()?;
        let event_bus = ctx.data::<TransactionalEventBus>()?;
        let mutations = ProfileMutationService::new(db, event_bus);
        let tenant = ctx.data::<TenantContext>()?;

        let profile = observe_profile_write(
            ProfileOperation::UpdateHandle,
            tenant.id,
            auth.user_id,
            mutations.update_profile_handle_with_event(
                self_service_mutation_context(tenant, auth.user_id),
                &handle,
            ),
        )
        .await?;

        Ok(profile.into())
    }

    async fn update_my_profile_content(
        &self,
        ctx: &Context<'_>,
        input: GqlUpdateMyProfileContentInput,
    ) -> Result<GqlProfile> {
        require_module_enabled(ctx, MODULE_SLUG).await?;
        let auth = require_human_user(ctx)?;
        let db = ctx.data::<DatabaseConnection>()?;
        let event_bus = ctx.data::<TransactionalEventBus>()?;
        let mutations = ProfileMutationService::new(db, event_bus);
        let tenant = ctx.data::<TenantContext>()?;

        let profile = observe_profile_write(
            ProfileOperation::UpdateContent,
            tenant.id,
            auth.user_id,
            mutations.update_profile_content_with_event(
                self_service_mutation_context(tenant, auth.user_id),
                &input.display_name,
                input.bio.as_deref(),
            ),
        )
        .await?;

        Ok(profile.into())
    }

    async fn update_my_profile_locale(
        &self,
        ctx: &Context<'_>,
        preferred_locale: Option<String>,
    ) -> Result<GqlProfile> {
        require_module_enabled(ctx, MODULE_SLUG).await?;
        let auth = require_human_user(ctx)?;
        let db = ctx.data::<DatabaseConnection>()?;
        let event_bus = ctx.data::<TransactionalEventBus>()?;
        let mutations = ProfileMutationService::new(db, event_bus);
        let tenant = ctx.data::<TenantContext>()?;

        let profile = observe_profile_write(
            ProfileOperation::UpdateLocale,
            tenant.id,
            auth.user_id,
            mutations.update_profile_locale_with_event(
                self_service_mutation_context(tenant, auth.user_id),
                preferred_locale.as_deref(),
            ),
        )
        .await?;

        Ok(profile.into())
    }

    async fn update_my_profile_visibility(
        &self,
        ctx: &Context<'_>,
        visibility: GqlProfileVisibility,
    ) -> Result<GqlProfile> {
        require_module_enabled(ctx, MODULE_SLUG).await?;
        let auth = require_human_user(ctx)?;
        let db = ctx.data::<DatabaseConnection>()?;
        let event_bus = ctx.data::<TransactionalEventBus>()?;
        let mutations = ProfileMutationService::new(db, event_bus);
        let tenant = ctx.data::<TenantContext>()?;

        let profile = observe_profile_write(
            ProfileOperation::UpdateVisibility,
            tenant.id,
            auth.user_id,
            mutations.update_profile_visibility_with_event(
                self_service_mutation_context(tenant, auth.user_id),
                visibility.into(),
            ),
        )
        .await?;

        Ok(profile.into())
    }

    async fn update_my_profile_media(
        &self,
        ctx: &Context<'_>,
        input: GqlUpdateMyProfileMediaInput,
    ) -> Result<GqlProfile> {
        require_module_enabled(ctx, MODULE_SLUG).await?;
        let auth = require_human_user(ctx)?;
        let db = ctx.data::<DatabaseConnection>()?;
        let event_bus = ctx.data::<TransactionalEventBus>()?;
        let mutations = ProfileMutationService::new(db, event_bus);
        let tenant = ctx.data::<TenantContext>()?;

        validate_profile_media_references(
            ctx,
            &auth,
            tenant.id,
            input.avatar_media_id,
            input.banner_media_id,
        )
        .await?;

        let profile = observe_profile_write(
            ProfileOperation::UpdateMedia,
            tenant.id,
            auth.user_id,
            mutations.update_profile_media_with_event(
                self_service_mutation_context(tenant, auth.user_id),
                input.avatar_media_id,
                input.banner_media_id,
            ),
        )
        .await?;

        Ok(profile.into())
    }
}

fn self_service_mutation_context<'a>(
    tenant: &'a TenantContext,
    user_id: Uuid,
) -> ProfileMutationContext<'a> {
    ProfileMutationContext {
        tenant_id: tenant.id,
        actor_id: user_id,
        user_id,
        tenant_default_locale: Some(tenant.default_locale.as_str()),
    }
}

async fn observe_profile_write<F>(
    operation: ProfileOperation,
    tenant_id: Uuid,
    user_id: Uuid,
    future: F,
) -> Result<ProfileRecord>
where
    F: Future<Output = ProfileResult<ProfileRecord>>,
{
    let timer = ProfileOperationTimer::start(operation, tenant_id, user_id);
    let result = future.await;
    timer.finish_profile_result(&result);
    result.map_err(map_profile_error)
}

async fn validate_profile_media_references(
    ctx: &Context<'_>,
    auth: &AuthContext,
    tenant_id: Uuid,
    avatar_media_id: Option<Uuid>,
    banner_media_id: Option<Uuid>,
) -> Result<()> {
    if auth.tenant_id != tenant_id {
        return Err(<FieldError as GraphQLError>::permission_denied(
            "Profile media updates must use the current tenant",
        ));
    }

    if avatar_media_id.is_none() && banner_media_id.is_none() {
        return Ok(());
    }

    let db = ctx.data::<DatabaseConnection>()?;
    let storage = ctx.data::<StorageRuntime>()?;
    let media = MediaService::new(db.clone(), storage.clone());

    for (slot, media_id) in [
        (ProfileMediaSlot::Avatar, avatar_media_id),
        (ProfileMediaSlot::Banner, banner_media_id),
    ] {
        let Some(media_id) = media_id else {
            continue;
        };
        let context = PortContext::new(
            tenant_id.to_string(),
            auth.port_actor(),
            "und",
            format!(
                "profile-media:{}:{}:{}",
                slot.as_str(),
                auth.user_id,
                media_id
            ),
        )
        .with_deadline(PROFILE_MEDIA_READ_DEADLINE);
        let asset = media
            .get_asset(context, media_id)
            .await
            .map_err(|error| map_profile_media_read_error(slot, error))?;
        validate_profile_media_asset(tenant_id, auth.user_id, slot, &asset)
            .map_err(map_profile_error)?;
    }

    Ok(())
}

fn map_profile_media_read_error(slot: ProfileMediaSlot, error: PortError) -> async_graphql::Error {
    match &error.kind {
        PortErrorKind::NotFound => <FieldError as GraphQLError>::bad_user_input(&format!(
            "profile {} media asset was not found",
            slot.as_str()
        )),
        _ => <FieldError as GraphQLError>::internal_error(&error.message),
    }
}

fn require_human_user(ctx: &Context<'_>) -> Result<AuthContext> {
    let auth = ctx
        .data::<AuthContext>()
        .cloned()
        .map_err(|_| <FieldError as GraphQLError>::unauthenticated())?;
    if auth.is_service_principal() {
        return Err(<FieldError as GraphQLError>::permission_denied(
            "Profile self-service mutations require human-user credentials",
        ));
    }
    Ok(auth)
}

fn map_profile_error(error: ProfileError) -> async_graphql::Error {
    let message = error.to_string();
    match error {
        ProfileError::EmptyDisplayName
        | ProfileError::DisplayNameTooLong
        | ProfileError::EmptyHandle
        | ProfileError::InvalidHandle
        | ProfileError::HandleTooShort
        | ProfileError::HandleTooLong
        | ProfileError::ReservedHandle(_)
        | ProfileError::InvalidLocale(_)
        | ProfileError::Validation(_)
        | ProfileError::DuplicateHandle(_) => {
            <FieldError as GraphQLError>::bad_user_input(&message)
        }
        ProfileError::ProfileNotFound(_) | ProfileError::ProfileByHandleNotFound(_) => {
            <FieldError as GraphQLError>::not_found(&message)
        }
        ProfileError::LocalizedCopyNotFound(_)
        | ProfileError::PresentationUnavailable
        | ProfileError::EventPublishUnavailable
        | ProfileError::Database(_) => <FieldError as GraphQLError>::internal_error(&message),
    }
}
