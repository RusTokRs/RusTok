//! Workflow-owned durable cron admission. No lease expiry authorizes replay.
use chrono::{DateTime, Duration as ChronoDuration, FixedOffset, Utc};
use cron::Schedule;
use sea_orm::sea_query::{Expr, OnConflict};
use sea_orm::{
    ActiveModelTrait, ColumnTrait, DatabaseConnection, EntityTrait, QueryFilter, QueryOrder,
    QuerySelect, Set, TransactionTrait,
};
use sha2::{Digest, Sha256};
use uuid::Uuid;

use crate::entities::{
    ExecutionStatus, Workflow, WorkflowExecutionActiveModel, WorkflowExecutionEntity, WorkflowStatus, workflow,
    workflow_cron_cursor as cursor, workflow_cron_firing as firing,
    workflow_cron_review as review, workflow_execution,
};
use crate::error::{WorkflowError, WorkflowResult};
use crate::services::{WorkflowEngine, WorkflowService};

const MAX_SLOTS_PER_WORKFLOW_PASS: usize = 32;
pub(super) const MAX_PENDING_FIRINGS_PER_TICK: u64 = 32;
pub(super) const MAX_ACTIVE_EXECUTIONS: usize = 32;
const CATCH_UP_HORIZON_HOURS: i64 = 24;
const STALE_ATTEMPT_MINUTES: i64 = 10;
const STALE_SCAN_LIMIT: u64 = 32;

fn at(time: DateTime<Utc>) -> DateTime<FixedOffset> {
    time.fixed_offset()
}

/// Revision includes workflow.updated_at so a trigger reconfiguration
/// cannot reinterpret occurrences scheduled under older workflow state.
pub(super) fn schedule_revision(workflow: &Workflow, expression: &str) -> String {
    let mut digest = Sha256::new();
    digest.update(b"rustok:workflow-cron-schedule:v1\0");
    digest.update(workflow.id.as_bytes());
    digest.update(workflow.updated_at.timestamp_micros().to_be_bytes());
    digest.update(expression.as_bytes());
    format!("{:x}", digest.finalize())
}

fn seed_cursor(workflow: &Workflow, now: DateTime<Utc>) -> DateTime<Utc> {
    // A wall-clock-skewed node may persist updated_at in the future.
    // Keep the definition revision monotonic, but never seed a due-slot
    // cursor beyond now: doing so would silently freeze the schedule.
    workflow.updated_at.with_timezone(&Utc)
        .max(workflow.created_at.with_timezone(&Utc))
        .min(now)
        .max(now - ChronoDuration::hours(CATCH_UP_HORIZON_HOURS))
}

/// Pure schedule expansion. Unlike the legacy one-second comparison, this
/// returns slots missed during process downtime (bounded per pass).
fn due_slots(
    schedule: &Schedule,
    after: DateTime<Utc>,
    now: DateTime<Utc>,
) -> Vec<DateTime<Utc>> {
    schedule
        .after(&after)
        .take(MAX_SLOTS_PER_WORKFLOW_PASS)
        .take_while(|time| *time <= now)
        .collect()
}

/// One transaction advances the schedule cursor and inserts its slot rows.
/// Competing processes observe a CAS loser and must retry on a later tick.
pub(super) async fn enqueue_due_slots(
    db: &DatabaseConnection,
    workflow: &Workflow,
    expression: &str,
    schedule: &Schedule,
    now: DateTime<Utc>,
) -> WorkflowResult<usize> {
    let revision = schedule_revision(workflow, expression);
    let seed = seed_cursor(workflow, now);
    let transaction = db.begin().await?;

    cursor::Entity::insert(cursor::ActiveModel {
        workflow_id: Set(workflow.id),
        tenant_id: Set(workflow.tenant_id),
        schedule_revision: Set(revision.clone()),
        last_enqueued_at: Set(at(seed)),
        generation: Set(0),
        updated_at: Set(at(now)),
    })
    .on_conflict(OnConflict::column(cursor::Column::WorkflowId).do_nothing().to_owned())
    .exec_without_returning(&transaction)
    .await?;

    let stored = cursor::Entity::find_by_id(workflow.id)
        .one(&transaction)
        .await?
        .ok_or_else(|| WorkflowError::InvalidTriggerConfig(
            "cron cursor disappeared during insertion".into()
        ))?;
    // Any workflow edit changes its revision. A new definition starts at its
    // own update time, not at the previous expression's schedule cursor.
    let changed = stored.schedule_revision != revision;
    let after = if changed { seed } else { stored.last_enqueued_at.with_timezone(&Utc) };
    let slots = due_slots(schedule, after, now);
    if slots.is_empty() && !changed {
        transaction.commit().await?;
        return Ok(0);
    }

    let last = slots.last().copied().unwrap_or(after);
    let next_generation = stored.generation.checked_add(1).ok_or_else(|| {
        WorkflowError::InvalidTriggerConfig("cron cursor generation exhausted".into())
    })?;

    let affected = cursor::Entity::update_many()
        .col_expr(cursor::Column::ScheduleRevision, Expr::value(revision.clone()))
        .col_expr(cursor::Column::LastEnqueuedAt, Expr::value(at(last)))
        .col_expr(cursor::Column::UpdatedAt, Expr::value(at(now)))
        .col_expr(cursor::Column::Generation, Expr::value(next_generation))
        .filter(cursor::Column::WorkflowId.eq(workflow.id))
        .filter(cursor::Column::Generation.eq(stored.generation))
        .exec(&transaction)
        .await?
        .rows_affected;
    if affected != 1 {
        // Dropping the transaction rolls back all prior writes, including
        // the attempted cursor seed. Another replica already won.
        return Ok(0);
    }

    for scheduled_at in &slots {
        firing::Entity::insert(firing::ActiveModel {
            id: Set(Uuid::new_v4()),
            workflow_id: Set(workflow.id),
            tenant_id: Set(workflow.tenant_id),
            schedule_revision: Set(revision.clone()),
            scheduled_at: Set(at(*scheduled_at)),
            status: Set(firing::PENDING.to_owned()),
            attempt_token: Set(None),
            claimed_at: Set(None),
            execution_id: Set(None),
            reason: Set(None),
            created_at: Set(at(now)),
            completed_at: Set(None),
        })
        .on_conflict(
            OnConflict::columns([
                firing::Column::WorkflowId,
                firing::Column::ScheduleRevision,
                firing::Column::ScheduledAt,
            ]).do_nothing().to_owned()
        )
        .exec_without_returning(&transaction)
        .await?;
    }

    transaction.commit().await?;
    Ok(slots.len())
}

