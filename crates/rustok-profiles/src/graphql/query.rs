use async_graphql::{dataloader::DataLoader, Context, FieldError, Object, Result};
use rustok_api::{
    graphql::{require_module_enabled, resolve_graphql_locale, GraphQLError},
    AuthContext, TenantContext,
};
use sea_orm::DatabaseConnection;
use uuid::Uuid;

use crate::{ProfileError, ProfileService, ProfileSummaryLoader, ProfileSummaryLoaderKey};

use super::{types::*, MODULE_SLUG};

#[derive(Default)]
pub struct ProfilesQuery;

#[Object]
impl ProfilesQuery {
    async fn profile_by_handle(
        &self,
        ctx: &Context<'_>,
        handle: String,
        locale: Option<String>,
        tenant_id: Option<Uuid>,
    ) -> Result<Option<GqlProfile>> {
        require_module_enabled(ctx, MODULE_SLUG).await?;
        let db = ctx.data::<DatabaseConnection>()?;
        let tenant = ctx.data::<TenantContext>()?;
        let tenant_id = tenant_id.unwrap_or(tenant.id);
        let locale = resolve_graphql_locale(ctx, locale.as_deref());

        let service = ProfileService::new(db.clone());
        match service
            .get_profile_by_handle(
                tenant_id,
                &handle,
                Some(locale.as_str()),
                Some(tenant.default_locale.as_str()),
            )
            .await
        {
            Ok(profile) => Ok(Some(profile.into())),
            Err(ProfileError::ProfileByHandleNotFound(_)) => Ok(None),
            Err(err) => Err(map_profile_error(err)),
        }
    }

    async fn me_profile(
        &self,
        ctx: &Context<'_>,
        locale: Option<String>,
    ) -> Result<Option<GqlProfile>> {
        require_module_enabled(ctx, MODULE_SLUG).await?;
        let auth = require_auth(ctx)?;
        let db = ctx.data::<DatabaseConnection>()?;
        let tenant = ctx.data::<TenantContext>()?;
        let locale = resolve_graphql_locale(ctx, locale.as_deref());

        let service = ProfileService::new(db.clone());
        match service
            .get_profile(
                tenant.id,
                auth.user_id,
                Some(locale.as_str()),
                Some(tenant.default_locale.as_str()),
            )
            .await
        {
            Ok(profile) => Ok(Some(profile.into())),
            Err(ProfileError::ProfileNotFound(_)) => Ok(None),
            Err(err) => Err(map_profile_error(err)),
        }
    }

    async fn profile_summary(
        &self,
        ctx: &Context<'_>,
        user_id: Uuid,
        locale: Option<String>,
        tenant_id: Option<Uuid>,
    ) -> Result<Option<GqlProfileSummary>> {
        require_module_enabled(ctx, MODULE_SLUG).await?;
        let db = ctx.data::<DatabaseConnection>()?;
        let tenant = ctx.data::<TenantContext>()?;
        let tenant_id = tenant_id.unwrap_or(tenant.id);
        let locale = resolve_graphql_locale(ctx, locale.as_deref());

        if let Some(loader) = ctx.data_opt::<DataLoader<ProfileSummaryLoader>>() {
            let summary = loader
                .load_one(ProfileSummaryLoaderKey {
                    tenant_id,
                    user_id,
                    requested_locale: Some(locale.clone()),
                    tenant_default_locale: Some(tenant.default_locale.clone()),
                })
                .await?;
            return Ok(summary.map(Into::into));
        }

        let service = ProfileService::new(db.clone());
        match service
            .get_profile_summary(
                tenant_id,
                user_id,
                Some(locale.as_str()),
                Some(tenant.default_locale.as_str()),
            )
            .await
        {
            Ok(summary) => Ok(Some(summary.into())),
            Err(ProfileError::ProfileNotFound(_)) => Ok(None),
            Err(err) => Err(map_profile_error(err)),
        }
    }
}

fn require_auth(ctx: &Context<'_>) -> Result<AuthContext> {
    ctx.data::<AuthContext>()
        .cloned()
        .map_err(|_| <FieldError as GraphQLError>::unauthenticated())
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
        ProfileError::Database(_) => <FieldError as GraphQLError>::internal_error(&err.to_string()),
    }
}
