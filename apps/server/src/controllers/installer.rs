use axum::{
    extract::{Path, State},
    http::{HeaderMap, StatusCode},
    routing::{get, post},
    Json,
};
use chrono::{DateTime, Utc};
use loco_rs::app::AppContext;
use loco_rs::controller::{ErrorDetail, Routes};
use once_cell::sync::Lazy;
use rustok_installer::{evaluate_preflight, redact_install_plan, InstallPlan};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use tokio::sync::Mutex;
use uuid::Uuid;

use crate::error::{Error, Result};
use crate::installer_cli::{apply_plan, InstallerApplyOptions, InstallerApplyOutput};
use crate::models::install_step_receipt;
use crate::services::installer_persistence::InstallerPersistenceService;

static INSTALL_JOBS: Lazy<Mutex<HashMap<Uuid, InstallJobStatusResponse>>> =
    Lazy::new(|| Mutex::new(HashMap::new()));

#[derive(Debug, Serialize)]
pub struct InstallPlanResponse {
    pub redacted_plan: serde_json::Value,
}

#[derive(Debug, Serialize)]
pub struct InstallPreflightResponse {
    pub passed: bool,
    pub report: rustok_installer::PreflightReport,
    pub redacted_plan: serde_json::Value,
}

#[derive(Debug, Deserialize)]
pub struct InstallApplyRequest {
    pub plan: InstallPlan,
    pub lock_owner: Option<String>,
    pub lock_ttl_secs: Option<i64>,
    pub pg_admin_url: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum InstallJobState {
    Running,
    Succeeded,
    Failed,
}

#[derive(Debug, Clone, Serialize)]
pub struct InstallApplyJobResponse {
    pub job_id: Uuid,
    pub status: InstallJobState,
    pub submitted_at: DateTime<Utc>,
    pub status_url: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct InstallJobStatusResponse {
    pub job_id: Uuid,
    pub status: InstallJobState,
    pub submitted_at: DateTime<Utc>,
    pub started_at: Option<DateTime<Utc>>,
    pub finished_at: Option<DateTime<Utc>>,
    pub session_id: Option<Uuid>,
    pub tenant_id: Option<Uuid>,
    pub output: Option<InstallerApplyOutput>,
    pub error: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct InstallReceiptsResponse {
    pub session_id: Uuid,
    pub receipts: Vec<install_step_receipt::Model>,
}

#[derive(Debug, Serialize)]
pub struct InstallStatusResponse {
    pub status: String,
    pub initialized: bool,
    pub completed: bool,
    pub session_id: Option<Uuid>,
    pub tenant_id: Option<Uuid>,
    pub lock_owner: Option<String>,
    pub lock_expires_at: Option<chrono::DateTime<chrono::Utc>>,
    pub completed_at: Option<chrono::DateTime<chrono::Utc>>,
}

async fn status(State(ctx): State<AppContext>) -> Result<Json<InstallStatusResponse>> {
    let persistence = InstallerPersistenceService::new(ctx.db.clone());
    match persistence.latest_session().await {
        Ok(Some(session)) => {
            let completed = session.status == "completed";
            Ok(Json(InstallStatusResponse {
                status: session.status,
                initialized: true,
                completed,
                session_id: Some(session.id),
                tenant_id: session.tenant_id,
                lock_owner: session.lock_owner,
                lock_expires_at: session.lock_expires_at,
                completed_at: session.completed_at,
            }))
        }
        Ok(None) => Ok(Json(InstallStatusResponse {
            status: "not_started".to_string(),
            initialized: true,
            completed: false,
            session_id: None,
            tenant_id: None,
            lock_owner: None,
            lock_expires_at: None,
            completed_at: None,
        })),
        Err(error) if installer_schema_missing(&error) => Ok(Json(InstallStatusResponse {
            status: "not_initialized".to_string(),
            initialized: false,
            completed: false,
            session_id: None,
            tenant_id: None,
            lock_owner: None,
            lock_expires_at: None,
            completed_at: None,
        })),
        Err(error) => Err(internal_error(format!(
            "failed to read installer status: {error}"
        ))),
    }
}

async fn plan(
    headers: HeaderMap,
    Json(plan): Json<InstallPlan>,
) -> Result<Json<InstallPlanResponse>> {
    require_setup_token(&headers, plan.environment.is_production())?;
    Ok(Json(InstallPlanResponse {
        redacted_plan: redact_install_plan(&plan),
    }))
}

async fn preflight(
    headers: HeaderMap,
    Json(plan): Json<InstallPlan>,
) -> Result<Json<InstallPreflightResponse>> {
    require_setup_token(&headers, plan.environment.is_production())?;
    let report = evaluate_preflight(&plan);
    Ok(Json(InstallPreflightResponse {
        passed: report.passed(),
        report,
        redacted_plan: redact_install_plan(&plan),
    }))
}

async fn apply(
    headers: HeaderMap,
    Json(request): Json<InstallApplyRequest>,
) -> Result<(StatusCode, Json<InstallApplyJobResponse>)> {
    require_setup_token(&headers, request.plan.environment.is_production())?;
    let job_id = rustok_core::generate_id();
    let submitted_at = Utc::now();
    let apply_options = InstallerApplyOptions {
        lock_owner: request
            .lock_owner
            .filter(|value| !value.trim().is_empty())
            .unwrap_or_else(|| "http".to_string()),
        lock_ttl_secs: request.lock_ttl_secs.unwrap_or(900),
        pg_admin_url: request.pg_admin_url,
    };
    INSTALL_JOBS.lock().await.insert(
        job_id,
        InstallJobStatusResponse {
            job_id,
            status: InstallJobState::Running,
            submitted_at,
            started_at: Some(submitted_at),
            finished_at: None,
            session_id: None,
            tenant_id: None,
            output: None,
            error: None,
        },
    );

    tokio::spawn(async move {
        let result = apply_plan(request.plan, apply_options).await;
        let finished_at = Utc::now();
        let mut jobs = INSTALL_JOBS.lock().await;
        let Some(job) = jobs.get_mut(&job_id) else {
            return;
        };
        match result {
            Ok(output) => {
                job.status = InstallJobState::Succeeded;
                job.session_id = Some(output.session_id);
                job.tenant_id = output.tenant_id;
                job.output = Some(output);
                job.error = None;
            }
            Err(error) => {
                job.status = InstallJobState::Failed;
                job.error = Some(error.to_string());
            }
        }
        job.finished_at = Some(finished_at);
    });

    Ok((
        StatusCode::ACCEPTED,
        Json(InstallApplyJobResponse {
            job_id,
            status: InstallJobState::Running,
            submitted_at,
            status_url: format!("/api/install/jobs/{job_id}"),
        }),
    ))
}

async fn job_status(
    headers: HeaderMap,
    Path(job_id): Path<Uuid>,
) -> Result<Json<InstallJobStatusResponse>> {
    require_setup_token(&headers, false)?;
    INSTALL_JOBS
        .lock()
        .await
        .get(&job_id)
        .cloned()
        .map(Json)
        .ok_or_else(|| not_found_error(format!("installer job {job_id} not found")))
}

async fn receipts(
    headers: HeaderMap,
    State(ctx): State<AppContext>,
    Path(session_id): Path<Uuid>,
) -> Result<Json<InstallReceiptsResponse>> {
    require_setup_token(&headers, false)?;
    let persistence = InstallerPersistenceService::new(ctx.db.clone());
    let receipts = persistence
        .list_receipts(session_id)
        .await
        .map_err(|error| internal_error(format!("failed to read installer receipts: {error}")))?;

    Ok(Json(InstallReceiptsResponse {
        session_id,
        receipts,
    }))
}

fn require_setup_token(headers: &HeaderMap, production: bool) -> Result<()> {
    let expected = std::env::var("RUSTOK_INSTALL_SETUP_TOKEN")
        .ok()
        .filter(|value| !value.trim().is_empty());
    let Some(expected) = expected else {
        if production {
            return Err(forbidden_error(
                "production installer HTTP requests require RUSTOK_INSTALL_SETUP_TOKEN",
            ));
        }
        return Ok(());
    };

    let provided = headers
        .get("x-rustok-setup-token")
        .and_then(|value| value.to_str().ok())
        .or_else(|| {
            headers
                .get(axum::http::header::AUTHORIZATION)
                .and_then(|value| value.to_str().ok())
                .and_then(|value| value.strip_prefix("Bearer "))
        });

    if provided.is_some_and(|value| value == expected) {
        Ok(())
    } else {
        Err(forbidden_error("invalid installer setup token"))
    }
}

fn installer_schema_missing(error: &sea_orm::DbErr) -> bool {
    let message = error.to_string();
    message.contains("install_sessions")
        && (message.contains("does not exist")
            || message.contains("no such table")
            || message.contains("not found"))
}

fn forbidden_error(description: impl Into<String>) -> Error {
    Error::CustomError(
        StatusCode::FORBIDDEN,
        ErrorDetail::new("forbidden", description.into().as_str()),
    )
}

fn not_found_error(description: impl Into<String>) -> Error {
    Error::CustomError(
        StatusCode::NOT_FOUND,
        ErrorDetail::new("not_found", description.into().as_str()),
    )
}

fn internal_error(description: impl Into<String>) -> Error {
    Error::CustomError(
        StatusCode::INTERNAL_SERVER_ERROR,
        ErrorDetail::new("installer_error", description.into().as_str()),
    )
}

pub fn routes() -> Routes {
    Routes::new()
        .prefix("api/install")
        .add("/status", get(status))
        .add("/jobs/{job_id}", get(job_status))
        .add("/sessions/{session_id}/receipts", get(receipts))
        .add("/plan", post(plan))
        .add("/preflight", post(preflight))
        .add("/apply", post(apply))
}
