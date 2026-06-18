use std::sync::Arc;

use async_trait::async_trait;
use chrono::Utc;
use sea_orm::{
    ActiveModelTrait, ActiveValue::Set, ColumnTrait, DatabaseConnection, EntityTrait, QueryFilter,
};
use sha2::{Digest, Sha256};
use uuid::Uuid;

use crate::error::{Error, Result};
use crate::models::{mcp_clients, mcp_policies, mcp_tokens};
use crate::services::mcp_management::{
    ApplyMcpScaffoldDraftInput, McpManagementService, RecordMcpAuditEventInput,
    StageMcpScaffoldDraftInput,
};
use rustok_core::ModuleRegistry;
use rustok_mcp::{
    ApplyModuleScaffoldRequest, ApplyModuleScaffoldResponse, McpAccessContext, McpAccessPolicy,
    McpAccessResolver, McpActorType, McpAuditSink, McpIdentity, McpRuntimeBinding,
    McpScaffoldDraftRuntimeContext, McpScaffoldDraftStore, McpServerConfig, McpSessionContext,
    McpToolCallAuditEvent, McpToolCallOutcome, ReviewModuleScaffoldRequest,
    ReviewModuleScaffoldResponse, ScaffoldModuleRequest, StageModuleScaffoldResponse,
    TOOL_MCP_WHOAMI,
};

#[derive(Debug, Clone, serde::Serialize)]
pub struct McpRemoteBootstrapResponse {
    pub transport: String,
    pub correlation_id: Option<String>,
    pub tenant_id: Option<String>,
    pub client_id: Option<String>,
    pub token_id: Option<String>,
    pub access_context: McpAccessContext,
}

pub struct DbBackedMcpRuntimeBridge {
    db: DatabaseConnection,
}

impl DbBackedMcpRuntimeBridge {
    pub fn new(db: DatabaseConnection) -> Self {
        Self { db }
    }

    pub fn shared(db: DatabaseConnection) -> Arc<Self> {
        Arc::new(Self::new(db))
    }

    pub fn bind_stdio_config(
        self: &Arc<Self>,
        registry: ModuleRegistry,
        session_context: McpSessionContext,
    ) -> McpServerConfig {
        McpServerConfig::new(registry)
            .with_session_context(session_context)
            .with_access_resolver(Arc::clone(self))
            .with_audit_sink(Arc::clone(self))
    }

    pub async fn bootstrap_remote_session(
        &self,
        session_context: McpSessionContext,
    ) -> Result<McpRemoteBootstrapResponse> {
        if session_context.transport.trim().is_empty() || session_context.transport == "stdio" {
            return Err(Error::BadRequest(
                "Remote MCP bootstrap requires a non-stdio transport".to_string(),
            ));
        }

        let plaintext_token = session_context
            .plaintext_token
            .as_deref()
            .ok_or_else(|| Error::Unauthorized("MCP bearer token is required".into()))?;
        let binding = self.resolve_binding_for_token(plaintext_token).await?;

        McpManagementService::record_audit_event(
            &self.db,
            RecordMcpAuditEventInput {
                tenant_id: parse_uuid(binding.tenant_id.as_deref()).ok_or_else(|| {
                    Error::BadRequest("MCP runtime binding did not include tenant_id".to_string())
                })?,
                client_id: parse_uuid(binding.client_id.as_deref()),
                token_id: parse_uuid(binding.token_id.as_deref()),
                actor_id: binding
                    .access_context
                    .identity
                    .as_ref()
                    .map(|identity| identity.actor_id.clone()),
                actor_type: binding
                    .access_context
                    .identity
                    .as_ref()
                    .map(|identity| actor_type_slug(identity.actor_type).to_string()),
                action: "remote_session_bootstrapped".to_string(),
                outcome: "success".to_string(),
                tool_name: None,
                reason: None,
                correlation_id: session_context.correlation_id.clone(),
                metadata: serde_json::json!({
                    "transport": session_context.transport,
                    "session_metadata": session_context.metadata,
                }),
                created_by: actor_user_id_from_access_context(&binding.access_context),
            },
        )
        .await?;

        Ok(McpRemoteBootstrapResponse {
            transport: session_context.transport,
            correlation_id: session_context.correlation_id,
            tenant_id: binding.tenant_id,
            client_id: binding.client_id,
            token_id: binding.token_id,
            access_context: binding.access_context,
        })
    }

