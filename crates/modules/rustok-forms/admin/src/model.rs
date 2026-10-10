use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct FormSubmission {
    pub id: Uuid,
    #[serde(rename = "formId")]
    pub form_id: String,
    pub locale: String,
    #[serde(rename = "pageId")]
    pub page_id: Option<Uuid>,
    pub payload: serde_json::Value,
    pub state: String,
    #[serde(rename = "createdAt")]
    pub created_at: String,
    #[serde(rename = "handledAt")]
    pub handled_at: Option<String>,
    #[serde(rename = "handledBy")]
    pub handled_by: Option<Uuid>,
}
