use axum::{
    Json,
    extract::{Path, Query, State},
    http::HeaderMap,
    routing::{get, post, put},
};
use chrono::{DateTime, Utc};
use loco_rs::app::AppContext;
use loco_rs::controller::Routes;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::error::Result;
use crate::extractors::{
    rbac::{RequireMcpManage, RequireMcpRead},
    tenant::CurrentTenant,
};
use crate::services::mcp_management::{
    ApplyMcpScaffoldDraftInput, CreateMcpClientInput, McpAuditFilters, McpClientDetails,
    McpManagementService, RotateMcpTokenInput, StageMcpScaffoldDraftInput, UpdateMcpPolicyInput,
};
use crate::services::mcp_runtime::{DbBackedMcpRuntimeBridge, McpRemoteBootstrapResponse};
use rustok_mcp::{McpActorType, McpSessionContext, ScaffoldModuleRequest};

#[derive(Debug, Deserialize)]
pub struct CreateMcpClientRequest {
    pub slug: String,
    pub display_name: String,
    pub description: Option<String>,
    pub actor_type: String,
    pub delegated_user_id: Option<Uuid>,
    pub token_name: Option<String>,
    pub token_expires_at: Option<DateTime<Utc>>,
    #[serde(default)]
    pub allowed_tools: Vec<String>,
    #[serde(default)]
    pub denied_tools: Vec<String>,
    #[serde(default)]
    pub granted_permissions: Vec<String>,
    #[serde(default)]
    pub granted_scopes: Vec<String>,
    #[serde(default = "default_metadata")]
    pub metadata: serde_json::Value,
}

#[derive(Debug, Deserialize)]
pub struct RotateMcpTokenRequest {
    pub token_name: Option<String>,
    pub expires_at: Option<DateTime<Utc>>,
    pub revoke_existing_tokens: Option<bool>,
    #[serde(default = "default_metadata")]
    pub metadata: serde_json::Value,
}

#[derive(Debug, Deserialize)]
pub struct UpdateMcpPolicyRequest {
    #[serde(default)]
    pub allowed_tools: Vec<String>,
    #[serde(default)]
    pub denied_tools: Vec<String>,
    #[serde(default)]
    pub granted_permissions: Vec<String>,
    #[serde(default)]
    pub granted_scopes: Vec<String>,
    #[serde(default = "default_metadata")]
    pub metadata: serde_json::Value,
}

#[derive(Debug, Deserialize)]
pub struct BootstrapMcpRemoteSessionRequest {
    pub transport: Option<String>,
    pub plaintext_token: Option<String>,
    pub correlation_id: Option<String>,
    #[serde(default = "default_metadata")]
    pub metadata: serde_json::Value,
}

#[derive(Debug, Deserialize)]
pub struct McpAuditQuery {
    pub client_id: Option<Uuid>,
    pub outcome: Option<String>,
    pub limit: Option<u64>,
}

#[derive(Debug, Deserialize)]
pub struct StageMcpModuleScaffoldDraftRequest {
    pub client_id: Option<Uuid>,
    pub slug: String,
    pub name: String,
    pub description: String,
    #[serde(default)]
    pub dependencies: Vec<String>,
    pub with_graphql: Option<bool>,
    pub with_rest: Option<bool>,
}

#[derive(Debug, Deserialize)]
pub struct ApplyMcpModuleScaffoldDraftRequest {
    pub workspace_root: String,
    pub confirm: bool,
}

#[derive(Debug, Serialize)]
pub struct McpClientSummaryResponse {
    pub id: Uuid,
    pub client_key: Uuid,
    pub slug: String,
    pub display_name: String,
    pub actor_type: String,
    pub is_active: bool,
    pub last_used_at: Option<DateTime<Utc>>,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Serialize)]
pub struct McpPolicyResponse {
    pub allowed_tools: Vec<String>,
    pub denied_tools: Vec<String>,
    pub granted_permissions: Vec<String>,
    pub granted_scopes: Vec<String>,
    pub metadata: serde_json::Value,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Serialize)]