    pub async fn resolve_binding_for_token(
        &self,
        plaintext_token: &str,
    ) -> Result<McpRuntimeBinding> {
        let token = mcp_tokens::Entity::find_active_by_hash(&self.db, &hash_token(plaintext_token))
            .await
            .map_err(map_db_err)?
            .ok_or_else(|| Error::Unauthorized("Invalid or expired MCP token".into()))?;

        let client = mcp_clients::Entity::find_by_id(token.client_id)
            .filter(mcp_clients::Column::TenantId.eq(token.tenant_id))
            .one(&self.db)
            .await
            .map_err(map_db_err)?
            .ok_or_else(|| Error::Unauthorized("MCP client not found".into()))?;

        if !client.is_active() {
            return Err(Error::Unauthorized("MCP client is inactive".into()));
        }

        let policy = mcp_policies::Entity::find_by_client(&self.db, client.id)
            .await
            .map_err(map_db_err)?;

        touch_last_used(&self.db, &client, &token).await?;

        Ok(McpRuntimeBinding {
            access_context: access_context_for_client(&client, policy.as_ref()),
            tenant_id: Some(client.tenant_id.to_string()),
            client_id: Some(client.id.to_string()),
            token_id: Some(token.id.to_string()),
        })
    }
}

#[async_trait]
impl McpAccessResolver for DbBackedMcpRuntimeBridge {
    async fn resolve_runtime_binding(
        &self,
        session: &McpSessionContext,
    ) -> anyhow::Result<McpRuntimeBinding> {
        let plaintext_token = session
            .plaintext_token
            .as_deref()
            .ok_or_else(|| anyhow::anyhow!("MCP session plaintext token is required"))?;

        self.resolve_binding_for_token(plaintext_token)
            .await
            .map_err(|error| anyhow::anyhow!(error.to_string()))
    }
}

#[async_trait]
impl McpAuditSink for DbBackedMcpRuntimeBridge {
    async fn record_tool_call(&self, event: McpToolCallAuditEvent) -> anyhow::Result<()> {
        let Some(tenant_id) = parse_uuid(event.tenant_id.as_deref()) else {
            tracing::warn!(
                tool = %event.tool_name,
                "Skipping MCP tool audit persistence because tenant_id is missing"
            );
            return Ok(());
        };

        McpManagementService::record_audit_event(
            &self.db,
            RecordMcpAuditEventInput {
                tenant_id,
                client_id: parse_uuid(event.client_id.as_deref()),
                token_id: parse_uuid(event.token_id.as_deref()),
                actor_id: event
                    .identity
                    .as_ref()
                    .map(|identity| identity.actor_id.clone()),
                actor_type: event
                    .identity
                    .as_ref()
                    .map(|identity| actor_type_slug(identity.actor_type).to_string()),
                action: "tool_call".to_string(),
                outcome: match event.outcome {
                    McpToolCallOutcome::Allowed => "allowed".to_string(),
                    McpToolCallOutcome::Denied => "denied".to_string(),
                },
                tool_name: Some(event.tool_name),
                reason: event.reason,
                correlation_id: event.correlation_id,
                metadata: serde_json::json!({
                    "transport": event.transport,
                    "session_metadata": event.metadata,
                }),
                created_by: None,
            },
        )
        .await
        .map_err(|error| anyhow::anyhow!(error.to_string()))?;

        Ok(())
    }
}

