use std::sync::Arc;

use async_trait::async_trait;
use chrono::{DateTime, Utc};
use thiserror::Error;
use uuid::Uuid;

use crate::{McpActorType, ScaffoldModuleRequest};

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct McpManagementContext {
    pub actor_id: Uuid,
    pub tenant_id: Uuid,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CreateMcpClientCommand {
    pub slug: String,
    pub display_name: String,
    pub description: Option<String>,
    pub actor_type: McpActorType,
    pub delegated_user_id: Option<Uuid>,
    pub token_name: Option<String>,
    pub token_expires_at: Option<String>,
    pub allowed_tools: Vec<String>,
    pub denied_tools: Vec<String>,
    pub granted_permissions: Vec<String>,
    pub granted_scopes: Vec<String>,
    pub metadata: serde_json::Value,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RotateMcpTokenCommand {
    pub client_id: Uuid,
    pub token_name: Option<String>,
    pub expires_at: Option<String>,
    pub revoke_existing_tokens: bool,
    pub metadata: serde_json::Value,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct UpdateMcpPolicyCommand {
    pub client_id: Uuid,
    pub allowed_tools: Vec<String>,
    pub denied_tools: Vec<String>,
    pub granted_permissions: Vec<String>,
    pub granted_scopes: Vec<String>,
    pub metadata: serde_json::Value,
}

#[derive(Clone, Debug)]
pub struct StageMcpScaffoldDraftCommand {
    pub client_id: Option<Uuid>,
    pub request: ScaffoldModuleRequest,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ApplyMcpScaffoldDraftCommand {
    pub draft_id: Uuid,
    pub workspace_root: String,
    pub confirm: bool,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct McpClientRecord {
    pub id: Uuid,
    pub tenant_id: Uuid,
    pub client_key: Uuid,
    pub slug: String,
    pub display_name: String,
    pub description: Option<String>,
    pub actor_type: McpActorType,
    pub delegated_user_id: Option<Uuid>,
    pub is_active: bool,
    pub revoked_at: Option<DateTime<Utc>>,
    pub last_used_at: Option<DateTime<Utc>>,
    pub metadata: serde_json::Value,
    pub created_by: Option<Uuid>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct McpTokenRecord {
    pub id: Uuid,
    pub client_id: Uuid,
    pub token_name: String,
    pub token_preview: String,
    pub is_active: bool,
    pub last_used_at: Option<DateTime<Utc>>,
    pub expires_at: Option<DateTime<Utc>>,
    pub revoked_at: Option<DateTime<Utc>>,
    pub metadata: serde_json::Value,
    pub created_at: DateTime<Utc>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct McpTokenSecretResult {
    pub client: McpClientRecord,
    pub policy: Option<McpPolicyRecord>,
    pub token: McpTokenRecord,
    pub plaintext_token: String,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct McpPolicyRecord {
    pub id: Uuid,
    pub client_id: Uuid,
    pub allowed_tools: Vec<String>,
    pub denied_tools: Vec<String>,
    pub granted_permissions: Vec<String>,
    pub granted_scopes: Vec<String>,
    pub metadata: serde_json::Value,
    pub updated_by: Option<Uuid>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct McpClientDetailsRecord {
    pub client: McpClientRecord,
    pub policy: Option<McpPolicyRecord>,
    pub tokens: Vec<McpTokenRecord>,
    pub effective_access_context: Option<crate::McpAccessContext>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct McpAuditEventRecord {
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
    pub created_by: Option<Uuid>,
    pub created_at: DateTime<Utc>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct McpScaffoldDraftRecord {
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

#[derive(Clone, Debug, Error, PartialEq, Eq)]
pub enum McpManagementMutationError {
    #[error("invalid MCP management mutation: {0}")]
    Validation(String),
    #[error("MCP management resource not found: {0}")]
    NotFound(String),
    #[error("MCP management mutation conflict: {0}")]
    Conflict(String),
    #[error("MCP management mutation failed: {0}")]
    Internal(String),
}

#[async_trait]
pub trait McpManagementPort: Send + Sync {
    async fn list_clients(
        &self,
        context: &McpManagementContext,
        limit: Option<u64>,
    ) -> Result<Vec<McpClientRecord>, McpManagementMutationError>;

    async fn get_client(
        &self,
        context: &McpManagementContext,
        client_id: Uuid,
    ) -> Result<Option<McpClientDetailsRecord>, McpManagementMutationError>;

    async fn list_audit_events(
        &self,
        context: &McpManagementContext,
        client_id: Option<Uuid>,
        outcome: Option<String>,
        limit: Option<u64>,
    ) -> Result<Vec<McpAuditEventRecord>, McpManagementMutationError>;

    async fn list_scaffold_drafts(
        &self,
        context: &McpManagementContext,
        limit: Option<u64>,
    ) -> Result<Vec<McpScaffoldDraftRecord>, McpManagementMutationError>;

    async fn get_scaffold_draft(
        &self,
        context: &McpManagementContext,
        draft_id: Uuid,
    ) -> Result<Option<McpScaffoldDraftRecord>, McpManagementMutationError>;

    async fn create_client(
        &self,
        context: &McpManagementContext,
        command: CreateMcpClientCommand,
    ) -> Result<McpTokenSecretResult, McpManagementMutationError>;

    async fn rotate_token(
        &self,
        context: &McpManagementContext,
        command: RotateMcpTokenCommand,
    ) -> Result<McpTokenSecretResult, McpManagementMutationError>;

    async fn update_policy(
        &self,
        context: &McpManagementContext,
        command: UpdateMcpPolicyCommand,
    ) -> Result<McpPolicyRecord, McpManagementMutationError>;

    async fn revoke_token(
        &self,
        context: &McpManagementContext,
        token_id: Uuid,
        reason: Option<String>,
    ) -> Result<(), McpManagementMutationError>;

    async fn deactivate_client(
        &self,
        context: &McpManagementContext,
        client_id: Uuid,
        reason: Option<String>,
    ) -> Result<(), McpManagementMutationError>;

    async fn stage_scaffold_draft(
        &self,
        context: &McpManagementContext,
        command: StageMcpScaffoldDraftCommand,
    ) -> Result<McpScaffoldDraftRecord, McpManagementMutationError>;

    async fn apply_scaffold_draft(
        &self,
        context: &McpManagementContext,
        command: ApplyMcpScaffoldDraftCommand,
    ) -> Result<McpScaffoldDraftRecord, McpManagementMutationError>;
}

#[derive(Clone)]
pub struct McpManagementRuntime {
    port: Arc<dyn McpManagementPort>,
}

impl McpManagementRuntime {
    pub fn new(port: Arc<dyn McpManagementPort>) -> Self {
        Self { port }
    }

    pub fn port(&self) -> &dyn McpManagementPort {
        self.port.as_ref()
    }
}
