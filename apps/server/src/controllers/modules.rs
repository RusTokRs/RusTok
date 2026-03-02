use axum::extract::State;
use axum::Extension;
use axum::Json;
use loco_rs::prelude::*;
use rustok_core::ModuleRegistry;
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

use crate::context::TenantContext;
use crate::extractors::rbac::{RequireSettingsRead, RequireSettingsUpdate};
use crate::models::_entities::tenant_modules;
use crate::services::module_lifecycle::ModuleLifecycleService;

#[derive(Debug, Serialize)]
pub struct ModuleInfo {
    pub slug: String,
    pub name: String,
    pub description: String,
    pub version: String,
    pub kind: &'static str,
    pub dependencies: Vec<String>,
    pub enabled: bool,
}

#[derive(Debug, Serialize)]
pub struct ModulesListResponse {
    pub modules: Vec<ModuleInfo>,
}

#[derive(Debug, Deserialize, ToSchema)]
pub struct ToggleModuleInput {
    pub enabled: bool,
}

/// GET /api/modules — list all registered modules with their enabled status
#[utoipa::path(
    get,
    path = "/api/modules",
    tag = "modules",
    responses(
        (status = 200, description = "List of all registered modules"),
        (status = 401, description = "Unauthorized"),
        (status = 403, description = "Forbidden")
    )
)]
pub async fn list_modules(
    State(ctx): State<AppContext>,
    tenant: TenantContext,
    _user: RequireSettingsRead,
    Extension(registry): Extension<ModuleRegistry>,
) -> Result<Json<ModulesListResponse>> {
    let enabled_slugs = tenant_modules::Entity::find_enabled(&ctx.db, tenant.id)
        .await
        .map_err(|e| Error::BadRequest(e.to_string()))?;

    let modules = registry
        .list()
        .into_iter()
        .map(|m| {
            let is_core = registry.is_core(m.slug());
            ModuleInfo {
                slug: m.slug().to_string(),
                name: m.name().to_string(),
                description: m.description().to_string(),
                version: m.version().to_string(),
                kind: if is_core { "core" } else { "optional" },
                dependencies: m.dependencies().iter().map(|d| d.to_string()).collect(),
                enabled: is_core || enabled_slugs.contains(&m.slug().to_string()),
            }
        })
        .collect();

    Ok(Json(ModulesListResponse { modules }))
}

/// PUT /api/modules/:slug/toggle — enable or disable an optional module
#[utoipa::path(
    put,
    path = "/api/modules/{slug}/toggle",
    tag = "modules",
    params(
        ("slug" = String, Path, description = "Module slug")
    ),
    request_body = ToggleModuleInput,
    responses(
        (status = 200, description = "Module toggled"),
        (status = 400, description = "Cannot toggle core module"),
        (status = 404, description = "Module not found"),
        (status = 401, description = "Unauthorized"),
        (status = 403, description = "Forbidden")
    )
)]
pub async fn toggle_module(
    State(ctx): State<AppContext>,
    tenant: TenantContext,
    _user: RequireSettingsUpdate,
    Extension(registry): Extension<ModuleRegistry>,
    axum::extract::Path(slug): axum::extract::Path<String>,
    Json(input): Json<ToggleModuleInput>,
) -> Result<Json<ModuleInfo>> {
    let module = registry.get(&slug).ok_or_else(|| Error::NotFound)?;

    let record =
        ModuleLifecycleService::toggle_module(&ctx.db, &registry, tenant.id, &slug, input.enabled)
            .await
            .map_err(|e| Error::BadRequest(e.to_string()))?;

    let is_core = registry.is_core(&slug);
    Ok(Json(ModuleInfo {
        slug: module.slug().to_string(),
        name: module.name().to_string(),
        description: module.description().to_string(),
        version: module.version().to_string(),
        kind: if is_core { "core" } else { "optional" },
        dependencies: module
            .dependencies()
            .iter()
            .map(|d| d.to_string())
            .collect(),
        enabled: record.enabled,
    }))
}

pub fn routes() -> Routes {
    Routes::new()
        .prefix("api/modules")
        .add("/", get(list_modules))
        .add("/:slug/toggle", put(toggle_module))
}
