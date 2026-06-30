use std::sync::Arc;

use async_trait::async_trait;
use thiserror::Error;
use uuid::Uuid;

use crate::McpActorType;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct McpManagementMutationContext {
    pub actor_id: Uuid,
    pub tenant_id: Uuid,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CreateMcpClientCommand {
    pub slug: String,
    pub display_name: String,
    pub description: Option<String>,
    pub actor_type: McpActorType,
    pub token_name: Option<String>,
    pub token_expires_at: Option<String>,
    pub allowed_tools: Vec<String>,
    pub denied_tools: Vec<String>,
    pub granted_permissions: Vec<String>,
    pub granted_scopes: Vec<String>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RotateMcpTokenCommand {
    pub client_id: Uuid,
    pub token_name: Option<String>,
    pub expires_at: Option<String>,
    pub revoke_existing_tokens: bool,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct UpdateMcpPolicyCommand {
    pub client_id: Uuid,
    pub allowed_tools: Vec<String>,
    pub denied_tools: Vec<String>,
    pub granted_permissions: Vec<String>,
    pub granted_scopes: Vec<String>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct McpClientMutationRecord {
    pub id: Uuid,
    pub slug: String,
    pub display_name: String,
    pub actor_type: McpActorType,
    pub is_active: bool,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct McpTokenSecretResult {
    pub client: McpClientMutationRecord,
    pub token_id: Uuid,
    pub token_name: String,
    pub token_preview: String,
    pub plaintext_token: String,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct McpPolicyMutationRecord {
    pub client_id: Uuid,
    pub allowed_tools: Vec<String>,
    pub denied_tools: Vec<String>,
    pub granted_permissions: Vec<String>,
    pub granted_scopes: Vec<String>,
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
pub trait McpManagementMutationPort: Send + Sync {
    async fn create_client(
        &self,
        context: &McpManagementMutationContext,
        command: CreateMcpClientCommand,
    ) -> Result<McpTokenSecretResult, McpManagementMutationError>;

    async fn rotate_token(
        &self,
        context: &McpManagementMutationContext,
        command: RotateMcpTokenCommand,
    ) -> Result<McpTokenSecretResult, McpManagementMutationError>;

    async fn update_policy(
        &self,
        context: &McpManagementMutationContext,
        command: UpdateMcpPolicyCommand,
    ) -> Result<McpPolicyMutationRecord, McpManagementMutationError>;

    async fn revoke_token(
        &self,
        context: &McpManagementMutationContext,
        token_id: Uuid,
        reason: Option<String>,
    ) -> Result<(), McpManagementMutationError>;

    async fn deactivate_client(
        &self,
        context: &McpManagementMutationContext,
        client_id: Uuid,
        reason: Option<String>,
    ) -> Result<(), McpManagementMutationError>;
}

#[derive(Clone)]
pub struct McpManagementMutationRuntime {
    port: Arc<dyn McpManagementMutationPort>,
}

impl McpManagementMutationRuntime {
    pub fn new(port: Arc<dyn McpManagementMutationPort>) -> Self {
        Self { port }
    }

    pub fn port(&self) -> &dyn McpManagementMutationPort {
        self.port.as_ref()
    }
}
