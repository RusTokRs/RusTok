use serde::{Deserialize, Serialize};
use utoipa::{IntoParams, ToSchema};
use uuid::Uuid;

/// Which triage state a submission is in.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "snake_case")]
pub enum FormSubmissionState {
    New,
    Read,
    Handled,
    Spam,
}

impl FormSubmissionState {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::New => "new",
            Self::Read => "read",
            Self::Handled => "handled",
            Self::Spam => "spam",
        }
    }

    pub fn parse(value: &str) -> Option<Self> {
        match value {
            "new" => Some(Self::New),
            "read" => Some(Self::Read),
            "handled" => Some(Self::Handled),
            "spam" => Some(Self::Spam),
            _ => None,
        }
    }
}

/// Result of one intake call; honeypot captures answer with the same shape.
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct SubmitFormResponse {
    pub id: Uuid,
    pub accepted: bool,
}

/// One stored form submission with its triage state.
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct FormSubmissionResponse {
    pub id: Uuid,
    pub form_id: String,
    pub locale: String,
    pub page_id: Option<Uuid>,
    pub payload: serde_json::Value,
    pub state: FormSubmissionState,
    pub created_at: String,
    pub handled_at: Option<String>,
    pub handled_by: Option<Uuid>,
}

#[derive(Debug, Clone, Serialize, Deserialize, IntoParams, ToSchema)]
pub struct ListFormSubmissionsFilter {
    pub form_id: Option<String>,
    pub state: Option<FormSubmissionState>,
    #[serde(default = "default_page")]
    pub page: u64,
    #[serde(default = "default_per_page")]
    pub per_page: u64,
}

fn default_page() -> u64 {
    1
}

fn default_per_page() -> u64 {
    20
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct UpdateFormSubmissionStateInput {
    pub state: FormSubmissionState,
}
