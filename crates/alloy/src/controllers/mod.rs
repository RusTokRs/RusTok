use std::collections::HashMap;

use axum::{
    extract::{Path, Query, State},
    http::StatusCode,
    routing::{get, post},
    Extension, Json,
};
use chrono::Utc;
use rustok_api::{
    has_any_effective_permission, Action, AuthContextExtension, HostRuntimeContext, Permission,
    Resource, TenantContext,
};
use rustok_web::{HttpError, HttpResult};
use uuid::Uuid;

use crate::{
    api::{
        CreateScriptRequest, EntityInput, ExecutionLogResponse, ListExecutionLogQuery,
        ListExecutionLogResponse, ListScriptsQuery, ListScriptsResponse, ReviewDecisionResponse,
        ReviewScriptRequest, RunScriptRequest, RunScriptResponse, RunWorkspaceTestRequest,
        ScriptResponse, ScriptRevisionRequest, StageReleaseRequest, StageReleaseResponse,
        TestRunResponse, UpdateScriptRequest,
    },
    model::{EntityProxy, ReviewCommand, Script, ScriptStatus, ScriptTrigger},
    runner::ExecutionOutcome,
    storage::ScriptRegistry,
    utils::{dynamic_to_json, json_to_dynamic, validate_cron_expression},
    AlloyReleaseGovernanceHandle, RevisionedReleaseStager, RevisionedTestRunner,
    ScopedAlloyRuntime, ScriptError, SharedAlloyRuntime, TestCommand,
};

pub use crate::api::AXUM_EXECUTION_HISTORY_ROUTES as EXECUTION_HISTORY_ROUTES;

#[derive(Clone)]
pub struct AlloyHttpRuntime {
    runtime: SharedAlloyRuntime,
    release_governance: AlloyReleaseGovernanceHandle,
}

impl AlloyHttpRuntime {
    fn scoped(&self, tenant_id: Uuid) -> HttpResult<ScopedAlloyRuntime> {
        Ok(self.runtime.0.scoped(tenant_id))
    }
}

impl AlloyHttpRuntime {
    fn from_host(runtime: &HostRuntimeContext) -> anyhow::Result<Self> {
        let shared_runtime = runtime.shared_get::<SharedAlloyRuntime>().ok_or_else(|| {
            anyhow::anyhow!("Alloy HTTP routes require SharedAlloyRuntime in HostRuntimeContext")
        })?;
        let release_governance = runtime
            .shared_get::<AlloyReleaseGovernanceHandle>()
            .ok_or_else(|| {
                anyhow::anyhow!(
                    "Alloy HTTP routes require AlloyReleaseGovernanceHandle in HostRuntimeContext"
                )
            })?;
        Ok(Self {
            runtime: shared_runtime,
            release_governance,
        })
    }
}

fn script_error(error: ScriptError) -> HttpError {
    match error {
        ScriptError::NotFound { .. } => {
            HttpError::not_found("alloy_script_not_found", "Script not found")
        }
        ScriptError::RevisionConflict { expected } => HttpError::new(
            StatusCode::CONFLICT,
            "alloy_script_revision_conflict",
            format!("Script revision conflict: expected version {expected}"),
        ),
        ScriptError::Compilation(message)
        | ScriptError::InvalidTrigger(message)
        | ScriptError::InvalidStatus(message)
        | ScriptError::InvalidWorkspace(message) => {
            HttpError::bad_request("invalid_alloy_script", message)
        }
        ScriptError::Review(crate::ReviewError::IdempotencyConflict) => HttpError::new(
            StatusCode::CONFLICT,
            "alloy_review_idempotency_conflict",
            "Review idempotency key was reused for a different command",
        ),
        review_error @ ScriptError::Review(
            crate::ReviewError::InvalidCommand | crate::ReviewError::InvalidTransition { .. },
        ) => HttpError::bad_request("invalid_alloy_review", review_error.to_string()),
        ScriptError::TestRun(crate::TestRunError::IdempotencyConflict) => HttpError::new(
            StatusCode::CONFLICT,
            "alloy_test_idempotency_conflict",
            "Test idempotency key was reused for a different command",
        ),
        ScriptError::TestRun(crate::TestRunError::LeaseLost) => HttpError::new(
            StatusCode::CONFLICT,
            "alloy_test_lease_lost",
            "Test execution lease was lost; retry with the same idempotency key",
        ),
        test_error @ ScriptError::TestRun(
            crate::TestRunError::InvalidCommand | crate::TestRunError::InvalidCompletion,
        ) => HttpError::bad_request("invalid_alloy_test", test_error.to_string()),
        other => HttpError::internal(other.to_string()),
    }
}

