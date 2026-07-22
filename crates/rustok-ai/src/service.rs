pub mod helpers;
pub mod mapping;
pub mod mcp;
pub mod types;

use chrono::Utc;
use sea_orm::{
    ActiveModelTrait, ActiveValue::Set, ColumnTrait, Condition, DatabaseConnection, EntityTrait,
    PaginatorTrait, QueryFilter, QueryOrder, QuerySelect, TransactionTrait, sea_query::Expr,
};
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::collections::{BTreeSet, HashMap};
use std::sync::{Arc, Mutex};
use uuid::Uuid;

use rustok_api::Permission;
use rustok_api::TenantRbacCatalog;

use crate::direct::{DirectExecutionRegistry, DirectExecutionRequest};
use crate::engine::RigAgentDriver;
use crate::engine::{InferenceEngine, inference_for_slug};
use crate::entities::{
    ai_agent_model_assignments, ai_agent_principals, ai_agent_workflow_runs,
    ai_agent_workflow_stages, ai_approval_requests, ai_chat_messages, ai_chat_runs,
    ai_chat_sessions, ai_provider_profiles, ai_task_profiles, ai_tool_profiles, ai_tool_traces,
};
use crate::metrics::{self as ai_metrics, AiRuntimeMetricsSnapshot};
use crate::model::{
    ChatMessage, ChatMessageRole, ExecutionMode, ExecutionOverride, ProviderStreamEmitter,
    ProviderTestResult, RuntimeOutcome, ToolTrace,
};
use crate::router::AiRouter;
use crate::streaming::{AiRunStreamEvent, ai_run_stream_hub};
use crate::{AiError, AiResult, McpClientAdapter, ProviderSlug};
use crate::{RagCoordinator, RagRetrievalStrategy, RagSearchRequest};

pub use helpers::*;
pub use mapping::*;
pub use mcp::*;
pub use types::*;

/// Identifies why a task job may select an explicit provider, model, or
/// execution mode. Only a previously validated agent model assignment can
/// bypass the caller's router-override permission; this type is private so a
/// transport caller cannot assert that authority.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum TaskJobExecutionAuthority {
    OperatorOverride,
    RegisteredAgentAssignment,
}

const MAX_RAG_CONTEXT_ATOMS: usize = 32;

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
            .to_system_message()
            .map_err(|error| AiError::Runtime(error.to_string()))?,
    );
    Ok(messages)
}

fn ensure_agent_provider_capabilities(
    provider: &ai_provider_profiles::Model,
    descriptor: &crate::AgentDescriptor,
) -> AiResult<()> {
    let provider_capabilities = capability_list(&provider.capabilities)?;
    if descriptor
        .required_capabilities
        .iter()
        .any(|capability| !provider_capabilities.contains(capability))
    {
        return Err(AiError::Validation(
            "provider profile does not satisfy the agent descriptor capabilities".to_string(),
        ));
    }
    Ok(())
}

/// Resolves the persisted authority of an agent principal from deployment-owned
/// RBAC roles. A descriptor's permission floor must be fully granted by the
/// selected roles; callers can never submit arbitrary permission strings.
fn resolve_agent_principal_rbac(
    tenant_rbac_catalog: &dyn TenantRbacCatalog,
    tenant_id: Uuid,
    requested_role_slugs: Vec<String>,
    descriptor: &crate::AgentDescriptor,
) -> AiResult<(Vec<String>, Vec<String>)> {
    let role_slugs = requested_role_slugs
        .into_iter()
        .map(|slug| slug.trim().to_string())
        .collect::<BTreeSet<_>>();
    if role_slugs.contains("") {
        return Err(AiError::Validation(
            "agent role slugs must not be empty".to_string(),
        ));
    }
    let role_slugs = role_slugs.into_iter().collect::<Vec<_>>();
    tenant_rbac_catalog
        .validate_assignment(tenant_id, &role_slugs, &[])
        .map_err(|error| AiError::Validation(error.to_string()))?;

    let selected_roles = role_slugs.iter().collect::<BTreeSet<_>>();
    let permission_slugs = tenant_rbac_catalog
        .roles(tenant_id)
        .into_iter()
        .filter(|role| selected_roles.contains(&role.slug))
        .flat_map(|role| role.permission_slugs)
        .collect::<BTreeSet<_>>();
    if !descriptor.required_permissions.is_subset(&permission_slugs) {
        return Err(AiError::Validation(format!(
            "selected agent roles do not grant every permission required by descriptor `{}`",
            descriptor.slug
        )));
    }
    Ok((role_slugs, permission_slugs.into_iter().collect()))
}

/// Canonical aggregate state for a workflow derived from its persisted stage
/// states. Keep this pure so terminal semantics are regression-testable
/// without a database runtime.
fn aggregate_agent_workflow_status<'a>(
    stage_statuses: impl Iterator<Item = &'a str>,
) -> &'static str {
    let stage_statuses = stage_statuses.collect::<Vec<_>>();
    if stage_statuses.contains(&"failed") {
        "failed"
    } else if stage_statuses.contains(&"cancelled") {
        "cancelled"
    } else if !stage_statuses.is_empty()
        && stage_statuses.iter().all(|status| *status == "completed")
    {
        "completed"
    } else if stage_statuses.contains(&"waiting_approval") {
        "waiting_approval"
    } else if stage_statuses.contains(&"running") {
        "running"
    } else {
        "queued"
    }
}

/// Reconstructs the authority of the workflow initiator, then constrains it
/// to the owner descriptor. Scheduler credentials must never become the
/// authority of a tenant agent.
fn agent_workflow_execution_context(
    scheduler_operator: &AiOperatorContext,
    workflow_run: &ai_agent_workflow_runs::Model,
    catalog: &crate::AgentCatalog,
    principal: &crate::AgentPrincipal,
) -> AiResult<AiOperatorContext> {
    let context = workflow_run
        .metadata
        .get("agent_execution_context")
        .and_then(serde_json::Value::as_object)
        .ok_or_else(|| {
            AiError::Validation(
                "workflow run is missing its persisted agent execution context".to_string(),
            )
        })?;
    let persisted_permissions = context
        .get("initiator_permissions")
        .and_then(serde_json::Value::as_array)
        .ok_or_else(|| {
            AiError::Validation(
                "workflow run is missing its persisted initiator permissions".to_string(),
            )
        })?
        .iter()
        .map(|permission| {
            permission.as_str().map(str::to_owned).ok_or_else(|| {
                AiError::Validation(
                    "workflow run has an invalid persisted initiator permission".to_string(),
                )
            })
        })
        .collect::<AiResult<std::collections::BTreeSet<_>>>()?;
    let effective_permissions = catalog.effective_permissions(&persisted_permissions, principal)?;
    let permissions = effective_permissions
        .into_iter()
        .map(|permission| {
            permission
                .parse::<Permission>()
                .map_err(AiError::Validation)
        })
        .collect::<AiResult<Vec<_>>>()?;
    let preferred_locale = context
        .get("preferred_locale")
        .and_then(serde_json::Value::as_str)
        .map(str::trim)
        .filter(|locale| !locale.is_empty())
        .map(str::to_owned);
    Ok(AiOperatorContext {
        tenant_id: scheduler_operator.tenant_id,
        user_id: workflow_run.initiator_id,
        permissions,
        role_slugs: principal.role_slugs.iter().cloned().collect(),
        preferred_locale,
    })
}

/// Returns the constrained authority for an already-created agent run. Normal
/// interactive runs deliberately return `None` and retain their caller
/// context; the durable stage association is the boundary discriminator.
async fn agent_execution_context_for_run(
    db: &DatabaseConnection,
    scheduler_operator: &AiOperatorContext,
    run_id: Uuid,
) -> AiResult<Option<AiOperatorContext>> {
    let Some(stage) = ai_agent_workflow_stages::Entity::find()
        .filter(ai_agent_workflow_stages::Column::TenantId.eq(scheduler_operator.tenant_id))
        .filter(ai_agent_workflow_stages::Column::RunId.eq(run_id))
        .one(db)
        .await
        .map_err(db_err)?
    else {
        return Ok(None);
    };
    let workflow_run = ai_agent_workflow_runs::Entity::find_by_id(stage.workflow_run_id)
        .filter(ai_agent_workflow_runs::Column::TenantId.eq(scheduler_operator.tenant_id))
        .one(db)
        .await
        .map_err(db_err)?
        .ok_or_else(|| {
            AiError::Validation("agent run parent workflow is unavailable".to_string())
        })?;
    if !matches!(
        workflow_run.status.as_str(),
        "queued" | "running" | "waiting_approval"
    ) {
        return Err(AiError::Validation(
            "agent run parent workflow is terminal".to_string(),
        ));
    }
    let principal = ai_agent_principals::Entity::find_by_id(stage.agent_principal_id)
        .filter(ai_agent_principals::Column::TenantId.eq(scheduler_operator.tenant_id))
        .filter(ai_agent_principals::Column::IsActive.eq(true))
        .one(db)
        .await
        .map_err(db_err)?
        .ok_or_else(|| AiError::Validation("agent run principal is unavailable".to_string()))?;
    let catalog = crate::agent_catalog()?;
    let principal_contract = crate::AgentPrincipal {
        id: principal.id,
        tenant_id: principal.tenant_id,
        agent_slug: principal.descriptor_slug,
        role_slugs: string_list(&principal.role_slugs).into_iter().collect(),
        permission_slugs: string_list(&principal.permission_slugs)
            .into_iter()
            .collect(),
    };
    Ok(Some(agent_workflow_execution_context(
        scheduler_operator,
        &workflow_run,
        &catalog,
        &principal_contract,
    )?))
}

pub struct AiManagementService;

