//! # rustok-ai
//!
//! AI orchestration capability and operator control plane for the RusToK platform.
//!
//! Exposes Rig-based agent execution, model providers, vector store integrations,
//! MCP tool coordination, and structured LLM workflows.

pub mod module;
pub use module::AiModule;

#[cfg(feature = "server")]
mod accounting;
pub mod agent;
#[cfg(feature = "server")]
mod agent_safety;
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
pub mod ports;
pub mod rag;
pub mod router;
#[cfg(feature = "server")]
mod runtime_extensions;
#[cfg(feature = "server")]
mod rustok_blog;
#[cfg(feature = "server")]
pub mod scheduler;
#[cfg(feature = "server")]
pub mod service;
#[cfg(feature = "server")]
pub mod streaming;
#[cfg(feature = "server")]
mod structured;
#[cfg(all(test, feature = "server"))]
mod structured_live_tests;
#[cfg(feature = "server")]
mod structured_result;
#[cfg(feature = "server")]
mod structured_runtime;
#[cfg(all(test, feature = "server"))]
mod structured_test_support;

#[cfg(feature = "server")]
pub use agent::agent_catalog;
pub use agent::{
    AgentCatalog, AgentDescriptor, AgentKind, AgentPrincipal, AgentStageStatus,
    AgentWorkflowDescriptor, AgentWorkflowStage, AgentWorkflowStatus,
};
#[cfg(feature = "server")]
pub use direct::{
    AlloyScriptAssistHandler, BlogDraftHandler, DirectExecutionRegistry, DirectExecutionRequest,
    DirectExecutionResult, DirectTaskHandler, MediaImageAssetHandler, ProductCopyHandler,
};
#[cfg(feature = "server")]
pub use engine::{
    AiProviderTarget, AiProviderTargetCatalog, ProviderEgressPolicy, provider_factory_supports,
};
#[cfg(feature = "server")]
pub use engine::{
    EmbeddingRequest, EmbeddingResponse, RerankItem, RerankRequest, RerankResponse, RigAgentDriver,
    embed, rerank,
};
#[cfg(feature = "server")]
pub use engine::{InferenceEngine, inference_for_slug};
pub use engine::{
    ProviderCatalogEntry, ProviderConfigField, ProviderDefaultSetting, ProviderFeature,
    ProviderFieldKind, ProviderSlug, ProviderTargetAuth, ProviderTargetId, provider_catalog,
    provider_catalog_entry,
};
pub use error::{AiError, AiResult};
#[cfg(feature = "graphql")]
pub use graphql_runtime::AI_GRAPHQL_CONTRIBUTION;
#[cfg(all(feature = "graphql", feature = "server"))]
pub use graphql_runtime::{AiGraphqlRuntimeData, attach_schema_data};
pub use mcp::{McpClientAdapter, ToolExecutionResult, ToolSourceLineage};
#[cfg(feature = "server")]
pub use metrics::{AiMetricBucket, AiRuntimeMetricsSnapshot};
#[cfg(feature = "server")]
pub use migrations::AiMigrationSource;
pub use model::{
    AgentPromptTemplateEvidence, AgentUsageEvidence, AiAlloyTaskInput, AiBlogDraftTaskInput,
    AiImageAssetTaskInput, AiProductCopyTaskInput, AiProviderConfig, AiRunDecisionTrace,
    AiRunRequest, ChatMessage, ChatMessageRole, DirectExecutionTarget, ExecutionMode,
    ExecutionOverride, PendingApproval, ProviderCapability, ProviderChatRequest,
    ProviderChatResponse, ProviderImageRequest, ProviderImageResponse, ProviderStreamEmitter,
    ProviderStreamEvent, ProviderStructuredRequest, ProviderStructuredResponse, ProviderTestResult,
    ProviderUsage, ProviderUsagePolicy, RuntimeOutcome, RuntimeRequest, TaskProfile, ToolCall,
    ToolDefinition, ToolOperationClass, ToolPolicyEvidence, ToolTrace,
    default_provider_capabilities,
};
pub use policy::ToolExecutionPolicy;
pub use ports::{
    AiStructuredTaskAttempt, AiStructuredTaskAvailability, AiStructuredTaskCatalog,
    AiStructuredTaskDescriptor, AiStructuredTaskEstimate, AiStructuredTaskExecution,
    AiStructuredTaskExecutionKey, AiStructuredTaskExecutionRef, AiStructuredTaskHealth,
    AiStructuredTaskHealthRequest, AiStructuredTaskLimits, AiStructuredTaskPort,
    AiStructuredTaskRequest, AiStructuredTaskRequestBinding, AiStructuredTaskStatus,
    AiStructuredTaskUsage, AiTaskDataClassification, MAX_STRUCTURED_TASK_EVIDENCE_ENTRIES,
    MAX_STRUCTURED_TASK_INPUT_BYTES, MAX_STRUCTURED_TASK_OUTPUT_BYTES,
    MAX_STRUCTURED_TASK_SCHEMA_BYTES, MAX_STRUCTURED_TASK_SYSTEM_PROMPT_BYTES,
};
#[cfg(feature = "server")]
pub use rag::RigRagEmbeddingProvider;
pub use rag::{
    RagAtom, RagCandidate, RagChunk, RagChunkingPolicy, RagCitation, RagContext, RagCoordinator,
    RagDocument, RagEmbedding, RagEmbeddingCoordinator, RagEmbeddingPort, RagEmbeddingRequest,
    RagError, RagExpandRequest, RagIngestRequest, RagIngestResult, RagIngestionCoordinator,
    RagIngestionPort, RagResult, RagRetrievalPort, RagRetrievalStrategy, RagSearchRequest,
    RagSourceRef, chunk_document,
};
pub use router::{AiRouter, ResolvedExecutionPlan, RouterProviderProfile};
#[cfg(feature = "server")]
pub use scheduler::{AGENT_WORKFLOW_STAGE_WORKER, AiAgentWorkflowWorkAdapter};
#[cfg(feature = "server")]
pub use service::{
    AiAgentModelAssignmentRecord, AiAgentPrincipalRecord, AiApprovalRequestRecord,
    AiChatMessageRecord, AiChatRunRecord, AiChatSessionDetail, AiChatSessionSummary, AiHostRuntime,
    AiManagementService, AiOperatorContext, AiProviderProfileRecord, AiRecentRunRecord,
    AiSendMessageResult, AiStructuredBudgetPolicyRecord, AiStructuredProviderPolicyRecord,
    AiTaskProfileRecord, AiToolProfileRecord, CreateAiAgentModelAssignmentInput,
    CreateAiAgentPrincipalInput, CreateAiAgentWorkflowRunInput, CreateAiProviderProfileInput,
    CreateAiTaskProfileInput, CreateAiToolProfileInput, PutAiStructuredBudgetPolicyInput,
    PutAiStructuredProviderPolicyInput, ResolveAiAgentWorkflowStageApprovalInput,
    ResumeAiApprovalInput, RunAiTaskJobInput, SendAiChatMessageInput, SharedAiEgressPolicy,
    SharedAiOrderStatusPort, SharedAiProductCatalogReadPort, SharedAiProviderTargetCatalog,
    SharedAiRagRetrievalPort, SharedAiSecretResolverRegistry,
    SharedAiStructuredResultKeyringConfig, StartAiChatSessionInput,
    UpdateAiAgentModelAssignmentInput, UpdateAiAgentPrincipalInput, UpdateAiProviderProfileInput,
    UpdateAiTaskProfileInput, UpdateAiToolProfileInput, ai_host_runtime_from_context,
};
#[cfg(feature = "server")]
pub use streaming::{AiRunStreamEvent, AiRunStreamEventKind, AiRunStreamHub, ai_run_stream_hub};
#[cfg(feature = "server")]
pub use structured_result::AiStructuredResultKeyringConfig;
#[cfg(feature = "server")]
pub use structured_runtime::structured_task_port_from_context;

#[cfg(test)]
mod contract_tests;
