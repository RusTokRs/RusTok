use serde::{Deserialize, Serialize};

use crate::entities::module::{ModuleInfo, ToggleModuleResult};
use crate::shared::api::{request, ApiError};

pub const MODULE_REGISTRY_QUERY: &str =
    "query ModuleRegistry { moduleRegistry { moduleSlug name description version kind dependencies enabled } }";

pub const TOGGLE_MODULE_MUTATION: &str =
    "mutation ToggleModule($moduleSlug: String!, $enabled: Boolean!) { toggleModule(moduleSlug: $moduleSlug, enabled: $enabled) { moduleSlug enabled settings } }";

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct ModuleRegistryResponse {
    #[serde(rename = "moduleRegistry")]
    pub module_registry: Vec<ModuleInfo>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct ToggleModuleResponse {
    #[serde(rename = "toggleModule")]
    pub toggle_module: ToggleModuleResult,
}

#[derive(Clone, Debug, Serialize)]
pub struct ToggleModuleVariables {
    #[serde(rename = "moduleSlug")]
    pub module_slug: String,
    pub enabled: bool,
}

pub async fn fetch_modules(
    token: Option<String>,
    tenant_slug: Option<String>,
) -> Result<Vec<ModuleInfo>, ApiError> {
    let response: ModuleRegistryResponse =
        request(MODULE_REGISTRY_QUERY, serde_json::json!({}), token, tenant_slug).await?;
    Ok(response.module_registry)
}

pub async fn toggle_module(
    module_slug: String,
    enabled: bool,
    token: Option<String>,
    tenant_slug: Option<String>,
) -> Result<ToggleModuleResult, ApiError> {
    let variables = ToggleModuleVariables {
        module_slug,
        enabled,
    };
    let response: ToggleModuleResponse =
        request(TOGGLE_MODULE_MUTATION, variables, token, tenant_slug).await?;
    Ok(response.toggle_module)
}
