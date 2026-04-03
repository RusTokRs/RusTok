#![cfg(feature = "server")]

use std::collections::HashMap;
use std::sync::{Arc, Mutex};

use chrono::{DateTime, Utc};
use loco_rs::app::AppContext;
use once_cell::sync::Lazy;
use rustok_core::permissions::Permission;
use rustok_core::registry::ModuleRegistry;
use rustok_mcp::alloy_tools::{
    self, AlloyMcpState, ApplyModuleScaffoldRequest, CreateScriptRequest, DeleteScriptRequest,
    GetScriptRequest, ListScriptsRequest, ReviewModuleScaffoldRequest, RunScriptRequest,
    UpdateScriptRequest, ValidateScriptRequest, TOOL_ALLOY_APPLY_MODULE_SCAFFOLD,
    TOOL_ALLOY_CREATE_SCRIPT, TOOL_ALLOY_DELETE_SCRIPT, TOOL_ALLOY_GET_SCRIPT,
    TOOL_ALLOY_LIST_ENTITY_TYPES, TOOL_ALLOY_LIST_SCRIPTS, TOOL_ALLOY_REVIEW_MODULE_SCAFFOLD,
    TOOL_ALLOY_RUN_SCRIPT, TOOL_ALLOY_SCAFFOLD_MODULE, TOOL_ALLOY_SCRIPT_HELPERS,
    TOOL_ALLOY_UPDATE_SCRIPT, TOOL_ALLOY_VALIDATE_SCRIPT,
};
use rustok_mcp::tools::{
    self, McpHealthResponse, McpState, McpToolResponse, ModuleLookupRequest, ModuleQueryRequest,
    TOOL_BLOG_MODULE, TOOL_CONTENT_MODULE, TOOL_FORUM_MODULE, TOOL_LIST_MODULES, TOOL_MCP_HEALTH,
    TOOL_MCP_WHOAMI, TOOL_MODULE_DETAILS, TOOL_MODULE_EXISTS, TOOL_PAGES_MODULE,
    TOOL_QUERY_MODULES,
};
use rustok_mcp::{
    default_tool_requirement, McpAccessContext, McpAccessPolicy, McpActorType, McpIdentity,
    StagedModuleScaffold,
};
use schemars::schema_for;
use sea_orm::{
    ActiveModelTrait, ActiveValue::Set, ColumnTrait, Condition, DatabaseConnection, EntityTrait,
    PaginatorTrait, QueryFilter, QueryOrder, TransactionTrait,
};
use serde::{Deserialize, Serialize};
use serde_json::json;
use uuid::Uuid;

use crate::entities::{
    ai_approval_requests, ai_chat_messages, ai_chat_runs, ai_chat_sessions, ai_provider_profiles,
    ai_task_profiles, ai_tool_profiles, ai_tool_traces,
};
use crate::mcp::{McpClientAdapter, ToolExecutionResult};
use crate::model::{
    AiProviderConfig, AiRunDecisionTrace, ChatMessage, ChatMessageRole, ExecutionMode,
    ExecutionOverride, PendingApproval, ProviderCapability, ProviderKind, ProviderTestResult,
    ProviderUsagePolicy, RuntimeOutcome, RuntimeRequest, TaskProfile, ToolCall, ToolDefinition,
    ToolTrace,
};
use crate::policy::ToolExecutionPolicy;
use crate::provider::{provider_for_kind, ModelProvider};
use crate::router::{AiRouter, RouterProviderProfile};
use crate::runtime::AiRuntime;
use crate::{AiError, AiResult};

static STAGED_SCAFFOLDS: Lazy<Arc<Mutex<HashMap<Uuid, StagedModuleScaffold>>>> =
    Lazy::new(|| Arc::new(Mutex::new(HashMap::new())));

#[derive(Clone)]
pub struct SharedAiModuleRegistry(pub ModuleRegistry);

