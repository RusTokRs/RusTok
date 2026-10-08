use chrono::Utc;
use sea_orm::{
    ActiveModelTrait, ActiveValue::Set, ColumnTrait, Condition, DatabaseConnection, EntityTrait,
    QueryFilter, QueryOrder, QuerySelect, TransactionTrait, sea_query::Expr,
};
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::collections::BTreeSet;
use uuid::Uuid;

use rustok_api::Permission;
use rustok_api::TenantRbacCatalog;

use crate::entities::{
    ai_agent_model_assignments, ai_agent_principals, ai_agent_workflow_runs,
    ai_agent_workflow_stages, ai_approval_requests, ai_chat_runs, ai_provider_profiles,
    ai_task_profiles,
};
use crate::{AiError, AiResult};

use super::AiManagementService;
use super::TaskJobExecutionAuthority;
use super::helpers::*;
use super::mapping::*;
use super::types::*;

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
pub(crate) async fn agent_execution_context_for_run(
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
                sea_orm::sea_query::ExprTrait::add(
                    Expr::col(ai_agent_workflow_stages::Column::AttemptCount),
                    1,
                ),
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
    pub(crate) async fn sync_agent_workflow_run_status(
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
pub(crate) struct ApprovalExecutionOutcome {
    pub(crate) content: String,
    pub(crate) output_evidence: serde_json::Value,
    pub(crate) duration_ms: i64,
}

pub(crate) fn approval_policy_evidence(
    metadata: &serde_json::Value,
) -> AiResult<Option<crate::model::ToolPolicyEvidence>> {
    metadata
        .get("tool_policy_evidence")
        .cloned()
        .map(serde_json::from_value)
        .transpose()
        .map_err(json_err)
}

pub(crate) fn validate_approval_policy_evidence(
    expected: &crate::model::ToolPolicyEvidence,
    current: crate::model::ToolPolicyEvidence,
) -> AiResult<()> {
    if current != *expected {
        return Err(AiError::Validation(
            "approved tool policy or typed contract changed; request a new approval".to_string(),
        ));
    }
    if !current.requires_operator_approval {
        return Err(AiError::Validation(
            "approved tool no longer has an operator-approval boundary; request a new run"
                .to_string(),
        ));
    }
    Ok(())
}

pub(crate) fn approval_execution_outcome(
    metadata: &serde_json::Value,
) -> AiResult<Option<ApprovalExecutionOutcome>> {
    metadata
        .get("execution_outcome")
        .cloned()
        .map(serde_json::from_value)
        .transpose()
        .map_err(json_err)
}

pub(crate) async fn persist_approval_execution_outcome(
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
pub(crate) async fn claim_approval_resolution(
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

pub(crate) enum ApprovalBatchRunTransition {
    WaitingForNext,
    ReadyToContinue,
}

pub(crate) async fn transition_run_after_approval_resolution(
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

pub(crate) fn validate_approval_resolution_policy(
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
    use super::super::{decision_trace_with_agent_usage, decision_trace_with_prompt_template};
    use super::{
        ApprovalExecutionOutcome, approval_execution_outcome, validate_approval_policy_evidence,
    };
    use crate::entities::{ai_approval_requests, ai_chat_runs, ai_tool_traces};
    use crate::{
        AgentPromptTemplateEvidence, AgentUsageEvidence, AiRunDecisionTrace, ToolDefinition,
        ToolExecutionPolicy, ToolOperationClass, model::ToolTrace,
    };
    use chrono::Utc;
    use sea_orm::{
        ActiveModelTrait, ActiveValue::Set, ConnectionTrait, DatabaseConnection, DbBackend,
        EntityTrait, Statement, TransactionTrait,
    };
    use uuid::Uuid;

    async fn approval_test_db() -> DatabaseConnection {
        let db = rustok_test_utils::setup_test_db().await;
        db.execute_raw(Statement::from_string(
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
        db.execute_raw(Statement::from_string(
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
            output_evidence: serde_json::json!({ "record": "42" }),
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
        assert!(
            approval_execution_outcome(&serde_json::json!({
                "execution_outcome": {
                    "content": "old raw payload must not decode",
                    "raw_payload": { "secret": "do-not-retain" },
                    "duration_ms": 1
                }
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

    #[test]
    fn decision_trace_persists_prompt_revision_and_provider_usage_evidence() {
        let trace = decision_trace_with_prompt_template(
            serde_json::json!({}),
            AgentPromptTemplateEvidence {
                template_digest: "sha256:template".to_string(),
                owner_task_policy_digest: Some("sha256:owner-policy".to_string()),
            },
        )
        .expect("prompt trace");
        let trace = decision_trace_with_agent_usage(
            trace,
            AgentUsageEvidence {
                provider_turns: 2,
                provider_reported_turns: 1,
                unavailable_provider_usage_turns: 1,
                invalid_provider_usage_turns: 0,
                input_tokens: 3,
                output_tokens: 5,
                total_tokens: 8,
            },
        )
        .expect("usage trace");

        let trace: AiRunDecisionTrace = serde_json::from_value(trace).expect("stored trace");
        assert_eq!(
            trace
                .prompt_template
                .as_ref()
                .map(|value| value.template_digest.as_str()),
            Some("sha256:template")
        );
        assert_eq!(
            trace.agent_usage.as_ref().map(|value| value.total_tokens),
            Some(8)
        );
    }

    #[test]
    fn approval_evidence_rejects_a_changed_schema_or_operation_class() {
        let policy = ToolExecutionPolicy::new(None, Vec::new(), Vec::new());
        let original = ToolDefinition {
            name: "workspace_apply".to_string(),
            description: "Apply reviewed workspace changes".to_string(),
            input_schema: serde_json::json!({
                "type": "object",
                "properties": { "review_id": { "type": "string" } },
                "required": ["review_id"],
                "additionalProperties": false,
            }),
            operation_class: ToolOperationClass::WorkspaceMutation,
            sensitive: false,
        };
        let expected = policy.evidence(&original);
        validate_approval_policy_evidence(&expected, policy.evidence(&original))
            .expect("unchanged approval policy evidence");

        let changed_schema = ToolDefinition {
            input_schema: serde_json::json!({
                "type": "object",
                "properties": { "review_id": { "type": "string", "minLength": 1 } },
                "required": ["review_id"],
                "additionalProperties": false,
            }),
            ..original.clone()
        };
        assert!(
            validate_approval_policy_evidence(&expected, policy.evidence(&changed_schema)).is_err()
        );

        let changed_operation = ToolDefinition {
            operation_class: ToolOperationClass::TrustPolicyChange,
            ..original
        };
        assert!(
            validate_approval_policy_evidence(&expected, policy.evidence(&changed_operation))
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
                output_evidence: serde_json::json!({ "record": "42" }),
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
        db.execute_raw(Statement::from_string(
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
                .execute_raw(Statement::from_string(
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
        ResolveAiAgentWorkflowStageApprovalInput, ai_agent_workflow_stages,
    };
    use crate::model::ExecutionMode;
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
        ) -> crate::AiResult<crate::ProviderStructuredResponse> {
            Ok(crate::ProviderStructuredResponse {
                output: json!({
                    "brand": "Example brand",
                    "material": "Cotton",
                    "color": "Blue",
                    "size": null,
                    "dimensions": null,
                    "compatibility": null,
                    "care_instructions": "Machine wash cold",
                    "hazmat": null,
                    "flex_attributes": [{"key": "fabric_weight", "value": "180 gsm"}]
                }),
                usage: None,
            })
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
                .execute_raw(Statement::from_string(
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
            .execute_raw(Statement::from_sql_and_values(
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
