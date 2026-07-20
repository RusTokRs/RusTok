use std::collections::HashMap;
use std::sync::Arc;

use axum::{
    extract::{Path, Query, State},
    http::StatusCode,
    response::IntoResponse,
    Json,
};
use tracing::{info, instrument};
use uuid::Uuid;

use crate::error::ScriptError;
use crate::model::{EntityProxy, ScriptTrigger};
use crate::runner::ScriptOrchestrator;
use crate::storage::{ScriptQuery, ScriptRegistry};
use crate::utils::{dynamic_to_json, json_to_dynamic, validate_cron_expression};
use crate::SeaOrmExecutionLog;

use super::dto::*;

pub struct AppState<S: ScriptRegistry> {
    pub registry: Arc<S>,
    pub orchestrator: Arc<ScriptOrchestrator<S>>,
    pub engine: Arc<crate::engine::ScriptEngine>,
    pub execution_log: Arc<SeaOrmExecutionLog>,
}

type ApiResult<T> = Result<T, ApiError>;

impl IntoResponse for ApiError {
    fn into_response(self) -> axum::response::Response {
        let status = match self.code.as_str() {
            "not_found" => StatusCode::NOT_FOUND,
            "validation" => StatusCode::BAD_REQUEST,
            "conflict" => StatusCode::CONFLICT,
            _ => StatusCode::INTERNAL_SERVER_ERROR,
        };
        (status, Json(self)).into_response()
    }
}

impl From<ScriptError> for ApiError {
    fn from(e: ScriptError) -> Self {
        match e {
            ScriptError::NotFound { name } => ApiError {
                error: format!("Script not found: {name}"),
                code: "not_found".to_string(),
            },
            ScriptError::RevisionConflict { expected } => ApiError {
                error: format!("Script revision conflict: expected version {expected}"),
                code: "conflict".to_string(),
            },
            ScriptError::Compilation(msg) => ApiError {
                error: format!("Compilation error: {msg}"),
                code: "validation".to_string(),
            },
            ScriptError::InvalidTrigger(msg) => ApiError {
                error: format!("Invalid trigger: {msg}"),
                code: "validation".to_string(),
            },
            ScriptError::InvalidStatus(msg) => ApiError {
                error: format!("Invalid status: {msg}"),
                code: "validation".to_string(),
            },
            ScriptError::InvalidWorkspace(msg) => ApiError {
                error: format!("Invalid Alloy workspace: {msg}"),
                code: "validation".to_string(),
            },
            _ => ApiError {
                error: e.to_string(),
                code: "internal".to_string(),
            },
        }
    }
}

// ============ CRUD Handlers ============

#[instrument(skip(state))]
pub async fn list_scripts<S: ScriptRegistry>(
    State(state): State<Arc<AppState<S>>>,
    Query(query): Query<ListScriptsQuery>,
) -> ApiResult<Json<ListScriptsResponse>> {
    let script_query = match query.status_filter().map_err(|message| ApiError {
        error: message,
        code: "validation".to_string(),
    })? {
        Some(status) => ScriptQuery::ByStatus(status),
        None => ScriptQuery::All,
    };

    let offset = query.offset();
    let limit = query.limit();

    let page = state
        .registry
        .find_paginated(script_query, offset, limit)
        .await
        .map_err(ApiError::from)?;

    let scripts: Vec<ScriptResponse> = page.items.into_iter().map(Into::into).collect();

    Ok(Json(ListScriptsResponse::new(
        scripts,
        page.total as usize,
        query.normalized_page(),
        query.normalized_per_page(),
    )))
}

#[instrument(skip(state))]
pub async fn get_script<S: ScriptRegistry>(
    State(state): State<Arc<AppState<S>>>,
    Path(id): Path<Uuid>,
) -> ApiResult<Json<ScriptResponse>> {
    let script = state.registry.get(id).await.map_err(ApiError::from)?;
    Ok(Json(script.into()))
}

#[instrument(skip(state))]
pub async fn create_script<S: ScriptRegistry>(
    State(state): State<Arc<AppState<S>>>,
    Json(req): Json<CreateScriptRequest>,
) -> ApiResult<(StatusCode, Json<ScriptResponse>)> {
    let existing = state.registry.get_by_name(&req.name).await;
    if existing.is_ok() {
        return Err(ApiError {
            error: format!("Script with name '{}' already exists", req.name),
            code: "conflict".to_string(),
        });
    }

    validate_trigger(&req.trigger)?;
    req.workspace
        .validate_rhai_workspace()
        .map_err(ScriptError::from)?;
    let source = req
        .workspace
        .entrypoint_source()
        .map_err(ScriptError::from)?;
    let mut scope = rhai::Scope::new();
    state.engine.compile(&req.name, source, &mut scope)?;

    let mut script = crate::model::Script::new(req.name, req.workspace, req.trigger);
    if let Some(tenant_id) = req.tenant_id {
        script.tenant_id = tenant_id;
    }
    script.description = req.description;
    script.permissions = req.permissions;
    script.run_as_system = req.run_as_system;

    let saved = state.registry.save(script).await.map_err(ApiError::from)?;

    info!(script_id = %saved.id, script_name = %saved.name, "Script created");

    Ok((StatusCode::CREATED, Json(saved.into())))
}

