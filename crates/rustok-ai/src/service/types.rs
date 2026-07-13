use chrono::{DateTime, Utc};
use once_cell::sync::Lazy;
use rustok_api::Permission;
use rustok_core::registry::ModuleRegistry;
use rustok_outbox::TransactionalEventBus;
use rustok_secrets::{SecretRef, SecretResolverRegistry};
use rustok_storage::StorageService;
use sea_orm::DatabaseConnection;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::{
    collections::HashMap,
    sync::{Arc, Mutex},
};
use tokio::sync::watch;
use uuid::Uuid;

use crate::model::{
    AiRunDecisionTrace, ChatMessageRole, ExecutionMode, ExecutionOverride, ProviderCapability,
    ProviderUsagePolicy, ToolCall, ToolTrace,
};
use crate::{ProviderSlug, ProviderTargetId};

static AI_RUN_CANCELLATIONS: Lazy<Arc<Mutex<HashMap<Uuid, watch::Sender<()>>>>> =
    Lazy::new(|| Arc::new(Mutex::new(HashMap::new())));

#[derive(Clone)]
pub struct SharedAiModuleRegistry(pub ModuleRegistry);

#[derive(Clone)]
pub struct SharedAiSecretResolverRegistry(pub SecretResolverRegistry);

#[derive(Clone)]
pub struct SharedAiEgressPolicy(pub crate::ProviderEgressPolicy);

#[derive(Clone)]
pub struct SharedAiProviderTargetCatalog(pub crate::AiProviderTargetCatalog);

#[derive(Clone)]
pub struct AiHostRuntime {
    db: DatabaseConnection,
    event_bus: TransactionalEventBus,
    module_registry: ModuleRegistry,
    storage: Option<StorageService>,
    alloy_runtime: Option<alloy::SharedAlloyRuntime>,
    secret_registry: SecretResolverRegistry,
    egress_policy: crate::ProviderEgressPolicy,
    provider_targets: crate::AiProviderTargetCatalog,
    cancellations: Arc<Mutex<HashMap<Uuid, watch::Sender<()>>>>,
}

impl AiHostRuntime {
    pub fn new(
        db: DatabaseConnection,
        event_bus: TransactionalEventBus,
        module_registry: ModuleRegistry,
    ) -> Self {
        Self {
            db,
            event_bus,
            module_registry,
            storage: None,
            alloy_runtime: None,
            secret_registry: SecretResolverRegistry::builder().build(),
            egress_policy: crate::ProviderEgressPolicy::default(),
            provider_targets: crate::AiProviderTargetCatalog::from_environment()
                .expect("invalid deployment AI provider target configuration"),
            cancellations: Arc::clone(&AI_RUN_CANCELLATIONS),
        }
    }

    pub fn with_storage(mut self, storage: Option<StorageService>) -> Self {
        self.storage = storage;
        self
    }

    pub fn with_alloy_runtime(mut self, alloy_runtime: Option<alloy::SharedAlloyRuntime>) -> Self {
        self.alloy_runtime = alloy_runtime;
        self
    }

    pub fn with_secret_registry(mut self, secret_registry: SecretResolverRegistry) -> Self {
        self.secret_registry = secret_registry;
        self
    }

    pub fn secret_registry(&self) -> &SecretResolverRegistry {
        &self.secret_registry
    }

    pub fn with_egress_policy(mut self, egress_policy: crate::ProviderEgressPolicy) -> Self {
        self.egress_policy = egress_policy;
        self
    }

    pub fn egress_policy(&self) -> &crate::ProviderEgressPolicy {
        &self.egress_policy
    }

    pub fn with_provider_targets(
        mut self,
        provider_targets: crate::AiProviderTargetCatalog,
    ) -> Self {
        self.provider_targets = provider_targets;
        self
    }

    pub fn provider_targets(&self) -> &crate::AiProviderTargetCatalog {
        &self.provider_targets
    }

    pub fn register_run_cancellation(&self, run_id: Uuid) -> watch::Receiver<()> {
        let (sender, receiver) = watch::channel(());
        self.cancellations
            .lock()
            .expect("AI cancellation registry mutex poisoned")
            .insert(run_id, sender);
        receiver
    }

    pub fn cancel_active_run(&self, run_id: Uuid) {
        if let Some(sender) = self
            .cancellations
            .lock()
            .expect("AI cancellation registry mutex poisoned")
            .remove(&run_id)
        {
            let _ = sender.send(());
        }
    }

