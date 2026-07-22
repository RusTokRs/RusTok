use chrono::{DateTime, Utc};
use rustok_secrets::SecretRef;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::sync::Arc;
use uuid::Uuid;

use crate::engine::ProviderSlug;

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

/// Derives the tenant-facing default capabilities from the deployment's
/// catalogued provider descriptor. Both GraphQL and native transports call this
/// function when an operator leaves the capability selection empty.
pub fn default_provider_capabilities(
    provider_slug: &crate::ProviderSlug,
) -> Vec<ProviderCapability> {
    let Some(descriptor) = crate::provider_catalog_entry(provider_slug) else {
        return Vec::new();
    };
    let mut capabilities = Vec::new();
    for feature in descriptor.features {
        let capability = match feature {
            crate::ProviderFeature::Chat => Some(ProviderCapability::TextGeneration),
            crate::ProviderFeature::StructuredOutput => {
                Some(ProviderCapability::StructuredGeneration)
            }
            crate::ProviderFeature::Image => Some(ProviderCapability::ImageGeneration),
            crate::ProviderFeature::Multimodal => Some(ProviderCapability::MultimodalUnderstanding),
            _ => None,
        };
        if let Some(capability) = capability.filter(|value| !capabilities.contains(value)) {
            capabilities.push(capability);
        }
    }
    capabilities
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
    pub tenant_id: Uuid,
    pub provider_slug: ProviderSlug,
    #[serde(default)]
    pub target_auth: crate::ProviderTargetAuth,
    pub model: String,
    #[serde(default)]
    pub settings: BTreeMap<String, serde_json::Value>,
    #[serde(default)]
    pub credential_refs: BTreeMap<String, SecretRef>,
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

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ToolCall {
    pub id: String,
    pub name: String,
    pub arguments: serde_json::Value,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProviderUsage {
    pub input_tokens: u64,
    pub output_tokens: u64,
    pub total_tokens: u64,
}

impl ProviderUsage {
    pub fn normalized(input_tokens: u64, output_tokens: u64, total_tokens: Option<u64>) -> Self {
        Self {
            input_tokens,
            output_tokens,
            total_tokens: total_tokens
                .unwrap_or_else(|| input_tokens.saturating_add(output_tokens)),
        }
    }
}

#[cfg(test)]
mod provider_usage_tests {
    use super::{ProviderCapability, ProviderUsage, default_provider_capabilities};
    use crate::ProviderSlug;

    #[test]
    fn derives_missing_total_without_overflow() {
        assert_eq!(ProviderUsage::normalized(3, 5, None).total_tokens, 8);
        assert_eq!(
            ProviderUsage::normalized(u64::MAX, 1, None).total_tokens,
            u64::MAX
        );
        assert_eq!(ProviderUsage::normalized(3, 5, Some(9)).total_tokens, 9);
    }

    #[test]
    fn derives_catalog_defaults_once_for_all_transports() {
        let capabilities = default_provider_capabilities(&ProviderSlug::openai_compatible());
        assert!(capabilities.contains(&ProviderCapability::TextGeneration));
        assert!(capabilities.contains(&ProviderCapability::StructuredGeneration));
        assert_eq!(
            capabilities
                .iter()
                .filter(|value| **value == ProviderCapability::TextGeneration)
                .count(),
            1
        );
    }
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
    pub locale: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProviderStructuredRequest {
    pub request: ProviderChatRequest,
    pub output_schema: serde_json::Value,
}

#[derive(Debug, Clone, PartialEq)]
pub enum ProviderStreamEvent {
    TextDelta(String),
    ToolCall(ToolCall),
    Usage(ProviderUsage),
}

#[derive(Clone)]
pub struct ProviderStreamEmitter {
    inner: Arc<dyn Fn(ProviderStreamEvent) + Send + Sync>,
}

impl ProviderStreamEmitter {
    pub fn new<F>(callback: F) -> Self
    where
        F: Fn(ProviderStreamEvent) + Send + Sync + 'static,
    {
        Self {
            inner: Arc::new(callback),
        }
    }

    pub fn emit(&self, event: ProviderStreamEvent) {
        (self.inner)(event);
    }

    pub fn emit_text_delta(&self, delta: impl Into<String>) {
        self.emit(ProviderStreamEvent::TextDelta(delta.into()));
    }

    pub fn emit_tool_call(&self, tool_call: ToolCall) {
        self.emit(ProviderStreamEvent::ToolCall(tool_call));
    }

    pub fn emit_usage(&self, usage: ProviderUsage) {
        self.emit(ProviderStreamEvent::Usage(usage));
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProviderChatResponse {
    pub assistant_message: ChatMessage,
    pub finish_reason: Option<String>,
    #[serde(default)]
    pub raw_payload: serde_json::Value,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProviderImageRequest {
    pub model: String,
    pub prompt: String,
    pub negative_prompt: Option<String>,
    pub size: Option<String>,
    pub locale: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProviderImageResponse {
    pub bytes: Vec<u8>,
    pub mime_type: String,
    pub revised_prompt: Option<String>,
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

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum DirectExecutionTarget {
    Alloy,
    Media,
    Commerce,
    Blog,
    Moderation,
    Orders,
}

impl DirectExecutionTarget {
    pub const fn slug(&self) -> &'static str {
        match self {
            Self::Alloy => "alloy",
            Self::Media => "media",
            Self::Commerce => "commerce",
            Self::Blog => "blog",
            Self::Moderation => "moderation",
            Self::Orders => "orders",
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct AiRunDecisionTrace {
    pub task_profile_id: Option<Uuid>,
    pub task_profile_slug: Option<String>,
    pub provider_profile_id: Option<Uuid>,
    pub provider_slug: Option<String>,
    pub selected_model: Option<String>,
    pub execution_mode: Option<ExecutionMode>,
    pub execution_target: Option<String>,
    pub requested_locale: Option<String>,
    pub resolved_locale: Option<String>,
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
    pub locale: Option<String>,
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
        pending_approvals: Vec<PendingApproval>,
    },
    Failed {
        appended_messages: Vec<ChatMessage>,
        traces: Vec<ToolTrace>,
        error_message: String,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct AiAlloyTaskInput {
    pub operation: rustok_ai_alloy::AlloyOperation,
    pub script_id: Option<Uuid>,
    pub script_name: Option<String>,
    pub script_source: Option<String>,
    pub runtime_payload_json: Option<String>,
    pub assistant_prompt: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct AiImageAssetTaskInput {
    pub prompt: String,
    pub negative_prompt: Option<String>,
    pub title: Option<String>,
    pub alt_text: Option<String>,
    pub caption: Option<String>,
    pub file_name: Option<String>,
    pub size: Option<String>,
    pub assistant_prompt: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct AiProductCopyTaskInput {
    pub product_id: Uuid,
    pub source_locale: Option<String>,
    pub source_title: Option<String>,
    pub source_description: Option<String>,
    pub source_meta_title: Option<String>,
    pub source_meta_description: Option<String>,
    pub copy_instructions: Option<String>,
    pub assistant_prompt: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct AiBlogDraftTaskInput {
    pub post_id: Option<Uuid>,
    pub source_locale: Option<String>,
    pub source_title: Option<String>,
    pub source_body: Option<String>,
    pub source_excerpt: Option<String>,
    pub source_seo_title: Option<String>,
    pub source_seo_description: Option<String>,
    #[serde(default)]
    pub tags: Vec<String>,
    pub category_id: Option<Uuid>,
    pub featured_image_url: Option<String>,
    pub copy_instructions: Option<String>,
    pub assistant_prompt: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct AiContentModerationTaskInput {
    pub content_id: Option<Uuid>,
    pub content_type: Option<String>,
    pub title: Option<String>,
    pub body: Option<String>,
    pub locale: Option<String>,
    pub assistant_prompt: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct AiProductAttributesTaskInput {
    pub product_id: Uuid,
    pub category_slug: Option<String>,
    #[serde(default)]
    pub image_urls: Vec<String>,
    pub source_title: Option<String>,
    pub source_description: Option<String>,
    pub copy_instructions: Option<String>,
    pub assistant_prompt: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct AiOrderAnalyticsTaskInput {
    #[serde(default)]
    pub order_ids: Vec<Uuid>,
    pub date_from: Option<DateTime<Utc>>,
    pub date_to: Option<DateTime<Utc>>,
    pub focus: Option<String>,
    pub assistant_prompt: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct AiOrderOpsAssistantTaskInput {
    pub order_id: Uuid,
    pub recommended_action: Option<String>,
    pub context: Option<String>,
    pub assistant_prompt: Option<String>,
}