#[instrument(skip(state))]
pub async fn update_script<S: ScriptRegistry>(
    State(state): State<Arc<AppState<S>>>,
    Path(id): Path<Uuid>,
    Json(req): Json<UpdateScriptRequest>,
) -> ApiResult<Json<ScriptResponse>> {
    let mut script = state.registry.get(id).await.map_err(ApiError::from)?;
    if script.version != req.expected_version {
        return Err(ApiError::from(ScriptError::RevisionConflict {
            expected: req.expected_version,
        }));
    }

    if let Some(name) = req.name {
        state.engine.invalidate(&script.name);
        script.name = name;
    }
    if let Some(desc) = req.description {
        script.description = Some(desc);
    }
    if let Some(workspace) = req.workspace {
        state.engine.invalidate(&script.name);
        workspace
            .validate_rhai_workspace()
            .map_err(ScriptError::from)?;
        let source = workspace.entrypoint_source().map_err(ScriptError::from)?;
        let mut scope = rhai::Scope::new();
        state.engine.compile(&script.name, source, &mut scope)?;
        script.workspace = workspace;
    }
    if let Some(trigger) = req.trigger {
        validate_trigger(&trigger)?;
        script.trigger = trigger;
    }
    if let Some(status) = req.status {
        script.status = status;
    }
    if let Some(perms) = req.permissions {
        script.permissions = perms;
    }

    let saved = state.registry.save(script).await.map_err(ApiError::from)?;

    info!(script_id = %saved.id, script_name = %saved.name, "Script updated");

    Ok(Json(saved.into()))
}

#[instrument(skip(state))]
pub async fn delete_script<S: ScriptRegistry>(
    State(state): State<Arc<AppState<S>>>,
    Path(id): Path<Uuid>,
    Json(request): Json<ScriptRevisionRequest>,
) -> ApiResult<StatusCode> {
    let script = state.registry.get(id).await.map_err(ApiError::from)?;
    if script.version != request.expected_version {
        return Err(ApiError::from(ScriptError::RevisionConflict {
            expected: request.expected_version,
        }));
    }
    state
        .registry
        .delete(id, request.expected_version)
        .await
        .map_err(ApiError::from)?;
    state.engine.invalidate(&script.name);

    info!(script_id = %id, "Script deleted");

    Ok(StatusCode::NO_CONTENT)
}

// ============ Execution Handlers ============

#[instrument(skip(state), fields(script_id = %id))]
pub async fn run_script<S: ScriptRegistry>(
    State(state): State<Arc<AppState<S>>>,
    Path(id): Path<Uuid>,
    Json(req): Json<RunScriptRequest>,
) -> ApiResult<Json<RunScriptResponse>> {
    let script = state.registry.get(id).await.map_err(ApiError::from)?;
    if script.version != req.expected_version {
        return Err(ApiError::from(ScriptError::RevisionConflict {
            expected: req.expected_version,
        }));
    }

    let params = req
        .params
        .into_iter()
        .map(|(k, v)| (k, json_to_dynamic(v)))
        .collect();

    let entity = req.entity.map(|entity_input| {
        let data = entity_input
            .data
            .into_iter()
            .map(|(k, v)| (k, json_to_dynamic(v)))
            .collect();

        EntityProxy::new(entity_input.id, entity_input.entity_type, data)
    });

    let result = state
        .orchestrator
        .run_manual_snapshot(&script, params, entity, None)
        .await;

    let (success, error, changes, return_value) = match &result.outcome {
        crate::runner::ExecutionOutcome::Success {
            return_value,
            entity_changes,
        } => (
            true,
            None,
            Some(convert_map(entity_changes.clone())),
            return_value
                .clone()
                .map(dynamic_to_json)
                .unwrap_or(serde_json::Value::Null),
        ),
        crate::runner::ExecutionOutcome::Aborted { reason } => (
            false,
            Some(reason.to_string()),
            None,
            serde_json::Value::Null,
        ),
        crate::runner::ExecutionOutcome::Failed { error } => (
            false,
            Some(error.to_string()),
            None,
            serde_json::Value::Null,
        ),
    };

    info!(
        script_id = %id,
        success = success,
        duration_ms = result.duration_ms(),
        "Script executed"
    );

    Ok(Json(RunScriptResponse {
        execution_id: result.execution_id.to_string(),
        success,
        duration_ms: result.duration_ms(),
        error,
        changes,
        return_value,
    }))
}

