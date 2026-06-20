use std::collections::HashMap;

use axum::{
    Json,
    extract::{Path, Query, State},
    http::StatusCode,
    routing::{get, post},
};
use chrono::Utc;
use loco_rs::{Error, Result, app::AppContext, controller::Routes};
use rustok_api::TenantContext;
use uuid::Uuid;

use crate::{
    ScriptError, ScriptRegistry,
    api::{
        CreateScriptRequest, EntityInput, ExecutionLogResponse, ListExecutionLogResponse,
        ListScriptsQuery, ListScriptsResponse, RunScriptRequest, RunScriptResponse, ScriptResponse,
        UpdateScriptRequest,
    },
    model::{EntityProxy, Script, ScriptStatus},
    runner::ExecutionOutcome,
    utils::{dynamic_to_json, json_to_dynamic},
};

pub const LOCO_EXECUTION_HISTORY_ROUTES: &[&str] =
    &["/executions", "/scripts/{id}/executions"];

fn script_error(error: ScriptError) -> Error {
    match error {
        ScriptError::NotFound { .. } => Error::NotFound,
        ScriptError::Compilation(message)
        | ScriptError::InvalidTrigger(message)
        | ScriptError::InvalidStatus(message) => Error::BadRequest(message),
        other => Error::Message(other.to_string()),
    }
}

fn entity_to_proxy(entity: EntityInput) -> EntityProxy {
    let data = entity
        .data
        .into_iter()
        .map(|(key, value)| (key, json_to_dynamic(value)))
        .collect();

    EntityProxy::new(entity.id, entity.entity_type, data)
}

pub async fn list_scripts(
    State(ctx): State<AppContext>,
    tenant: TenantContext,
    Query(query): Query<ListScriptsQuery>,
) -> Result<Json<ListScriptsResponse>> {
    let runtime = crate::runtime::scoped_runtime(&ctx, tenant.id);
    let script_query = match query.status.as_deref().and_then(ScriptStatus::parse) {
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
        query.page,
        query.per_page,
    )))
}

pub async fn get_script(
    State(ctx): State<AppContext>,
    tenant: TenantContext,
    Path(id): Path<Uuid>,
) -> Result<Json<ScriptResponse>> {
    let runtime = crate::runtime::scoped_runtime(&ctx, tenant.id);
    let script = runtime.storage.get(id).await.map_err(script_error)?;
    Ok(Json(script.into()))
}

pub async fn create_script(
    State(ctx): State<AppContext>,
    tenant: TenantContext,
    Json(req): Json<CreateScriptRequest>,
) -> Result<(StatusCode, Json<ScriptResponse>)> {
    let runtime = crate::runtime::scoped_runtime(&ctx, tenant.id);

    if runtime.storage.get_by_name(&req.name).await.is_ok() {
        return Err(Error::BadRequest(format!(
            "Script with name '{}' already exists",
            req.name
        )));
    }

    let mut script = Script::new(req.name, req.code, req.trigger);
    script.tenant_id = req.tenant_id.unwrap_or(tenant.id);
    script.description = req.description;
    script.permissions = req.permissions;
    script.run_as_system = req.run_as_system;

    let saved = runtime.storage.save(script).await.map_err(script_error)?;
    Ok((StatusCode::CREATED, Json(saved.into())))
}