#[async_trait]
impl McpScaffoldDraftStore for DbBackedMcpRuntimeBridge {
    async fn stage_scaffold_draft(
        &self,
        context: &McpScaffoldDraftRuntimeContext,
        request: ScaffoldModuleRequest,
    ) -> anyhow::Result<StageModuleScaffoldResponse> {
        let tenant_id = tenant_id_from_runtime_context(context)?;
        let client_id = client_id_from_runtime_context(context)?;
        let created_by = actor_user_id_from_runtime_context(context);

        let draft = McpManagementService::stage_scaffold_draft(
            &self.db,
            tenant_id,
            StageMcpScaffoldDraftInput {
                client_id,
                request,
                created_by,
            },
        )
        .await
        .map_err(|error| anyhow::anyhow!(error.to_string()))?;

        let preview = draft
            .preview()
            .map_err(|error| anyhow::anyhow!(error.to_string()))?;

        Ok(StageModuleScaffoldResponse {
            draft_id: draft.id.to_string(),
            preview,
            status: draft.status_value(),
            review_required: true,
            apply_tool: rustok_mcp::TOOL_ALLOY_APPLY_MODULE_SCAFFOLD.to_string(),
            next_steps: vec![
                format!(
                    "Review the persisted draft with {} before applying it.",
                    rustok_mcp::TOOL_ALLOY_REVIEW_MODULE_SCAFFOLD
                ),
                format!(
                    "Apply the persisted draft with {} and confirm=true.",
                    rustok_mcp::TOOL_ALLOY_APPLY_MODULE_SCAFFOLD
                ),
            ],
        })
    }

    async fn review_scaffold_draft(
        &self,
        context: &McpScaffoldDraftRuntimeContext,
        request: ReviewModuleScaffoldRequest,
    ) -> anyhow::Result<ReviewModuleScaffoldResponse> {
        let tenant_id = tenant_id_from_runtime_context(context)?;
        let draft_id = Uuid::parse_str(&request.draft_id)
            .map_err(|error| anyhow::anyhow!("Invalid draft id: {error}"))?;
        let draft = McpManagementService::get_scaffold_draft(&self.db, tenant_id, draft_id)
            .await
            .map_err(|error| anyhow::anyhow!(error.to_string()))?
            .ok_or_else(|| anyhow::anyhow!("Unknown scaffold draft: {}", request.draft_id))?;

        Ok(ReviewModuleScaffoldResponse {
            draft: draft
                .to_staged_draft()
                .map_err(|error| anyhow::anyhow!(error.to_string()))?,
        })
    }

    async fn apply_scaffold_draft(
        &self,
        context: &McpScaffoldDraftRuntimeContext,
        request: ApplyModuleScaffoldRequest,
    ) -> anyhow::Result<ApplyModuleScaffoldResponse> {
        let tenant_id = tenant_id_from_runtime_context(context)?;
        let applied_by = actor_user_id_from_runtime_context(context);
        let draft_id = Uuid::parse_str(&request.draft_id)
            .map_err(|error| anyhow::anyhow!("Invalid draft id: {error}"))?;

        let (_, response) = McpManagementService::apply_scaffold_draft(
            &self.db,
            tenant_id,
            draft_id,
            ApplyMcpScaffoldDraftInput {
                workspace_root: request.workspace_root,
                confirm: request.confirm,
                applied_by,
            },
        )
        .await
        .map_err(|error| anyhow::anyhow!(error.to_string()))?;

        Ok(response)
    }
}

fn access_context_for_client(
    client: &mcp_clients::Model,
    policy: Option<&mcp_policies::Model>,
) -> McpAccessContext {
    match policy {
        Some(policy) => policy.to_access_context(client, Some(identity_for_client(client, policy))),
        None => McpAccessContext {
            identity: Some(McpIdentity {
                actor_id: client.id.to_string(),
                actor_type: client.actor_type(),
                tenant_id: Some(client.tenant_id.to_string()),
                delegated_user_id: client.delegated_user_id.map(|value| value.to_string()),
                display_name: Some(client.display_name.clone()),
                scopes: Vec::new(),
            }),
            granted_permissions: Vec::new(),
            policy: McpAccessPolicy {
                allowed_tools: Some(vec![TOOL_MCP_WHOAMI.to_string()]),
                denied_tools: Vec::new(),
            },
        },
    }
}