fn release_error(error: crate::AlloyReleaseError) -> HttpError {
    match error {
        crate::AlloyReleaseError::StaleRevision { expected } => HttpError::new(
            StatusCode::CONFLICT,
            "alloy_release_revision_conflict",
            format!("Alloy release revision conflict: expected version {expected}"),
        ),
        crate::AlloyReleaseError::ReviewNotApproved => HttpError::new(
            StatusCode::CONFLICT,
            "alloy_release_review_conflict",
            "The current Alloy revision does not have an approved review",
        ),
        crate::AlloyReleaseError::ArtifactSourceDigestMismatch => HttpError::new(
            StatusCode::CONFLICT,
            "alloy_release_artifact_conflict",
            "The artifact digest does not match the reviewed source workspace",
        ),
        crate::AlloyReleaseError::GovernanceConflict(message) => HttpError::new(
            StatusCode::CONFLICT,
            "alloy_release_governance_conflict",
            message,
        ),
        crate::AlloyReleaseError::GovernanceNotFound(message) => {
            HttpError::new(StatusCode::NOT_FOUND, "alloy_release_not_found", message)
        }
        other => HttpError::bad_request("invalid_alloy_release", other.to_string()),
    }
}

fn review_actor(
    auth: Option<Extension<AuthContextExtension>>,
    tenant: &TenantContext,
) -> HttpResult<String> {
    let auth = auth
        .map(|Extension(auth)| auth)
        .ok_or_else(|| HttpError::unauthorized("unauthenticated", "Authentication is required"))?;
    if auth.0.tenant_id != tenant.id {
        return Err(HttpError::forbidden(
            "forbidden",
            "Review tenant context does not match the authenticated principal",
        ));
    }
    let required = Permission::new(Resource::Scripts, Action::Manage);
    if !has_any_effective_permission(&auth.0.permissions, &[required]) {
        return Err(HttpError::forbidden(
            "forbidden",
            "Script review requires scripts.manage permission",
        ));
    }
    Ok(auth.0.user_id.to_string())
}

fn test_actor(
    auth: Option<Extension<AuthContextExtension>>,
    tenant: &TenantContext,
) -> HttpResult<String> {
    let auth = auth
        .map(|Extension(auth)| auth)
        .ok_or_else(|| HttpError::unauthorized("unauthenticated", "Authentication is required"))?;
    if auth.0.tenant_id != tenant.id {
        return Err(HttpError::forbidden(
            "forbidden",
            "Test tenant context does not match the authenticated principal",
        ));
    }
    let required = Permission::new(Resource::Scripts, Action::Manage);
    if !has_any_effective_permission(&auth.0.permissions, &[required]) {
        return Err(HttpError::forbidden(
            "forbidden",
            "Script test requires scripts.manage permission",
        ));
    }
    Ok(auth.0.user_id.to_string())
}

fn release_actor(
    auth: Option<Extension<AuthContextExtension>>,
    tenant: &TenantContext,
) -> HttpResult<String> {
    let auth = auth
        .map(|Extension(auth)| auth)
        .ok_or_else(|| HttpError::unauthorized("unauthenticated", "Authentication is required"))?;
    if auth.0.tenant_id != tenant.id {
        return Err(HttpError::forbidden(
            "forbidden",
            "Release tenant context does not match the authenticated principal",
        ));
    }
    let scripts_manage = Permission::new(Resource::Scripts, Action::Manage);
    let modules_manage = Permission::new(Resource::Modules, Action::Manage);
    if !has_any_effective_permission(&auth.0.permissions, &[scripts_manage])
        || !has_any_effective_permission(&auth.0.permissions, &[modules_manage])
    {
        return Err(HttpError::forbidden(
            "forbidden",
            "Alloy release staging requires scripts.manage and modules.manage permissions",
        ));
    }
    Ok(auth.0.user_id.to_string())
}

fn entity_to_proxy(entity: EntityInput) -> EntityProxy {
    let data = entity
        .data
        .into_iter()
        .map(|(key, value)| (key, json_to_dynamic(value)))
        .collect();

    EntityProxy::new(entity.id, entity.entity_type, data)
}

