use leptos::prelude::*;
use serde::{Deserialize, Serialize};
use std::fmt::{Display, Formatter};

use crate::model::TenantAdminBootstrap;
#[cfg(feature = "ssr")]
use crate::model::{TenantAdminModule, TenantAdminTenant};
#[cfg(feature = "ssr")]
use std::collections::{HashMap, HashSet};
#[cfg(feature = "ssr")]
use std::time::{SystemTime, UNIX_EPOCH};

#[cfg(feature = "ssr")]
const TENANT_ADMIN_OWNER: &str = "rustok_tenant.admin_transport";
#[cfg(feature = "ssr")]
const TENANT_ADMIN_BOUNDARY: &str = "tenant_admin_native_transport";

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

#[cfg(feature = "ssr")]
fn tenant_admin_correlation_id() -> String {
    let timestamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos();
    format!("tenant-admin:bootstrap:{timestamp}")
}

#[cfg(feature = "ssr")]
fn tenant_admin_scope_matches(auth_tenant_id: uuid::Uuid, resolved_tenant_id: uuid::Uuid) -> bool {
    auth_tenant_id == resolved_tenant_id
}

#[cfg(feature = "ssr")]
fn tenant_admin_context_error<E: std::fmt::Debug>(
    error: E,
    context_kind: &'static str,
    correlation_id: &str,
    code: &'static str,
    public_message: &'static str,
) -> ServerFnError {
    tracing::error!(
        error = ?error,
        owner = TENANT_ADMIN_OWNER,
        owner_operation = "tenant_bootstrap_native",
        context_kind,
        correlation_id,
        code,
        boundary = TENANT_ADMIN_BOUNDARY,
        "tenant admin request context extraction failed"
    );
    ServerFnError::new(public_message)
}

#[cfg(feature = "ssr")]
fn tenant_admin_owner_error(
    error: rustok_tenant::TenantError,
    owner_operation: &'static str,
    correlation_id: &str,
    tenant_id: uuid::Uuid,
    public_message: &'static str,
) -> ServerFnError {
    match error {
        rustok_tenant::TenantError::NotFound => {
            tracing::warn!(
                owner = "rustok_tenant",
                consumer = TENANT_ADMIN_OWNER,
                owner_operation,
                correlation_id,
                tenant_id = %tenant_id,
                code = "tenant.admin_not_found",
                boundary = TENANT_ADMIN_BOUNDARY,
                "tenant admin owner record was not found"
            );
            ServerFnError::new("Tenant was not found")
        }
        error => {
            tracing::error!(
                error = ?error,
                owner = "rustok_tenant",
                consumer = TENANT_ADMIN_OWNER,
                owner_operation,
                correlation_id,
                tenant_id = %tenant_id,
                code = "tenant.admin_owner_unavailable",
                boundary = TENANT_ADMIN_BOUNDARY,
                "tenant admin owner operation failed"
            );
            ServerFnError::new(public_message)
        }
    }
}

