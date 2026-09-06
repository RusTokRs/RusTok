use std::sync::Arc;

use anyhow::Result;
use async_trait::async_trait;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::{
    ApplyModuleScaffoldRequest, ApplyModuleScaffoldResponse, McpAccessContext, McpIdentity,
    ReviewModuleScaffoldRequest, ReviewModuleScaffoldResponse, ScaffoldModuleRequest,
    StageModuleScaffoldResponse,
};

fn default_transport() -> String {
    "stdio".to_string()
}

fn default_metadata() -> serde_json::Value {
    serde_json::json!({})
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct McpSessionContext {
    #[serde(default = "default_transport")]
    pub transport: String,
    pub plaintext_token: Option<String>,
    pub correlation_id: Option<String>,
    #[serde(default = "default_metadata")]
    pub metadata: serde_json::Value,
}

impl Default for McpSessionContext {
    fn default() -> Self {
        Self {
            transport: default_transport(),
            plaintext_token: None,
            correlation_id: None,
            metadata: default_metadata(),
        }
    }
}

impl McpSessionContext {
    pub fn stdio() -> Self {
        Self::default()
    }

    pub fn with_transport(mut self, transport: impl Into<String>) -> Self {
        self.transport = transport.into();
        self
    }

    pub fn with_plaintext_token(mut self, plaintext_token: impl Into<String>) -> Self {
        self.plaintext_token = Some(plaintext_token.into());
        self
    }

    pub fn with_correlation_id(mut self, correlation_id: impl Into<String>) -> Self {
        self.correlation_id = Some(correlation_id.into());
        self
    }

    pub fn with_metadata(mut self, metadata: serde_json::Value) -> Self {
        self.metadata = metadata;
        self
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct McpRuntimeBinding {
    pub access_context: McpAccessContext,
    pub tenant_id: Option<String>,
    pub client_id: Option<String>,
    pub token_id: Option<String>,
}

impl McpRuntimeBinding {
    pub fn from_access_context(access_context: McpAccessContext) -> Self {
        Self {
            access_context,
            tenant_id: None,
            client_id: None,
            token_id: None,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct McpScaffoldDraftRuntimeContext {
    pub session: McpSessionContext,
    pub runtime_binding: Option<McpRuntimeBinding>,
    pub access_context: Option<McpAccessContext>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum McpToolCallOutcome {
    Allowed,
    Denied,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct McpToolCallAuditEvent {
    pub transport: String,
    pub correlation_id: Option<String>,
    pub tenant_id: Option<String>,
    pub client_id: Option<String>,
    pub token_id: Option<String>,
    pub identity: Option<McpIdentity>,
    pub tool_name: String,
    pub outcome: McpToolCallOutcome,
    pub reason: Option<String>,
    #[serde(default = "default_metadata")]
    pub metadata: serde_json::Value,
}

impl McpToolCallAuditEvent {
    pub fn allowed(
        session: &McpSessionContext,
        access_context: Option<&McpAccessContext>,
        binding: Option<&McpRuntimeBinding>,
        tool_name: impl Into<String>,
    ) -> Self {
        Self::new(
            session,
            access_context,
            binding,
            tool_name.into(),
            McpToolCallOutcome::Allowed,
            None,
        )
    }

    pub fn denied(
        session: &McpSessionContext,
        access_context: Option<&McpAccessContext>,
        binding: Option<&McpRuntimeBinding>,
        tool_name: impl Into<String>,
        reason: Option<String>,
    ) -> Self {
        Self::new(
            session,
            access_context,
            binding,
            tool_name.into(),
            McpToolCallOutcome::Denied,
            reason,
        )
    }

    fn new(
        session: &McpSessionContext,
        access_context: Option<&McpAccessContext>,
        binding: Option<&McpRuntimeBinding>,
        tool_name: String,
        outcome: McpToolCallOutcome,
        reason: Option<String>,
    ) -> Self {
        Self {
            transport: session.transport.clone(),
            correlation_id: session.correlation_id.clone(),
            tenant_id: binding.and_then(|value| value.tenant_id.clone()),
            client_id: binding.and_then(|value| value.client_id.clone()),
            token_id: binding.and_then(|value| value.token_id.clone()),
            identity: access_context.and_then(|value| value.identity.clone()),
            tool_name,
            outcome,
            reason,
            metadata: session.metadata.clone(),
        }
    }
}

#[async_trait]
pub trait McpAccessResolver: Send + Sync {
    async fn resolve_runtime_binding(
        &self,
        session: &McpSessionContext,
    ) -> Result<McpRuntimeBinding>;
}

#[async_trait]
pub trait McpAuditSink: Send + Sync {
    async fn record_tool_call(&self, event: McpToolCallAuditEvent) -> Result<()>;
}

#[async_trait]
pub trait McpScaffoldDraftStore: Send + Sync {
    async fn stage_scaffold_draft(
        &self,
        context: &McpScaffoldDraftRuntimeContext,
        request: ScaffoldModuleRequest,
    ) -> Result<StageModuleScaffoldResponse>;

    async fn review_scaffold_draft(
        &self,
        context: &McpScaffoldDraftRuntimeContext,
        request: ReviewModuleScaffoldRequest,
    ) -> Result<ReviewModuleScaffoldResponse>;

    async fn apply_scaffold_draft(
        &self,
        context: &McpScaffoldDraftRuntimeContext,
        request: ApplyModuleScaffoldRequest,
    ) -> Result<ApplyModuleScaffoldResponse>;
}

pub type SharedMcpAccessResolver = Arc<dyn McpAccessResolver>;
pub type SharedMcpAuditSink = Arc<dyn McpAuditSink>;
pub type SharedMcpScaffoldDraftStore = Arc<dyn McpScaffoldDraftStore>;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn session_context_defaults_to_stdio() {
        let session = McpSessionContext::default();

        assert_eq!(session.transport, "stdio");
        assert!(session.plaintext_token.is_none());
    }
}
