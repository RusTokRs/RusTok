use std::time::Duration;

use async_graphql::{Context, FieldError, Object, Result};
use rustok_api::{
    AuthContext, PortContext, PortError, PortErrorKind, TenantContext,
    graphql::{GraphQLError, require_module_enabled},
};
use rustok_events::DomainEvent;
use rustok_media::{MediaAssetReadPort, MediaService};
use rustok_outbox::TransactionalEventBus;
use rustok_storage::StorageRuntime;
use sea_orm::DatabaseConnection;
use uuid::Uuid;

use crate::{
    ProfileError, ProfileMediaSlot, ProfileService, validate_profile_media_asset,
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
        let tenant = ctx.data::<TenantContext>()?;

        validate_profile_media_references(
            ctx,
            &auth,
            tenant.id,
            input.avatar_media_id,
            input.banner_media_id,
        )
        .await?;

        let service = ProfileService::new(db.clone());
        let profile = service
            .upsert_profile(
                tenant.id,
                auth.user_id,
                input.into(),
                Some(tenant.default_locale.as_str()),
            )
            .await
            .map_err(map_profile_error)?;
        publish_profile_updated(event_bus, tenant.id, auth.user_id, &profile).await?;

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
        let tenant = ctx.data::<TenantContext>()?;

        let profile = ProfileService::new(db.clone())
            .update_profile_handle(
                tenant.id,
                auth.user_id,
                &handle,
                Some(tenant.default_locale.as_str()),
            )
            .await
            .map_err(map_profile_error)?;
        publish_profile_updated(event_bus, tenant.id, auth.user_id, &profile).await?;

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
        let tenant = ctx.data::<TenantContext>()?;

        let profile = ProfileService::new(db.clone())
            .update_profile_content(
                tenant.id,
                auth.user_id,
                &input.display_name,
                input.bio.as_deref(),
                Some(tenant.default_locale.as_str()),
            )
            .await
            .map_err(map_profile_error)?;
        publish_profile_updated(event_bus, tenant.id, auth.user_id, &profile).await?;

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
        let tenant = ctx.data::<TenantContext>()?;

        let profile = ProfileService::new(db.clone())
            .update_profile_locale(
                tenant.id,
                auth.user_id,
                preferred_locale.as_deref(),
                Some(tenant.default_locale.as_str()),
            )
            .await
            .map_err(map_profile_error)?;
        publish_profile_updated(event_bus, tenant.id, auth.user_id, &profile).await?;

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
        let tenant = ctx.data::<TenantContext>()?;

        let profile = ProfileService::new(db.clone())
            .update_profile_visibility(
                tenant.id,
                auth.user_id,
                visibility.into(),
                Some(tenant.default_locale.as_str()),
            )
            .await
            .map_err(map_profile_error)?;
        publish_profile_updated(event_bus, tenant.id, auth.user_id, &profile).await?;

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
        let tenant = ctx.data::<TenantContext>()?;

        validate_profile_media_references(
            ctx,
            &auth,
            tenant.id,
            input.avatar_media_id,
            input.banner_media_id,
        )
        .await?;

        let profile = ProfileService::new(db.clone())
            .update_profile_media(
                tenant.id,
                auth.user_id,
                input.avatar_media_id,
                input.banner_media_id,
                Some(tenant.default_locale.as_str()),
            )
            .await
            .map_err(map_profile_error)?;
        publish_profile_updated(event_bus, tenant.id, auth.user_id, &profile).await?;

        Ok(profile.into())
    }
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

fn map_profile_media_read_error(
    slot: ProfileMediaSlot,
    error: PortError,
) -> async_graphql::Error {
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

async fn publish_profile_updated(
    event_bus: &TransactionalEventBus,
    tenant_id: uuid::Uuid,
    actor_id: uuid::Uuid,
    profile: &crate::ProfileRecord,
) -> Result<()> {
    event_bus
        .publish(
            tenant_id,
            Some(actor_id),
            DomainEvent::ProfileUpdated {
                user_id: profile.user_id,
                handle: profile.handle.clone(),
                locale: profile.preferred_locale.clone(),
            },
        )
        .await
        .map_err(|err| <FieldError as GraphQLError>::internal_error(&err.to_string()))
}

fn map_profile_error(err: ProfileError) -> async_graphql::Error {
    match err {
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
            <FieldError as GraphQLError>::bad_user_input(&err.to_string())
        }
        ProfileError::ProfileNotFound(_) | ProfileError::ProfileByHandleNotFound(_) => {
            <FieldError as GraphQLError>::not_found(&err.to_string())
        }
        ProfileError::LocalizedCopyNotFound(_) | ProfileError::Database(_) => {
            <FieldError as GraphQLError>::internal_error(&err.to_string())
        }
    }
}
