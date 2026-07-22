use std::{collections::BTreeSet, sync::Arc};

use chrono::Utc;
use rig::{
    OneOrMany,
    agent::{AgentRun, AgentRunStep, InvalidToolCallHookAction, ModelTurn, ModelTurnOutcome},
    completion::Usage,
    message::UserContent,
};
use tokio::sync::watch;

use crate::{
    engine::{InferenceEngine, assistant_choice, map_message, map_rig_message},
    error::{AiError, AiResult},
    mcp::McpClientAdapter,
    model::{
        ChatMessage, ChatMessageRole, ExecutionMode, PendingApproval, ProviderChatRequest,
        ProviderStreamEmitter, RuntimeOutcome, RuntimeRequest, ToolTrace,
    },
    policy::ToolExecutionPolicy,
};

pub struct RigAgentDriver {
    provider: Arc<dyn InferenceEngine>,
    mcp_client: Arc<dyn McpClientAdapter>,
    tool_policy: ToolExecutionPolicy,
}

impl RigAgentDriver {
    pub fn new(
        provider: Arc<dyn InferenceEngine>,
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
        stream_emitter: Option<ProviderStreamEmitter>,
        mut cancellation: Option<watch::Receiver<()>>,
    ) -> AiResult<RuntimeOutcome> {
        let tools = if matches!(request.execution_mode, ExecutionMode::McpTooling) {
            self.tool_policy.apply(self.mcp_client.list_tools().await?)
        } else {
            Vec::new()
        };
        let messages = localized_messages(&request);
        let mut rig_messages = messages
            .iter()
            .cloned()
            .map(map_message)
            .collect::<AiResult<Vec<_>>>()?;
        let prompt = rig_messages.pop().ok_or_else(|| {
            AiError::Validation("AI runtime requires at least one prompt message".to_string())
        })?;
        let mut run = AgentRun::new(prompt)
            .with_history(rig_messages)
            .max_turns(request.max_turns.max(1))
            .max_invalid_tool_call_retries(1);
        let tool_names = tools
            .iter()
            .map(|tool| tool.name.clone())
            .collect::<BTreeSet<_>>();
        let mut appended_messages = Vec::new();
        let mut traces = Vec::new();

        loop {
            if cancellation
                .as_ref()
                .is_some_and(|receiver| receiver.has_changed().unwrap_or(false))
            {
                return Err(AiError::Runtime("AI run cancelled".to_string()));
            }
            match run
                .next_step()
                .map_err(|error| AiError::Runtime(error.to_string()))?
            {
                AgentRunStep::CallModel {
                    prompt, history, ..
                } => {
                    let mut provider_messages =
                        history.into_iter().map(map_rig_message).collect::<Vec<_>>();
                    provider_messages.push(map_rig_message(prompt));
                    let provider_request = ProviderChatRequest {
                        model: request.model.clone(),
                        messages: provider_messages,
                        tools: tools.clone(),
                        temperature: request.temperature,
                        max_tokens: request.max_tokens,
                        locale: request.locale.clone(),
                    };
                    let response = if let Some(receiver) = cancellation.as_mut() {
                        tokio::select! {
                            result = self.provider.complete_stream(config, provider_request, stream_emitter.clone()) => result?,
                            changed = receiver.changed() => {
                                let _ = changed;
                                return Err(AiError::Runtime("AI run cancelled".to_string()));
                            }
                        }
                    } else {
                        self.provider
                            .complete_stream(config, provider_request, stream_emitter.clone())
                            .await?
                    };
                    let assistant = response.assistant_message;
                    let choice = assistant_choice(&assistant)?;
                    appended_messages.push(assistant);
                    let turn = ModelTurn::new(
                        None,
                        choice,
                        Usage::new(),
                        tool_names.clone(),
                        tool_names.clone(),
                    );
                    match run
                        .model_response(turn)
                        .map_err(|error| AiError::Runtime(error.to_string()))?
                    {
                        ModelTurnOutcome::Continue { .. } | ModelTurnOutcome::TurnRetried => {}
                        ModelTurnOutcome::NeedsResolution(_) => {
                            let invalid = run.pending_invalid_tool_call().ok_or_else(|| {
                                AiError::Runtime(
                                    "Rig requested invalid tool resolution without context"
                                        .to_string(),
                                )
                            })?;
                            let reason = format!(
                                "Tool `{}` is unavailable for this run and was not executed",
                                invalid.tool_name
                            );
                            let outcome = run
                                .resolve_invalid_tool_call(InvalidToolCallHookAction::skip(
                                    reason.clone(),
                                ))
                                .map_err(|error| AiError::Runtime(error.to_string()))?;
                            appended_messages.push(ChatMessage {
                                role: ChatMessageRole::Tool,
                                content: Some(reason.clone()),
                                name: Some(invalid.tool_name.clone()),
                                tool_call_id: invalid.tool_call_id,
                                tool_calls: Vec::new(),
                                metadata: serde_json::json!({
                                    "engine": "rig_0_39",
                                    "skipped": true,
                                    "reason": "unknown_or_denied_tool"
                                }),
                            });
                            traces.push(ToolTrace {
                                tool_name: invalid.tool_name,
                                input_payload: invalid
                                    .args
                                    .and_then(|value| serde_json::from_str(&value).ok())
                                    .unwrap_or(serde_json::Value::Null),
                                output_payload: Some(serde_json::json!({"reason": reason})),
                                status: "skipped".to_string(),
                                duration_ms: 0,
                                sensitive: false,
                                error_message: None,
                                created_at: Utc::now(),
                            });
                            if matches!(outcome, ModelTurnOutcome::NeedsResolution(_)) {
                                return Ok(RuntimeOutcome::Failed {
                                    appended_messages,
                                    traces,
                                    error_message: "Rig could not recover the invalid tool call"
                                        .to_string(),
                                });
                            }
                        }
                    }
                }
                AgentRunStep::CallTools { calls } => {
                    let pending_approvals = calls
                        .iter()
                        .filter(|call| {
                            self.tool_policy
                                .is_tool_allowed(&call.tool_call.function.name)
                                && self
                                    .tool_policy
                                    .is_tool_sensitive(&call.tool_call.function.name)
                        })
                        .map(|call| PendingApproval {
                            tool_name: call.tool_call.function.name.clone(),
                            tool_call_id: call.tool_call.id.clone(),
                            input_payload: call.tool_call.function.arguments.clone(),
                            reason: format!(
                                "Tool `{}` requires operator approval before execution",
                                call.tool_call.function.name
                            ),
                        })
                        .collect::<Vec<_>>();
                    let pending_sensitive_call_ids = pending_approvals
                        .iter()
                        .map(|approval| approval.tool_call_id.as_str())
                        .collect::<BTreeSet<_>>();

                    let mut results = Vec::with_capacity(calls.len());
                    for call in calls {
                        let name = call.tool_call.function.name.clone();
                        let arguments = call.tool_call.function.arguments.clone();
                        if pending_sensitive_call_ids.contains(call.tool_call.id.as_str()) {
                            continue;
                        }
                        let started = std::time::Instant::now();
                        if !self.tool_policy.is_tool_allowed(&name) {
                            let reason = format!(
                                "Tool `{name}` is denied by the execution policy and was not executed"
                            );
                            appended_messages.push(ChatMessage {
                                role: ChatMessageRole::Tool,
                                content: Some(reason.clone()),
                                name: Some(name.clone()),
                                tool_call_id: Some(call.tool_call.id.clone()),
                                tool_calls: Vec::new(),
                                metadata: serde_json::json!({
                                    "engine": "rig_0_39",
                                    "skipped": true,
                                    "reason": "tool_execution_policy"
                                }),
                            });
                            traces.push(ToolTrace {
                                tool_name: name,
                                input_payload: arguments,
                                output_payload: Some(serde_json::json!({"reason": reason})),
                                status: "skipped".to_string(),
                                duration_ms: started.elapsed().as_millis() as i64,
                                sensitive: false,
                                error_message: None,
                                created_at: Utc::now(),
                            });
                            results.push(UserContent::tool_result(
                                call.tool_call.id,
                                OneOrMany::one(reason.into()),
                            ));
                            continue;
                        }
                        match self.mcp_client.call_tool(&name, arguments.clone()).await {
                            Ok(result) => {
                                let tool_message = ChatMessage {
                                    role: ChatMessageRole::Tool,
                                    content: Some(result.content.clone()),
                                    name: Some(name.clone()),
                                    tool_call_id: Some(call.tool_call.id.clone()),
                                    tool_calls: Vec::new(),
                                    metadata: serde_json::json!({
                                        "raw_payload": result.raw_payload,
                                        "engine": "rig_0_39"
                                    }),
                                };
                                appended_messages.push(tool_message);
                                traces.push(ToolTrace {
                                    tool_name: name,
                                    input_payload: arguments,
                                    output_payload: Some(result.raw_payload),
                                    status: "completed".to_string(),
                                    duration_ms: started.elapsed().as_millis() as i64,
                                    sensitive: false,
                                    error_message: None,
                                    created_at: Utc::now(),
                                });
                                results.push(UserContent::tool_result(
                                    call.tool_call.id,
                                    OneOrMany::one(result.content.into()),
                                ));
                            }
                            Err(error) => {
                                traces.push(ToolTrace {
                                    tool_name: name,
                                    input_payload: arguments,
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
                    if !pending_approvals.is_empty() {
                        return Ok(RuntimeOutcome::WaitingApproval {
                            appended_messages,
                            traces,
                            pending_approvals,
                        });
                    }
                    run.tool_results(results)
                        .map_err(|error| AiError::Runtime(error.to_string()))?;
                }
                AgentRunStep::Done(_) => {
                    return Ok(RuntimeOutcome::Completed {
                        appended_messages,
                        traces,
                    });
                }
            }
        }
    }
}

fn localized_messages(request: &RuntimeRequest) -> Vec<ChatMessage> {
    let mut messages = request.messages.clone();
    let system_prompt = match (&request.system_prompt, &request.locale) {
        (Some(prompt), Some(locale)) => Some(format!(
            "{prompt}\n\nRespond in locale `{locale}` unless the task explicitly requires another language."
        )),
        (Some(prompt), None) => Some(prompt.clone()),
        (None, Some(locale)) => Some(format!(
            "Respond in locale `{locale}` unless the task explicitly requires another language."
        )),
        (None, None) => None,
    };
    if let Some(content) = system_prompt {
        messages.insert(
            0,
            ChatMessage {
                role: ChatMessageRole::System,
                content: Some(content),
                name: None,
                tool_call_id: None,
                tool_calls: Vec::new(),
                metadata: serde_json::json!({
                    "system_prompt": true,
                    "locale": request.locale,
                }),
            },
        );
    }
    messages
}

#[cfg(test)]
mod tests {
    use std::{
        collections::{BTreeMap, VecDeque},
        sync::Arc,
    };

    use async_trait::async_trait;
    use tokio::sync::Mutex;
    use uuid::Uuid;

    use super::RigAgentDriver;
    use crate::{
        AiResult, ProviderSlug, ToolExecutionPolicy,
        engine::InferenceEngine,
        mcp::{McpClientAdapter, ToolExecutionResult},
        model::{
            AiProviderConfig, ChatMessage, ChatMessageRole, ExecutionMode, ProviderChatRequest,
            ProviderChatResponse, ProviderStructuredRequest, ProviderTestResult, RuntimeOutcome,
            RuntimeRequest, ToolCall, ToolDefinition,
        },
    };

    struct ScriptedEngine {
        responses: Mutex<VecDeque<ProviderChatResponse>>,
    }

    #[async_trait]
    impl InferenceEngine for ScriptedEngine {
        async fn test_connection(
            &self,
            _config: &AiProviderConfig,
        ) -> AiResult<ProviderTestResult> {
            unreachable!("agent test never probes connectivity")
        }

        async fn complete(
            &self,
            _config: &AiProviderConfig,
            _request: ProviderChatRequest,
        ) -> AiResult<ProviderChatResponse> {
            unreachable!("agent driver uses streaming completion")
        }

        async fn complete_stream(
            &self,
            _config: &AiProviderConfig,
            _request: ProviderChatRequest,
            _emitter: Option<crate::ProviderStreamEmitter>,
        ) -> AiResult<ProviderChatResponse> {
            Ok(self
                .responses
                .lock()
                .await
                .pop_front()
                .expect("scripted model response"))
        }

        async fn complete_structured(
            &self,
            _request: ProviderStructuredRequest,
        ) -> AiResult<serde_json::Value> {
            unreachable!("agent test never requests structured output")
        }
    }

    #[derive(Default)]
    struct RecordingMcp {
        calls: Mutex<Vec<String>>,
    }

    struct FailingMcp;

    #[async_trait]
    impl McpClientAdapter for RecordingMcp {
        async fn list_tools(&self) -> AiResult<Vec<ToolDefinition>> {
            Ok(vec![
                ToolDefinition {
                    name: "publish".to_string(),
                    description: "Publish a draft".to_string(),
                    input_schema: serde_json::json!({"type": "object"}),
                    sensitive: false,
                },
                ToolDefinition {
                    name: "notify".to_string(),
                    description: "Notify an operator".to_string(),
                    input_schema: serde_json::json!({"type": "object"}),
                    sensitive: false,
                },
            ])
        }

        async fn call_tool(
            &self,
            name: &str,
            _arguments: serde_json::Value,
        ) -> AiResult<ToolExecutionResult> {
            self.calls.lock().await.push(name.to_string());
            Ok(ToolExecutionResult {
                content: "published".to_string(),
                raw_payload: serde_json::json!({"published": true}),
            })
        }
    }

    #[async_trait]
    impl McpClientAdapter for FailingMcp {
        async fn list_tools(&self) -> AiResult<Vec<ToolDefinition>> {
            Ok(vec![ToolDefinition {
                name: "publish".to_string(),
                description: "Publish a draft".to_string(),
                input_schema: serde_json::json!({"type": "object"}),
                sensitive: false,
            }])
        }

        async fn call_tool(
            &self,
            _name: &str,
            _arguments: serde_json::Value,
        ) -> AiResult<ToolExecutionResult> {
            Err(crate::AiError::Mcp("tool backend unavailable".to_string()))
        }
    }

    fn config() -> AiProviderConfig {
        AiProviderConfig {
            tenant_id: Uuid::nil(),
            provider_slug: ProviderSlug::new("openai_compatible").unwrap(),
            target_auth: crate::ProviderTargetAuth::SecretRefs,
            model: "test-model".to_string(),
            settings: BTreeMap::new(),
            credential_refs: BTreeMap::new(),
            temperature: None,
            max_tokens: None,
            capabilities: Vec::new(),
            usage_policy: Default::default(),
        }
    }

    fn request() -> RuntimeRequest {
        RuntimeRequest {
            model: "test-model".to_string(),
            messages: vec![ChatMessage {
                role: ChatMessageRole::User,
                content: Some("publish it".to_string()),
                name: None,
                tool_call_id: None,
                tool_calls: Vec::new(),
                metadata: serde_json::Value::Null,
            }],
            temperature: None,
            max_tokens: None,
            max_turns: 3,
            execution_mode: ExecutionMode::McpTooling,
            system_prompt: None,
            locale: None,
        }
    }

    #[tokio::test]
    async fn sensitive_tool_waits_for_approval_without_execution() {
        let engine = Arc::new(ScriptedEngine {
            responses: Mutex::new(VecDeque::from([ProviderChatResponse {
                assistant_message: ChatMessage {
                    role: ChatMessageRole::Assistant,
                    content: None,
                    name: None,
                    tool_call_id: None,
                    tool_calls: vec![ToolCall {
                        id: "call-1".to_string(),
                        name: "publish".to_string(),
                        arguments: serde_json::json!({"id": "draft-1"}),
                    }],
                    metadata: serde_json::Value::Null,
                },
                finish_reason: Some("tool_calls".to_string()),
                raw_payload: serde_json::Value::Null,
            }])),
        });
        let mcp = Arc::new(RecordingMcp::default());
        let driver = RigAgentDriver::new(
            engine,
            mcp.clone(),
            ToolExecutionPolicy::new(None, Vec::new(), vec!["publish".to_string()]),
        );

        let outcome = driver.run(&config(), request(), None, None).await.unwrap();
        let RuntimeOutcome::WaitingApproval {
            pending_approvals, ..
        } = outcome
        else {
            panic!("sensitive tool must stop for approval")
        };
        assert_eq!(pending_approvals.len(), 1);
        assert_eq!(pending_approvals[0].tool_name, "publish");
        assert!(mcp.calls.lock().await.is_empty());
    }

    #[tokio::test]
    async fn sensitive_tool_defers_only_itself_in_a_multi_tool_turn() {
        let engine = Arc::new(ScriptedEngine {
            responses: Mutex::new(VecDeque::from([ProviderChatResponse {
                assistant_message: ChatMessage {
                    role: ChatMessageRole::Assistant,
                    content: None,
                    name: None,
                    tool_call_id: None,
                    tool_calls: vec![
                        ToolCall {
                            id: "publish-1".to_string(),
                            name: "publish".to_string(),
                            arguments: serde_json::json!({"id": "draft-1"}),
                        },
                        ToolCall {
                            id: "notify-1".to_string(),
                            name: "notify".to_string(),
                            arguments: serde_json::json!({"channel": "operator"}),
                        },
                    ],
                    metadata: serde_json::Value::Null,
                },
                finish_reason: Some("tool_calls".to_string()),
                raw_payload: serde_json::Value::Null,
            }])),
        });
        let mcp = Arc::new(RecordingMcp::default());
        let driver = RigAgentDriver::new(
            engine,
            mcp.clone(),
            ToolExecutionPolicy::new(None, Vec::new(), vec!["publish".to_string()]),
        );

        let outcome = driver.run(&config(), request(), None, None).await.unwrap();
        let RuntimeOutcome::WaitingApproval {
            pending_approvals,
            traces,
            ..
        } = outcome
        else {
            panic!("the sensitive tool must wait for approval")
        };
        assert_eq!(pending_approvals.len(), 1);
        assert_eq!(pending_approvals[0].tool_name, "publish");
        assert_eq!(traces.len(), 1);
        assert_eq!(traces[0].tool_name, "notify");
        assert_eq!(mcp.calls.lock().await.as_slice(), ["notify".to_string()]);
    }

    #[tokio::test]
    async fn multiple_sensitive_tools_are_returned_as_one_approval_batch() {
        let engine = Arc::new(ScriptedEngine {
            responses: Mutex::new(VecDeque::from([ProviderChatResponse {
                assistant_message: ChatMessage {
                    role: ChatMessageRole::Assistant,
                    content: None,
                    name: None,
                    tool_call_id: None,
                    tool_calls: vec![
                        ToolCall {
                            id: "publish-1".to_string(),
                            name: "publish".to_string(),
                            arguments: serde_json::json!({"id": "draft-1"}),
                        },
                        ToolCall {
                            id: "notify-1".to_string(),
                            name: "notify".to_string(),
                            arguments: serde_json::json!({"channel": "operator"}),
                        },
                    ],
                    metadata: serde_json::Value::Null,
                },
                finish_reason: Some("tool_calls".to_string()),
                raw_payload: serde_json::Value::Null,
            }])),
        });
        let mcp = Arc::new(RecordingMcp::default());
        let driver = RigAgentDriver::new(
            engine,
            mcp.clone(),
            ToolExecutionPolicy::new(
                None,
                Vec::new(),
                vec!["publish".to_string(), "notify".to_string()],
            ),
        );

        let outcome = driver.run(&config(), request(), None, None).await.unwrap();
        let RuntimeOutcome::WaitingApproval {
            pending_approvals,
            traces,
            ..
        } = outcome
        else {
            panic!("sensitive tools must wait as one batch")
        };
        assert_eq!(pending_approvals.len(), 2);
        assert!(traces.is_empty());
        assert!(mcp.calls.lock().await.is_empty());
    }

    #[tokio::test]
    async fn denied_tool_is_rejected_without_execution() {
        let engine = Arc::new(ScriptedEngine {
            responses: Mutex::new(VecDeque::from([
                ProviderChatResponse {
                    assistant_message: ChatMessage {
                        role: ChatMessageRole::Assistant,
                        content: None,
                        name: None,
                        tool_call_id: None,
                        tool_calls: vec![ToolCall {
                            id: "call-1".to_string(),
                            name: "publish".to_string(),
                            arguments: serde_json::json!({"id": "draft-1"}),
                        }],
                        metadata: serde_json::Value::Null,
                    },
                    finish_reason: Some("tool_calls".to_string()),
                    raw_payload: serde_json::Value::Null,
                },
                ProviderChatResponse {
                    assistant_message: ChatMessage {
                        role: ChatMessageRole::Assistant,
                        content: Some("I cannot publish this draft.".to_string()),
                        name: None,
                        tool_call_id: None,
                        tool_calls: Vec::new(),
                        metadata: serde_json::Value::Null,
                    },
                    finish_reason: Some("stop".to_string()),
                    raw_payload: serde_json::Value::Null,
                },
            ])),
        });
        let mcp = Arc::new(RecordingMcp::default());
        let driver = RigAgentDriver::new(
            engine,
            mcp.clone(),
            ToolExecutionPolicy::new(None, vec!["publish".to_string()], Vec::new()),
        );

        let outcome = driver.run(&config(), request(), None, None).await.unwrap();
        let RuntimeOutcome::Completed {
            appended_messages,
            traces,
        } = outcome
        else {
            panic!("unknown tool should recover without an MCP call")
        };
        assert!(appended_messages.iter().any(|message| {
            message.role == ChatMessageRole::Tool
                && message.metadata["reason"] == "unknown_or_denied_tool"
        }));
        assert!(traces.iter().any(|trace| trace.status == "skipped"));
        assert!(mcp.calls.lock().await.is_empty());
    }

    #[tokio::test]
    async fn allowed_multi_tool_turn_executes_every_call_once() {
        let engine = Arc::new(ScriptedEngine {
            responses: Mutex::new(VecDeque::from([
                ProviderChatResponse {
                    assistant_message: ChatMessage {
                        role: ChatMessageRole::Assistant,
                        content: None,
                        name: None,
                        tool_call_id: None,
                        tool_calls: vec![
                            ToolCall {
                                id: "publish-1".to_string(),
                                name: "publish".to_string(),
                                arguments: serde_json::json!({"id": "draft-1"}),
                            },
                            ToolCall {
                                id: "notify-1".to_string(),
                                name: "notify".to_string(),
                                arguments: serde_json::json!({"channel": "operator"}),
                            },
                        ],
                        metadata: serde_json::Value::Null,
                    },
                    finish_reason: Some("tool_calls".to_string()),
                    raw_payload: serde_json::Value::Null,
                },
                ProviderChatResponse {
                    assistant_message: ChatMessage {
                        role: ChatMessageRole::Assistant,
                        content: Some("Published and notified the operator.".to_string()),
                        name: None,
                        tool_call_id: None,
                        tool_calls: Vec::new(),
                        metadata: serde_json::Value::Null,
                    },
                    finish_reason: Some("stop".to_string()),
                    raw_payload: serde_json::Value::Null,
                },
            ])),
        });
        let mcp = Arc::new(RecordingMcp::default());
        let driver = RigAgentDriver::new(
            engine,
            mcp.clone(),
            ToolExecutionPolicy::new(None, Vec::new(), Vec::new()),
        );

        let outcome = driver.run(&config(), request(), None, None).await.unwrap();
        let RuntimeOutcome::Completed { traces, .. } = outcome else {
            panic!("allowed multi-tool turn should complete")
        };
        assert_eq!(traces.len(), 2);
        let calls = mcp.calls.lock().await.clone();
        assert_eq!(calls, vec!["publish".to_string(), "notify".to_string()]);
    }

    #[tokio::test]
    async fn tool_failure_is_persistable_as_a_failed_trace() {
        let engine = Arc::new(ScriptedEngine {
            responses: Mutex::new(VecDeque::from([ProviderChatResponse {
                assistant_message: ChatMessage {
                    role: ChatMessageRole::Assistant,
                    content: None,
                    name: None,
                    tool_call_id: None,
                    tool_calls: vec![ToolCall {
                        id: "publish-1".to_string(),
                        name: "publish".to_string(),
                        arguments: serde_json::json!({ "id": "draft-1" }),
                    }],
                    metadata: serde_json::Value::Null,
                },
                finish_reason: Some("tool_calls".to_string()),
                raw_payload: serde_json::Value::Null,
            }])),
        });
        let driver = RigAgentDriver::new(
            engine,
            Arc::new(FailingMcp),
            ToolExecutionPolicy::new(None, Vec::new(), Vec::new()),
        );

        let outcome = driver.run(&config(), request(), None, None).await.unwrap();
        let RuntimeOutcome::Failed {
            traces,
            error_message,
            ..
        } = outcome
        else {
            panic!("tool backend failure must stop the turn with a trace")
        };
        assert_eq!(traces.len(), 1);
        assert_eq!(traces[0].status, "failed");
        assert!(
            traces[0]
                .error_message
                .as_deref()
                .is_some_and(|message| message.contains("unavailable"))
        );
        assert!(error_message.contains("unavailable"));
    }

    #[tokio::test]
    async fn max_turns_stops_a_tool_loop_after_the_budget_is_exhausted() {
        let engine = Arc::new(ScriptedEngine {
            responses: Mutex::new(VecDeque::from([
                ProviderChatResponse {
                    assistant_message: ChatMessage {
                        role: ChatMessageRole::Assistant,
                        content: None,
                        name: None,
                        tool_call_id: None,
                        tool_calls: vec![ToolCall {
                            id: "publish-1".to_string(),
                            name: "publish".to_string(),
                            arguments: serde_json::json!({"id": "draft-1"}),
                        }],
                        metadata: serde_json::Value::Null,
                    },
                    finish_reason: Some("tool_calls".to_string()),
                    raw_payload: serde_json::Value::Null,
                },
                ProviderChatResponse {
                    assistant_message: ChatMessage {
                        role: ChatMessageRole::Assistant,
                        content: None,
                        name: None,
                        tool_call_id: None,
                        tool_calls: vec![ToolCall {
                            id: "publish-2".to_string(),
                            name: "publish".to_string(),
                            arguments: serde_json::json!({"id": "draft-1"}),
                        }],
                        metadata: serde_json::Value::Null,
                    },
                    finish_reason: Some("tool_calls".to_string()),
                    raw_payload: serde_json::Value::Null,
                },
                ProviderChatResponse {
                    assistant_message: ChatMessage {
                        role: ChatMessageRole::Assistant,
                        content: None,
                        name: None,
                        tool_call_id: None,
                        tool_calls: vec![ToolCall {
                            id: "publish-3".to_string(),
                            name: "publish".to_string(),
                            arguments: serde_json::json!({"id": "draft-1"}),
                        }],
                        metadata: serde_json::Value::Null,
                    },
                    finish_reason: Some("tool_calls".to_string()),
                    raw_payload: serde_json::Value::Null,
                },
            ])),
        });
        let mcp = Arc::new(RecordingMcp::default());
        let driver = RigAgentDriver::new(
            engine.clone(),
            mcp.clone(),
            ToolExecutionPolicy::new(None, Vec::new(), Vec::new()),
        );
        let mut limited_request = request();
        limited_request.max_turns = 1;

        let error = driver
            .run(&config(), limited_request, None, None)
            .await
            .expect_err("max turn budget must stop the next model step");
        assert!(error.to_string().contains("turn"));
        assert_eq!(mcp.calls.lock().await.as_slice(), ["publish"]);
        assert_eq!(engine.responses.lock().await.len(), 2);
    }

    #[tokio::test]
    async fn resumes_from_canonical_persisted_tool_history_without_checkpoint() {
        let engine = Arc::new(ScriptedEngine {
            responses: Mutex::new(VecDeque::from([ProviderChatResponse {
                assistant_message: ChatMessage {
                    role: ChatMessageRole::Assistant,
                    content: Some("The prior tool result was applied.".to_string()),
                    name: None,
                    tool_call_id: None,
                    tool_calls: Vec::new(),
                    metadata: serde_json::Value::Null,
                },
                finish_reason: Some("stop".to_string()),
                raw_payload: serde_json::Value::Null,
            }])),
        });
        let mcp = Arc::new(RecordingMcp::default());
        let driver = RigAgentDriver::new(
            engine,
            mcp.clone(),
            ToolExecutionPolicy::new(None, Vec::new(), Vec::new()),
        );
        let mut recovered = request();
        recovered.messages.extend([
            ChatMessage {
                role: ChatMessageRole::Assistant,
                content: None,
                name: None,
                tool_call_id: None,
                tool_calls: vec![ToolCall {
                    id: "persisted-call".to_string(),
                    name: "publish".to_string(),
                    arguments: serde_json::json!({ "id": "draft-1" }),
                }],
                metadata: serde_json::json!({ "persisted": true }),
            },
            ChatMessage {
                role: ChatMessageRole::Tool,
                content: Some("published".to_string()),
                name: Some("publish".to_string()),
                tool_call_id: Some("persisted-call".to_string()),
                tool_calls: Vec::new(),
                metadata: serde_json::json!({ "approval_approved": true }),
            },
        ]);

        let outcome = driver.run(&config(), recovered, None, None).await.unwrap();
        let RuntimeOutcome::Completed {
            appended_messages, ..
        } = outcome
        else {
            panic!("persisted tool history should resume directly into a model turn")
        };
        assert_eq!(
            appended_messages
                .last()
                .and_then(|message| message.content.as_deref()),
            Some("The prior tool result was applied.")
        );
        assert!(mcp.calls.lock().await.is_empty());
    }

    #[tokio::test]
    async fn cancellation_stops_before_the_next_provider_call() {
        let engine = Arc::new(ScriptedEngine {
            responses: Mutex::new(VecDeque::new()),
        });
        let mcp = Arc::new(RecordingMcp::default());
        let driver = RigAgentDriver::new(
            engine,
            mcp,
            ToolExecutionPolicy::new(None, Vec::new(), Vec::new()),
        );
        let (sender, receiver) = tokio::sync::watch::channel(());
        sender.send(()).unwrap();

        let error = driver
            .run(&config(), request(), None, Some(receiver))
            .await
            .expect_err("cancelled run must not invoke the provider");
        assert!(error.to_string().contains("cancelled"));
    }
}
