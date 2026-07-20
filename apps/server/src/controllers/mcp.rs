use axum::{
    Extension, Json,
    extract::{Path, Query, State},
    http::HeaderMap,
    response::sse::{Event, KeepAlive, Sse},
    routing::{get, post, put},
};
use std::str::FromStr;
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
use crate::services::server_runtime_context::ServerRuntimeContext;
use rustok_core::ModuleRegistry;
use rustok_mcp::{
    ApplyMcpModuleScaffoldDraftRequest, ApplyModuleScaffoldRequest,
    BootstrapMcpRemoteSessionRequest, CreateMcpClientRequest, CreateMcpClientResponse,
    McpActorType, McpAuditEventResponse, McpAuditQuery, McpAuditSink, McpClientDetailsResponse,
    McpClientSummaryResponse, McpModuleScaffoldDraftResponse, McpPolicyResponse,
    McpRemoteToolCallRequest, McpRemoteToolCallResponse, McpRuntimeBinding,
    McpScaffoldDraftRuntimeContext, McpScaffoldDraftStore, McpSessionContext, McpTokenResponse,
    McpToolCallAuditEvent, McpToolCallOutcome, McpToolResponse, ModuleDetailsResponse, ModuleInfo,
    ModuleListResponse, ModuleLookupRequest, ModuleLookupResponse, ReviewModuleScaffoldRequest,
    RotateMcpTokenRequest, RotateMcpTokenResponse, ScaffoldModuleRequest,
    StageMcpModuleScaffoldDraftRequest, TOOL_ALLOY_APPLY_MODULE_SCAFFOLD,
    TOOL_ALLOY_REVIEW_MODULE_SCAFFOLD, TOOL_ALLOY_SCAFFOLD_MODULE, TOOL_LIST_MODULES,
    TOOL_MCP_HEALTH, TOOL_MCP_WHOAMI, TOOL_MODULE_DETAILS, TOOL_MODULE_EXISTS, TOOL_QUERY_MODULES,
    UpdateMcpPolicyRequest, default_tool_requirement,
};
use tokio_stream::once;