fn validate_trigger(trigger: &ScriptTrigger) -> HttpResult<()> {
    if let ScriptTrigger::Cron { expression } = trigger {
        validate_cron_expression(expression).map_err(|error| {
            HttpError::bad_request(
                "invalid_alloy_script",
                format!("Invalid cron expression: {error}"),
            )
        })?;
    }
    Ok(())
}

pub async fn list_scripts(
    State(runtime): State<AlloyHttpRuntime>,
    tenant: TenantContext,
    Query(query): Query<ListScriptsQuery>,
) -> HttpResult<Json<ListScriptsResponse>> {
    let runtime = runtime.scoped(tenant.id)?;
    let script_query = match query
        .status_filter()
        .map_err(|error| HttpError::bad_request("invalid_alloy_script_status", error))?
    {
        Some(status) => crate::storage::ScriptQuery::ByStatus(status),
        None => crate::storage::ScriptQuery::All,
    };

    let page = runtime
        .storage
        .find_paginated(script_query, query.offset(), query.limit())
        .await
        .map_err(script_error)?;

    let scripts = page.items.into_iter().map(ScriptResponse::from).collect();

    Ok(Json(ListScriptsResponse::new(
        scripts,
        page.total as usize,
        query.normalized_page(),
        query.normalized_per_page(),
    )))
}

pub async fn get_script(
    State(runtime): State<AlloyHttpRuntime>,
    tenant: TenantContext,
    Path(id): Path<Uuid>,
) -> HttpResult<Json<ScriptResponse>> {
    let runtime = runtime.scoped(tenant.id)?;
    let script = runtime.storage.get(id).await.map_err(script_error)?;
    Ok(Json(script.into()))
}

pub async fn create_script(
    State(runtime): State<AlloyHttpRuntime>,
    tenant: TenantContext,
    Json(req): Json<CreateScriptRequest>,
) -> HttpResult<(StatusCode, Json<ScriptResponse>)> {
    let runtime = runtime.scoped(tenant.id)?;

    if runtime.storage.get_by_name(&req.name).await.is_ok() {
        return Err(HttpError::bad_request(
            "invalid_alloy_request",
            format!("Script with name '{}' already exists", req.name),
        ));
    }
    validate_trigger(&req.trigger)?;
    req.workspace
        .validate_rhai_workspace()
        .map_err(ScriptError::from)
        .map_err(script_error)?;
    let source = req
        .workspace
        .entrypoint_source()
        .map_err(ScriptError::from)
        .map_err(script_error)?;
    let mut scope = rhai::Scope::new();
    runtime
        .engine
        .compile(&req.name, source, &mut scope)
        .map_err(script_error)?;

    let mut script = Script::new(req.name, req.workspace, req.trigger);
    script.tenant_id = req.tenant_id.unwrap_or(tenant.id);
    script.description = req.description;
    script.permissions = req.permissions;
    script.run_as_system = req.run_as_system;

    let saved = runtime.storage.save(script).await.map_err(script_error)?;
    Ok((StatusCode::CREATED, Json(saved.into())))
}

pub async fn update_script(
    State(runtime): State<AlloyHttpRuntime>,
    tenant: TenantContext,
    Path(id): Path<Uuid>,
    Json(req): Json<UpdateScriptRequest>,
) -> HttpResult<Json<ScriptResponse>> {
    let runtime = runtime.scoped(tenant.id)?;
    let mut script = runtime.storage.get(id).await.map_err(script_error)?;
    if script.version != req.expected_version {
        return Err(script_error(ScriptError::RevisionConflict {
            expected: req.expected_version,
        }));
    }

    if let Some(name) = req.name {
        runtime.engine.invalidate(&script.name);
        script.name = name;
    }
    if let Some(description) = req.description {
        script.description = Some(description);
    }
    if let Some(workspace) = req.workspace {
        runtime.engine.invalidate(&script.name);
        workspace
            .validate_rhai_workspace()
            .map_err(ScriptError::from)
            .map_err(script_error)?;
        let source = workspace
            .entrypoint_source()
            .map_err(ScriptError::from)
            .map_err(script_error)?;
        let mut scope = rhai::Scope::new();
        runtime
            .engine
            .compile(&script.name, source, &mut scope)
            .map_err(script_error)?;
        script.workspace = workspace;
    }
    if let Some(ref trigger) = req.trigger {
        validate_trigger(trigger)?;
    }
    if let Some(trigger) = req.trigger {
        script.trigger = trigger;
    }
    if let Some(status) = req.status {
        script.status = status;
    }
    if let Some(permissions) = req.permissions {
        script.permissions = permissions;
    }

    let saved = runtime.storage.save(script).await.map_err(script_error)?;
    Ok(Json(saved.into()))
}