    pub fn complete_run_cancellation(&self, run_id: Uuid) {
        self.cancellations
            .lock()
            .expect("AI cancellation registry mutex poisoned")
            .remove(&run_id);
    }

    pub fn db(&self) -> &DatabaseConnection {
        &self.db
    }

    pub fn db_clone(&self) -> DatabaseConnection {
        self.db.clone()
    }

    pub fn event_bus(&self) -> TransactionalEventBus {
        self.event_bus.clone()
    }

    pub fn module_registry(&self) -> ModuleRegistry {
        self.module_registry.clone()
    }

    pub fn storage(&self) -> Option<StorageService> {
        self.storage.clone()
    }

    pub fn scoped_alloy_runtime(&self, tenant_id: Uuid) -> Option<alloy::ScopedAlloyRuntime> {
        self.alloy_runtime
            .as_ref()
            .map(|runtime| runtime.0.scoped(tenant_id))
    }
}

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
    pub provider_target_id: ProviderTargetId,
    pub model: String,
    pub credential_refs: BTreeMap<String, SecretRef>,
    pub temperature: Option<f32>,
    pub max_tokens: Option<i32>,
    pub capabilities: Vec<ProviderCapability>,
    pub usage_policy: ProviderUsagePolicy,
    pub metadata: serde_json::Value,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UpdateAiProviderProfileInput {
    pub display_name: String,
    pub provider_target_id: ProviderTargetId,
    pub model: String,
    pub credential_refs: BTreeMap<String, SecretRef>,
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
    pub model_override: Option<String>,
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

/// Resolves an owner-declared workflow stage gate before it may be claimed by
/// the scheduler. This is distinct from an approval request emitted by an
/// already running model tool call.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResolveAiAgentWorkflowStageApprovalInput {
    pub approved: bool,
    pub reason: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreateAiAgentPrincipalInput {
    pub slug: String,
    pub descriptor_owner: String,
    pub descriptor_slug: String,
    pub metadata: serde_json::Value,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UpdateAiAgentPrincipalInput {
    pub metadata: serde_json::Value,
    pub is_active: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreateAiAgentModelAssignmentInput {
    pub agent_principal_id: Uuid,
    pub provider_profile_id: Uuid,
    pub model_override: Option<String>,
    pub execution_mode: ExecutionMode,
    pub metadata: serde_json::Value,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UpdateAiAgentModelAssignmentInput {
    pub model_override: Option<String>,
    pub execution_mode: ExecutionMode,
    pub metadata: serde_json::Value,
    pub is_active: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreateAiAgentWorkflowRunInput {
    pub workflow_owner: String,
    pub workflow_slug: String,
    /// Maps each owner-declared stage id to its tenant-scoped agent principal.
    pub stage_principal_ids: BTreeMap<String, Uuid>,
    /// Maps each owner-declared stage id to an active assignment for that principal.
    pub stage_model_assignment_ids: BTreeMap<String, Uuid>,
    /// Owner-validated task input for each stage execution binding.
    pub stage_input_payloads: BTreeMap<String, serde_json::Value>,
    pub input_payload: serde_json::Value,
    pub metadata: serde_json::Value,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AiProviderProfileRecord {
    pub id: Uuid,
    pub slug: String,
    pub display_name: String,
    pub provider_slug: ProviderSlug,
    pub provider_target_id: ProviderTargetId,
    pub model: String,
    pub credential_refs: BTreeMap<String, SecretRef>,
    pub temperature: Option<f32>,
    pub max_tokens: Option<i32>,
    pub is_active: bool,
    pub has_credentials: bool,
    pub capabilities: Vec<ProviderCapability>,
    pub usage_policy: ProviderUsagePolicy,
    pub metadata: serde_json::Value,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AiAgentPrincipalRecord {
    pub id: Uuid,
    pub slug: String,
    pub descriptor_owner: String,
    pub descriptor_slug: String,
    pub role_slugs: Vec<String>,
    pub permission_slugs: Vec<String>,
    pub is_active: bool,
    pub metadata: serde_json::Value,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AiAgentModelAssignmentRecord {
    pub id: Uuid,
    pub agent_principal_id: Uuid,
    pub provider_profile_id: Uuid,
    pub model_override: Option<String>,
    pub execution_mode: ExecutionMode,
    pub is_active: bool,
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
    pub provider_slug: ProviderSlug,
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
    pub approval_batch_id: String,
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