#[instrument(skip(state), fields(script_name = %name))]
pub async fn run_script_by_name<S: ScriptRegistry>(
    State(state): State<Arc<AppState<S>>>,
    Path(name): Path<String>,
    Json(req): Json<RunScriptRequest>,
) -> ApiResult<Json<RunScriptResponse>> {
    let script = state
        .registry
        .get_by_name(&name)
        .await
        .map_err(ApiError::from)?;
    if script.version != req.expected_version {
        return Err(ApiError::from(ScriptError::RevisionConflict {
            expected: req.expected_version,
        }));
    }

    let params = req
        .params
        .into_iter()
        .map(|(k, v)| (k, json_to_dynamic(v)))
        .collect();

    let entity = req.entity.map(|entity_input| {
        let data = entity_input
            .data
            .into_iter()
            .map(|(k, v)| (k, json_to_dynamic(v)))
            .collect();

        EntityProxy::new(entity_input.id, entity_input.entity_type, data)
    });

    let result = state
        .orchestrator
        .run_manual_snapshot(&script, params, entity, None)
        .await;

    let (success, error, changes, return_value) = match &result.outcome {
        crate::runner::ExecutionOutcome::Success {
            return_value,
            entity_changes,
        } => (
            true,
            None,
            Some(convert_map(entity_changes.clone())),
            return_value
                .clone()
                .map(dynamic_to_json)
                .unwrap_or(serde_json::Value::Null),
        ),
        crate::runner::ExecutionOutcome::Aborted { reason } => (
            false,
            Some(reason.to_string()),
            None,
            serde_json::Value::Null,
        ),
        crate::runner::ExecutionOutcome::Failed { error } => (
            false,
            Some(error.to_string()),
            None,
            serde_json::Value::Null,
        ),
    };

    Ok(Json(RunScriptResponse {
        execution_id: result.execution_id.to_string(),
        success,
        duration_ms: result.duration_ms(),
        error,
        changes,
        return_value,
    }))
}

#[instrument(skip(state))]
pub async fn list_recent_executions<S: ScriptRegistry>(
    State(state): State<Arc<AppState<S>>>,
    Query(query): Query<ListExecutionLogQuery>,
) -> ApiResult<Json<ListExecutionLogResponse>> {
    let offset = query.offset();
    let limit = query.limit();
    let executions = state
        .execution_log
        .list_recent_paginated(offset, limit)
        .await
        .map_err(ApiError::from)?;
    let total = state
        .execution_log
        .count_recent()
        .await
        .map_err(ApiError::from)? as usize;
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

#[instrument(skip(state), fields(script_id = %id))]
pub async fn list_script_executions<S: ScriptRegistry>(
    State(state): State<Arc<AppState<S>>>,
    Path(id): Path<Uuid>,
    Query(query): Query<ListExecutionLogQuery>,
) -> ApiResult<Json<ListExecutionLogResponse>> {
    let offset = query.offset();
    let limit = query.limit();
    let executions = state
        .execution_log
        .list_for_script_paginated(id, offset, limit)
        .await
        .map_err(ApiError::from)?;
    let total = state
        .execution_log
        .count_for_script(id)
        .await
        .map_err(ApiError::from)? as usize;
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

#[instrument(skip(state))]
pub async fn validate_script<S: ScriptRegistry>(
    State(state): State<Arc<AppState<S>>>,
    Json(req): Json<CreateScriptRequest>,
) -> ApiResult<Json<serde_json::Value>> {
    req.workspace
        .validate_rhai_workspace()
        .map_err(ScriptError::from)?;
    let mut scope = rhai::Scope::new();
    match state.engine.compile(
        "__validation__",
        req.workspace
            .entrypoint_source()
            .map_err(ScriptError::from)?,
        &mut scope,
    ) {
        Ok(_) => Ok(Json(serde_json::json!({
            "valid": true,
            "message": "Script compiles successfully"
        }))),
        Err(e) => Ok(Json(serde_json::json!({
            "valid": false,
            "message": e.to_string()
        }))),
    }
}

// ============ Helpers ============

fn validate_trigger(trigger: &ScriptTrigger) -> ApiResult<()> {
    if let ScriptTrigger::Cron { expression } = trigger {
        validate_cron_expression(expression).map_err(|error| ApiError {
            error: format!("Invalid cron expression: {error}"),
            code: "validation".to_string(),
        })?;
    }

    Ok(())
}

fn convert_map(map: HashMap<String, rhai::Dynamic>) -> HashMap<String, serde_json::Value> {
    map.into_iter()
        .map(|(k, v)| (k, dynamic_to_json(v)))
        .collect()
}
