use async_graphql::{Context, Enum, ErrorExtensions, Object, Result, SimpleObject};
use rustok_api::{
    AuthContext, Permission, TenantContext, graphql::require_module_enabled, has_effective_permission,
    tenant_module_settings,
};
use rustok_page_builder::{
    dto::{
        BuilderCapabilityKind, PAGE_BUILDER_FEATURE_DISABLED_ERROR_CODE, PageBuilderErrorKind,
    },
    rollout::{BuilderCapabilityFlags, BuilderRolloutError, ensure_capability},
    service::PageBuilderCapabilityPermissions,
};
use sea_orm::DatabaseConnection;
use uuid::Uuid;

const MODULE_SLUG: &str = "pages";

#[derive(Clone, Debug, PartialEq, Eq, SimpleObject)]
pub struct GqlPageBuilderRolloutSnapshot {
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
            tenant_slug: tenant.slug.clone(),
            builder_enabled: flags.builder_enabled,
            preview_enabled: flags.preview_enabled,
            properties_enabled: flags.properties_enabled,
            publish_enabled: flags.publish_enabled,
            provider_health_observed: false,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Enum)]
pub enum GqlPageBuilderCapability {
    Preview,
    Tree,
    Properties,
    Publish,
}

impl From<GqlPageBuilderCapability> for BuilderCapabilityKind {
    fn from(value: GqlPageBuilderCapability) -> Self {
        match value {
            GqlPageBuilderCapability::Preview => Self::Preview,
            GqlPageBuilderCapability::Tree => Self::Tree,
            GqlPageBuilderCapability::Properties => Self::Properties,
            GqlPageBuilderCapability::Publish => Self::Publish,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, SimpleObject)]
pub struct GqlPageBuilderCapabilityPreflight {
    pub capability: GqlPageBuilderCapability,
    pub allowed: bool,
    pub error_kind: Option<String>,
    pub error_code: Option<String>,
}

impl GqlPageBuilderCapabilityPreflight {
    fn allowed(capability: GqlPageBuilderCapability) -> Self {
        Self {
            capability,
            allowed: true,
            error_kind: None,
            error_code: None,
        }
    }

    fn feature_disabled(capability: GqlPageBuilderCapability) -> Self {
        Self {
            capability,
            allowed: false,
            error_kind: Some(PageBuilderErrorKind::FeatureDisabled.as_str().to_string()),
            error_code: Some(PAGE_BUILDER_FEATURE_DISABLED_ERROR_CODE.to_string()),
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
        let flags = load_rollout_flags(db, tenant).await?;

        Ok(GqlPageBuilderRolloutSnapshot::new(tenant, flags))
    }

    /// Non-mutating rollout/RBAC preflight for canonical Page Builder capability evidence.
    ///
    /// The permission check follows the same capability -> Pages permission mapping as the
    /// Page Builder authorizer. The shared rollout guard then yields the canonical
    /// `feature-disabled / FEATURE_DISABLED` contract without invoking preview rendering or
    /// publish persistence.
    async fn page_builder_capability_preflight(
        &self,
        ctx: &Context<'_>,
        capability: GqlPageBuilderCapability,
    ) -> Result<GqlPageBuilderCapabilityPreflight> {
        require_module_enabled(ctx, MODULE_SLUG).await?;
        let db = ctx.data::<DatabaseConnection>()?;
        let auth = ctx.data::<AuthContext>()?;
        let tenant = ctx.data::<TenantContext>()?;
        ensure_tenant_authority(auth, tenant)?;

        let capability_kind: BuilderCapabilityKind = capability.into();
        let required_permission =
            PageBuilderCapabilityPermissions::default().required_for(capability_kind);
        if !has_effective_permission(&auth.permissions, &required_permission) {
            return Err(async_graphql::Error::new(format!(
                "Pages permission `{required_permission}` is required for Page Builder `{capability_kind}` preflight"
            ))
            .extend_with(|_, ext| ext.set("code", "FORBIDDEN")));
        }

        let flags = load_rollout_flags(db, tenant).await?;
        match ensure_capability(&flags, capability_kind) {
            Ok(()) => Ok(GqlPageBuilderCapabilityPreflight::allowed(capability)),
            Err(BuilderRolloutError::CapabilityDisabled(_)) => Ok(
                GqlPageBuilderCapabilityPreflight::feature_disabled(capability),
            ),
            Err(BuilderRolloutError::InvalidFlagCombination(message)) => {
                Err(rollout_invalid_error(message))
            }
        }
    }
}

async fn load_rollout_flags(
    db: &DatabaseConnection,
    tenant: &TenantContext,
) -> Result<BuilderCapabilityFlags> {
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
    BuilderCapabilityFlags::from_module_settings(&settings)
        .map_err(|error| rollout_invalid_error(error.to_string()))
}

fn rollout_invalid_error(message: impl std::fmt::Display) -> async_graphql::Error {
    async_graphql::Error::new(format!(
        "Pages Page Builder rollout settings are invalid: {message}"
    ))
    .extend_with(|_, ext| ext.set("code", "PAGE_BUILDER_ROLLOUT_INVALID"))
}

fn ensure_tenant_authority(auth: &AuthContext, tenant: &TenantContext) -> Result<()> {
    if auth.tenant_id != tenant.id {
        return Err(async_graphql::Error::new("Pages Page Builder rollout access is denied")
            .extend_with(|_, ext| ext.set("code", "FORBIDDEN")));
    }
    Ok(())
}

fn ensure_pages_read_authority(auth: &AuthContext, tenant: &TenantContext) -> Result<()> {
    ensure_tenant_authority(auth, tenant)?;
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

    #[test]
    fn capability_preflight_uses_canonical_feature_disabled_contract() {
        let disabled = GqlPageBuilderCapabilityPreflight::feature_disabled(
            GqlPageBuilderCapability::Publish,
        );
        assert!(!disabled.allowed);
        assert_eq!(disabled.error_kind.as_deref(), Some("feature-disabled"));
        assert_eq!(
            disabled.error_code.as_deref(),
            Some(PAGE_BUILDER_FEATURE_DISABLED_ERROR_CODE)
        );

        let allowed =
            GqlPageBuilderCapabilityPreflight::allowed(GqlPageBuilderCapability::Preview);
        assert!(allowed.allowed);
        assert!(allowed.error_kind.is_none());
        assert!(allowed.error_code.is_none());
    }
}
