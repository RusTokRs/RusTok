use async_graphql::{Enum, InputObject, SimpleObject};
use chrono::{DateTime, Utc};
use uuid::Uuid;

use rustok_ai::{
    AiApprovalRequestRecord, AiChatMessageRecord, AiChatRunRecord, AiChatSessionDetail,
    AiChatSessionSummary, AiProviderProfileRecord, AiTaskProfileRecord, AiToolProfileRecord,
    ChatMessageRole, ExecutionMode, ProviderCapability, ProviderKind, ProviderUsagePolicy,
    ToolCall, ToolTrace,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Enum)]
pub enum AiProviderKindGql {
    OpenAiCompatible,
    Anthropic,
    Gemini,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Enum)]
pub enum AiProviderCapabilityGql {
    TextGeneration,
    StructuredGeneration,
    ImageGeneration,
    MultimodalUnderstanding,
    CodeGeneration,
    AlloyAssist,
}

impl From<AiProviderCapabilityGql> for ProviderCapability {
    fn from(value: AiProviderCapabilityGql) -> Self {
        match value {
            AiProviderCapabilityGql::TextGeneration => ProviderCapability::TextGeneration,
            AiProviderCapabilityGql::StructuredGeneration => {
                ProviderCapability::StructuredGeneration
            }
            AiProviderCapabilityGql::ImageGeneration => ProviderCapability::ImageGeneration,
            AiProviderCapabilityGql::MultimodalUnderstanding => {
                ProviderCapability::MultimodalUnderstanding
            }
            AiProviderCapabilityGql::CodeGeneration => ProviderCapability::CodeGeneration,
            AiProviderCapabilityGql::AlloyAssist => ProviderCapability::AlloyAssist,
        }
    }
}

impl From<ProviderCapability> for AiProviderCapabilityGql {
    fn from(value: ProviderCapability) -> Self {
        match value {
            ProviderCapability::TextGeneration => Self::TextGeneration,
            ProviderCapability::StructuredGeneration => Self::StructuredGeneration,
            ProviderCapability::ImageGeneration => Self::ImageGeneration,
            ProviderCapability::MultimodalUnderstanding => Self::MultimodalUnderstanding,
            ProviderCapability::CodeGeneration => Self::CodeGeneration,
            ProviderCapability::AlloyAssist => Self::AlloyAssist,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Enum)]
pub enum AiExecutionModeGql {
    Auto,
    Direct,
    McpTooling,
}

#[derive(Debug, Clone, SimpleObject)]
pub struct AiProviderUsagePolicyGql {
    pub allowed_task_profiles: Vec<String>,
    pub denied_task_profiles: Vec<String>,
    pub restricted_role_slugs: Vec<String>,
}

impl From<ProviderUsagePolicy> for AiProviderUsagePolicyGql {
    fn from(value: ProviderUsagePolicy) -> Self {
        Self {
            allowed_task_profiles: value.allowed_task_profiles,
            denied_task_profiles: value.denied_task_profiles,
            restricted_role_slugs: value.restricted_role_slugs,
        }
    }
}

#[derive(Debug, Clone, InputObject)]
pub struct AiProviderUsagePolicyInputGql {
    pub allowed_task_profiles: Vec<String>,
    pub denied_task_profiles: Vec<String>,
    pub restricted_role_slugs: Vec<String>,
}

impl From<AiProviderUsagePolicyInputGql> for ProviderUsagePolicy {
    fn from(value: AiProviderUsagePolicyInputGql) -> Self {
        Self {
            allowed_task_profiles: value.allowed_task_profiles,
            denied_task_profiles: value.denied_task_profiles,
            restricted_role_slugs: value.restricted_role_slugs,
        }
    }
}

impl From<AiExecutionModeGql> for ExecutionMode {
    fn from(value: AiExecutionModeGql) -> Self {
        match value {
            AiExecutionModeGql::Auto => ExecutionMode::Auto,
            AiExecutionModeGql::Direct => ExecutionMode::Direct,
            AiExecutionModeGql::McpTooling => ExecutionMode::McpTooling,
        }
    }
}

impl From<ExecutionMode> for AiExecutionModeGql {
    fn from(value: ExecutionMode) -> Self {
        match value {
            ExecutionMode::Auto => Self::Auto,
            ExecutionMode::Direct => Self::Direct,
            ExecutionMode::McpTooling => Self::McpTooling,
        }
    }
}

impl From<AiProviderKindGql> for ProviderKind {
    fn from(value: AiProviderKindGql) -> Self {
        match value {
            AiProviderKindGql::OpenAiCompatible => ProviderKind::OpenAiCompatible,
            AiProviderKindGql::Anthropic => ProviderKind::Anthropic,
            AiProviderKindGql::Gemini => ProviderKind::Gemini,
        }
    }
}

impl From<ProviderKind> for AiProviderKindGql {
    fn from(value: ProviderKind) -> Self {
        match value {
            ProviderKind::OpenAiCompatible => Self::OpenAiCompatible,
            ProviderKind::Anthropic => Self::Anthropic,
            ProviderKind::Gemini => Self::Gemini,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Enum)]
pub enum AiChatMessageRoleGql {
    System,
    User,
    Assistant,
    Tool,
}

impl From<ChatMessageRole> for AiChatMessageRoleGql {
    fn from(value: ChatMessageRole) -> Self {
        match value {
            ChatMessageRole::System => Self::System,
            ChatMessageRole::User => Self::User,
            ChatMessageRole::Assistant => Self::Assistant,
            ChatMessageRole::Tool => Self::Tool,
        }
    }
}

#[derive(Debug, Clone, SimpleObject)]
pub struct AiToolCallGql {
    pub id: String,
    pub name: String,
    pub arguments_json: String,
}

impl TryFrom<ToolCall> for AiToolCallGql {
    type Error = async_graphql::Error;

