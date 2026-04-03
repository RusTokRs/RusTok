use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ProviderKind {
    OpenAiCompatible,
    Anthropic,
    Gemini,
}

impl ProviderKind {
    pub const fn slug(self) -> &'static str {
        match self {
            Self::OpenAiCompatible => "openai_compatible",
            Self::Anthropic => "anthropic",
            Self::Gemini => "gemini",
        }
    }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Hash)]
#[serde(rename_all = "snake_case")]
pub enum ProviderCapability {
    TextGeneration,
    StructuredGeneration,
    ImageGeneration,
    MultimodalUnderstanding,
    CodeGeneration,
    AlloyAssist,
}

impl ProviderCapability {
    pub const fn slug(self) -> &'static str {
        match self {
            Self::TextGeneration => "text_generation",
            Self::StructuredGeneration => "structured_generation",
            Self::ImageGeneration => "image_generation",
            Self::MultimodalUnderstanding => "multimodal_understanding",
            Self::CodeGeneration => "code_generation",
            Self::AlloyAssist => "alloy_assist",
        }
    }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ExecutionMode {
    Auto,
    Direct,
    McpTooling,
}

impl ExecutionMode {
    pub const fn slug(self) -> &'static str {
        match self {
            Self::Auto => "auto",
            Self::Direct => "direct",
            Self::McpTooling => "mcp_tooling",
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ProviderUsagePolicy {
    #[serde(default)]
    pub allowed_task_profiles: Vec<String>,
    #[serde(default)]
    pub denied_task_profiles: Vec<String>,
    #[serde(default)]
    pub restricted_role_slugs: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AiProviderConfig {
    pub provider_kind: ProviderKind,
    pub base_url: String,
    pub api_key: Option<String>,
    pub model: String,
    pub temperature: Option<f32>,
    pub max_tokens: Option<u32>,
    #[serde(default)]
    pub capabilities: Vec<ProviderCapability>,
    #[serde(default)]
    pub usage_policy: ProviderUsagePolicy,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ChatMessageRole {
    System,
    User,
    Assistant,
    Tool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolCall {
    pub id: String,
    pub name: String,
    pub arguments: serde_json::Value,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChatMessage {
    pub role: ChatMessageRole,
    pub content: Option<String>,
    pub name: Option<String>,
    pub tool_call_id: Option<String>,
    #[serde(default)]
    pub tool_calls: Vec<ToolCall>,
    #[serde(default)]
    pub metadata: serde_json::Value,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolDefinition {
    pub name: String,
    pub description: String,
    #[serde(default)]
    pub input_schema: serde_json::Value,
    pub sensitive: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProviderChatRequest {
    pub model: String,
    pub messages: Vec<ChatMessage>,
    pub tools: Vec<ToolDefinition>,
    pub temperature: Option<f32>,
    pub max_tokens: Option<u32>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProviderChatResponse {
    pub assistant_message: ChatMessage,
    pub finish_reason: Option<String>,
    #[serde(default)]
    pub raw_payload: serde_json::Value,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProviderTestResult {
    pub ok: bool,
    pub provider: String,
    pub model: Option<String>,
    pub latency_ms: i64,
    pub message: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolTrace {
    pub tool_name: String,
    pub input_payload: serde_json::Value,
    pub output_payload: Option<serde_json::Value>,
    pub status: String,
    pub duration_ms: i64,
    pub sensitive: bool,
    pub error_message: Option<String>,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PendingApproval {
    pub tool_name: String,
    pub tool_call_id: String,
    pub input_payload: serde_json::Value,
    pub reason: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TaskProfile {
    pub id: Uuid,
    pub slug: String,
    pub display_name: String,
    pub description: Option<String>,
    pub target_capability: ProviderCapability,
    pub system_prompt: Option<String>,
    #[serde(default)]
    pub allowed_provider_profile_ids: Vec<Uuid>,
    #[serde(default)]
    pub preferred_provider_profile_ids: Vec<Uuid>,
    pub fallback_strategy: String,
    pub tool_profile_id: Option<Uuid>,
    #[serde(default)]
    pub approval_policy: serde_json::Value,
    pub default_execution_mode: ExecutionMode,
    pub is_active: bool,
    #[serde(default)]
    pub metadata: serde_json::Value,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ExecutionOverride {
    pub provider_profile_id: Option<Uuid>,
    pub model: Option<String>,
    pub execution_mode: Option<ExecutionMode>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AiRunRequest {
    pub task_profile_id: Option<Uuid>,
    pub provider_profile_id: Option<Uuid>,
    pub tool_profile_id: Option<Uuid>,
    pub messages: Vec<ChatMessage>,
    pub override_config: ExecutionOverride,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct AiRunDecisionTrace {
    pub task_profile_id: Option<Uuid>,
    pub task_profile_slug: Option<String>,
    pub provider_profile_id: Option<Uuid>,
    pub provider_slug: Option<String>,
    pub provider_kind: Option<ProviderKind>,
    pub selected_model: Option<String>,
    pub execution_mode: Option<ExecutionMode>,
    #[serde(default)]
    pub reasons: Vec<String>,
    pub used_override: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RuntimeRequest {
    pub model: String,
    pub messages: Vec<ChatMessage>,
    pub temperature: Option<f32>,
    pub max_tokens: Option<u32>,
    pub max_turns: usize,
    pub execution_mode: ExecutionMode,
    pub system_prompt: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum RuntimeOutcome {
    Completed {
        appended_messages: Vec<ChatMessage>,
        traces: Vec<ToolTrace>,
    },
    WaitingApproval {
        appended_messages: Vec<ChatMessage>,
        traces: Vec<ToolTrace>,
        pending_approval: PendingApproval,
    },
    Failed {
        appended_messages: Vec<ChatMessage>,
        traces: Vec<ToolTrace>,
        error_message: String,
    },
}
