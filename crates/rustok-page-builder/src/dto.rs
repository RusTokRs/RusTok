use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum BuilderCapabilityKind {
    Preview,
    Tree,
    Properties,
    Publish,
}

impl BuilderCapabilityKind {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Preview => "preview",
            Self::Tree => "tree",
            Self::Properties => "properties",
            Self::Publish => "publish",
        }
    }
}

impl std::fmt::Display for BuilderCapabilityKind {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.as_str())
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct BuilderTreeNode {
    pub id: String,
    pub label: String,
    #[serde(default)]
    pub children: Vec<BuilderTreeNode>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PreviewPageBuilderInput {
    pub page_id: String,
    pub schema_version: String,
    pub project_data: serde_json::Value,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PreviewPageBuilderResult {
    pub page_id: String,
    pub html: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct BuilderTreeInput {
    pub page_id: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct BuilderTreeResult {
    pub page_id: String,
    #[serde(default)]
    pub nodes: Vec<BuilderTreeNode>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct BuilderNodePropertiesInput {
    pub page_id: String,
    pub node_id: String,
    pub properties: serde_json::Value,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct BuilderNodePropertiesResult {
    pub page_id: String,
    pub node_id: String,
    pub properties: serde_json::Value,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PublishPageBuilderInput {
    pub page_id: String,
    pub revision_id: String,
    pub schema_version: String,
    pub project_data: serde_json::Value,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PublishPageBuilderResult {
    pub page_id: String,
    pub revision_id: String,
    pub published: bool,
}