pub async fn delete_script(
    State(runtime): State<AlloyHttpRuntime>,
    tenant: TenantContext,
    Path(id): Path<Uuid>,
    Json(request): Json<ScriptRevisionRequest>,
) -> HttpResult<StatusCode> {
    let runtime = runtime.scoped(tenant.id)?;
    let script = runtime.storage.get(id).await.map_err(script_error)?;
    if script.version != request.expected_version {
        return Err(script_error(crate::ScriptError::RevisionConflict {
            expected: request.expected_version,
        }));
    }
    runtime
        .storage
        .delete(id, request.expected_version)
        .await
        .map_err(script_error)?;
    runtime.engine.invalidate(&script.name);
    Ok(StatusCode::NO_CONTENT)
}

pub async fn run_script(
    State(runtime): State<AlloyHttpRuntime>,
    tenant: TenantContext,
    Path(id): Path<Uuid>,
    Json(req): Json<RunScriptRequest>,
) -> HttpResult<Json<RunScriptResponse>> {
    let runtime = runtime.scoped(tenant.id)?;
    let script = runtime.storage.get(id).await.map_err(script_error)?;
    if script.version != req.expected_version {
        return Err(script_error(ScriptError::RevisionConflict {
            expected: req.expected_version,
        }));
    }

    let params = req
        .params
        .into_iter()
        .map(|(key, value)| (key, json_to_dynamic(value)))
        .collect::<HashMap<_, _>>();
    let entity = req.entity.map(entity_to_proxy);

    let result = runtime
        .orchestrator
        .run_manual_snapshot(&script, params, entity, None)
        .await;

    Ok(Json(run_response(result)))
}

pub async fn run_script_by_name(
    State(runtime): State<AlloyHttpRuntime>,
    tenant: TenantContext,
    Path(name): Path<String>,
    Json(req): Json<RunScriptRequest>,
) -> HttpResult<Json<RunScriptResponse>> {
    let runtime = runtime.scoped(tenant.id)?;
    let script = runtime
        .storage
        .get_by_name(&name)
        .await
        .map_err(script_error)?;
    if script.version != req.expected_version {
        return Err(script_error(ScriptError::RevisionConflict {
            expected: req.expected_version,
        }));
    }

    let params = req
        .params
        .into_iter()
        .map(|(key, value)| (key, json_to_dynamic(value)))
        .collect::<HashMap<_, _>>();
    let entity = req.entity.map(entity_to_proxy);

    let result = runtime
        .orchestrator
        .run_manual_snapshot(&script, params, entity, None)
        .await;

    Ok(Json(run_response(result)))
}

pub async fn list_recent_executions(
    State(runtime): State<AlloyHttpRuntime>,
    tenant: TenantContext,
    Query(query): Query<ListExecutionLogQuery>,
) -> HttpResult<Json<ListExecutionLogResponse>> {
    let runtime = runtime.scoped(tenant.id)?;
    let offset = query.offset();
    let limit = query.limit();
    let executions = runtime
        .execution_log
        .list_recent_for_tenant_paginated(tenant.id, offset, limit)
        .await
        .map_err(script_error)?;
    let total = runtime
        .execution_log
        .count_recent_for_tenant(tenant.id)
        .await
        .map_err(script_error)? as usize;
    let executions = executions
        .into_iter()
        .map(ExecutionLogResponse::from)
        .collect();

    Ok(Json(ListExecutionLogResponse::new(
        executions,
        total,
        query.normalized_page(),
        query.normalized_per_page(),
    )))
}

pub async fn list_script_executions(
    State(runtime): State<AlloyHttpRuntime>,
    tenant: TenantContext,
    Path(id): Path<Uuid>,
    Query(query): Query<ListExecutionLogQuery>,
) -> HttpResult<Json<ListExecutionLogResponse>> {
    let runtime = runtime.scoped(tenant.id)?;
    let offset = query.offset();
    let limit = query.limit();
    let executions = runtime
        .execution_log
        .list_for_script_for_tenant_paginated(id, tenant.id, offset, limit)
        .await
        .map_err(script_error)?;
    let total = runtime
        .execution_log
        .count_for_script_for_tenant(id, tenant.id)
        .await
        .map_err(script_error)? as usize;
    let executions = executions
        .into_iter()
        .map(ExecutionLogResponse::from)
        .collect();

    Ok(Json(ListExecutionLogResponse::new(
        executions,
        total,
        query.normalized_page(),
        query.normalized_per_page(),
    )))
}

