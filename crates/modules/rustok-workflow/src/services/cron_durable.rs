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
    ExecutionStatus, Workflow, WorkflowExecutionEntity, WorkflowStatus, workflow,
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
    workflow.updated_at.with_timezone(&Utc).max(
        workflow.created_at.with_timezone(&Utc),
    ).max(now - ChronoDuration::hours(CATCH_UP_HORIZON_HOURS))
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

    let context = serde_json::json!({
        "trigger": {
            "type": "cron",
            "scheduled_at": item.scheduled_at.to_rfc3339(),
            "firing_id": item.id.to_string()
        }
    });
    // Reusing the stored firing UUID as trigger_event_id invokes the
    // existing unique (workflow_id, trigger_event_id) engine guard.
    let result = engine.execute(
        item.workflow_id, item.tenant_id, Some(item.id), steps, context,
    ).await;

    let recorded = WorkflowExecutionEntity::find()
        .filter(workflow_execution::Column::WorkflowId.eq(item.workflow_id))
        .filter(workflow_execution::Column::TriggerEventId.eq(item.id))
        .one(db).await?;
    match (result, recorded) {
        (Ok(id), Some(execution))
            if id == execution.id
                && matches!(execution.status, ExecutionStatus::Completed | ExecutionStatus::Failed | ExecutionStatus::TimedOut) =>
        {
            let _ = settle_claim(db, &item, token, firing::COMPLETED, Some(id),
                Some(if execution.status == ExecutionStatus::Completed { "completed" } else { "execution_failed" })
            ).await?;
        }
        (_, maybe_execution) => {
            let _ = settle_claim(db, &item, token, firing::RECONCILIATION_REQUIRED,
                maybe_execution.map(|execution| execution.id),
                Some("execution_outcome_ambiguous")).await?;
        }
    }
    Ok(())
}

/// A crash leaves a running attempt. Time alone does not authorize retry:
/// classify old claims for operator reconciliation without re-dispatching.
pub(super) async fn quarantine_stale_attempts(
    db: &DatabaseConnection,
    now: DateTime<Utc>,
) -> WorkflowResult<u64> {
    let stale = firing::Entity::find()
        .filter(firing::Column::Status.eq(firing::RUNNING))
        .filter(firing::Column::ClaimedAt.lt(at(now - ChronoDuration::minutes(STALE_ATTEMPT_MINUTES))))
        .order_by_asc(firing::Column::ClaimedAt)
        .limit(STALE_SCAN_LIMIT)
        .all(db).await?;
    let mut quarantined = 0;
    for item in stale {
        if let Some(token) = item.attempt_token {
            if settle_claim(db, &item, token, firing::RECONCILIATION_REQUIRED,
                item.execution_id, Some("stale_attempt_requires_reconciliation")).await? {
                quarantined += 1;
            }
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

/// All decisions require a privileged transport to supply the authenticated
/// actor. A stale claim has no automatic retry authority.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CronReconciliationDecision {
    ConfirmTerminal,
    RequeueConfirmedUnstarted,
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
        assert!(claim_firing(&db, first, now).await.unwrap().is_some());
        assert!(claim_firing(&db, first, now).await.unwrap().is_none());
    }
}
