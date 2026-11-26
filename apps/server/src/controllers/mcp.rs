use axum::{
    Extension, Json,
    extract::{Path, Query, State},
    http::HeaderMap,
    response::sse::{Event, KeepAlive, Sse},
    routing::{get, post, put},
};
use std::str::FromStr;
use uuid::Uuid;

use crate::error::{Error, Result};
use crate::extractors::{
    rbac::{RequireMcpManage, RequireMcpRead},
    tenant::CurrentTenant,
};
use crate::services::mcp_host::{management, persistence};
use crate::services::server_runtime_context::ServerRuntimeContext;
use rustok_core::ModuleRegistry;
use rustok_mcp::McpManagementAuthorityError;
use rustok_mcp::persistence::McpPersistenceError;
use rustok_mcp::{
    ApplyMcpModuleScaffoldDraftRequest, ApplyModuleScaffoldRequest,
    BootstrapMcpRemoteSessionRequest, CreateMcpClientRequest, CreateMcpClientResponse,
    McpActorType, McpAuditEventResponse, McpAuditQuery, McpAuditSink, McpClientDetailsResponse,
    McpClientSummaryResponse, McpModuleScaffoldDraftResponse, McpPolicyResponse,
    McpRemoteBootstrapResponse, McpRemoteToolCallRequest, McpRemoteToolCallResponse,
    McpRuntimeBinding, McpScaffoldDraftRuntimeContext, McpScaffoldDraftStore, McpSessionContext,
    McpTokenResponse, McpToolCallAuditEvent, McpToolCallOutcome, McpToolResponse,
    RegistryToolInvocationError, ReviewModuleScaffoldRequest, RotateMcpTokenRequest,
    RotateMcpTokenResponse, ScaffoldModuleRequest, StageMcpModuleScaffoldDraftRequest,
    TOOL_ALLOY_APPLY_MODULE_SCAFFOLD, TOOL_ALLOY_IMPORT_PUBLISHED_RELEASE,
    TOOL_ALLOY_REVIEW_MODULE_SCAFFOLD, TOOL_ALLOY_SCAFFOLD_MODULE, TOOL_MCP_HEALTH,
    UpdateMcpPolicyRequest, default_tool_requirement, invoke_registry_tool,
    is_remote_alloy_authoring_tool,
};
use rustok_mcp::{
    ApplyMcpScaffoldDraftCommand, CreateMcpClientCommand, McpAuditEventRecord,
    McpClientDetailsRecord, McpClientRecord, McpManagementContext, McpManagementMutationError,
    McpPolicyRecord, McpScaffoldDraftRecord, McpTokenRecord, RotateMcpTokenCommand,
    StageMcpScaffoldDraftCommand, UpdateMcpPolicyCommand,
};
use tokio_stream::once;

#[cfg(feature = "mod-alloy")]
use rustok_mcp::{
    AlloyPublishedReleaseImportRequest, TOOL_ALLOY_CHANGE_DELETED_EVIDENCE_RETENTION,
    TOOL_ALLOY_CHANGE_SCRIPT_LIFECYCLE, TOOL_ALLOY_CREATE_SCRIPT, TOOL_ALLOY_DELETE_SCRIPT,
    TOOL_ALLOY_GET_DELETED_EVIDENCE_RETENTION, TOOL_ALLOY_GET_SCRIPT,
    TOOL_ALLOY_LIST_SCRIPT_REVIEWS, TOOL_ALLOY_LIST_SCRIPT_REVISIONS, TOOL_ALLOY_LIST_SCRIPTS,
    TOOL_ALLOY_REVIEW_SCRIPT, TOOL_ALLOY_RUN_SCRIPT, TOOL_ALLOY_RUN_WORKSPACE_TEST,
    TOOL_ALLOY_UPDATE_SCRIPT, TOOL_ALLOY_VALIDATE_SCRIPT, import_published_release,
};

async fn bootstrap_remote_session(
    State(ctx): State<ServerRuntimeContext>,
    headers: HeaderMap,
    Json(input): Json<BootstrapMcpRemoteSessionRequest>,
) -> Result<Json<McpRemoteBootstrapResponse>> {
    let plaintext_token = resolve_remote_mcp_token(&headers, input.plaintext_token)?;
    let correlation_id = normalize_remote_correlation_id(input.correlation_id)?;
    let transport = input
        .transport
        .filter(|value| !value.trim().is_empty())
        .unwrap_or_else(|| "http".to_string());

    tracing::debug!(
        transport_len = transport.len(),
        has_token = true,
        "Handling MCP bootstrap remote session request"
    );

    let bridge = persistence(&ctx)?;

    let response = bridge
        .bootstrap_remote_session(
            McpSessionContext::default()
                .with_transport(transport)
                .with_plaintext_token(plaintext_token)
                .with_metadata(input.metadata)
                .with_correlation_id(correlation_id),
        )
        .await
        .map_err(map_persistence_error)?;

    Ok(Json(response))
}

