pub mod helpers;
pub mod mapping;
pub mod mcp;
pub mod types;
pub mod workflow;
pub mod profiles;

use chrono::Utc;
use sea_orm::{
    ActiveModelTrait, ActiveValue::Set, ColumnTrait, Condition, DatabaseConnection, EntityTrait,
    PaginatorTrait, QueryFilter, QueryOrder, TransactionTrait, sea_query::Expr,
};
use serde::Deserialize;
use serde_json::json;
use std::sync::{Arc, Mutex};
use uuid::Uuid;

use rustok_api::Permission;

use crate::agent_safety::{
    agent_prompt_template_evidence, bounded_tool_result, redacted_json_evidence,
    redacted_tool_execution_evidence, validate_tool_arguments, validate_tool_inventory,
};
use crate::direct::{DirectExecutionRegistry, DirectExecutionRequest};
use crate::engine::RigAgentDriver;
use crate::engine::{InferenceEngine, inference_for_slug};
use crate::entities::{
    ai_agent_workflow_stages, ai_approval_requests, ai_chat_messages, ai_chat_runs,
    ai_chat_sessions, ai_provider_profiles, ai_task_profiles, ai_tool_profiles, ai_tool_traces,
};
use crate::metrics as ai_metrics;
use crate::model::{
    AgentPromptTemplateEvidence, AgentUsageEvidence, AiRunDecisionTrace, ChatMessage,
    ChatMessageRole, ExecutionMode, ExecutionOverride, ProviderStreamEmitter, RuntimeOutcome,
    ToolTrace,
};
use crate::router::AiRouter;
use crate::{AiError, AiResult, McpClientAdapter, ProviderSlug};
use crate::{RagCoordinator, RagRetrievalStrategy, RagSearchRequest};

pub use helpers::*;
pub use mapping::*;
pub use mcp::*;
pub use types::*;
use workflow::{
    agent_execution_context_for_run, approval_execution_outcome, approval_policy_evidence,
    claim_approval_resolution, persist_approval_execution_outcome,
    transition_run_after_approval_resolution, validate_approval_policy_evidence,
    validate_approval_resolution_policy, ApprovalBatchRunTransition, ApprovalExecutionOutcome,
};

/// Identifies why a task job may select an explicit provider, model, or
/// execution mode. Only a previously validated agent model assignment can
/// bypass the caller's router-override permission; this type is private so a
/// transport caller cannot assert that authority.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum TaskJobExecutionAuthority {
    OperatorOverride,
    RegisteredAgentAssignment,
}

const MAX_RAG_CONTEXT_ATOMS: usize = 32;

pub(crate) fn decision_trace_with_prompt_template(
    current: serde_json::Value,
    prompt_template: AgentPromptTemplateEvidence,
) -> AiResult<serde_json::Value> {
    let mut trace: AiRunDecisionTrace = serde_json::from_value(current).map_err(AiError::Json)?;
    trace.prompt_template = Some(prompt_template);
    serde_json::to_value(trace).map_err(AiError::Json)
}

pub(crate) fn decision_trace_with_agent_usage(
    current: serde_json::Value,
    usage: AgentUsageEvidence,
) -> AiResult<serde_json::Value> {
    let mut trace: AiRunDecisionTrace = serde_json::from_value(current).map_err(AiError::Json)?;
    trace.agent_usage = Some(usage);
    serde_json::to_value(trace).map_err(AiError::Json)
}

#[derive(Debug, Deserialize)]
#[serde(default, deny_unknown_fields)]
struct RagTaskPolicy {
    enabled: bool,
    strategy: RagRetrievalStrategy,
    limit: usize,
    source_ids: Vec<String>,
}

impl Default for RagTaskPolicy {
    fn default() -> Self {
        Self {
            enabled: false,
            strategy: RagRetrievalStrategy::Hybrid,
            limit: 8,
            source_ids: Vec::new(),
        }
    }
}

async fn apply_rag_context(
    runtime: &AiHostRuntime,
    tenant_id: Uuid,
    task_profile: Option<&ai_task_profiles::Model>,
    mut messages: Vec<ChatMessage>,
) -> AiResult<Vec<ChatMessage>> {
    let Some(policy_value) = task_profile.and_then(|profile| profile.metadata.get("rag")) else {
        return Ok(messages);
    };
    let policy: RagTaskPolicy = serde_json::from_value(policy_value.clone()).map_err(|error| {
        AiError::Validation(format!("task profile has an invalid RAG policy: {error}"))
    })?;
    if !policy.enabled {
        return Ok(messages);
    }
    if policy.limit == 0 || policy.limit > MAX_RAG_CONTEXT_ATOMS {
        return Err(AiError::Validation(format!(
            "task profile RAG limit must be between 1 and {MAX_RAG_CONTEXT_ATOMS}"
        )));
    }
    let query = messages
        .iter()
        .rev()
        .find(|message| message.role == ChatMessageRole::User)
        .and_then(|message| message.content.as_deref())
        .map(str::trim)
        .filter(|query| !query.is_empty())
        .ok_or_else(|| {
            AiError::Validation("RAG-enabled task requires a non-empty user message".to_string())
        })?;
    let provider = runtime.rag_retrieval_port().ok_or_else(|| {
        AiError::Runtime(
            "RAG is enabled for this task, but SharedAiRagRetrievalPort is not registered"
                .to_string(),
        )
    })?;
    let coordinator = RagCoordinator::new(provider, MAX_RAG_CONTEXT_ATOMS)
        .map_err(|error| AiError::Runtime(error.to_string()))?;
    let context = coordinator
        .retrieve(RagSearchRequest {
            tenant_id,
            query: query.to_string(),
            strategy: policy.strategy,
            limit: policy.limit,
            source_ids: policy.source_ids,
        })
        .await
        .map_err(|error| AiError::Runtime(error.to_string()))?;
    messages.insert(
        0,
        context
            .to_untrusted_message()
            .map_err(|error| AiError::Runtime(error.to_string()))?,
    );
    Ok(messages)
}