/// Missing Workflow-owned migration is a startup failure, not a healthy
/// scheduler that logs a database error forever.
pub(super) async fn verify_cron_schema(db: &DatabaseConnection) -> WorkflowResult<()> {
    cursor::Entity::find().limit(1).all(db).await?;
    firing::Entity::find().limit(1).all(db).await?;
    review::Entity::find().limit(1).all(db).await?;
    Ok(())
}

/// Enumerate bounded pending work. Selecting is not a claim: every caller
/// must perform a status CAS before invoking the engine.
pub(super) async fn pending_firings(
    db: &DatabaseConnection,
    max: u64,
) -> WorkflowResult<Vec<firing::Model>> {
    Ok(firing::Entity::find()
        .filter(firing::Column::Status.eq(firing::PENDING))
        .order_by_asc(firing::Column::ScheduledAt)
        .order_by_asc(firing::Column::Id)
        .limit(max.min(MAX_PENDING_FIRINGS_PER_TICK))
        .all(db)
        .await?)
}

/// Return Some only to the process that atomically changed pending->running.
pub(super) async fn claim_firing(
    db: &DatabaseConnection,
    item: &firing::Model,
    now: DateTime<Utc>,
) -> WorkflowResult<Option<Uuid>> {
    let token = Uuid::new_v4();
    let affected = firing::Entity::update_many()
        .col_expr(firing::Column::Status, Expr::value(firing::RUNNING))
        .col_expr(firing::Column::AttemptToken, Expr::value(token))
        .col_expr(firing::Column::ClaimedAt, Expr::value(at(now)))
        .filter(firing::Column::Id.eq(item.id))
        .filter(firing::Column::Status.eq(firing::PENDING))
        .exec(db).await?.rows_affected;
    Ok((affected == 1).then_some(token))
}

/// Atomic terminal transition must match the current attempt. A late
/// completion cannot overwrite a recovery/reconciliation decision.
async fn settle_claim(
    db: &DatabaseConnection,
    item: &firing::Model,
    token: Uuid,
    status: &str,
    execution_id: Option<Uuid>,
    reason: Option<&str>,
) -> WorkflowResult<bool> {
    let affected = firing::Entity::update_many()
        .col_expr(firing::Column::Status, Expr::value(status))
        .col_expr(firing::Column::ExecutionId, Expr::value(execution_id))
        .col_expr(firing::Column::Reason, Expr::value(reason.map(str::to_owned)))
        .col_expr(firing::Column::CompletedAt, Expr::value(at(Utc::now())))
        .filter(firing::Column::Id.eq(item.id))
        .filter(firing::Column::Status.eq(firing::RUNNING))
        .filter(firing::Column::AttemptToken.eq(token))
        .exec(db).await?.rows_affected;
    Ok(affected == 1)
}

/// Heartbeat only the currently fenced attempt. This reduces false stale
/// classifications of legitimate long-running workflow chains while never
/// granting ownership of an already reviewed or superseded attempt.
async fn heartbeat_claim(
    db: &DatabaseConnection,
    item: &firing::Model,
    token: Uuid,
) -> WorkflowResult<bool> {
    let affected = firing::Entity::update_many()
        .col_expr(firing::Column::ClaimedAt, Expr::value(at(Utc::now())))
        .filter(firing::Column::Id.eq(item.id))
        .filter(firing::Column::TenantId.eq(item.tenant_id))
        .filter(firing::Column::Status.eq(firing::RUNNING))
        .filter(firing::Column::AttemptToken.eq(token))
        .exec(db).await?.rows_affected;
    Ok(affected == 1)
}

/// Verify the current attempt immediately before executing a workflow.
/// Operator reconciliation invalidates old tokens; this additional fence
/// narrows the admission race, while external worker termination must still
/// be certified before an uncertain firing can be explicitly requeued.
async fn owns_current_claim(
    db: &DatabaseConnection,
    item: &firing::Model,
    token: Uuid,
) -> WorkflowResult<bool> {
    Ok(firing::Entity::find_by_id(item.id)
        .filter(firing::Column::Status.eq(firing::RUNNING))
        .filter(firing::Column::AttemptToken.eq(token))
        .one(db).await?.is_some())
}