async fn call_remote_tool(
    State(ctx): State<ServerRuntimeContext>,
    Extension(registry): Extension<ModuleRegistry>,
    headers: HeaderMap,
    Json(input): Json<McpRemoteToolCallRequest>,
) -> Result<Json<McpRemoteToolCallResponse>> {
    tracing::debug!(
        tool_name_len = input.tool_name.len(),
        correlation_id_present = input
            .correlation_id
            .as_ref()
            .is_some_and(|value| !value.trim().is_empty()),
        "Handling MCP remote tool call request"
    );
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
    tracing::debug!(
        tool_name_len = input.tool_name.len(),
        correlation_id_present = input
            .correlation_id
            .as_ref()
            .is_some_and(|value| !value.trim().is_empty()),
        "Handling MCP stream remote tool request"
    );
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

fn normalize_remote_correlation_id(value: Option<String>) -> Result<String> {
    let Some(value) = value.filter(|value| !value.trim().is_empty()) else {
        return Ok(Uuid::new_v4().to_string());
    };

    let value = value.trim();
    if value.len() > 128 || !value.bytes().all(|byte| byte.is_ascii_graphic()) {
        return Err(crate::error::Error::BadRequest(
            "MCP correlation_id must contain 1-128 printable ASCII characters".to_string(),
        ));
    }

    Ok(value.to_string())
}

async fn execute_remote_tool_call(
    ctx: &ServerRuntimeContext,
    registry: ModuleRegistry,
    headers: HeaderMap,
    input: McpRemoteToolCallRequest,
    transport: &str,
) -> Result<McpRemoteToolCallResponse> {
    let plaintext_token = resolve_remote_mcp_token(&headers, input.plaintext_token)?;
    let correlation_id = normalize_remote_correlation_id(input.correlation_id)?;

    let bridge = persistence(ctx)?;
    let binding = bridge
        .resolve_binding_for_token(&plaintext_token)
        .await
        .map_err(map_persistence_error)?;
    let audit_metadata = remote_tool_audit_metadata(&input.tool_name, input.metadata.clone());

    #[cfg(feature = "mod-alloy")]
    let mut decision = if input.tool_name == TOOL_MCP_HEALTH {
        rustok_mcp::McpAuthorizationDecision::allow()
    } else {
        binding
            .access_context
            .authorize_tool(&default_tool_requirement(&input.tool_name))
    };
    #[cfg(not(feature = "mod-alloy"))]
    let decision = if input.tool_name == TOOL_MCP_HEALTH {
        rustok_mcp::McpAuthorizationDecision::allow()
    } else {
        binding
            .access_context
            .authorize_tool(&default_tool_requirement(&input.tool_name))
    };
    #[cfg(feature = "mod-alloy")]
    if decision.allowed
        && (is_remote_alloy_authoring_tool(&input.tool_name)
            || input.tool_name == TOOL_ALLOY_IMPORT_PUBLISHED_RELEASE)
        && remote_alloy_authoring_identity(&binding).is_err()
    {
        decision = rustok_mcp::McpAuthorizationDecision::deny(
            "access_denied",
            "Remote Alloy authoring requires a matching tenant-bound MCP identity",
        );
    }

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
                metadata: audit_metadata.clone(),
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
            metadata: audit_metadata,
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
    } else if is_remote_alloy_authoring_tool(&input.tool_name) {
        #[cfg(feature = "mod-alloy")]
        {
            execute_remote_alloy_authoring(ctx, &binding, &input.tool_name, input.arguments).await?
        }
        #[cfg(not(feature = "mod-alloy"))]
        {
            envelope_value(McpToolResponse::<()>::error(
                "tool_not_supported",
                "Alloy script authoring is unavailable on this server",
            ))?
        }
    } else if input.tool_name == TOOL_ALLOY_IMPORT_PUBLISHED_RELEASE {
        #[cfg(feature = "mod-alloy")]
        {
            execute_remote_alloy_published_release_import(ctx, &binding, input.arguments).await?
        }
        #[cfg(not(feature = "mod-alloy"))]
        {
            envelope_value(McpToolResponse::<()>::error(
                "tool_not_supported",
                "Published Alloy release import is unavailable on this server",
            ))?
        }
    } else {
        match invoke_registry_tool(
            &registry,
            &binding.access_context,
            &input.tool_name,
            input.arguments,
        )
        .await
        {
            Ok(result) => result,
            Err(RegistryToolInvocationError::Denied) => serde_json::to_value(
                McpToolResponse::<()>::error("access_denied", "MCP access policy denied this tool"),
            )?,
            Err(RegistryToolInvocationError::InvalidArguments) => serde_json::to_value(
                McpToolResponse::<()>::error("invalid_arguments", "MCP tool arguments are invalid"),
            )?,
            Err(RegistryToolInvocationError::UnsupportedTool) => {
                serde_json::to_value(McpToolResponse::<()>::error(
                    "tool_not_supported",
                    "Remote MCP tool is not supported",
                ))?
            }
            Err(RegistryToolInvocationError::Serialization) => {
                return Err(crate::error::Error::Message(
                    "MCP tool response serialization failed".to_string(),
                ));
            }
        }
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

/// Owner-bound Alloy commands must not use caller-controlled audit metadata.
/// The durable audit row confirms the operation and its binding without becoming
/// a persistence path for script source, tests, diagnostics, or retention reasons.
fn remote_tool_audit_metadata(tool_name: &str, metadata: serde_json::Value) -> serde_json::Value {
    if is_remote_alloy_authoring_tool(tool_name) || tool_name == TOOL_ALLOY_IMPORT_PUBLISHED_RELEASE
    {
        serde_json::json!({
            "redacted": true,
            "reason": "source_bearing_alloy_authoring",
        })
    } else {
        metadata
    }
}

#[cfg(feature = "mod-alloy")]
async fn execute_remote_alloy_authoring(
    ctx: &ServerRuntimeContext,
    binding: &McpRuntimeBinding,
    tool_name: &str,
    arguments: Option<serde_json::Value>,
) -> Result<serde_json::Value> {
    let (tenant_id, actor_id) = match remote_alloy_authoring_identity(binding) {
        Ok(identity) => identity,
        Err(()) => {
            return envelope_value(McpToolResponse::<()>::error(
                "access_denied",
                "Remote Alloy authoring requires a matching tenant-bound MCP identity",
            ));
        }
    };
    let Some(runtime) = ctx.shared_get::<alloy::SharedAlloyRuntime>() else {
        return envelope_value(McpToolResponse::<()>::error(
            "alloy_runtime_unavailable",
            "Alloy script authoring is unavailable on this server",
        ));
    };
    let service = alloy::AlloyAuthoringService::from_scoped(runtime.0.scoped(tenant_id));

    match tool_name {
        TOOL_ALLOY_LIST_SCRIPTS => remote_alloy_result(
            service
                .list_scripts(parse_remote_alloy_args(arguments)?)
                .await,
        ),
        TOOL_ALLOY_GET_SCRIPT => remote_alloy_result(
            service
                .get_script(parse_remote_alloy_args(arguments)?)
                .await,
        ),
        TOOL_ALLOY_LIST_SCRIPT_REVISIONS => remote_alloy_result(
            service
                .list_source_revisions(parse_remote_alloy_args(arguments)?)
                .await,
        ),
        TOOL_ALLOY_CREATE_SCRIPT => remote_alloy_result(
            service
                .create_script(&actor_id, parse_remote_alloy_args(arguments)?)
                .await,
        ),
        TOOL_ALLOY_UPDATE_SCRIPT => remote_alloy_result(
            service
                .update_script(&actor_id, parse_remote_alloy_args(arguments)?)
                .await,
        ),
        TOOL_ALLOY_DELETE_SCRIPT => remote_alloy_result(
            service
                .delete_script(&actor_id, parse_remote_alloy_args(arguments)?)
                .await,
        ),
        TOOL_ALLOY_GET_DELETED_EVIDENCE_RETENTION => remote_alloy_result(
            service
                .get_deleted_evidence_retention(parse_remote_alloy_args(arguments)?)
                .await,
        ),
        TOOL_ALLOY_CHANGE_DELETED_EVIDENCE_RETENTION => remote_alloy_result(
            service
                .change_deleted_evidence_retention(&actor_id, parse_remote_alloy_args(arguments)?)
                .await,
        ),
        TOOL_ALLOY_VALIDATE_SCRIPT => {
            remote_alloy_result(service.validate_script(parse_remote_alloy_args(arguments)?))
        }
        TOOL_ALLOY_RUN_SCRIPT => remote_alloy_result(
            service
                .run_script(&actor_id, parse_remote_alloy_args(arguments)?)
                .await,
        ),
        TOOL_ALLOY_REVIEW_SCRIPT => remote_alloy_result(
            service
                .review_script(&actor_id, parse_remote_alloy_args(arguments)?)
                .await,
        ),
        TOOL_ALLOY_LIST_SCRIPT_REVIEWS => remote_alloy_result(
            service
                .list_reviews(parse_remote_alloy_args(arguments)?)
                .await,
        ),
        TOOL_ALLOY_RUN_WORKSPACE_TEST => remote_alloy_result(
            service
                .run_workspace_test(&actor_id, parse_remote_alloy_args(arguments)?)
                .await,
        ),
        TOOL_ALLOY_CHANGE_SCRIPT_LIFECYCLE => remote_alloy_result(
            service
                .change_lifecycle(&actor_id, parse_remote_alloy_args(arguments)?)
                .await,
        ),
        _ => envelope_value(McpToolResponse::<()>::error(
            "tool_not_supported",
            "Remote Alloy authoring tool is not supported",
        )),
    }
}

#[cfg(feature = "mod-alloy")]
fn remote_alloy_authoring_identity(
    binding: &McpRuntimeBinding,
) -> std::result::Result<(Uuid, String), ()> {
    let Some(tenant_id) = binding
        .tenant_id
        .as_deref()
        .and_then(|value| Uuid::parse_str(value).ok())
    else {
        return Err(());
    };
    let Some(identity) = binding.access_context.identity.as_ref() else {
        return Err(());
    };
    if identity.actor_id.trim().is_empty()
        || identity.tenant_id.as_deref() != binding.tenant_id.as_deref()
    {
        return Err(());
    }
    Ok((tenant_id, identity.actor_id.clone()))
}

#[cfg(feature = "mod-alloy")]
fn parse_remote_alloy_args<T>(arguments: Option<serde_json::Value>) -> Result<T>
where
    T: serde::de::DeserializeOwned,
{
    serde_json::from_value(arguments.unwrap_or_else(|| serde_json::json!({}))).map_err(|_| {
        crate::error::Error::BadRequest("Remote Alloy authoring arguments are invalid".to_string())
    })
}

#[cfg(feature = "mod-alloy")]
fn remote_alloy_result<T>(
    result: std::result::Result<T, alloy::AlloyAuthoringError>,
) -> Result<serde_json::Value>
where
    T: serde::Serialize,
{
    match result {
        Ok(response) => envelope_value(McpToolResponse::success(response)),
        Err(alloy::AlloyAuthoringError::NotFound) => envelope_value(McpToolResponse::<()>::error(
            "alloy_script_not_found",
            "Alloy script was not found",
        )),
        Err(alloy::AlloyAuthoringError::RevisionConflict { .. }) => {
            envelope_value(McpToolResponse::<()>::error(
                "alloy_script_revision_conflict",
                "Alloy script revision conflict",
            ))
        }
        Err(alloy::AlloyAuthoringError::PresentationRevisionConflict { .. }) => {
            envelope_value(McpToolResponse::<()>::error(
                "alloy_script_presentation_revision_conflict",
                "Alloy script presentation revision conflict",
            ))
        }
        Err(alloy::AlloyAuthoringError::RetentionRevisionConflict { .. }) => {
            envelope_value(McpToolResponse::<()>::error(
                "alloy_evidence_retention_revision_conflict",
                "Alloy evidence retention revision conflict",
            ))
        }
        Err(alloy::AlloyAuthoringError::Invalid) => envelope_value(McpToolResponse::<()>::error(
            "invalid_alloy_authoring_command",
            "Alloy authoring command is invalid",
        )),
        Err(alloy::AlloyAuthoringError::Failed) => envelope_value(McpToolResponse::<()>::error(
            "alloy_authoring_failed",
            "Alloy authoring operation failed",
        )),
    }
}

#[cfg(feature = "mod-alloy")]
async fn execute_remote_alloy_published_release_import(
    ctx: &ServerRuntimeContext,
    binding: &McpRuntimeBinding,
    arguments: Option<serde_json::Value>,
) -> Result<serde_json::Value> {
    let (tenant_id, actor_id) = match remote_alloy_authoring_identity(binding) {
        Ok(identity) => identity,
        Err(()) => {
            return envelope_value(McpToolResponse::<()>::error(
                "access_denied",
                "Published Alloy release import requires a matching tenant-bound MCP identity",
            ));
        }
    };
    let Some(runtime) = ctx.shared_get::<alloy::SharedAlloyRuntime>() else {
        return envelope_value(McpToolResponse::<()>::error(
            "alloy_runtime_unavailable",
            "Published Alloy release import is unavailable on this server",
        ));
    };
    let Some(storage) = ctx.shared_get::<rustok_storage::StorageRuntime>() else {
        return envelope_value(McpToolResponse::<()>::error(
            "alloy_source_unavailable",
            "The canonical published Rhai workspace is unavailable",
        ));
    };
    let request = match arguments {
        Some(arguments) => {
            match serde_json::from_value::<AlloyPublishedReleaseImportRequest>(arguments) {
                Ok(request) => request,
                Err(_) => {
                    return envelope_value(McpToolResponse::<()>::error(
                        "invalid_arguments",
                        "Published Alloy release import arguments are invalid",
                    ));
                }
            }
        }
        None => {
            return envelope_value(McpToolResponse::<()>::error(
                "invalid_arguments",
                "Published Alloy release import arguments are required",
            ));
        }
    };
    let source = crate::services::registry_governance::alloy_published_rhai_source_provider_handle(
        ctx.db_clone(),
        storage,
    );
    let runtime = runtime.0.scoped(tenant_id);
    match import_published_release(runtime.storage, source, tenant_id, actor_id, request).await {
        Ok(response) => envelope_value(McpToolResponse::success(response)),
        Err(
            alloy::AlloyImportError::InvalidCommand
            | alloy::AlloyImportError::IneligibleRelease
            | alloy::AlloyImportError::InvalidSource,
        ) => envelope_value(McpToolResponse::<()>::error(
            "invalid_alloy_release_import",
            "The published release cannot be imported as an Alloy Rhai workspace",
        )),
        Err(alloy::AlloyImportError::SourceUnavailable(_)) => {
            envelope_value(McpToolResponse::<()>::error(
                "alloy_release_import_source_not_found",
                "The canonical published Rhai workspace is unavailable",
            ))
        }
        Err(alloy::AlloyImportError::IdempotencyConflict) => {
            envelope_value(McpToolResponse::<()>::error(
                "alloy_release_import_idempotency_conflict",
                "Alloy import idempotency key was reused for a different release command",
            ))
        }
        Err(alloy::AlloyImportError::DraftNameConflict) => {
            envelope_value(McpToolResponse::<()>::error(
                "alloy_release_import_draft_name_conflict",
                "An Alloy draft with the requested tenant-scoped name already exists",
            ))
        }
        Err(alloy::AlloyImportError::Storage(_)) => envelope_value(McpToolResponse::<()>::error(
            "alloy_release_import_failed",
            "Published Alloy release import failed",
        )),
    }
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
                Err(_error) => {
                    tracing::warn!(
                        failure_code = "mcp_scaffold_stage_failed",
                        "MCP scaffold stage failed"
                    );
                    envelope_value(McpToolResponse::<()>::error(
                        "scaffold_stage_failed",
                        "MCP scaffold stage request was invalid",
                    ))
                }
            }
        }
        TOOL_ALLOY_REVIEW_MODULE_SCAFFOLD => {
            let request: ReviewModuleScaffoldRequest = parse_tool_args(arguments)?;
            match draft_store.review_scaffold_draft(&context, request).await {
                Ok(response) => envelope_value(McpToolResponse::success(response)),
                Err(_error) => {
                    tracing::warn!(
                        failure_code = "mcp_scaffold_review_failed",
                        "MCP scaffold review failed"
                    );
                    envelope_value(McpToolResponse::<()>::error(
                        "scaffold_review_failed",
                        "MCP scaffold review request failed",
                    ))
                }
            }
        }
        TOOL_ALLOY_APPLY_MODULE_SCAFFOLD => {
            let request: ApplyModuleScaffoldRequest = parse_tool_args(arguments)?;
            match draft_store.apply_scaffold_draft(&context, request).await {
                Ok(response) => envelope_value(McpToolResponse::success(response)),
                Err(_error) => {
                    tracing::error!(
                        failure_code = "mcp_scaffold_apply_failed",
                        "MCP scaffold apply failed"
                    );
                    envelope_value(McpToolResponse::<()>::error(
                        "scaffold_apply_failed",
                        "MCP scaffold apply failed",
                    ))
                }
            }
        }
        _ => envelope_value(McpToolResponse::<()>::error(
            "tool_not_supported",
            format!("Remote HTTP transport does not support scaffold tool: {tool_name}"),
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

async fn list_clients(
    State(ctx): State<ServerRuntimeContext>,
    CurrentTenant(tenant): CurrentTenant,
    RequireMcpRead(user): RequireMcpRead,
) -> Result<Json<Vec<McpClientSummaryResponse>>> {
    let clients = management(&ctx)?
        .port()
        .list_clients(&management_context(tenant.id, &user), Some(100))
        .await
        .map_err(map_management_error)?;
    Ok(Json(clients.into_iter().map(map_client_summary).collect()))
}

async fn get_client(
    State(ctx): State<ServerRuntimeContext>,
    CurrentTenant(tenant): CurrentTenant,
    RequireMcpRead(user): RequireMcpRead,
    Path(client_id): Path<Uuid>,
) -> Result<Json<McpClientDetailsResponse>> {
    let details = management(&ctx)?
        .port()
        .get_client(&management_context(tenant.id, &user), client_id)
        .await
        .map_err(map_management_error)?
        .ok_or(Error::NotFound)?;
    Ok(Json(map_client_details(details)?))
}

async fn create_client(
    State(ctx): State<ServerRuntimeContext>,
    CurrentTenant(tenant): CurrentTenant,
    RequireMcpManage(user): RequireMcpManage,
    Json(input): Json<CreateMcpClientRequest>,
) -> Result<Json<CreateMcpClientResponse>, McpManagementHttpError> {
    let result = management(&ctx)?
        .port()
        .create_client(
            &management_context(tenant.id, &user),
            CreateMcpClientCommand {
                slug: input.slug,
                display_name: input.display_name,
                description: input.description,
                actor_type: parse_actor_type(&input.actor_type)?,
                delegated_user_id: input.delegated_user_id,
                token_name: input.token_name,
                token_expires_at: input.token_expires_at.map(|value| value.to_rfc3339()),
                allowed_tools: input.allowed_tools,
                denied_tools: input.denied_tools,
                granted_permissions: input.granted_permissions,
                granted_scopes: input.granted_scopes,
                metadata: input.metadata,
            },
        )
        .await
        .map_err(McpManagementHttpError::from)?;
    Ok(Json(CreateMcpClientResponse {
        client: map_client_summary(result.client),
        policy: map_policy(result.policy.ok_or(Error::InternalServerError)?),
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
) -> Result<Json<RotateMcpTokenResponse>, McpManagementHttpError> {
    let result = management(&ctx)?
        .port()
        .rotate_token(
            &management_context(tenant.id, &user),
            RotateMcpTokenCommand {
                client_id,
                token_name: input.token_name,
                expires_at: input.expires_at.map(|value| value.to_rfc3339()),
                metadata: input.metadata,
                revoke_existing_tokens: input.revoke_existing_tokens.unwrap_or(true),
            },
        )
        .await
        .map_err(McpManagementHttpError::from)?;
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
) -> Result<Json<McpPolicyResponse>, McpManagementHttpError> {
    let policy = management(&ctx)?
        .port()
        .update_policy(
            &management_context(tenant.id, &user),
            UpdateMcpPolicyCommand {
                client_id,
                allowed_tools: input.allowed_tools,
                denied_tools: input.denied_tools,
                granted_permissions: input.granted_permissions,
                granted_scopes: input.granted_scopes,
                metadata: input.metadata,
            },
        )
        .await
        .map_err(McpManagementHttpError::from)?;
    Ok(Json(map_policy(policy)))
}

async fn revoke_token_by_id(
    State(ctx): State<ServerRuntimeContext>,
    CurrentTenant(tenant): CurrentTenant,
    RequireMcpManage(user): RequireMcpManage,
    Path(token_id): Path<Uuid>,
) -> Result<Json<serde_json::Value>> {
    management(&ctx)?
        .port()
        .revoke_token(&management_context(tenant.id, &user), token_id, None)
        .await
        .map_err(map_management_error)?;
    Ok(Json(serde_json::json!({ "success": true })))
}

async fn deactivate_client(
    State(ctx): State<ServerRuntimeContext>,
    CurrentTenant(tenant): CurrentTenant,
    RequireMcpManage(user): RequireMcpManage,
    Path(client_id): Path<Uuid>,
) -> Result<Json<serde_json::Value>> {
    management(&ctx)?
        .port()
        .deactivate_client(&management_context(tenant.id, &user), client_id, None)
        .await
        .map_err(map_management_error)?;
    Ok(Json(serde_json::json!({ "success": true })))
}

async fn list_audit_events(
    State(ctx): State<ServerRuntimeContext>,
    CurrentTenant(tenant): CurrentTenant,
    RequireMcpRead(user): RequireMcpRead,
    Query(query): Query<McpAuditQuery>,
) -> Result<Json<Vec<McpAuditEventResponse>>> {
    let records = management(&ctx)?
        .port()
        .list_audit_events(
            &management_context(tenant.id, &user),
            query.client_id,
            query.outcome,
            query.limit,
        )
        .await
        .map_err(map_management_error)?;
    Ok(Json(records.into_iter().map(map_audit_event).collect()))
}

async fn list_scaffold_drafts(
    State(ctx): State<ServerRuntimeContext>,
    CurrentTenant(tenant): CurrentTenant,
    RequireMcpManage(user): RequireMcpManage,
) -> Result<Json<Vec<McpModuleScaffoldDraftResponse>>> {
    let records = management(&ctx)?
        .port()
        .list_scaffold_drafts(&management_context(tenant.id, &user), Some(100))
        .await
        .map_err(map_management_error)?;
    Ok(Json(records.into_iter().map(map_scaffold_draft).collect()))
}

async fn get_scaffold_draft(
    State(ctx): State<ServerRuntimeContext>,
    CurrentTenant(tenant): CurrentTenant,
    RequireMcpManage(user): RequireMcpManage,
    Path(draft_id): Path<Uuid>,
) -> Result<Json<McpModuleScaffoldDraftResponse>> {
    let record = management(&ctx)?
        .port()
        .get_scaffold_draft(&management_context(tenant.id, &user), draft_id)
        .await
        .map_err(map_management_error)?
        .ok_or(Error::NotFound)?;
    Ok(Json(map_scaffold_draft(record)))
}

fn management_context(
    tenant_id: Uuid,
    user: &crate::extractors::auth::CurrentUser,
) -> McpManagementContext {
    let (actor_id, actor_type) = if user.principal_kind.is_service() {
        (
            user.client_id.unwrap_or(user.user.id),
            McpActorType::ServiceClient,
        )
    } else {
        (user.user.id, McpActorType::HumanUser)
    };
    McpManagementContext::new(tenant_id, actor_id, actor_type, user.permissions.clone())
}

async fn stage_scaffold_draft(
    State(ctx): State<ServerRuntimeContext>,
    CurrentTenant(tenant): CurrentTenant,
    RequireMcpManage(user): RequireMcpManage,
    Json(input): Json<StageMcpModuleScaffoldDraftRequest>,
) -> Result<Json<McpModuleScaffoldDraftResponse>> {
    let draft = management(&ctx)?
        .port()
        .stage_scaffold_draft(
            &management_context(tenant.id, &user),
            StageMcpScaffoldDraftCommand {
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
            },
        )
        .await
        .map_err(map_management_error)?;
    Ok(Json(map_scaffold_draft(draft)))
}

async fn apply_scaffold_draft(
    State(ctx): State<ServerRuntimeContext>,
    CurrentTenant(tenant): CurrentTenant,
    RequireMcpManage(user): RequireMcpManage,
    Path(draft_id): Path<Uuid>,
    Json(input): Json<ApplyMcpModuleScaffoldDraftRequest>,
) -> Result<Json<McpModuleScaffoldDraftResponse>> {
    let draft = management(&ctx)?
        .port()
        .apply_scaffold_draft(
            &management_context(tenant.id, &user),
            ApplyMcpScaffoldDraftCommand {
                draft_id,
                workspace_root: input.workspace_root,
                confirm: input.confirm,
            },
        )
        .await
        .map_err(map_management_error)?;
    Ok(Json(map_scaffold_draft(draft)))
}

pub fn router() -> crate::routes::ServerRouter {
    axum::Router::new()
        .route("/api/mcp/runtime/bootstrap", post(bootstrap_remote_session))
        .route("/api/mcp/runtime/tools/call", post(call_remote_tool))
        .route("/api/mcp/runtime/tools/stream", post(stream_remote_tool))
        .route("/api/mcp/clients", get(list_clients).post(create_client))
        .route("/api/mcp/clients/", get(list_clients).post(create_client))
        .route("/api/mcp/clients/{id}", get(get_client))
        .route("/api/mcp/clients/{id}/rotate-token", post(rotate_token))
        .route("/api/mcp/clients/{id}/policy", put(update_policy))
        .route("/api/mcp/clients/{id}/deactivate", post(deactivate_client))
        .route("/api/mcp/tokens/{id}/revoke", post(revoke_token_by_id))
        .route(
            "/api/mcp/scaffold-drafts",
            get(list_scaffold_drafts).post(stage_scaffold_draft),
        )
        .route(
            "/api/mcp/scaffold-drafts/",
            get(list_scaffold_drafts).post(stage_scaffold_draft),
        )
        .route("/api/mcp/scaffold-drafts/{id}", get(get_scaffold_draft))
        .route(
            "/api/mcp/scaffold-drafts/{id}/apply",
            post(apply_scaffold_draft),
        )
        .route("/api/mcp/audit", get(list_audit_events))
        .route("/api/mcp/audit/", get(list_audit_events))
}

/// Resolve the single MCP credential before binding or audit writes.
/// A body token must never hide malformed or additional Authorization fields.
fn resolve_remote_mcp_token(headers: &HeaderMap, body_token: Option<String>) -> Result<String> {
    let mut values = headers.get_all(axum::http::header::AUTHORIZATION).iter();
    let first = values.next();
    if values.next().is_some() || (first.is_some() && body_token.is_some()) {
        return Err(Error::BadRequest(
            "Exactly one MCP credential transport is allowed".to_string(),
        ));
    }
    if let Some(value) = first {
        return value
            .to_str()
            .ok()
            .and_then(parse_bearer_token)
            .map(ToOwned::to_owned)
            .ok_or_else(|| Error::Unauthorized("Invalid MCP bearer token".into()));
    }
    body_token
        .filter(|token| !token.trim().is_empty())
        .ok_or_else(|| Error::Unauthorized("MCP bearer token is required".into()))
}

fn parse_bearer_token(value: &str) -> Option<&str> {
    let (scheme, token) = value.trim().split_once(' ')?;
    if !scheme.eq_ignore_ascii_case("bearer") {
        return None;
    }
    let token = token.trim();
    let token = if let Some(quoted) = token.strip_prefix('"') {
        quoted.strip_suffix('"')?
    } else {
        token
    };
    (!token.is_empty()
        && token.bytes().all(|byte| byte.is_ascii_graphic() && byte != b'"' && byte != b','))
    .then_some(token)
}

enum McpManagementHttpError {
    Delegation(McpManagementAuthorityError),
    Request(Error),
}

impl From<Error> for McpManagementHttpError {
    fn from(error: Error) -> Self {
        Self::Request(error)
    }
}

impl From<McpManagementMutationError> for McpManagementHttpError {
    fn from(error: McpManagementMutationError) -> Self {
        match error {
            McpManagementMutationError::Authority(error) => Self::Delegation(error),
            other => Self::Request(map_management_error(other)),
        }
    }
}

impl axum::response::IntoResponse for McpManagementHttpError {
    fn into_response(self) -> axum::response::Response {
        use axum::http::StatusCode;

        let (status, code, message) = match self {
            Self::Request(error) => return error.into_response(),
            Self::Delegation(error) => match error {
                McpManagementAuthorityError::Invalid(message) => {
                    (StatusCode::BAD_REQUEST, "invalid_request", message)
                }
                McpManagementAuthorityError::Forbidden(message) => {
                    (StatusCode::FORBIDDEN, "access_denied", message)
                }
                McpManagementAuthorityError::NotFound(message) => {
                    (StatusCode::NOT_FOUND, "not_found", message)
                }
                McpManagementAuthorityError::Internal(message) => {
                    (StatusCode::INTERNAL_SERVER_ERROR, "internal_error", message)
                }
            },
        };
        // These REST endpoints published delegation errors in this envelope.
        (
            status,
            Json(serde_json::json!({ "error": code, "message": message })),
        )
            .into_response()
    }
}

fn map_management_error(error: McpManagementMutationError) -> Error {
    match error {
        McpManagementMutationError::Authority(error) => map_authority_error(error),
        McpManagementMutationError::Validation(message) => Error::BadRequest(message),
        McpManagementMutationError::Forbidden(message) => Error::Forbidden(message),
        McpManagementMutationError::NotFound(_) => Error::NotFound,
        McpManagementMutationError::Conflict(message) => Error::Conflict(message),
        McpManagementMutationError::Internal(_) => Error::InternalServerError,
    }
}

fn map_persistence_error(error: McpPersistenceError) -> Error {
    match error {
        McpPersistenceError::Authority(error) => map_authority_error(error),
        McpPersistenceError::BadRequest(message) => Error::BadRequest(message),
        McpPersistenceError::Unauthorized(message) => Error::Unauthorized(message),
        McpPersistenceError::Forbidden(message) => Error::Forbidden(message),
        McpPersistenceError::NotFound => Error::NotFound,
        McpPersistenceError::InternalServerError => Error::InternalServerError,
        McpPersistenceError::Message(message) => Error::Message(message),
    }
}

fn map_authority_error(error: McpManagementAuthorityError) -> Error {
    match error {
        McpManagementAuthorityError::Invalid(message) => Error::BadRequest(message),
        McpManagementAuthorityError::Forbidden(message) => Error::Forbidden(message),
        McpManagementAuthorityError::NotFound(_) => Error::NotFound,
        McpManagementAuthorityError::Internal(_) => Error::InternalServerError,
    }
}

fn parse_actor_type(value: &str) -> Result<McpActorType> {
    McpActorType::from_str(value).map_err(|err| crate::error::Error::BadRequest(err.to_string()))
}

fn map_client_summary(model: McpClientRecord) -> McpClientSummaryResponse {
    let is_active = model.is_active;
    McpClientSummaryResponse {
        id: model.id,
        client_key: model.client_key,
        slug: model.slug,
        display_name: model.display_name,
        actor_type: model.actor_type.as_str().to_string(),
        is_active,
        last_used_at: model.last_used_at,
        created_at: model.created_at,
    }
}

fn map_policy(model: McpPolicyRecord) -> McpPolicyResponse {
    McpPolicyResponse {
        allowed_tools: model.allowed_tools,
        denied_tools: model.denied_tools,
        granted_permissions: model.granted_permissions,
        granted_scopes: model.granted_scopes,
        metadata: model.metadata,
        updated_at: model.updated_at,
    }
}

fn map_token(model: McpTokenRecord) -> McpTokenResponse {
    let is_active = model.is_active;
    McpTokenResponse {
        id: model.id,
        token_name: model.token_name,
        token_preview: model.token_preview,
        is_active,
        expires_at: model.expires_at,
        revoked_at: model.revoked_at,
        last_used_at: model.last_used_at,
        created_at: model.created_at,
    }
}

fn map_scaffold_draft(model: McpScaffoldDraftRecord) -> McpModuleScaffoldDraftResponse {
    McpModuleScaffoldDraftResponse {
        id: model.id,
        client_id: model.client_id,
        slug: model.slug,
        crate_name: model.crate_name,
        status: model.status,
        request_payload: model.request_payload,
        preview_payload: model.preview_payload,
        workspace_root: model.workspace_root,
        applied_at: model.applied_at,
        created_by: model.created_by,
        created_at: model.created_at,
        updated_at: model.updated_at,
    }
}

fn map_client_details(details: McpClientDetailsRecord) -> Result<McpClientDetailsResponse> {
    Ok(McpClientDetailsResponse {
        client: map_client_summary(details.client.clone()),
        description: details.client.description,
        delegated_user_id: details.client.delegated_user_id,
        metadata: details.client.metadata,
        policy: details.policy.map(map_policy),
        tokens: details.tokens.into_iter().map(map_token).collect(),
        effective_access_context: details
            .effective_access_context
            .map(serde_json::to_value)
            .transpose()?,
    })
}

fn map_audit_event(model: McpAuditEventRecord) -> McpAuditEventResponse {
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
        created_at: model.created_at,
    }
}

#[cfg(test)]
mod tests {
    use axum::{http::StatusCode, response::IntoResponse};

    use super::*;

    #[test]
    fn maps_mcp_authority_error_to_stable_http_statuses() {
        assert_eq!(
            map_management_error(McpManagementMutationError::Validation("bad".to_string()))
                .into_response()
                .status(),
            StatusCode::BAD_REQUEST
        );
        assert_eq!(
            map_management_error(McpManagementMutationError::Forbidden("no".to_string()))
                .into_response()
                .status(),
            StatusCode::FORBIDDEN
        );
        assert_eq!(
            map_management_error(McpManagementMutationError::NotFound("client".to_string()))
                .into_response()
                .status(),
            StatusCode::NOT_FOUND
        );
        assert_eq!(
            map_management_error(McpManagementMutationError::Internal("db".to_string()))
                .into_response()
                .status(),
            StatusCode::INTERNAL_SERVER_ERROR
        );
    }

    #[tokio::test]
    async fn delegation_denial_keeps_the_management_rest_error_envelope() {
        let response = McpManagementHttpError::from(McpManagementMutationError::Authority(
            McpManagementAuthorityError::Forbidden("grant exceeds authority".to_string()),
        ))
        .into_response();
        assert_eq!(response.status(), StatusCode::FORBIDDEN);
        let bytes = axum::body::to_bytes(response.into_body(), 1024)
            .await
            .unwrap();
        let body: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
        assert_eq!(
            body,
            serde_json::json!({
                "error": "access_denied",
                "message": "grant exceeds authority",
            })
        );
    }

    #[test]
    fn source_bearing_remote_authoring_audit_metadata_is_replaced() {
        let metadata = serde_json::json!({
            "workspace": { "files": { "main.rhai": "let credential = 'secret';" } },
            "note": "must not persist",
        });

        let redacted =
            remote_tool_audit_metadata(rustok_mcp::TOOL_ALLOY_CREATE_SCRIPT, metadata.clone());
        assert_eq!(redacted["redacted"], true);
        assert_eq!(redacted["reason"], "source_bearing_alloy_authoring");
        assert!(!redacted.to_string().contains("credential"));

        let import_metadata =
            remote_tool_audit_metadata(TOOL_ALLOY_IMPORT_PUBLISHED_RELEASE, metadata);
        assert_eq!(import_metadata["redacted"], true);
        assert_eq!(import_metadata["reason"], "source_bearing_alloy_authoring");
        assert!(!import_metadata.to_string().contains("credential"));
    }

    #[cfg(feature = "mod-alloy")]
    #[test]
    fn remote_authoring_rejects_identity_from_another_tenant() {
        let bound_tenant = Uuid::new_v4();
        let binding = McpRuntimeBinding {
            access_context: rustok_mcp::McpAccessContext {
                identity: Some(rustok_mcp::McpIdentity {
                    actor_id: "operator-1".to_string(),
                    actor_type: McpActorType::HumanUser,
                    tenant_id: Some(Uuid::new_v4().to_string()),
                    delegated_user_id: None,
                    display_name: None,
                    scopes: Vec::new(),
                }),
                ..Default::default()
            },
            tenant_id: Some(bound_tenant.to_string()),
            client_id: Some("client-1".to_string()),
            token_id: Some("token-1".to_string()),
        };

        assert!(remote_alloy_authoring_identity(&binding).is_err());
    }

    #[test]
    fn remote_correlation_ids_are_safe_for_sse_and_audit() {
        assert_eq!(
            normalize_remote_correlation_id(Some("  trace-id:123  ".to_string())).unwrap(),
            "trace-id:123"
        );

        let generated = normalize_remote_correlation_id(Some("  ".to_string())).unwrap();
        assert!(Uuid::parse_str(&generated).is_ok());
        let safe_event_id =
            normalize_remote_correlation_id(Some("trace-id:123".to_string())).unwrap();
        let _ = Event::default().id(safe_event_id);

        for invalid in [
            "line1\nline2".to_string(),
            "line1\rline2".to_string(),
            "nul\0value".to_string(),
            "x".repeat(129),
            "non-ascii-☃".to_string(),
        ] {
            assert!(normalize_remote_correlation_id(Some(invalid)).is_err());
        }
    }

    #[test]
    fn parses_bearer_tokens_case_insensitively_with_whitespace_tolerance() {
        assert_eq!(parse_bearer_token("Bearer token-123"), Some("token-123"));
        assert_eq!(parse_bearer_token("bearer token-456"), Some("token-456"));
        assert_eq!(parse_bearer_token("BEARER token-789"), Some("token-789"));
        assert_eq!(
            parse_bearer_token("  Bearer   token-abc  "),
            Some("token-abc")
        );
        assert_eq!(
            parse_bearer_token("Bearer \"quoted-token\""),
            Some("quoted-token")
        );
        assert_eq!(parse_bearer_token("Basic token-123"), None);
        assert_eq!(parse_bearer_token("Bearer"), None);
        assert_eq!(parse_bearer_token("Bearer   "), None);
        assert_eq!(parse_bearer_token(""), None);
        for invalid in [
            "Bearer \"token",
            "Bearer token\"",
            "Bearer \"\"token\"\"",
            "Bearer token,token",
            "Bearer token token",
        ] {
            assert_eq!(parse_bearer_token(invalid), None, "{invalid}");
        }
    }

    #[test]
    fn remote_mcp_token_rejects_duplicate_and_mixed_credentials() {
        use axum::http::header::AUTHORIZATION;

        for second in ["Bearer token-1", "Bearer token-2"] {
            let mut headers = HeaderMap::new();
            headers.append(AUTHORIZATION, "Bearer token-1".parse().unwrap());
            headers.append(AUTHORIZATION, second.parse().unwrap());
            assert!(matches!(
                resolve_remote_mcp_token(&headers, None),
                Err(Error::BadRequest(_))
            ));
            assert!(matches!(
                resolve_remote_mcp_token(&headers, Some("token-1".to_string())),
                Err(Error::BadRequest(_))
            ));
        }

        let mut headers = HeaderMap::new();
        assert_eq!(
            resolve_remote_mcp_token(&headers, Some("body-token".to_string())).unwrap(),
            "body-token"
        );
        assert!(matches!(
            resolve_remote_mcp_token(&headers, None),
            Err(Error::Unauthorized(_))
        ));
        headers.insert(AUTHORIZATION, "Bearer header-token".parse().unwrap());
        assert_eq!(
            resolve_remote_mcp_token(&headers, None).unwrap(),
            "header-token"
        );
        assert!(matches!(
            resolve_remote_mcp_token(&headers, Some("header-token".to_string())),
            Err(Error::BadRequest(_))
        ));
        headers.insert(AUTHORIZATION, "Basic invalid".parse().unwrap());
        assert!(matches!(
            resolve_remote_mcp_token(&headers, Some("body-token".to_string())),
            Err(Error::BadRequest(_))
        ));
        assert!(matches!(
            resolve_remote_mcp_token(&headers, None),
            Err(Error::Unauthorized(_))
        ));
    }
}
