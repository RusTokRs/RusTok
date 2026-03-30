use async_graphql::{Context, FieldError, Object, Result};
use rustok_api::{
    graphql::{require_module_enabled, GraphQLError},
    AuthContext, TenantContext,
};
use rustok_events::DomainEvent;
use rustok_outbox::TransactionalEventBus;
use sea_orm::DatabaseConnection;

use crate::{ProfileError, ProfileService};

use super::{types::*, MODULE_SLUG};

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
        let auth = require_auth(ctx)?;
        let db = ctx.data::<DatabaseConnection>()?;
        let event_bus = ctx.data::<TransactionalEventBus>()?;
        let tenant = ctx.data::<TenantContext>()?;

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
        let auth = require_auth(ctx)?;
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
        let auth = require_auth(ctx)?;
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
        let auth = require_auth(ctx)?;
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
        let auth = require_auth(ctx)?;
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
        let auth = require_auth(ctx)?;
        let db = ctx.data::<DatabaseConnection>()?;
        let event_bus = ctx.data::<TransactionalEventBus>()?;
        let tenant = ctx.data::<TenantContext>()?;

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

fn require_auth(ctx: &Context<'_>) -> Result<AuthContext> {
    ctx.data::<AuthContext>()
        .cloned()
        .map_err(|_| <FieldError as GraphQLError>::unauthenticated().into())
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
        .map_err(|err| <FieldError as GraphQLError>::internal_error(&err.to_string()).into())
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
            <FieldError as GraphQLError>::bad_user_input(&err.to_string()).into()
        }
        ProfileError::ProfileNotFound(_) | ProfileError::ProfileByHandleNotFound(_) => {
            <FieldError as GraphQLError>::not_found(&err.to_string()).into()
        }
        ProfileError::Database(_) => {
            <FieldError as GraphQLError>::internal_error(&err.to_string()).into()
        }
    }
}