pub struct McpTokenResponse {
    pub id: Uuid,
    pub token_name: String,
    pub token_preview: String,
    pub is_active: bool,
    pub expires_at: Option<DateTime<Utc>>,
    pub revoked_at: Option<DateTime<Utc>>,
    pub last_used_at: Option<DateTime<Utc>>,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Serialize)]
pub struct McpClientDetailsResponse {
    pub client: McpClientSummaryResponse,
    pub description: Option<String>,
    pub delegated_user_id: Option<Uuid>,
    pub metadata: serde_json::Value,
    pub policy: Option<McpPolicyResponse>,
    pub tokens: Vec<McpTokenResponse>,
    pub effective_access_context: Option<serde_json::Value>,
}

#[derive(Debug, Serialize)]
pub struct CreateMcpClientResponse {
    pub client: McpClientSummaryResponse,
    pub policy: McpPolicyResponse,
    pub token: McpTokenResponse,
    pub plaintext_token: String,
}

#[derive(Debug, Serialize)]
pub struct RotateMcpTokenResponse {
    pub client: McpClientSummaryResponse,
    pub token: McpTokenResponse,
    pub plaintext_token: String,
}

#[derive(Debug, Serialize)]
pub struct McpAuditEventResponse {
    pub id: Uuid,
    pub client_id: Option<Uuid>,
    pub token_id: Option<Uuid>,
    pub actor_id: Option<String>,
    pub actor_type: Option<String>,
    pub action: String,
    pub outcome: String,
    pub tool_name: Option<String>,
    pub reason: Option<String>,
    pub correlation_id: Option<String>,
    pub metadata: serde_json::Value,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Serialize)]
