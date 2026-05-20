use leptos::prelude::*;
use serde::{Deserialize, Serialize};
use std::fmt::{Display, Formatter};

use crate::model::TenantAdminBootstrap;
#[cfg(feature = "ssr")]
use crate::model::{TenantAdminModule, TenantAdminTenant};
#[cfg(feature = "ssr")]
use std::collections::HashMap;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ApiError {
    ServerFn(String),
}

impl Display for ApiError {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::ServerFn(error) => write!(f, "{error}"),
        }
    }
}

impl std::error::Error for ApiError {}

impl From<ServerFnError> for ApiError {
    fn from(value: ServerFnError) -> Self {
        Self::ServerFn(value.to_string())
    }
}

pub async fn fetch_bootstrap() -> Result<TenantAdminBootstrap, ApiError> {
    tenant_bootstrap_native().await.map_err(Into::into)
}

#[server(prefix = "/api/fn", endpoint = "tenant/bootstrap")]
async fn tenant_bootstrap_native() -> Result<TenantAdminBootstrap, ServerFnError> {
    #[cfg(feature = "ssr")]
    {
        use leptos::prelude::expect_context;
        use loco_rs::app::AppContext;
        use rustok_api::{has_any_effective_permission, AuthContext, TenantContext};
        use rustok_core::{ModuleRegistry, Permission};
        use rustok_tenant::TenantService;

        let app_ctx = expect_context::<AppContext>();
        let registry = expect_context::<ModuleRegistry>();
        let auth = leptos_axum::extract::<AuthContext>()
            .await
            .map_err(ServerFnError::new)?;
        let tenant = leptos_axum::extract::<TenantContext>()
            .await
            .map_err(ServerFnError::new)?;

        let can_read_tenant = has_any_effective_permission(
            &auth.permissions,
            &[
                Permission::TENANTS_READ,
                Permission::TENANTS_LIST,
                Permission::TENANTS_MANAGE,
            ],
        );
        let can_read_modules = has_any_effective_permission(
            &auth.permissions,
            &[
                Permission::MODULES_READ,
                Permission::MODULES_LIST,
                Permission::MODULES_MANAGE,
            ],
        );
        if !(can_read_tenant && can_read_modules) {
            return Err(ServerFnError::new(
                "tenant admin bootstrap requires tenants:(read|list|manage) and modules:(read|list|manage)",
            ));
        }

        let service = TenantService::new(app_ctx.db.clone());
        let tenant_record = service
            .get_tenant(tenant.id)
            .await
            .map_err(ServerFnError::new)?;
        let explicit_modules = service
            .list_tenant_modules(tenant.id)
            .await
            .map_err(ServerFnError::new)?
            .into_iter()
            .map(|module| (module.module_slug, module.enabled))
            .collect::<HashMap<_, _>>();

        let mut modules = registry
            .list()
            .into_iter()
            .map(|module| {
                let is_core = registry.is_core(module.slug());
                let explicit = explicit_modules.get(module.slug()).copied();
                TenantAdminModule {
                    slug: module.slug().to_string(),
                    name: module.name().to_string(),
                    description: module.description().to_string(),
                    kind: if is_core { "core" } else { "optional" }.to_string(),
                    enabled: if is_core {
                        true
                    } else {
                        explicit.unwrap_or(false)
                    },
                    source: if is_core {
                        "core-default".to_string()
                    } else if explicit.is_some() {
                        "tenant-override".to_string()
                    } else {
                        "disabled".to_string()
                    },
                }
            })
            .collect::<Vec<_>>();
        modules.sort_by(|left, right| left.slug.cmp(&right.slug));

        Ok(TenantAdminBootstrap {
            tenant: TenantAdminTenant {
                id: tenant_record.id.to_string(),
                slug: tenant_record.slug,
                name: tenant_record.name,
                domain: tenant_record.domain,
                is_active: tenant_record.is_active,
                created_at: tenant_record.created_at,
                updated_at: tenant_record.updated_at,
            },
            modules,
        })
    }
    #[cfg(not(feature = "ssr"))]
    {
        Err(ServerFnError::new(
            "rustok-tenant-admin requires the `ssr` feature for native bootstrap",
        ))
    }
}
