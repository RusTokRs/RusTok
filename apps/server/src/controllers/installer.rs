use axum::{
    Json,
    extract::{Path, State},
    http::{HeaderMap, StatusCode},
    routing::{get, post},
};
use chrono::{DateTime, Utc};
use once_cell::sync::Lazy;
use rustok_installer::{
    InstallApplyOptions, InstallApplyOutput, InstallComposition, InstallDistributionBinding,
    InstallExecutor, InstallPlan, bind_instance_placement, evaluate_preflight_with_deployment,
    load_base_distribution_receipt, redact_install_plan,
};
use rustok_installer_persistence::{InstallerPersistenceService, entities::install_step_receipt};
use rustok_web::HttpError;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use tokio::sync::Mutex;
use uuid::Uuid;

use crate::error::{Error, Result, http_error};
use crate::installer_execution::ServerInstallExecutor;
use crate::services::server_runtime_context::ServerRuntimeContext;

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
    pub output: Option<InstallApplyOutput>,
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

async fn status(State(ctx): State<ServerRuntimeContext>) -> Result<Json<InstallStatusResponse>> {
    let persistence = InstallerPersistenceService::new(ctx.db_clone());
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
    State(ctx): State<ServerRuntimeContext>,
    Json(plan): Json<InstallPlan>,
) -> Result<Json<InstallPlanResponse>> {
    require_setup_token(&headers, plan.environment.is_production())?;
    let plan = bind_host_install_plan(&ctx, plan).await?;
    Ok(Json(InstallPlanResponse {
        redacted_plan: redact_install_plan(&plan),
    }))
}

async fn preflight(
    headers: HeaderMap,
    State(ctx): State<ServerRuntimeContext>,
    Json(plan): Json<InstallPlan>,
) -> Result<Json<InstallPreflightResponse>> {
    require_setup_token(&headers, plan.environment.is_production())?;
    let plan = bind_host_install_plan(&ctx, plan).await?;
    let report = evaluate_preflight_with_deployment(&plan, false);
    Ok(Json(InstallPreflightResponse {
        passed: report.passed(),
        report,
        redacted_plan: redact_install_plan(&plan),
    }))
}