pub struct McpModuleScaffoldDraftResponse {
    pub id: Uuid,
    pub client_id: Option<Uuid>,
    pub slug: String,
    pub crate_name: String,
    pub status: String,
    pub request_payload: serde_json::Value,
    pub preview_payload: serde_json::Value,
    pub workspace_root: Option<String>,
    pub applied_at: Option<DateTime<Utc>>,
    pub created_by: Option<Uuid>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

async fn bootstrap_remote_session(
    State(ctx): State<AppContext>,
    headers: HeaderMap,
    Json(input): Json<BootstrapMcpRemoteSessionRequest>,
) -> Result<Json<McpRemoteBootstrapResponse>> {
    let plaintext_token = input
        .plaintext_token
        .or_else(|| bearer_token_from_headers(&headers))
        .ok_or_else(|| crate::error::Error::Unauthorized("MCP bearer token is required".into()))?;
    let transport = input
        .transport
        .filter(|value| !value.trim().is_empty())
        .unwrap_or_else(|| "http".to_string());

    let bridge = ctx
        .shared_store
        .get::<std::sync::Arc<DbBackedMcpRuntimeBridge>>()
        .unwrap_or_else(|| DbBackedMcpRuntimeBridge::shared(ctx.db.clone()));

    let response = bridge
        .bootstrap_remote_session(
            McpSessionContext::default()
                .with_transport(transport)
                .with_plaintext_token(plaintext_token)
                .with_metadata(input.metadata)
                .with_correlation_id(
                    input
                        .correlation_id
                        .clone()
                        .unwrap_or_else(|| Uuid::new_v4().to_string()),
                ),
        )
        .await?;

    Ok(Json(response))
}

async fn list_clients(
    State(ctx): State<AppContext>,
    CurrentTenant(tenant): CurrentTenant,
    RequireMcpRead(_user): RequireMcpRead,
) -> Result<Json<Vec<McpClientSummaryResponse>>> {
    let clients = McpManagementService::list_clients(&ctx.db, tenant.id, Some(100)).await?;
    Ok(Json(clients.into_iter().map(map_client_summary).collect()))
}

async fn get_client(
    State(ctx): State<AppContext>,
    CurrentTenant(tenant): CurrentTenant,
    RequireMcpRead(_user): RequireMcpRead,
    Path(client_id): Path<Uuid>,
) -> Result<Json<McpClientDetailsResponse>> {
    let details = McpManagementService::get_client_details(&ctx.db, tenant.id, client_id)
        .await?
        .ok_or(crate::error::Error::NotFound)?;
    Ok(Json(map_client_details(details)))
}

async fn create_client(
    State(ctx): State<AppContext>,
    CurrentTenant(tenant): CurrentTenant,
    RequireMcpManage(user): RequireMcpManage,
    Json(input): Json<CreateMcpClientRequest>,
) -> Result<Json<CreateMcpClientResponse>> {
    let result = McpManagementService::create_client(
        &ctx.db,
        tenant.id,
        CreateMcpClientInput {
            slug: input.slug,
            display_name: input.display_name,
            description: input.description,
            actor_type: parse_actor_type(&input.actor_type)?,
            delegated_user_id: input.delegated_user_id,
            token_name: input.token_name,
            token_expires_at: input.token_expires_at,
            allowed_tools: input.allowed_tools,
            denied_tools: input.denied_tools,
            granted_permissions: input.granted_permissions,
            granted_scopes: input.granted_scopes,
            metadata: input.metadata,
            created_by: Some(user.user.id),
        },
    )
    .await?;

    Ok(Json(CreateMcpClientResponse {
        client: map_client_summary(result.client),
        policy: map_policy(result.policy),
        token: map_token(result.token),
        plaintext_token: result.plaintext_token,
    }))
}

async fn rotate_token(
    State(ctx): State<AppContext>,
    CurrentTenant(tenant): CurrentTenant,
    RequireMcpManage(user): RequireMcpManage,
    Path(client_id): Path<Uuid>,
    Json(input): Json<RotateMcpTokenRequest>,
) -> Result<Json<RotateMcpTokenResponse>> {
    let result = McpManagementService::rotate_token(
        &ctx.db,
        tenant.id,
        client_id,
        RotateMcpTokenInput {
            token_name: input.token_name,
            expires_at: input.expires_at,
            metadata: input.metadata,
            created_by: Some(user.user.id),
            revoke_existing_tokens: input.revoke_existing_tokens.unwrap_or(true),
        },
    )
    .await?;

    Ok(Json(RotateMcpTokenResponse {
        client: map_client_summary(result.client),
        token: map_token(result.token),
        plaintext_token: result.plaintext_token,
    }))
}

async fn update_policy(
    State(ctx): State<AppContext>,
    CurrentTenant(tenant): CurrentTenant,
    RequireMcpManage(user): RequireMcpManage,
    Path(client_id): Path<Uuid>,
    Json(input): Json<UpdateMcpPolicyRequest>,
) -> Result<Json<McpPolicyResponse>> {
    let policy = McpManagementService::update_policy(
        &ctx.db,
        tenant.id,
        client_id,
        UpdateMcpPolicyInput {
            allowed_tools: input.allowed_tools,
            denied_tools: input.denied_tools,
            granted_permissions: input.granted_permissions,
            granted_scopes: input.granted_scopes,
            metadata: input.metadata,
            updated_by: Some(user.user.id),
        },
    )
    .await?;

    Ok(Json(map_policy(policy)))
}

async fn revoke_token_by_id(
    State(ctx): State<AppContext>,
    CurrentTenant(tenant): CurrentTenant,
    RequireMcpManage(user): RequireMcpManage,
    Path(token_id): Path<Uuid>,
) -> Result<Json<serde_json::Value>> {
    McpManagementService::revoke_token(&ctx.db, tenant.id, token_id, Some(user.user.id), None)
        .await?;
    Ok(Json(serde_json::json!({ "success": true })))
}

async fn deactivate_client(
    State(ctx): State<AppContext>,
    CurrentTenant(tenant): CurrentTenant,
    RequireMcpManage(user): RequireMcpManage,
    Path(client_id): Path<Uuid>,
) -> Result<Json<serde_json::Value>> {
    McpManagementService::deactivate_client(
        &ctx.db,
        tenant.id,
        client_id,
        Some(user.user.id),
        None,
    )
    .await?;
    Ok(Json(serde_json::json!({ "success": true })))
}

async fn list_audit_events(
    State(ctx): State<AppContext>,
    CurrentTenant(tenant): CurrentTenant,
    RequireMcpRead(_user): RequireMcpRead,
    Query(query): Query<McpAuditQuery>,
) -> Result<Json<Vec<McpAuditEventResponse>>> {
    let events = McpManagementService::list_audit_events(
        &ctx.db,
        tenant.id,
        McpAuditFilters {
            client_id: query.client_id,
            outcome: query.outcome,
            limit: query.limit,
        },
    )
    .await?;

    Ok(Json(
        events.into_iter().map(map_audit_event).collect::<Vec<_>>(),
    ))
}

async fn list_scaffold_drafts(
    State(ctx): State<AppContext>,
    CurrentTenant(tenant): CurrentTenant,
    RequireMcpManage(_user): RequireMcpManage,
) -> Result<Json<Vec<McpModuleScaffoldDraftResponse>>> {
    let drafts = McpManagementService::list_scaffold_drafts(&ctx.db, tenant.id, Some(100)).await?;
    Ok(Json(drafts.into_iter().map(map_scaffold_draft).collect()))
}

async fn get_scaffold_draft(
    State(ctx): State<AppContext>,
    CurrentTenant(tenant): CurrentTenant,
    RequireMcpManage(_user): RequireMcpManage,
    Path(draft_id): Path<Uuid>,
) -> Result<Json<McpModuleScaffoldDraftResponse>> {
    let draft = McpManagementService::get_scaffold_draft(&ctx.db, tenant.id, draft_id)
        .await?
        .ok_or(crate::error::Error::NotFound)?;
    Ok(Json(map_scaffold_draft(draft)))
}

async fn stage_scaffold_draft(
    State(ctx): State<AppContext>,
    CurrentTenant(tenant): CurrentTenant,
    RequireMcpManage(user): RequireMcpManage,
    Json(input): Json<StageMcpModuleScaffoldDraftRequest>,
) -> Result<Json<McpModuleScaffoldDraftResponse>> {
    let draft = McpManagementService::stage_scaffold_draft(
        &ctx.db,
        tenant.id,
        StageMcpScaffoldDraftInput {
            client_id: input.client_id,
            request: ScaffoldModuleRequest {
                slug: input.slug,
                name: input.name,
                description: input.description,
                dependencies: input.dependencies,
                with_graphql: input.with_graphql.unwrap_or(true),
                with_rest: input.with_rest.unwrap_or(true),
                write_files: false,
            },
            created_by: Some(user.user.id),
        },
    )
    .await?;

    Ok(Json(map_scaffold_draft(draft)))
}

async fn apply_scaffold_draft(
    State(ctx): State<AppContext>,
    CurrentTenant(tenant): CurrentTenant,
    RequireMcpManage(user): RequireMcpManage,
    Path(draft_id): Path<Uuid>,
    Json(input): Json<ApplyMcpModuleScaffoldDraftRequest>,
) -> Result<Json<McpModuleScaffoldDraftResponse>> {
    let (draft, _) = McpManagementService::apply_scaffold_draft(
        &ctx.db,
        tenant.id,
        draft_id,
        ApplyMcpScaffoldDraftInput {
            workspace_root: input.workspace_root,
            confirm: input.confirm,
            applied_by: Some(user.user.id),
        },
    )
    .await?;

    Ok(Json(map_scaffold_draft(draft)))
}

pub fn routes() -> Routes {
    Routes::new()
        .prefix("api/mcp")
        .add("/runtime/bootstrap", post(bootstrap_remote_session))
        .add("/clients", get(list_clients).post(create_client))
        .add("/clients/{id}", get(get_client))
        .add("/clients/{id}/rotate-token", post(rotate_token))
        .add("/clients/{id}/policy", put(update_policy))
        .add("/clients/{id}/deactivate", post(deactivate_client))
        .add("/tokens/{id}/revoke", post(revoke_token_by_id))
        .add(
            "/scaffold-drafts",
            get(list_scaffold_drafts).post(stage_scaffold_draft),
        )
        .add("/scaffold-drafts/{id}", get(get_scaffold_draft))
        .add("/scaffold-drafts/{id}/apply", post(apply_scaffold_draft))
        .add("/audit", get(list_audit_events))
}

fn bearer_token_from_headers(headers: &HeaderMap) -> Option<String> {
    headers
        .get(axum::http::header::AUTHORIZATION)
        .and_then(|value| value.to_str().ok())
        .and_then(|value| value.strip_prefix("Bearer "))
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(ToOwned::to_owned)
}

fn parse_actor_type(value: &str) -> Result<McpActorType> {
    match value {
        "human_user" => Ok(McpActorType::HumanUser),
        "service_client" => Ok(McpActorType::ServiceClient),
        "model_agent" => Ok(McpActorType::ModelAgent),
        _ => Err(crate::error::Error::BadRequest(format!(
            "Unknown MCP actor type: {value}"
        ))),
    }
}

fn map_client_summary(model: crate::models::mcp_clients::Model) -> McpClientSummaryResponse {
    let is_active = model.is_active();
    McpClientSummaryResponse {
        id: model.id,
        client_key: model.client_key,
        slug: model.slug,
        display_name: model.display_name,
        actor_type: model.actor_type,
        is_active,
        last_used_at: model.last_used_at.map(Into::into),
        created_at: model.created_at.into(),
    }
}

fn map_policy(model: crate::models::mcp_policies::Model) -> McpPolicyResponse {
    McpPolicyResponse {
        allowed_tools: model.allowed_tools_list(),
        denied_tools: model.denied_tools_list(),
        granted_permissions: model.granted_permissions_list(),
        granted_scopes: model.granted_scopes_list(),
        metadata: model.metadata,
        updated_at: model.updated_at.into(),
    }
}

fn map_token(model: crate::models::mcp_tokens::Model) -> McpTokenResponse {
    let is_active = model.is_active();
    McpTokenResponse {
        id: model.id,
        token_name: model.token_name,
        token_preview: model.token_preview,
        is_active,
        expires_at: model.expires_at.map(Into::into),
        revoked_at: model.revoked_at.map(Into::into),
        last_used_at: model.last_used_at.map(Into::into),
        created_at: model.created_at.into(),
    }
}

fn map_scaffold_draft(
    model: crate::models::mcp_scaffold_drafts::Model,
) -> McpModuleScaffoldDraftResponse {
    McpModuleScaffoldDraftResponse {
        id: model.id,
        client_id: model.client_id,
        slug: model.slug,
        crate_name: model.crate_name,
        status: model.status,
        request_payload: model.request_payload,
        preview_payload: model.preview_payload,
        workspace_root: model.workspace_root,
        applied_at: model.applied_at.map(Into::into),
        created_by: model.created_by,
        created_at: model.created_at.into(),
        updated_at: model.updated_at.into(),
    }
}

fn map_client_details(details: McpClientDetails) -> McpClientDetailsResponse {
    McpClientDetailsResponse {
        client: map_client_summary(details.client.clone()),
        description: details.client.description,
        delegated_user_id: details.client.delegated_user_id,
        metadata: details.client.metadata,
        policy: details.policy.map(map_policy),
        tokens: details.tokens.into_iter().map(map_token).collect(),
        effective_access_context: details
            .effective_access_context
            .and_then(|value| serde_json::to_value(value).ok()),
    }
}

fn map_audit_event(model: crate::models::mcp_audit_logs::Model) -> McpAuditEventResponse {
    McpAuditEventResponse {
        id: model.id,
        client_id: model.client_id,
        token_id: model.token_id,
        actor_id: model.actor_id,
        actor_type: model.actor_type,
        action: model.action,
        outcome: model.outcome,
        tool_name: model.tool_name,
        reason: model.reason,
        correlation_id: model.correlation_id,
        metadata: model.metadata,
        created_at: model.created_at.into(),
    }
}

fn default_metadata() -> serde_json::Value {
    serde_json::json!({})
}