    fn try_from(value: ToolCall) -> Result<Self, Self::Error> {
        Ok(Self {
            id: value.id,
            name: value.name,
            arguments_json: serde_json::to_string(&value.arguments)
                .map_err(|err| async_graphql::Error::new(err.to_string()))?,
        })
    }
}

#[derive(Debug, Clone, SimpleObject)]
pub struct AiProviderProfileGql {
    pub id: Uuid,
    pub slug: String,
    pub display_name: String,
    pub provider_kind: AiProviderKindGql,
    pub base_url: String,
    pub model: String,
    pub temperature: Option<f32>,
    pub max_tokens: Option<i32>,
    pub is_active: bool,
    pub has_secret: bool,
    pub capabilities: Vec<AiProviderCapabilityGql>,
    pub usage_policy: AiProviderUsagePolicyGql,
    pub metadata: String,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

impl From<AiProviderProfileRecord> for AiProviderProfileGql {
    fn from(value: AiProviderProfileRecord) -> Self {
        Self {
            id: value.id,
            slug: value.slug,
            display_name: value.display_name,
            provider_kind: value.provider_kind.into(),
            base_url: value.base_url,
            model: value.model,
            temperature: value.temperature,
            max_tokens: value.max_tokens,
            is_active: value.is_active,
            has_secret: value.has_secret,
            capabilities: value.capabilities.into_iter().map(Into::into).collect(),
            usage_policy: value.usage_policy.into(),
            metadata: value.metadata.to_string(),
            created_at: value.created_at,
            updated_at: value.updated_at,
        }
    }
}

#[derive(Debug, Clone, SimpleObject)]
pub struct AiToolProfileGql {
    pub id: Uuid,
    pub slug: String,
    pub display_name: String,
    pub description: Option<String>,
    pub allowed_tools: Vec<String>,
    pub denied_tools: Vec<String>,
    pub sensitive_tools: Vec<String>,
    pub is_active: bool,
    pub metadata: String,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

impl From<AiToolProfileRecord> for AiToolProfileGql {
    fn from(value: AiToolProfileRecord) -> Self {
        Self {
            id: value.id,
            slug: value.slug,
            display_name: value.display_name,
            description: value.description,
            allowed_tools: value.allowed_tools,
            denied_tools: value.denied_tools,
            sensitive_tools: value.sensitive_tools,
            is_active: value.is_active,
            metadata: value.metadata.to_string(),
            created_at: value.created_at,
            updated_at: value.updated_at,
        }
    }
}

#[derive(Debug, Clone, SimpleObject)]
pub struct AiTaskProfileGql {
    pub id: Uuid,
    pub slug: String,
    pub display_name: String,
    pub description: Option<String>,
    pub target_capability: AiProviderCapabilityGql,
    pub system_prompt: Option<String>,
    pub allowed_provider_profile_ids: Vec<Uuid>,
    pub preferred_provider_profile_ids: Vec<Uuid>,
    pub fallback_strategy: String,
    pub tool_profile_id: Option<Uuid>,
    pub default_execution_mode: AiExecutionModeGql,
    pub is_active: bool,
    pub metadata: String,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

impl From<AiTaskProfileRecord> for AiTaskProfileGql {
    fn from(value: AiTaskProfileRecord) -> Self {
        Self {
            id: value.id,
            slug: value.slug,
            display_name: value.display_name,
            description: value.description,
            target_capability: value.target_capability.into(),
            system_prompt: value.system_prompt,
            allowed_provider_profile_ids: value.allowed_provider_profile_ids,
            preferred_provider_profile_ids: value.preferred_provider_profile_ids,
            fallback_strategy: value.fallback_strategy,
            tool_profile_id: value.tool_profile_id,
            default_execution_mode: value.default_execution_mode.into(),
            is_active: value.is_active,
            metadata: value.metadata.to_string(),
            created_at: value.created_at,
            updated_at: value.updated_at,
        }
    }
}

#[derive(Debug, Clone, SimpleObject)]
pub struct AiChatMessageGql {
    pub id: Uuid,
    pub session_id: Uuid,
    pub run_id: Option<Uuid>,
    pub role: AiChatMessageRoleGql,
    pub content: Option<String>,
    pub name: Option<String>,
    pub tool_call_id: Option<String>,
    pub tool_calls: Vec<AiToolCallGql>,
    pub metadata: String,
    pub created_at: DateTime<Utc>,
}

impl TryFrom<AiChatMessageRecord> for AiChatMessageGql {
    type Error = async_graphql::Error;