#[cfg(feature = "ssr")]
fn tenant_admin_internal_error<E: std::fmt::Debug>(
    error: E,
    owner_operation: &'static str,
    correlation_id: &str,
    tenant_id: uuid::Uuid,
    code: &'static str,
    public_message: &'static str,
) -> ServerFnError {
    tracing::error!(
        error = ?error,
        owner = TENANT_ADMIN_OWNER,
        owner_operation,
        correlation_id,
        tenant_id = %tenant_id,
        code,
        boundary = TENANT_ADMIN_BOUNDARY,
        "tenant admin internal operation failed"
    );
    ServerFnError::new(public_message)
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

        let correlation_id = tenant_admin_correlation_id();
        let runtime_ctx = expect_context::<HostRuntimeContext>();
        let registry = expect_context::<ModuleRegistry>();
        let auth = leptos_axum::extract::<AuthContext>()
            .await
            .map_err(|error| {
                tenant_admin_context_error(
                    error,
                    "auth",
                    correlation_id.as_str(),
                    "tenant.admin_auth_context_unavailable",
                    "Tenant authentication context is temporarily unavailable",
                )
            })?;
        let tenant = leptos_axum::extract::<TenantContext>()
            .await
            .map_err(|error| {
                tenant_admin_context_error(
                    error,
                    "tenant",
                    correlation_id.as_str(),
                    "tenant.admin_tenant_context_unavailable",
                    "Tenant context is temporarily unavailable",
                )
            })?;

        if !tenant_admin_scope_matches(auth.tenant_id, tenant.id) {
            tracing::warn!(
                owner = TENANT_ADMIN_OWNER,
                owner_operation = "tenant_bootstrap_native",
                correlation_id,
                auth_tenant_id = %auth.tenant_id,
                resolved_tenant_id = %tenant.id,
                code = "tenant.admin_tenant_scope_mismatch",
                boundary = TENANT_ADMIN_BOUNDARY,
                "tenant admin auth and resolved tenant scopes do not match"
            );
            return Err(ServerFnError::new("Tenant admin access is denied"));
        }

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
            tracing::warn!(
                owner = TENANT_ADMIN_OWNER,
                owner_operation = "tenant_bootstrap_native",
                correlation_id,
                tenant_id = %tenant.id,
                permission_count = auth.permissions.len(),
                code = "tenant.admin_access_denied",
                boundary = TENANT_ADMIN_BOUNDARY,
                "tenant admin request denied"
            );
            return Err(ServerFnError::new("Tenant admin access is denied"));
        }

        let db = runtime_ctx.db_clone();
        let service = TenantService::new(db.clone());
        let tenant_record = service
            .get_tenant(tenant.id)
            .await
            .map_err(|error| {
                tenant_admin_owner_error(
                    error,
                    "get_tenant",
                    correlation_id.as_str(),
                    tenant.id,
                    "Tenant data is temporarily unavailable",
                )
            })?;
        let explicit_modules = service
            .list_tenant_modules(tenant.id)
            .await
            .map_err(|error| {
                tenant_admin_owner_error(
                    error,
                    "list_tenant_modules",
                    correlation_id.as_str(),
                    tenant.id,
                    "Tenant module state is temporarily unavailable",
                )
            })?
            .into_iter()
            .map(|module| (module.module_slug, module.enabled))
            .collect::<HashMap<_, _>>();

        let control_plane = rustok_modules::ModuleControlPlane::new(db);
        let snapshot = control_plane
            .composition()
            .active_snapshot()
            .await
            .map_err(|error| {
                tenant_admin_internal_error(
                    error,
                    "active_snapshot",
                    correlation_id.as_str(),
                    tenant.id,
                    "tenant.admin_composition_unavailable",
                    "Module composition is temporarily unavailable",
                )
            })?;
        let manifest: RuntimeModulesManifest = serde_json::from_value(snapshot.manifest).map_err(
            |error| {
                tenant_admin_internal_error(
                    error,
                    "decode_active_manifest",
                    correlation_id.as_str(),
                    tenant.id,
                    "tenant.admin_manifest_invalid",
                    "Module configuration is temporarily unavailable",
                )
            },
        )?;
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
            .map_err(|error| {
                tenant_admin_internal_error(
                    error,
                    "resolve_enabled",
                    correlation_id.as_str(),
                    tenant.id,
                    "tenant.admin_effective_policy_unavailable",
                    "Effective module policy is temporarily unavailable",
                )
            })?;

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

#[cfg(all(test, feature = "ssr"))]
mod tests {
    use super::tenant_admin_scope_matches;
    use uuid::Uuid;

    #[test]
    fn tenant_admin_scope_requires_matching_tenant() {
        let tenant_id = Uuid::new_v4();
        assert!(tenant_admin_scope_matches(tenant_id, tenant_id));
        assert!(!tenant_admin_scope_matches(tenant_id, Uuid::new_v4()));
    }
}
