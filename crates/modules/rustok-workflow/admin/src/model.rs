use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum WorkflowStatus {
    Draft,
    Active,
    Paused,
    Archived,
    #[serde(other)]
    Unknown,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct WorkflowSummary {
    pub id: String,
    #[serde(rename = "tenantId")]
    pub tenant_id: String,
    pub name: String,
    pub status: WorkflowStatus,
    #[serde(rename = "failureCount")]
    pub failure_count: i32,
    #[serde(rename = "createdAt")]
    pub created_at: String,
    #[serde(rename = "updatedAt")]
    pub updated_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorkflowTemplateDto {
    pub id: String,
    pub name: String,
    pub description: String,
    pub category: String,
    #[serde(rename = "triggerConfig")]
    pub trigger_config: serde_json::Value,
}