/// Atomically reserve the execution journal in the same transaction that
/// verifies and writes the current firing attempt. A reconciliation transaction
/// cannot see 'no execution' and requeue while the previous worker is about
/// to create an execution: one of the two operations wins the firing-row CAS.
async fn reserve_execution_for_claim(
    db: &DatabaseConnection,
    item: &firing::Model,
    token: Uuid,
    context: &serde_json::Value,
) -> WorkflowResult<Option<Uuid>> {
    let transaction = db.begin().await?;
    // Serialize with workflow edit/step edit/restore, all of which lock the
    // same workflow row before writing the definition. The revision check
    // must be inside the claim+journal transaction: the earlier read-only
    // preparation check cannot fence a concurrent step graph mutation.
    let live_workflow =
        super::workflow_service::lock_workflow_for_update(
            &transaction, item.tenant_id, item.workflow_id,
        ).await?;
    let still_eligible =
        live_workflow.status == WorkflowStatus::Active
            && live_workflow.trigger_config.get("type").and_then(|v| v.as_str()) == Some("cron")
            && live_workflow.trigger_config.get("expression").and_then(|v| v.as_str())
                .is_some_and(|expression|
                    schedule_revision(&live_workflow, expression) == item.schedule_revision
                );
    if !still_eligible {
        firing::Entity::update_many()
            .col_expr(firing::Column::Status, Expr::value(firing::SKIPPED))
            .col_expr(firing::Column::Reason, Expr::value("workflow_changed_before_execution"))
            .col_expr(firing::Column::CompletedAt, Expr::value(at(Utc::now())))
            .filter(firing::Column::Id.eq(item.id))
            .filter(firing::Column::TenantId.eq(item.tenant_id))
            .filter(firing::Column::Status.eq(firing::RUNNING))
            .filter(firing::Column::AttemptToken.eq(token))
            .exec(&transaction).await?;
        transaction.commit().await?;
        return Ok(None);
    }

    let execution_id = Uuid::new_v4();
    let affected = firing::Entity::update_many()
        .col_expr(firing::Column::ClaimedAt, Expr::value(at(Utc::now())))
        .col_expr(firing::Column::ExecutionId, Expr::value(Some(execution_id)))
        .filter(firing::Column::Id.eq(item.id))
        .filter(firing::Column::TenantId.eq(item.tenant_id))
        .filter(firing::Column::Status.eq(firing::RUNNING))
        .filter(firing::Column::AttemptToken.eq(token))
        .exec(&transaction).await?.rows_affected;
    if affected != 1 {
        return Ok(None);
    }

    let prior = WorkflowExecutionEntity::find()
        .filter(workflow_execution::Column::WorkflowId.eq(item.workflow_id))
        .filter(workflow_execution::Column::TenantId.eq(item.tenant_id))
        .filter(workflow_execution::Column::TriggerEventId.eq(item.id))
        .one(&transaction).await?;
    if prior.is_some() {
        return Err(WorkflowError::CronReconciliationConflict(item.id));
    }

    WorkflowExecutionActiveModel {
        id: Set(execution_id),
        workflow_id: Set(item.workflow_id),
        tenant_id: Set(item.tenant_id),
        trigger_event_id: Set(Some(item.id)),
        status: Set(ExecutionStatus::Running),
        context: Set(context.clone()),
        error: Set(None),
        started_at: Set(at(Utc::now())),
        completed_at: Set(None),
    }.insert(&transaction).await?;
    transaction.commit().await?;
    Ok(Some(execution_id))
}

