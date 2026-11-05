use serde::{Deserialize, Serialize};
use serde_json::Value;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct BlogWidgetPropertyRequest {
    pub widget_type: String,
    pub property_schema: Value,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct BlogWidgetPropertySchema {
    pub schema_id: String,
    pub schema: Value,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct BlogWidgetValidationRequest {
    pub widget_type: String,
    pub property_schema: Value,
    pub props: Value,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct BlogWidgetPreviewRequest {
    pub widget_type: String,
    pub props: Value,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct BlogWidgetValidation {
    pub widget_type: String,
    pub valid: bool,
    pub normalized_props: Value,
    pub issues: Vec<BlogWidgetIssue>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct BlogWidgetIssue {
    pub class: String,
    pub code: String,
    pub message: String,
    pub path: Option<String>,
}

#[derive(Debug, Deserialize)]
pub(crate) struct WidgetCatalog {
    pub items: Vec<WidgetSchema>,
}

#[derive(Debug, Deserialize)]
pub(crate) struct WidgetSchema {
    pub widget_type: String,
    pub schema_id: String,
    pub props_schema: Value,
}