async fn apply(
    headers: HeaderMap,
    State(ctx): State<ServerRuntimeContext>,
    Json(request): Json<InstallApplyRequest>,
) -> Result<(StatusCode, Json<InstallApplyJobResponse>)> {
    require_setup_token(&headers, request.plan.environment.is_production())?;
    let plan = bind_host_install_plan(&ctx, request.plan).await?;
    let job_id = rustok_core::generate_id();
    let submitted_at = Utc::now();
    let apply_options = InstallApplyOptions {
        lock_owner: request
            .lock_owner
            .filter(|value| !value.trim().is_empty())
            .unwrap_or_else(|| "http".to_string()),
        lock_ttl_secs: request.lock_ttl_secs.unwrap_or(900),
        pg_admin_url: request.pg_admin_url,
        bootstrap_public_key_base64: configured_value(
            "RUSTOK_INSTALL_BASE_DISTRIBUTION_PUBLIC_KEY",
        ),
    };
    let registry = ctx
        .shared_get::<rustok_core::ModuleRegistry>()
        .ok_or_else(|| {
            Error::Message(
                "static module registry is unavailable before installer execution".to_string(),
            )
        })?;
    let executor = ServerInstallExecutor::new(registry);
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
        let result = executor.apply(plan, apply_options).await;
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

async fn bind_host_install_plan(
    ctx: &ServerRuntimeContext,
    mut plan: InstallPlan,
) -> Result<InstallPlan> {
    let composition = rustok_distribution::composition_identity();
    let host_composition = InstallComposition {
        revision: composition.revision,
        hash: composition.hash,
    };
    plan.topology = plan.topology.bind_composition(
        host_composition.revision.clone(),
        host_composition.hash.clone(),
    );
    // Bundle identity is release-owner authority, never wizard input.
    plan.topology.distribution = None;
    plan.topology.distribution =
        Some(resolve_host_distribution_binding(ctx, &host_composition).await?);
    let configured_root = std::env::var("RUSTOK_INSTANCE_ROOT")
        .ok()
        .filter(|value| !value.trim().is_empty());
    if plan.environment.is_production() && configured_root.is_none() {
        return Err(bad_request_error(
            "production installer HTTP requests require a host-selected RUSTOK_INSTANCE_ROOT",
        ));
    }
    let requested_root = configured_root.unwrap_or_else(|| plan.placement.root.clone());
    let invocation_dir = std::env::current_dir().map_err(|error| {
        internal_error(format!(
            "failed to resolve server invocation directory: {error}"
        ))
    })?;
    plan.placement = bind_instance_placement(requested_root, invocation_dir)
        .map_err(|error| bad_request_error(error.to_string()))?;
    Ok(plan)
}

async fn resolve_host_distribution_binding(
    ctx: &ServerRuntimeContext,
    host_composition: &InstallComposition,
) -> Result<InstallDistributionBinding> {
    let release_id = configured_value("RUSTOK_INSTALL_DISTRIBUTION_RELEASE_ID");
    let receipt_path = configured_value("RUSTOK_INSTALL_BASE_DISTRIBUTION_RECEIPT");
    let receipt_public_key = configured_value("RUSTOK_INSTALL_BASE_DISTRIBUTION_PUBLIC_KEY");

    match (release_id, receipt_path, receipt_public_key) {
        (Some(_), Some(_), _) | (Some(_), _, Some(_)) => Err(bad_request_error(
            "configure either an owner-ledger distribution release or a signed base-distribution receipt, not both",
        )),
        (Some(release_id), None, None) => {
            let release_id = release_id.parse::<Uuid>().map_err(|_| {
                bad_request_error("RUSTOK_INSTALL_DISTRIBUTION_RELEASE_ID must be a canonical UUID")
            })?;
            let binding = rustok_modules::resolve_static_distribution_install_binding(
                ctx.db(),
                release_id,
            )
            .await
            .map_err(|error| match error {
                rustok_modules::ModuleStaticDistributionReleaseError::InvalidCommand
                | rustok_modules::ModuleStaticDistributionReleaseError::ReleaseNotFound
                | rustok_modules::ModuleStaticDistributionReleaseError::InstallReleaseNotCurrent => {
                    bad_request_error(
                        "host-selected distribution release is not the current admitted release",
                    )
                }
                _ => internal_error("failed to resolve the host-selected distribution release"),
            })?;
            Ok(InstallDistributionBinding {
                preparation_id: binding.preparation_id,
                distribution_release_id: binding.distribution_release_id,
                bundle_reference: binding.bundle_reference,
                bundle_root_digest: binding.bundle_root_digest,
                role_set_digest: binding.role_set_digest,
                roles: binding.roles,
                bootstrap_receipt: None,
            })
        }
        (None, Some(receipt_path), Some(receipt_public_key)) => {
            let receipt =
                load_base_distribution_receipt(receipt_path, &receipt_public_key, Utc::now())
                    .map_err(|_| {
                        bad_request_error(
                            "configured base-distribution receipt could not be verified",
                        )
                    })?;
            if receipt.payload().host_composition_revision != host_composition.revision
                || receipt.payload().host_composition_hash != host_composition.hash
            {
                return Err(bad_request_error(
                    "signed base-distribution receipt is not compatible with this installer host",
                ));
            }
            receipt.into_binding().map_err(|_| {
                bad_request_error(
                    "configured base-distribution receipt could not be bound to the install plan",
                )
            })
        }
        (None, None, None) => Err(bad_request_error(
            "installation requires an owner-ledger release or a signed base-distribution receipt",
        )),
        (None, _, _) => Err(bad_request_error(
            "signed base-distribution installation requires both RUSTOK_INSTALL_BASE_DISTRIBUTION_RECEIPT and RUSTOK_INSTALL_BASE_DISTRIBUTION_PUBLIC_KEY",
        )),
    }
}

fn configured_value(name: &str) -> Option<String> {
    std::env::var(name)
        .ok()
        .filter(|value| !value.trim().is_empty())
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
    State(ctx): State<ServerRuntimeContext>,
    Path(session_id): Path<Uuid>,
) -> Result<Json<InstallReceiptsResponse>> {
    require_setup_token(&headers, false)?;
    let persistence = InstallerPersistenceService::new(ctx.db_clone());
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
    http_error(HttpError::forbidden("forbidden", description))
}

fn bad_request_error(description: impl Into<String>) -> Error {
    http_error(HttpError::new(
        StatusCode::BAD_REQUEST,
        "invalid_install_plan",
        description,
    ))
}

fn not_found_error(description: impl Into<String>) -> Error {
    http_error(HttpError::not_found("not_found", description))
}

fn internal_error(description: impl Into<String>) -> Error {
    http_error(HttpError::new(
        StatusCode::INTERNAL_SERVER_ERROR,
        "installer_error",
        description,
    ))
}

pub fn router() -> crate::routes::ServerRouter {
    axum::Router::new()
        .route("/api/install/status", get(status))
        .route("/api/install/jobs/{job_id}", get(job_status))
        .route("/api/install/sessions/{session_id}/receipts", get(receipts))
        .route("/api/install/plan", post(plan))
        .route("/api/install/preflight", post(preflight))
        .route("/api/install/apply", post(apply))
}