/// A firing can only advance when its schedule is still active. Claims
/// from old revisions become skipped rather than being executed under
/// changed workflow definition or a paused/archived workflow.
pub(super) async fn execute_claim(
    db: &DatabaseConnection,
    engine: &WorkflowEngine,
    service: &WorkflowService,
    item: firing::Model,
    token: Uuid,
) -> WorkflowResult<()> {
    let current = workflow::Entity::find_by_id(item.workflow_id)
        .one(db)
        .await?;
    let eligible = current.as_ref().is_some_and(|wf| {
        wf.tenant_id == item.tenant_id
            && wf.status == WorkflowStatus::Active
            && wf.trigger_config.get("type").and_then(|v| v.as_str()) == Some("cron")
            && wf.trigger_config.get("expression").and_then(|v| v.as_str())
                .is_some_and(|expression| schedule_revision(wf, expression) == item.schedule_revision)
    });
    if !eligible {
        settle_claim(db, &item, token, firing::SKIPPED, None, Some("workflow_definition_changed")).await?;
        return Ok(());
    }

    let steps = match service.load_steps(item.workflow_id).await {
        Ok(steps) => steps,
        Err(_) => {
            settle_claim(db, &item, token, firing::RECONCILIATION_REQUIRED, None, Some("steps_unavailable")).await?;
            return Ok(());
        }
    };
    // A configuration edit may race the step query. Do not apply a firing
    // against steps from an unverified newer definition.
    let current_after_load = workflow::Entity::find_by_id(item.workflow_id)
        .one(db).await?;
    if current_after_load.as_ref().is_none_or(|wf| {
        wf.status != WorkflowStatus::Active
            || wf.tenant_id != item.tenant_id
            || wf.trigger_config.get("expression").and_then(|v| v.as_str())
                .is_none_or(|expression| schedule_revision(wf, expression) != item.schedule_revision)
    }) {
        settle_claim(db, &item, token, firing::SKIPPED, None, Some("workflow_changed_during_preparation")).await?;
        return Ok(());
    }

    if !owns_current_claim(db, &item, token).await? {
        return Ok(());
    }

    let context = serde_json::json!({
        "trigger": {
            "type": "cron",
            "scheduled_at": item.scheduled_at.to_rfc3339(),
            "firing_id": item.id.to_string()
        }
    });
    // Both the attempt check and the first execution record are committed
    // together, before the first possible external step side effect. This
    // closes the stale-worker vs operator "no journal yet" requeue window.
    let execution_id = match reserve_execution_for_claim(db, &item, token, &context).await? {
        Some(execution_id) => execution_id,
        None => return Ok(()),
    };
    // Keep an accepted operation alive while the engine is processing an
    // external provider call. Polling a pinned future in the select preserves
    // it across heartbeats; shutdown does not cancel the owner operation.
    let execution = engine.execute_preclaimed_cron(
        execution_id, item.id, token, steps, context,
    );
    tokio::pin!(execution);
    let mut heartbeat = tokio::time::interval_at(
        tokio::time::Instant::now() + tokio::time::Duration::from_secs(30),
        tokio::time::Duration::from_secs(30),
    );
    let result = loop {
        tokio::select! {
            result = &mut execution => break result,
            _ = heartbeat.tick() => {
                match heartbeat_claim(db, &item, token).await {
                    Ok(true) => {}
                    Ok(false) => {
                        tracing::warn!(
                            firing_id = %item.id,
                            "Workflow cron attempt lost its claim; subsequent steps will fail closed"
                        );
                    }
                    Err(error) => {
                        tracing::error!(
                            firing_id = %item.id,
                            %error,
                            "Workflow cron heartbeat failed; external operation remains under review"
                        );
                    }
                }
            }
        }
    };

    let recorded = WorkflowExecutionEntity::find()
        .filter(workflow_execution::Column::WorkflowId.eq(item.workflow_id))
        .filter(workflow_execution::Column::TenantId.eq(item.tenant_id))
        .filter(workflow_execution::Column::TriggerEventId.eq(item.id))
        .one(db).await?;
    // The journal is the authority. If the engine returned a transport error
    // after writing a terminal receipt, do not overwrite that known result
    // with an ambiguous firing status.
    let (next_status, reason) = match recorded.as_ref() {
        Some(execution)
            if execution.id == execution_id
                && matches!(execution.status,
                    ExecutionStatus::Completed | ExecutionStatus::Failed | ExecutionStatus::TimedOut
                ) =>
        {
            if let Err(error) = &result {
                tracing::warn!(
                    firing_id = %item.id, %error,
                    "Workflow engine returned an error after a durable terminal journal"
                );
            }
            (
                firing::COMPLETED,
                if execution.status == ExecutionStatus::Completed {
                    "completed"
                } else {
                    "execution_failed"
                },
            )
        }
        _ => (firing::RECONCILIATION_REQUIRED, "execution_outcome_ambiguous"),
    };
    // The reservation wrote this ID before any external effect. Keep the ID
    // even if a subsequent journal lookup fails, so operators can investigate.
    if !settle_claim(db, &item, token, next_status, Some(execution_id), Some(reason)).await? {
        tracing::warn!(
            firing_id = %item.id,
            "Cron firing was already fenced by a concurrent recovery decision"
        );
    }
    Ok(())
}

/// Quarantine only if the SAME attempt remains older than the cutoff.
/// A previously selected candidate may have heartbeated since the read:
/// checking the age again in the update CAS is mandatory.
async fn quarantine_stale_candidate(
    db: &DatabaseConnection,
    item: &firing::Model,
    cutoff: DateTime<Utc>,
    now: DateTime<Utc>,
) -> WorkflowResult<bool> {
    let Some(token) = item.attempt_token else { return Ok(false) };
    let affected = firing::Entity::update_many()
        .col_expr(firing::Column::Status, Expr::value(firing::RECONCILIATION_REQUIRED))
        .col_expr(firing::Column::Reason, Expr::value("stale_attempt_requires_reconciliation"))
        .col_expr(firing::Column::CompletedAt, Expr::value(at(now)))
        .filter(firing::Column::Id.eq(item.id))
        .filter(firing::Column::TenantId.eq(item.tenant_id))
        .filter(firing::Column::Status.eq(firing::RUNNING))
        .filter(firing::Column::AttemptToken.eq(token))
        .filter(firing::Column::ClaimedAt.lt(at(cutoff)))
        .exec(db).await?.rows_affected;
    Ok(affected == 1)
}

