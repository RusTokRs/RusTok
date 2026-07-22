pub mod agent;
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
pub mod rag;
pub mod router;
#[cfg(feature = "server")]
mod runtime_extensions;
#[cfg(feature = "server")]
pub mod scheduler;
#[cfg(feature = "server")]
pub mod service;
#[cfg(feature = "server")]
pub mod streaming;

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
pub use mcp::{McpClientAdapter, ToolExecutionResult};
#[cfg(feature = "server")]
pub use metrics::{AiMetricBucket, AiRuntimeMetricsSnapshot};
#[cfg(feature = "server")]
pub use migrations::AiMigrationSource;
pub use model::{
    AiAlloyTaskInput, AiBlogDraftTaskInput, AiImageAssetTaskInput, AiProductCopyTaskInput,
    AiProviderConfig, AiRunDecisionTrace, AiRunRequest, ChatMessage, ChatMessageRole,
    DirectExecutionTarget, ExecutionMode, ExecutionOverride, PendingApproval, ProviderCapability,
    ProviderChatRequest, ProviderChatResponse, ProviderImageRequest, ProviderImageResponse,
    ProviderStreamEmitter, ProviderStreamEvent, ProviderStructuredRequest, ProviderTestResult,
    ProviderUsage, ProviderUsagePolicy, RuntimeOutcome, RuntimeRequest, TaskProfile, ToolCall,
    ToolDefinition, ToolTrace, default_provider_capabilities,
};
pub use policy::ToolExecutionPolicy;
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
    AiSendMessageResult, AiTaskProfileRecord, AiToolProfileRecord,
    CreateAiAgentModelAssignmentInput, CreateAiAgentPrincipalInput, CreateAiAgentWorkflowRunInput,
    CreateAiProviderProfileInput, CreateAiTaskProfileInput, CreateAiToolProfileInput,
    ResolveAiAgentWorkflowStageApprovalInput, ResumeAiApprovalInput, RunAiTaskJobInput,
    SendAiChatMessageInput, SharedAiEgressPolicy, SharedAiOrderStatusPort,
    SharedAiProductCatalogReadPort, SharedAiProviderTargetCatalog, SharedAiRagRetrievalPort,
    SharedAiSecretResolverRegistry, StartAiChatSessionInput, UpdateAiAgentModelAssignmentInput,
    UpdateAiAgentPrincipalInput, UpdateAiProviderProfileInput, UpdateAiTaskProfileInput,
    UpdateAiToolProfileInput, ai_host_runtime_from_context,
};
#[cfg(feature = "server")]
pub use streaming::{AiRunStreamEvent, AiRunStreamEventKind, AiRunStreamHub, ai_run_stream_hub};

#[cfg(feature = "server")]
pub struct AiModule;

#[cfg(feature = "server")]
impl rustok_core::MigrationSource for AiModule {
    fn migrations(&self) -> Vec<Box<dyn sea_orm_migration::MigrationTrait>> {
        migrations::migrations()
    }
}

#[cfg(feature = "server")]
#[async_trait::async_trait]
impl rustok_core::RusToKModule for AiModule {
    fn slug(&self) -> &'static str {
        "ai"
    }

    fn name(&self) -> &'static str {
        "AI"
    }

    fn description(&self) -> &'static str {
        "Rig-based AI orchestration capability"
    }

    fn version(&self) -> &'static str {
        env!("CARGO_PKG_VERSION")
    }

    /// AI is composed at deployment scope. Tenant profiles and principals are
    /// still tenant-scoped, but a tenant-module toggle must never remove the
    /// generic runtime handles or durable worker from a running deployment.
    fn kind(&self) -> rustok_core::ModuleKind {
        rustok_core::ModuleKind::Core
    }

    fn register_runtime_extensions(&self, extensions: &mut rustok_core::ModuleRuntimeExtensions) {
        let deployment = runtime_extensions::AiDeploymentRuntime::from_environment()
            .unwrap_or_else(|error| {
                panic!("invalid deployment-owned AI runtime configuration: {error}")
            });
        extensions.insert(deployment.secret_registry);
        extensions.insert(deployment.egress_policy);
        extensions.insert(deployment.provider_targets);
        extensions
            .get_or_insert_with::<rustok_runtime::ModuleWorkRegistrations, _>(Default::default)
            .register(std::sync::Arc::new(
                scheduler::AiAgentWorkflowWorkRegistration,
            ));
    }
}

#[cfg(all(test, feature = "server"))]
mod module_tests {
    use rustok_core::{ModuleKind, RusToKModule};

    #[test]
    fn ai_module_is_deployment_scoped_and_globally_active() {
        let module = super::AiModule;
        assert_eq!(module.slug(), "ai");
        assert_eq!(module.kind(), ModuleKind::Core);
    }
}
