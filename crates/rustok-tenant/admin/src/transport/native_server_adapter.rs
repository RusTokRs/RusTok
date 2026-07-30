use leptos::prelude::*;
use serde::{Deserialize, Serialize};
use std::fmt::{Display, Formatter};

use crate::model::TenantAdminBootstrap;
#[cfg(feature = "ssr")]
use crate::model::{TenantAdminModule, TenantAdminTenant};
#[cfg(feature = "ssr")]
use std::collections::{HashMap, HashSet};

#[cfg(feature = "ssr")]
#[derive(Debug, Deserialize, Default)]
struct RuntimeModulesManifest {
    #[serde(default)]
    settings: RuntimeSettingsManifest,
}

#[cfg(feature = "ssr")]
#[derive(Debug, Deserialize, Default)]
struct RuntimeSettingsManifest {
    #[serde(default)]
    default_enabled: Vec<String>,
}

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

#[server(prefix = "/api/fn", endpoint = "tenant/bootstrap")]
pub async fn tenant_bootstrap_native() -> Result<TenantAdminBootstrap, ServerFnError> {
    #[cfg(feature = "ssr")]
    {
        use leptos::prelude::expect_context;
        use rustok_api::Permission;
        use rustok_api::{
            AuthContext, HostRuntimeContext, TenantContext, has_any_effective_permission,
        };
        use rustok_core::ModuleRegistry;
        use rustok_tenant::TenantService;

        let runtime_ctx = expect_context::<HostRuntimeContext>();
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

        let db = runtime_ctx.db_clone();
        let service = TenantService::new(db.clone());
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

        let control_plane = rustok_modules::ModuleControlPlane::new(db);
        let snapshot = control_plane
            .composition()
            .active_snapshot()
            .await
            .map_err(ServerFnError::new)?;
        let manifest: RuntimeModulesManifest =
            serde_json::from_value(snapshot.manifest).map_err(|error| {
                ServerFnError::new(format!("invalid active module manifest: {error}"))
            })?;
        let manifest_defaults = manifest
            .settings
            .default_enabled
            .iter()
            .cloned()
            .collect::<HashSet<_>>();
        let effective_modules = control_plane
            .effective_policy(&registry, manifest.settings.default_enabled)
            .resolve_enabled(tenant.id)
            .await
            .map_err(ServerFnError::new)?;

        let mut modules = registry
            .list()
            .into_iter()
            .map(|module| {
                let is_core = registry.is_core(module.slug());
                let explicit = explicit_modules.get(module.slug()).copied();
                let enabled = effective_modules.contains(module.slug());
                TenantAdminModule {
                    slug: module.slug().to_string(),
                    name: module.name().to_string(),
                    description: module.description().to_string(),
                    kind: if is_core { "core" } else { "optional" }.to_string(),
                    enabled,
                    source: if is_core {
                        "core-default".to_string()
                    } else if explicit.is_some() {
                        "tenant-override".to_string()
                    } else if manifest_defaults.contains(module.slug()) {
                        "manifest-default".to_string()
                    } else if enabled {
                        "policy-dependency".to_string()
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
