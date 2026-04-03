use std::sync::Arc;

use chrono::Utc;

use crate::{
    error::{AiError, AiResult},
    mcp::McpClientAdapter,
    model::{
        ChatMessage, ChatMessageRole, ExecutionMode, PendingApproval, ProviderChatRequest,
        RuntimeOutcome, RuntimeRequest, ToolTrace,
    },
    policy::ToolExecutionPolicy,
    provider::ModelProvider,
};

pub struct AiRuntime {
    provider: Arc<dyn ModelProvider>,
    mcp_client: Arc<dyn McpClientAdapter>,
    tool_policy: ToolExecutionPolicy,
}

impl AiRuntime {
    pub fn new(
        provider: Arc<dyn ModelProvider>,
        mcp_client: Arc<dyn McpClientAdapter>,
        tool_policy: ToolExecutionPolicy,
    ) -> Self {
        Self {
            provider,
            mcp_client,
            tool_policy,
        }
    }

    pub async fn run(
        &self,
        config: &crate::model::AiProviderConfig,
        request: RuntimeRequest,
    ) -> AiResult<RuntimeOutcome> {
        let tools = if matches!(request.execution_mode, ExecutionMode::McpTooling) {
            self.tool_policy.apply(self.mcp_client.list_tools().await?)
        } else {
            Vec::new()
        };
        let mut messages = if let Some(system_prompt) = request.system_prompt.as_ref() {
            let mut prefixed = Vec::with_capacity(request.messages.len() + 1);
            prefixed.push(ChatMessage {
                role: ChatMessageRole::System,
                content: Some(system_prompt.clone()),
                name: None,
                tool_call_id: None,
                tool_calls: Vec::new(),
                metadata: serde_json::json!({ "system_prompt": true }),
            });
            prefixed.extend(request.messages.clone());
            prefixed
        } else {
            request.messages.clone()
        };
        let mut appended_messages = Vec::new();
        let mut traces = Vec::new();

        for _ in 0..request.max_turns.max(1) {
            let response = self
                .provider
                .complete(
                    config,
                    ProviderChatRequest {
                        model: request.model.clone(),
                        messages: messages.clone(),
                        tools: tools.clone(),
                        temperature: request.temperature,
                        max_tokens: request.max_tokens,
                    },
                )
                .await?;

            let assistant_message = response.assistant_message.clone();
            messages.push(assistant_message.clone());
            appended_messages.push(assistant_message.clone());

            if assistant_message.tool_calls.is_empty()
                || !matches!(request.execution_mode, ExecutionMode::McpTooling)
            {
                return Ok(RuntimeOutcome::Completed {
                    appended_messages,
                    traces,
                });
            }

            for tool_call in &assistant_message.tool_calls {
                let sensitive = self.tool_policy.is_tool_sensitive(&tool_call.name);
                if sensitive {
                    return Ok(RuntimeOutcome::WaitingApproval {
                        appended_messages,
                        traces,
                        pending_approval: PendingApproval {
                            tool_name: tool_call.name.clone(),
                            tool_call_id: tool_call.id.clone(),
                            input_payload: tool_call.arguments.clone(),
                            reason: format!(
                                "Tool `{}` requires operator approval before execution",
                                tool_call.name
                            ),
                        },
                    });
                }

                let started = std::time::Instant::now();
                match self
                    .mcp_client
                    .call_tool(&tool_call.name, tool_call.arguments.clone())
                    .await
                {
                    Ok(tool_result) => {
                        let tool_message = ChatMessage {
                            role: ChatMessageRole::Tool,
                            content: Some(tool_result.content.clone()),
                            name: Some(tool_call.name.clone()),
                            tool_call_id: Some(tool_call.id.clone()),
                            tool_calls: Vec::new(),
                            metadata: serde_json::json!({ "raw_payload": tool_result.raw_payload }),
                        };
                        messages.push(tool_message.clone());
                        appended_messages.push(tool_message);
                        traces.push(ToolTrace {
                            tool_name: tool_call.name.clone(),
                            input_payload: tool_call.arguments.clone(),
                            output_payload: Some(tool_result.raw_payload),
                            status: "completed".to_string(),
                            duration_ms: started.elapsed().as_millis() as i64,
                            sensitive: false,
                            error_message: None,
                            created_at: Utc::now(),
                        });
                    }
                    Err(error) => {
                        traces.push(ToolTrace {
                            tool_name: tool_call.name.clone(),
                            input_payload: tool_call.arguments.clone(),
                            output_payload: None,
                            status: "failed".to_string(),
                            duration_ms: started.elapsed().as_millis() as i64,
                            sensitive: false,
                            error_message: Some(error.to_string()),
                            created_at: Utc::now(),
                        });
                        return Ok(RuntimeOutcome::Failed {
                            appended_messages,
                            traces,
                            error_message: error.to_string(),
                        });
                    }
                }
            }
        }

        Err(AiError::Runtime(
            "maximum AI runtime turn count reached".to_string(),
        ))
    }
}