/// A crash leaves a running attempt. Time alone does not authorize retry:
/// classify old claims for operator reconciliation without re-dispatching.
pub(super) async fn quarantine_stale_attempts(
    db: &DatabaseConnection,
    now: DateTime<Utc>,
) -> WorkflowResult<u64> {
    let cutoff = now - ChronoDuration::minutes(STALE_ATTEMPT_MINUTES);
    let stale = firing::Entity::find()
        .filter(firing::Column::Status.eq(firing::RUNNING))
        .filter(firing::Column::ClaimedAt.lt(at(cutoff)))
        .order_by_asc(firing::Column::ClaimedAt)
        .limit(STALE_SCAN_LIMIT)
        .all(db).await?;
    let mut quarantined = 0;
    for item in stale {
        if quarantine_stale_candidate(db, &item, cutoff, now).await? {
            quarantined += 1;
        }
    }
    Ok(quarantined)
}

/// Read-only owner projection for an authenticated tenant operator surface.
/// It intentionally does not provide a blind "reset to pending" operation.
pub async fn reconciliation_required_for_tenant(
    db: &DatabaseConnection, tenant_id: Uuid, limit: u64,
) -> WorkflowResult<Vec<firing::Model>> {
    if tenant_id.is_nil() { return Ok(Vec::new()); }
    Ok(firing::Entity::find()
        .filter(firing::Column::TenantId.eq(tenant_id))
        .filter(firing::Column::Status.eq(firing::RECONCILIATION_REQUIRED))
        .order_by_asc(firing::Column::ScheduledAt)
        .limit(limit.min(100))
        .all(db).await?)
}

/// Read-only visibility for legacy or event-driven executions without a
/// cron firing row. Never infer that running means safe to replay: old builds
/// may have emitted remote side effects before a process crash.
pub async fn unconfirmed_executions_for_tenant(
    db: &DatabaseConnection,
    tenant_id: Uuid,
    limit: u64,
) -> WorkflowResult<Vec<workflow_execution::Model>> {
    if tenant_id.is_nil() {
        return Ok(Vec::new());
    }
    Ok(WorkflowExecutionEntity::find()
        .filter(workflow_execution::Column::TenantId.eq(tenant_id))
        .filter(workflow_execution::Column::Status.is_in([
            ExecutionStatus::Running.to_string(),
            ExecutionStatus::ReconciliationRequired.to_string(),
        ]))
        .order_by_asc(workflow_execution::Column::StartedAt)
        .order_by_asc(workflow_execution::Column::Id)
        .limit(limit.min(100))
        .all(db).await?)
}

/// All decisions require a privileged transport to supply the authenticated
/// actor. A stale claim has no automatic retry authority.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CronReconciliationDecision {
    ConfirmTerminal,
    RequeueConfirmedUnstarted,
    /// Operator has independently reconciled the entire ambiguous workflow
    /// and certifies the full execution completed.
    ConfirmExternalCompleted,
    /// Operator has fenced the prior worker and certifies the execution
    /// failed/stopped; no remaining steps will be automatically reissued.
    ConfirmExternalFailed,
}