fn tenant_id_from_runtime_context(
    context: &McpScaffoldDraftRuntimeContext,
) -> anyhow::Result<Uuid> {
    context
        .runtime_binding
        .as_ref()
        .and_then(|binding| binding.tenant_id.as_deref())
        .and_then(|value| parse_uuid(Some(value)))
        .ok_or_else(|| {
            anyhow::anyhow!("Persisted scaffold draft flow requires tenant-bound MCP runtime")
        })
}

fn client_id_from_runtime_context(
    context: &McpScaffoldDraftRuntimeContext,
) -> anyhow::Result<Option<Uuid>> {
    match context
        .runtime_binding
        .as_ref()
        .and_then(|binding| binding.client_id.as_deref())
    {
        Some(value) => Ok(Some(Uuid::parse_str(value).map_err(|error| {
            anyhow::anyhow!("Invalid MCP client id in runtime binding: {error}")
        })?)),
        None => Ok(None),
    }
}

fn actor_user_id_from_runtime_context(context: &McpScaffoldDraftRuntimeContext) -> Option<Uuid> {
    context
        .access_context
        .as_ref()
        .and_then(actor_user_id_from_access_context)
}

fn actor_user_id_from_access_context(access: &McpAccessContext) -> Option<Uuid> {
    access.identity.as_ref().and_then(|identity| {
        identity
            .delegated_user_id
            .as_deref()
            .and_then(|value| parse_uuid(Some(value)))
            .or_else(|| match identity.actor_type {
                McpActorType::HumanUser => parse_uuid(Some(identity.actor_id.as_str())),
                _ => None,
            })
    })
}

fn identity_for_client(client: &mcp_clients::Model, policy: &mcp_policies::Model) -> McpIdentity {
    McpIdentity {
        actor_id: client.id.to_string(),
        actor_type: client.actor_type(),
        tenant_id: Some(client.tenant_id.to_string()),
        delegated_user_id: client.delegated_user_id.map(|value| value.to_string()),
        display_name: Some(client.display_name.clone()),
        scopes: policy.granted_scopes_list(),
    }
}

async fn touch_last_used(
    db: &DatabaseConnection,
    client: &mcp_clients::Model,
    token: &mcp_tokens::Model,
) -> Result<()> {
    let now = Utc::now();

    let mut client_active: mcp_clients::ActiveModel = client.clone().into();
    client_active.last_used_at = Set(Some(now.into()));
    client_active.update(db).await.map_err(map_db_err)?;

    let mut token_active: mcp_tokens::ActiveModel = token.clone().into();
    token_active.last_used_at = Set(Some(now.into()));
    token_active.update(db).await.map_err(map_db_err)?;

    Ok(())
}

fn hash_token(token: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(token.as_bytes());
    hex::encode(hasher.finalize())
}

fn parse_uuid(value: Option<&str>) -> Option<Uuid> {
    value.and_then(|value| Uuid::parse_str(value).ok())
}

fn actor_type_slug(actor_type: McpActorType) -> &'static str {
    match actor_type {
        McpActorType::HumanUser => "human_user",
        McpActorType::ServiceClient => "service_client",
        McpActorType::ModelAgent => "model_agent",
    }
}

fn map_db_err(err: sea_orm::DbErr) -> Error {
    Error::BadRequest(err.to_string())
}

#[cfg(test)]
mod tests {
    use super::{access_context_for_client, actor_user_id_from_runtime_context};
    use crate::models::{mcp_clients, mcp_policies};
    use rustok_mcp::{
        McpAccessContext, McpActorType, McpIdentity, McpScaffoldDraftRuntimeContext,
        McpSessionContext, TOOL_MCP_WHOAMI,
    };
    use sea_orm::entity::prelude::Uuid;