pub struct AiManagementService;

pub(crate) async fn runtime_inference_engine(
    runtime: &AiHostRuntime,
    provider_slug: &ProviderSlug,
    provider_config: &crate::AiProviderConfig,
) -> AiResult<Arc<dyn InferenceEngine>> {
    #[cfg(test)]
    if let Some(engine) = runtime.test_inference_engine() {
        return Ok(engine);
    }

    Ok(Arc::<dyn InferenceEngine>::from(
        inference_for_slug(provider_slug, provider_config, runtime.secret_registry()).await?,
    ))
}

impl AiManagementService {
    pub async fn start_chat_session(
        runtime: &AiHostRuntime,
        operator: &AiOperatorContext,
        input: StartAiChatSessionInput,
    ) -> AiResult<AiSendMessageResult> {
        let db = runtime.db();
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
        enforce_task_permissions(operator, task_profile.as_ref())?;
        if input.override_config.provider_profile_id.is_some()
            || input.override_config.model.is_some()
            || input.execution_mode.is_some()
        {
            ensure_permission(operator, Permission::AI_ROUTER_OVERRIDE)?;
        }
        if let Some(tool_profile_id) = input.tool_profile_id {
            let tool_profile =
                require_tool_profile(db, operator.tenant_id, tool_profile_id).await?;
            if !tool_profile.is_active {
                return Err(AiError::Validation("tool profile is inactive".to_string()));
            }
        }
        let resolved_locale = resolve_task_locale(
            db,
            operator.tenant_id,
            operator.preferred_locale.as_deref(),
            input.locale.as_deref(),
            task_profile.as_ref().map(|profile| profile.slug.as_str()),
        )
        .await?;
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
            &operator.role_slugs,
        )?;
        let mut decision_trace = enrich_decision_trace(
            execution_plan.decision_trace,
            execution_plan.execution_mode,
            input.locale.clone(),
            resolved_locale.clone(),
        );
        if execution_plan.execution_mode == ExecutionMode::McpTooling {
            decision_trace.prompt_template = Some(agent_prompt_template_evidence(
                task_profile
                    .as_ref()
                    .and_then(|profile| profile.system_prompt.as_deref()),
                Some(resolved_locale.as_str()),
            ));
        }
        ai_metrics::observe_locale_resolution(input.locale.as_deref(), resolved_locale.as_str());
        ai_metrics::observe_router_resolution("start_chat_session", &decision_trace);

