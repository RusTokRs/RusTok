#[cfg(feature = "server")]
pub mod direct;
pub mod engine;
#[cfg(feature = "server")]
pub mod entities;
pub mod error;
#[cfg(feature = "graphql")]
pub mod graphql;
#[cfg(feature = "graphql")]
pub mod graphql_runtime;
pub mod mcp;
#[cfg(feature = "server")]
pub mod metrics;
#[cfg(feature = "server")]
pub mod migrations;
pub mod model;
pub mod policy;
pub mod router;
#[cfg(feature = "server")]
pub mod service;
#[cfg(feature = "server")]
pub mod streaming;

#[cfg(feature = "server")]
pub use direct::{
    AlloyScriptAssistHandler, BlogDraftHandler, DirectExecutionRegistry, DirectExecutionRequest,
    DirectExecutionResult, DirectTaskHandler, MediaImageAssetHandler, ProductCopyHandler,
};
#[cfg(feature = "server")]
pub use engine::{inference_for_slug, InferenceEngine};
#[cfg(feature = "server")]
pub use engine::{
    embed, rerank, EmbeddingRequest, EmbeddingResponse, RerankItem, RerankRequest,
    RerankResponse, RigAgentDriver,
};
pub use engine::{
    provider_catalog, provider_catalog_entry, ProviderCatalogEntry, ProviderConfigField,
    ProviderDefaultSetting,
    ProviderFeature, ProviderFieldKind, ProviderSlug,
};
#[cfg(feature = "server")]
pub use engine::{provider_factory_supports, ProviderEgressPolicy};
pub use error::{AiError, AiResult};
#[cfg(feature = "graphql")]
pub use graphql_runtime::{AiGraphqlRoleSlugProvider, AiGraphqlRoleSlugProviderHandle};
pub use mcp::{McpClientAdapter, ToolExecutionResult};
#[cfg(feature = "server")]
pub use metrics::{AiMetricBucket, AiRuntimeMetricsSnapshot};
#[cfg(feature = "server")]
pub use migrations::AiMigrationSource;
pub use model::{
    AiAlloyOperation, AiAlloyTaskInput, AiBlogDraftTaskInput, AiImageAssetTaskInput,
    AiProductCopyTaskInput, AiProviderConfig, AiRunDecisionTrace, AiRunRequest, ChatMessage,
    ChatMessageRole, DirectExecutionTarget, ExecutionMode, ExecutionOverride, PendingApproval,
    ProviderCapability, ProviderChatRequest, ProviderChatResponse, ProviderImageRequest,
    ProviderImageResponse, ProviderStreamEmitter, ProviderStreamEvent, ProviderStructuredRequest,
    ProviderTestResult, ProviderUsagePolicy, RuntimeOutcome, RuntimeRequest, TaskProfile, ToolCall,
    ToolDefinition, ToolTrace,
};
pub use policy::ToolExecutionPolicy;
pub use router::{AiRouter, ResolvedExecutionPlan, RouterProviderProfile};
#[cfg(feature = "server")]
pub use service::{
    AiApprovalRequestRecord, AiChatMessageRecord, AiChatRunRecord, AiChatSessionDetail,
    AiChatSessionSummary, AiHostRuntime, AiManagementService, AiOperatorContext,
    AiProviderProfileRecord, AiRecentRunRecord, AiSendMessageResult, AiTaskProfileRecord,
    AiToolProfileRecord, CreateAiProviderProfileInput, CreateAiTaskProfileInput,
    CreateAiToolProfileInput, ResumeAiApprovalInput, RunAiTaskJobInput, SendAiChatMessageInput,
    SharedAiEgressPolicy, SharedAiModuleRegistry, SharedAiSecretResolverRegistry,
    StartAiChatSessionInput, UpdateAiProviderProfileInput,
    UpdateAiTaskProfileInput, UpdateAiToolProfileInput,
};
#[cfg(feature = "server")]
pub use streaming::{ai_run_stream_hub, AiRunStreamEvent, AiRunStreamEventKind, AiRunStreamHub};