/// Operator recovery is an explicit single-transaction state transition with
/// immutable audit evidence, scoped to the current tenant and attempt fence.
/// `prior_worker_fenced` must only be attested after the operator verified the
/// previous runtime instance can no longer issue effects (e.g. terminated
/// replica + confirmed DB rollback). A timestamp alone is not sufficient.
pub async fn reconcile_firing(
    db: &DatabaseConnection,
    tenant_id: Uuid,
    firing_id: Uuid,
    actor_id: Uuid,
    decision: CronReconciliationDecision,
    evidence: &str,
    prior_worker_fenced: bool,
) -> WorkflowResult<firing::Model> {
    let evidence = evidence.trim();
    if tenant_id.is_nil() || actor_id.is_nil()
        || evidence.len() < 32 || evidence.len() > 2_048
        || evidence.contains('\0')
    {
        return Err(WorkflowError::InvalidCronReconciliationEvidence);
    }
    let txn = db.begin().await?;
    let item = firing::Entity::find_by_id(firing_id)
        .filter(firing::Column::TenantId.eq(tenant_id))
        .one(&txn).await?
        .ok_or(WorkflowError::CronReconciliationConflict(firing_id))?;
    if item.status != firing::RECONCILIATION_REQUIRED {
        return Err(WorkflowError::CronReconciliationConflict(firing_id));
    }
    let previous_token = item.attempt_token.ok_or(
        WorkflowError::CronReconciliationConflict(firing_id)
    )?;
    let existing = WorkflowExecutionEntity::find()
        .filter(workflow_execution::Column::WorkflowId.eq(item.workflow_id))
        .filter(workflow_execution::Column::TenantId.eq(tenant_id))
        .filter(workflow_execution::Column::TriggerEventId.eq(item.id))
        .one(&txn).await?;

    let (next_status, execution_id, reason) = match decision {
        CronReconciliationDecision::ConfirmTerminal => {
            let execution = existing.ok_or(
                WorkflowError::CronReconciliationConflict(firing_id)
            )?;
            if matches!(execution.status,
                ExecutionStatus::Running | ExecutionStatus::ReconciliationRequired
            ) {
                return Err(WorkflowError::CronReconciliationConflict(firing_id));
            }
            (firing::COMPLETED, Some(execution.id), "operator_confirmed_terminal")
        }
        CronReconciliationDecision::ConfirmExternalCompleted
        | CronReconciliationDecision::ConfirmExternalFailed => {
            // No operator may settle an ambiguous result while the old
            // executor can still write. A completed/failed declaration here
            // is an explicit human reconciliation, never inference from time.
            if !prior_worker_fenced {
                return Err(WorkflowError::CronReconciliationConflict(firing_id));
            }
            let execution = existing.ok_or(
                WorkflowError::CronReconciliationConflict(firing_id)
            )?;
            if !matches!(execution.status,
                ExecutionStatus::Running | ExecutionStatus::ReconciliationRequired
            ) {
                return Err(WorkflowError::CronReconciliationConflict(firing_id));
            }
            let completed_status = match decision {
                CronReconciliationDecision::ConfirmExternalCompleted =>
                    ExecutionStatus::Completed,
                _ => ExecutionStatus::Failed,
            };
            let affected = WorkflowExecutionEntity::update_many()
                .col_expr(
                    workflow_execution::Column::Status,
                    Expr::value(completed_status.to_string()),
                )
                .col_expr(
                    workflow_execution::Column::Error,
                    Expr::value(Some("operator_reconciled_external_outcome".to_owned())),
                )
                .col_expr(
                    workflow_execution::Column::CompletedAt,
                    Expr::value(at(Utc::now())),
                )
                .filter(workflow_execution::Column::Id.eq(execution.id))
                .filter(workflow_execution::Column::TenantId.eq(tenant_id))
                .filter(workflow_execution::Column::WorkflowId.eq(item.workflow_id))
                .filter(workflow_execution::Column::TriggerEventId.eq(firing_id))
                .filter(workflow_execution::Column::Status.is_in([
                    ExecutionStatus::Running.to_string(),
                    ExecutionStatus::ReconciliationRequired.to_string(),
                ]))
                .exec(&txn).await?.rows_affected;
            if affected != 1 {
                return Err(WorkflowError::CronReconciliationConflict(firing_id));
            }
            (
                firing::COMPLETED,
                Some(execution.id),
                if completed_status == ExecutionStatus::Completed {
                    "operator_confirmed_external_completed"
                } else {
                    "operator_confirmed_external_failed"
                },
            )
        }
        CronReconciliationDecision::RequeueConfirmedUnstarted => {
            // An execution journal is inserted *before* the engine's first
            // step effect. If one exists, this command cannot prove that any
            // effect was not already emitted. The old worker must also have
            // been externally fenced; no process-local lease expiry suffices.
            if existing.is_some() || !prior_worker_fenced {
                return Err(WorkflowError::CronReconciliationConflict(firing_id));
            }
            (firing::PENDING, None, "operator_confirmed_unstarted")
        }
    };

    let affected = firing::Entity::update_many()
        .col_expr(firing::Column::Status, Expr::value(next_status))
        .col_expr(firing::Column::ExecutionId, Expr::value(execution_id))
        .col_expr(firing::Column::Reason, Expr::value(reason))
        .col_expr(firing::Column::AttemptToken, Expr::value(None::<Uuid>))
        .col_expr(firing::Column::ClaimedAt, Expr::value(None::<DateTime<FixedOffset>>))
        .col_expr(firing::Column::CompletedAt, Expr::value(
            if next_status == firing::COMPLETED { Some(at(Utc::now())) } else { None }
        ))
        .filter(firing::Column::Id.eq(firing_id))
        .filter(firing::Column::TenantId.eq(tenant_id))
        .filter(firing::Column::Status.eq(firing::RECONCILIATION_REQUIRED))
        .filter(firing::Column::AttemptToken.eq(previous_token))
        .exec(&txn).await?.rows_affected;
    if affected != 1 {
        return Err(WorkflowError::CronReconciliationConflict(firing_id));
    }

    review::Entity::insert(review::ActiveModel {
        id: Set(Uuid::new_v4()),
        firing_id: Set(firing_id),
        tenant_id: Set(tenant_id),
        actor_id: Set(actor_id),
        decision: Set(match decision {
            CronReconciliationDecision::ConfirmTerminal => "confirm_terminal",
            CronReconciliationDecision::RequeueConfirmedUnstarted => "requeue_confirmed_unstarted",
            CronReconciliationDecision::ConfirmExternalCompleted => "confirm_external_completed",
            CronReconciliationDecision::ConfirmExternalFailed => "confirm_external_failed",
        }.to_owned()),
        evidence: Set(evidence.to_owned()),
        previous_attempt_token: Set(Some(previous_token)),
        created_at: Set(at(Utc::now())),
    })
    .exec_without_returning(&txn).await?;
    txn.commit().await?;

    firing::Entity::find_by_id(firing_id).one(db).await?
        .ok_or(WorkflowError::CronReconciliationConflict(firing_id))
}

#[cfg(test)]
mod tests {
    use super::*;
    use sea_orm::{Database, EntityTrait};
    use sea_orm_migration::SchemaManager;

    #[test]
    fn missed_cron_slots_are_bounded_and_reproducible() {
        use std::str::FromStr;
        let schedule = Schedule::from_str("0 * * * * *").unwrap();
        let now = DateTime::parse_from_rfc3339("2026-11-23T12:30:00Z").unwrap().with_timezone(&Utc);
        let previous = now - ChronoDuration::minutes(90);
        let slots = due_slots(&schedule, previous, now);
        assert_eq!(slots.len(), MAX_SLOTS_PER_WORKFLOW_PASS);
        assert_eq!(slots[0], previous + ChronoDuration::minutes(1));
    }