        let txn = db.begin().await.map_err(db_err)?;
        let session = ai_chat_sessions::ActiveModel {
            id: Set(Uuid::new_v4()),
            tenant_id: Set(operator.tenant_id),
            title: Set(input.title),
            provider_profile_id: Set(execution_plan.provider_profile_id),
            task_profile_id: Set(execution_plan.task_profile_id),
            tool_profile_id: Set(execution_plan.tool_profile_id),
            execution_mode: Set(execution_plan.execution_mode.slug().to_string()),
            requested_locale: Set(input.locale.clone()),
            resolved_locale: Set(resolved_locale.clone()),
            status: Set("active".to_string()),
            created_by: Set(Some(operator.user_id)),
            metadata: Set(merge_metadata(
                input.metadata,
                json!({ "decision_trace": decision_trace }),
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
            Self::execute_latest_turn(runtime, operator, session.id).await
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
                    requested_locale: detail.session.requested_locale.clone(),
                    resolved_locale: detail.session.resolved_locale.clone(),
                    temperature: detail.provider_profile.temperature,
                    max_tokens: detail.provider_profile.max_tokens,
                    error_message: None,
                    pending_approval_id: None,
                    decision_trace: crate::model::AiRunDecisionTrace::default(),
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

    pub async fn run_task_job(
        runtime: &AiHostRuntime,
        operator: &AiOperatorContext,
        input: RunAiTaskJobInput,
    ) -> AiResult<AiSendMessageResult> {
        Self::run_task_job_with_authority(
            runtime,
            operator,
            input,
            TaskJobExecutionAuthority::OperatorOverride,
        )
        .await
    }

    async fn run_task_job_with_authority(
        runtime: &AiHostRuntime,
        operator: &AiOperatorContext,
        input: RunAiTaskJobInput,
        authority: TaskJobExecutionAuthority,
    ) -> AiResult<AiSendMessageResult> {
        let db = runtime.db();
        let task_profile =
            require_task_profile(db, operator.tenant_id, input.task_profile_id).await?;
        if !task_profile.is_active {
            return Err(AiError::Validation("task profile is inactive".to_string()));
        }
        enforce_task_permissions(operator, Some(&task_profile))?;
        if authority == TaskJobExecutionAuthority::OperatorOverride
            && (input.provider_profile_id.is_some()
                || input.model_override.is_some()
                || input.execution_mode.is_some())
        {
            ensure_permission(operator, Permission::AI_ROUTER_OVERRIDE)?;
        }

        let resolved_locale = resolve_task_locale(
            db,
            operator.tenant_id,
            operator.preferred_locale.as_deref(),
            input.locale.as_deref(),
            Some(task_profile.slug.as_str()),
        )
        .await?;

        let task_profile_record = map_task_profile(task_profile.clone())?;
        let providers = list_router_provider_profiles(db, operator.tenant_id).await?;
        let execution_plan = AiRouter::resolve(
            Some(&task_profile_runtime(&task_profile_record)),
            &providers,
            input.provider_profile_id,
            task_profile.tool_profile_id,
            &ExecutionOverride {
                execution_mode: input.execution_mode,
                model: input.model_override,
                ..ExecutionOverride::default()
            },
            &operator.role_slugs,
        )?;
        let mut decision_trace = enrich_decision_trace(
            execution_plan.decision_trace,
            execution_plan.execution_mode,
            input.locale.clone(),
            resolved_locale.clone(),
        );
        if execution_plan.execution_mode == ExecutionMode::McpTooling {
            decision_trace.prompt_template = Some(agent_prompt_template_evidence(
                task_profile.system_prompt.as_deref(),
                Some(resolved_locale.as_str()),
            ));
        }
        ai_metrics::observe_locale_resolution(input.locale.as_deref(), resolved_locale.as_str());
        ai_metrics::observe_router_resolution("run_task_job", &decision_trace);

        let txn = db.begin().await.map_err(db_err)?;
        let session = ai_chat_sessions::ActiveModel {
            id: Set(Uuid::new_v4()),
            tenant_id: Set(operator.tenant_id),
            title: Set(input.title),
            provider_profile_id: Set(execution_plan.provider_profile_id),
            task_profile_id: Set(Some(task_profile.id)),
            tool_profile_id: Set(execution_plan.tool_profile_id),
            execution_mode: Set(execution_plan.execution_mode.slug().to_string()),
            requested_locale: Set(input.locale.clone()),
            resolved_locale: Set(resolved_locale.clone()),
            status: Set("active".to_string()),
            created_by: Set(Some(operator.user_id)),
            metadata: Set(merge_metadata(
                input.metadata,
                json!({
                    "decision_trace": decision_trace,
                    "task_input": input.task_input_json,
                    "task_job": true,
                }),
            )),
            created_at: sea_orm::ActiveValue::NotSet,
            updated_at: sea_orm::ActiveValue::NotSet,
        }
        .insert(&txn)
        .await
        .map_err(db_err)?;

        insert_message(
            &txn,
            operator.tenant_id,
            session.id,
            None,
            Some(operator.user_id),
            build_task_job_user_message(
                task_profile.slug.as_str(),
                input.locale.as_deref(),
                resolved_locale.as_str(),
                &input.task_input_json,
            ),
        )
        .await?;

        txn.commit().await.map_err(db_err)?;

        Self::execute_task_job_run(
            runtime,
            operator,
            session.id,
            input.task_input_json,
            input.locale,
            resolved_locale,
        )
        .await
    }

    pub async fn send_chat_message(
        runtime: &AiHostRuntime,
        operator: &AiOperatorContext,
        session_id: Uuid,
        input: SendAiChatMessageInput,
    ) -> AiResult<AiSendMessageResult> {
        let db = runtime.db();
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
        Self::execute_latest_turn(runtime, operator, session.id).await
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
                execution_mode: execution_mode_from_slug(&session.execution_mode)?,
                requested_locale: session.requested_locale,
                resolved_locale: session.resolved_locale,
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
            .collect::<AiResult<Vec<_>>>()?;
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
                execution_mode: execution_mode_from_slug(&session.execution_mode)?,
                requested_locale: session.requested_locale,
                resolved_locale: session.resolved_locale,
                status: session.status,
                created_at: to_utc(session.created_at),
                updated_at: to_utc(session.updated_at),
                latest_run_status,
                pending_approvals,
            },
            provider_profile: map_provider_profile(provider)?,
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
        runtime: &AiHostRuntime,
        operator: &AiOperatorContext,
        approval_id: Uuid,
        input: ResumeAiApprovalInput,
    ) -> AiResult<AiSendMessageResult> {
        let db = runtime.db();
        let approval = ai_approval_requests::Entity::find_by_id(approval_id)
            .filter(ai_approval_requests::Column::TenantId.eq(operator.tenant_id))
            .one(db)
            .await
            .map_err(db_err)?
            .ok_or_else(|| AiError::NotFound("approval request not found".to_string()))?;
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
        let tool_policy = policy_from_model(tool_profile.as_ref());
        validate_approval_resolution_policy(
            &approval.status,
            input.approved,
            tool_policy.is_tool_allowed(&approval.tool_name),
            &approval.tool_name,
        )?;

        let run = require_run(db, operator.tenant_id, approval.run_id).await?;
        if run.status != "waiting_approval" || run.pending_approval_id != Some(approval.id) {
            return Err(AiError::Validation(
                "approval request is not the active run approval".to_string(),
            ));
        }

        let execution_operator = agent_execution_context_for_run(db, operator, run.id)
            .await?
            .unwrap_or_else(|| operator.clone());

        // A previously staged outcome is the only case where we may replay a
        // result without revalidating and invoking the external tool. For a
        // new execution, bind the approval to the current actor-scoped MCP
        // inventory and its current typed schema before claiming the approval.
        // A revoked capability or changed schema must leave the approval
        // pending instead of turning it into an execution lease.
        let persisted_outcome = approval_execution_outcome(&approval.metadata)?;
        let expected_policy_evidence = approval_policy_evidence(&approval.metadata)?;
        let prevalidated_adapter = if input.approved && persisted_outcome.is_none() {
            let expected_policy_evidence = expected_policy_evidence.as_ref().ok_or_else(|| {
                AiError::Validation(
                    "approval request lacks immutable tool policy evidence; request a new approval"
                        .to_string(),
                )
            })?;
            let adapter = InProcessMcpAdapter::new(
                runtime,
                access_context_for_operator(&execution_operator),
            )?;
            let inventory = validate_tool_inventory(adapter.list_tools().await?)?;
            let definition = inventory.definition(&approval.tool_name).ok_or_else(|| {
                AiError::Validation(
                    "approved tool is no longer available to the execution identity".to_string(),
                )
            })?;
            validate_tool_arguments(definition, &approval.tool_input)?;
            let current_policy_evidence = tool_policy.evidence(definition);
            validate_approval_policy_evidence(expected_policy_evidence, current_policy_evidence)?;
            Some(adapter)
        } else {
            None
        };

        if !claim_approval_resolution(db, operator.tenant_id, approval.id, &approval.status).await?
        {
            return Err(AiError::Validation(
                "approval request was already claimed".to_string(),
            ));
        }

        let (tool_content, tool_metadata, trace) = if input.approved {
            let outcome = match persisted_outcome {
                Some(outcome) => outcome,
                None => {
                    let adapter = prevalidated_adapter.ok_or_else(|| {
                        AiError::Runtime(
                            "approved MCP tool was not prepared for execution".to_string(),
                        )
                    })?;
                    let started = std::time::Instant::now();
                    let tool_result = match adapter
                        .call_tool(&approval.tool_name, approval.tool_input.clone())
                        .await
                    {
                        Ok(value) => value,
                        Err(_error) => {
                            let mut retryable: ai_approval_requests::ActiveModel =
                                approval.clone().into();
                            retryable.status = Set("pending".to_string());
                            retryable.reason =
                                Set(Some("tool execution failed and may be retried".to_string()));
                            retryable.updated_at = Set(Utc::now().into());
                            retryable.update(db).await.map_err(db_err)?;
                            return Err(AiError::Mcp(
                                "approved MCP tool execution failed".to_string(),
                            ));
                        }
                    };
                    let outcome = ApprovalExecutionOutcome {
                        content: bounded_tool_result(&tool_result.content),
                        output_evidence: redacted_tool_execution_evidence(
                            &approval.tool_name,
                            &tool_result.raw_payload,
                            &tool_result.source_lineage,
                        )?,
                        duration_ms: started.elapsed().as_millis() as i64,
                    };
                    let _persisted =
                        persist_approval_execution_outcome(db, &approval, &outcome).await?;
                    outcome
                }
            };
            let trace = ToolTrace {
                tool_name: approval.tool_name.clone(),
                input_payload: redacted_json_evidence(&approval.tool_input),
                output_payload: Some(outcome.output_evidence.clone()),
                status: "completed".to_string(),
                duration_ms: outcome.duration_ms,
                sensitive: expected_policy_evidence
                    .as_ref()
                    .is_some_and(|evidence| evidence.requires_operator_approval),
                error_message: None,
                created_at: Utc::now(),
            };
            (
                outcome.content,
                json!({
                    "tool_output_evidence": outcome.output_evidence,
                    "approval_approved": true,
                    "untrusted_context": "mcp_tool_result",
                    "tool_policy_evidence": expected_policy_evidence,
                }),
                trace,
            )
        } else {
            let content = bounded_tool_result("Tool execution was rejected by the operator.");
            let trace = ToolTrace {
                tool_name: approval.tool_name.clone(),
                input_payload: redacted_json_evidence(&approval.tool_input),
                output_payload: Some(redacted_json_evidence(&json!({
                    "reason": "approval_rejected"
                }))),
                status: "rejected".to_string(),
                duration_ms: 0,
                sensitive: expected_policy_evidence
                    .as_ref()
                    .is_some_and(|evidence| evidence.requires_operator_approval),
                error_message: None,
                created_at: Utc::now(),
            };
            (
                content,
                json!({
                    "approval_rejected": true,
                    "untrusted_context": "mcp_tool_result",
                    "tool_policy_evidence": expected_policy_evidence,
                }),
                trace,
            )
        };

        // The external effect has already been durably staged above. Finalize all
        // RusToK records atomically so a database failure cannot duplicate a
        // tool trace or chat message on the next resume attempt.
        let transaction = db.begin().await.map_err(db_err)?;
        insert_tool_trace(
            &transaction,
            execution_operator.tenant_id,
            session.id,
            run.id,
            &trace,
        )
        .await?;
        insert_message(
            &transaction,
            execution_operator.tenant_id,
            session.id,
            Some(run.id),
            Some(execution_operator.user_id),
            ChatMessage {
                role: ChatMessageRole::Tool,
                content: Some(tool_content),
                name: Some(approval.tool_name.clone()),
                tool_call_id: Some(approval.tool_call_id.clone()),
                tool_calls: Vec::new(),
                metadata: tool_metadata,
            },
        )
        .await?;

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
        approval_active.update(&transaction).await.map_err(db_err)?;

        let (saved_run, transition) = transition_run_after_approval_resolution(
            &transaction,
            operator.tenant_id,
            run,
            &approval.approval_batch_id,
        )
        .await?;
        if matches!(transition, ApprovalBatchRunTransition::WaitingForNext) {
            transaction.commit().await.map_err(db_err)?;
            let detail = Self::chat_session_detail(db, operator.tenant_id, session.id)
                .await?
                .ok_or_else(|| AiError::Runtime("failed to reload AI chat session".to_string()))?;
            return Ok(AiSendMessageResult {
                session: detail,
                run: map_run_record(saved_run)?,
            });
        }

        transaction.commit().await.map_err(db_err)?;

        let result = Self::continue_run(
            runtime,
            &execution_operator,
            session.id,
            saved_run.id,
            provider,
            task_profile,
            tool_profile,
            execution_mode_from_slug(&session.execution_mode)?,
            session.requested_locale.clone(),
            session.resolved_locale.clone(),
            None,
        )
        .await?;
        Self::sync_workflow_stage_after_run(db, execution_operator.tenant_id, &result.run).await?;
        Ok(result)
    }

    async fn sync_workflow_stage_after_run(
        db: &DatabaseConnection,
        tenant_id: Uuid,
        run: &AiChatRunRecord,
    ) -> AiResult<()> {
        let stages = ai_agent_workflow_stages::Entity::find()
            .filter(ai_agent_workflow_stages::Column::TenantId.eq(tenant_id))
            .filter(ai_agent_workflow_stages::Column::RunId.eq(run.id))
            .filter(ai_agent_workflow_stages::Column::Status.eq("waiting_approval"))
            .all(db)
            .await
            .map_err(db_err)?;
        for stage in stages {
            let workflow_run_id = stage.workflow_run_id;
            let result = match run.status.as_str() {
                "completed" => ai_agent_workflow_stages::Entity::update_many()
                    .filter(ai_agent_workflow_stages::Column::TenantId.eq(tenant_id))
                    .filter(ai_agent_workflow_stages::Column::Id.eq(stage.id))
                    .filter(ai_agent_workflow_stages::Column::RunId.eq(run.id))
                    .filter(ai_agent_workflow_stages::Column::Status.eq("waiting_approval"))
                    .col_expr(
                        ai_agent_workflow_stages::Column::Status,
                        Expr::value("completed"),
                    )
                    .col_expr(
                        ai_agent_workflow_stages::Column::OutputPayload,
                        Expr::value(json!({"ai_run_id": run.id})),
                    )
                    .col_expr(
                        ai_agent_workflow_stages::Column::CompletedAt,
                        Expr::value(Utc::now()),
                    )
                    .col_expr(
                        ai_agent_workflow_stages::Column::UpdatedAt,
                        Expr::value(Utc::now()),
                    )
                    .exec(db)
                    .await
                    .map_err(db_err)?,
                "failed" => ai_agent_workflow_stages::Entity::update_many()
                    .filter(ai_agent_workflow_stages::Column::TenantId.eq(tenant_id))
                    .filter(ai_agent_workflow_stages::Column::Id.eq(stage.id))
                    .filter(ai_agent_workflow_stages::Column::RunId.eq(run.id))
                    .filter(ai_agent_workflow_stages::Column::Status.eq("waiting_approval"))
                    .col_expr(
                        ai_agent_workflow_stages::Column::Status,
                        Expr::value("failed"),
                    )
                    .col_expr(
                        ai_agent_workflow_stages::Column::ErrorMessage,
                        Expr::value(run.error_message.clone().unwrap_or_else(|| {
                            format!("workflow AI run finished with status `{}`", run.status)
                        })),
                    )
                    .col_expr(
                        ai_agent_workflow_stages::Column::CompletedAt,
                        Expr::value(Utc::now()),
                    )
                    .col_expr(
                        ai_agent_workflow_stages::Column::UpdatedAt,
                        Expr::value(Utc::now()),
                    )
                    .exec(db)
                    .await
                    .map_err(db_err)?,
                "cancelled" => ai_agent_workflow_stages::Entity::update_many()
                    .filter(ai_agent_workflow_stages::Column::TenantId.eq(tenant_id))
                    .filter(ai_agent_workflow_stages::Column::Id.eq(stage.id))
                    .filter(ai_agent_workflow_stages::Column::RunId.eq(run.id))
                    .filter(ai_agent_workflow_stages::Column::Status.eq("waiting_approval"))
                    .col_expr(
                        ai_agent_workflow_stages::Column::Status,
                        Expr::value("cancelled"),
                    )
                    .col_expr(
                        ai_agent_workflow_stages::Column::ErrorMessage,
                        Expr::value(
                            run.error_message
                                .clone()
                                .unwrap_or_else(|| "workflow AI run was cancelled".to_string()),
                        ),
                    )
                    .col_expr(
                        ai_agent_workflow_stages::Column::CompletedAt,
                        Expr::value(Utc::now()),
                    )
                    .col_expr(
                        ai_agent_workflow_stages::Column::UpdatedAt,
                        Expr::value(Utc::now()),
                    )
                    .exec(db)
                    .await
                    .map_err(db_err)?,
                _ => continue,
            };
            if result.rows_affected == 1 {
                if run.status == "completed" {
                    Self::promote_agent_workflow_stages(db, tenant_id, workflow_run_id).await?;
                } else {
                    Self::sync_agent_workflow_run_status(db, tenant_id, workflow_run_id).await?;
                }
            }
        }
        Ok(())
    }

    pub async fn cancel_run(
        runtime: &AiHostRuntime,
        operator: &AiOperatorContext,
        run_id: Uuid,
    ) -> AiResult<AiChatRunRecord> {
        let db = runtime.db();
        let run = require_run(db, operator.tenant_id, run_id).await?;
        if !matches!(run.status.as_str(), "running" | "waiting_approval") {
            return Err(AiError::Validation(
                "only running or waiting AI runs can be cancelled".to_string(),
            ));
        }
        runtime.cancel_active_run(run_id);
        let mut active: ai_chat_runs::ActiveModel = run.into();
        active.status = Set("cancelled".to_string());
        active.completed_at = Set(Some(Utc::now().into()));
        active.updated_at = Set(Utc::now().into());
        let saved = active.update(db).await.map_err(db_err)?;
        publish_ai_run_stream_event(
            saved.session_id,
            saved.id,
            crate::streaming::AiRunStreamEventKind::Cancelled,
            None,
            None,
            None,
        );
        let record = map_run_record(saved)?;
        Self::sync_workflow_stage_after_run(db, operator.tenant_id, &record).await?;
        Ok(record)
    }

    async fn execute_latest_turn(
        runtime: &AiHostRuntime,
        operator: &AiOperatorContext,
        session_id: Uuid,
    ) -> AiResult<AiSendMessageResult> {
        let db = runtime.db();
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
        let execution_mode = execution_mode_from_slug(&session.execution_mode)?;
        let requested_locale = session.requested_locale.clone();
        let resolved_locale = session.resolved_locale.clone();

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
            requested_locale: Set(requested_locale.clone()),
            resolved_locale: Set(resolved_locale.clone()),
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
            runtime,
            operator,
            session.id,
            run.id,
            provider,
            task_profile,
            tool_profile,
            execution_mode,
            requested_locale,
            resolved_locale,
            None,
        )
        .await
    }

    async fn execute_task_job_run(
        runtime: &AiHostRuntime,
        operator: &AiOperatorContext,
        session_id: Uuid,
        task_input_json: serde_json::Value,
        requested_locale: Option<String>,
        resolved_locale: String,
    ) -> AiResult<AiSendMessageResult> {
        let db = runtime.db();
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
        let execution_mode = execution_mode_from_slug(&session.execution_mode)?;

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
            requested_locale: Set(requested_locale.clone()),
            resolved_locale: Set(resolved_locale.clone()),
            temperature: Set(provider.temperature),
            max_tokens: Set(provider.max_tokens),
            error_message: Set(None),
            pending_approval_id: Set(None),
            decision_trace: Set(session
                .metadata
                .get("decision_trace")
                .cloned()
                .unwrap_or_else(|| json!({}))),
            metadata: Set(json!({ "task_input": task_input_json })),
            created_at: sea_orm::ActiveValue::NotSet,
            started_at: Set(Utc::now().into()),
            completed_at: Set(None),
            updated_at: Set(Utc::now().into()),
        }
        .insert(db)
        .await
        .map_err(db_err)?;

        Self::continue_run(
            runtime,
            operator,
            session.id,
            run.id,
            provider,
            task_profile,
            tool_profile,
            execution_mode,
            requested_locale,
            resolved_locale,
            Some(task_input_json),
        )
        .await
    }

