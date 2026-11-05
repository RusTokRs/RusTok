use serde::{Deserialize, Serialize};
use serde_json::Value;
use utoipa::ToSchema;

use super::{CategoryListItem, PostListResponse, PostResponse};

/// Configuration only. Identity and effective context are supplied by the transport.
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
#[serde(deny_unknown_fields)]
pub struct BlogWidgetInput {
    pub widget_type: String,
    pub props: Value,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct BlogWidgetCatalog {
    pub catalog_version: String,
    pub items: Vec<BlogWidgetSchema>,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct BlogWidgetSchema {
    pub widget_type: String,
    pub schema_id: String,
    pub props_schema: Value,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, ToSchema)]
pub struct BlogWidgetIssue {
    pub class: String,
    pub code: String,
    pub message: String,
    pub path: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct BlogWidgetValidation {
    pub widget_type: String,
    pub valid: bool,
    pub normalized_props: Value,
    pub issues: Vec<BlogWidgetIssue>,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct BlogWidgetPreview {
    pub widget_type: String,
    pub valid: bool,
    pub normalized_props: Value,
    pub issues: Vec<BlogWidgetIssue>,
    pub payload: Option<BlogWidgetPayload>,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum BlogWidgetPayload {
    PostList(PostListResponse),
    PostDetail(Box<PostResponse>),
    CategoryList {
        items: Vec<CategoryListItem>,
        total: u64,
        page: u64,
        per_page: u64,
    },
}