async fn bootstrap_remote_session(
    State(ctx): State<ServerRuntimeContext>,
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
        .shared_get::<std::sync::Arc<DbBackedMcpRuntimeBridge>>()
        .unwrap_or_else(|| DbBackedMcpRuntimeBridge::shared(ctx.db_clone()));

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

async fn call_remote_tool(
    State(ctx): State<ServerRuntimeContext>,
    Extension(registry): Extension<ModuleRegistry>,
    headers: HeaderMap,
    Json(input): Json<McpRemoteToolCallRequest>,
) -> Result<Json<McpRemoteToolCallResponse>> {
    let response = execute_remote_tool_call(&ctx, registry, headers, input, "http-json").await?;
    Ok(Json(response))
}

async fn stream_remote_tool(
    State(ctx): State<ServerRuntimeContext>,
    Extension(registry): Extension<ModuleRegistry>,
    headers: HeaderMap,
    Json(input): Json<McpRemoteToolCallRequest>,
) -> Result<
    Sse<impl futures_util::Stream<Item = std::result::Result<Event, std::convert::Infallible>>>,
> {
    let response = execute_remote_tool_call(&ctx, registry, headers, input, "sse").await?;
    let event = Event::default()
        .event("mcp.tool.result")
        .id(response.correlation_id.clone())
        .json_data(response)
        .map_err(|error| {
            crate::error::Error::Message(format!("Failed to serialize MCP SSE event: {error}"))
        })?;

    Ok(Sse::new(once(Ok(event))).keep_alive(KeepAlive::default()))
}

async fn execute_remote_tool_call(
    ctx: &ServerRuntimeContext,
    registry: ModuleRegistry,
    headers: HeaderMap,
    input: McpRemoteToolCallRequest,
    transport: &str,
) -> Result<McpRemoteToolCallResponse> {
    let plaintext_token = input
        .plaintext_token
        .or_else(|| bearer_token_from_headers(&headers))
        .ok_or_else(|| crate::error::Error::Unauthorized("MCP bearer token is required".into()))?;
    let correlation_id = input
        .correlation_id
        .filter(|value| !value.trim().is_empty())
        .unwrap_or_else(|| Uuid::new_v4().to_string());

    let bridge = ctx
        .shared_get::<std::sync::Arc<DbBackedMcpRuntimeBridge>>()
        .unwrap_or_else(|| DbBackedMcpRuntimeBridge::shared(ctx.db_clone()));
    let binding = bridge.resolve_binding_for_token(&plaintext_token).await?;

    let decision = if input.tool_name == TOOL_MCP_HEALTH {
        rustok_mcp::McpAuthorizationDecision::allow()
    } else {
        binding
            .access_context
            .authorize_tool(&default_tool_requirement(&input.tool_name))
    };

    if !decision.allowed {
        bridge
            .record_tool_call(McpToolCallAuditEvent {
                transport: transport.to_string(),
                tenant_id: binding.tenant_id.clone(),
                client_id: binding.client_id.clone(),
                token_id: binding.token_id.clone(),
                identity: binding.access_context.identity.clone(),
                tool_name: input.tool_name.clone(),
                outcome: McpToolCallOutcome::Denied,
                reason: decision.message.clone().or_else(|| decision.code.clone()),
                correlation_id: Some(correlation_id.clone()),
                metadata: input.metadata.clone(),
            })
            .await
            .map_err(|error| crate::error::Error::Message(error.to_string()))?;
        let result = serde_json::to_value(McpToolResponse::<()>::error(
            decision.code.unwrap_or_else(|| "access_denied".to_string()),
            decision
                .message
                .unwrap_or_else(|| "MCP access policy denied this tool".to_string()),
        ))?;
        return Ok(McpRemoteToolCallResponse {
            transport: transport.to_string(),
            correlation_id,
            tenant_id: binding.tenant_id,
            client_id: binding.client_id,
            token_id: binding.token_id,
            tool_name: input.tool_name,
            result,
        });
    }

    bridge
        .record_tool_call(McpToolCallAuditEvent {
            transport: transport.to_string(),
            tenant_id: binding.tenant_id.clone(),
            client_id: binding.client_id.clone(),
            token_id: binding.token_id.clone(),
            identity: binding.access_context.identity.clone(),
            tool_name: input.tool_name.clone(),
            outcome: McpToolCallOutcome::Allowed,
            reason: None,
            correlation_id: Some(correlation_id.clone()),
            metadata: input.metadata.clone(),
        })
        .await
        .map_err(|error| crate::error::Error::Message(error.to_string()))?;

    let result = if is_remote_scaffold_tool(&input.tool_name) {
        execute_remote_scaffold_tool(
            bridge.as_ref(),
            &binding,
            transport,
            &correlation_id,
            &input.tool_name,
            input.arguments,
            input.metadata.clone(),
        )
        .await?
    } else {
        execute_registry_tool(
            &registry,
            &binding.access_context,
            &input.tool_name,
            input.arguments,
        )?
    };

    Ok(McpRemoteToolCallResponse {
        transport: transport.to_string(),
        correlation_id,
        tenant_id: binding.tenant_id,
        client_id: binding.client_id,
        token_id: binding.token_id,
        tool_name: input.tool_name,
        result,
    })
}

fn is_remote_scaffold_tool(tool_name: &str) -> bool {
    matches!(
        tool_name,
        TOOL_ALLOY_SCAFFOLD_MODULE
            | TOOL_ALLOY_REVIEW_MODULE_SCAFFOLD
            | TOOL_ALLOY_APPLY_MODULE_SCAFFOLD
    )
}

async fn execute_remote_scaffold_tool(
    draft_store: &dyn McpScaffoldDraftStore,
    binding: &McpRuntimeBinding,
    transport: &str,
    correlation_id: &str,
    tool_name: &str,
    arguments: Option<serde_json::Value>,
    metadata: serde_json::Value,
) -> Result<serde_json::Value> {
    let context = McpScaffoldDraftRuntimeContext {
        session: McpSessionContext::default()
            .with_transport(transport.to_string())
            .with_correlation_id(correlation_id.to_string())
            .with_metadata(metadata),
        runtime_binding: Some(binding.clone()),
        access_context: Some(binding.access_context.clone()),
    };

    match tool_name {
        TOOL_ALLOY_SCAFFOLD_MODULE => {
            let request: ScaffoldModuleRequest = parse_tool_args(arguments)?;
            match draft_store.stage_scaffold_draft(&context, request).await {
                Ok(response) => envelope_value(McpToolResponse::success(response)),
                Err(error) => envelope_value(McpToolResponse::<()>::error(
                    "scaffold_stage_failed",
                    error.to_string(),
                )),
            }
        }
        TOOL_ALLOY_REVIEW_MODULE_SCAFFOLD => {
            let request: ReviewModuleScaffoldRequest = parse_tool_args(arguments)?;
            match draft_store.review_scaffold_draft(&context, request).await {
                Ok(response) => envelope_value(McpToolResponse::success(response)),
                Err(error) => envelope_value(McpToolResponse::<()>::error(
                    "scaffold_review_failed",
                    error.to_string(),
                )),
            }
        }
        TOOL_ALLOY_APPLY_MODULE_SCAFFOLD => {
            let request: ApplyModuleScaffoldRequest = parse_tool_args(arguments)?;
            match draft_store.apply_scaffold_draft(&context, request).await {
                Ok(response) => envelope_value(McpToolResponse::success(response)),
                Err(error) => envelope_value(McpToolResponse::<()>::error(
                    "scaffold_apply_failed",
                    error.to_string(),
                )),
            }
        }
        _ => envelope_value(McpToolResponse::<()>::error(
            "tool_not_supported",
            format!("Remote HTTP transport does not support scaffold tool: {tool_name}"),
        )),
    }
}

fn execute_registry_tool(
    registry: &ModuleRegistry,
    access_context: &rustok_mcp::McpAccessContext,
    tool_name: &str,
    arguments: Option<serde_json::Value>,
) -> Result<serde_json::Value> {
    match tool_name {
        TOOL_LIST_MODULES => envelope_value(McpToolResponse::success(ModuleListResponse {
            modules: registry.list().into_iter().map(module_info).collect(),
        })),
        TOOL_QUERY_MODULES => {
            let request: rustok_mcp::ModuleQueryRequest = parse_tool_args(arguments)?;
            let modules = registry
                .list()
                .into_iter()
                .filter(|module| {
                    request
                        .slug_prefix
                        .as_ref()
                        .is_none_or(|prefix| module.slug().starts_with(prefix))
                })
                .filter(|module| {
                    request.dependency.as_ref().is_none_or(|dependency| {
                        module
                            .dependencies()
                            .iter()
                            .any(|value| value == dependency)
                    })
                })
                .skip(request.offset.unwrap_or(0))
                .take(request.limit.unwrap_or(usize::MAX))
                .map(module_info)
                .collect::<Vec<_>>();
            envelope_value(McpToolResponse::success(ModuleListResponse { modules }))
        }
        TOOL_MODULE_EXISTS => {
            let request: ModuleLookupRequest = parse_tool_args(arguments)?;
            envelope_value(McpToolResponse::success(ModuleLookupResponse {
                exists: registry.contains(&request.slug),
                slug: request.slug,
            }))
        }
        TOOL_MODULE_DETAILS => {
            let request: ModuleLookupRequest = parse_tool_args(arguments)?;
            let module = registry.get(&request.slug).map(module_info);
            envelope_value(McpToolResponse::success(ModuleDetailsResponse {
                slug: request.slug,
                module,
            }))
        }
        TOOL_MCP_WHOAMI => envelope_value(McpToolResponse::success(access_context.whoami())),
        TOOL_MCP_HEALTH => {
            envelope_value(McpToolResponse::success(rustok_mcp::McpHealthResponse {
                status: "ready".to_string(),
                protocol_version: "2024-11-05".to_string(),
                tool_count: 9,
                enabled_tools: access_context.whoami().allowed_tools,
                access_mode: "policy".to_string(),
                identity: access_context.identity.clone(),
            }))
        }
        _ => envelope_value(McpToolResponse::<()>::error(
            "tool_not_supported",
            format!("Remote HTTP transport does not support tool: {tool_name}"),
        )),
    }
}

fn envelope_value<T: serde::Serialize>(envelope: McpToolResponse<T>) -> Result<serde_json::Value> {
    serde_json::to_value(envelope).map_err(Into::into)
}

fn parse_tool_args<T: serde::de::DeserializeOwned>(
    arguments: Option<serde_json::Value>,
) -> Result<T> {
    serde_json::from_value(arguments.unwrap_or_else(|| serde_json::json!({}))).map_err(Into::into)
}

fn module_info(module: &dyn rustok_core::RusToKModule) -> ModuleInfo {
    ModuleInfo {
        slug: module.slug().to_string(),
        name: module.name().to_string(),
        description: module.description().to_string(),
        version: module.version().to_string(),
        dependencies: module
            .dependencies()
            .iter()
            .map(|value| (*value).to_string())
            .collect(),
    }
}

async fn list_clients(
    State(ctx): State<ServerRuntimeContext>,
    CurrentTenant(tenant): CurrentTenant,
    RequireMcpRead(_user): RequireMcpRead,
) -> Result<Json<Vec<McpClientSummaryResponse>>> {
    let clients = McpManagementService::list_clients(ctx.db(), tenant.id, Some(100)).await?;
    Ok(Json(clients.into_iter().map(map_client_summary).collect()))
}

async fn get_client(
    State(ctx): State<ServerRuntimeContext>,
    CurrentTenant(tenant): CurrentTenant,
    RequireMcpRead(_user): RequireMcpRead,
    Path(client_id): Path<Uuid>,
) -> Result<Json<McpClientDetailsResponse>> {
    let details = McpManagementService::get_client_details(ctx.db(), tenant.id, client_id)
        .await?
        .ok_or(crate::error::Error::NotFound)?;
    Ok(Json(map_client_details(details)))
}

async fn create_client(
    State(ctx): State<ServerRuntimeContext>,
    CurrentTenant(tenant): CurrentTenant,
    RequireMcpManage(user): RequireMcpManage,
    Json(input): Json<CreateMcpClientRequest>,
) -> Result<Json<CreateMcpClientResponse>> {
    let result = McpManagementService::create_client(
        ctx.db(),
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
    State(ctx): State<ServerRuntimeContext>,
    CurrentTenant(tenant): CurrentTenant,
    RequireMcpManage(user): RequireMcpManage,
    Path(client_id): Path<Uuid>,
    Json(input): Json<RotateMcpTokenRequest>,
) -> Result<Json<RotateMcpTokenResponse>> {
    let result = McpManagementService::rotate_token(
        ctx.db(),
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
    State(ctx): State<ServerRuntimeContext>,
    CurrentTenant(tenant): CurrentTenant,
    RequireMcpManage(user): RequireMcpManage,
    Path(client_id): Path<Uuid>,
    Json(input): Json<UpdateMcpPolicyRequest>,
) -> Result<Json<McpPolicyResponse>> {
    let policy = McpManagementService::update_policy(
        ctx.db(),
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
    State(ctx): State<ServerRuntimeContext>,
    CurrentTenant(tenant): CurrentTenant,
    RequireMcpManage(user): RequireMcpManage,
    Path(token_id): Path<Uuid>,
) -> Result<Json<serde_json::Value>> {
    McpManagementService::revoke_token(ctx.db(), tenant.id, token_id, Some(user.user.id), None)
        .await?;
    Ok(Json(serde_json::json!({ "success": true })))
}

async fn deactivate_client(
    State(ctx): State<ServerRuntimeContext>,
    CurrentTenant(tenant): CurrentTenant,
    RequireMcpManage(user): RequireMcpManage,
    Path(client_id): Path<Uuid>,
) -> Result<Json<serde_json::Value>> {
    McpManagementService::deactivate_client(
        ctx.db(),
        tenant.id,
        client_id,
        Some(user.user.id),
        None,
    )
    .await?;
    Ok(Json(serde_json::json!({ "success": true })))
}

async fn list_audit_events(
    State(ctx): State<ServerRuntimeContext>,
    CurrentTenant(tenant): CurrentTenant,
    RequireMcpRead(_user): RequireMcpRead,
    Query(query): Query<McpAuditQuery>,
) -> Result<Json<Vec<McpAuditEventResponse>>> {
    let events = McpManagementService::list_audit_events(
        ctx.db(),
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
    State(ctx): State<ServerRuntimeContext>,
    CurrentTenant(tenant): CurrentTenant,
    RequireMcpManage(_user): RequireMcpManage,
) -> Result<Json<Vec<McpModuleScaffoldDraftResponse>>> {
    let drafts = McpManagementService::list_scaffold_drafts(ctx.db(), tenant.id, Some(100)).await?;
    Ok(Json(drafts.into_iter().map(map_scaffold_draft).collect()))
}

async fn get_scaffold_draft(
    State(ctx): State<ServerRuntimeContext>,
    CurrentTenant(tenant): CurrentTenant,
    RequireMcpManage(_user): RequireMcpManage,
    Path(draft_id): Path<Uuid>,
) -> Result<Json<McpModuleScaffoldDraftResponse>> {
    let draft = McpManagementService::get_scaffold_draft(ctx.db(), tenant.id, draft_id)
        .await?
        .ok_or(crate::error::Error::NotFound)?;
    Ok(Json(map_scaffold_draft(draft)))
}

async fn stage_scaffold_draft(
    State(ctx): State<ServerRuntimeContext>,
    CurrentTenant(tenant): CurrentTenant,
    RequireMcpManage(user): RequireMcpManage,
    Json(input): Json<StageMcpModuleScaffoldDraftRequest>,
) -> Result<Json<McpModuleScaffoldDraftResponse>> {
    let draft = McpManagementService::stage_scaffold_draft(
        ctx.db(),
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
    State(ctx): State<ServerRuntimeContext>,
    CurrentTenant(tenant): CurrentTenant,
    RequireMcpManage(user): RequireMcpManage,
    Path(draft_id): Path<Uuid>,
    Json(input): Json<ApplyMcpModuleScaffoldDraftRequest>,
) -> Result<Json<McpModuleScaffoldDraftResponse>> {
    let (draft, _) = McpManagementService::apply_scaffold_draft(
        ctx.db(),
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

pub fn router() -> crate::routes::ServerRouter {
    axum::Router::new()
        .route("/api/mcp/runtime/bootstrap", post(bootstrap_remote_session))
        .route("/api/mcp/runtime/tools/call", post(call_remote_tool))
        .route("/api/mcp/runtime/tools/stream", post(stream_remote_tool))
        .route("/api/mcp/clients", get(list_clients).post(create_client))
        .route("/api/mcp/clients/{id}", get(get_client))
        .route("/api/mcp/clients/{id}/rotate-token", post(rotate_token))
        .route("/api/mcp/clients/{id}/policy", put(update_policy))
        .route("/api/mcp/clients/{id}/deactivate", post(deactivate_client))
        .route("/api/mcp/tokens/{id}/revoke", post(revoke_token_by_id))
        .route(
            "/api/mcp/scaffold-drafts",
            get(list_scaffold_drafts).post(stage_scaffold_draft),
        )
        .route("/api/mcp/scaffold-drafts/{id}", get(get_scaffold_draft))
        .route(
            "/api/mcp/scaffold-drafts/{id}/apply",
            post(apply_scaffold_draft),
        )
        .route("/api/mcp/audit", get(list_audit_events))
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
    McpActorType::from_str(value).map_err(crate::error::Error::BadRequest)
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