    #[allow(clippy::too_many_arguments)]
    async fn continue_run(
        runtime: &AiHostRuntime,
        operator: &AiOperatorContext,
        session_id: Uuid,
        run_id: Uuid,
        provider_profile: ai_provider_profiles::Model,
        task_profile: Option<ai_task_profiles::Model>,
        tool_profile: Option<ai_tool_profiles::Model>,
        execution_mode: ExecutionMode,
        requested_locale: Option<String>,
        resolved_locale: String,
        task_input_json: Option<serde_json::Value>,
    ) -> AiResult<AiSendMessageResult> {
        let db = runtime.db();
        let run_started = std::time::Instant::now();
        let provider_slug = provider_slug_from_str(&provider_profile.provider_slug)?;
        publish_ai_run_stream_event(
            session_id,
            run_id,
            crate::streaming::AiRunStreamEventKind::Started,
            None,
            None,
            None,
        );
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

        let direct_registry = DirectExecutionRegistry::with_defaults();
        if matches!(execution_mode, ExecutionMode::Direct)
            && let (Some(task_profile), Some(handler)) = (
                task_profile.as_ref(),
                task_profile
                    .as_ref()
                    .and_then(|profile| direct_registry.handler(&profile.slug)),
            )
        {
            let stream_buffer = Arc::new(Mutex::new(String::new()));
            let stream_emitter = ProviderStreamEmitter::new({
                let stream_buffer = Arc::clone(&stream_buffer);
                move |event| {
                    publish_provider_stream_event(session_id, run_id, &stream_buffer, event)
                }
            });
            let task_input_json = match task_input_json {
                Some(task_input_json) => task_input_json,
                None => session_task_input(db, operator.tenant_id, session_id)
                    .await?
                    .ok_or_else(|| {
                        AiError::Validation(
                            "direct task execution requires task_input_json".to_string(),
                        )
                    })?,
            };
            let provider_config = provider_config(
                &provider_profile,
                runtime.provider_targets(),
                runtime.egress_policy(),
            )?;
            let provider =
                runtime_inference_engine(runtime, &provider_slug, &provider_config).await?;
            let direct_result = match handler
                .execute(
                    runtime,
                    operator,
                    DirectExecutionRequest {
                        task_slug: task_profile.slug.clone(),
                        task_input_json,
                        requested_locale: requested_locale.clone(),
                        resolved_locale: resolved_locale.clone(),
                        system_prompt: task_profile.system_prompt.clone(),
                        provider_config: provider_config.clone(),
                        provider,
                        stream_emitter: Some(stream_emitter),
                    },
                )
                .await
            {
                Ok(result) => result,
                Err(error) => {
                    mark_run_failed(db, operator.tenant_id, run_id, error.to_string()).await?;
                    publish_ai_run_stream_event(
                        session_id,
                        run_id,
                        crate::streaming::AiRunStreamEventKind::Failed,
                        None,
                        Some(read_stream_buffer(&stream_buffer)),
                        Some(error.to_string()),
                    );
                    ai_metrics::observe_run_outcome(
                        ExecutionMode::Direct,
                        Some("direct"),
                        &provider_slug,
                        Some(task_profile.slug.as_str()),
                        Some(resolved_locale.as_str()),
                        "failed",
                        run_started.elapsed().as_millis() as u64,
                    );
                    return Err(error);
                }
            };
            let mut run = require_run(db, operator.tenant_id, run_id).await?;
            persist_runtime_outputs(
                db,
                operator,
                session_id,
                run_id,
                direct_result.appended_messages,
                direct_result.traces,
            )
            .await?;
            let mut decision_trace: crate::model::AiRunDecisionTrace =
                serde_json::from_value(run.decision_trace.clone()).unwrap_or_default();
            decision_trace = enrich_decision_trace(
                decision_trace,
                ExecutionMode::Direct,
                requested_locale.clone(),
                resolved_locale.clone(),
            );
            let execution_target = format!("direct:{}", direct_result.execution_target.slug());
            decision_trace.execution_target = Some(execution_target.clone());
            let run_metadata = run.metadata.clone();
            let mut active: ai_chat_runs::ActiveModel = run.into();
            active.execution_path = Set(ExecutionMode::Direct.slug().to_string());
            active.completed_at = Set(Some(Utc::now().into()));
            active.updated_at = Set(Utc::now().into());
            active.decision_trace =
                Set(serde_json::to_value(decision_trace).unwrap_or_else(|_| json!({})));
            active.metadata = Set(merge_metadata(run_metadata, direct_result.metadata));
            active.status = Set("completed".to_string());
            run = active.update(db).await.map_err(db_err)?;
            let detail = Self::chat_session_detail(db, operator.tenant_id, session_id)
                .await?
                .ok_or_else(|| AiError::Runtime("failed to reload AI chat session".to_string()))?;
            ai_metrics::observe_run_outcome(
                ExecutionMode::Direct,
                Some(execution_target.as_str()),
                &provider_slug,
                Some(task_profile.slug.as_str()),
                Some(resolved_locale.as_str()),
                "completed",
                run_started.elapsed().as_millis() as u64,
            );
            publish_ai_run_stream_event(
                session_id,
                run_id,
                crate::streaming::AiRunStreamEventKind::Completed,
                None,
                Some(read_stream_buffer(&stream_buffer)),
                None,
            );
            return Ok(AiSendMessageResult {
                session: detail,
                run: map_run_record(run)?,
            });
        }

        let messages =
            apply_rag_context(runtime, operator.tenant_id, task_profile.as_ref(), messages).await?;
        let provider_config = provider_config(
            &provider_profile,
            runtime.provider_targets(),
            runtime.egress_policy(),
        )?;
        let provider = runtime_inference_engine(runtime, &provider_slug, &provider_config).await?;
        let access_context = access_context_for_operator(operator);
        let adapter = Arc::new(InProcessMcpAdapter::new(runtime, access_context)?);
        let policy = policy_from_model(tool_profile.as_ref());
        let agent_driver = RigAgentDriver::new(provider, adapter, policy);
        let stream_buffer = Arc::new(Mutex::new(String::new()));
        let stream_emitter = ProviderStreamEmitter::new({
            let stream_buffer = Arc::clone(&stream_buffer);
            move |event| publish_provider_stream_event(session_id, run_id, &stream_buffer, event)
        });
        let runtime_request = crate::model::RuntimeRequest {
            model: provider_profile.model.clone(),
            messages,
            temperature: provider_profile.temperature,
            max_tokens: provider_profile.max_tokens.map(|value| value.max(0) as u32),
            max_turns: 4,
            execution_mode,
            system_prompt: task_profile
                .as_ref()
                .and_then(|value| value.system_prompt.clone()),
            locale: Some(resolved_locale.clone()),
        };
        let prompt_template = agent_prompt_template_evidence(
            runtime_request.system_prompt.as_deref(),
            runtime_request.locale.as_deref(),
        );
        let run_before_provider_egress = require_run(db, operator.tenant_id, run_id).await?;
        let prompt_trace = decision_trace_with_prompt_template(
            run_before_provider_egress.decision_trace.clone(),
            prompt_template,
        )?;
        let mut active: ai_chat_runs::ActiveModel = run_before_provider_egress.into();
        active.decision_trace = Set(prompt_trace);
        active.updated_at = Set(Utc::now().into());
        active.update(db).await.map_err(db_err)?;
        let cancellation = runtime.register_run_cancellation(run_id);
        let outcome = match agent_driver
            .run(
                &provider_config,
                runtime_request,
                Some(stream_emitter),
                Some(cancellation),
            )
            .await
        {
            Ok(outcome) => outcome,
            Err(error) => {
                runtime.complete_run_cancellation(run_id);
                if error.to_string() == "AI run cancelled" {
                    return Err(error);
                }
                mark_run_failed(db, operator.tenant_id, run_id, error.to_string()).await?;
                publish_ai_run_stream_event(
                    session_id,
                    run_id,
                    crate::streaming::AiRunStreamEventKind::Failed,
                    None,
                    Some(read_stream_buffer(&stream_buffer)),
                    Some(error.to_string()),
                );
                ai_metrics::observe_run_outcome(
                    execution_mode,
                    Some(runtime_execution_target(execution_mode)),
                    &provider_slug,
                    task_profile.as_ref().map(|value| value.slug.as_str()),
                    Some(resolved_locale.as_str()),
                    "failed",
                    run_started.elapsed().as_millis() as u64,
                );
                return Err(error);
            }
        };

        let mut run = require_run(db, operator.tenant_id, run_id).await?;

        match outcome {
            RuntimeOutcome::Completed {
                appended_messages,
                traces,
                usage,
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
                let decision_trace =
                    decision_trace_with_agent_usage(run.decision_trace.clone(), usage)?;
                let mut active: ai_chat_runs::ActiveModel = run.into();
                active.status = Set("completed".to_string());
                active.completed_at = Set(Some(Utc::now().into()));
                active.updated_at = Set(Utc::now().into());
                active.decision_trace = Set(decision_trace);
                run = active.update(db).await.map_err(db_err)?;
                ai_metrics::observe_run_outcome(
                    execution_mode,
                    Some(runtime_execution_target(execution_mode)),
                    &provider_slug,
                    task_profile.as_ref().map(|value| value.slug.as_str()),
                    Some(resolved_locale.as_str()),
                    "completed",
                    run_started.elapsed().as_millis() as u64,
                );
                publish_ai_run_stream_event(
                    session_id,
                    run_id,
                    crate::streaming::AiRunStreamEventKind::Completed,
                    None,
                    Some(read_stream_buffer(&stream_buffer)),
                    None,
                );
            }
            RuntimeOutcome::Failed {
                appended_messages,
                traces,
                error_message,
                usage,
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
                let decision_trace =
                    decision_trace_with_agent_usage(run.decision_trace.clone(), usage)?;
                let mut active: ai_chat_runs::ActiveModel = run.into();
                active.status = Set("failed".to_string());
                active.error_message = Set(Some(error_message));
                active.completed_at = Set(Some(Utc::now().into()));
                active.updated_at = Set(Utc::now().into());
                active.decision_trace = Set(decision_trace);
                run = active.update(db).await.map_err(db_err)?;
                ai_metrics::observe_run_outcome(
                    execution_mode,
                    Some(runtime_execution_target(execution_mode)),
                    &provider_slug,
                    task_profile.as_ref().map(|value| value.slug.as_str()),
                    Some(resolved_locale.as_str()),
                    "failed",
                    run_started.elapsed().as_millis() as u64,
                );
                publish_ai_run_stream_event(
                    session_id,
                    run_id,
                    crate::streaming::AiRunStreamEventKind::Failed,
                    None,
                    Some(read_stream_buffer(&stream_buffer)),
                    run.error_message.clone(),
                );
            }
            RuntimeOutcome::WaitingApproval {
                appended_messages,
                traces,
                pending_approvals,
                usage,
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
                let approval_batch_id = Uuid::new_v4();
                let mut approvals = Vec::with_capacity(pending_approvals.len());
                for pending_approval in &pending_approvals {
                    approvals.push(
                        insert_approval_request(
                            db,
                            operator,
                            session_id,
                            run_id,
                            approval_batch_id,
                            pending_approval,
                        )
                        .await?,
                    );
                }
                let first_approval = approvals.first().ok_or_else(|| {
                    AiError::Runtime("waiting approval outcome has no pending calls".to_string())
                })?;
                let decision_trace =
                    decision_trace_with_agent_usage(run.decision_trace.clone(), usage)?;
                let mut active: ai_chat_runs::ActiveModel = run.into();
                active.status = Set("waiting_approval".to_string());
                active.pending_approval_id = Set(Some(first_approval.id));
                active.updated_at = Set(Utc::now().into());
                active.decision_trace = Set(decision_trace);
                run = active.update(db).await.map_err(db_err)?;
                ai_metrics::observe_run_outcome(
                    execution_mode,
                    Some(runtime_execution_target(execution_mode)),
                    &provider_slug,
                    task_profile.as_ref().map(|value| value.slug.as_str()),
                    Some(resolved_locale.as_str()),
                    "waiting_approval",
                    run_started.elapsed().as_millis() as u64,
                );
                publish_ai_run_stream_event(
                    session_id,
                    run_id,
                    crate::streaming::AiRunStreamEventKind::WaitingApproval,
                    None,
                    Some(read_stream_buffer(&stream_buffer)),
                    None,
                );
            }
        }

        runtime.complete_run_cancellation(run_id);
        let detail = Self::chat_session_detail(db, operator.tenant_id, session_id)
            .await?
            .ok_or_else(|| AiError::Runtime("failed to reload AI chat session".to_string()))?;
        Ok(AiSendMessageResult {
            session: detail,
            run: map_run_record(run)?,
        })
    }
}
