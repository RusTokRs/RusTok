use async_graphql::{Context, ErrorExtensions, Object, Result, SimpleObject};
use rustok_api::{
    AuthContext, Permission, TenantContext, graphql::require_module_enabled, has_effective_permission,
    tenant_module_settings,
};
use rustok_page_builder::rollout::BuilderCapabilityFlags;
use sea_orm::DatabaseConnection;
use uuid::Uuid;

const MODULE_SLUG: &str = "pages";

#[derive(Clone, Debug, PartialEq, Eq, SimpleObject)]
pub struct GqlPageBuilderRolloutSnapshot {
    pub tenant_id: Uuid,
    pub tenant_slug: String,
    pub builder_enabled: bool,
    pub preview_enabled: bool,
    pub properties_enabled: bool,
    pub publish_enabled: bool,
    pub provider_health_observed: bool,
}

impl GqlPageBuilderRolloutSnapshot {
    fn new(tenant: &TenantContext, flags: BuilderCapabilityFlags) -> Self {
        Self {
            tenant_id: tenant.id,
            tenant_slug: tenant.slug.clone(),
            builder_enabled: flags.builder_enabled,
            preview_enabled: flags.preview_enabled,
            properties_enabled: flags.properties_enabled,
            publish_enabled: flags.publish_enabled,
            provider_health_observed: false,
        }
    }
}

#[derive(Default)]
pub struct PageBuilderRolloutQuery;

#[Object]
impl PageBuilderRolloutQuery {
    async fn page_builder_rollout_snapshot(
        &self,
        ctx: &Context<'_>,
    ) -> Result<GqlPageBuilderRolloutSnapshot> {
        require_module_enabled(ctx, MODULE_SLUG).await?;
        let db = ctx.data::<DatabaseConnection>()?;
        let auth = ctx.data::<AuthContext>()?;
        let tenant = ctx.data::<TenantContext>()?;
        ensure_pages_read_authority(auth, tenant)?;

        let settings = tenant_module_settings(db, tenant.id, MODULE_SLUG)
            .await
            .map_err(|error| {
                tracing::error!(
                    tenant_id = %tenant.id,
                    module = MODULE_SLUG,
                    error = %error,
                    "failed to read Pages Page Builder rollout settings"
                );
                async_graphql::Error::new("Unable to read Pages Page Builder rollout settings")
                    .extend_with(|_, ext| ext.set("code", "INTERNAL_SERVER_ERROR"))
            })?
            .ok_or_else(|| {
                async_graphql::Error::new("Pages module is not enabled for the routed tenant")
                    .extend_with(|_, ext| ext.set("code", "MODULE_NOT_ENABLED"))
            })?;
        let flags = BuilderCapabilityFlags::from_module_settings(&settings).map_err(|error| {
            async_graphql::Error::new(format!(
                "Pages Page Builder rollout settings are invalid: {error}"
            ))
            .extend_with(|_, ext| ext.set("code", "PAGE_BUILDER_ROLLOUT_INVALID"))
        })?;

        Ok(GqlPageBuilderRolloutSnapshot::new(tenant, flags))
    }
}

fn ensure_pages_read_authority(auth: &AuthContext, tenant: &TenantContext) -> Result<()> {
    if auth.tenant_id != tenant.id {
        return Err(async_graphql::Error::new("Pages Page Builder rollout access is denied")
            .extend_with(|_, ext| ext.set("code", "FORBIDDEN")));
    }
    if !has_effective_permission(&auth.permissions, &Permission::PAGES_READ) {
        return Err(async_graphql::Error::new(
            "Pages read permission is required for Page Builder rollout status",
        )
        .extend_with(|_, ext| ext.set("code", "FORBIDDEN")));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use rustok_api::{Action, Resource};

    fn tenant(id: Uuid) -> TenantContext {
        TenantContext {
            id,
            name: "Pages tenant".to_string(),
            slug: "pages-tenant".to_string(),
            domain: None,
            settings: serde_json::json!({}),
            default_locale: "en".to_string(),
            is_active: true,
        }
    }

    fn auth(tenant_id: Uuid, permissions: Vec<Permission>) -> AuthContext {
        AuthContext {
            user_id: Uuid::new_v4(),
            session_id: Uuid::new_v4(),
            tenant_id,
            permissions,
            client_id: None,
            scopes: Vec::new(),
            grant_type: "session".to_string(),
        }
    }

    #[test]
    fn rollout_authority_is_tenant_bound_and_requires_pages_read() {
        let tenant_id = Uuid::new_v4();
        let tenant = tenant(tenant_id);
        assert!(
            ensure_pages_read_authority(
                &auth(tenant_id, vec![Permission::new(Resource::Pages, Action::Read)]),
                &tenant,
            )
            .is_ok()
        );
        assert!(ensure_pages_read_authority(&auth(tenant_id, Vec::new()), &tenant).is_err());
        assert!(
            ensure_pages_read_authority(
                &auth(
                    Uuid::new_v4(),
                    vec![Permission::new(Resource::Pages, Action::Read)]
                ),
                &tenant,
            )
            .is_err()
        );
    }
}
