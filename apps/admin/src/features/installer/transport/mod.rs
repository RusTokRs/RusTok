use rustok_installer::InstallPlan;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::shared::api::{api_base_url, extract_http_error};

pub use crate::features::installer::model::{
    InstallApplyJobResponse, InstallApplyRequest, InstallJobState, InstallJobStatusResponse,
    InstallPreflightResponse, InstallReceiptsResponse, InstallStatusResponse,
};

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