#[derive(Debug, Clone)]
pub struct AiOperatorContext {
    pub tenant_id: Uuid,
    pub user_id: Uuid,
    pub permissions: Vec<Permission>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreateAiProviderProfileInput {
    pub slug: String,
    pub display_name: String,
    pub provider_kind: ProviderKind,
    pub base_url: String,
    pub model: String,
    pub api_key_secret: Option<String>,
    pub temperature: Option<f32>,
    pub max_tokens: Option<i32>,
    pub capabilities: Vec<ProviderCapability>,
    pub usage_policy: ProviderUsagePolicy,
    pub metadata: serde_json::Value,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UpdateAiProviderProfileInput {
    pub display_name: String,
    pub base_url: String,
    pub model: String,
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
    pub initial_message: Option<String>,
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

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AiProviderProfileRecord {
    pub id: Uuid,
    pub slug: String,
    pub display_name: String,
    pub provider_kind: ProviderKind,
    pub base_url: String,
    pub model: String,
    pub temperature: Option<f32>,
    pub max_tokens: Option<i32>,
    pub is_active: bool,
    pub has_secret: bool,
    pub capabilities: Vec<ProviderCapability>,
    pub usage_policy: ProviderUsagePolicy,
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
pub struct AiApprovalRequestRecord {
    pub id: Uuid,
    pub session_id: Uuid,
    pub run_id: Uuid,
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

pub struct AiManagementService;

impl AiManagementService {
    pub async fn list_provider_profiles(
        db: &DatabaseConnection,
        tenant_id: Uuid,
    ) -> AiResult<Vec<AiProviderProfileRecord>> {
        let profiles = ai_provider_profiles::Entity::find()
            .filter(ai_provider_profiles::Column::TenantId.eq(tenant_id))
            .order_by_asc(ai_provider_profiles::Column::DisplayName)
            .all(db)
            .await
            .map_err(db_err)?;
        Ok(profiles.into_iter().map(map_provider_profile).collect())
    }

    pub async fn get_provider_profile(
        db: &DatabaseConnection,
        tenant_id: Uuid,
        id: Uuid,
    ) -> AiResult<Option<AiProviderProfileRecord>> {
        let profile = ai_provider_profiles::Entity::find_by_id(id)
            .filter(ai_provider_profiles::Column::TenantId.eq(tenant_id))
            .one(db)
            .await
            .map_err(db_err)?;
        Ok(profile.map(map_provider_profile))
    }

    pub async fn create_provider_profile(
        db: &DatabaseConnection,
        operator: &AiOperatorContext,
        input: CreateAiProviderProfileInput,
    ) -> AiResult<AiProviderProfileRecord> {
        validate_slug(&input.slug)?;
        let profile = ai_provider_profiles::ActiveModel {
            id: Set(Uuid::new_v4()),
            tenant_id: Set(operator.tenant_id),
            slug: Set(input.slug),
            display_name: Set(input.display_name),
            provider_kind: Set(provider_kind_slug(input.provider_kind).to_string()),
            base_url: Set(normalize_base_url(&input.base_url)),
            model: Set(input.model),
            api_key_secret: Set(input
                .api_key_secret
                .filter(|value| !value.trim().is_empty())),
            temperature: Set(input.temperature),
            max_tokens: Set(input.max_tokens),
            is_active: Set(true),
            capabilities: Set(capability_json_array(input.capabilities)),
            allowed_task_profiles: Set(to_json_array(input.usage_policy.allowed_task_profiles)?),
            denied_task_profiles: Set(to_json_array(input.usage_policy.denied_task_profiles)?),
            restricted_role_slugs: Set(to_json_array(input.usage_policy.restricted_role_slugs)?),
            metadata: Set(normalize_metadata(input.metadata)),
            created_by: Set(Some(operator.user_id)),
            updated_by: Set(Some(operator.user_id)),
            created_at: sea_orm::ActiveValue::NotSet,
            updated_at: sea_orm::ActiveValue::NotSet,
        }
        .insert(db)
        .await
        .map_err(db_err)?;
        Ok(map_provider_profile(profile))
    }

    pub async fn update_provider_profile(
        db: &DatabaseConnection,
        operator: &AiOperatorContext,
        id: Uuid,
        input: UpdateAiProviderProfileInput,
    ) -> AiResult<AiProviderProfileRecord> {
        let profile = require_provider_profile(db, operator.tenant_id, id).await?;
        let mut active: ai_provider_profiles::ActiveModel = profile.into();
        active.display_name = Set(input.display_name);
        active.base_url = Set(normalize_base_url(&input.base_url));
        active.model = Set(input.model);
        active.temperature = Set(input.temperature);
        active.max_tokens = Set(input.max_tokens);
        active.is_active = Set(input.is_active);
        active.capabilities = Set(capability_json_array(input.capabilities));
        active.allowed_task_profiles =
            Set(to_json_array(input.usage_policy.allowed_task_profiles)?);
        active.denied_task_profiles = Set(to_json_array(input.usage_policy.denied_task_profiles)?);
        active.restricted_role_slugs =
            Set(to_json_array(input.usage_policy.restricted_role_slugs)?);
        active.metadata = Set(normalize_metadata(input.metadata));
        active.updated_by = Set(Some(operator.user_id));
        active.updated_at = Set(Utc::now().into());
        let saved = active.update(db).await.map_err(db_err)?;
        Ok(map_provider_profile(saved))
    }

    pub async fn rotate_provider_secret(
        db: &DatabaseConnection,
        operator: &AiOperatorContext,
        id: Uuid,
        secret: Option<String>,
    ) -> AiResult<AiProviderProfileRecord> {
        let profile = require_provider_profile(db, operator.tenant_id, id).await?;
        let mut active: ai_provider_profiles::ActiveModel = profile.into();
        active.api_key_secret = Set(secret.filter(|value| !value.trim().is_empty()));
        active.updated_by = Set(Some(operator.user_id));
        active.updated_at = Set(Utc::now().into());
        let saved = active.update(db).await.map_err(db_err)?;
        Ok(map_provider_profile(saved))
    }

    pub async fn deactivate_provider_profile(
        db: &DatabaseConnection,
        operator: &AiOperatorContext,
        id: Uuid,
    ) -> AiResult<AiProviderProfileRecord> {
        let profile = require_provider_profile(db, operator.tenant_id, id).await?;
        let mut active: ai_provider_profiles::ActiveModel = profile.into();
        active.is_active = Set(false);
        active.updated_by = Set(Some(operator.user_id));
        active.updated_at = Set(Utc::now().into());
        let saved = active.update(db).await.map_err(db_err)?;
        Ok(map_provider_profile(saved))
    }

    pub async fn test_provider_profile(
        db: &DatabaseConnection,
        tenant_id: Uuid,
        id: Uuid,
    ) -> AiResult<ProviderTestResult> {
        let profile = require_provider_profile(db, tenant_id, id).await?;
        let provider = provider_for_kind(provider_kind_from_slug(&profile.provider_kind));
        provider.test_connection(&provider_config(&profile)?).await
    }

    pub async fn list_task_profiles(
        db: &DatabaseConnection,
        tenant_id: Uuid,
    ) -> AiResult<Vec<AiTaskProfileRecord>> {
        let profiles = ai_task_profiles::Entity::find()
            .filter(ai_task_profiles::Column::TenantId.eq(tenant_id))
            .order_by_asc(ai_task_profiles::Column::DisplayName)
            .all(db)
            .await
            .map_err(db_err)?;
        Ok(profiles
            .into_iter()
            .map(map_task_profile)
            .collect::<AiResult<Vec<_>>>()?)
    }

    pub async fn create_task_profile(
        db: &DatabaseConnection,
        operator: &AiOperatorContext,
        input: CreateAiTaskProfileInput,
    ) -> AiResult<AiTaskProfileRecord> {
        validate_slug(&input.slug)?;
        let profile = ai_task_profiles::ActiveModel {
            id: Set(Uuid::new_v4()),
            tenant_id: Set(operator.tenant_id),
            slug: Set(input.slug),
            display_name: Set(input.display_name),
            description: Set(input.description),
            target_capability: Set(input.target_capability.slug().to_string()),
            system_prompt: Set(input.system_prompt),
            allowed_provider_profile_ids: Set(uuid_json_array(input.allowed_provider_profile_ids)),
            preferred_provider_profile_ids: Set(uuid_json_array(
                input.preferred_provider_profile_ids,
            )),
            fallback_strategy: Set(normalize_nonempty(input.fallback_strategy, "ordered")),
            tool_profile_id: Set(input.tool_profile_id),
            approval_policy: Set(normalize_metadata(input.approval_policy)),
            default_execution_mode: Set(input.default_execution_mode.slug().to_string()),
            is_active: Set(true),
            metadata: Set(normalize_metadata(input.metadata)),
            created_by: Set(Some(operator.user_id)),
            updated_by: Set(Some(operator.user_id)),
            created_at: sea_orm::ActiveValue::NotSet,
            updated_at: sea_orm::ActiveValue::NotSet,
        }
        .insert(db)
        .await
        .map_err(db_err)?;
        map_task_profile(profile)
    }

    pub async fn update_task_profile(
        db: &DatabaseConnection,
        operator: &AiOperatorContext,
        id: Uuid,
        input: UpdateAiTaskProfileInput,
    ) -> AiResult<AiTaskProfileRecord> {
        let profile = require_task_profile(db, operator.tenant_id, id).await?;
        let mut active: ai_task_profiles::ActiveModel = profile.into();
        active.display_name = Set(input.display_name);
        active.description = Set(input.description);
        active.target_capability = Set(input.target_capability.slug().to_string());
        active.system_prompt = Set(input.system_prompt);
        active.allowed_provider_profile_ids =
            Set(uuid_json_array(input.allowed_provider_profile_ids));
        active.preferred_provider_profile_ids =
            Set(uuid_json_array(input.preferred_provider_profile_ids));
        active.fallback_strategy = Set(normalize_nonempty(input.fallback_strategy, "ordered"));
        active.tool_profile_id = Set(input.tool_profile_id);
        active.approval_policy = Set(normalize_metadata(input.approval_policy));
        active.default_execution_mode = Set(input.default_execution_mode.slug().to_string());
        active.is_active = Set(input.is_active);
        active.metadata = Set(normalize_metadata(input.metadata));
        active.updated_by = Set(Some(operator.user_id));
        active.updated_at = Set(Utc::now().into());
        let saved = active.update(db).await.map_err(db_err)?;
        map_task_profile(saved)
    }

    pub async fn list_tool_profiles(
        db: &DatabaseConnection,
        tenant_id: Uuid,
    ) -> AiResult<Vec<AiToolProfileRecord>> {
        let profiles = ai_tool_profiles::Entity::find()
            .filter(ai_tool_profiles::Column::TenantId.eq(tenant_id))
            .order_by_asc(ai_tool_profiles::Column::DisplayName)
            .all(db)
            .await
            .map_err(db_err)?;
        Ok(profiles.into_iter().map(map_tool_profile).collect())
    }

    pub async fn create_tool_profile(
        db: &DatabaseConnection,
        operator: &AiOperatorContext,
        input: CreateAiToolProfileInput,
    ) -> AiResult<AiToolProfileRecord> {
        validate_slug(&input.slug)?;
        let profile = ai_tool_profiles::ActiveModel {
            id: Set(Uuid::new_v4()),
            tenant_id: Set(operator.tenant_id),
            slug: Set(input.slug),
            display_name: Set(input.display_name),
            description: Set(input.description),
            allowed_tools: Set(to_json_array(input.allowed_tools)?),
            denied_tools: Set(to_json_array(input.denied_tools)?),
            sensitive_tools: Set(to_json_array(input.sensitive_tools)?),
            is_active: Set(true),
            metadata: Set(normalize_metadata(input.metadata)),
            created_by: Set(Some(operator.user_id)),
            updated_by: Set(Some(operator.user_id)),
            created_at: sea_orm::ActiveValue::NotSet,
            updated_at: sea_orm::ActiveValue::NotSet,
        }
        .insert(db)
        .await
        .map_err(db_err)?;
        Ok(map_tool_profile(profile))
    }

    pub async fn update_tool_profile(
        db: &DatabaseConnection,
        operator: &AiOperatorContext,
        id: Uuid,
        input: UpdateAiToolProfileInput,
    ) -> AiResult<AiToolProfileRecord> {
        let profile = require_tool_profile(db, operator.tenant_id, id).await?;
        let mut active: ai_tool_profiles::ActiveModel = profile.into();
        active.display_name = Set(input.display_name);
        active.description = Set(input.description);
        active.allowed_tools = Set(to_json_array(input.allowed_tools)?);
        active.denied_tools = Set(to_json_array(input.denied_tools)?);
        active.sensitive_tools = Set(to_json_array(input.sensitive_tools)?);
        active.is_active = Set(input.is_active);
        active.metadata = Set(normalize_metadata(input.metadata));
        active.updated_by = Set(Some(operator.user_id));
        active.updated_at = Set(Utc::now().into());
        let saved = active.update(db).await.map_err(db_err)?;
        Ok(map_tool_profile(saved))
    }

    pub async fn start_chat_session(
        app_ctx: &AppContext,
        operator: &AiOperatorContext,
        input: StartAiChatSessionInput,
    ) -> AiResult<AiSendMessageResult> {
        let db = &app_ctx.db;
        let task_profile = match input.task_profile_id {
            Some(task_profile_id) => {
                let task_profile =
                    require_task_profile(db, operator.tenant_id, task_profile_id).await?;
                if !task_profile.is_active {
                    return Err(AiError::Validation("task profile is inactive".to_string()));
                }
                Some(task_profile)
            }
            None => None,
        };
        if let Some(tool_profile_id) = input.tool_profile_id {
            let tool_profile =
                require_tool_profile(db, operator.tenant_id, tool_profile_id).await?;
            if !tool_profile.is_active {
                return Err(AiError::Validation("tool profile is inactive".to_string()));
            }
        }
        let providers = list_router_provider_profiles(db, operator.tenant_id).await?;
        let task_profile_record = match task_profile.as_ref() {
            Some(profile) => Some(map_task_profile(profile.clone())?),
            None => None,
        };
        let execution_plan = AiRouter::resolve(
            task_profile_record
                .as_ref()
                .map(task_profile_runtime)
                .as_ref(),
            &providers,
            input.provider_profile_id,
            input.tool_profile_id,
            &ExecutionOverride {
                execution_mode: input.execution_mode,
                ..input.override_config.clone()
            },
            &[],
        )?;

        let txn = db.begin().await.map_err(db_err)?;
        let session = ai_chat_sessions::ActiveModel {
            id: Set(Uuid::new_v4()),
            tenant_id: Set(operator.tenant_id),
            title: Set(input.title),
            provider_profile_id: Set(execution_plan.provider_profile_id),
            task_profile_id: Set(execution_plan.task_profile_id),
            tool_profile_id: Set(execution_plan.tool_profile_id),
            execution_mode: Set(execution_plan.execution_mode.slug().to_string()),
            status: Set("active".to_string()),
            created_by: Set(Some(operator.user_id)),
            metadata: Set(merge_metadata(
                input.metadata,
                json!({ "decision_trace": execution_plan.decision_trace }),
            )),
            created_at: sea_orm::ActiveValue::NotSet,
            updated_at: sea_orm::ActiveValue::NotSet,
        }
        .insert(&txn)
        .await
        .map_err(db_err)?;

        if let Some(initial) = input
            .initial_message
            .filter(|value| !value.trim().is_empty())
        {
            insert_message(
                &txn,
                operator.tenant_id,
                session.id,
                None,
                Some(operator.user_id),
                ChatMessage {
                    role: ChatMessageRole::User,
                    content: Some(initial),
                    name: None,
                    tool_call_id: None,
                    tool_calls: Vec::new(),
                    metadata: json!({}),
                },
            )
            .await?;
        }

        txn.commit().await.map_err(db_err)?;

        if session_has_user_messages(db, operator.tenant_id, session.id).await? {
            Self::execute_latest_turn(app_ctx, operator, session.id).await
        } else {
            let detail = Self::chat_session_detail(db, operator.tenant_id, session.id)
                .await?
                .ok_or_else(|| AiError::Runtime("failed to reload AI chat session".to_string()))?;
            Ok(AiSendMessageResult {
                run: AiChatRunRecord {
                    id: Uuid::nil(),
                    session_id: detail.session.id,
                    provider_profile_id: detail.provider_profile.id,
                    task_profile_id: detail.task_profile.as_ref().map(|value| value.id),
                    tool_profile_id: detail.tool_profile.as_ref().map(|value| value.id),
                    status: "idle".to_string(),
                    model: detail.provider_profile.model.clone(),
                    execution_mode: detail.session.execution_mode,
                    execution_path: detail.session.execution_mode,
                    temperature: detail.provider_profile.temperature,
                    max_tokens: detail.provider_profile.max_tokens,
                    error_message: None,
                    pending_approval_id: None,
                    decision_trace: AiRunDecisionTrace::default(),
                    metadata: json!({}),
                    created_at: Utc::now(),
                    started_at: Utc::now(),
                    completed_at: None,
                    updated_at: Utc::now(),
                },
                session: detail,
            })
        }
    }

    pub async fn send_chat_message(
        app_ctx: &AppContext,
        operator: &AiOperatorContext,
        session_id: Uuid,
        input: SendAiChatMessageInput,
    ) -> AiResult<AiSendMessageResult> {
        let db = &app_ctx.db;
        let session = require_session(db, operator.tenant_id, session_id).await?;
        insert_message(
            db,
            operator.tenant_id,
            session.id,
            None,
            Some(operator.user_id),
            ChatMessage {
                role: ChatMessageRole::User,
                content: Some(input.content),
                name: None,
                tool_call_id: None,
                tool_calls: Vec::new(),
                metadata: json!({}),
            },
        )
        .await?;
        Self::execute_latest_turn(app_ctx, operator, session.id).await
    }

    pub async fn list_chat_sessions(
        db: &DatabaseConnection,
        tenant_id: Uuid,
    ) -> AiResult<Vec<AiChatSessionSummary>> {
        let sessions = ai_chat_sessions::Entity::find()
            .filter(ai_chat_sessions::Column::TenantId.eq(tenant_id))
            .order_by_desc(ai_chat_sessions::Column::UpdatedAt)
            .all(db)
            .await
            .map_err(db_err)?;

        let mut summaries = Vec::with_capacity(sessions.len());
        for session in sessions {
            let latest_run = ai_chat_runs::Entity::find()
                .filter(
                    Condition::all()
                        .add(ai_chat_runs::Column::TenantId.eq(tenant_id))
                        .add(ai_chat_runs::Column::SessionId.eq(session.id)),
                )
                .order_by_desc(ai_chat_runs::Column::CreatedAt)
                .one(db)
                .await
                .map_err(db_err)?;
            let pending_count = ai_approval_requests::Entity::find()
                .filter(
                    Condition::all()
                        .add(ai_approval_requests::Column::TenantId.eq(tenant_id))
                        .add(ai_approval_requests::Column::SessionId.eq(session.id))
                        .add(ai_approval_requests::Column::Status.eq("pending")),
                )
                .count(db)
                .await
                .map_err(db_err)? as usize;
            summaries.push(AiChatSessionSummary {
                id: session.id,
                title: session.title,
                provider_profile_id: session.provider_profile_id,
                task_profile_id: session.task_profile_id,
                tool_profile_id: session.tool_profile_id,
                execution_mode: execution_mode_from_slug(&session.execution_mode),
                status: session.status,
                created_at: to_utc(session.created_at),
                updated_at: to_utc(session.updated_at),
                latest_run_status: latest_run.map(|value| value.status),
                pending_approvals: pending_count,
            });
        }
        Ok(summaries)
    }

    pub async fn chat_session_detail(
        db: &DatabaseConnection,
        tenant_id: Uuid,
        session_id: Uuid,
    ) -> AiResult<Option<AiChatSessionDetail>> {
        let Some(session) = ai_chat_sessions::Entity::find_by_id(session_id)
            .filter(ai_chat_sessions::Column::TenantId.eq(tenant_id))
            .one(db)
            .await
            .map_err(db_err)?
        else {
            return Ok(None);
        };

        let provider = require_provider_profile(db, tenant_id, session.provider_profile_id).await?;
        let task_profile = match session.task_profile_id {
            Some(id) => Some(map_task_profile(
                require_task_profile(db, tenant_id, id).await?,
            )?),
            None => None,
        };
        let tool_profile = match session.tool_profile_id {
            Some(id) => Some(map_tool_profile(
                require_tool_profile(db, tenant_id, id).await?,
            )),
            None => None,
        };
        let messages = ai_chat_messages::Entity::find()
            .filter(
                Condition::all()
                    .add(ai_chat_messages::Column::TenantId.eq(tenant_id))
                    .add(ai_chat_messages::Column::SessionId.eq(session.id)),
            )
            .order_by_asc(ai_chat_messages::Column::CreatedAt)
            .all(db)
            .await
            .map_err(db_err)?
            .into_iter()
            .map(map_message_record)
            .collect::<AiResult<Vec<_>>>()?;
        let runs: Vec<_> = ai_chat_runs::Entity::find()
            .filter(
                Condition::all()
                    .add(ai_chat_runs::Column::TenantId.eq(tenant_id))
                    .add(ai_chat_runs::Column::SessionId.eq(session.id)),
            )
            .order_by_desc(ai_chat_runs::Column::CreatedAt)
            .all(db)
            .await
            .map_err(db_err)?
            .into_iter()
            .map(map_run_record)
            .collect();
        let tool_traces: Vec<_> = ai_tool_traces::Entity::find()
            .filter(
                Condition::all()
                    .add(ai_tool_traces::Column::TenantId.eq(tenant_id))
                    .add(ai_tool_traces::Column::SessionId.eq(session.id)),
            )
            .order_by_desc(ai_tool_traces::Column::CreatedAt)
            .all(db)
            .await
            .map_err(db_err)?
            .into_iter()
            .map(map_trace_record)
            .collect();
        let approvals: Vec<_> = ai_approval_requests::Entity::find()
            .filter(
                Condition::all()
                    .add(ai_approval_requests::Column::TenantId.eq(tenant_id))
                    .add(ai_approval_requests::Column::SessionId.eq(session.id)),
            )
            .order_by_desc(ai_approval_requests::Column::CreatedAt)
            .all(db)
            .await
            .map_err(db_err)?
            .into_iter()
            .map(map_approval_record)
            .collect();
        let latest_run_status = runs
            .first()
            .map(|value: &AiChatRunRecord| value.status.clone());
        let pending_approvals = approvals
            .iter()
            .filter(|approval| approval.status == "pending")
            .count();

        Ok(Some(AiChatSessionDetail {
            session: AiChatSessionSummary {
                id: session.id,
                title: session.title,
                provider_profile_id: session.provider_profile_id,
                task_profile_id: session.task_profile_id,
                tool_profile_id: session.tool_profile_id,
                execution_mode: execution_mode_from_slug(&session.execution_mode),
                status: session.status,
                created_at: to_utc(session.created_at),
                updated_at: to_utc(session.updated_at),
                latest_run_status,
                pending_approvals,
            },
            provider_profile: map_provider_profile(provider),
            task_profile,
            tool_profile,
            messages,
            runs,
            tool_traces,
            approvals,
        }))
    }

    pub async fn list_tool_traces(
        db: &DatabaseConnection,
        tenant_id: Uuid,
        session_id: Option<Uuid>,
        run_id: Option<Uuid>,
    ) -> AiResult<Vec<ToolTrace>> {
        let mut query =
            ai_tool_traces::Entity::find().filter(ai_tool_traces::Column::TenantId.eq(tenant_id));
        if let Some(session_id) = session_id {
            query = query.filter(ai_tool_traces::Column::SessionId.eq(session_id));
        }
        if let Some(run_id) = run_id {
            query = query.filter(ai_tool_traces::Column::RunId.eq(run_id));
        }
        let traces = query
            .order_by_desc(ai_tool_traces::Column::CreatedAt)
            .all(db)
            .await
            .map_err(db_err)?
            .into_iter()
            .map(map_trace_record)
            .collect();
        Ok(traces)
    }

    pub async fn resume_approval(
        app_ctx: &AppContext,
        operator: &AiOperatorContext,
        approval_id: Uuid,
        input: ResumeAiApprovalInput,
    ) -> AiResult<AiSendMessageResult> {
        let db = &app_ctx.db;
        let approval = ai_approval_requests::Entity::find_by_id(approval_id)
            .filter(ai_approval_requests::Column::TenantId.eq(operator.tenant_id))
            .one(db)
            .await
            .map_err(db_err)?
            .ok_or_else(|| AiError::NotFound("approval request not found".to_string()))?;
        if approval.status != "pending" {
            return Err(AiError::Validation(
                "approval request is not pending".to_string(),
            ));
        }

        let session = require_session(db, operator.tenant_id, approval.session_id).await?;
        let provider =
            require_provider_profile(db, operator.tenant_id, session.provider_profile_id).await?;
        let task_profile = match session.task_profile_id {
            Some(id) => Some(require_task_profile(db, operator.tenant_id, id).await?),
            None => None,
        };
        let tool_profile = match session.tool_profile_id {
            Some(id) => Some(require_tool_profile(db, operator.tenant_id, id).await?),
            None => None,
        };

        let mut approval_active: ai_approval_requests::ActiveModel = approval.clone().into();
        approval_active.status = Set(if input.approved {
            "approved".to_string()
        } else {
            "rejected".to_string()
        });
        approval_active.reason = Set(input.reason.clone().or(approval.reason.clone()));
        approval_active.resolved_by = Set(Some(operator.user_id));
        approval_active.resolved_at = Set(Some(Utc::now().into()));
        approval_active.updated_at = Set(Utc::now().into());
        approval_active.update(db).await.map_err(db_err)?;

        let run = require_run(db, operator.tenant_id, approval.run_id).await?;
        let mut run_active: ai_chat_runs::ActiveModel = run.clone().into();

        if !input.approved {
            run_active.status = Set("failed".to_string());
            run_active.error_message = Set(Some("tool execution rejected by operator".to_string()));
            run_active.pending_approval_id = Set(None);
            run_active.completed_at = Set(Some(Utc::now().into()));
            run_active.updated_at = Set(Utc::now().into());
            let saved_run = run_active.update(db).await.map_err(db_err)?;
            insert_message(
                db,
                operator.tenant_id,
                session.id,
                Some(saved_run.id),
                Some(operator.user_id),
                ChatMessage {
                    role: ChatMessageRole::Assistant,
                    content: Some("Tool execution was rejected by the operator.".to_string()),
                    name: None,
                    tool_call_id: None,
                    tool_calls: Vec::new(),
                    metadata: json!({ "approval_rejected": true }),
                },
            )
            .await?;
            let detail = Self::chat_session_detail(db, operator.tenant_id, session.id)
                .await?
                .ok_or_else(|| AiError::Runtime("failed to reload AI chat session".to_string()))?;
            return Ok(AiSendMessageResult {
                session: detail,
                run: map_run_record(saved_run),
            });
        }

        let access_context = access_context_for_operator(operator);
        let tool_policy = policy_from_model(tool_profile.as_ref());
        let adapter = InProcessMcpAdapter::new(app_ctx, access_context)?;
        let started = std::time::Instant::now();
        let tool_result = adapter
            .call_tool(&approval.tool_name, approval.tool_input.clone())
            .await?;
        let trace = ToolTrace {
            tool_name: approval.tool_name.clone(),
            input_payload: approval.tool_input.clone(),
            output_payload: Some(tool_result.raw_payload.clone()),
            status: "completed".to_string(),
            duration_ms: started.elapsed().as_millis() as i64,
            sensitive: tool_policy.is_tool_sensitive(&approval.tool_name),
            error_message: None,
            created_at: Utc::now(),
        };

        insert_tool_trace(db, operator.tenant_id, session.id, run.id, &trace).await?;
        insert_message(
            db,
            operator.tenant_id,
            session.id,
            Some(run.id),
            Some(operator.user_id),
            ChatMessage {
                role: ChatMessageRole::Tool,
                content: Some(tool_result.content),
                name: Some(approval.tool_name.clone()),
                tool_call_id: Some(approval.tool_call_id.clone()),
                tool_calls: Vec::new(),
                metadata: json!({ "raw_payload": tool_result.raw_payload }),
            },
        )
        .await?;

        run_active.status = Set("running".to_string());
        run_active.pending_approval_id = Set(None);
        run_active.updated_at = Set(Utc::now().into());
        run_active.error_message = Set(None);
        run_active.update(db).await.map_err(db_err)?;

        Self::continue_run(
            app_ctx,
            operator,
            session.id,
            run.id,
            provider,
            task_profile,
            tool_profile,
            execution_mode_from_slug(&session.execution_mode),
        )
        .await
    }

    pub async fn cancel_run(
        db: &DatabaseConnection,
        operator: &AiOperatorContext,
        run_id: Uuid,
    ) -> AiResult<AiChatRunRecord> {
        let run = require_run(db, operator.tenant_id, run_id).await?;
        let mut active: ai_chat_runs::ActiveModel = run.into();
        active.status = Set("cancelled".to_string());
        active.completed_at = Set(Some(Utc::now().into()));
        active.updated_at = Set(Utc::now().into());
        let saved = active.update(db).await.map_err(db_err)?;
        Ok(map_run_record(saved))
    }

    async fn execute_latest_turn(
        app_ctx: &AppContext,
        operator: &AiOperatorContext,
        session_id: Uuid,
    ) -> AiResult<AiSendMessageResult> {
        let db = &app_ctx.db;
        let session = require_session(db, operator.tenant_id, session_id).await?;
        let provider =
            require_provider_profile(db, operator.tenant_id, session.provider_profile_id).await?;
        let task_profile = match session.task_profile_id {
            Some(id) => Some(require_task_profile(db, operator.tenant_id, id).await?),
            None => None,
        };
        let tool_profile = match session.tool_profile_id {
            Some(id) => Some(require_tool_profile(db, operator.tenant_id, id).await?),
            None => None,
        };
        let execution_mode = execution_mode_from_slug(&session.execution_mode);

        let run = ai_chat_runs::ActiveModel {
            id: Set(Uuid::new_v4()),
            tenant_id: Set(operator.tenant_id),
            session_id: Set(session.id),
            provider_profile_id: Set(provider.id),
            task_profile_id: Set(task_profile.as_ref().map(|value| value.id)),
            tool_profile_id: Set(tool_profile.as_ref().map(|value| value.id)),
            status: Set("running".to_string()),
            model: Set(provider.model.clone()),
            execution_mode: Set(execution_mode.slug().to_string()),
            execution_path: Set(execution_mode.slug().to_string()),
            temperature: Set(provider.temperature),
            max_tokens: Set(provider.max_tokens),
            error_message: Set(None),
            pending_approval_id: Set(None),
            decision_trace: Set(session
                .metadata
                .get("decision_trace")
                .cloned()
                .unwrap_or_else(|| json!({}))),
            metadata: Set(json!({})),
            created_at: sea_orm::ActiveValue::NotSet,
            started_at: Set(Utc::now().into()),
            completed_at: Set(None),
            updated_at: Set(Utc::now().into()),
        }
        .insert(db)
        .await
        .map_err(db_err)?;

        Self::continue_run(
            app_ctx,
            operator,
            session.id,
            run.id,
            provider,
            task_profile,
            tool_profile,
            execution_mode,
        )
        .await
    }

    async fn continue_run(
        app_ctx: &AppContext,
        operator: &AiOperatorContext,
        session_id: Uuid,
        run_id: Uuid,
        provider_profile: ai_provider_profiles::Model,
        task_profile: Option<ai_task_profiles::Model>,
        tool_profile: Option<ai_tool_profiles::Model>,
        execution_mode: ExecutionMode,
    ) -> AiResult<AiSendMessageResult> {
        let db = &app_ctx.db;
        let messages = ai_chat_messages::Entity::find()
            .filter(
                Condition::all()
                    .add(ai_chat_messages::Column::TenantId.eq(operator.tenant_id))
                    .add(ai_chat_messages::Column::SessionId.eq(session_id)),
            )
            .order_by_asc(ai_chat_messages::Column::CreatedAt)
            .all(db)
            .await
            .map_err(db_err)?
            .into_iter()
            .map(map_chat_message)
            .collect::<AiResult<Vec<_>>>()?;

        let provider = Arc::<dyn ModelProvider>::from(provider_for_kind(provider_kind_from_slug(
            &provider_profile.provider_kind,
        )));
        let access_context = access_context_for_operator(operator);
        let adapter = Arc::new(InProcessMcpAdapter::new(app_ctx, access_context)?);
        let policy = policy_from_model(tool_profile.as_ref());
        let runtime = AiRuntime::new(provider, adapter, policy);
        let outcome = runtime
            .run(
                &provider_config(&provider_profile)?,
                RuntimeRequest {
                    model: provider_profile.model.clone(),
                    messages,
                    temperature: provider_profile.temperature,
                    max_tokens: provider_profile.max_tokens.map(|value| value.max(0) as u32),
                    max_turns: 4,
                    execution_mode,
                    system_prompt: task_profile
                        .as_ref()
                        .and_then(|value| value.system_prompt.clone()),
                },
            )
            .await?;

        let mut run = require_run(db, operator.tenant_id, run_id).await?;

        match outcome {
            RuntimeOutcome::Completed {
                appended_messages,
                traces,
            } => {
                persist_runtime_outputs(
                    db,
                    operator,
                    session_id,
                    run_id,
                    appended_messages,
                    traces,
                )
                .await?;
                let mut active: ai_chat_runs::ActiveModel = run.into();
                active.status = Set("completed".to_string());
                active.completed_at = Set(Some(Utc::now().into()));
                active.updated_at = Set(Utc::now().into());
                run = active.update(db).await.map_err(db_err)?;
            }
            RuntimeOutcome::Failed {
                appended_messages,
                traces,
                error_message,
            } => {
                persist_runtime_outputs(
                    db,
                    operator,
                    session_id,
                    run_id,
                    appended_messages,
                    traces,
                )
                .await?;
                let mut active: ai_chat_runs::ActiveModel = run.into();
                active.status = Set("failed".to_string());
                active.error_message = Set(Some(error_message));
                active.completed_at = Set(Some(Utc::now().into()));
                active.updated_at = Set(Utc::now().into());
                run = active.update(db).await.map_err(db_err)?;
            }
            RuntimeOutcome::WaitingApproval {
                appended_messages,
                traces,
                pending_approval,
            } => {
                persist_runtime_outputs(
                    db,
                    operator,
                    session_id,
                    run_id,
                    appended_messages,
                    traces,
                )
                .await?;
                let approval =
                    insert_approval_request(db, operator, session_id, run_id, &pending_approval)
                        .await?;
                let mut active: ai_chat_runs::ActiveModel = run.into();
                active.status = Set("waiting_approval".to_string());
                active.pending_approval_id = Set(Some(approval.id));
                active.updated_at = Set(Utc::now().into());
                run = active.update(db).await.map_err(db_err)?;
            }
        }

        let detail = Self::chat_session_detail(db, operator.tenant_id, session_id)
            .await?
            .ok_or_else(|| AiError::Runtime("failed to reload AI chat session".to_string()))?;
        Ok(AiSendMessageResult {
            session: detail,
            run: map_run_record(run),
        })
    }
}

struct InProcessMcpAdapter {
    state: McpState,
    access_context: McpAccessContext,
    alloy: Option<AlloyMcpState<alloy::SeaOrmStorage>>,
}

impl InProcessMcpAdapter {
    fn new(app_ctx: &AppContext, access_context: McpAccessContext) -> AiResult<Self> {
        let registry = app_ctx
            .shared_store
            .get::<SharedAiModuleRegistry>()
            .map(|shared| shared.0.clone())
            .ok_or_else(|| AiError::Runtime("AI module registry is not initialized".to_string()))?;
        let alloy = if let Some(_) = app_ctx.shared_store.get::<alloy::SharedAlloyRuntime>() {
            let scoped = alloy::scoped_runtime(
                app_ctx,
                parse_uuid_str(
                    access_context
                        .identity
                        .as_ref()
                        .and_then(|identity| identity.tenant_id.as_deref()),
                )?,
            );
            let mut state = AlloyMcpState::new(scoped.storage, scoped.engine, scoped.orchestrator);
            state.staged_scaffolds = Arc::clone(&STAGED_SCAFFOLDS);
            Some(state)
        } else {
            None
        };
        Ok(Self {
            state: McpState { registry },
            access_context,
            alloy,
        })
    }

    async fn call_alloy_tool(
        &self,
        tool_name: &str,
        input: serde_json::Value,
    ) -> AiResult<ToolExecutionResult> {
        let Some(state) = &self.alloy else {
            return Err(AiError::Mcp(format!("unknown tool: {tool_name}")));
        };
        let content = match tool_name {
            TOOL_ALLOY_LIST_SCRIPTS => serde_json::to_value(
                alloy_tools::alloy_list_scripts(
                    state,
                    serde_json::from_value::<ListScriptsRequest>(input).map_err(json_err)?,
                )
                .await
                .map_err(AiError::Mcp)?,
            )
            .map_err(json_err)?,
            TOOL_ALLOY_GET_SCRIPT => serde_json::to_value(
                alloy_tools::alloy_get_script(
                    state,
                    serde_json::from_value::<GetScriptRequest>(input).map_err(json_err)?,
                )
                .await
                .map_err(AiError::Mcp)?,
            )
            .map_err(json_err)?,
            TOOL_ALLOY_CREATE_SCRIPT => serde_json::to_value(
                alloy_tools::alloy_create_script(
                    state,
                    serde_json::from_value::<CreateScriptRequest>(input).map_err(json_err)?,
                )
                .await
                .map_err(AiError::Mcp)?,
            )
            .map_err(json_err)?,
            TOOL_ALLOY_UPDATE_SCRIPT => serde_json::to_value(
                alloy_tools::alloy_update_script(
                    state,
                    serde_json::from_value::<UpdateScriptRequest>(input).map_err(json_err)?,
                )
                .await
                .map_err(AiError::Mcp)?,
            )
            .map_err(json_err)?,
            TOOL_ALLOY_DELETE_SCRIPT => serde_json::to_value(
                alloy_tools::alloy_delete_script(
                    state,
                    serde_json::from_value::<DeleteScriptRequest>(input).map_err(json_err)?,
                )
                .await
                .map_err(AiError::Mcp)?,
            )
            .map_err(json_err)?,
            TOOL_ALLOY_VALIDATE_SCRIPT => serde_json::to_value(alloy_tools::alloy_validate_script(
                state,
                serde_json::from_value::<ValidateScriptRequest>(input).map_err(json_err)?,
            ))
            .map_err(json_err)?,
            TOOL_ALLOY_RUN_SCRIPT => serde_json::to_value(
                alloy_tools::alloy_run_script(
                    state,
                    serde_json::from_value::<RunScriptRequest>(input).map_err(json_err)?,
                )
                .await
                .map_err(AiError::Mcp)?,
            )
            .map_err(json_err)?,
            TOOL_ALLOY_SCAFFOLD_MODULE => serde_json::to_value(
                alloy_tools::alloy_scaffold_module(
                    state,
                    None,
                    serde_json::from_value::<rustok_mcp::ScaffoldModuleRequest>(input)
                        .map_err(json_err)?,
                )
                .await
                .map_err(AiError::Mcp)?,
            )
            .map_err(json_err)?,
            TOOL_ALLOY_REVIEW_MODULE_SCAFFOLD => serde_json::to_value(
                alloy_tools::alloy_review_module_scaffold(
                    state,
                    None,
                    serde_json::from_value::<ReviewModuleScaffoldRequest>(input)
                        .map_err(json_err)?,
                )
                .await
                .map_err(AiError::Mcp)?,
            )
            .map_err(json_err)?,
            TOOL_ALLOY_APPLY_MODULE_SCAFFOLD => serde_json::to_value(
                alloy_tools::alloy_apply_module_scaffold(
                    state,
                    None,
                    serde_json::from_value::<ApplyModuleScaffoldRequest>(input)
                        .map_err(json_err)?,
                )
                .await
                .map_err(AiError::Mcp)?,
            )
            .map_err(json_err)?,
            TOOL_ALLOY_LIST_ENTITY_TYPES => {
                serde_json::to_value(alloy_tools::alloy_list_entity_types()).map_err(json_err)?
            }
            TOOL_ALLOY_SCRIPT_HELPERS => {
                serde_json::to_value(alloy_tools::alloy_script_helpers()).map_err(json_err)?
            }
            _ => return Err(AiError::Mcp(format!("unknown tool: {tool_name}"))),
        };

        Ok(ToolExecutionResult {
            content: serde_json::to_string(&content).map_err(json_err)?,
            raw_payload: content,
        })
    }
}

#[async_trait::async_trait]
impl McpClientAdapter for InProcessMcpAdapter {
    async fn list_tools(&self) -> AiResult<Vec<ToolDefinition>> {
        let mut tools = vec![
            tool_def(
                TOOL_LIST_MODULES,
                "List all registered RusToK modules with their metadata",
                schema_for!(()),
            ),
            tool_def(
                TOOL_QUERY_MODULES,
                "List modules with filters and pagination",
                schema_for!(ModuleQueryRequest),
            ),
            tool_def(
                TOOL_MODULE_EXISTS,
                "Check if a module exists by its slug",
                schema_for!(ModuleLookupRequest),
            ),
            tool_def(
                TOOL_MODULE_DETAILS,
                "Fetch module metadata by slug",
                schema_for!(ModuleLookupRequest),
            ),
            tool_def(
                TOOL_CONTENT_MODULE,
                "Fetch content module metadata",
                schema_for!(()),
            ),
            tool_def(
                TOOL_BLOG_MODULE,
                "Fetch blog module metadata",
                schema_for!(()),
            ),
            tool_def(
                TOOL_FORUM_MODULE,
                "Fetch forum module metadata",
                schema_for!(()),
            ),
            tool_def(
                TOOL_PAGES_MODULE,
                "Fetch pages module metadata",
                schema_for!(()),
            ),
            tool_def(
                TOOL_MCP_HEALTH,
                "MCP readiness and configuration status",
                schema_for!(()),
            ),
            tool_def(
                TOOL_MCP_WHOAMI,
                "Inspect the current MCP identity, permissions, scopes, and tool policy",
                schema_for!(()),
            ),
        ];

        if self.alloy.is_some() {
            tools.extend([
                tool_def(
                    TOOL_ALLOY_LIST_SCRIPTS,
                    "List Alloy scripts with optional status filter",
                    schema_for!(ListScriptsRequest),
                ),
                tool_def(
                    TOOL_ALLOY_GET_SCRIPT,
                    "Get a single Alloy script by name or UUID",
                    schema_for!(GetScriptRequest),
                ),
                tool_def(
                    TOOL_ALLOY_CREATE_SCRIPT,
                    "Create a new Alloy Rhai script",
                    schema_for!(CreateScriptRequest),
                ),
                tool_def(
                    TOOL_ALLOY_UPDATE_SCRIPT,
                    "Update an existing Alloy script (code, description, status)",
                    schema_for!(UpdateScriptRequest),
                ),
                tool_def(
                    TOOL_ALLOY_DELETE_SCRIPT,
                    "Delete an Alloy script by UUID",
                    schema_for!(DeleteScriptRequest),
                ),
                tool_def(
                    TOOL_ALLOY_VALIDATE_SCRIPT,
                    "Validate Rhai script syntax without executing",
                    schema_for!(ValidateScriptRequest),
                ),
                tool_def(
                    TOOL_ALLOY_RUN_SCRIPT,
                    "Execute an Alloy script manually with optional params and entity context",
                    schema_for!(RunScriptRequest),
                ),
                tool_def(
                    TOOL_ALLOY_SCAFFOLD_MODULE,
                    "Stage a reviewed draft RusToK module crate scaffold without writing it into the workspace yet",
                    schema_for!(rustok_mcp::ScaffoldModuleRequest),
                ),
                tool_def(
                    TOOL_ALLOY_REVIEW_MODULE_SCAFFOLD,
                    "Fetch a staged Alloy module scaffold draft for review before apply",
                    schema_for!(ReviewModuleScaffoldRequest),
                ),
                tool_def(
                    TOOL_ALLOY_APPLY_MODULE_SCAFFOLD,
                    "Apply a reviewed Alloy module scaffold draft into the workspace with explicit confirmation",
                    schema_for!(ApplyModuleScaffoldRequest),
                ),
                tool_def(
                    TOOL_ALLOY_LIST_ENTITY_TYPES,
                    "List all known entity types in the platform",
                    schema_for!(()),
                ),
                tool_def(
                    TOOL_ALLOY_SCRIPT_HELPERS,
                    "List available Rhai helper functions with signatures and descriptions",
                    schema_for!(()),
                ),
            ]);
        }

        Ok(tools
            .into_iter()
            .filter(|tool| {
                self.access_context
                    .authorize_tool(&default_tool_requirement(&tool.name))
                    .allowed
            })
            .collect())
    }

    async fn call_tool(
        &self,
        tool_name: &str,
        input: serde_json::Value,
    ) -> AiResult<ToolExecutionResult> {
        let decision = self
            .access_context
            .authorize_tool(&default_tool_requirement(tool_name));
        if !decision.allowed {
            return Err(AiError::Mcp(
                decision
                    .message
                    .unwrap_or_else(|| "tool access denied".to_string()),
            ));
        }

        match tool_name {
            TOOL_LIST_MODULES => serialize_result(McpToolResponse::success(
                tools::list_modules(&self.state).await,
            )),
            TOOL_QUERY_MODULES => serialize_result(McpToolResponse::success(
                tools::list_modules_filtered(
                    &self.state,
                    serde_json::from_value::<ModuleQueryRequest>(input).map_err(json_err)?,
                )
                .await,
            )),
            TOOL_MODULE_EXISTS => serialize_result(McpToolResponse::success(
                tools::module_exists(
                    &self.state,
                    serde_json::from_value::<ModuleLookupRequest>(input).map_err(json_err)?,
                )
                .await,
            )),
            TOOL_MODULE_DETAILS => serialize_result(McpToolResponse::success(
                tools::module_details(
                    &self.state,
                    serde_json::from_value::<ModuleLookupRequest>(input).map_err(json_err)?,
                )
                .await,
            )),
            TOOL_CONTENT_MODULE => serialize_result(McpToolResponse::success(
                tools::module_details_by_slug(&self.state, rustok_mcp::MODULE_CONTENT),
            )),
            TOOL_BLOG_MODULE => serialize_result(McpToolResponse::success(
                tools::module_details_by_slug(&self.state, rustok_mcp::MODULE_BLOG),
            )),
            TOOL_FORUM_MODULE => serialize_result(McpToolResponse::success(
                tools::module_details_by_slug(&self.state, rustok_mcp::MODULE_FORUM),
            )),
            TOOL_PAGES_MODULE => serialize_result(McpToolResponse::success(
                tools::module_details_by_slug(&self.state, rustok_mcp::MODULE_PAGES),
            )),
            TOOL_MCP_HEALTH => serialize_result(McpToolResponse::success(McpHealthResponse {
                status: "ok".to_string(),
                protocol_version: "in_process".to_string(),
                tool_count: self.list_tools().await?.len(),
                enabled_tools: None,
                access_mode: "direct".to_string(),
                identity: self.access_context.identity.clone(),
            })),
            TOOL_MCP_WHOAMI => {
                serialize_result(McpToolResponse::success(self.access_context.whoami()))
            }
            _ => self.call_alloy_tool(tool_name, input).await,
        }
    }
}

fn tool_def(name: &str, description: &str, schema: schemars::Schema) -> ToolDefinition {
    let input_schema = serde_json::to_value(schema)
        .ok()
        .and_then(|value| value.as_object().cloned())
        .map(serde_json::Value::Object)
        .unwrap_or_else(|| json!({}));
    ToolDefinition {
        name: name.to_string(),
        description: description.to_string(),
        input_schema,
        sensitive: false,
    }
}

fn serialize_result<T: Serialize>(payload: T) -> AiResult<ToolExecutionResult> {
    let raw_payload = serde_json::to_value(payload).map_err(json_err)?;
    Ok(ToolExecutionResult {
        content: serde_json::to_string(&raw_payload).map_err(json_err)?,
        raw_payload,
    })
}

fn access_context_for_operator(operator: &AiOperatorContext) -> McpAccessContext {
    McpAccessContext {
        identity: Some(McpIdentity {
            actor_id: operator.user_id.to_string(),
            actor_type: McpActorType::HumanUser,
            tenant_id: Some(operator.tenant_id.to_string()),
            delegated_user_id: None,
            display_name: Some("RusToK AI Operator".to_string()),
            scopes: Vec::new(),
        }),
        granted_permissions: operator
            .permissions
            .iter()
            .map(ToString::to_string)
            .collect(),
        policy: McpAccessPolicy {
            allowed_tools: None,
            denied_tools: Vec::new(),
        },
    }
}

fn provider_kind_slug(kind: ProviderKind) -> &'static str {
    match kind {
        ProviderKind::OpenAiCompatible => "openai_compatible",
        ProviderKind::Anthropic => "anthropic",
        ProviderKind::Gemini => "gemini",
    }
}

fn provider_kind_from_slug(value: &str) -> ProviderKind {
    match value {
        "openai_compatible" => ProviderKind::OpenAiCompatible,
        "anthropic" => ProviderKind::Anthropic,
        "gemini" => ProviderKind::Gemini,
        _ => ProviderKind::OpenAiCompatible,
    }
}

fn capability_from_slug(value: &str) -> ProviderCapability {
    match value {
        "structured_generation" => ProviderCapability::StructuredGeneration,
        "image_generation" => ProviderCapability::ImageGeneration,
        "multimodal_understanding" => ProviderCapability::MultimodalUnderstanding,
        "code_generation" => ProviderCapability::CodeGeneration,
        "alloy_assist" => ProviderCapability::AlloyAssist,
        _ => ProviderCapability::TextGeneration,
    }
}

fn execution_mode_from_slug(value: &str) -> ExecutionMode {
    match value {
        "direct" => ExecutionMode::Direct,
        "mcp_tooling" => ExecutionMode::McpTooling,
        _ => ExecutionMode::Auto,
    }
}

fn provider_config(model: &ai_provider_profiles::Model) -> AiResult<AiProviderConfig> {
    Ok(AiProviderConfig {
        provider_kind: provider_kind_from_slug(&model.provider_kind),
        base_url: model.base_url.clone(),
        api_key: model.api_key_secret.clone(),
        model: model.model.clone(),
        temperature: model.temperature,
        max_tokens: model.max_tokens.map(|value| value.max(0) as u32),
        capabilities: capability_list(&model.capabilities),
        usage_policy: ProviderUsagePolicy {
            allowed_task_profiles: string_list(&model.allowed_task_profiles),
            denied_task_profiles: string_list(&model.denied_task_profiles),
            restricted_role_slugs: string_list(&model.restricted_role_slugs),
        },
    })
}

fn policy_from_model(model: Option<&ai_tool_profiles::Model>) -> ToolExecutionPolicy {
    match model {
        Some(model) => ToolExecutionPolicy::new(
            match string_list(&model.allowed_tools) {
                values if values.is_empty() => None,
                values => Some(values),
            },
            string_list(&model.denied_tools),
            string_list(&model.sensitive_tools),
        ),
        None => ToolExecutionPolicy::default(),
    }
}

fn map_provider_profile(model: ai_provider_profiles::Model) -> AiProviderProfileRecord {
    AiProviderProfileRecord {
        id: model.id,
        slug: model.slug,
        display_name: model.display_name,
        provider_kind: provider_kind_from_slug(&model.provider_kind),
        base_url: model.base_url,
        model: model.model,
        temperature: model.temperature,
        max_tokens: model.max_tokens,
        is_active: model.is_active,
        has_secret: model
            .api_key_secret
            .as_ref()
            .map(|value| !value.trim().is_empty())
            .unwrap_or(false),
        capabilities: capability_list(&model.capabilities),
        usage_policy: ProviderUsagePolicy {
            allowed_task_profiles: string_list(&model.allowed_task_profiles),
            denied_task_profiles: string_list(&model.denied_task_profiles),
            restricted_role_slugs: string_list(&model.restricted_role_slugs),
        },
        metadata: model.metadata,
        created_at: to_utc(model.created_at),
        updated_at: to_utc(model.updated_at),
    }
}

fn map_task_profile(model: ai_task_profiles::Model) -> AiResult<AiTaskProfileRecord> {
    Ok(AiTaskProfileRecord {
        id: model.id,
        slug: model.slug,
        display_name: model.display_name,
        description: model.description,
        target_capability: capability_from_slug(&model.target_capability),
        system_prompt: model.system_prompt,
        allowed_provider_profile_ids: uuid_list(&model.allowed_provider_profile_ids),
        preferred_provider_profile_ids: uuid_list(&model.preferred_provider_profile_ids),
        fallback_strategy: model.fallback_strategy,
        tool_profile_id: model.tool_profile_id,
        approval_policy: model.approval_policy,
        default_execution_mode: execution_mode_from_slug(&model.default_execution_mode),
        is_active: model.is_active,
        metadata: model.metadata,
        created_at: to_utc(model.created_at),
        updated_at: to_utc(model.updated_at),
    })
}

fn task_profile_runtime(record: &AiTaskProfileRecord) -> TaskProfile {
    TaskProfile {
        id: record.id,
        slug: record.slug.clone(),
        display_name: record.display_name.clone(),
        description: record.description.clone(),
        target_capability: record.target_capability,
        system_prompt: record.system_prompt.clone(),
        allowed_provider_profile_ids: record.allowed_provider_profile_ids.clone(),
        preferred_provider_profile_ids: record.preferred_provider_profile_ids.clone(),
        fallback_strategy: record.fallback_strategy.clone(),
        tool_profile_id: record.tool_profile_id,
        approval_policy: record.approval_policy.clone(),
        default_execution_mode: record.default_execution_mode,
        is_active: record.is_active,
        metadata: record.metadata.clone(),
    }
}

fn map_tool_profile(model: ai_tool_profiles::Model) -> AiToolProfileRecord {
    AiToolProfileRecord {
        id: model.id,
        slug: model.slug,
        display_name: model.display_name,
        description: model.description,
        allowed_tools: string_list(&model.allowed_tools),
        denied_tools: string_list(&model.denied_tools),
        sensitive_tools: string_list(&model.sensitive_tools),
        is_active: model.is_active,
        metadata: model.metadata,
        created_at: to_utc(model.created_at),
        updated_at: to_utc(model.updated_at),
    }
}

fn map_message_record(model: ai_chat_messages::Model) -> AiResult<AiChatMessageRecord> {
    Ok(AiChatMessageRecord {
        id: model.id,
        session_id: model.session_id,
        run_id: model.run_id,
        role: map_role(&model.role)?,
        content: model.content,
        name: model.name,
        tool_call_id: model.tool_call_id,
        tool_calls: serde_json::from_value(model.tool_calls).map_err(json_err)?,
        metadata: model.metadata,
        created_at: to_utc(model.created_at),
    })
}

fn map_run_record(model: ai_chat_runs::Model) -> AiChatRunRecord {
    AiChatRunRecord {
        id: model.id,
        session_id: model.session_id,
        provider_profile_id: model.provider_profile_id,
        task_profile_id: model.task_profile_id,
        tool_profile_id: model.tool_profile_id,
        status: model.status,
        model: model.model,
        execution_mode: execution_mode_from_slug(&model.execution_mode),
        execution_path: execution_mode_from_slug(&model.execution_path),
        temperature: model.temperature,
        max_tokens: model.max_tokens,
        error_message: model.error_message,
        pending_approval_id: model.pending_approval_id,
        decision_trace: serde_json::from_value(model.decision_trace).unwrap_or_default(),
        metadata: model.metadata,
        created_at: to_utc(model.created_at),
        started_at: to_utc(model.started_at),
        completed_at: model.completed_at.map(to_utc),
        updated_at: to_utc(model.updated_at),
    }
}

fn map_approval_record(model: ai_approval_requests::Model) -> AiApprovalRequestRecord {
    AiApprovalRequestRecord {
        id: model.id,
        session_id: model.session_id,
        run_id: model.run_id,
        tool_name: model.tool_name,
        tool_call_id: model.tool_call_id,
        tool_input: model.tool_input,
        reason: model.reason,
        status: model.status,
        resolved_by: model.resolved_by,
        resolved_at: model.resolved_at.map(to_utc),
        metadata: model.metadata,
        created_at: to_utc(model.created_at),
        updated_at: to_utc(model.updated_at),
    }
}

fn map_trace_record(model: ai_tool_traces::Model) -> ToolTrace {
    ToolTrace {
        tool_name: model.tool_name,
        input_payload: model.input_payload,
        output_payload: model.output_payload,
        status: model.status,
        duration_ms: model.duration_ms.unwrap_or_default(),
        sensitive: model.sensitive,
        error_message: model.error_message,
        created_at: to_utc(model.created_at),
    }
}

fn map_chat_message(model: ai_chat_messages::Model) -> AiResult<ChatMessage> {
    Ok(ChatMessage {
        role: map_role(&model.role)?,
        content: model.content,
        name: model.name,
        tool_call_id: model.tool_call_id,
        tool_calls: serde_json::from_value(model.tool_calls).map_err(json_err)?,
        metadata: model.metadata,
    })
}

fn map_role(value: &str) -> AiResult<ChatMessageRole> {
    match value {
        "system" => Ok(ChatMessageRole::System),
        "user" => Ok(ChatMessageRole::User),
        "assistant" => Ok(ChatMessageRole::Assistant),
        "tool" => Ok(ChatMessageRole::Tool),
        other => Err(AiError::Runtime(format!(
            "unknown AI message role: {other}"
        ))),
    }
}

fn role_slug(role: ChatMessageRole) -> &'static str {
    match role {
        ChatMessageRole::System => "system",
        ChatMessageRole::User => "user",
        ChatMessageRole::Assistant => "assistant",
        ChatMessageRole::Tool => "tool",
    }
}

async fn insert_message<C>(
    db: &C,
    tenant_id: Uuid,
    session_id: Uuid,
    run_id: Option<Uuid>,
    created_by: Option<Uuid>,
    message: ChatMessage,
) -> AiResult<ai_chat_messages::Model>
where
    C: sea_orm::ConnectionTrait,
{
    ai_chat_messages::ActiveModel {
        id: Set(Uuid::new_v4()),
        tenant_id: Set(tenant_id),
        session_id: Set(session_id),
        run_id: Set(run_id),
        role: Set(role_slug(message.role).to_string()),
        content: Set(message.content),
        name: Set(message.name),
        tool_call_id: Set(message.tool_call_id),
        tool_calls: Set(serde_json::to_value(message.tool_calls).map_err(json_err)?),
        metadata: Set(message.metadata),
        created_by: Set(created_by),
        created_at: sea_orm::ActiveValue::NotSet,
    }
    .insert(db)
    .await
    .map_err(db_err)
}

async fn insert_tool_trace(
    db: &DatabaseConnection,
    tenant_id: Uuid,
    session_id: Uuid,
    run_id: Uuid,
    trace: &ToolTrace,
) -> AiResult<ai_tool_traces::Model> {
    ai_tool_traces::ActiveModel {
        id: Set(Uuid::new_v4()),
        tenant_id: Set(tenant_id),
        session_id: Set(session_id),
        run_id: Set(run_id),
        tool_name: Set(trace.tool_name.clone()),
        status: Set(trace.status.clone()),
        input_payload: Set(trace.input_payload.clone()),
        output_payload: Set(trace.output_payload.clone()),
        error_message: Set(trace.error_message.clone()),
        duration_ms: Set(Some(trace.duration_ms)),
        sensitive: Set(trace.sensitive),
        created_at: Set(trace.created_at.into()),
        updated_at: Set(trace.created_at.into()),
    }
    .insert(db)
    .await
    .map_err(db_err)
}

async fn insert_approval_request(
    db: &DatabaseConnection,
    operator: &AiOperatorContext,
    session_id: Uuid,
    run_id: Uuid,
    approval: &PendingApproval,
) -> AiResult<ai_approval_requests::Model> {
    ai_approval_requests::ActiveModel {
        id: Set(Uuid::new_v4()),
        tenant_id: Set(operator.tenant_id),
        session_id: Set(session_id),
        run_id: Set(run_id),
        tool_name: Set(approval.tool_name.clone()),
        tool_call_id: Set(approval.tool_call_id.clone()),
        tool_input: Set(approval.input_payload.clone()),
        reason: Set(Some(approval.reason.clone())),
        status: Set("pending".to_string()),
        resolved_by: Set(None),
        resolved_at: Set(None),
        metadata: Set(json!({})),
        created_at: sea_orm::ActiveValue::NotSet,
        updated_at: sea_orm::ActiveValue::NotSet,
    }
    .insert(db)
    .await
    .map_err(db_err)
}

async fn persist_runtime_outputs(
    db: &DatabaseConnection,
    operator: &AiOperatorContext,
    session_id: Uuid,
    run_id: Uuid,
    messages: Vec<ChatMessage>,
    traces: Vec<ToolTrace>,
) -> AiResult<()> {
    for message in messages {
        insert_message(
            db,
            operator.tenant_id,
            session_id,
            Some(run_id),
            Some(operator.user_id),
            message,
        )
        .await?;
    }
    for trace in traces {
        insert_tool_trace(db, operator.tenant_id, session_id, run_id, &trace).await?;
    }
    let session = require_session(db, operator.tenant_id, session_id).await?;
    let mut active: ai_chat_sessions::ActiveModel = session.into();
    active.updated_at = Set(Utc::now().into());
    active.update(db).await.map_err(db_err)?;
    Ok(())
}

async fn session_has_user_messages(
    db: &DatabaseConnection,
    tenant_id: Uuid,
    session_id: Uuid,
) -> AiResult<bool> {
    let count = ai_chat_messages::Entity::find()
        .filter(
            Condition::all()
                .add(ai_chat_messages::Column::TenantId.eq(tenant_id))
                .add(ai_chat_messages::Column::SessionId.eq(session_id))
                .add(ai_chat_messages::Column::Role.eq("user")),
        )
        .count(db)
        .await
        .map_err(db_err)?;
    Ok(count > 0)
}

async fn require_provider_profile(
    db: &DatabaseConnection,
    tenant_id: Uuid,
    id: Uuid,
) -> AiResult<ai_provider_profiles::Model> {
    ai_provider_profiles::Entity::find_by_id(id)
        .filter(ai_provider_profiles::Column::TenantId.eq(tenant_id))
        .one(db)
        .await
        .map_err(db_err)?
        .ok_or_else(|| AiError::NotFound("AI provider profile not found".to_string()))
}

async fn require_tool_profile(
    db: &DatabaseConnection,
    tenant_id: Uuid,
    id: Uuid,
) -> AiResult<ai_tool_profiles::Model> {
    ai_tool_profiles::Entity::find_by_id(id)
        .filter(ai_tool_profiles::Column::TenantId.eq(tenant_id))
        .one(db)
        .await
        .map_err(db_err)?
        .ok_or_else(|| AiError::NotFound("AI tool profile not found".to_string()))
}

async fn require_task_profile(
    db: &DatabaseConnection,
    tenant_id: Uuid,
    id: Uuid,
) -> AiResult<ai_task_profiles::Model> {
    ai_task_profiles::Entity::find_by_id(id)
        .filter(ai_task_profiles::Column::TenantId.eq(tenant_id))
        .one(db)
        .await
        .map_err(db_err)?
        .ok_or_else(|| AiError::NotFound("AI task profile not found".to_string()))
}

async fn require_session(
    db: &DatabaseConnection,
    tenant_id: Uuid,
    id: Uuid,
) -> AiResult<ai_chat_sessions::Model> {
    ai_chat_sessions::Entity::find_by_id(id)
        .filter(ai_chat_sessions::Column::TenantId.eq(tenant_id))
        .one(db)
        .await
        .map_err(db_err)?
        .ok_or_else(|| AiError::NotFound("AI chat session not found".to_string()))
}

async fn require_run(
    db: &DatabaseConnection,
    tenant_id: Uuid,
    id: Uuid,
) -> AiResult<ai_chat_runs::Model> {
    ai_chat_runs::Entity::find_by_id(id)
        .filter(ai_chat_runs::Column::TenantId.eq(tenant_id))
        .one(db)
        .await
        .map_err(db_err)?
        .ok_or_else(|| AiError::NotFound("AI chat run not found".to_string()))
}

fn string_list(value: &serde_json::Value) -> Vec<String> {
    value
        .as_array()
        .into_iter()
        .flat_map(|items| items.iter())
        .filter_map(|item| item.as_str().map(|value| value.to_string()))
        .collect()
}

fn uuid_list(value: &serde_json::Value) -> Vec<Uuid> {
    string_list(value)
        .into_iter()
        .filter_map(|value| Uuid::parse_str(&value).ok())
        .collect()
}

fn capability_list(value: &serde_json::Value) -> Vec<ProviderCapability> {
    string_list(value)
        .into_iter()
        .map(|value| capability_from_slug(&value))
        .collect()
}

fn to_json_array(values: Vec<String>) -> AiResult<serde_json::Value> {
    serde_json::to_value(
        values
            .into_iter()
            .map(|value| value.trim().to_string())
            .filter(|value| !value.is_empty())
            .collect::<Vec<_>>(),
    )
    .map_err(json_err)
}

fn uuid_json_array(values: Vec<Uuid>) -> serde_json::Value {
    serde_json::to_value(
        values
            .into_iter()
            .map(|value| value.to_string())
            .collect::<Vec<_>>(),
    )
    .unwrap_or_else(|_| json!([]))
}

fn capability_json_array(values: Vec<ProviderCapability>) -> serde_json::Value {
    serde_json::to_value(
        values
            .into_iter()
            .map(|value| value.slug().to_string())
            .collect::<Vec<_>>(),
    )
    .unwrap_or_else(|_| json!([]))
}

fn normalize_metadata(value: serde_json::Value) -> serde_json::Value {
    if value.is_object() {
        value
    } else {
        json!({})
    }
}

fn normalize_base_url(value: &str) -> String {
    value.trim().trim_end_matches('/').to_string()
}

fn normalize_nonempty(value: String, fallback: &str) -> String {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        fallback.to_string()
    } else {
        trimmed.to_string()
    }
}

fn merge_metadata(base: serde_json::Value, extension: serde_json::Value) -> serde_json::Value {
    let mut merged = normalize_metadata(base);
    if let (Some(target), Some(source)) = (merged.as_object_mut(), extension.as_object()) {
        for (key, value) in source {
            target.insert(key.clone(), value.clone());
        }
    }
    merged
}

async fn list_router_provider_profiles(
    db: &DatabaseConnection,
    tenant_id: Uuid,
) -> AiResult<Vec<RouterProviderProfile>> {
    ai_provider_profiles::Entity::find()
        .filter(ai_provider_profiles::Column::TenantId.eq(tenant_id))
        .all(db)
        .await
        .map_err(db_err)?
        .into_iter()
        .map(|model| {
            Ok(RouterProviderProfile {
                id: model.id,
                slug: model.slug,
                provider_kind: provider_kind_from_slug(&model.provider_kind),
                model: model.model,
                capabilities: capability_list(&model.capabilities),
                usage_policy: ProviderUsagePolicy {
                    allowed_task_profiles: string_list(&model.allowed_task_profiles),
                    denied_task_profiles: string_list(&model.denied_task_profiles),
                    restricted_role_slugs: string_list(&model.restricted_role_slugs),
                },
                is_active: model.is_active,
            })
        })
        .collect()
}

fn validate_slug(value: &str) -> AiResult<()> {
    let slug = value.trim();
    if slug.is_empty() {
        return Err(AiError::Validation("slug is required".to_string()));
    }
    if slug.len() > 96 {
        return Err(AiError::Validation("slug is too long".to_string()));
    }
    if !slug
        .chars()
        .all(|ch| ch.is_ascii_lowercase() || ch.is_ascii_digit() || ch == '-' || ch == '_')
    {
        return Err(AiError::Validation(
            "slug must contain only lowercase letters, digits, '-' or '_'".to_string(),
        ));
    }
    Ok(())
}

fn parse_uuid_str(value: Option<&str>) -> AiResult<Uuid> {
    let value = value
        .ok_or_else(|| AiError::Runtime("tenant id is missing in AI access context".to_string()))?;
    Uuid::parse_str(value).map_err(|error| AiError::Runtime(format!("invalid uuid: {error}")))
}

fn json_err(error: impl std::fmt::Display) -> AiError {
    AiError::Serialization(error.to_string())
}

fn db_err(error: impl std::fmt::Display) -> AiError {
    AiError::Runtime(error.to_string())
}

fn to_utc(value: sea_orm::prelude::DateTimeWithTimeZone) -> DateTime<Utc> {
    value.with_timezone(&Utc)
}
