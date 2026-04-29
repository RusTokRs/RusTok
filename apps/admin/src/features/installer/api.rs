use chrono::{DateTime, Utc};
use rustok_installer::{InstallPlan, PreflightReport};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use uuid::Uuid;

use crate::shared::api::{api_base_url, extract_http_error};

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct InstallStatusResponse {
    pub status: String,
    pub initialized: bool,
    pub completed: bool,
    pub session_id: Option<Uuid>,
    pub tenant_id: Option<Uuid>,
    pub lock_owner: Option<String>,
    pub lock_expires_at: Option<DateTime<Utc>>,
    pub completed_at: Option<DateTime<Utc>>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct InstallPreflightResponse {
    pub passed: bool,
    pub report: PreflightReport,
    pub redacted_plan: Value,
}

#[derive(Clone, Debug, Serialize)]
pub struct InstallApplyRequest {
    pub plan: InstallPlan,
    pub lock_owner: Option<String>,
    pub lock_ttl_secs: Option<i64>,
    pub pg_admin_url: Option<String>,
}

#[derive(Clone, Copy, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum InstallJobState {
    Running,
    Succeeded,
    Failed,
}

impl InstallJobState {
    pub fn label(self) -> &'static str {
        match self {
            Self::Running => "running",
            Self::Succeeded => "succeeded",
            Self::Failed => "failed",
        }
    }
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct InstallApplyJobResponse {
    pub job_id: Uuid,
    pub status: InstallJobState,
    pub submitted_at: DateTime<Utc>,
    pub status_url: String,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct InstallJobStatusResponse {
    pub job_id: Uuid,
    pub status: InstallJobState,
    pub submitted_at: DateTime<Utc>,
    pub started_at: Option<DateTime<Utc>>,
    pub finished_at: Option<DateTime<Utc>>,
    pub session_id: Option<Uuid>,
    pub tenant_id: Option<Uuid>,
    pub output: Option<Value>,
    pub error: Option<String>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct InstallReceiptsResponse {
    pub session_id: Uuid,
    pub receipts: Vec<InstallReceiptRow>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct InstallReceiptRow {
    pub id: Uuid,
    pub session_id: Uuid,
    pub step: String,
    pub outcome: String,
    pub input_checksum: String,
    pub diagnostics: Value,
    pub installer_version: String,
    pub created_at: DateTime<Utc>,
}

pub async fn fetch_status() -> Result<InstallStatusResponse, String> {
    get_json("/api/install/status", None).await
}

pub async fn preflight(
    plan: InstallPlan,
    setup_token: String,
) -> Result<InstallPreflightResponse, String> {
    post_json("/api/install/preflight", Some(setup_token), &plan).await
}

pub async fn apply(
    request: InstallApplyRequest,
    setup_token: String,
) -> Result<InstallApplyJobResponse, String> {
    post_json("/api/install/apply", Some(setup_token), &request).await
}

pub async fn fetch_job(
    job_id: Uuid,
    setup_token: String,
) -> Result<InstallJobStatusResponse, String> {
    get_json(&format!("/api/install/jobs/{job_id}"), Some(setup_token)).await
}

pub async fn fetch_receipts(
    session_id: Uuid,
    setup_token: String,
) -> Result<InstallReceiptsResponse, String> {
    get_json(
        &format!("/api/install/sessions/{session_id}/receipts"),
        Some(setup_token),
    )
    .await
}

async fn get_json<TResp>(path: &str, setup_token: Option<String>) -> Result<TResp, String>
where
    TResp: for<'de> Deserialize<'de>,
{
    let client = reqwest::Client::new();
    let mut request = client.get(format!("{}{}", api_base_url(), path));
    if let Some(token) = setup_token.filter(|value| !value.trim().is_empty()) {
        request = request.header("x-rustok-setup-token", token);
    }

    let response = request.send().await.map_err(|err| err.to_string())?;
    if !response.status().is_success() {
        return Err(extract_http_error(response).await);
    }

    response
        .json::<TResp>()
        .await
        .map_err(|err| err.to_string())
}

async fn post_json<TReq, TResp>(
    path: &str,
    setup_token: Option<String>,
    body: &TReq,
) -> Result<TResp, String>
where
    TReq: Serialize + ?Sized,
    TResp: for<'de> Deserialize<'de>,
{
    let client = reqwest::Client::new();
    let mut request = client
        .post(format!("{}{}", api_base_url(), path))
        .header(reqwest::header::CONTENT_TYPE, "application/json")
        .json(body);
    if let Some(token) = setup_token.filter(|value| !value.trim().is_empty()) {
        request = request.header("x-rustok-setup-token", token);
    }

    let response = request.send().await.map_err(|err| err.to_string())?;
    if !response.status().is_success() {
        return Err(extract_http_error(response).await);
    }

    response
        .json::<TResp>()
        .await
        .map_err(|err| err.to_string())
}