pub async fn update_script(
    State(ctx): State<AppContext>,
    tenant: TenantContext,
    Path(id): Path<Uuid>,
    Json(req): Json<UpdateScriptRequest>,
) -> Result<Json<ScriptResponse>> {
    let runtime = crate::runtime::scoped_runtime(&ctx, tenant.id);
    let mut script = runtime.storage.get(id).await.map_err(script_error)?;

    if let Some(name) = req.name {
        runtime.engine.invalidate(&script.name);
        script.name = name;
    }
    if let Some(description) = req.description {
        script.description = Some(description);
    }
    if let Some(code) = req.code {
        runtime.engine.invalidate(&script.name);
        script.code = code;
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
    State(ctx): State<AppContext>,
    tenant: TenantContext,
    Path(id): Path<Uuid>,
) -> Result<StatusCode> {
    let runtime = crate::runtime::scoped_runtime(&ctx, tenant.id);
    let script = runtime.storage.get(id).await.map_err(script_error)?;
    runtime.engine.invalidate(&script.name);
    runtime.storage.delete(id).await.map_err(script_error)?;
    Ok(StatusCode::NO_CONTENT)
}

pub async fn run_script(
    State(ctx): State<AppContext>,
    tenant: TenantContext,
    Path(id): Path<Uuid>,
    Json(req): Json<RunScriptRequest>,
) -> Result<Json<RunScriptResponse>> {
    let runtime = crate::runtime::scoped_runtime(&ctx, tenant.id);
    let script = runtime.storage.get(id).await.map_err(script_error)?;

    let params = req
        .params
        .into_iter()
        .map(|(key, value)| (key, json_to_dynamic(value)))
        .collect::<HashMap<_, _>>();
    let entity = req.entity.map(entity_to_proxy);

    let result = runtime
        .orchestrator
        .run_manual_with_entity(&script.name, params, entity, None)
        .await
        .map_err(script_error)?;

    Ok(Json(run_response(result)))
}

pub async fn run_script_by_name(
    State(ctx): State<AppContext>,
    tenant: TenantContext,
    Path(name): Path<String>,
    Json(req): Json<RunScriptRequest>,
) -> Result<Json<RunScriptResponse>> {
    let runtime = crate::runtime::scoped_runtime(&ctx, tenant.id);
    let script = runtime
        .storage
        .get_by_name(&name)
        .await
        .map_err(script_error)?;

    let params = req
        .params
        .into_iter()
        .map(|(key, value)| (key, json_to_dynamic(value)))
        .collect::<HashMap<_, _>>();
    let entity = req.entity.map(entity_to_proxy);

    let result = runtime
        .orchestrator
        .run_manual_with_entity(&script.name, params, entity, None)
        .await
        .map_err(script_error)?;

    Ok(Json(run_response(result)))
}

pub async fn list_recent_executions(
    State(ctx): State<AppContext>,
    tenant: TenantContext,
    Query(query): Query<ListScriptsQuery>,
) -> Result<Json<ListExecutionLogResponse>> {
    let runtime = crate::runtime::scoped_runtime(&ctx, tenant.id);
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
        query.page,
        query.per_page,
    )))
}

pub async fn list_script_executions(
    State(ctx): State<AppContext>,
    tenant: TenantContext,
    Path(id): Path<Uuid>,
    Query(query): Query<ListScriptsQuery>,
) -> Result<Json<ListExecutionLogResponse>> {
    let runtime = crate::runtime::scoped_runtime(&ctx, tenant.id);
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
        query.page,
        query.per_page,
    )))
}

pub async fn validate_script(
    State(ctx): State<AppContext>,
    tenant: TenantContext,
    Json(req): Json<CreateScriptRequest>,
) -> Result<Json<serde_json::Value>> {
    let runtime = crate::runtime::scoped_runtime(&ctx, tenant.id);
    let mut scope = rhai_full::Scope::new();

    match runtime
        .engine
        .compile("__validation__", &req.code, &mut scope)
    {
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
    State(ctx): State<AppContext>,
    tenant: TenantContext,
    Path(id): Path<Uuid>,
) -> Result<Json<ScriptResponse>> {
    let runtime = crate::runtime::scoped_runtime(&ctx, tenant.id);
    let mut script = runtime.storage.get(id).await.map_err(script_error)?;
    script.activate();
    let saved = runtime.storage.save(script).await.map_err(script_error)?;
    Ok(Json(saved.into()))
}

pub async fn pause_script(
    State(ctx): State<AppContext>,
    tenant: TenantContext,
    Path(id): Path<Uuid>,
) -> Result<Json<ScriptResponse>> {
    let runtime = crate::runtime::scoped_runtime(&ctx, tenant.id);
    let mut script = runtime.storage.get(id).await.map_err(script_error)?;
    script.status = ScriptStatus::Paused;
    script.updated_at = Utc::now();
    let saved = runtime.storage.save(script).await.map_err(script_error)?;
    Ok(Json(saved.into()))
}

pub fn routes() -> Routes {
    Routes::new()
        .prefix("api/alloy")
        .add("/scripts", get(list_scripts).post(create_script))
        .add(LOCO_EXECUTION_HISTORY_ROUTES[0], get(list_recent_executions))
        .add("/scripts/validate", post(validate_script))
        .add(
            "/scripts/{id}",
            get(get_script).put(update_script).delete(delete_script),
        )
        .add("/scripts/{id}/run", post(run_script))
        .add(
            LOCO_EXECUTION_HISTORY_ROUTES[1],
            get(list_script_executions),
        )
        .add("/scripts/name/{name}/run", post(run_script_by_name))
        .add("/scripts/{id}/activate", post(activate_script))
        .add("/scripts/{id}/pause", post(pause_script))
}

#[cfg(test)]
mod tests {
    use super::LOCO_EXECUTION_HISTORY_ROUTES;

    #[test]
    fn loco_execution_history_routes_match_operator_contract() {
        assert_eq!(
            LOCO_EXECUTION_HISTORY_ROUTES,
            &["/executions", "/scripts/{id}/executions"]
        );
    }
}