    #[tokio::test]
    async fn missing_cron_owner_schema_rejects_bootstrap() {
        let db = Database::connect("sqlite::memory:").await.unwrap();
        assert!(verify_cron_schema(&db).await.is_err());
    }

    #[tokio::test]
    async fn expired_claim_requires_evidence_and_old_attempt_is_fenced() {
        let db = Database::connect("sqlite::memory:").await.unwrap();
        for migration in crate::migrations::migrations() {
            migration.up(&SchemaManager::new(&db)).await.unwrap();
        }
        let now = Utc::now();
        let wf_id = Uuid::new_v4();
        let tenant_id = Uuid::new_v4();
        workflow::ActiveModel {
            id: Set(wf_id),
            tenant_id: Set(tenant_id),
            name: Set("Reviewable cron".to_owned()),
            description: Set(None),
            status: Set(WorkflowStatus::Active),
            trigger_config: Set(serde_json::json!({"type":"cron","expression":"0 * * * * *"})),
            created_by: Set(None),
            created_at: Set(at(now - ChronoDuration::minutes(5))),
            updated_at: Set(at(now - ChronoDuration::minutes(5))),
            failure_count: Set(0),
            auto_disabled_at: Set(None),
            webhook_slug: Set(None),
            webhook_secret: Set(None),
        }.insert(&db).await.unwrap();
        let current = workflow::Entity::find_by_id(wf_id).one(&db).await.unwrap().unwrap();
        let expression = "0 * * * * *";
        enqueue_due_slots(&db, &current, expression, &expression.parse().unwrap(), now)
            .await.unwrap();
        let item = pending_firings(&db, 1).await.unwrap().remove(0);
        let token = claim_firing(&db, &item, now - ChronoDuration::minutes(15))
            .await.unwrap().expect("first attempt owns the claim");
        let previously_stale = firing::Entity::find_by_id(item.id)
            .one(&db).await.unwrap().unwrap();
        assert!(heartbeat_claim(&db, &item, token).await.unwrap());
        assert!(!quarantine_stale_candidate(
            &db, &previously_stale,
            now - ChronoDuration::minutes(STALE_ATTEMPT_MINUTES), now,
        ).await.unwrap(), "a refreshed heartbeat must win over a stale scan");
        let running = firing::Entity::find_by_id(item.id).one(&db).await.unwrap().unwrap();
        assert_eq!(running.status, firing::RUNNING);

        // Simulate an actually dead worker by moving the heartbeat back.
        firing::Entity::update_many()
            .col_expr(
                firing::Column::ClaimedAt,
                Expr::value(at(now - ChronoDuration::minutes(15))),
            )
            .filter(firing::Column::Id.eq(item.id))
            .filter(firing::Column::AttemptToken.eq(token))
            .exec(&db).await.unwrap();
        assert_eq!(quarantine_stale_attempts(&db, now).await.unwrap(), 1);
        assert!(pending_firings(&db, 10).await.unwrap().iter().all(|row| row.id != item.id));

        let actor_id = Uuid::new_v4();
        let evidence = "Process was stopped and no execution journal entry was created";
        assert!(reconcile_firing(&db, tenant_id, item.id, actor_id,
            CronReconciliationDecision::RequeueConfirmedUnstarted, evidence, false,
        ).await.is_err(), "time expiry alone cannot requeue uncertain work");
        assert!(reconcile_firing(&db, Uuid::new_v4(), item.id, actor_id,
            CronReconciliationDecision::RequeueConfirmedUnstarted, evidence, true,
        ).await.is_err(), "another tenant cannot reconcile this firing");

        let requeued = reconcile_firing(&db, tenant_id, item.id, actor_id,
            CronReconciliationDecision::RequeueConfirmedUnstarted, evidence, true,
        ).await.unwrap();
        assert_eq!(requeued.status, firing::PENDING);
        assert!(!owns_current_claim(&db, &item, token).await.unwrap());
        assert!(!settle_claim(&db, &item, token, firing::COMPLETED, None, Some("stale"))
            .await.unwrap(), "old attempt cannot acknowledge a new admission");
        let next_token = claim_firing(&db, &requeued, now).await.unwrap().unwrap();
        assert_ne!(next_token, token);
        let audit = review::Entity::find().all(&db).await.unwrap();
        assert_eq!(audit.len(), 1);
        assert_eq!(audit[0].actor_id, actor_id);
        assert_eq!(audit[0].previous_attempt_token, Some(token));
        assert_eq!(audit[0].evidence, evidence);

        // A later attempt has a durable execution journal; it cannot be
        // requeued even with a second operator attestation.
        let execution_id = Uuid::new_v4();
        workflow_execution::ActiveModel {
            id: Set(execution_id),
            workflow_id: Set(wf_id),
            tenant_id: Set(tenant_id),
            trigger_event_id: Set(Some(item.id)),
            status: Set(ExecutionStatus::Completed),
            context: Set(serde_json::Value::Null),
            error: Set(None),
            started_at: Set(at(now)),
            completed_at: Set(Some(at(now))),
        }.insert(&db).await.unwrap();

        assert!(settle_claim(
            &db, &requeued, next_token, firing::RECONCILIATION_REQUIRED,
            Some(execution_id), Some("acknowledgement_lost"),
        ).await.unwrap());
        assert!(reconcile_firing(&db, tenant_id, item.id, actor_id,
            CronReconciliationDecision::RequeueConfirmedUnstarted, evidence, true,
        ).await.is_err(), "a started execution must never be blindly requeued");

        let completed = reconcile_firing(&db, tenant_id, item.id, actor_id,
            CronReconciliationDecision::ConfirmTerminal, evidence, false,
        ).await.unwrap();
        assert_eq!(completed.status, firing::COMPLETED);
        assert_eq!(completed.execution_id, Some(execution_id));
        assert!(claim_firing(&db, &completed, now).await.unwrap().is_none());
        let audit = review::Entity::find().all(&db).await.unwrap();
        assert_eq!(audit.len(), 2);
    }

