use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct PageBuilderModuleMetadata {
    pub module_slug: &'static str,
    pub capabilities: &'static [&'static str],
}

impl PageBuilderModuleMetadata {
    pub const CURRENT: Self = Self {
        module_slug: "page_builder",
        capabilities: &["preview", "tree", "properties", "publish"],
    };
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

fn empty_runtime_context() -> serde_json::Value {
    serde_json::Value::Object(serde_json::Map::new())
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PageBuilderPreviewRuntime {
    #[serde(default = "empty_runtime_context")]
    pub context: serde_json::Value,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub scenario_id: Option<String>,
}

impl Default for PageBuilderPreviewRuntime {
    fn default() -> Self {
        Self {
            context: empty_runtime_context(),
            scenario_id: None,
        }
    }
}

impl PageBuilderPreviewRuntime {
    pub fn new(context: serde_json::Value, scenario_id: Option<String>) -> Self {
        Self {
            context,
            scenario_id,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PreviewPageBuilderInput {
    pub page_id: String,
    pub project_data: serde_json::Value,
    #[serde(default)]
    pub runtime: PageBuilderPreviewRuntime,
}

impl PreviewPageBuilderInput {
    pub fn new(page_id: impl Into<String>, project_data: serde_json::Value) -> Self {
        Self {
            page_id: page_id.into(),
            project_data,
            runtime: PageBuilderPreviewRuntime::default(),
        }
    }

    pub fn with_runtime(mut self, runtime: PageBuilderPreviewRuntime) -> Self {
        self.runtime = runtime;
        self
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PreviewPageBuilderResult {
    pub page_id: String,
    pub html: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub runtime_scenario_id: Option<String>,
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
    pub project_data: serde_json::Value,
}

impl PublishPageBuilderInput {
    pub fn new(
        page_id: impl Into<String>,
        revision_id: impl Into<String>,
        project_data: serde_json::Value,
    ) -> Self {
        Self {
            page_id: page_id.into(),
            revision_id: revision_id.into(),
            project_data,
        }
    }
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