    fn try_from(value: AiChatMessageRecord) -> Result<Self, Self::Error> {
        Ok(Self {
            id: value.id,
            session_id: value.session_id,
            run_id: value.run_id,
            role: value.role.into(),
            content: value.content,
            name: value.name,
            tool_call_id: value.tool_call_id,
            tool_calls: value
                .tool_calls
                .into_iter()
                .map(TryInto::try_into)
                .collect::<Result<Vec<_>, _>>()?,
            metadata: value.metadata.to_string(),
            created_at: value.created_at,
        })
    }
}

#[derive(Debug, Clone, SimpleObject)]
pub struct AiChatRunGql {
    pub id: Uuid,
    pub session_id: Uuid,
    pub provider_profile_id: Uuid,
    pub task_profile_id: Option<Uuid>,
    pub tool_profile_id: Option<Uuid>,
    pub status: String,
    pub model: String,
    pub execution_mode: AiExecutionModeGql,
    pub execution_path: AiExecutionModeGql,
    pub temperature: Option<f32>,
    pub max_tokens: Option<i32>,
    pub error_message: Option<String>,
    pub pending_approval_id: Option<Uuid>,
    pub decision_trace: String,
    pub metadata: String,
    pub created_at: DateTime<Utc>,
    pub started_at: DateTime<Utc>,
    pub completed_at: Option<DateTime<Utc>>,
    pub updated_at: DateTime<Utc>,
}

impl From<AiChatRunRecord> for AiChatRunGql {
    fn from(value: AiChatRunRecord) -> Self {
        Self {
            id: value.id,
            session_id: value.session_id,
            provider_profile_id: value.provider_profile_id,
            task_profile_id: value.task_profile_id,
            tool_profile_id: value.tool_profile_id,
            status: value.status,
            model: value.model,
            execution_mode: value.execution_mode.into(),
            execution_path: value.execution_path.into(),
            temperature: value.temperature,
            max_tokens: value.max_tokens,
            error_message: value.error_message,
            pending_approval_id: value.pending_approval_id,
            decision_trace: serde_json::to_string(&value.decision_trace)
                .unwrap_or_else(|_| "{}".to_string()),
            metadata: value.metadata.to_string(),
            created_at: value.created_at,
            started_at: value.started_at,
            completed_at: value.completed_at,
            updated_at: value.updated_at,
        }
    }
}

#[derive(Debug, Clone, SimpleObject)]
pub struct AiApprovalRequestGql {
    pub id: Uuid,
    pub session_id: Uuid,
    pub run_id: Uuid,
    pub tool_name: String,
    pub tool_call_id: String,
    pub tool_input: String,
    pub reason: Option<String>,
    pub status: String,
    pub resolved_by: Option<Uuid>,
    pub resolved_at: Option<DateTime<Utc>>,
    pub metadata: String,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

impl AiApprovalRequestGql {
    pub fn from_record(value: AiApprovalRequestRecord) -> Self {
        Self {
            id: value.id,
            session_id: value.session_id,
            run_id: value.run_id,
            tool_name: value.tool_name,
            tool_call_id: value.tool_call_id,
            tool_input: value.tool_input.to_string(),
            reason: value.reason,
            status: value.status,
            resolved_by: value.resolved_by,
            resolved_at: value.resolved_at,
            metadata: value.metadata.to_string(),
            created_at: value.created_at,
            updated_at: value.updated_at,
        }
    }
}

#[derive(Debug, Clone, SimpleObject)]
pub struct AiToolTraceGql {
    pub tool_name: String,
    pub input_payload: String,
    pub output_payload: Option<String>,
    pub status: String,
    pub duration_ms: i64,
    pub sensitive: bool,
    pub error_message: Option<String>,
    pub created_at: DateTime<Utc>,
}

impl AiToolTraceGql {
    pub fn from_record(value: ToolTrace) -> Self {
        Self {
            tool_name: value.tool_name,
            input_payload: value.input_payload.to_string(),
            output_payload: value.output_payload.map(|payload| payload.to_string()),
            status: value.status,
            duration_ms: value.duration_ms,
            sensitive: value.sensitive,
            error_message: value.error_message,
            created_at: value.created_at,
        }
    }
}

#[derive(Debug, Clone, SimpleObject)]
pub struct AiChatSessionSummaryGql {
    pub id: Uuid,
    pub title: String,
    pub provider_profile_id: Uuid,
    pub task_profile_id: Option<Uuid>,
    pub tool_profile_id: Option<Uuid>,
    pub execution_mode: AiExecutionModeGql,
    pub status: String,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub latest_run_status: Option<String>,
    pub pending_approvals: i32,
}

impl From<AiChatSessionSummary> for AiChatSessionSummaryGql {
    fn from(value: AiChatSessionSummary) -> Self {
        Self {
            id: value.id,
            title: value.title,
            provider_profile_id: value.provider_profile_id,
            task_profile_id: value.task_profile_id,
            tool_profile_id: value.tool_profile_id,
            execution_mode: value.execution_mode.into(),
            status: value.status,
            created_at: value.created_at,
            updated_at: value.updated_at,
            latest_run_status: value.latest_run_status,
            pending_approvals: value.pending_approvals as i32,
        }
    }
}

#[derive(Debug, Clone, SimpleObject)]
pub struct AiChatSessionDetailGql {
    pub session: AiChatSessionSummaryGql,
    pub provider_profile: AiProviderProfileGql,
    pub task_profile: Option<AiTaskProfileGql>,
    pub tool_profile: Option<AiToolProfileGql>,
    pub messages: Vec<AiChatMessageGql>,
    pub runs: Vec<AiChatRunGql>,
    pub tool_traces: Vec<AiToolTraceGql>,
    pub approvals: Vec<AiApprovalRequestGql>,
}

impl TryFrom<AiChatSessionDetail> for AiChatSessionDetailGql {
    type Error = async_graphql::Error;