pub async fn validate_script(
    State(runtime): State<AlloyHttpRuntime>,
    tenant: TenantContext,
    Json(req): Json<CreateScriptRequest>,
) -> HttpResult<Json<serde_json::Value>> {
    let runtime = runtime.scoped(tenant.id)?;
    req.workspace
        .validate_rhai_workspace()
        .map_err(ScriptError::from)
        .map_err(script_error)?;
    let mut scope = rhai::Scope::new();

    match runtime.engine.compile(
        "__validation__",
        req.workspace
            .entrypoint_source()
            .map_err(ScriptError::from)
            .map_err(script_error)?,
        &mut scope,
    ) {
        Ok(_) => Ok(Json(serde_json::json!({
            "valid": true,
            "message": "Script compiles successfully",
        }))),
        Err(error) => Ok(Json(serde_json::json!({
            "valid": false,
            "message": error.to_string(),
        }))),
    }
}

pub async fn review_script(
    State(runtime): State<AlloyHttpRuntime>,
    tenant: TenantContext,
    auth: Option<Extension<AuthContextExtension>>,
    Path(id): Path<Uuid>,
    Json(request): Json<ReviewScriptRequest>,
) -> HttpResult<Json<ReviewDecisionResponse>> {
    let actor_id = review_actor(auth, &tenant)?;
    let runtime = runtime.scoped(tenant.id)?;
    let decision = runtime
        .storage
        .review(ReviewCommand {
            script_id: id,
            expected_revision: request.expected_version,
            status: request.status,
            policy_revision: request.policy_revision,
            actor_id,
            reason: request.reason,
            idempotency_key: request.idempotency_key,
        })
        .await
        .map_err(script_error)?;
    Ok(Json(decision.into()))
}

pub async fn list_reviews(
    State(runtime): State<AlloyHttpRuntime>,
    tenant: TenantContext,
    auth: Option<Extension<AuthContextExtension>>,
    Path((id, revision)): Path<(Uuid, u32)>,
) -> HttpResult<Json<Vec<ReviewDecisionResponse>>> {
    review_actor(auth, &tenant)?;
    let runtime = runtime.scoped(tenant.id)?;
    let decisions = runtime
        .storage
        .list_reviews(id, revision)
        .await
        .map_err(script_error)?
        .into_iter()
        .map(ReviewDecisionResponse::from)
        .collect();
    Ok(Json(decisions))
}

pub async fn run_workspace_test(
    State(runtime): State<AlloyHttpRuntime>,
    tenant: TenantContext,
    auth: Option<Extension<AuthContextExtension>>,
    Path(id): Path<Uuid>,
    Json(request): Json<RunWorkspaceTestRequest>,
) -> HttpResult<Json<TestRunResponse>> {
    let actor_id = test_actor(auth, &tenant)?;
    let runtime = runtime.scoped(tenant.id)?;
    let run = RevisionedTestRunner::new(runtime.sandbox.clone(), runtime.storage.clone())
        .execute(TestCommand {
            script_id: id,
            expected_revision: request.expected_version,
            test_path: request.test_path,
            actor_id,
            idempotency_key: request.idempotency_key,
        })
        .await
        .map_err(script_error)?;
    Ok(Json(run.into()))
}

pub async fn stage_release(
    State(runtime): State<AlloyHttpRuntime>,
    tenant: TenantContext,
    auth: Option<Extension<AuthContextExtension>>,
    Path(id): Path<Uuid>,
    Json(request): Json<StageReleaseRequest>,
) -> HttpResult<Json<StageReleaseResponse>> {
    let actor_id = release_actor(auth, &tenant)?;
    let governance = runtime.release_governance.0.clone();
    let runtime = runtime.scoped(tenant.id)?;
    let stager =
        RevisionedReleaseStager::new(runtime.sandbox.clone(), runtime.storage.clone(), governance);
    let result = stager
        .stage(crate::AlloyReleaseStageCommand {
            script_id: id,
            expected_revision: request.expected_version,
            publish_request_id: request.publish_request_id,
            artifact_digest: request.artifact_digest,
            actor_id,
            idempotency_key: request.idempotency_key,
        })
        .await
        .map_err(release_error)?;
    Ok(Json(StageReleaseResponse {
        staging_id: result.staging_id,
        created: result.created,
    }))
}

