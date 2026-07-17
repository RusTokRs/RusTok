use fly::GRAPESJS_V1;
use serde::{Deserialize, Serialize};

pub const PAGE_BUILDER_SUPPORTED_DOCUMENT_CONTRACTS: [&str; 1] = [GRAPESJS_V1];

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct PageBuilderContractMetadata {
    pub module_slug: &'static str,
    pub contract: &'static str,
    pub builder_contract_version: &'static str,
    pub consumer_min_version: &'static str,
    pub capabilities: &'static [&'static str],
}

impl PageBuilderContractMetadata {
    pub const BASELINE: Self = Self {
        module_slug: "page_builder",
        contract: GRAPESJS_V1,
        builder_contract_version: "1.0",
        consumer_min_version: "1.0",
        capabilities: &["preview", "tree", "properties", "publish"],
    };

    pub fn supports_document_contract(contract: &str) -> bool {
        PAGE_BUILDER_SUPPORTED_DOCUMENT_CONTRACTS.contains(&contract)
    }
}

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

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum PageBuilderErrorKind {
    Validation,
    Sanitize,
    Runtime,
    FeatureDisabled,
}

impl PageBuilderErrorKind {
    pub const ALL: [Self; 4] = [
        Self::Validation,
        Self::Sanitize,
        Self::Runtime,
        Self::FeatureDisabled,
    ];

    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Validation => "validation",
            Self::Sanitize => "sanitize",
            Self::Runtime => "runtime",
            Self::FeatureDisabled => "feature-disabled",
        }
    }
}

impl std::fmt::Display for PageBuilderErrorKind {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.as_str())
    }
}

pub const PAGE_BUILDER_FEATURE_DISABLED_ERROR_CODE: &str = "FEATURE_DISABLED";

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub struct PageBuilderErrorCatalogEntry {
    pub key: &'static str,
    pub kind: PageBuilderErrorKind,
    pub code: Option<&'static str>,
}

pub const PAGE_BUILDER_ERROR_CATALOG: [PageBuilderErrorCatalogEntry; 4] = [
    PageBuilderErrorCatalogEntry {
        key: "validation",
        kind: PageBuilderErrorKind::Validation,
        code: None,
    },
    PageBuilderErrorCatalogEntry {
        key: "sanitize",
        kind: PageBuilderErrorKind::Sanitize,
        code: None,
    },
    PageBuilderErrorCatalogEntry {
        key: "runtime",
        kind: PageBuilderErrorKind::Runtime,
        code: None,
    },
    PageBuilderErrorCatalogEntry {
        key: "feature_disabled",
        kind: PageBuilderErrorKind::FeatureDisabled,
        code: Some(PAGE_BUILDER_FEATURE_DISABLED_ERROR_CODE),
    },
];

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

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "capability", content = "input", rename_all = "snake_case")]
pub enum PageBuilderCapabilityRequest {
    Preview(PreviewPageBuilderInput),
    Tree(BuilderTreeInput),
    Properties(BuilderNodePropertiesInput),
    Publish(PublishPageBuilderInput),
}

impl PageBuilderCapabilityRequest {
    pub const fn capability(&self) -> BuilderCapabilityKind {
        match self {
            Self::Preview(_) => BuilderCapabilityKind::Preview,
            Self::Tree(_) => BuilderCapabilityKind::Tree,
            Self::Properties(_) => BuilderCapabilityKind::Properties,
            Self::Publish(_) => BuilderCapabilityKind::Publish,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "capability", content = "result", rename_all = "snake_case")]
pub enum PageBuilderCapabilityResponse {
    Preview(PreviewPageBuilderResult),
    Tree(BuilderTreeResult),
    Properties(BuilderNodePropertiesResult),
    Publish(PublishPageBuilderResult),
}

impl PageBuilderCapabilityResponse {
    pub const fn capability(&self) -> BuilderCapabilityKind {
        match self {
            Self::Preview(_) => BuilderCapabilityKind::Preview,
            Self::Tree(_) => BuilderCapabilityKind::Tree,
            Self::Properties(_) => BuilderCapabilityKind::Properties,
            Self::Publish(_) => BuilderCapabilityKind::Publish,
        }
    }
}
