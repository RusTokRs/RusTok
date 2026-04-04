#[cfg(feature = "server")]
pub mod direct;
#[cfg(feature = "server")]
pub mod entities;
pub mod error;
pub mod mcp;
#[cfg(feature = "server")]
pub mod metrics;
pub mod model;
pub mod policy;
pub mod provider;
pub mod router;
pub mod runtime;
#[cfg(feature = "server")]
pub mod service;
#[cfg(feature = "server")]
pub mod streaming;

#[cfg(feature = "server")]
pub use direct::{
    AlloyScriptAssistHandler, BlogDraftHandler, DirectExecutionRegistry, DirectExecutionRequest,
    DirectExecutionResult, DirectTaskHandler, MediaImageAssetHandler, ProductCopyHandler,
};
pub use error::{AiError, AiResult};
pub use mcp::{McpClientAdapter, ToolExecutionResult};
#[cfg(feature = "server")]
pub use metrics::{AiMetricBucket, AiRuntimeMetricsSnapshot};
pub use model::{
    AiAlloyOperation, AiAlloyTaskInput, AiBlogDraftTaskInput, AiImageAssetTaskInput,
    AiProductCopyTaskInput, AiProviderConfig, AiRunDecisionTrace, AiRunRequest, ChatMessage,
    ChatMessageRole, DirectExecutionTarget, ExecutionMode, ExecutionOverride, PendingApproval,
    ProviderCapability, ProviderChatRequest, ProviderChatResponse, ProviderImageRequest,
    ProviderImageResponse, ProviderKind, ProviderStreamEmitter, ProviderStreamEvent,
    ProviderTestResult, ProviderUsagePolicy, RuntimeOutcome, RuntimeRequest, TaskProfile, ToolCall,
    ToolDefinition, ToolTrace,
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
    AiRecentRunRecord, AiSendMessageResult, AiTaskProfileRecord, AiToolProfileRecord,
    CreateAiProviderProfileInput, CreateAiTaskProfileInput, CreateAiToolProfileInput,
    ResumeAiApprovalInput, RunAiTaskJobInput, SendAiChatMessageInput, SharedAiModuleRegistry,
    StartAiChatSessionInput, UpdateAiProviderProfileInput, UpdateAiTaskProfileInput,
    UpdateAiToolProfileInput,
};
#[cfg(feature = "server")]
pub use streaming::{ai_run_stream_hub, AiRunStreamEvent, AiRunStreamEventKind, AiRunStreamHub};