async fn runtime_inference_engine(
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
    /// Atomically claims a scheduler-ready stage. Only the holder of the
    /// returned lease token may later persist its execution result.
    pub async fn claim_agent_workflow_stage(
        db: &DatabaseConnection,
        tenant_id: Uuid,
        stage_id: Uuid,
        lease_token: Uuid,
        lease_expires_at: chrono::DateTime<Utc>,
    ) -> AiResult<bool> {
        let stage = ai_agent_workflow_stages::Entity::find_by_id(stage_id)
            .filter(ai_agent_workflow_stages::Column::TenantId.eq(tenant_id))
            .filter(ai_agent_workflow_stages::Column::Status.eq("ready"))
            .one(db)
            .await
            .map_err(db_err)?;
        let Some(stage) = stage else {
            return Ok(false);
        };
        let executable_workflow = ai_agent_workflow_runs::Entity::find_by_id(stage.workflow_run_id)
            .filter(ai_agent_workflow_runs::Column::TenantId.eq(tenant_id))
            .filter(
                Condition::any()
                    .add(ai_agent_workflow_runs::Column::Status.eq("queued"))
                    .add(ai_agent_workflow_runs::Column::Status.eq("running"))
                    .add(ai_agent_workflow_runs::Column::Status.eq("waiting_approval")),
            )
            .one(db)
            .await
            .map_err(db_err)?
            .is_some();
        if !executable_workflow {
            return Ok(false);
        }
        let claimed = ai_agent_workflow_stages::Entity::update_many()
            .filter(ai_agent_workflow_stages::Column::TenantId.eq(tenant_id))
            .filter(ai_agent_workflow_stages::Column::Id.eq(stage_id))
            .filter(ai_agent_workflow_stages::Column::Status.eq("ready"))
            .col_expr(
                ai_agent_workflow_stages::Column::Status,
                Expr::value("running"),
            )
            .col_expr(
                ai_agent_workflow_stages::Column::LeaseToken,
                Expr::value(lease_token),
            )
            .col_expr(
                ai_agent_workflow_stages::Column::LeaseExpiresAt,
                Expr::value(lease_expires_at),
            )
            .col_expr(
                ai_agent_workflow_stages::Column::AttemptCount,
                Expr::col(ai_agent_workflow_stages::Column::AttemptCount).add(1),
            )
            .col_expr(
                ai_agent_workflow_stages::Column::StartedAt,
                Expr::value(Utc::now()),
            )
            .col_expr(
                ai_agent_workflow_stages::Column::UpdatedAt,
                Expr::value(Utc::now()),
            )
            .exec(db)
            .await
            .map_err(db_err)?;
        if claimed.rows_affected != 1 {
            return Ok(false);
        }
        Self::sync_agent_workflow_run_status(db, tenant_id, stage.workflow_run_id).await?;
        Ok(true)
    }

    /// Requeues abandoned stage claims. A caller should invoke this from the
    /// module-owned scheduler loop before looking for newly ready work.
    pub async fn requeue_expired_agent_stage_leases(
        db: &DatabaseConnection,
        tenant_id: Uuid,
        now: chrono::DateTime<Utc>,
    ) -> AiResult<u64> {
        let affected_workflow_runs = ai_agent_workflow_stages::Entity::find()
            .filter(ai_agent_workflow_stages::Column::TenantId.eq(tenant_id))
            .filter(ai_agent_workflow_stages::Column::Status.eq("running"))
            .filter(ai_agent_workflow_stages::Column::LeaseExpiresAt.lt(now))
            .select_only()
            .column(ai_agent_workflow_stages::Column::WorkflowRunId)
            .into_tuple::<Uuid>()
            .all(db)
            .await
            .map_err(db_err)?
            .into_iter()
            .collect::<std::collections::BTreeSet<_>>();
        let result = ai_agent_workflow_stages::Entity::update_many()
            .filter(ai_agent_workflow_stages::Column::TenantId.eq(tenant_id))
            .filter(ai_agent_workflow_stages::Column::Status.eq("running"))
            .filter(ai_agent_workflow_stages::Column::LeaseExpiresAt.lt(now))
            .col_expr(
                ai_agent_workflow_stages::Column::Status,
                Expr::value("ready"),
            )
            .col_expr(
                ai_agent_workflow_stages::Column::LeaseToken,
                Expr::cust("NULL"),
            )
            .col_expr(
                ai_agent_workflow_stages::Column::LeaseExpiresAt,
                Expr::cust("NULL"),
            )
            .col_expr(
                ai_agent_workflow_stages::Column::UpdatedAt,
                Expr::value(Utc::now()),
            )
            .exec(db)
            .await
            .map_err(db_err)?;
        if result.rows_affected > 0 {
            for workflow_run_id in affected_workflow_runs {
                Self::sync_agent_workflow_run_status(db, tenant_id, workflow_run_id).await?;
            }
        }
        Ok(result.rows_affected)
    }

    /// Finishes a stage only when the scheduler still owns its active lease.
    pub async fn complete_agent_workflow_stage(
        db: &DatabaseConnection,
        tenant_id: Uuid,
        stage_id: Uuid,
        lease_token: Uuid,
        output_payload: serde_json::Value,
    ) -> AiResult<bool> {
        let stage = ai_agent_workflow_stages::Entity::find_by_id(stage_id)
            .filter(ai_agent_workflow_stages::Column::TenantId.eq(tenant_id))
            .filter(ai_agent_workflow_stages::Column::Status.eq("running"))
            .filter(ai_agent_workflow_stages::Column::LeaseToken.eq(lease_token))
            .filter(ai_agent_workflow_stages::Column::LeaseExpiresAt.gte(Utc::now()))
            .one(db)
            .await
            .map_err(db_err)?;
        let Some(stage) = stage else {
            return Ok(false);
        };
        let result = ai_agent_workflow_stages::Entity::update_many()
            .filter(ai_agent_workflow_stages::Column::TenantId.eq(tenant_id))
            .filter(ai_agent_workflow_stages::Column::Id.eq(stage_id))
            .filter(ai_agent_workflow_stages::Column::Status.eq("running"))
            .filter(ai_agent_workflow_stages::Column::LeaseToken.eq(lease_token))
            .filter(ai_agent_workflow_stages::Column::LeaseExpiresAt.gte(Utc::now()))
            .col_expr(
                ai_agent_workflow_stages::Column::Status,
                Expr::value("completed"),
            )
            .col_expr(
                ai_agent_workflow_stages::Column::OutputPayload,
                Expr::value(normalize_metadata(output_payload)),
            )
            .col_expr(
                ai_agent_workflow_stages::Column::LeaseToken,
                Expr::cust("NULL"),
            )
            .col_expr(
                ai_agent_workflow_stages::Column::LeaseExpiresAt,
                Expr::cust("NULL"),
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
            .map_err(db_err)?;
        if result.rows_affected != 1 {
            return Ok(false);
        }
        Self::promote_agent_workflow_stages(db, tenant_id, stage.workflow_run_id).await?;
        Self::sync_agent_workflow_run_status(db, tenant_id, stage.workflow_run_id).await?;
        Ok(true)
    }

    /// Promotes persisted pending stages after dependency completion. Approval
    /// gates are retained instead of being implicitly scheduled.
    pub async fn promote_agent_workflow_stages(
        db: &DatabaseConnection,
        tenant_id: Uuid,
        workflow_run_id: Uuid,
    ) -> AiResult<u64> {
        let stages = ai_agent_workflow_stages::Entity::find()
            .filter(ai_agent_workflow_stages::Column::TenantId.eq(tenant_id))
            .filter(ai_agent_workflow_stages::Column::WorkflowRunId.eq(workflow_run_id))
            .all(db)
            .await
            .map_err(db_err)?;
        let completed = stages
            .iter()
            .filter(|stage| stage.status == "completed")
            .map(|stage| stage.stage_id.clone())
            .collect::<std::collections::BTreeSet<_>>();
        let now = Utc::now();
        let mut promoted = 0;
        for stage in stages.into_iter().filter(|stage| stage.status == "pending") {
            let dependencies = stage
                .metadata
                .get("depends_on")
                .map(string_list)
                .unwrap_or_default();
            if dependencies
                .iter()
                .all(|dependency| completed.contains(dependency.as_str()))
            {
                let requires_approval = stage.requires_approval;
                let result = ai_agent_workflow_stages::Entity::update_many()
                    .filter(ai_agent_workflow_stages::Column::TenantId.eq(tenant_id))
                    .filter(ai_agent_workflow_stages::Column::Id.eq(stage.id))
                    .filter(ai_agent_workflow_stages::Column::Status.eq("pending"))
                    .col_expr(
                        ai_agent_workflow_stages::Column::Status,
                        Expr::value(if requires_approval {
                            "waiting_approval"
                        } else {
                            "ready"
                        }),
                    )
                    .col_expr(
                        ai_agent_workflow_stages::Column::UpdatedAt,
                        Expr::value(now),
                    )
                    .exec(db)
                    .await
                    .map_err(db_err)?;
                promoted += result.rows_affected;
            }
        }
        Self::sync_agent_workflow_run_status(db, tenant_id, workflow_run_id).await?;
        Ok(promoted)
    }

    /// Derives the durable workflow status from canonical persisted stages.
    /// Scheduler loops and approval transports must not maintain their own
    /// competing aggregate state machine.
    async fn sync_agent_workflow_run_status(
        db: &DatabaseConnection,
        tenant_id: Uuid,
        workflow_run_id: Uuid,
    ) -> AiResult<()> {
        let stages = ai_agent_workflow_stages::Entity::find()
            .filter(ai_agent_workflow_stages::Column::TenantId.eq(tenant_id))
            .filter(ai_agent_workflow_stages::Column::WorkflowRunId.eq(workflow_run_id))
            .all(db)
            .await
            .map_err(db_err)?;
        let Some(run) = ai_agent_workflow_runs::Entity::find_by_id(workflow_run_id)
            .filter(ai_agent_workflow_runs::Column::TenantId.eq(tenant_id))
            .one(db)
            .await
            .map_err(db_err)?
        else {
            return Ok(());
        };
        if matches!(run.status.as_str(), "failed" | "cancelled" | "completed") {
            return Ok(());
        }
        let status =
            aggregate_agent_workflow_status(stages.iter().map(|stage| stage.status.as_str()));
        if run.status == status {
            return Ok(());
        }
        let now = Utc::now();
        let set_started_at = status == "running" && run.started_at.is_none();
        let set_completed_at = matches!(status, "completed" | "failed" | "cancelled");
        let mut active: ai_agent_workflow_runs::ActiveModel = run.into();
        active.status = Set(status.to_string());
        if set_started_at {
            active.started_at = Set(Some(now.into()));
        }
        if set_completed_at {
            active.completed_at = Set(Some(now.into()));
        }
        active.updated_at = Set(now.into());
        active.update(db).await.map_err(db_err)?;
        Ok(())
    }

    /// Resolves an owner-declared stage admission gate. The compare-and-set on
    /// `waiting_approval` prevents two operators from approving or rejecting
    /// the same stage, and model-tool approvals remain a separate lifecycle.
    pub async fn resolve_agent_workflow_stage_approval(
        db: &DatabaseConnection,
        operator: &AiOperatorContext,
        stage_id: Uuid,
        input: ResolveAiAgentWorkflowStageApprovalInput,
    ) -> AiResult<bool> {
        ensure_permission(operator, Permission::AI_APPROVALS_RESOLVE)?;
        let stage = ai_agent_workflow_stages::Entity::find_by_id(stage_id)
            .filter(ai_agent_workflow_stages::Column::TenantId.eq(operator.tenant_id))
            .filter(ai_agent_workflow_stages::Column::Status.eq("waiting_approval"))
            .filter(ai_agent_workflow_stages::Column::RequiresApproval.eq(true))
            .filter(ai_agent_workflow_stages::Column::RunId.is_null())
            .one(db)
            .await
            .map_err(db_err)?
            .ok_or_else(|| {
                AiError::Validation(
                    "workflow stage is not awaiting an admission approval".to_string(),
                )
            })?;
        let parent_is_active = ai_agent_workflow_runs::Entity::find_by_id(stage.workflow_run_id)
            .filter(ai_agent_workflow_runs::Column::TenantId.eq(operator.tenant_id))
            .filter(
                Condition::any()
                    .add(ai_agent_workflow_runs::Column::Status.eq("queued"))
                    .add(ai_agent_workflow_runs::Column::Status.eq("running"))
                    .add(ai_agent_workflow_runs::Column::Status.eq("waiting_approval")),
            )
            .one(db)
            .await
            .map_err(db_err)?
            .is_some();
        if !parent_is_active {
            return Err(AiError::Validation(
                "workflow stage parent run is terminal".to_string(),
            ));
        }
        let now = Utc::now();
        let approval_reason = input.reason.clone();
        let mut metadata = normalize_metadata(stage.metadata.clone());
        metadata["stage_approval"] = json!({
            "approved": input.approved,
            "reason": approval_reason.clone(),
            "resolved_by": operator.user_id,
            "resolved_at": now.clone(),
        });
        let status = if input.approved { "ready" } else { "failed" };
        let result = ai_agent_workflow_stages::Entity::update_many()
            .filter(ai_agent_workflow_stages::Column::TenantId.eq(operator.tenant_id))
            .filter(ai_agent_workflow_stages::Column::Id.eq(stage_id))
            .filter(ai_agent_workflow_stages::Column::Status.eq("waiting_approval"))
            .filter(ai_agent_workflow_stages::Column::RunId.is_null())
            .col_expr(
                ai_agent_workflow_stages::Column::Status,
                Expr::value(status),
            )
            .col_expr(
                ai_agent_workflow_stages::Column::Metadata,
                Expr::value(metadata),
            )
            .col_expr(
                ai_agent_workflow_stages::Column::ErrorMessage,
                Expr::value((!input.approved).then(|| {
                    approval_reason
                        .clone()
                        .unwrap_or_else(|| "workflow stage approval was rejected".to_string())
                })),
            )
            .col_expr(
                ai_agent_workflow_stages::Column::CompletedAt,
                if input.approved {
                    Expr::cust("NULL")
                } else {
                    Expr::value(now)
                },
            )
            .col_expr(
                ai_agent_workflow_stages::Column::UpdatedAt,
                Expr::value(now),
            )
            .exec(db)
            .await
            .map_err(db_err)?;
        if result.rows_affected != 1 {
            return Ok(false);
        }
        if !input.approved {
            ai_agent_workflow_runs::Entity::update_many()
                .filter(ai_agent_workflow_runs::Column::TenantId.eq(operator.tenant_id))
                .filter(ai_agent_workflow_runs::Column::Id.eq(stage.workflow_run_id))
                .filter(
                    Condition::any()
                        .add(ai_agent_workflow_runs::Column::Status.eq("queued"))
                        .add(ai_agent_workflow_runs::Column::Status.eq("running"))
                        .add(ai_agent_workflow_runs::Column::Status.eq("waiting_approval")),
                )
                .col_expr(
                    ai_agent_workflow_runs::Column::Status,
                    Expr::value("failed"),
                )
                .col_expr(
                    ai_agent_workflow_runs::Column::OutputPayload,
                    Expr::value(json!({
                        "rejected_stage_id": stage.stage_id,
                        "reason": approval_reason.clone(),
                    })),
                )
                .col_expr(
                    ai_agent_workflow_runs::Column::CompletedAt,
                    Expr::value(now),
                )
                .col_expr(ai_agent_workflow_runs::Column::UpdatedAt, Expr::value(now))
                .exec(db)
                .await
                .map_err(db_err)?;
        } else {
            Self::sync_agent_workflow_run_status(db, operator.tenant_id, stage.workflow_run_id)
                .await?;
        }
        Ok(true)
    }

    /// Executes a claimed Alloy workflow stage through the canonical task-run
    /// path. It does not create a parallel provider or tool execution path.
    pub async fn execute_agent_workflow_stage(
        runtime: &AiHostRuntime,
        operator: &AiOperatorContext,
        stage_id: Uuid,
        lease_token: Uuid,
    ) -> AiResult<AiChatRunRecord> {
        let db = runtime.db();
        let stage = ai_agent_workflow_stages::Entity::find_by_id(stage_id)
            .filter(ai_agent_workflow_stages::Column::TenantId.eq(operator.tenant_id))
            .filter(ai_agent_workflow_stages::Column::Status.eq("running"))
            .filter(ai_agent_workflow_stages::Column::LeaseToken.eq(lease_token))
            .filter(ai_agent_workflow_stages::Column::LeaseExpiresAt.gte(Utc::now()))
            .one(db)
            .await
            .map_err(db_err)?
            .ok_or_else(|| {
                AiError::Validation("workflow stage is not owned by this lease".to_string())
            })?;
        let workflow_run = ai_agent_workflow_runs::Entity::find_by_id(stage.workflow_run_id)
            .filter(ai_agent_workflow_runs::Column::TenantId.eq(operator.tenant_id))
            .one(db)
            .await
            .map_err(db_err)?
            .ok_or_else(|| {
                AiError::Validation("workflow stage parent run is unavailable".to_string())
            })?;
        if !matches!(
            workflow_run.status.as_str(),
            "queued" | "running" | "waiting_approval"
        ) {
            return Err(AiError::Validation(
                "workflow stage parent run is terminal".to_string(),
            ));
        }
        let principal = ai_agent_principals::Entity::find_by_id(stage.agent_principal_id)
            .filter(ai_agent_principals::Column::TenantId.eq(operator.tenant_id))
            .filter(ai_agent_principals::Column::IsActive.eq(true))
            .one(db)
            .await
            .map_err(db_err)?
            .ok_or_else(|| {
                AiError::Validation("workflow stage agent principal is unavailable".to_string())
            })?;
        let catalog = crate::agent_catalog()?;
        let principal_contract = crate::AgentPrincipal {
            id: principal.id,
            tenant_id: principal.tenant_id,
            agent_slug: principal.descriptor_slug.clone(),
            role_slugs: string_list(&principal.role_slugs).into_iter().collect(),
            permission_slugs: string_list(&principal.permission_slugs)
                .into_iter()
                .collect(),
        };
        let agent_operator = agent_workflow_execution_context(
            operator,
            &workflow_run,
            &catalog,
            &principal_contract,
        )?;
        let descriptor = catalog
            .descriptor(&principal.descriptor_slug)
            .filter(|descriptor| descriptor.owner == principal.descriptor_owner)
            .cloned()
            .ok_or_else(|| {
                AiError::Validation("agent descriptor is no longer available".to_string())
            })?;
        let execution =
            catalog.validate_stage_execution(&principal.descriptor_slug, &stage.input_payload)?;
        let assignment_id = stage.model_assignment_id.ok_or_else(|| {
            AiError::Validation("workflow stage has no model assignment".to_string())
        })?;
        let assignment = ai_agent_model_assignments::Entity::find_by_id(assignment_id)
            .filter(ai_agent_model_assignments::Column::TenantId.eq(operator.tenant_id))
            .filter(ai_agent_model_assignments::Column::AgentPrincipalId.eq(principal.id))
            .filter(ai_agent_model_assignments::Column::IsActive.eq(true))
            .one(db)
            .await
            .map_err(db_err)?
            .ok_or_else(|| {
                AiError::Validation("workflow stage model assignment is unavailable".to_string())
            })?;
        let provider =
            require_provider_profile(db, operator.tenant_id, assignment.provider_profile_id)
                .await?;
        if !provider.is_active {
            return Err(AiError::Validation(
                "workflow stage provider profile is inactive".to_string(),
            ));
        }
        ensure_agent_provider_capabilities(&provider, &descriptor)?;
        let task_profile = ai_task_profiles::Entity::find()
            .filter(ai_task_profiles::Column::TenantId.eq(operator.tenant_id))
            .filter(ai_task_profiles::Column::Slug.eq(execution.task_slug.as_str()))
            .filter(ai_task_profiles::Column::IsActive.eq(true))
            .one(db)
            .await
            .map_err(db_err)?
            .ok_or_else(|| {
                AiError::Validation(format!(
                    "active task profile `{}` required by the Alloy stage is unavailable",
                    execution.task_slug
                ))
            })?;

        let result = Self::run_task_job_with_authority(
            runtime,
            &agent_operator,
            RunAiTaskJobInput {
                title: format!("{}: {}", principal.slug, stage.stage_id),
                provider_profile_id: Some(assignment.provider_profile_id),
                model_override: assignment.model_override,
                task_profile_id: task_profile.id,
                execution_mode: Some(execution_mode_from_slug(&assignment.execution_mode)?),
                locale: agent_operator.preferred_locale.clone(),
                task_input_json: stage.input_payload.clone(),
                metadata: json!({
                    "agent_workflow_run_id": stage.workflow_run_id,
                    "agent_workflow_stage_id": stage.id,
                    "agent_principal_id": principal.id,
                }),
            },
            TaskJobExecutionAuthority::RegisteredAgentAssignment,
        )
        .await;

        match result {
            Ok(result) => {
                let run = result.run;
                let recorded = ai_agent_workflow_stages::Entity::update_many()
                    .filter(ai_agent_workflow_stages::Column::TenantId.eq(operator.tenant_id))
                    .filter(ai_agent_workflow_stages::Column::Id.eq(stage_id))
                    .filter(ai_agent_workflow_stages::Column::Status.eq("running"))
                    .filter(ai_agent_workflow_stages::Column::LeaseToken.eq(lease_token))
                    .filter(ai_agent_workflow_stages::Column::LeaseExpiresAt.gte(Utc::now()))
                    .col_expr(ai_agent_workflow_stages::Column::RunId, Expr::value(run.id))
                    .col_expr(
                        ai_agent_workflow_stages::Column::UpdatedAt,
                        Expr::value(Utc::now()),
                    )
                    .exec(db)
                    .await
                    .map_err(db_err)?;
                if recorded.rows_affected != 1 {
                    return Err(AiError::Validation(
                        "workflow stage lease expired before its AI run could be recorded"
                            .to_string(),
                    ));
                }
                if run.status == "completed" {
                    Self::complete_agent_workflow_stage(
                        db,
                        operator.tenant_id,
                        stage_id,
                        lease_token,
                        json!({"ai_run_id": run.id}),
                    )
                    .await?;
                } else if run.status == "waiting_approval" {
                    let waiting = ai_agent_workflow_stages::Entity::update_many()
                        .filter(ai_agent_workflow_stages::Column::TenantId.eq(operator.tenant_id))
                        .filter(ai_agent_workflow_stages::Column::Id.eq(stage_id))
                        .filter(ai_agent_workflow_stages::Column::Status.eq("running"))
                        .filter(ai_agent_workflow_stages::Column::LeaseToken.eq(lease_token))
                        .filter(ai_agent_workflow_stages::Column::LeaseExpiresAt.gte(Utc::now()))
                        .col_expr(
                            ai_agent_workflow_stages::Column::Status,
                            Expr::value("waiting_approval"),
                        )
                        .col_expr(
                            ai_agent_workflow_stages::Column::LeaseToken,
                            Expr::cust("NULL"),
                        )
                        .col_expr(
                            ai_agent_workflow_stages::Column::LeaseExpiresAt,
                            Expr::cust("NULL"),
                        )
                        .exec(db)
                        .await
                        .map_err(db_err)?;
                    if waiting.rows_affected != 1 {
                        return Err(AiError::Validation(
                            "workflow stage lease expired before its AI approval state could be recorded"
                                .to_string(),
                        ));
                    }
                    Self::sync_agent_workflow_run_status(
                        db,
                        operator.tenant_id,
                        stage.workflow_run_id,
                    )
                    .await?;
                }
                Ok(run)
            }
            Err(error) => {
                let workflow_run_id = stage.workflow_run_id;
                let failed = ai_agent_workflow_stages::Entity::update_many()
                    .filter(ai_agent_workflow_stages::Column::TenantId.eq(operator.tenant_id))
                    .filter(ai_agent_workflow_stages::Column::Id.eq(stage_id))
                    .filter(ai_agent_workflow_stages::Column::Status.eq("running"))
                    .filter(ai_agent_workflow_stages::Column::LeaseToken.eq(lease_token))
                    .filter(ai_agent_workflow_stages::Column::LeaseExpiresAt.gte(Utc::now()))
                    .col_expr(
                        ai_agent_workflow_stages::Column::Status,
                        Expr::value("failed"),
                    )
                    .col_expr(
                        ai_agent_workflow_stages::Column::ErrorMessage,
                        Expr::value(error.to_string()),
                    )
                    .col_expr(
                        ai_agent_workflow_stages::Column::LeaseToken,
                        Expr::cust("NULL"),
                    )
                    .col_expr(
                        ai_agent_workflow_stages::Column::LeaseExpiresAt,
                        Expr::cust("NULL"),
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
                    .map_err(db_err)?;
                if failed.rows_affected == 1 {
                    Self::sync_agent_workflow_run_status(db, operator.tenant_id, workflow_run_id)
                        .await?;
                }
                Err(error)
            }
        }
    }

    pub async fn list_agent_principals(
        db: &DatabaseConnection,
        tenant_id: Uuid,
    ) -> AiResult<Vec<AiAgentPrincipalRecord>> {
        let principals = ai_agent_principals::Entity::find()
            .filter(ai_agent_principals::Column::TenantId.eq(tenant_id))
            .order_by_asc(ai_agent_principals::Column::Slug)
            .all(db)
            .await
            .map_err(db_err)?;
        Ok(principals.into_iter().map(map_agent_principal).collect())
    }

    pub async fn create_agent_principal(
        db: &DatabaseConnection,
        operator: &AiOperatorContext,
        tenant_rbac_catalog: &dyn TenantRbacCatalog,
        input: CreateAiAgentPrincipalInput,
    ) -> AiResult<AiAgentPrincipalRecord> {
        validate_slug(&input.slug)?;
        let catalog = crate::agent_catalog()?;
        let descriptor = catalog
            .descriptor(&input.descriptor_slug)
            .filter(|descriptor| descriptor.owner == input.descriptor_owner)
            .ok_or_else(|| {
                AiError::Validation("unknown owner-owned agent descriptor".to_string())
            })?;
        let (role_slugs, permission_slugs) = resolve_agent_principal_rbac(
            tenant_rbac_catalog,
            operator.tenant_id,
            input.role_slugs,
            descriptor,
        )?;
        let saved = ai_agent_principals::ActiveModel {
            id: Set(Uuid::new_v4()),
            tenant_id: Set(operator.tenant_id),
            slug: Set(input.slug),
            descriptor_owner: Set(input.descriptor_owner),
            descriptor_slug: Set(input.descriptor_slug),
            role_slugs: Set(json!(role_slugs)),
            permission_slugs: Set(json!(permission_slugs)),
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
        Ok(map_agent_principal(saved))
    }

    pub async fn update_agent_principal(
        db: &DatabaseConnection,
        operator: &AiOperatorContext,
        tenant_rbac_catalog: &dyn TenantRbacCatalog,
        id: Uuid,
        input: UpdateAiAgentPrincipalInput,
    ) -> AiResult<AiAgentPrincipalRecord> {
        let existing = ai_agent_principals::Entity::find_by_id(id)
            .filter(ai_agent_principals::Column::TenantId.eq(operator.tenant_id))
            .one(db)
            .await
            .map_err(db_err)?
            .ok_or_else(|| AiError::NotFound("AI agent principal not found".to_string()))?;
        let catalog = crate::agent_catalog()?;
        let descriptor = catalog
            .descriptor(&existing.descriptor_slug)
            .filter(|descriptor| descriptor.owner == existing.descriptor_owner)
            .ok_or_else(|| {
                AiError::Validation("agent descriptor is no longer available".to_string())
            })?;
        let (role_slugs, permission_slugs) = resolve_agent_principal_rbac(
            tenant_rbac_catalog,
            operator.tenant_id,
            input.role_slugs,
            descriptor,
        )?;
        let mut active: ai_agent_principals::ActiveModel = existing.into();
        active.role_slugs = Set(json!(role_slugs));
        active.permission_slugs = Set(json!(permission_slugs));
        active.metadata = Set(normalize_metadata(input.metadata));
        active.is_active = Set(input.is_active);
        active.updated_by = Set(Some(operator.user_id));
        active.updated_at = Set(Utc::now().into());
        let saved = active.update(db).await.map_err(db_err)?;
        Ok(map_agent_principal(saved))
    }

    pub async fn list_agent_model_assignments(
        db: &DatabaseConnection,
        tenant_id: Uuid,
        agent_principal_id: Uuid,
    ) -> AiResult<Vec<AiAgentModelAssignmentRecord>> {
        let items = ai_agent_model_assignments::Entity::find()
            .filter(ai_agent_model_assignments::Column::TenantId.eq(tenant_id))
            .filter(ai_agent_model_assignments::Column::AgentPrincipalId.eq(agent_principal_id))
            .order_by_asc(ai_agent_model_assignments::Column::CreatedAt)
            .all(db)
            .await
            .map_err(db_err)?;
        items.into_iter().map(map_agent_model_assignment).collect()
    }

    /// Lists the tenant-owned assignment catalog for owner-admin bootstrap
    /// surfaces, avoiding one query per principal.
    pub async fn list_tenant_agent_model_assignments(
        db: &DatabaseConnection,
        tenant_id: Uuid,
    ) -> AiResult<Vec<AiAgentModelAssignmentRecord>> {
        let items = ai_agent_model_assignments::Entity::find()
            .filter(ai_agent_model_assignments::Column::TenantId.eq(tenant_id))
            .order_by_asc(ai_agent_model_assignments::Column::AgentPrincipalId)
            .order_by_asc(ai_agent_model_assignments::Column::CreatedAt)
            .all(db)
            .await
            .map_err(db_err)?;
        items.into_iter().map(map_agent_model_assignment).collect()
    }

    pub async fn create_agent_model_assignment(
        db: &DatabaseConnection,
        operator: &AiOperatorContext,
        input: CreateAiAgentModelAssignmentInput,
    ) -> AiResult<AiAgentModelAssignmentRecord> {
        let principal = ai_agent_principals::Entity::find_by_id(input.agent_principal_id)
            .filter(ai_agent_principals::Column::TenantId.eq(operator.tenant_id))
            .one(db)
            .await
            .map_err(db_err)?
            .ok_or_else(|| AiError::NotFound("AI agent principal not found".to_string()))?;
        if !principal.is_active {
            return Err(AiError::Validation(
                "AI agent principal is inactive".to_string(),
            ));
        }
        let provider =
            require_provider_profile(db, operator.tenant_id, input.provider_profile_id).await?;
        if !provider.is_active {
            return Err(AiError::Validation(
                "AI provider profile is inactive".to_string(),
            ));
        }
        let catalog = crate::agent_catalog()?;
        let descriptor = catalog
            .descriptor(&principal.descriptor_slug)
            .filter(|descriptor| descriptor.owner == principal.descriptor_owner)
            .ok_or_else(|| {
                AiError::Validation("agent descriptor is no longer available".to_string())
            })?;
        ensure_agent_provider_capabilities(&provider, descriptor)?;
        let saved = ai_agent_model_assignments::ActiveModel {
            id: Set(Uuid::new_v4()),
            tenant_id: Set(operator.tenant_id),
            agent_principal_id: Set(input.agent_principal_id),
            provider_profile_id: Set(input.provider_profile_id),
            model_override: Set(input
                .model_override
                .filter(|model| !model.trim().is_empty())),
            execution_mode: Set(input.execution_mode.slug().to_string()),
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
        map_agent_model_assignment(saved)
    }

    pub async fn update_agent_model_assignment(
        db: &DatabaseConnection,
        operator: &AiOperatorContext,
        id: Uuid,
        input: UpdateAiAgentModelAssignmentInput,
    ) -> AiResult<AiAgentModelAssignmentRecord> {
        let existing = ai_agent_model_assignments::Entity::find_by_id(id)
            .filter(ai_agent_model_assignments::Column::TenantId.eq(operator.tenant_id))
            .one(db)
            .await
            .map_err(db_err)?
            .ok_or_else(|| AiError::NotFound("AI agent model assignment not found".to_string()))?;
        let mut active: ai_agent_model_assignments::ActiveModel = existing.into();
        active.model_override = Set(input
            .model_override
            .filter(|model| !model.trim().is_empty()));
        active.execution_mode = Set(input.execution_mode.slug().to_string());
        active.metadata = Set(normalize_metadata(input.metadata));
        active.is_active = Set(input.is_active);
        active.updated_by = Set(Some(operator.user_id));
        active.updated_at = Set(Utc::now().into());
        let saved = active.update(db).await.map_err(db_err)?;
        map_agent_model_assignment(saved)
    }

    pub async fn create_agent_workflow_run(
        db: &DatabaseConnection,
        operator: &AiOperatorContext,
        input: CreateAiAgentWorkflowRunInput,
    ) -> AiResult<Uuid> {
        let catalog = crate::agent_catalog()?;
        let workflow = catalog
            .workflows()
            .iter()
            .find(|workflow| {
                workflow.slug == input.workflow_slug && workflow.owner == input.workflow_owner
            })
            .ok_or_else(|| AiError::Validation("unknown owner-owned agent workflow".to_string()))?;
        let expected_stage_ids = workflow
            .stages
            .iter()
            .map(|stage| stage.id.clone())
            .collect::<std::collections::BTreeSet<_>>();
        for (binding_name, supplied_stage_ids) in [
            (
                "agent principal",
                input
                    .stage_principal_ids
                    .keys()
                    .cloned()
                    .collect::<std::collections::BTreeSet<_>>(),
            ),
            (
                "model assignment",
                input
                    .stage_model_assignment_ids
                    .keys()
                    .cloned()
                    .collect::<std::collections::BTreeSet<_>>(),
            ),
            (
                "task input",
                input
                    .stage_input_payloads
                    .keys()
                    .cloned()
                    .collect::<std::collections::BTreeSet<_>>(),
            ),
        ] {
            if supplied_stage_ids != expected_stage_ids {
                return Err(AiError::Validation(format!(
                    "workflow {binding_name} bindings must match the owner-declared stages exactly"
                )));
            }
        }
        let initiator_permissions = operator
            .permissions
            .iter()
            .map(ToString::to_string)
            .collect::<std::collections::BTreeSet<_>>();
        let transaction = db.begin().await.map_err(db_err)?;
        let workflow_run_id = Uuid::new_v4();
        let now = Utc::now();
        ai_agent_workflow_runs::ActiveModel {
            id: Set(workflow_run_id),
            tenant_id: Set(operator.tenant_id),
            workflow_owner: Set(input.workflow_owner),
            workflow_slug: Set(input.workflow_slug),
            initiator_id: Set(operator.user_id),
            status: Set("queued".to_string()),
            input_payload: Set(normalize_metadata(input.input_payload.clone())),
            output_payload: Set(None),
            metadata: Set(merge_metadata(
                input.metadata,
                json!({
                    "agent_execution_context": {
                        "initiator_permissions": operator
                            .permissions
                            .iter()
                            .map(ToString::to_string)
                            .collect::<Vec<_>>(),
                        "preferred_locale": operator.preferred_locale,
                    }
                }),
            )),
            created_at: Set(now.into()),
            started_at: Set(None),
            completed_at: Set(None),
            updated_at: Set(now.into()),
        }
        .insert(&transaction)
        .await
        .map_err(db_err)?;

        for stage in &workflow.stages {
            let principal_id = input
                .stage_principal_ids
                .get(&stage.id)
                .copied()
                .ok_or_else(|| {
                    AiError::Validation(format!(
                        "agent workflow stage `{}` has no principal",
                        stage.id
                    ))
                })?;
            let principal = ai_agent_principals::Entity::find_by_id(principal_id)
                .filter(ai_agent_principals::Column::TenantId.eq(operator.tenant_id))
                .filter(ai_agent_principals::Column::IsActive.eq(true))
                .one(&transaction)
                .await
                .map_err(db_err)?
                .ok_or_else(|| {
                    AiError::Validation(format!(
                        "agent principal for stage `{}` is unavailable",
                        stage.id
                    ))
                })?;
            if principal.descriptor_slug != stage.agent_slug {
                return Err(AiError::Validation(format!(
                    "agent principal for stage `{}` does not match owner descriptor `{}`",
                    stage.id, stage.agent_slug
                )));
            }
            let principal_contract = crate::AgentPrincipal {
                id: principal.id,
                tenant_id: principal.tenant_id,
                agent_slug: principal.descriptor_slug.clone(),
                role_slugs: string_list(&principal.role_slugs).into_iter().collect(),
                permission_slugs: string_list(&principal.permission_slugs)
                    .into_iter()
                    .collect(),
            };
            catalog.effective_permissions(&initiator_permissions, &principal_contract)?;
            let assignment_id = input
                .stage_model_assignment_ids
                .get(&stage.id)
                .copied()
                .ok_or_else(|| {
                    AiError::Validation(format!(
                        "agent workflow stage `{}` has no model assignment",
                        stage.id
                    ))
                })?;
            let assignment = ai_agent_model_assignments::Entity::find_by_id(assignment_id)
                .filter(ai_agent_model_assignments::Column::TenantId.eq(operator.tenant_id))
                .filter(ai_agent_model_assignments::Column::AgentPrincipalId.eq(principal.id))
                .filter(ai_agent_model_assignments::Column::IsActive.eq(true))
                .one(&transaction)
                .await
                .map_err(db_err)?
                .ok_or_else(|| {
                    AiError::Validation(format!(
                        "model assignment for stage `{}` is unavailable",
                        stage.id
                    ))
                })?;
            let descriptor = catalog
                .descriptor(&stage.agent_slug)
                .filter(|descriptor| descriptor.owner == principal.descriptor_owner)
                .ok_or_else(|| {
                    AiError::Validation(format!(
                        "agent principal for stage `{}` no longer matches its owner descriptor",
                        stage.id
                    ))
                })?;
            let provider = ai_provider_profiles::Entity::find_by_id(assignment.provider_profile_id)
                .filter(ai_provider_profiles::Column::TenantId.eq(operator.tenant_id))
                .one(&transaction)
                .await
                .map_err(db_err)?
                .ok_or_else(|| {
                    AiError::Validation(format!(
                        "provider profile for stage `{}` is unavailable",
                        stage.id
                    ))
                })?;
            if !provider.is_active {
                return Err(AiError::Validation(format!(
                    "provider profile for stage `{}` is inactive",
                    stage.id
                )));
            }
            ensure_agent_provider_capabilities(&provider, descriptor)?;
            let stage_input_payload = input
                .stage_input_payloads
                .get(&stage.id)
                .cloned()
                .ok_or_else(|| {
                    AiError::Validation(format!(
                        "agent workflow stage `{}` has no task input",
                        stage.id
                    ))
                })?;
            catalog.validate_stage_execution(&stage.agent_slug, &stage_input_payload)?;
            let status = if stage.depends_on.is_empty() {
                if stage.requires_approval {
                    "waiting_approval"
                } else {
                    "ready"
                }
            } else {
                "pending"
            };
            ai_agent_workflow_stages::ActiveModel {
                id: Set(Uuid::new_v4()),
                tenant_id: Set(operator.tenant_id),
                workflow_run_id: Set(workflow_run_id),
                stage_id: Set(stage.id.clone()),
                agent_principal_id: Set(principal.id),
                model_assignment_id: Set(Some(assignment.id)),
                run_id: Set(None),
                status: Set(status.to_string()),
                requires_approval: Set(stage.requires_approval),
                input_payload: Set(normalize_metadata(stage_input_payload)),
                output_payload: Set(None),
                error_message: Set(None),
                metadata: Set(json!({"depends_on": stage.depends_on})),
                lease_token: Set(None),
                lease_expires_at: Set(None),
                attempt_count: Set(0),
                created_at: Set(now.into()),
                started_at: Set(None),
                completed_at: Set(None),
                updated_at: Set(now.into()),
            }
            .insert(&transaction)
            .await
            .map_err(db_err)?;
        }
        transaction.commit().await.map_err(db_err)?;
        Ok(workflow_run_id)
    }
}

/// Durable result of one approved external tool execution.
///
/// It is written before the result is appended to the canonical chat history.
/// If the latter write fails, retrying approval finalization replays this value
/// and never invokes the external tool a second time.
#[derive(Debug, Clone, Serialize, Deserialize)]
struct ApprovalExecutionOutcome {
    content: String,
    raw_payload: serde_json::Value,
    duration_ms: i64,
}

fn approval_execution_outcome(
    metadata: &serde_json::Value,
) -> AiResult<Option<ApprovalExecutionOutcome>> {
    metadata
        .get("execution_outcome")
        .cloned()
        .map(serde_json::from_value)
        .transpose()
        .map_err(json_err)
}

async fn persist_approval_execution_outcome(
    db: &DatabaseConnection,
    approval: &ai_approval_requests::Model,
    outcome: &ApprovalExecutionOutcome,
) -> AiResult<ai_approval_requests::Model> {
    let mut metadata = approval.metadata.clone();
    if !metadata.is_object() {
        metadata = json!({});
    }
    metadata["execution_outcome"] = serde_json::to_value(outcome).map_err(json_err)?;
    let mut active: ai_approval_requests::ActiveModel = approval.clone().into();
    active.metadata = Set(metadata);
    active.status = Set("executed".to_string());
    active.updated_at = Set(Utc::now().into());
    active.update(db).await.map_err(db_err)
}

/// Claims one approval transition with compare-and-set semantics.
/// The caller supplies the observed state so an already-running resolver can
/// never obtain a second lease for the same tool call.
async fn claim_approval_resolution(
    db: &DatabaseConnection,
    tenant_id: Uuid,
    approval_id: Uuid,
    expected_status: &str,
) -> AiResult<bool> {
    let claimed = ai_approval_requests::Entity::update_many()
        .col_expr(
            ai_approval_requests::Column::Status,
            Expr::value("resolving".to_string()),
        )
        .filter(ai_approval_requests::Column::Id.eq(approval_id))
        .filter(ai_approval_requests::Column::TenantId.eq(tenant_id))
        .filter(ai_approval_requests::Column::Status.eq(expected_status))
        .exec(db)
        .await
        .map_err(db_err)?;
    Ok(claimed.rows_affected == 1)
}

async fn next_pending_approval_in_batch(
    db: &impl sea_orm::ConnectionTrait,
    tenant_id: Uuid,
    run_id: Uuid,
    approval_batch_id: &str,
) -> AiResult<Option<ai_approval_requests::Model>> {
    ai_approval_requests::Entity::find()
        .filter(ai_approval_requests::Column::TenantId.eq(tenant_id))
        .filter(ai_approval_requests::Column::RunId.eq(run_id))
        .filter(ai_approval_requests::Column::ApprovalBatchId.eq(approval_batch_id))
        .filter(ai_approval_requests::Column::Status.eq("pending"))
        .order_by_asc(ai_approval_requests::Column::CreatedAt)
        .one(db)
        .await
        .map_err(db_err)
}

enum ApprovalBatchRunTransition {
    WaitingForNext,
    ReadyToContinue,
}

async fn transition_run_after_approval_resolution(
    db: &impl sea_orm::ConnectionTrait,
    tenant_id: Uuid,
    run: ai_chat_runs::Model,
    approval_batch_id: &str,
) -> AiResult<(ai_chat_runs::Model, ApprovalBatchRunTransition)> {
    let next_pending =
        next_pending_approval_in_batch(db, tenant_id, run.id, approval_batch_id).await?;
    let mut active: ai_chat_runs::ActiveModel = run.into();
    active.updated_at = Set(Utc::now().into());
    let transition = if let Some(next_pending) = next_pending {
        active.status = Set("waiting_approval".to_string());
        active.pending_approval_id = Set(Some(next_pending.id));
        ApprovalBatchRunTransition::WaitingForNext
    } else {
        active.status = Set("running".to_string());
        active.pending_approval_id = Set(None);
        active.error_message = Set(None);
        ApprovalBatchRunTransition::ReadyToContinue
    };
    Ok((active.update(db).await.map_err(db_err)?, transition))
}

fn validate_approval_resolution_policy(
    approval_status: &str,
    approved: bool,
    tool_allowed: bool,
    tool_name: &str,
) -> AiResult<()> {
    if !matches!(approval_status, "pending" | "executed") {
        return Err(AiError::Validation(
            "approval request is not available for resolution".to_string(),
        ));
    }
    if approval_status == "executed" && !approved {
        return Err(AiError::Validation(
            "an executed approval must be finalized as approved".to_string(),
        ));
    }
    if approved && approval_status == "pending" && !tool_allowed {
        return Err(AiError::Validation(format!(
            "tool `{tool_name}` is no longer allowed by the execution policy"
        )));
    }
    Ok(())
}

#[cfg(test)]
mod approval_outcome_tests {
    use super::{ApprovalExecutionOutcome, approval_execution_outcome};
    use crate::entities::{ai_approval_requests, ai_chat_runs, ai_tool_traces};
    use crate::model::ToolTrace;
    use chrono::Utc;
    use sea_orm::{
        ActiveModelTrait, ActiveValue::Set, ConnectionTrait, DatabaseConnection, DbBackend,
        EntityTrait, Statement, TransactionTrait,
    };
    use uuid::Uuid;

    async fn approval_test_db() -> DatabaseConnection {
        let db = rustok_test_utils::setup_test_db().await;
        db.execute(Statement::from_string(
            DbBackend::Sqlite,
            "CREATE TABLE ai_approval_requests (\
                id TEXT PRIMARY KEY NOT NULL, tenant_id TEXT NOT NULL, session_id TEXT NOT NULL,\
                run_id TEXT NOT NULL, approval_batch_id TEXT NOT NULL, tool_name TEXT NOT NULL,\
                tool_call_id TEXT NOT NULL, tool_input TEXT NOT NULL, reason TEXT NULL,\
                status TEXT NOT NULL, resolved_by TEXT NULL, resolved_at TEXT NULL,\
                metadata TEXT NOT NULL, created_at TEXT NOT NULL, updated_at TEXT NOT NULL\
            )"
            .to_string(),
        ))
        .await
        .expect("approval test schema");
        db
    }

    async fn add_chat_run_test_schema(db: &DatabaseConnection) {
        db.execute(Statement::from_string(
            DbBackend::Sqlite,
            "CREATE TABLE ai_chat_runs (\
                id TEXT PRIMARY KEY NOT NULL, tenant_id TEXT NOT NULL, session_id TEXT NOT NULL,\
                provider_profile_id TEXT NOT NULL, task_profile_id TEXT NULL, tool_profile_id TEXT NULL,\
                status TEXT NOT NULL, model TEXT NOT NULL, execution_mode TEXT NOT NULL,\
                execution_path TEXT NOT NULL, requested_locale TEXT NULL, resolved_locale TEXT NOT NULL,\
                temperature REAL NULL, max_tokens INTEGER NULL, error_message TEXT NULL,\
                pending_approval_id TEXT NULL, decision_trace TEXT NOT NULL, metadata TEXT NOT NULL,\
                created_at TEXT NOT NULL, started_at TEXT NOT NULL, completed_at TEXT NULL,\
                updated_at TEXT NOT NULL\
            )"
            .to_string(),
        ))
        .await
        .expect("chat run test schema");
    }

    async fn insert_waiting_run(
        db: &DatabaseConnection,
        tenant_id: Uuid,
        session_id: Uuid,
        run_id: Uuid,
        pending_approval_id: Uuid,
    ) -> ai_chat_runs::Model {
        let now = Utc::now();
        ai_chat_runs::ActiveModel {
            id: Set(run_id),
            tenant_id: Set(tenant_id),
            session_id: Set(session_id),
            provider_profile_id: Set(Uuid::new_v4()),
            task_profile_id: Set(None),
            tool_profile_id: Set(None),
            status: Set("waiting_approval".to_string()),
            model: Set("test-model".to_string()),
            execution_mode: Set("mcp_tooling".to_string()),
            execution_path: Set("mcp_tooling".to_string()),
            requested_locale: Set(None),
            resolved_locale: Set("en".to_string()),
            temperature: Set(None),
            max_tokens: Set(None),
            error_message: Set(Some("awaiting approval".to_string())),
            pending_approval_id: Set(Some(pending_approval_id)),
            decision_trace: Set(serde_json::json!({})),
            metadata: Set(serde_json::json!({})),
            created_at: Set(now.into()),
            started_at: Set(Utc::now().into()),
            completed_at: Set(None),
            updated_at: Set(Utc::now().into()),
        }
        .insert(db)
        .await
        .expect("insert waiting run")
    }

    #[test]
    fn decodes_only_a_complete_durable_execution_outcome() {
        let outcome = ApprovalExecutionOutcome {
            content: "done".to_string(),
            raw_payload: serde_json::json!({ "record": "42" }),
            duration_ms: 12,
        };
        let metadata = serde_json::json!({ "execution_outcome": outcome });
        assert_eq!(
            approval_execution_outcome(&metadata)
                .unwrap()
                .expect("outcome")
                .content,
            "done"
        );
        assert!(
            approval_execution_outcome(&serde_json::json!({}))
                .unwrap()
                .is_none()
        );
        assert!(
            approval_execution_outcome(&serde_json::json!({
                "execution_outcome": { "content": "missing fields" }
            }))
            .is_err()
        );
    }

    #[test]
    fn rejects_stale_policy_without_reexecuting_a_durable_outcome() {
        let stale_policy =
            super::validate_approval_resolution_policy("pending", true, false, "catalog.write")
                .expect_err("pending approval must observe current policy");
        assert!(stale_policy.to_string().contains("no longer allowed"));
        super::validate_approval_resolution_policy("executed", true, false, "catalog.write")
            .expect("staged external outcome may be finalized after a policy change");
        assert!(
            super::validate_approval_resolution_policy("executed", false, false, "catalog.write",)
                .is_err()
        );
    }

    #[tokio::test]
    async fn stages_external_execution_before_history_finalization() {
        let db = approval_test_db().await;
        let now = Utc::now();
        let approval = ai_approval_requests::ActiveModel {
            id: Set(Uuid::new_v4()),
            tenant_id: Set(Uuid::new_v4()),
            session_id: Set(Uuid::new_v4()),
            run_id: Set(Uuid::new_v4()),
            approval_batch_id: Set("batch-1".to_string()),
            tool_name: Set("catalog.read".to_string()),
            tool_call_id: Set("call-1".to_string()),
            tool_input: Set(serde_json::json!({ "id": "42" })),
            reason: Set(None),
            status: Set("resolving".to_string()),
            resolved_by: Set(None),
            resolved_at: Set(None),
            metadata: Set(serde_json::json!({})),
            created_at: Set(now.into()),
            updated_at: Set(Utc::now().into()),
        }
        .insert(&db)
        .await
        .expect("insert pending approval");
        let staged = super::persist_approval_execution_outcome(
            &db,
            &approval,
            &ApprovalExecutionOutcome {
                content: "tool response".to_string(),
                raw_payload: serde_json::json!({ "record": "42" }),
                duration_ms: 21,
            },
        )
        .await
        .expect("stage external outcome");

        assert_eq!(staged.status, "executed");
        assert_eq!(
            approval_execution_outcome(&staged.metadata)
                .expect("decode staged outcome")
                .expect("outcome")
                .content,
            "tool response"
        );
        let reloaded = ai_approval_requests::Entity::find_by_id(approval.id)
            .one(&db)
            .await
            .expect("reload approval")
            .expect("approval persists");
        assert_eq!(reloaded.status, "executed");
        assert!(
            super::claim_approval_resolution(&db, approval.tenant_id, approval.id, "executed",)
                .await
                .expect("first resolver claims staged approval")
        );
        assert!(
            !super::claim_approval_resolution(&db, approval.tenant_id, approval.id, "executed",)
                .await
                .expect("second resolver sees compare-and-set miss")
        );
    }

    #[tokio::test]
    async fn selects_next_pending_approval_through_mixed_batch_resolutions() {
        let db = approval_test_db().await;
        add_chat_run_test_schema(&db).await;
        let tenant_id = Uuid::new_v4();
        let session_id = Uuid::new_v4();
        let run_id = Uuid::new_v4();
        let first_id = Uuid::new_v4();
        let second_id = Uuid::new_v4();
        let first_created_at = Utc::now();
        let second_created_at = first_created_at + chrono::Duration::seconds(1);
        let other_batch_created_at = Utc::now();
        for (id, batch, status, created_at) in [
            (first_id, "batch-a", "pending", first_created_at),
            (second_id, "batch-a", "pending", second_created_at),
            (Uuid::new_v4(), "batch-b", "pending", other_batch_created_at),
        ] {
            ai_approval_requests::ActiveModel {
                id: Set(id),
                tenant_id: Set(tenant_id),
                session_id: Set(session_id),
                run_id: Set(run_id),
                approval_batch_id: Set(batch.to_string()),
                tool_name: Set("catalog.read".to_string()),
                tool_call_id: Set(format!("call-{id}")),
                tool_input: Set(serde_json::json!({})),
                reason: Set(None),
                status: Set(status.to_string()),
                resolved_by: Set(None),
                resolved_at: Set(None),
                metadata: Set(serde_json::json!({})),
                created_at: Set(created_at.into()),
                updated_at: Set(Utc::now().into()),
            }
            .insert(&db)
            .await
            .expect("insert approval batch member");
        }
        let run = insert_waiting_run(&db, tenant_id, session_id, run_id, first_id).await;

        let next = super::next_pending_approval_in_batch(&db, tenant_id, run_id, "batch-a")
            .await
            .expect("find first pending")
            .expect("pending approval");
        assert_eq!(next.id, first_id);
        let mut resolved: ai_approval_requests::ActiveModel = next.into();
        resolved.status = Set("rejected".to_string());
        resolved.update(&db).await.expect("reject first approval");
        let (run, transition) =
            super::transition_run_after_approval_resolution(&db, tenant_id, run, "batch-a")
                .await
                .expect("advance batch to next approval");
        assert!(matches!(
            transition,
            super::ApprovalBatchRunTransition::WaitingForNext
        ));
        assert_eq!(run.pending_approval_id, Some(second_id));
        let second = super::next_pending_approval_in_batch(&db, tenant_id, run_id, "batch-a")
            .await
            .expect("find second pending")
            .expect("second approval");
        assert_eq!(second.id, second_id);
        let mut approved: ai_approval_requests::ActiveModel = second.into();
        approved.status = Set("approved".to_string());
        approved.update(&db).await.expect("approve second approval");
        let (run, transition) =
            super::transition_run_after_approval_resolution(&db, tenant_id, run, "batch-a")
                .await
                .expect("advance completed batch");
        assert!(matches!(
            transition,
            super::ApprovalBatchRunTransition::ReadyToContinue
        ));
        assert_eq!(run.status, "running");
        assert_eq!(run.pending_approval_id, None);
        assert_eq!(run.error_message, None);
    }

    #[tokio::test]
    async fn rolls_back_trace_when_later_finalization_write_fails() {
        let db = approval_test_db().await;
        db.execute(Statement::from_string(
            DbBackend::Sqlite,
            "CREATE TABLE ai_tool_traces (\
                id TEXT PRIMARY KEY NOT NULL, tenant_id TEXT NOT NULL, session_id TEXT NOT NULL,\
                run_id TEXT NOT NULL, tool_name TEXT NOT NULL, status TEXT NOT NULL,\
                input_payload TEXT NOT NULL, output_payload TEXT NULL, error_message TEXT NULL,\
                duration_ms INTEGER NULL, sensitive BOOLEAN NOT NULL, created_at TEXT NOT NULL,\
                updated_at TEXT NOT NULL\
            )"
            .to_string(),
        ))
        .await
        .expect("tool trace test schema");
        let transaction = db.begin().await.expect("begin finalization transaction");
        super::insert_tool_trace(
            &transaction,
            Uuid::new_v4(),
            Uuid::new_v4(),
            Uuid::new_v4(),
            &ToolTrace {
                tool_name: "catalog.write".to_string(),
                input_payload: serde_json::json!({ "id": "42" }),
                output_payload: Some(serde_json::json!({ "ok": true })),
                status: "completed".to_string(),
                duration_ms: 5,
                sensitive: true,
                error_message: None,
                created_at: Utc::now(),
            },
        )
        .await
        .expect("insert trace before later finalization step");
        assert!(
            transaction
                .execute(Statement::from_string(
                    DbBackend::Sqlite,
                    "INSERT INTO ai_chat_messages (id) VALUES ('missing-table')".to_string(),
                ))
                .await
                .is_err()
        );
        drop(transaction);
        assert!(
            ai_tool_traces::Entity::find()
                .all(&db)
                .await
                .expect("read rolled-back traces")
                .is_empty()
        );
    }
}

#[cfg(test)]
mod agent_workflow_status_tests {
    use super::aggregate_agent_workflow_status;

    #[test]
    fn cancellation_is_preserved_unless_a_stage_failed() {
        assert_eq!(
            aggregate_agent_workflow_status(["completed", "cancelled"].into_iter()),
            "cancelled"
        );
        assert_eq!(
            aggregate_agent_workflow_status(["failed", "cancelled"].into_iter()),
            "failed"
        );
    }
}

#[cfg(test)]
mod agent_principal_rbac_tests {
    use std::collections::BTreeSet;

    use rustok_api::{TenantRbacCatalog, TenantRbacPermission, TenantRbacRole};
    use uuid::Uuid;

    use super::resolve_agent_principal_rbac;
    use crate::{AgentDescriptor, AgentKind};

    struct TestCatalog;

    impl TenantRbacCatalog for TestCatalog {
        fn roles(&self, _tenant_id: Uuid) -> Vec<TenantRbacRole> {
            vec![
                TenantRbacRole {
                    slug: "catalog-editor".to_string(),
                    display_name: "Catalog editor".to_string(),
                    permission_slugs: vec!["product.read".to_string(), "product.write".to_string()],
                },
                TenantRbacRole {
                    slug: "catalog-reader".to_string(),
                    display_name: "Catalog reader".to_string(),
                    permission_slugs: vec!["product.read".to_string()],
                },
            ]
        }

        fn permissions(&self, _tenant_id: Uuid) -> Vec<TenantRbacPermission> {
            Vec::new()
        }
    }

    fn descriptor() -> AgentDescriptor {
        AgentDescriptor {
            slug: "catalog-enricher".to_string(),
            display_name: "Catalog enricher".to_string(),
            owner: "product".to_string(),
            kind: AgentKind::Product,
            responsibility: "Enrich catalog data".to_string(),
            required_permissions: BTreeSet::from(["product.write".to_string()]),
            allowed_operations: BTreeSet::new(),
            required_capabilities: Vec::new(),
            can_orchestrate: false,
        }
    }

    #[test]
    fn derives_permissions_only_from_catalogued_roles_and_enforces_descriptor_floor() {
        let tenant_id = Uuid::new_v4();
        let catalog = TestCatalog;
        assert_eq!(
            resolve_agent_principal_rbac(
                &catalog,
                tenant_id,
                vec!["catalog-editor".to_string(), "catalog-editor".to_string()],
                &descriptor(),
            )
            .expect("editor role grants descriptor floor"),
            (
                vec!["catalog-editor".to_string()],
                vec!["product.read".to_string(), "product.write".to_string()],
            )
        );
        assert!(
            resolve_agent_principal_rbac(
                &catalog,
                tenant_id,
                vec!["catalog-reader".to_string()],
                &descriptor(),
            )
            .is_err()
        );
        assert!(
            resolve_agent_principal_rbac(
                &catalog,
                tenant_id,
                vec!["unknown".to_string()],
                &descriptor(),
            )
            .is_err()
        );
    }
}

#[cfg(test)]
mod product_agent_workflow_persistence_tests {
    use std::collections::BTreeMap;
    use std::sync::Arc;

    use async_trait::async_trait;
    use chrono::{Duration, Utc};
    use sea_orm::{
        ActiveModelTrait, ColumnTrait, ConnectionTrait, Database, DbBackend, EntityTrait,
        QueryFilter, Set, Statement,
    };
    use serde_json::json;
    use uuid::Uuid;

    use super::{
        AiManagementService, AiOperatorContext, CreateAiAgentModelAssignmentInput,
        CreateAiAgentPrincipalInput, CreateAiAgentWorkflowRunInput, CreateAiTaskProfileInput,
        ExecutionMode, ResolveAiAgentWorkflowStageApprovalInput, ai_agent_workflow_stages,
    };
    use crate::{
        AiHostRuntime, AiProviderConfig, AiProviderTarget, AiProviderTargetCatalog, ChatMessage,
        ChatMessageRole, ProviderCapability, ProviderChatRequest, ProviderChatResponse,
        ProviderEgressPolicy, ProviderStructuredRequest, ProviderTargetAuth, ProviderTestResult,
        engine::InferenceEngine, entities::ai_provider_profiles,
    };
    use rustok_api::{Permission, TenantRbacCatalog, TenantRbacPermission, TenantRbacRole};
    use rustok_core::registry::ModuleRegistry;
    use rustok_outbox::{OutboxTransport, TransactionalEventBus};
    use rustok_secrets::SecretResolverRegistry;

    struct ProductAgentRoleCatalog;

    struct WorkflowAttributesEngine;

    #[async_trait]
    impl InferenceEngine for WorkflowAttributesEngine {
        async fn test_connection(
            &self,
            _config: &AiProviderConfig,
        ) -> crate::AiResult<ProviderTestResult> {
            unreachable!("workflow test uses deterministic structured generation")
        }

        async fn complete(
            &self,
            _config: &AiProviderConfig,
            _request: ProviderChatRequest,
        ) -> crate::AiResult<ProviderChatResponse> {
            unreachable!("workflow attributes task uses structured generation")
        }

        async fn complete_stream(
            &self,
            _config: &AiProviderConfig,
            _request: ProviderChatRequest,
            _emitter: Option<crate::ProviderStreamEmitter>,
        ) -> crate::AiResult<ProviderChatResponse> {
            Ok(ProviderChatResponse {
                assistant_message: ChatMessage {
                    role: ChatMessageRole::Assistant,
                    content: Some("Product attributes are ready for review.".to_string()),
                    name: None,
                    tool_call_id: None,
                    tool_calls: Vec::new(),
                    metadata: json!({}),
                },
                finish_reason: Some("stop".to_string()),
                raw_payload: json!({}),
            })
        }

        async fn complete_structured(
            &self,
            _request: ProviderStructuredRequest,
        ) -> crate::AiResult<serde_json::Value> {
            Ok(json!({
                "brand": "Example brand",
                "material": "Cotton",
                "color": "Blue",
                "size": null,
                "dimensions": null,
                "compatibility": null,
                "care_instructions": "Machine wash cold",
                "hazmat": null,
                "flex_attributes": [{"key": "fabric_weight", "value": "180 gsm"}]
            }))
        }
    }

    impl TenantRbacCatalog for ProductAgentRoleCatalog {
        fn roles(&self, _tenant_id: Uuid) -> Vec<TenantRbacRole> {
            vec![TenantRbacRole {
                slug: "product-ai-operator".to_string(),
                display_name: "Product AI operator".to_string(),
                permission_slugs: vec![
                    Permission::AI_TASKS_TEXT_RUN.to_string(),
                    Permission::PRODUCTS_UPDATE.to_string(),
                ],
            }]
        }

        fn permissions(&self, _tenant_id: Uuid) -> Vec<TenantRbacPermission> {
            Vec::new()
        }
    }

    async fn database() -> sea_orm::DatabaseConnection {
        let database = Database::connect("sqlite::memory:")
            .await
            .expect("product agent workflow database");
        for statement in [
            "CREATE TABLE tenants (id UUID PRIMARY KEY, default_locale TEXT NULL, settings JSON NOT NULL)",
            "CREATE TABLE ai_provider_profiles (\
                id UUID PRIMARY KEY, tenant_id UUID NOT NULL, slug TEXT NOT NULL, \
                display_name TEXT NOT NULL, provider_slug TEXT NOT NULL, \
                provider_target_id TEXT NOT NULL, model TEXT NOT NULL, credential_refs JSON NOT NULL, \
                temperature REAL NULL, max_tokens INTEGER NULL, is_active BOOLEAN NOT NULL, \
                capabilities JSON NOT NULL, allowed_task_profiles JSON NOT NULL, \
                denied_task_profiles JSON NOT NULL, restricted_role_slugs JSON NOT NULL, \
                metadata JSON NOT NULL, created_by UUID NULL, updated_by UUID NULL, \
                created_at TIMESTAMPTZ NOT NULL, updated_at TIMESTAMPTZ NOT NULL)",
            "CREATE TABLE ai_agent_principals (\
                id UUID PRIMARY KEY, tenant_id UUID NOT NULL, slug TEXT NOT NULL, \
                descriptor_owner TEXT NOT NULL, descriptor_slug TEXT NOT NULL, \
                role_slugs JSON NOT NULL, permission_slugs JSON NOT NULL, is_active BOOLEAN NOT NULL, \
                metadata JSON NOT NULL, created_by UUID NULL, updated_by UUID NULL, \
                created_at TIMESTAMPTZ NOT NULL DEFAULT CURRENT_TIMESTAMP, \
                updated_at TIMESTAMPTZ NOT NULL DEFAULT CURRENT_TIMESTAMP)",
            "CREATE TABLE ai_agent_model_assignments (\
                id UUID PRIMARY KEY, tenant_id UUID NOT NULL, agent_principal_id UUID NOT NULL, \
                provider_profile_id UUID NOT NULL, model_override TEXT NULL, execution_mode TEXT NOT NULL, \
                is_active BOOLEAN NOT NULL, metadata JSON NOT NULL, created_by UUID NULL, \
                updated_by UUID NULL, created_at TIMESTAMPTZ NOT NULL DEFAULT CURRENT_TIMESTAMP, \
                updated_at TIMESTAMPTZ NOT NULL DEFAULT CURRENT_TIMESTAMP)",
            "CREATE TABLE ai_agent_workflow_runs (\
                id UUID PRIMARY KEY, tenant_id UUID NOT NULL, workflow_owner TEXT NOT NULL, \
                workflow_slug TEXT NOT NULL, initiator_id UUID NOT NULL, status TEXT NOT NULL, \
                input_payload JSON NOT NULL, output_payload JSON NULL, metadata JSON NOT NULL, \
                created_at TIMESTAMPTZ NOT NULL, started_at TIMESTAMPTZ NULL, \
                completed_at TIMESTAMPTZ NULL, updated_at TIMESTAMPTZ NOT NULL)",
            "CREATE TABLE ai_agent_workflow_stages (\
                id UUID PRIMARY KEY, tenant_id UUID NOT NULL, workflow_run_id UUID NOT NULL, \
                stage_id TEXT NOT NULL, agent_principal_id UUID NOT NULL, \
                model_assignment_id UUID NULL, run_id UUID NULL, status TEXT NOT NULL, \
                requires_approval BOOLEAN NOT NULL, input_payload JSON NOT NULL, \
                output_payload JSON NULL, error_message TEXT NULL, metadata JSON NOT NULL, \
                lease_token UUID NULL, lease_expires_at TIMESTAMPTZ NULL, attempt_count INTEGER NOT NULL, \
                created_at TIMESTAMPTZ NOT NULL, started_at TIMESTAMPTZ NULL, \
                completed_at TIMESTAMPTZ NULL, updated_at TIMESTAMPTZ NOT NULL)",
            "CREATE TABLE ai_task_profiles (\
                id UUID PRIMARY KEY, tenant_id UUID NOT NULL, slug TEXT NOT NULL, display_name TEXT NOT NULL, \
                description TEXT NULL, target_capability TEXT NOT NULL, system_prompt TEXT NULL, \
                allowed_provider_profile_ids JSON NOT NULL, preferred_provider_profile_ids JSON NOT NULL, \
                fallback_strategy TEXT NOT NULL, tool_profile_id UUID NULL, approval_policy JSON NOT NULL, \
                default_execution_mode TEXT NOT NULL, is_active BOOLEAN NOT NULL, metadata JSON NOT NULL, \
                created_by UUID NULL, updated_by UUID NULL, \
                created_at TIMESTAMPTZ NOT NULL DEFAULT CURRENT_TIMESTAMP, \
                updated_at TIMESTAMPTZ NOT NULL DEFAULT CURRENT_TIMESTAMP)",
            "CREATE TABLE ai_chat_sessions (\
                id UUID PRIMARY KEY, tenant_id UUID NOT NULL, title TEXT NOT NULL, \
                provider_profile_id UUID NOT NULL, task_profile_id UUID NULL, tool_profile_id UUID NULL, \
                execution_mode TEXT NOT NULL, requested_locale TEXT NULL, resolved_locale TEXT NOT NULL, \
                status TEXT NOT NULL, created_by UUID NULL, metadata JSON NOT NULL, \
                created_at TIMESTAMPTZ NOT NULL DEFAULT CURRENT_TIMESTAMP, \
                updated_at TIMESTAMPTZ NOT NULL DEFAULT CURRENT_TIMESTAMP)",
            "CREATE TABLE ai_chat_messages (\
                id UUID PRIMARY KEY, tenant_id UUID NOT NULL, session_id UUID NOT NULL, run_id UUID NULL, \
                role TEXT NOT NULL, content TEXT NULL, name TEXT NULL, tool_call_id TEXT NULL, \
                tool_calls JSON NOT NULL, metadata JSON NOT NULL, created_by UUID NULL, \
                created_at TIMESTAMPTZ NOT NULL DEFAULT CURRENT_TIMESTAMP)",
            "CREATE TABLE ai_chat_runs (\
                id UUID PRIMARY KEY, tenant_id UUID NOT NULL, session_id UUID NOT NULL, \
                provider_profile_id UUID NOT NULL, task_profile_id UUID NULL, tool_profile_id UUID NULL, \
                status TEXT NOT NULL, model TEXT NOT NULL, execution_mode TEXT NOT NULL, \
                execution_path TEXT NOT NULL, requested_locale TEXT NULL, resolved_locale TEXT NOT NULL, \
                temperature REAL NULL, max_tokens INTEGER NULL, error_message TEXT NULL, \
                pending_approval_id UUID NULL, decision_trace JSON NOT NULL, metadata JSON NOT NULL, \
                created_at TIMESTAMPTZ NOT NULL DEFAULT CURRENT_TIMESTAMP, started_at TIMESTAMPTZ NOT NULL, \
                completed_at TIMESTAMPTZ NULL, updated_at TIMESTAMPTZ NOT NULL DEFAULT CURRENT_TIMESTAMP)",
            "CREATE TABLE ai_tool_traces (\
                id UUID PRIMARY KEY, tenant_id UUID NOT NULL, session_id UUID NOT NULL, run_id UUID NOT NULL, \
                tool_name TEXT NOT NULL, status TEXT NOT NULL, input_payload JSON NOT NULL, \
                output_payload JSON NULL, error_message TEXT NULL, duration_ms INTEGER NULL, \
                sensitive BOOLEAN NOT NULL, created_at TIMESTAMPTZ NOT NULL, updated_at TIMESTAMPTZ NOT NULL)",
            "CREATE TABLE ai_approval_requests (\
                id UUID PRIMARY KEY, tenant_id UUID NOT NULL, session_id UUID NOT NULL, run_id UUID NOT NULL, \
                approval_batch_id TEXT NOT NULL, tool_name TEXT NOT NULL, tool_call_id TEXT NOT NULL, \
                tool_input JSON NOT NULL, reason TEXT NULL, status TEXT NOT NULL, resolved_by UUID NULL, \
                resolved_at TIMESTAMPTZ NULL, metadata JSON NOT NULL, \
                created_at TIMESTAMPTZ NOT NULL DEFAULT CURRENT_TIMESTAMP, \
                updated_at TIMESTAMPTZ NOT NULL DEFAULT CURRENT_TIMESTAMP)",
        ] {
            database
                .execute(Statement::from_string(
                    DbBackend::Sqlite,
                    statement.to_string(),
                ))
                .await
                .expect("product agent workflow schema");
        }
        database
    }

    fn operator() -> AiOperatorContext {
        AiOperatorContext {
            tenant_id: Uuid::new_v4(),
            user_id: Uuid::new_v4(),
            permissions: [
                Permission::AI_APPROVALS_RESOLVE,
                Permission::AI_TASKS_TEXT_RUN,
                Permission::PRODUCTS_UPDATE,
            ]
            .into_iter()
            .collect(),
            role_slugs: vec!["product-ai-operator".to_string()],
            preferred_locale: Some("en".to_string()),
        }
    }

    async fn stage(
        database: &sea_orm::DatabaseConnection,
        tenant_id: Uuid,
        workflow_run_id: Uuid,
        stage_id: &str,
    ) -> ai_agent_workflow_stages::Model {
        ai_agent_workflow_stages::Entity::find()
            .filter(ai_agent_workflow_stages::Column::TenantId.eq(tenant_id))
            .filter(ai_agent_workflow_stages::Column::WorkflowRunId.eq(workflow_run_id))
            .filter(ai_agent_workflow_stages::Column::StageId.eq(stage_id))
            .one(database)
            .await
            .expect("product agent workflow stage query")
            .expect("product agent workflow stage")
    }

    #[tokio::test]
    async fn product_enrichment_workflow_persists_owner_bindings_and_approval_gates() {
        let database = database().await;
        let operator = operator();
        database
            .execute(Statement::from_sql_and_values(
                DbBackend::Sqlite,
                "INSERT INTO tenants (id, default_locale, settings) VALUES (?, ?, ?)".to_string(),
                vec![
                    operator.tenant_id.into(),
                    "en".into(),
                    r#"{"enabled_locales":["en"]}"#.into(),
                ],
            ))
            .await
            .expect("product agent workflow tenant");
        let provider_id = Uuid::new_v4();
        let now = Utc::now();
        ai_provider_profiles::ActiveModel {
            id: Set(provider_id),
            tenant_id: Set(operator.tenant_id),
            slug: Set("product-agent-provider".to_string()),
            display_name: Set("Product agent provider".to_string()),
            provider_slug: Set("openai_compatible".to_string()),
            provider_target_id: Set("openai_compatible".to_string()),
            model: Set("test-model".to_string()),
            credential_refs: Set(json!({})),
            temperature: Set(None),
            max_tokens: Set(None),
            is_active: Set(true),
            capabilities: Set(json!([
                ProviderCapability::TextGeneration.slug(),
                ProviderCapability::StructuredGeneration.slug(),
            ])),
            allowed_task_profiles: Set(json!([])),
            denied_task_profiles: Set(json!([])),
            restricted_role_slugs: Set(json!([])),
            metadata: Set(json!({})),
            created_by: Set(Some(operator.user_id)),
            updated_by: Set(Some(operator.user_id)),
            created_at: Set(now.into()),
            updated_at: Set(now.into()),
        }
        .insert(&database)
        .await
        .expect("product agent provider profile");

        let role_catalog = ProductAgentRoleCatalog;
        let copy_principal = AiManagementService::create_agent_principal(
            &database,
            &operator,
            &role_catalog,
            CreateAiAgentPrincipalInput {
                slug: "product-copywriter-agent".to_string(),
                descriptor_owner: "rustok-ai-product".to_string(),
                descriptor_slug: "product_copywriter".to_string(),
                role_slugs: vec!["product-ai-operator".to_string()],
                metadata: json!({}),
            },
        )
        .await
        .expect("product copywriter principal");
        let attributes_principal = AiManagementService::create_agent_principal(
            &database,
            &operator,
            &role_catalog,
            CreateAiAgentPrincipalInput {
                slug: "product-attribute-agent".to_string(),
                descriptor_owner: "rustok-ai-product".to_string(),
                descriptor_slug: "product_attribute_enricher".to_string(),
                role_slugs: vec!["product-ai-operator".to_string()],
                metadata: json!({}),
            },
        )
        .await
        .expect("product attribute principal");
        let copy_assignment = AiManagementService::create_agent_model_assignment(
            &database,
            &operator,
            CreateAiAgentModelAssignmentInput {
                agent_principal_id: copy_principal.id,
                provider_profile_id: provider_id,
                model_override: None,
                execution_mode: ExecutionMode::Direct,
                metadata: json!({}),
            },
        )
        .await
        .expect("product copywriter assignment");
        let attributes_assignment = AiManagementService::create_agent_model_assignment(
            &database,
            &operator,
            CreateAiAgentModelAssignmentInput {
                agent_principal_id: attributes_principal.id,
                provider_profile_id: provider_id,
                model_override: None,
                execution_mode: ExecutionMode::Direct,
                metadata: json!({}),
            },
        )
        .await
        .expect("product attribute assignment");
        let attributes_task = AiManagementService::create_task_profile(
            &database,
            &operator,
            CreateAiTaskProfileInput {
                slug: rustok_ai_product::PRODUCT_ATTRIBUTES_TASK_SLUG.to_string(),
                display_name: "Product attributes".to_string(),
                description: None,
                target_capability: ProviderCapability::StructuredGeneration,
                system_prompt: None,
                allowed_provider_profile_ids: vec![provider_id],
                preferred_provider_profile_ids: vec![provider_id],
                fallback_strategy: "ordered".to_string(),
                tool_profile_id: None,
                approval_policy: json!({}),
                default_execution_mode: ExecutionMode::Direct,
                metadata: json!({}),
            },
        )
        .await
        .expect("product attributes task profile");

        let product_id = Uuid::new_v4();
        let stage_inputs = BTreeMap::from([
            ("copy".to_string(), json!({"product_id": product_id})),
            ("attributes".to_string(), json!({"product_id": product_id})),
        ]);
        let workflow_run_id = AiManagementService::create_agent_workflow_run(
            &database,
            &operator,
            CreateAiAgentWorkflowRunInput {
                workflow_owner: "rustok-ai-product".to_string(),
                workflow_slug: "product_enrichment".to_string(),
                stage_principal_ids: BTreeMap::from([
                    ("copy".to_string(), copy_principal.id),
                    ("attributes".to_string(), attributes_principal.id),
                ]),
                stage_model_assignment_ids: BTreeMap::from([
                    ("copy".to_string(), copy_assignment.id),
                    ("attributes".to_string(), attributes_assignment.id),
                ]),
                stage_input_payloads: stage_inputs,
                input_payload: json!({"product_id": product_id}),
                metadata: json!({}),
            },
        )
        .await
        .expect("product enrichment workflow");

        let copy_stage = stage(&database, operator.tenant_id, workflow_run_id, "copy").await;
        let attributes_stage =
            stage(&database, operator.tenant_id, workflow_run_id, "attributes").await;
        assert_eq!(copy_stage.status, "waiting_approval");
        assert_eq!(attributes_stage.status, "pending");

        assert!(
            AiManagementService::resolve_agent_workflow_stage_approval(
                &database,
                &operator,
                copy_stage.id,
                ResolveAiAgentWorkflowStageApprovalInput {
                    approved: true,
                    reason: Some("copy reviewed".to_string()),
                },
            )
            .await
            .expect("copy stage approval")
        );
        let copy_lease = Uuid::new_v4();
        assert!(
            AiManagementService::claim_agent_workflow_stage(
                &database,
                operator.tenant_id,
                copy_stage.id,
                copy_lease,
                Utc::now() + Duration::minutes(1),
            )
            .await
            .expect("copy stage claim")
        );
        assert!(
            AiManagementService::complete_agent_workflow_stage(
                &database,
                operator.tenant_id,
                copy_stage.id,
                copy_lease,
                json!({"ai_run_id": Uuid::new_v4()}),
            )
            .await
            .expect("copy stage completion")
        );

        let attributes_stage =
            stage(&database, operator.tenant_id, workflow_run_id, "attributes").await;
        assert_eq!(attributes_stage.status, "waiting_approval");
        assert!(
            AiManagementService::resolve_agent_workflow_stage_approval(
                &database,
                &operator,
                attributes_stage.id,
                ResolveAiAgentWorkflowStageApprovalInput {
                    approved: true,
                    reason: Some("attributes reviewed".to_string()),
                },
            )
            .await
            .expect("attributes stage approval")
        );
        let attributes_lease = Uuid::new_v4();
        assert!(
            AiManagementService::claim_agent_workflow_stage(
                &database,
                operator.tenant_id,
                attributes_stage.id,
                attributes_lease,
                Utc::now() + Duration::minutes(1),
            )
            .await
            .expect("attributes stage claim")
        );
        let egress_policy = ProviderEgressPolicy {
            allowed_origins: vec!["provider.example.test".to_string()],
            allow_local_origins: false,
        };
        let provider_targets = AiProviderTargetCatalog::new_with_egress_policy(
            vec![AiProviderTarget {
                id: crate::ProviderTargetId::new("openai_compatible").expect("provider target id"),
                provider_slug: crate::ProviderSlug::openai_compatible(),
                display_name: "Workflow test provider".to_string(),
                auth: ProviderTargetAuth::None,
                settings: BTreeMap::from([(
                    "base_url".to_string(),
                    json!("https://provider.example.test/v1"),
                )]),
            }],
            &egress_policy,
        )
        .expect("workflow test provider targets");
        let runtime = AiHostRuntime::new(
            database.clone(),
            TransactionalEventBus::new(Arc::new(OutboxTransport::new(database.clone()))),
            ModuleRegistry::new(),
            SecretResolverRegistry::builder().build(),
            egress_policy,
            provider_targets,
        )
        .with_test_inference_engine(Arc::new(WorkflowAttributesEngine));
        let run = AiManagementService::execute_agent_workflow_stage(
            &runtime,
            &operator,
            attributes_stage.id,
            attributes_lease,
        )
        .await
        .expect("canonical product attributes stage execution");
        assert_eq!(run.status, "completed");
        assert_eq!(run.task_profile_id, Some(attributes_task.id));

        let completed_attributes =
            stage(&database, operator.tenant_id, workflow_run_id, "attributes").await;
        assert_eq!(completed_attributes.status, "completed");
        assert_eq!(completed_attributes.run_id, Some(run.id));
        assert_eq!(
            run.metadata
                .get("product_context")
                .and_then(|value| value.get("source"))
                .and_then(serde_json::Value::as_str),
            Some("degraded")
        );

        let workflow_run = super::ai_agent_workflow_runs::Entity::find_by_id(workflow_run_id)
            .one(&database)
            .await
            .expect("product enrichment workflow query")
            .expect("product enrichment workflow run");
        assert_eq!(workflow_run.status, "completed");
    }
}

impl AiManagementService {
    pub fn metrics_snapshot() -> AiRuntimeMetricsSnapshot {
        ai_metrics::metrics_snapshot()
    }

    pub fn recent_stream_events(session_id: Option<Uuid>, limit: usize) -> Vec<AiRunStreamEvent> {
        ai_run_stream_hub().recent_events(session_id, limit)
    }

    pub async fn list_recent_runs(
        db: &DatabaseConnection,
        tenant_id: Uuid,
        limit: usize,
    ) -> AiResult<Vec<AiRecentRunRecord>> {
        let limit = limit.max(1) as u64;
        let runs = ai_chat_runs::Entity::find()
            .filter(ai_chat_runs::Column::TenantId.eq(tenant_id))
            .order_by_desc(ai_chat_runs::Column::CreatedAt)
            .limit(limit)
            .all(db)
            .await
            .map_err(db_err)?;

        if runs.is_empty() {
            return Ok(Vec::new());
        }

        let session_ids: Vec<Uuid> = runs.iter().map(|run| run.session_id).collect();
        let provider_ids: Vec<Uuid> = runs.iter().map(|run| run.provider_profile_id).collect();
        let task_ids: Vec<Uuid> = runs.iter().filter_map(|run| run.task_profile_id).collect();

        let session_map: HashMap<Uuid, ai_chat_sessions::Model> = ai_chat_sessions::Entity::find()
            .filter(ai_chat_sessions::Column::TenantId.eq(tenant_id))
            .filter(ai_chat_sessions::Column::Id.is_in(session_ids))
            .all(db)
            .await
            .map_err(db_err)?
            .into_iter()
            .map(|session| (session.id, session))
            .collect();

        let provider_map: HashMap<Uuid, ai_provider_profiles::Model> =
            ai_provider_profiles::Entity::find()
                .filter(ai_provider_profiles::Column::TenantId.eq(tenant_id))
                .filter(ai_provider_profiles::Column::Id.is_in(provider_ids))
                .all(db)
                .await
                .map_err(db_err)?
                .into_iter()
                .map(|provider| (provider.id, provider))
                .collect();

        let task_map: HashMap<Uuid, ai_task_profiles::Model> = if task_ids.is_empty() {
            HashMap::new()
        } else {
            ai_task_profiles::Entity::find()
                .filter(ai_task_profiles::Column::TenantId.eq(tenant_id))
                .filter(ai_task_profiles::Column::Id.is_in(task_ids))
                .all(db)
                .await
                .map_err(db_err)?
                .into_iter()
                .map(|task| (task.id, task))
                .collect()
        };

        runs.into_iter()
            .map(|run| map_recent_run_record(run, &session_map, &provider_map, &task_map))
            .collect()
    }

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
        profiles.into_iter().map(map_provider_profile).collect()
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
        profile.map(map_provider_profile).transpose()
    }

    pub async fn create_provider_profile(
        db: &DatabaseConnection,
        operator: &AiOperatorContext,
        provider_targets: &crate::AiProviderTargetCatalog,
        egress_policy: &crate::ProviderEgressPolicy,
        secrets: &rustok_secrets::SecretResolverRegistry,
        input: CreateAiProviderProfileInput,
    ) -> AiResult<AiProviderProfileRecord> {
        validate_slug(&input.slug)?;
        let provider_slug = validate_provider_target_profile_contract(
            provider_targets,
            &input.provider_target_id,
            &input.credential_refs,
            egress_policy,
        )?;
        for reference in input.credential_refs.values() {
            secrets
                .validate_reference_for_tenant(operator.tenant_id, reference)
                .map_err(|error| AiError::Validation(error.to_string()))?;
        }
        let profile = ai_provider_profiles::ActiveModel {
            id: Set(Uuid::new_v4()),
            tenant_id: Set(operator.tenant_id),
            slug: Set(input.slug),
            display_name: Set(input.display_name),
            provider_slug: Set(provider_slug.to_string()),
            provider_target_id: Set(input.provider_target_id.to_string()),
            model: Set(input.model),
            credential_refs: Set(serde_json::to_value(input.credential_refs).map_err(json_err)?),
            temperature: Set(input.temperature),
            max_tokens: Set(input.max_tokens),
            is_active: Set(true),
            capabilities: Set(capability_json_array(input.capabilities)),
            allowed_task_profiles: Set(to_json_array(input.usage_policy.allowed_task_profiles)?),
            denied_task_profiles: Set(to_json_array(input.usage_policy.denied_task_profiles)?),
            // Role restrictions are platform-owned. Do not accept a package-local
            // role vocabulary through the AI service input.
            restricted_role_slugs: Set(serde_json::json!([])),
            metadata: Set(normalize_metadata(input.metadata)),
            created_by: Set(Some(operator.user_id)),
            updated_by: Set(Some(operator.user_id)),
            created_at: sea_orm::ActiveValue::NotSet,
            updated_at: sea_orm::ActiveValue::NotSet,
        }
        .insert(db)
        .await
        .map_err(db_err)?;
        secrets.invalidate(None).await;
        map_provider_profile(profile)
    }

    pub async fn update_provider_profile(
        db: &DatabaseConnection,
        operator: &AiOperatorContext,
        provider_targets: &crate::AiProviderTargetCatalog,
        egress_policy: &crate::ProviderEgressPolicy,
        secrets: &rustok_secrets::SecretResolverRegistry,
        id: Uuid,
        input: UpdateAiProviderProfileInput,
    ) -> AiResult<AiProviderProfileRecord> {
        let existing = require_provider_profile(db, operator.tenant_id, id).await?;
        let provider_slug = validate_provider_target_profile_contract(
            provider_targets,
            &input.provider_target_id,
            &input.credential_refs,
            egress_policy,
        )?;
        for reference in input.credential_refs.values() {
            secrets
                .validate_reference_for_tenant(operator.tenant_id, reference)
                .map_err(|error| AiError::Validation(error.to_string()))?;
        }
        let mut active: ai_provider_profiles::ActiveModel = existing.into();
        active.display_name = Set(input.display_name);
        active.provider_slug = Set(provider_slug.to_string());
        active.provider_target_id = Set(input.provider_target_id.to_string());
        active.model = Set(input.model);
        active.credential_refs =
            Set(serde_json::to_value(input.credential_refs).map_err(json_err)?);
        active.temperature = Set(input.temperature);
        active.max_tokens = Set(input.max_tokens);
        active.is_active = Set(input.is_active);
        active.capabilities = Set(capability_json_array(input.capabilities));
        active.allowed_task_profiles =
            Set(to_json_array(input.usage_policy.allowed_task_profiles)?);
        active.denied_task_profiles = Set(to_json_array(input.usage_policy.denied_task_profiles)?);
        // Updating a profile cannot introduce package-local role restrictions.
        active.restricted_role_slugs = Set(serde_json::json!([]));
        active.metadata = Set(normalize_metadata(input.metadata));
        active.updated_by = Set(Some(operator.user_id));
        active.updated_at = Set(Utc::now().into());
        let saved = active.update(db).await.map_err(db_err)?;
        secrets.invalidate(None).await;
        map_provider_profile(saved)
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
        map_provider_profile(saved)
    }

    pub async fn test_provider_profile(
        db: &DatabaseConnection,
        provider_targets: &crate::AiProviderTargetCatalog,
        egress_policy: &crate::ProviderEgressPolicy,
        secrets: &rustok_secrets::SecretResolverRegistry,
        tenant_id: Uuid,
        id: Uuid,
    ) -> AiResult<ProviderTestResult> {
        let profile = require_provider_profile(db, tenant_id, id).await?;
        secrets.invalidate(None).await;
        let config = provider_config(&profile, provider_targets, egress_policy)?;
        if crate::provider_factory_supports(&config.provider_slug, crate::ProviderFeature::Chat) {
            let provider = inference_for_slug(&config.provider_slug, &config, secrets).await?;
            return provider.test_connection(&config).await;
        }
        let started = std::time::Instant::now();
        if crate::provider_factory_supports(
            &config.provider_slug,
            crate::ProviderFeature::Embeddings,
        ) {
            crate::embed(
                &config,
                secrets,
                crate::EmbeddingRequest {
                    model: config.model.clone(),
                    documents: vec!["RusToK connectivity test".to_string()],
                    dimensions: None,
                },
            )
            .await?;
        } else if crate::provider_factory_supports(
            &config.provider_slug,
            crate::ProviderFeature::Rerank,
        ) {
            crate::rerank(
                &config,
                secrets,
                crate::RerankRequest {
                    model: config.model.clone(),
                    query: "connectivity".to_string(),
                    documents: vec!["RusToK connectivity test".to_string()],
                    top_n: Some(1),
                },
            )
            .await?;
        } else {
            return Err(AiError::InvalidConfig(format!(
                "Rig provider `{}` has no connectivity-test entrypoint",
                config.provider_slug
            )));
        }
        Ok(ProviderTestResult {
            ok: true,
            provider: config.provider_slug.to_string(),
            model: Some(config.model),
            latency_ms: started.elapsed().as_millis() as i64,
            message: "Provider responded successfully".to_string(),
        })
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
        profiles
            .into_iter()
            .map(map_task_profile)
            .collect::<AiResult<Vec<_>>>()
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
        let decision_trace = enrich_decision_trace(
            execution_plan.decision_trace,
            execution_plan.execution_mode,
            input.locale.clone(),
            resolved_locale.clone(),
        );
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
        let decision_trace = enrich_decision_trace(
            execution_plan.decision_trace,
            execution_plan.execution_mode,
            input.locale.clone(),
            resolved_locale.clone(),
        );
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

        if !claim_approval_resolution(db, operator.tenant_id, approval.id, &approval.status).await?
        {
            return Err(AiError::Validation(
                "approval request was already claimed".to_string(),
            ));
        }

        let access_context = access_context_for_operator(&execution_operator);
        let (tool_content, tool_metadata, trace) = if input.approved {
            let outcome = match approval_execution_outcome(&approval.metadata)? {
                Some(outcome) => outcome,
                None => {
                    let adapter = InProcessMcpAdapter::new(runtime, access_context)?;
                    let started = std::time::Instant::now();
                    let tool_result = match adapter
                        .call_tool(&approval.tool_name, approval.tool_input.clone())
                        .await
                    {
                        Ok(value) => value,
                        Err(error) => {
                            let mut retryable: ai_approval_requests::ActiveModel =
                                approval.clone().into();
                            retryable.status = Set("pending".to_string());
                            retryable.reason = Set(Some(format!(
                                "tool execution failed and may be retried: {error}"
                            )));
                            retryable.updated_at = Set(Utc::now().into());
                            retryable.update(db).await.map_err(db_err)?;
                            return Err(error);
                        }
                    };
                    let outcome = ApprovalExecutionOutcome {
                        content: tool_result.content,
                        raw_payload: tool_result.raw_payload,
                        duration_ms: started.elapsed().as_millis() as i64,
                    };
                    let _persisted =
                        persist_approval_execution_outcome(db, &approval, &outcome).await?;
                    outcome
                }
            };
            let trace = ToolTrace {
                tool_name: approval.tool_name.clone(),
                input_payload: approval.tool_input.clone(),
                output_payload: Some(outcome.raw_payload.clone()),
                status: "completed".to_string(),
                duration_ms: outcome.duration_ms,
                sensitive: tool_policy.is_tool_sensitive(&approval.tool_name),
                error_message: None,
                created_at: Utc::now(),
            };
            (
                outcome.content,
                json!({ "raw_payload": outcome.raw_payload, "approval_approved": true }),
                trace,
            )
        } else {
            let content = "Tool execution was rejected by the operator.".to_string();
            let trace = ToolTrace {
                tool_name: approval.tool_name.clone(),
                input_payload: approval.tool_input.clone(),
                output_payload: Some(json!({ "reason": "approval_rejected" })),
                status: "rejected".to_string(),
                duration_ms: 0,
                sensitive: tool_policy.is_tool_sensitive(&approval.tool_name),
                error_message: None,
                created_at: Utc::now(),
            };
            (content, json!({ "approval_rejected": true }), trace)
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
        if matches!(execution_mode, ExecutionMode::Direct) {
            if let (Some(task_profile), Some(handler)) = (
                task_profile.as_ref(),
                task_profile
                    .as_ref()
                    .and_then(|profile| direct_registry.handler(&profile.slug)),
            ) {
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
                    .ok_or_else(|| {
                        AiError::Runtime("failed to reload AI chat session".to_string())
                    })?;
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
        let cancellation = runtime.register_run_cancellation(run_id);
        let outcome = match agent_driver
            .run(
                &provider_config,
                crate::model::RuntimeRequest {
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
                },
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
                let mut active: ai_chat_runs::ActiveModel = run.into();
                active.status = Set("waiting_approval".to_string());
                active.pending_approval_id = Set(Some(first_approval.id));
                active.updated_at = Set(Utc::now().into());
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
