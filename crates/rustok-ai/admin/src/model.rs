use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct AiAdminBootstrap {
    pub providers: Vec<AiProviderProfilePayload>,
    pub task_profiles: Vec<AiTaskProfilePayload>,
    pub tool_profiles: Vec<AiToolProfilePayload>,
    pub sessions: Vec<AiChatSessionSummaryPayload>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AiProviderProfilePayload {
    pub id: String,
    pub slug: String,
    pub display_name: String,
    pub provider_kind: String,
    pub base_url: String,
    pub model: String,
    pub temperature: Option<f32>,
    pub max_tokens: Option<i32>,
    pub is_active: bool,
    pub has_secret: bool,
    pub capabilities: Vec<String>,
    pub allowed_task_profiles: Vec<String>,
    pub denied_task_profiles: Vec<String>,
    pub restricted_role_slugs: Vec<String>,
    pub metadata: String,
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
