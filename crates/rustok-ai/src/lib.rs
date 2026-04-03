#[cfg(feature = "server")]
pub mod entities;
pub mod error;
pub mod mcp;
pub mod model;
pub mod policy;
pub mod provider;
pub mod router;
pub mod runtime;
#[cfg(feature = "server")]
pub mod service;

pub use error::{AiError, AiResult};
pub use mcp::{McpClientAdapter, ToolExecutionResult};
pub use model::{
    AiProviderConfig, AiRunDecisionTrace, AiRunRequest, ChatMessage, ChatMessageRole,
    ExecutionMode, ExecutionOverride, PendingApproval, ProviderCapability, ProviderChatRequest,
    ProviderChatResponse, ProviderKind, ProviderTestResult, ProviderUsagePolicy, RuntimeOutcome,
    RuntimeRequest, TaskProfile, ToolCall, ToolDefinition, ToolTrace,
};
pub use policy::ToolExecutionPolicy;
pub use provider::{
    provider_for_kind, AnthropicProvider, GeminiProvider, ModelProvider, OpenAiCompatibleProvider,
};
pub use router::{AiRouter, ResolvedExecutionPlan, RouterProviderProfile};
pub use runtime::AiRuntime;
#[cfg(feature = "server")]
pub use service::{
    AiApprovalRequestRecord, AiChatMessageRecord, AiChatRunRecord, AiChatSessionDetail,
    AiChatSessionSummary, AiManagementService, AiOperatorContext, AiProviderProfileRecord,
    AiSendMessageResult, AiTaskProfileRecord, AiToolProfileRecord, CreateAiProviderProfileInput,
    CreateAiTaskProfileInput, CreateAiToolProfileInput, ResumeAiApprovalInput,
    SendAiChatMessageInput, SharedAiModuleRegistry, StartAiChatSessionInput,
    UpdateAiProviderProfileInput, UpdateAiTaskProfileInput, UpdateAiToolProfileInput,
};
