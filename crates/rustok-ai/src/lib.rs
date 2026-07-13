#[cfg(feature = "server")]
pub mod direct;
pub mod agent;
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
pub use engine::{
    embed, rerank, EmbeddingRequest, EmbeddingResponse, RerankItem, RerankRequest, RerankResponse,
    RigAgentDriver,
};
#[cfg(feature = "server")]
pub use engine::{inference_for_slug, InferenceEngine};
pub use engine::{
    provider_catalog, provider_catalog_entry, ProviderCatalogEntry, ProviderConfigField,
    ProviderDefaultSetting, ProviderFeature, ProviderFieldKind, ProviderSlug, ProviderTargetAuth,
    ProviderTargetId,
};
#[cfg(feature = "server")]
pub use engine::{
    provider_factory_supports, AiProviderTarget, AiProviderTargetCatalog, ProviderEgressPolicy,
};
pub use error::{AiError, AiResult};
pub use agent::{
    AgentCatalog, AgentDescriptor, AgentKind, AgentPrincipal, AgentWorkflowDescriptor,
    AgentWorkflowStage,
};
#[cfg(feature = "server")]
pub use agent::alloy_agent_catalog;
#[cfg(all(feature = "graphql", feature = "server"))]
pub use graphql_runtime::{
    attach_schema_data, AiGraphqlRuntimeData, SeaOrmAiGraphqlRoleSlugProvider,
};
#[cfg(feature = "graphql")]
pub use graphql_runtime::{
    AiGraphqlRoleSlugProvider, AiGraphqlRoleSlugProviderHandle, AI_GRAPHQL_CONTRIBUTION,
};
pub use mcp::{McpClientAdapter, ToolExecutionResult};
#[cfg(feature = "server")]
pub use metrics::{AiMetricBucket, AiRuntimeMetricsSnapshot};
#[cfg(feature = "server")]
pub use migrations::AiMigrationSource;
pub use model::{
    default_provider_capabilities, AiAlloyOperation, AiAlloyTaskInput, AiBlogDraftTaskInput,
    AiImageAssetTaskInput, AiProductCopyTaskInput, AiProviderConfig, AiRunDecisionTrace,
    AiRunRequest, ChatMessage, ChatMessageRole, DirectExecutionTarget, ExecutionMode,
    ExecutionOverride, PendingApproval, ProviderCapability, ProviderChatRequest,
    ProviderChatResponse, ProviderImageRequest, ProviderImageResponse, ProviderStreamEmitter,
    ProviderStreamEvent, ProviderStructuredRequest, ProviderTestResult, ProviderUsage,
    ProviderUsagePolicy, RuntimeOutcome, RuntimeRequest, TaskProfile, ToolCall, ToolDefinition,
    ToolTrace,
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
    SharedAiEgressPolicy, SharedAiModuleRegistry, SharedAiProviderTargetCatalog,
    SharedAiSecretResolverRegistry, StartAiChatSessionInput, UpdateAiProviderProfileInput,
    UpdateAiTaskProfileInput, UpdateAiToolProfileInput,
};
#[cfg(feature = "server")]
pub use streaming::{ai_run_stream_hub, AiRunStreamEvent, AiRunStreamEventKind, AiRunStreamHub};
