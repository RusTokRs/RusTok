use async_graphql::{Context, FieldError, Object, Result, dataloader::DataLoader};
use rustok_api::{
    AuthContext, TenantContext,
    graphql::{GraphQLError, require_module_enabled, resolve_graphql_locale},
};
use sea_orm::DatabaseConnection;
use uuid::Uuid;

use crate::{ProfileError, ProfileService, ProfileSummaryLoader, ProfileSummaryLoaderKey};

use super::{MODULE_SLUG, types::*};

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
        let tenant_id = current_tenant_id(tenant, tenant_id)?;
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
        let auth = require_human_user(ctx)?;
        let db = ctx.data::<DatabaseConnection>()?;
        let tenant = ctx.data::<TenantContext>()?;
        let locale = resolve_graphql_locale(ctx, locale.as_deref());

        if auth.tenant_id != tenant.id {
            return Err(<FieldError as GraphQLError>::permission_denied(
                "Authenticated profile reads must use the current tenant",
            ));
        }

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
        let tenant_id = current_tenant_id(tenant, tenant_id)?;
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

fn current_tenant_id(tenant: &TenantContext, requested: Option<Uuid>) -> Result<Uuid> {
    if requested.is_some_and(|tenant_id| tenant_id != tenant.id) {
        return Err(<FieldError as GraphQLError>::permission_denied(
            "Profile reads must use the current tenant",
        ));
    }
    Ok(tenant.id)
}

fn require_human_user(ctx: &Context<'_>) -> Result<AuthContext> {
    let auth = ctx
        .data::<AuthContext>()
        .cloned()
        .map_err(|_| <FieldError as GraphQLError>::unauthenticated())?;
    if auth.is_service_principal() {
        return Err(<FieldError as GraphQLError>::permission_denied(
            "meProfile requires human-user credentials",
        ));
    }
    Ok(auth)
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

#[cfg(test)]
mod tests {
    use super::current_tenant_id;
    use rustok_api::TenantContext;
    use uuid::Uuid;

    fn tenant(id: Uuid) -> TenantContext {
        TenantContext {
            id,
            name: "Tenant".to_string(),
            slug: "tenant".to_string(),
            domain: None,
            settings: serde_json::json!({}),
            default_locale: "en".to_string(),
            is_active: true,
        }
    }

    #[test]
    fn profile_tenant_override_fails_closed() {
        let current = Uuid::new_v4();
        assert_eq!(current_tenant_id(&tenant(current), None).unwrap(), current);
        assert_eq!(
            current_tenant_id(&tenant(current), Some(current)).unwrap(),
            current
        );
        assert!(current_tenant_id(&tenant(current), Some(Uuid::new_v4())).is_err());
    }
}