    #[test]
    fn missing_policy_falls_back_to_whoami_only_access() {
        let client = mcp_clients::Model {
            id: Uuid::new_v4(),
            tenant_id: Uuid::new_v4(),
            client_key: Uuid::new_v4(),
            slug: "writer-agent".to_string(),
            display_name: "Writer".to_string(),
            description: None,
            actor_type: "model_agent".to_string(),
            delegated_user_id: None,
            is_active: true,
            revoked_at: None,
            last_used_at: None,
            metadata: serde_json::json!({}),
            created_by: None,
            created_at: chrono::Utc::now().into(),
            updated_at: chrono::Utc::now().into(),
        };

        let access_context = access_context_for_client(&client, None);

        assert_eq!(
            access_context.policy.allowed_tools,
            Some(vec![TOOL_MCP_WHOAMI.to_string()])
        );
        assert!(access_context.granted_permissions.is_empty());
    }

    #[test]
    fn policy_driven_access_preserves_granted_scopes() {
        let client = mcp_clients::Model {
            id: Uuid::new_v4(),
            tenant_id: Uuid::new_v4(),
            client_key: Uuid::new_v4(),
            slug: "svc-client".to_string(),
            display_name: "Service".to_string(),
            description: None,
            actor_type: "service_client".to_string(),
            delegated_user_id: None,
            is_active: true,
            revoked_at: None,
            last_used_at: None,
            metadata: serde_json::json!({}),
            created_by: None,
            created_at: chrono::Utc::now().into(),
            updated_at: chrono::Utc::now().into(),
        };
        let policy = mcp_policies::Model {
            id: Uuid::new_v4(),
            tenant_id: client.tenant_id,
            client_id: client.id,
            allowed_tools: serde_json::json!(["list_modules"]),
            denied_tools: serde_json::json!([]),
            granted_permissions: serde_json::json!(["modules:list"]),
            granted_scopes: serde_json::json!(["modules.read"]),
            metadata: serde_json::json!({}),
            updated_by: None,
            created_at: chrono::Utc::now().into(),
            updated_at: chrono::Utc::now().into(),
        };

        let access_context = access_context_for_client(&client, Some(&policy));

        assert_eq!(
            access_context
                .identity
                .and_then(|identity| identity.scopes.into_iter().next()),
            Some("modules.read".to_string())
        );
    }

    #[test]
    fn draft_actor_user_prefers_delegated_user_id() {
        let delegated_user_id = Uuid::new_v4();
        let runtime_context = McpScaffoldDraftRuntimeContext {
            session: McpSessionContext::stdio(),
            runtime_binding: None,
            access_context: Some(McpAccessContext {
                identity: Some(McpIdentity {
                    actor_id: Uuid::new_v4().to_string(),
                    actor_type: McpActorType::ModelAgent,
                    tenant_id: None,
                    delegated_user_id: Some(delegated_user_id.to_string()),
                    display_name: None,
                    scopes: Vec::new(),
                }),
                granted_permissions: Vec::new(),
                policy: Default::default(),
            }),
        };

        assert_eq!(
            actor_user_id_from_runtime_context(&runtime_context),
            Some(delegated_user_id)
        );
    }

    #[test]
    fn draft_actor_user_uses_human_actor_id_when_it_is_uuid() {
        let actor_id = Uuid::new_v4();
        let runtime_context = McpScaffoldDraftRuntimeContext {
            session: McpSessionContext::stdio(),
            runtime_binding: None,
            access_context: Some(McpAccessContext {
                identity: Some(McpIdentity {
                    actor_id: actor_id.to_string(),
                    actor_type: McpActorType::HumanUser,
                    tenant_id: None,
                    delegated_user_id: None,
                    display_name: None,
                    scopes: Vec::new(),
                }),
                granted_permissions: Vec::new(),
                policy: Default::default(),
            }),
        };

        assert_eq!(
            actor_user_id_from_runtime_context(&runtime_context),
            Some(actor_id)
        );
    }
}
