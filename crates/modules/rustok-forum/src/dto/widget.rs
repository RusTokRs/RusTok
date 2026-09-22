use serde::{Deserialize, Serialize};
use serde_json::Value;
use utoipa::ToSchema;

use super::{reply::ReplyResponse, topic::TopicListItem, topic::TopicResponse};

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

/// Forum-owned Page Builder preview input. `props` is always normalized through
/// `ForumWidgetContractService` before any owner read executes.
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct PreviewForumWidgetInput {
    pub widget_type: String,
    #[serde(default)]
    pub props: Value,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct ForumWidgetPreviewResponse {
    pub widget_type: String,
    pub data_contract_version: String,
    pub valid: bool,
    pub normalized_props: Value,
    pub issues: Vec<ForumWidgetValidationIssue>,
    pub payload: Option<ForumWidgetPreviewPayload>,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum ForumWidgetPreviewPayload {
    TopicList(ForumTopicListWidgetPreview),
    TopicDetail(ForumTopicDetailWidgetPreview),
    ReplyStream(ForumReplyStreamWidgetPreview),
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct ForumTopicListWidgetPreview {
    pub items: Vec<TopicListItem>,
    pub total: u64,
    pub page: u64,
    pub per_page: u64,
    pub sort: String,
    pub include_pinned: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct ForumTopicDetailWidgetPreview {
    pub topic: TopicResponse,
    pub replies: Vec<ReplyResponse>,
    pub replies_total: u64,
    pub include_replies: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct ForumReplyStreamWidgetPreview {
    pub topic_id: String,
    pub items: Vec<ReplyResponse>,
    pub total: u64,
    pub page: u64,
    pub per_page: u64,
    pub approved_only: bool,
}
