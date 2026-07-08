use serde::{Deserialize, Serialize};
use serde_json::Value;

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct CreateWorkflowInput {
    pub name: String,
    pub description: Option<String>,
    #[serde(rename = "triggerConfig")]
    pub trigger_config: Value,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct UpdateWorkflowInput {
    pub name: Option<String>,
    pub description: Option<String>,
    pub status: Option<String>,
    #[serde(rename = "triggerConfig")]
    pub trigger_config: Option<Value>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct CreateStepInput {
    pub position: i32,
    #[serde(rename = "stepType")]
    pub step_type: String,
    pub config: Value,
    #[serde(rename = "onError")]
    pub on_error: String,
    #[serde(rename = "timeoutMs")]
    pub timeout_ms: Option<i64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorkflowTemplateDto {
    pub id: String,
    pub name: String,
    pub description: String,
    pub category: String,
    #[serde(rename = "triggerConfig")]
    pub trigger_config: Value,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorkflowVersionSummaryDto {
    pub id: String,
    pub version: i32,
    #[serde(rename = "createdBy")]
    pub created_by: Option<String>,
    #[serde(rename = "createdAt")]
    pub created_at: String,
}
