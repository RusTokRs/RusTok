use chrono::{DateTime, Utc};
use rustok_core::permissions::Permission;
use rustok_core::registry::ModuleRegistry;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::model::{
    AiRunDecisionTrace, ChatMessageRole, ExecutionMode, ExecutionOverride, ProviderCapability,
    ProviderKind, ProviderUsagePolicy, ToolCall, ToolTrace,
};

#[derive(Clone)]
pub struct SharedAiModuleRegistry(pub ModuleRegistry);

#[derive(Debug, Clone)]
pub struct AiOperatorContext {
    pub tenant_id: Uuid,
    pub user_id: Uuid,
    pub permissions: Vec<Permission>,
    pub role_slugs: Vec<String>,
    pub preferred_locale: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreateAiProviderProfileInput {
    pub slug: String,
    pub display_name: String,
    pub provider_kind: ProviderKind,
    pub base_url: String,
    pub model: String,
    pub api_key_secret: Option<String>,
    pub temperature: Option<f32>,
    pub max_tokens: Option<i32>,
    pub capabilities: Vec<ProviderCapability>,
    pub usage_policy: ProviderUsagePolicy,
    pub metadata: serde_json::Value,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UpdateAiProviderProfileInput {
    pub display_name: String,
    pub base_url: String,
    pub model: String,
    pub temperature: Option<f32>,
    pub max_tokens: Option<i32>,
    pub capabilities: Vec<ProviderCapability>,
    pub usage_policy: ProviderUsagePolicy,
    pub metadata: serde_json::Value,
    pub is_active: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreateAiTaskProfileInput {
    pub slug: String,
    pub display_name: String,
    pub description: Option<String>,
    pub target_capability: ProviderCapability,
    pub system_prompt: Option<String>,
    pub allowed_provider_profile_ids: Vec<Uuid>,
    pub preferred_provider_profile_ids: Vec<Uuid>,
    pub fallback_strategy: String,
    pub tool_profile_id: Option<Uuid>,
    pub approval_policy: serde_json::Value,
    pub default_execution_mode: ExecutionMode,
    pub metadata: serde_json::Value,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UpdateAiTaskProfileInput {
    pub display_name: String,
    pub description: Option<String>,
    pub target_capability: ProviderCapability,
    pub system_prompt: Option<String>,
    pub allowed_provider_profile_ids: Vec<Uuid>,
    pub preferred_provider_profile_ids: Vec<Uuid>,
    pub fallback_strategy: String,
    pub tool_profile_id: Option<Uuid>,
    pub approval_policy: serde_json::Value,
    pub default_execution_mode: ExecutionMode,
    pub metadata: serde_json::Value,
    pub is_active: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreateAiToolProfileInput {
    pub slug: String,
    pub display_name: String,
    pub description: Option<String>,
    pub allowed_tools: Vec<String>,
    pub denied_tools: Vec<String>,
    pub sensitive_tools: Vec<String>,
    pub metadata: serde_json::Value,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UpdateAiToolProfileInput {
    pub display_name: String,
    pub description: Option<String>,
    pub allowed_tools: Vec<String>,
    pub denied_tools: Vec<String>,
    pub sensitive_tools: Vec<String>,
    pub metadata: serde_json::Value,
    pub is_active: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StartAiChatSessionInput {
    pub title: String,
    pub provider_profile_id: Option<Uuid>,
    pub task_profile_id: Option<Uuid>,
    pub tool_profile_id: Option<Uuid>,
    pub execution_mode: Option<ExecutionMode>,
    pub override_config: ExecutionOverride,
    pub locale: Option<String>,
    pub initial_message: Option<String>,
    pub metadata: serde_json::Value,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RunAiTaskJobInput {
    pub title: String,
    pub provider_profile_id: Option<Uuid>,
    pub task_profile_id: Uuid,
    pub execution_mode: Option<ExecutionMode>,
    pub locale: Option<String>,
    pub task_input_json: serde_json::Value,
    pub metadata: serde_json::Value,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SendAiChatMessageInput {
    pub content: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResumeAiApprovalInput {
    pub approved: bool,
    pub reason: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AiProviderProfileRecord {
    pub id: Uuid,
    pub slug: String,
    pub display_name: String,
    pub provider_kind: ProviderKind,
    pub base_url: String,
    pub model: String,
    pub temperature: Option<f32>,
    pub max_tokens: Option<i32>,
    pub is_active: bool,
    pub has_secret: bool,
    pub capabilities: Vec<ProviderCapability>,
    pub usage_policy: ProviderUsagePolicy,
    pub metadata: serde_json::Value,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AiTaskProfileRecord {
    pub id: Uuid,
    pub slug: String,
    pub display_name: String,
    pub description: Option<String>,
    pub target_capability: ProviderCapability,
    pub system_prompt: Option<String>,
    pub allowed_provider_profile_ids: Vec<Uuid>,
    pub preferred_provider_profile_ids: Vec<Uuid>,
    pub fallback_strategy: String,
    pub tool_profile_id: Option<Uuid>,
    pub approval_policy: serde_json::Value,
    pub default_execution_mode: ExecutionMode,
    pub is_active: bool,
    pub metadata: serde_json::Value,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AiToolProfileRecord {
    pub id: Uuid,
    pub slug: String,
    pub display_name: String,
    pub description: Option<String>,
    pub allowed_tools: Vec<String>,
    pub denied_tools: Vec<String>,
    pub sensitive_tools: Vec<String>,
    pub is_active: bool,
    pub metadata: serde_json::Value,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AiChatMessageRecord {
    pub id: Uuid,
    pub session_id: Uuid,
    pub run_id: Option<Uuid>,
    pub role: ChatMessageRole,
    pub content: Option<String>,
    pub name: Option<String>,
    pub tool_call_id: Option<String>,
    pub tool_calls: Vec<ToolCall>,
    pub metadata: serde_json::Value,
    pub created_at: DateTime<Utc>,
    pub created_by: Option<Uuid>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AiChatRunRecord {
    pub id: Uuid,
    pub session_id: Uuid,
    pub provider_profile_id: Uuid,
    pub task_profile_id: Option<Uuid>,
    pub tool_profile_id: Option<Uuid>,
    pub status: String,
    pub model: String,
    pub execution_mode: ExecutionMode,
    pub execution_path: ExecutionMode,
    pub requested_locale: Option<String>,
    pub resolved_locale: String,
    pub temperature: Option<f32>,
    pub max_tokens: Option<i32>,
    pub error_message: Option<String>,
    pub pending_approval_id: Option<Uuid>,
    pub decision_trace: AiRunDecisionTrace,
    pub metadata: serde_json::Value,
    pub created_at: DateTime<Utc>,
    pub started_at: DateTime<Utc>,
    pub completed_at: Option<DateTime<Utc>>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AiRecentRunRecord {
    pub id: Uuid,
    pub session_id: Uuid,
    pub session_title: String,
    pub provider_profile_id: Uuid,
    pub provider_display_name: String,
    pub provider_kind: ProviderKind,
    pub task_profile_id: Option<Uuid>,
    pub task_profile_slug: Option<String>,
    pub status: String,
    pub model: String,
    pub execution_mode: ExecutionMode,
    pub execution_path: ExecutionMode,
    pub execution_target: Option<String>,
    pub requested_locale: Option<String>,
    pub resolved_locale: String,
    pub error_message: Option<String>,
    pub started_at: DateTime<Utc>,
    pub completed_at: Option<DateTime<Utc>>,
    pub updated_at: DateTime<Utc>,
    pub duration_ms: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AiApprovalRequestRecord {
    pub id: Uuid,
    pub session_id: Uuid,
    pub run_id: Uuid,
    pub tool_name: String,
    pub tool_call_id: String,
    pub tool_input: serde_json::Value,
    pub reason: Option<String>,
    pub status: String,
    pub resolved_by: Option<Uuid>,
    pub resolved_at: Option<DateTime<Utc>>,
    pub metadata: serde_json::Value,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AiChatSessionSummary {
    pub id: Uuid,
    pub title: String,
    pub provider_profile_id: Uuid,
    pub task_profile_id: Option<Uuid>,
    pub tool_profile_id: Option<Uuid>,
    pub execution_mode: ExecutionMode,
    pub requested_locale: Option<String>,
    pub resolved_locale: String,
    pub status: String,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub latest_run_status: Option<String>,
    pub pending_approvals: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AiChatSessionDetail {
    pub session: AiChatSessionSummary,
    pub provider_profile: AiProviderProfileRecord,
    pub task_profile: Option<AiTaskProfileRecord>,
    pub tool_profile: Option<AiToolProfileRecord>,
    pub messages: Vec<AiChatMessageRecord>,
    pub runs: Vec<AiChatRunRecord>,
    pub tool_traces: Vec<ToolTrace>,
    pub approvals: Vec<AiApprovalRequestRecord>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AiSendMessageResult {
    pub session: AiChatSessionDetail,
    pub run: AiChatRunRecord,
}
