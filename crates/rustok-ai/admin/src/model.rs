use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct AiAdminBootstrap {
    pub metrics: AiRuntimeMetricsPayload,
    pub provider_catalog: Vec<AiProviderCatalogEntryPayload>,
    pub provider_targets: Vec<AiProviderTargetPayload>,
    pub providers: Vec<AiProviderProfilePayload>,
    pub task_profiles: Vec<AiTaskProfilePayload>,
    pub tool_profiles: Vec<AiToolProfilePayload>,
    pub sessions: Vec<AiChatSessionSummaryPayload>,
    pub recent_runs: Vec<AiRecentRunPayload>,
    pub recent_stream_events: Vec<AiRunStreamEventPayload>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct AiMetricBucketPayload {
    pub label: String,
    pub total: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct AiRuntimeMetricsPayload {
    pub router_resolutions_total: u64,
    pub router_overrides_total: u64,
    pub selected_auto_total: u64,
    pub selected_direct_total: u64,
    pub selected_mcp_total: u64,
    pub completed_runs_total: u64,
    pub failed_runs_total: u64,
    pub waiting_approval_runs_total: u64,
    pub locale_fallback_total: u64,
    pub run_latency_ms_total: u64,
    pub run_latency_samples: u64,
    pub provider_slug_totals: Vec<AiMetricBucketPayload>,
    pub execution_target_totals: Vec<AiMetricBucketPayload>,
    pub task_profile_totals: Vec<AiMetricBucketPayload>,
    pub resolved_locale_totals: Vec<AiMetricBucketPayload>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AiProviderProfilePayload {
    pub id: String,
    pub slug: String,
    pub display_name: String,
    pub provider_slug: String,
    pub provider_target_id: String,
    pub model: String,
    pub credential_refs: Vec<AiCredentialRefPayload>,
    pub temperature: Option<f32>,
    pub max_tokens: Option<i32>,
    pub is_active: bool,
    pub has_credentials: bool,
    pub capabilities: Vec<String>,
    pub allowed_task_profiles: Vec<String>,
    pub denied_task_profiles: Vec<String>,
    pub restricted_role_slugs: Vec<String>,
    pub metadata: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct AiProviderCatalogEntryPayload {
    pub slug: String,
    pub display_name: String,
    pub features: Vec<String>,
    pub settings_schema: Vec<AiProviderFieldPayload>,
    pub credential_schema: Vec<AiProviderFieldPayload>,
    pub default_settings: Vec<AiProviderSettingPayload>,
    pub compiled_in: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct AiProviderTargetPayload {
    pub id: String,
    pub provider_slug: String,
    pub display_name: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct AiProviderFieldPayload {
    pub key: String,
    pub label: String,
    pub kind: String,
    pub required: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct AiProviderSettingPayload {
    pub key: String,
    pub text_value: Option<String>,
    pub integer_value: Option<i64>,
    pub boolean_value: Option<bool>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct AiCredentialRefPayload {
    pub key: String,
    pub resolver: String,
    pub secret_key: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AiToolProfilePayload {
    pub id: String,
    pub slug: String,
    pub display_name: String,
    pub description: Option<String>,
    pub allowed_tools: Vec<String>,
    pub denied_tools: Vec<String>,
    pub sensitive_tools: Vec<String>,
    pub is_active: bool,
    pub metadata: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AiTaskProfilePayload {
    pub id: String,
    pub slug: String,
    pub display_name: String,
    pub description: Option<String>,
    pub target_capability: String,
    pub system_prompt: Option<String>,
    pub allowed_provider_profile_ids: Vec<String>,
    pub preferred_provider_profile_ids: Vec<String>,
    pub fallback_strategy: String,
    pub tool_profile_id: Option<String>,
    pub default_execution_mode: String,
    pub is_active: bool,
    pub metadata: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AiChatSessionSummaryPayload {
    pub id: String,
    pub title: String,
    pub provider_profile_id: String,
    pub task_profile_id: Option<String>,
    pub tool_profile_id: Option<String>,
    pub execution_mode: String,
    pub requested_locale: Option<String>,
    pub resolved_locale: String,
    pub status: String,
    pub latest_run_status: Option<String>,
    pub pending_approvals: i32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AiToolCallPayload {
    pub id: String,
    pub name: String,
    pub arguments_json: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AiChatMessagePayload {
    pub id: String,
    pub session_id: String,
    pub run_id: Option<String>,
    pub role: String,
    pub content: Option<String>,
    pub name: Option<String>,
    pub tool_call_id: Option<String>,
    pub tool_calls: Vec<AiToolCallPayload>,
    pub metadata: String,
    pub created_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AiChatRunPayload {
    pub id: String,
    pub session_id: String,
    pub provider_profile_id: String,
    pub task_profile_id: Option<String>,
    pub tool_profile_id: Option<String>,
    pub status: String,
    pub model: String,
    pub execution_mode: String,
    pub execution_path: String,
    pub requested_locale: Option<String>,
    pub resolved_locale: String,
    pub temperature: Option<f32>,
    pub max_tokens: Option<i32>,
    pub error_message: Option<String>,
    pub pending_approval_id: Option<String>,
    pub decision_trace: String,
    pub metadata: String,
    pub created_at: String,
    pub started_at: String,
    pub completed_at: Option<String>,
    pub updated_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AiRecentRunPayload {
    pub id: String,
    pub session_id: String,
    pub session_title: String,
    pub provider_profile_id: String,
    pub provider_display_name: String,
    pub provider_slug: String,
    pub task_profile_id: Option<String>,
    pub task_profile_slug: Option<String>,
    pub status: String,
    pub model: String,
    pub execution_mode: String,
    pub execution_path: String,
    pub execution_target: Option<String>,
    pub requested_locale: Option<String>,
    pub resolved_locale: String,
    pub error_message: Option<String>,
    pub started_at: String,
    pub completed_at: Option<String>,
    pub updated_at: String,
    pub duration_ms: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AiToolTracePayload {
    pub tool_name: String,
    pub input_payload: String,
    pub output_payload: Option<String>,
    pub status: String,
    pub duration_ms: i64,
    pub sensitive: bool,
    pub error_message: Option<String>,
    pub created_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AiApprovalRequestPayload {
    pub id: String,
    pub session_id: String,
    pub run_id: String,
    pub approval_batch_id: String,
    pub tool_name: String,
    pub tool_call_id: String,
    pub tool_input: String,
    pub reason: Option<String>,
    pub status: String,
    pub resolved_by: Option<String>,
    pub resolved_at: Option<String>,
    pub metadata: String,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AiChatSessionDetailPayload {
    pub session: AiChatSessionSummaryPayload,
    pub provider_profile: AiProviderProfilePayload,
    pub task_profile: Option<AiTaskProfilePayload>,
    pub tool_profile: Option<AiToolProfilePayload>,
    pub messages: Vec<AiChatMessagePayload>,
    pub runs: Vec<AiChatRunPayload>,
    pub tool_traces: Vec<AiToolTracePayload>,
    pub approvals: Vec<AiApprovalRequestPayload>,
    pub recent_stream_events: Vec<AiRunStreamEventPayload>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AiProviderTestResultPayload {
    pub ok: bool,
    pub provider: String,
    pub model: Option<String>,
    pub latency_ms: i64,
    pub message: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AiSendMessageResultPayload {
    pub session: AiChatSessionDetailPayload,
    pub run: AiChatRunPayload,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq, Eq)]
pub struct AiLiveStreamStatePayload {
    pub run_id: String,
    pub status: String,
    pub content: String,
    pub error_message: Option<String>,
    pub sequence: u64,
    pub connected: bool,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum AiRunStreamEventKindPayload {
    Started,
    Delta,
    ToolCall,
    Completed,
    Failed,
    Cancelled,
    WaitingApproval,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AiStreamToolCallPayload {
    pub id: String,
    pub name: String,
    pub arguments: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AiRunStreamEventPayload {
    pub session_id: String,
    pub run_id: String,
    pub event_kind: AiRunStreamEventKindPayload,
    pub content_delta: Option<String>,
    pub accumulated_content: Option<String>,
    pub error_message: Option<String>,
    pub tool_call: Option<AiStreamToolCallPayload>,
    pub sequence: u64,
    pub created_at: String,
}

#[cfg(target_arch = "wasm32")]
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum AiSessionSubscriptionEnvelope {
    ConnectionAck,
    Next {
        payload: AiSessionSubscriptionPayload,
    },
    Error {
        payload: Vec<AiSessionSubscriptionError>,
    },
    Complete,
    Ping {
        payload: Option<serde_json::Value>,
    },
    Pong,
}

#[cfg(target_arch = "wasm32")]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AiSessionSubscriptionPayload {
    pub data: Option<AiSessionSubscriptionData>,
    pub errors: Option<Vec<AiSessionSubscriptionError>>,
}

#[cfg(target_arch = "wasm32")]
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AiSessionSubscriptionData {
    pub ai_session_events: Option<AiRunStreamEventPayload>,
}

#[cfg(target_arch = "wasm32")]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AiSessionSubscriptionError {
    pub message: String,
}