    #[tokio::test]
    async fn same_slot_is_queued_once_and_claimed_by_one_attempt() {
        let db = Database::connect("sqlite::memory:").await.unwrap();
        for migration in crate::migrations::migrations() {
            migration.up(&SchemaManager::new(&db)).await.unwrap();
        }
        let now = Utc::now();
        let wf_id = Uuid::new_v4();
        let tenant_id = Uuid::new_v4();
        let wf = workflow::ActiveModel {
            id: Set(wf_id), tenant_id: Set(tenant_id),
            name: Set("Durable cron".to_string()), description: Set(None),
            status: Set(WorkflowStatus::Active),
            trigger_config: Set(serde_json::json!({"type":"cron","expression":"0 * * * * *"})),
            created_by: Set(None), created_at: Set(at(now - ChronoDuration::minutes(5))),
            updated_at: Set(at(now - ChronoDuration::minutes(5))),
            failure_count: Set(0), auto_disabled_at: Set(None),
            webhook_slug: Set(None), webhook_secret: Set(None),
        };
        wf.insert(&db).await.unwrap();
        let record = workflow::Entity::find_by_id(wf_id).one(&db).await.unwrap().unwrap();
        let schedule = "0 * * * * *".parse::<Schedule>().unwrap();
        let _ = enqueue_due_slots(&db, &record, "0 * * * * *", &schedule, now).await.unwrap();
        let _ = enqueue_due_slots(&db, &record, "0 * * * * *", &schedule, now).await.unwrap();
        let pending = pending_firings(&db, 32).await.unwrap();
        assert_eq!(pending.len(), 5);
        let first = &pending[0];
        // Two contenders attempt to claim the same durable slot. Only one
        // process gets an attempt token even when both selected the same row.
        let (first_claim, second_claim) = tokio::join!(
            claim_firing(&db, first, now),
            claim_firing(&db, first, now),
        );
        let owners = [first_claim.unwrap(), second_claim.unwrap()];
        assert_eq!(owners.iter().filter(|value| value.is_some()).count(), 1);
        let token = owners.into_iter().flatten().next().unwrap();
        assert!(claim_firing(&db, first, now).await.unwrap().is_none());

        let context = serde_json::json!({
            "trigger": {"type":"cron","firing_id":first.id.to_string()}
        });
        let execution_id = reserve_execution_for_claim(&db, first, token, &context)
            .await.unwrap().expect("journal reserved under owning attempt");
        let journal = WorkflowExecutionEntity::find_by_id(execution_id)
            .one(&db).await.unwrap().unwrap();
        assert_eq!(journal.trigger_event_id, Some(first.id));
        assert_eq!(journal.status, ExecutionStatus::Running);
        assert!(reserve_execution_for_claim(&db, first, token, &context).await.is_err(),
            "a journal cannot be allocated twice for one trigger");

        // Even after a stale-attempt quarantine, the operator cannot requeue
        // a firing for which a step execution could have begun.
        assert_eq!(quarantine_stale_attempts(
            &db, Utc::now() + ChronoDuration::minutes(15),
        ).await.unwrap(), 1);
        let actor_id = Uuid::new_v4();
        let evidence = "External provider case verified and prior worker process fully fenced";
        assert!(reconcile_firing(
            &db, tenant_id, first.id, actor_id,
            CronReconciliationDecision::RequeueConfirmedUnstarted,
            evidence, true,
        ).await.is_err());
        assert!(reconcile_firing(
            &db, tenant_id, first.id, actor_id,
            CronReconciliationDecision::ConfirmExternalFailed,
            evidence, false,
        ).await.is_err(), "an active worker cannot be overridden by an unverified review");

        let resolved = reconcile_firing(
            &db, tenant_id, first.id, actor_id,
            CronReconciliationDecision::ConfirmExternalFailed,
            evidence, true,
        ).await.unwrap();
        assert_eq!(resolved.status, firing::COMPLETED);
        let journal = WorkflowExecutionEntity::find_by_id(execution_id)
            .one(&db).await.unwrap().unwrap();
        assert_eq!(journal.status, ExecutionStatus::Failed);
        assert!(!heartbeat_claim(&db, first, token).await.unwrap(),
            "a previous worker may not extend an operator-settled attempt");
        let audit = review::Entity::find().all(&db).await.unwrap();
        assert_eq!(audit.len(), 1);
        assert_eq!(audit[0].actor_id, actor_id);
        assert_eq!(audit[0].evidence, evidence);
    }
}
