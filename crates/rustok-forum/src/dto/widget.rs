use serde::{Deserialize, Serialize};
use serde_json::Value;
use utoipa::ToSchema;

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct ForumWidgetCatalogResponse {
    pub catalog_version: String,
    pub builder_contract_version: String,
    pub consumer_min_version: String,
    pub compatibility_matrix: Vec<ForumWidgetCompatibilityEntry>,
    pub items: Vec<ForumWidgetCatalogItem>,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct ForumWidgetCompatibilityEntry {
    pub provider_contract_version: String,
    pub consumer_min_version: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct ForumWidgetCatalogItem {
    pub widget_type: String,
    pub data_contract_version: String,
    pub props_schema: Value,
    pub capability_requirements: ForumWidgetCapabilityRequirements,
    pub fallback_mode: String,
    pub error_mapping: ForumWidgetErrorMapping,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct ForumWidgetCapabilityRequirements {
    pub preview: bool,
    pub publish: bool,
    pub moderation_view: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct ForumWidgetErrorMapping {
    pub validation: String,
    pub sanitize: String,
    pub rbac: String,
    pub runtime: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct ValidateForumWidgetPropsInput {
    pub widget_type: String,
    #[serde(default)]
    pub props: Value,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct ForumWidgetPropsValidationResponse {
    pub widget_type: String,
    pub valid: bool,
    pub normalized_props: Value,
    pub issues: Vec<ForumWidgetValidationIssue>,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct ForumWidgetValidationIssue {
    pub class: String,
    pub code: String,
    pub message: String,
    pub path: Option<String>,
}