fn run_response(result: crate::ExecutionResult) -> RunScriptResponse {
    let duration_ms = result.duration_ms();
    let (success, error, changes, return_value) = match result.outcome {
        ExecutionOutcome::Success {
            return_value,
            entity_changes,
        } => (
            true,
            None,
            Some(
                entity_changes
                    .into_iter()
                    .map(|(key, value)| (key, dynamic_to_json(value)))
                    .collect(),
            ),
            return_value
                .map(dynamic_to_json)
                .unwrap_or(serde_json::Value::Null),
        ),
        ExecutionOutcome::Aborted { reason } => {
            (false, Some(reason), None, serde_json::Value::Null)
        }
        ExecutionOutcome::Failed { ref error } => (
            false,
            Some(error.to_string()),
            None,
            serde_json::Value::Null,
        ),
    };

    RunScriptResponse {
        execution_id: result.execution_id.to_string(),
        success,
        duration_ms,
        error,
        changes,
        return_value,
    }
}

pub async fn activate_script(
    State(runtime): State<AlloyHttpRuntime>,
    tenant: TenantContext,
    Path(id): Path<Uuid>,
    Json(request): Json<ScriptRevisionRequest>,
) -> HttpResult<Json<ScriptResponse>> {
    let runtime = runtime.scoped(tenant.id)?;
    let mut script = runtime.storage.get(id).await.map_err(script_error)?;
    if script.version != request.expected_version {
        return Err(script_error(crate::ScriptError::RevisionConflict {
            expected: request.expected_version,
        }));
    }
    script.activate();
    let saved = runtime.storage.save(script).await.map_err(script_error)?;
    Ok(Json(saved.into()))
}

pub async fn pause_script(
    State(runtime): State<AlloyHttpRuntime>,
    tenant: TenantContext,
    Path(id): Path<Uuid>,
    Json(request): Json<ScriptRevisionRequest>,
) -> HttpResult<Json<ScriptResponse>> {
    let runtime = runtime.scoped(tenant.id)?;
    let mut script = runtime.storage.get(id).await.map_err(script_error)?;
    if script.version != request.expected_version {
        return Err(script_error(crate::ScriptError::RevisionConflict {
            expected: request.expected_version,
        }));
    }
    script.status = ScriptStatus::Paused;
    script.updated_at = Utc::now();
    let saved = runtime.storage.save(script).await.map_err(script_error)?;
    Ok(Json(saved.into()))
}

pub fn axum_router(runtime: &HostRuntimeContext) -> anyhow::Result<axum::Router> {
    let state = AlloyHttpRuntime::from_host(runtime)?;
    Ok(axum::Router::new()
        .route("/api/alloy/scripts", get(list_scripts).post(create_script))
        .route("/api/alloy/executions", get(list_recent_executions))
        .route("/api/alloy/scripts/validate", post(validate_script))
        .route(
            "/api/alloy/scripts/{id}",
            get(get_script).put(update_script).delete(delete_script),
        )
        .route("/api/alloy/scripts/{id}/run", post(run_script))
        .route(
            "/api/alloy/scripts/{id}/tests/run",
            post(run_workspace_test),
        )
        .route(
            "/api/alloy/scripts/{id}/releases/stage",
            post(stage_release),
        )
        .route("/api/alloy/scripts/{id}/reviews", post(review_script))
        .route(
            "/api/alloy/scripts/{id}/revisions/{revision}/reviews",
            get(list_reviews),
        )
        .route(
            "/api/alloy/scripts/{id}/executions",
            get(list_script_executions),
        )
        .route(
            "/api/alloy/scripts/name/{name}/run",
            post(run_script_by_name),
        )
        .route("/api/alloy/scripts/{id}/activate", post(activate_script))
        .route("/api/alloy/scripts/{id}/pause", post(pause_script))
        .with_state(state))
}

#[cfg(test)]
mod tests {
    use super::EXECUTION_HISTORY_ROUTES;

    #[test]
    fn execution_history_routes_match_operator_contract() {
        assert_eq!(
            EXECUTION_HISTORY_ROUTES,
            &["/executions", "/scripts/{id}/executions"]
        );
    }
}
