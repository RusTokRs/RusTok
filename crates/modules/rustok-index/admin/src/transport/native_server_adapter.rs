use leptos::prelude::*;

use crate::model::IndexAdminBootstrap;
#[cfg(feature = "ssr")]
use crate::model::{IndexModuleSnapshot, IndexTenantSnapshot};

#[cfg(feature = "ssr")]
fn require_index_admin_tenant_scope(
    auth_tenant_id: uuid::Uuid,
    resolved_tenant_id: uuid::Uuid,
) -> Result<(), ServerFnError> {
    if auth_tenant_id == resolved_tenant_id {
        return Ok(());
    }

    tracing::warn!(
        auth_tenant_id = %auth_tenant_id,
        resolved_tenant_id = %resolved_tenant_id,
        code = "index.admin_tenant_scope_mismatch",
        boundary = "index_admin_native_transport",
        "index admin permissions cannot cross the resolved tenant boundary"
    );
    Err(ServerFnError::new("Index admin access is denied"))
}

#[server(prefix = "/api/fn", endpoint = "index/bootstrap")]
pub async fn fetch_bootstrap_native() -> Result<IndexAdminBootstrap, ServerFnError> {
    #[cfg(feature = "ssr")]
    {
        use rustok_api::{AuthContext, Permission, TenantContext, has_effective_permission};
        use rustok_core::RusToKModule;

        let auth = leptos_axum::extract::<AuthContext>()
            .await
            .map_err(ServerFnError::new)?;
        let tenant = leptos_axum::extract::<TenantContext>()
            .await
            .map_err(ServerFnError::new)?;
        require_index_admin_tenant_scope(auth.tenant_id, tenant.id)?;

        if !has_effective_permission(&auth.permissions, &Permission::SETTINGS_READ) {
            return Err(ServerFnError::new(
                "settings:read required to inspect index administration state",
            ));
        }

        let module = rustok_index::IndexModule;
        Ok(IndexAdminBootstrap {
            tenant: IndexTenantSnapshot {
                id: tenant.id.to_string(),
                slug: tenant.slug,
                name: tenant.name,
                default_locale: tenant.default_locale,
            },
            module: IndexModuleSnapshot {
                slug: module.slug().to_string(),
                name: module.name().to_string(),
                description: module.description().to_string(),
                rewrite_status: "in_progress".to_string(),
                current_milestone: "M0/M1".to_string(),
            },
        })
    }
    #[cfg(not(feature = "ssr"))]
    {
        Err(ServerFnError::new(
            "rustok-index-admin requires the `ssr` feature for native bootstrap",
        ))
    }
}