    fn try_from(value: AiChatSessionDetail) -> Result<Self, Self::Error> {
        Ok(Self {
            session: value.session.into(),
            provider_profile: value.provider_profile.into(),
            task_profile: value.task_profile.map(Into::into),
            tool_profile: value.tool_profile.map(Into::into),
            messages: value
                .messages
                .into_iter()
                .map(TryInto::try_into)
                .collect::<Result<Vec<_>, _>>()?,
            runs: value.runs.into_iter().map(Into::into).collect(),
            tool_traces: value
                .tool_traces
                .into_iter()
                .map(AiToolTraceGql::from_record)
                .collect(),
            approvals: value
                .approvals
                .into_iter()
                .map(AiApprovalRequestGql::from_record)
                .collect(),
        })
    }
}

#[derive(Debug, Clone, SimpleObject)]
pub struct AiSendMessageResultGql {
    pub session: AiChatSessionDetailGql,
    pub run: AiChatRunGql,
}

#[derive(Debug, Clone, SimpleObject)]
pub struct AiProviderTestResultGql {
    pub ok: bool,
    pub provider: String,
    pub model: Option<String>,
    pub latency_ms: i64,
    pub message: String,
}

impl From<rustok_ai::ProviderTestResult> for AiProviderTestResultGql {
    fn from(value: rustok_ai::ProviderTestResult) -> Self {
        Self {
            ok: value.ok,
            provider: value.provider,
            model: value.model,
            latency_ms: value.latency_ms,
            message: value.message,
        }
    }
}

#[derive(Debug, Clone, InputObject)]
pub struct CreateAiProviderProfileInputGql {
    pub slug: String,
    pub display_name: String,
    pub provider_kind: AiProviderKindGql,
    pub base_url: String,
    pub model: String,
    pub api_key_secret: Option<String>,
    pub temperature: Option<f32>,
    pub max_tokens: Option<i32>,
    pub capabilities: Vec<AiProviderCapabilityGql>,
    pub usage_policy: AiProviderUsagePolicyInputGql,
    pub metadata: Option<String>,
}

#[derive(Debug, Clone, InputObject)]
pub struct UpdateAiProviderProfileInputGql {
    pub display_name: String,
    pub base_url: String,
    pub model: String,
    pub temperature: Option<f32>,
    pub max_tokens: Option<i32>,
    pub capabilities: Vec<AiProviderCapabilityGql>,
    pub usage_policy: AiProviderUsagePolicyInputGql,
    pub is_active: bool,
    pub metadata: Option<String>,
}

#[derive(Debug, Clone, InputObject)]
pub struct CreateAiToolProfileInputGql {
    pub slug: String,
    pub display_name: String,
    pub description: Option<String>,
    pub allowed_tools: Vec<String>,
    pub denied_tools: Vec<String>,
    pub sensitive_tools: Vec<String>,
    pub metadata: Option<String>,
}

#[derive(Debug, Clone, InputObject)]
pub struct UpdateAiToolProfileInputGql {
    pub display_name: String,
    pub description: Option<String>,
    pub allowed_tools: Vec<String>,
    pub denied_tools: Vec<String>,
    pub sensitive_tools: Vec<String>,
    pub is_active: bool,
    pub metadata: Option<String>,
}

#[derive(Debug, Clone, InputObject)]
pub struct StartAiChatSessionInputGql {
    pub title: String,
    pub provider_profile_id: Option<Uuid>,
    pub task_profile_id: Option<Uuid>,
    pub tool_profile_id: Option<Uuid>,
    pub initial_message: Option<String>,
    pub metadata: Option<String>,
}

#[derive(Debug, Clone, InputObject)]
pub struct CreateAiTaskProfileInputGql {
    pub slug: String,
    pub display_name: String,
    pub description: Option<String>,
    pub target_capability: AiProviderCapabilityGql,
    pub system_prompt: Option<String>,
    pub allowed_provider_profile_ids: Vec<Uuid>,
    pub preferred_provider_profile_ids: Vec<Uuid>,
    pub fallback_strategy: Option<String>,
    pub tool_profile_id: Option<Uuid>,
    pub default_execution_mode: AiExecutionModeGql,
    pub metadata: Option<String>,
}

#[derive(Debug, Clone, InputObject)]
pub struct UpdateAiTaskProfileInputGql {
    pub display_name: String,
    pub description: Option<String>,
    pub target_capability: AiProviderCapabilityGql,
    pub system_prompt: Option<String>,
    pub allowed_provider_profile_ids: Vec<Uuid>,
    pub preferred_provider_profile_ids: Vec<Uuid>,
    pub fallback_strategy: Option<String>,
    pub tool_profile_id: Option<Uuid>,
    pub default_execution_mode: AiExecutionModeGql,
    pub is_active: bool,
    pub metadata: Option<String>,
}

#[derive(Debug, Clone, InputObject)]
pub struct ResumeAiApprovalInputGql {
    pub approved: bool,
    pub reason: Option<String>,
}

pub fn parse_metadata(value: Option<String>) -> Result<serde_json::Value, async_graphql::Error> {
    match value {
        Some(value) if !value.trim().is_empty() => {
            serde_json::from_str(&value).map_err(|err| async_graphql::Error::new(err.to_string()))
        }
        _ => Ok(serde_json::json!({})),
    }
}
