use std::collections::HashMap;

use axum::{
    Extension, Json,
    extract::{Path, Query, State},
    http::StatusCode,
    routing::{get, post},
};
use chrono::Utc;
use rustok_api::{
    Action, AuthContextExtension, HostRuntimeContext, Permission, Resource, TenantContext,
    has_any_effective_permission,
};
use rustok_web::{HttpError, HttpResult};
use uuid::Uuid;

use crate::{
    AlloyImportError, AlloyPublishedReleaseImportCommand, AlloyPublishedRhaiSourceProviderHandle,
    AlloyReleaseGovernanceHandle, AlloyReleaseImporter, RevisionedReleaseStager,
    RevisionedTestRunner, ScopedAlloyRuntime, ScriptError, ScriptEvidenceRetentionCommand,
    SharedAlloyRuntime, TestCommand,
    api::{
        CreateScriptRequest, DeleteScriptRequest, EntityInput, ExecutionLogResponse,
        ImportPublishedReleaseRequest, ImportPublishedReleaseResponse, ListExecutionLogQuery,
        ListExecutionLogResponse, ListScriptsQuery, ListScriptsResponse, ReviewDecisionResponse,
        ReviewScriptRequest, RunScriptRequest, RunScriptResponse, RunWorkspaceTestRequest,
        ScriptResponse, ScriptRevisionRequest, StageReleaseRequest, StageReleaseResponse,
        TestRunResponse, UpdateDeletedEvidenceRetentionRequest, UpdateScriptRequest,
    },
    model::{
        EntityProxy, ReviewCommand, Script, ScriptDeletionCommand, ScriptStatus, ScriptTrigger,
        SourceProvenance,
    },
    runner::ExecutionOutcome,
    storage::ScriptRegistry,
    utils::{dynamic_to_json, json_to_dynamic, validate_cron_expression},
};

pub const EXECUTION_HISTORY_ROUTES: &[&str] = &[
    "/api/alloy/executions",
    "/api/alloy/scripts/{id}/executions",
];

