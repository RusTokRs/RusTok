use async_trait::async_trait;

use crate::{error::AiResult, model::ToolDefinition};

#[derive(Debug, Clone)]
pub struct ToolExecutionResult {
    pub content: String,
    pub raw_payload: serde_json::Value,
}

#[async_trait]
pub trait McpClientAdapter: Send + Sync {
    async fn list_tools(&self) -> AiResult<Vec<ToolDefinition>>;
    async fn call_tool(
        &self,
        name: &str,
        arguments: serde_json::Value,
    ) -> AiResult<ToolExecutionResult>;
}