#[derive(Clone)]
pub struct AlloyHttpRuntime {
    runtime: SharedAlloyRuntime,
    release_governance: AlloyReleaseGovernanceHandle,
    published_rhai_source: AlloyPublishedRhaiSourceProviderHandle,
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
        let published_rhai_source = runtime
            .shared_get::<AlloyPublishedRhaiSourceProviderHandle>()
            .ok_or_else(|| {
                anyhow::anyhow!(
                    "Alloy HTTP routes require AlloyPublishedRhaiSourceProviderHandle in HostRuntimeContext"
                )
            })?;
        Ok(Self {
            runtime: shared_runtime,
            release_governance,
            published_rhai_source,
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
        ScriptError::RetentionRevisionConflict { expected } => HttpError::new(
            StatusCode::CONFLICT,
            "alloy_evidence_retention_revision_conflict",
            format!("Evidence retention revision conflict: expected revision {expected}"),
        ),
        ScriptError::ImportIdempotencyConflict => HttpError::new(
            StatusCode::CONFLICT,
            "alloy_import_idempotency_conflict",
            "Alloy import idempotency key was reused for a different release command",
        ),
        ScriptError::ImportDraftNameConflict => HttpError::new(
            StatusCode::CONFLICT,
            "alloy_import_draft_name_conflict",
            "An Alloy draft with the requested tenant-scoped name already exists",
        ),
        ScriptError::Compilation(message)
        | ScriptError::InvalidTrigger(message)
        | ScriptError::InvalidStatus(message)
        | ScriptError::InvalidWorkspace(message)
        | ScriptError::InvalidLineage(message) => {
            HttpError::bad_request("invalid_alloy_script", message)
        }
        ScriptError::Review(crate::ReviewError::IdempotencyConflict) => HttpError::new(
            StatusCode::CONFLICT,
            "alloy_review_idempotency_conflict",
            "Review idempotency key was reused for a different command",
        ),
        ScriptError::EvidenceRetention(
            crate::ScriptEvidenceRetentionError::IdempotencyConflict,
        ) => HttpError::new(
            StatusCode::CONFLICT,
            "alloy_evidence_retention_idempotency_conflict",
            "Evidence retention idempotency key was reused for a different command",
        ),
        retention_error @ ScriptError::EvidenceRetention(
            crate::ScriptEvidenceRetentionError::InvalidCommand
            | crate::ScriptEvidenceRetentionError::InvalidTransition
            | crate::ScriptEvidenceRetentionError::InvalidStoredState
            | crate::ScriptEvidenceRetentionError::RevisionOverflow
            | crate::ScriptEvidenceRetentionError::Serialize(_),
        ) => HttpError::bad_request(
            "invalid_alloy_evidence_retention",
            retention_error.to_string(),
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

fn import_error(error: AlloyImportError) -> HttpError {
    match error {
        AlloyImportError::InvalidCommand
        | AlloyImportError::IneligibleRelease
        | AlloyImportError::InvalidSource => HttpError::bad_request(
            "invalid_alloy_release_import",
            "The published release cannot be imported as an Alloy Rhai workspace",
        ),
        AlloyImportError::SourceUnavailable(_) => HttpError::not_found(
            "alloy_release_import_source_not_found",
            "The canonical published Rhai workspace is unavailable",
        ),
        AlloyImportError::IdempotencyConflict => HttpError::new(
            StatusCode::CONFLICT,
            "alloy_release_import_idempotency_conflict",
            "Alloy import idempotency key was reused for a different release command",
        ),
        AlloyImportError::DraftNameConflict => HttpError::new(
            StatusCode::CONFLICT,
            "alloy_release_import_draft_name_conflict",
            "An Alloy draft with the requested tenant-scoped name already exists",
        ),
        AlloyImportError::Storage(_) => HttpError::internal("Alloy release import failed"),
    }
}

fn scripts_manage_auth(
    auth: Option<Extension<AuthContextExtension>>,
    tenant: &TenantContext,
    operation: &str,
) -> HttpResult<AuthContextExtension> {
    let auth = auth
        .map(|Extension(auth)| auth)
        .ok_or_else(|| HttpError::unauthorized("unauthenticated", "Authentication is required"))?;
    if auth.0.tenant_id != tenant.id {
        return Err(HttpError::forbidden(
            "forbidden",
            format!("{operation} tenant context does not match the authenticated principal"),
        ));
    }
    let required = Permission::new(Resource::Scripts, Action::Manage);
    if !has_any_effective_permission(&auth.0.permissions, &[required]) {
        return Err(HttpError::forbidden(
            "forbidden",
            format!("{operation} requires scripts.manage permission"),
        ));
    }
    Ok(auth)
}

fn scripts_manage_actor(
    auth: Option<Extension<AuthContextExtension>>,
    tenant: &TenantContext,
    operation: &str,
) -> HttpResult<String> {
    Ok(scripts_manage_auth(auth, tenant, operation)?
        .0
        .user_id
        .to_string())
}

fn release_actor(
    auth: Option<Extension<AuthContextExtension>>,
    tenant: &TenantContext,
) -> HttpResult<String> {
    let auth = scripts_manage_auth(auth, tenant, "Alloy release staging")?;
    let modules_manage = Permission::new(Resource::Modules, Action::Manage);
    if !has_any_effective_permission(&auth.0.permissions, &[modules_manage]) {
        return Err(HttpError::forbidden(
            "forbidden",
            "Alloy release staging requires modules.manage permission",
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
    auth: Option<Extension<AuthContextExtension>>,
    Query(query): Query<ListScriptsQuery>,
) -> HttpResult<Json<ListScriptsResponse>> {
    scripts_manage_actor(auth, &tenant, "Alloy script listing")?;
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
    auth: Option<Extension<AuthContextExtension>>,
    Path(id): Path<Uuid>,
) -> HttpResult<Json<ScriptResponse>> {
    scripts_manage_actor(auth, &tenant, "Alloy script lookup")?;
    let runtime = runtime.scoped(tenant.id)?;
    let script = runtime.storage.get(id).await.map_err(script_error)?;
    Ok(Json(script.into()))
}

pub async fn create_script(
    State(runtime): State<AlloyHttpRuntime>,
    tenant: TenantContext,
    auth: Option<Extension<AuthContextExtension>>,
    Json(req): Json<CreateScriptRequest>,
) -> HttpResult<(StatusCode, Json<ScriptResponse>)> {
    let actor_id = scripts_manage_actor(auth, &tenant, "Alloy script creation")?;
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
    script.tenant_id = tenant.id;
    script.description = req.description;
    script.permissions = req.permissions;
    script.run_as_system = req.run_as_system;
    script.author_id = Some(actor_id);
    script.source_provenance = SourceProvenance::http("alloy_create_script");

    let saved = runtime.storage.save(script).await.map_err(script_error)?;
    Ok((StatusCode::CREATED, Json(saved.into())))
}

pub async fn update_script(
    State(runtime): State<AlloyHttpRuntime>,
    tenant: TenantContext,
    auth: Option<Extension<AuthContextExtension>>,
    Path(id): Path<Uuid>,
    Json(req): Json<UpdateScriptRequest>,
) -> HttpResult<Json<ScriptResponse>> {
    let actor_id = scripts_manage_actor(auth, &tenant, "Alloy script update")?;
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
    script.author_id = Some(actor_id);
    script.source_provenance = SourceProvenance::http("alloy_update_script");

    let saved = runtime.storage.save(script).await.map_err(script_error)?;
    Ok(Json(saved.into()))
}

pub async fn delete_script(
    State(runtime): State<AlloyHttpRuntime>,
    tenant: TenantContext,
    auth: Option<Extension<AuthContextExtension>>,
    Path(id): Path<Uuid>,
    Json(request): Json<DeleteScriptRequest>,
) -> HttpResult<StatusCode> {
    let actor_id = scripts_manage_actor(auth, &tenant, "Alloy script deletion")?;
    let runtime = runtime.scoped(tenant.id)?;
    let script = match runtime.storage.get(id).await {
        Ok(script) => {
            if script.version != request.expected_version {
                return Err(script_error(crate::ScriptError::RevisionConflict {
                    expected: request.expected_version,
                }));
            }
            Some(script)
        }
        Err(crate::ScriptError::NotFound { .. }) => None,
        Err(error) => return Err(script_error(error)),
    };
    runtime
        .storage
        .delete(ScriptDeletionCommand {
            script_id: id,
            expected_revision: request.expected_version,
            actor_id,
            reason: request.reason,
            idempotency_key: request.idempotency_key,
        })
        .await
        .map_err(script_error)?;
    if let Some(script) = script {
        runtime.engine.invalidate(&script.name);
    }
    Ok(StatusCode::NO_CONTENT)
}

pub async fn get_deleted_evidence_retention(
    State(runtime): State<AlloyHttpRuntime>,
    tenant: TenantContext,
    auth: Option<Extension<AuthContextExtension>>,
    Path(id): Path<Uuid>,
) -> HttpResult<Json<crate::RedactedAlloyEvidenceRetention>> {
    scripts_manage_actor(auth, &tenant, "Alloy deleted evidence retention lookup")?;
    let runtime = runtime.scoped(tenant.id)?;
    let retention = runtime
        .storage
        .get_deleted_evidence_retention(id)
        .await
        .map_err(script_error)?;
    Ok(Json(retention.into()))
}

pub async fn update_deleted_evidence_retention(
    State(runtime): State<AlloyHttpRuntime>,
    tenant: TenantContext,
    auth: Option<Extension<AuthContextExtension>>,
    Path(id): Path<Uuid>,
    Json(request): Json<UpdateDeletedEvidenceRetentionRequest>,
) -> HttpResult<Json<crate::RedactedAlloyEvidenceRetention>> {
    let actor_id = scripts_manage_actor(auth, &tenant, "Alloy deleted evidence retention update")?;
    let runtime = runtime.scoped(tenant.id)?;
    let retention = runtime
        .storage
        .update_deleted_evidence_retention(ScriptEvidenceRetentionCommand {
            script_id: id,
            deletion_request_digest: request.deletion_request_digest,
            expected_retention_revision: request.expected_retention_revision,
            action: request.action,
            actor_id,
            reason: request.reason,
            idempotency_key: request.idempotency_key,
        })
        .await
        .map_err(script_error)?;
    Ok(Json(retention.into()))
}

pub async fn run_script(
    State(runtime): State<AlloyHttpRuntime>,
    tenant: TenantContext,
    auth: Option<Extension<AuthContextExtension>>,
    Path(id): Path<Uuid>,
    Json(req): Json<RunScriptRequest>,
) -> HttpResult<Json<RunScriptResponse>> {
    let actor_id = scripts_manage_actor(auth, &tenant, "Alloy script execution")?;
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
        .run_manual_snapshot(&script, params, entity, Some(actor_id))
        .await;

    Ok(Json(run_response(result)))
}

pub async fn run_script_by_name(
    State(runtime): State<AlloyHttpRuntime>,
    tenant: TenantContext,
    auth: Option<Extension<AuthContextExtension>>,
    Path(name): Path<String>,
    Json(req): Json<RunScriptRequest>,
) -> HttpResult<Json<RunScriptResponse>> {
    let actor_id = scripts_manage_actor(auth, &tenant, "Alloy script execution")?;
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
        .run_manual_snapshot(&script, params, entity, Some(actor_id))
        .await;

    Ok(Json(run_response(result)))
}

pub async fn list_recent_executions(
    State(runtime): State<AlloyHttpRuntime>,
    tenant: TenantContext,
    auth: Option<Extension<AuthContextExtension>>,
    Query(query): Query<ListExecutionLogQuery>,
) -> HttpResult<Json<ListExecutionLogResponse>> {
    scripts_manage_actor(auth, &tenant, "Alloy execution history lookup")?;
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
    auth: Option<Extension<AuthContextExtension>>,
    Path(id): Path<Uuid>,
    Query(query): Query<ListExecutionLogQuery>,
) -> HttpResult<Json<ListExecutionLogResponse>> {
    scripts_manage_actor(auth, &tenant, "Alloy execution history lookup")?;
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
    auth: Option<Extension<AuthContextExtension>>,
    Json(req): Json<CreateScriptRequest>,
) -> HttpResult<Json<serde_json::Value>> {
    scripts_manage_actor(auth, &tenant, "Alloy script validation")?;
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
    let actor_id = scripts_manage_actor(auth, &tenant, "Alloy script review")?;
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
    scripts_manage_actor(auth, &tenant, "Alloy script review lookup")?;
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
    let actor_id = scripts_manage_actor(auth, &tenant, "Alloy script test")?;
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

pub async fn import_published_release(
    State(runtime): State<AlloyHttpRuntime>,
    tenant: TenantContext,
    auth: Option<Extension<AuthContextExtension>>,
    Json(request): Json<ImportPublishedReleaseRequest>,
) -> HttpResult<Json<ImportPublishedReleaseResponse>> {
    let actor_id = release_actor(auth, &tenant)?;
    let source = runtime.published_rhai_source.0.clone();
    let runtime = runtime.scoped(tenant.id)?;
    let result = AlloyReleaseImporter::new(runtime.storage.clone(), source)
        .import(AlloyPublishedReleaseImportCommand {
            tenant_id: tenant.id,
            release: request.release,
            draft_name: request.draft_name,
            actor_id,
            idempotency_key: request.idempotency_key,
        })
        .await
        .map_err(import_error)?;
    Ok(Json(ImportPublishedReleaseResponse {
        script: result.script.into(),
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
    auth: Option<Extension<AuthContextExtension>>,
    Path(id): Path<Uuid>,
    Json(request): Json<ScriptRevisionRequest>,
) -> HttpResult<Json<ScriptResponse>> {
    let actor_id = scripts_manage_actor(auth, &tenant, "Alloy script activation")?;
    let runtime = runtime.scoped(tenant.id)?;
    let mut script = runtime.storage.get(id).await.map_err(script_error)?;
    if script.version != request.expected_version {
        return Err(script_error(crate::ScriptError::RevisionConflict {
            expected: request.expected_version,
        }));
    }
    script.activate();
    script.author_id = Some(actor_id);
    let saved = runtime.storage.save(script).await.map_err(script_error)?;
    Ok(Json(saved.into()))
}

pub async fn pause_script(
    State(runtime): State<AlloyHttpRuntime>,
    tenant: TenantContext,
    auth: Option<Extension<AuthContextExtension>>,
    Path(id): Path<Uuid>,
    Json(request): Json<ScriptRevisionRequest>,
) -> HttpResult<Json<ScriptResponse>> {
    let actor_id = scripts_manage_actor(auth, &tenant, "Alloy script pause")?;
    let runtime = runtime.scoped(tenant.id)?;
    let mut script = runtime.storage.get(id).await.map_err(script_error)?;
    if script.version != request.expected_version {
        return Err(script_error(crate::ScriptError::RevisionConflict {
            expected: request.expected_version,
        }));
    }
    script.status = ScriptStatus::Paused;
    script.updated_at = Utc::now();
    script.author_id = Some(actor_id);
    let saved = runtime.storage.save(script).await.map_err(script_error)?;
    Ok(Json(saved.into()))
}

pub fn axum_router(runtime: &HostRuntimeContext) -> anyhow::Result<axum::Router> {
    let state = AlloyHttpRuntime::from_host(runtime)?;
    Ok(axum::Router::new()
        .route("/api/alloy/scripts", get(list_scripts).post(create_script))
        .route(EXECUTION_HISTORY_ROUTES[0], get(list_recent_executions))
        .route("/api/alloy/scripts/validate", post(validate_script))
        .route(
            "/api/alloy/scripts/{id}",
            get(get_script).put(update_script).delete(delete_script),
        )
        .route(
            "/api/alloy/deleted-scripts/{id}/retention",
            get(get_deleted_evidence_retention).put(update_deleted_evidence_retention),
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
        .route("/api/alloy/releases/import", post(import_published_release))
        .route("/api/alloy/scripts/{id}/reviews", post(review_script))
        .route(
            "/api/alloy/scripts/{id}/revisions/{revision}/reviews",
            get(list_reviews),
        )
        .route(EXECUTION_HISTORY_ROUTES[1], get(list_script_executions))
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
    use axum::Extension;
    use rustok_api::{AuthContext, AuthContextExtension, TenantContext};
    use uuid::Uuid;

    use super::{EXECUTION_HISTORY_ROUTES, scripts_manage_actor};

    fn tenant(id: Uuid) -> TenantContext {
        TenantContext {
            id,
            name: "test tenant".to_string(),
            slug: "test-tenant".to_string(),
            domain: None,
            settings: serde_json::Value::Null,
            default_locale: "en".to_string(),
            is_active: true,
        }
    }

    fn auth(tenant_id: Uuid, permissions: Vec<rustok_api::Permission>) -> AuthContext {
        AuthContext {
            user_id: Uuid::new_v4(),
            session_id: Uuid::new_v4(),
            tenant_id,
            permissions,
            client_id: None,
            scopes: Vec::new(),
            grant_type: "password".to_string(),
        }
    }

    #[test]
    fn execution_history_routes_match_operator_contract() {
        assert_eq!(
            EXECUTION_HISTORY_ROUTES,
            &[
                "/api/alloy/executions",
                "/api/alloy/scripts/{id}/executions"
            ]
        );
    }

    #[test]
    fn scripts_manage_actor_fails_closed_and_binds_the_authenticated_tenant() {
        let tenant_id = Uuid::new_v4();
        let context = tenant(tenant_id);
        let manage = rustok_api::Permission::SCRIPTS_MANAGE;

        assert_eq!(
            scripts_manage_actor(None, &context, "test operation")
                .expect_err("missing authentication must be rejected")
                .status,
            axum::http::StatusCode::UNAUTHORIZED
        );
        assert_eq!(
            scripts_manage_actor(
                Some(Extension(AuthContextExtension(auth(
                    Uuid::new_v4(),
                    vec![manage.clone()]
                )))),
                &context,
                "test operation",
            )
            .expect_err("a different authenticated tenant must be rejected")
            .status,
            axum::http::StatusCode::FORBIDDEN
        );
        assert_eq!(
            scripts_manage_actor(
                Some(Extension(AuthContextExtension(auth(tenant_id, Vec::new())))),
                &context,
                "test operation",
            )
            .expect_err("a principal without scripts.manage must be rejected")
            .status,
            axum::http::StatusCode::FORBIDDEN
        );

        let actor = auth(tenant_id, vec![manage]);
        assert_eq!(
            scripts_manage_actor(
                Some(Extension(AuthContextExtension(actor.clone()))),
                &context,
                "test operation",
            )
            .expect("a matching scripts.manage principal must be accepted"),
            actor.user_id.to_string(),
        );
    }
}
