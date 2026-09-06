use chrono::Utc;
use rustok_api::{PortActorKind, PortContext};
use sea_orm::{ColumnTrait, DatabaseTransaction, EntityTrait, QueryFilter, sea_query::Expr};
use uuid::Uuid;

use crate::commands::finish;
use crate::domain::{
    ModerationApplicationOperationStatus, ModerationApplicationRecoveryRecord,
    ModerationCaseStatus, ReconcileLegacyModerationApplicationCommand,
    RequeueModerationApplicationCommand,
};
use crate::entities::{moderation_application_operation, moderation_case, moderation_decision};
use crate::error::{ModerationError, ModerationResult};
use crate::receipts::{
    ModerationReceiptAdmission, NewModerationReceipt, admit, replay, replay_existing, request_hash,
    required_idempotency_key,
};
use crate::service::{ModerationService, actor_uuid, append_event, find_case, parse_tenant_id};

const OP_REQUEUE_APPLICATION: &str = "operator_requeue_application";
const OP_RECONCILE_LEGACY_APPLICATION: &str = "operator_reconcile_legacy_application";
pub const MAX_APPLICATION_RECOVERY_REASON_BYTES: usize = 1_000;

impl ModerationService {
    pub async fn operator_requeue_application_replay_safe(
        &self,
        context: PortContext,
        mut command: RequeueModerationApplicationCommand,
    ) -> ModerationResult<ModerationApplicationRecoveryRecord> {
        context
            .require_write_semantics()
            .map_err(|error| ModerationError::Validation(error.message))?;
        let tenant_id = parse_tenant_id(&context)?;
        let operator_id = require_human_operator(&context)?;
        validate_expected_case_revision(command.expected_case_revision)?;
        command.reason = normalize_recovery_reason(command.reason)?;

        let key = required_idempotency_key(&context)?;
        let hash = request_hash(OP_REQUEUE_APPLICATION, &context.actor, &command)?;
        if let Some(response) = replay_existing(
            self.database(),
            tenant_id,
            OP_REQUEUE_APPLICATION,
            key.as_str(),
            hash.as_str(),
        )
        .await?
        {
            return Ok(response);
        }

        match admit(
            self.database(),
            tenant_id,
            OP_REQUEUE_APPLICATION,
            key,
            hash.as_str(),
        )
        .await?
        {
            ModerationReceiptAdmission::Replay(receipt) => {
                replay(receipt, OP_REQUEUE_APPLICATION, hash.as_str())
            }
            ModerationReceiptAdmission::New(receipt) => {
                let result =
                    requeue_application_in_transaction(&receipt, tenant_id, operator_id, command)
                        .await;
                finish(receipt, result).await
            }
        }
    }

    pub async fn operator_reconcile_legacy_application_replay_safe(
        &self,
        context: PortContext,
        mut command: ReconcileLegacyModerationApplicationCommand,
    ) -> ModerationResult<ModerationApplicationRecoveryRecord> {
        context
            .require_write_semantics()
            .map_err(|error| ModerationError::Validation(error.message))?;
        let tenant_id = parse_tenant_id(&context)?;
        let operator_id = require_human_operator(&context)?;
        validate_expected_case_revision(command.expected_case_revision)?;
        command.reason = normalize_recovery_reason(command.reason)?;

        let key = required_idempotency_key(&context)?;
        let hash = request_hash(OP_RECONCILE_LEGACY_APPLICATION, &context.actor, &command)?;
        if let Some(response) = replay_existing(
            self.database(),
            tenant_id,
            OP_RECONCILE_LEGACY_APPLICATION,
            key.as_str(),
            hash.as_str(),
        )
        .await?
        {
            return Ok(response);
        }

        match admit(
            self.database(),
            tenant_id,
            OP_RECONCILE_LEGACY_APPLICATION,
            key,
            hash.as_str(),
        )
        .await?
        {
            ModerationReceiptAdmission::Replay(receipt) => {
                replay(receipt, OP_RECONCILE_LEGACY_APPLICATION, hash.as_str())
            }
            ModerationReceiptAdmission::New(receipt) => {
                let result = reconcile_legacy_application_in_transaction(
                    &receipt,
                    tenant_id,
                    operator_id,
                    command,
                )
                .await;
                finish(receipt, result).await
            }
        }
    }
}

async fn requeue_application_in_transaction(
    receipt: &NewModerationReceipt,
    tenant_id: Uuid,
    operator_id: Uuid,
    command: RequeueModerationApplicationCommand,
) -> ModerationResult<ModerationApplicationRecoveryRecord> {
    let operation = find_operation(&receipt.transaction, tenant_id, command.decision_id).await?;
    let operation_status = parse_operation_status(operation.status.as_str())?;
    if !matches!(
        operation_status,
        ModerationApplicationOperationStatus::Rejected
            | ModerationApplicationOperationStatus::OperatorReview
    ) {
        return Err(ModerationError::Validation(
            "operator requeue requires a rejected or operator_review application; applied decisions must never be requeued"
                .to_string(),
        ));
    }
    validate_terminal_operation_shape(&operation, operation_status)?;

    let case = find_case(&receipt.transaction, tenant_id, operation.case_id).await?;
    validate_recovery_identity(&receipt.transaction, tenant_id, &operation, &case).await?;
    require_expected_case_revision(&case, command.expected_case_revision)?;
    let previous_case_status = parse_case_status(case.status.as_str())?;
    if !matches!(
        previous_case_status,
        ModerationCaseStatus::Escalated | ModerationCaseStatus::Decided
    ) {
        return Err(ModerationError::LifecycleConflict {
            from: case.status,
            to: ModerationCaseStatus::ApplyingDecision.as_str().to_string(),
        });
    }

    let previous_error_code = operation.last_error_code.clone();
    let previous_error_message = operation.last_error_message.clone();
    let now = Utc::now().fixed_offset();
    let updated = moderation_application_operation::Entity::update_many()
        .col_expr(
            moderation_application_operation::Column::Status,
            Expr::value(ModerationApplicationOperationStatus::Retryable.as_str()),
        )
        .col_expr(
            moderation_application_operation::Column::NextAttemptAt,
            Expr::value(now),
        )
        .col_expr(
            moderation_application_operation::Column::LeaseToken,
            Expr::value(Option::<Uuid>::None),
        )
        .col_expr(
            moderation_application_operation::Column::LeaseOwner,
            Expr::value(Option::<String>::None),
        )
        .col_expr(
            moderation_application_operation::Column::LeaseExpiresAt,
            Expr::value(Option::<chrono::DateTime<chrono::FixedOffset>>::None),
        )
        .col_expr(
            moderation_application_operation::Column::LastErrorCode,
            Expr::value(Option::<String>::None),
        )
        .col_expr(
            moderation_application_operation::Column::LastErrorMessage,
            Expr::value(Option::<String>::None),
        )
        .col_expr(
            moderation_application_operation::Column::UpdatedAt,
            Expr::value(now),
        )
        .filter(moderation_application_operation::Column::TenantId.eq(tenant_id))
        .filter(moderation_application_operation::Column::DecisionId.eq(command.decision_id))
        .filter(moderation_application_operation::Column::Status.eq(operation_status.as_str()))
        .exec(&receipt.transaction)
        .await?;
    if updated.rows_affected != 1 {
        return Err(ModerationError::ApplicationRecoveryConflict(
            command.decision_id,
        ));
    }

    let case_revision = transition_case_status_for_recovery(
        &receipt.transaction,
        tenant_id,
        &case,
        previous_case_status,
        ModerationCaseStatus::ApplyingDecision,
        now,
    )
    .await?;

    append_event(
        &receipt.transaction,
        tenant_id,
        "application",
        command.decision_id,
        "application_operator_requeued",
        serde_json::json!({
            "case_id": operation.case_id,
            "operator_id": operator_id,
            "reason": command.reason.clone(),
            "previous_application_status": operation_status.as_str(),
            "previous_error_code": previous_error_code,
            "previous_error_message": previous_error_message,
            "previous_case_status": previous_case_status.as_str(),
            "next_attempt_at": now,
            "case_revision": case_revision,
        }),
    )
    .await?;
    append_event(
        &receipt.transaction,
        tenant_id,
        "case",
        operation.case_id,
        "case_application_requeued",
        serde_json::json!({
            "decision_id": command.decision_id,
            "operator_id": operator_id,
            "reason": command.reason,
            "previous_application_status": operation_status.as_str(),
            "previous_case_status": previous_case_status.as_str(),
            "case_revision": case_revision,
        }),
    )
    .await?;

    Ok(ModerationApplicationRecoveryRecord {
        decision_id: command.decision_id,
        case_id: operation.case_id,
        operation_status: ModerationApplicationOperationStatus::Retryable,
        case_status: ModerationCaseStatus::ApplyingDecision,
        case_revision,
        changed: true,
    })
}

async fn reconcile_legacy_application_in_transaction(
    receipt: &NewModerationReceipt,
    tenant_id: Uuid,
    operator_id: Uuid,
    command: ReconcileLegacyModerationApplicationCommand,
) -> ModerationResult<ModerationApplicationRecoveryRecord> {
    let operation = find_operation(&receipt.transaction, tenant_id, command.decision_id).await?;
    let operation_status = parse_operation_status(operation.status.as_str())?;
    if !operation_status.is_terminal() {
        return Err(ModerationError::Validation(
            "legacy reconciliation requires a terminal applied, rejected, or operator_review application"
                .to_string(),
        ));
    }
    validate_terminal_operation_shape(&operation, operation_status)?;

    let case = find_case(&receipt.transaction, tenant_id, operation.case_id).await?;
    validate_recovery_identity(&receipt.transaction, tenant_id, &operation, &case).await?;
    require_expected_case_revision(&case, command.expected_case_revision)?;
    let previous_case_status = parse_case_status(case.status.as_str())?;
    let target_case_status = terminal_case_status(operation_status).ok_or_else(|| {
        ModerationError::Invariant(
            "terminal moderation application has no recovery case state".to_string(),
        )
    })?;

    if previous_case_status == target_case_status {
        validate_consistent_terminal_case(&case, target_case_status)?;
        return Ok(ModerationApplicationRecoveryRecord {
            decision_id: command.decision_id,
            case_id: operation.case_id,
            operation_status,
            case_status: target_case_status,
            case_revision: case.revision,
            changed: false,
        });
    }
    if !matches!(
        previous_case_status,
        ModerationCaseStatus::Decided | ModerationCaseStatus::ApplyingDecision
    ) {
        return Err(ModerationError::LifecycleConflict {
            from: case.status,
            to: target_case_status.as_str().to_string(),
        });
    }

    let now = Utc::now().fixed_offset();
    let case_revision = transition_case_status_for_recovery(
        &receipt.transaction,
        tenant_id,
        &case,
        previous_case_status,
        target_case_status,
        now,
    )
    .await?;

    append_event(
        &receipt.transaction,
        tenant_id,
        "application",
        command.decision_id,
        "application_legacy_terminal_reconciled",
        serde_json::json!({
            "case_id": operation.case_id,
            "operator_id": operator_id,
            "reason": command.reason.clone(),
            "terminal_application_status": operation_status.as_str(),
            "stored_applied_revision": operation.applied_revision,
            "stored_applied_at": operation.applied_at.clone(),
            "previous_case_status": previous_case_status.as_str(),
            "reconciled_case_status": target_case_status.as_str(),
            "case_revision": case_revision,
        }),
    )
    .await?;
    append_event(
        &receipt.transaction,
        tenant_id,
        "case",
        operation.case_id,
        "case_legacy_terminal_reconciled",
        serde_json::json!({
            "decision_id": command.decision_id,
            "operator_id": operator_id,
            "reason": command.reason,
            "terminal_application_status": operation_status.as_str(),
            "previous_case_status": previous_case_status.as_str(),
            "reconciled_case_status": target_case_status.as_str(),
            "case_revision": case_revision,
        }),
    )
    .await?;

    Ok(ModerationApplicationRecoveryRecord {
        decision_id: command.decision_id,
        case_id: operation.case_id,
        operation_status,
        case_status: target_case_status,
        case_revision,
        changed: true,
    })
}

async fn find_operation(
    transaction: &DatabaseTransaction,
    tenant_id: Uuid,
    decision_id: Uuid,
) -> ModerationResult<moderation_application_operation::Model> {
    moderation_application_operation::Entity::find_by_id(decision_id)
        .filter(moderation_application_operation::Column::TenantId.eq(tenant_id))
        .one(transaction)
        .await?
        .ok_or(ModerationError::ApplicationOperationNotFound(decision_id))
}

async fn validate_recovery_identity(
    transaction: &DatabaseTransaction,
    tenant_id: Uuid,
    operation: &moderation_application_operation::Model,
    case: &moderation_case::Model,
) -> ModerationResult<()> {
    let decision = moderation_decision::Entity::find_by_id(operation.decision_id)
        .filter(moderation_decision::Column::TenantId.eq(tenant_id))
        .one(transaction)
        .await?
        .ok_or(ModerationError::DecisionNotFound(operation.decision_id))?;
    if decision.case_id != operation.case_id
        || decision.decision_hash != operation.decision_hash
        || decision.subject_revision != operation.subject_revision
        || case.id != operation.case_id
        || case.subject_module != operation.subject_module
        || case.subject_kind != operation.subject_kind
        || case.subject_id != operation.subject_id
        || case.subject_revision != operation.subject_revision
    {
        return Err(ModerationError::Invariant(
            "application recovery identity does not match immutable decision and case facts"
                .to_string(),
        ));
    }
    Ok(())
}

fn validate_terminal_operation_shape(
    operation: &moderation_application_operation::Model,
    status: ModerationApplicationOperationStatus,
) -> ModerationResult<()> {
    if operation.lease_token.is_some()
        || operation.lease_owner.is_some()
        || operation.lease_expires_at.is_some()
    {
        return Err(ModerationError::Invariant(
            "terminal moderation application must not retain a live lease tuple".to_string(),
        ));
    }
    match status {
        ModerationApplicationOperationStatus::Applied => {
            let applied_revision = operation.applied_revision.ok_or_else(|| {
                ModerationError::Invariant(
                    "applied moderation application is missing applied_revision".to_string(),
                )
            })?;
            if applied_revision < operation.subject_revision || operation.applied_at.is_none() {
                return Err(ModerationError::Invariant(
                    "applied moderation application has invalid stored evidence".to_string(),
                ));
            }
        }
        ModerationApplicationOperationStatus::Rejected
        | ModerationApplicationOperationStatus::OperatorReview => {
            if operation.applied_revision.is_some() || operation.applied_at.is_some() {
                return Err(ModerationError::Invariant(
                    "non-applied terminal moderation application must not retain applied evidence"
                        .to_string(),
                ));
            }
        }
        _ => {
            return Err(ModerationError::Validation(
                "application recovery requires a terminal application state".to_string(),
            ));
        }
    }
    Ok(())
}

async fn transition_case_status_for_recovery(
    transaction: &DatabaseTransaction,
    tenant_id: Uuid,
    case: &moderation_case::Model,
    from: ModerationCaseStatus,
    to: ModerationCaseStatus,
    now: chrono::DateTime<chrono::FixedOffset>,
) -> ModerationResult<i64> {
    let stored_status = parse_case_status(case.status.as_str())?;
    if stored_status != from {
        return Err(ModerationError::LifecycleConflict {
            from: case.status.clone(),
            to: to.as_str().to_string(),
        });
    }
    let next_revision = case
        .revision
        .checked_add(1)
        .ok_or(ModerationError::RevisionConflict)?;
    let mut update = moderation_case::Entity::update_many()
        .col_expr(moderation_case::Column::Status, Expr::value(to.as_str()))
        .col_expr(
            moderation_case::Column::Revision,
            Expr::value(next_revision),
        )
        .col_expr(moderation_case::Column::UpdatedAt, Expr::value(now))
        .filter(moderation_case::Column::TenantId.eq(tenant_id))
        .filter(moderation_case::Column::Id.eq(case.id))
        .filter(moderation_case::Column::Revision.eq(case.revision))
        .filter(moderation_case::Column::Status.eq(from.as_str()));
    if to == ModerationCaseStatus::Closed {
        update = update
            .col_expr(moderation_case::Column::ClosedAt, Expr::value(Some(now)))
            .col_expr(
                moderation_case::Column::ActiveDeduplicationKey,
                Expr::value(Option::<String>::None),
            );
    }
    let updated = update.exec(transaction).await?;
    if updated.rows_affected != 1 {
        return Err(ModerationError::RevisionConflict);
    }
    Ok(next_revision)
}

fn require_human_operator(context: &PortContext) -> ModerationResult<Uuid> {
    if context.actor.kind != PortActorKind::User {
        return Err(ModerationError::Validation(
            "moderation application recovery requires a human user actor".to_string(),
        ));
    }
    actor_uuid(context)
}

fn validate_expected_case_revision(revision: i64) -> ModerationResult<()> {
    if revision < 1 {
        return Err(ModerationError::Validation(
            "expected_case_revision must be at least 1".to_string(),
        ));
    }
    Ok(())
}

fn require_expected_case_revision(
    case: &moderation_case::Model,
    expected_revision: i64,
) -> ModerationResult<()> {
    if case.revision != expected_revision {
        return Err(ModerationError::RevisionConflict);
    }
    Ok(())
}

fn normalize_recovery_reason(value: String) -> ModerationResult<String> {
    let value = value.trim().to_string();
    if value.is_empty() || value.len() > MAX_APPLICATION_RECOVERY_REASON_BYTES {
        return Err(ModerationError::Validation(format!(
            "reason must contain 1 to {MAX_APPLICATION_RECOVERY_REASON_BYTES} bytes"
        )));
    }
    Ok(value)
}

fn parse_operation_status(value: &str) -> ModerationResult<ModerationApplicationOperationStatus> {
    ModerationApplicationOperationStatus::parse(value)
        .ok_or_else(|| ModerationError::Invariant("unknown stored application status".to_string()))
}

fn parse_case_status(value: &str) -> ModerationResult<ModerationCaseStatus> {
    ModerationCaseStatus::parse(value)
        .ok_or_else(|| ModerationError::Invariant("unknown stored case status".to_string()))
}

fn terminal_case_status(
    status: ModerationApplicationOperationStatus,
) -> Option<ModerationCaseStatus> {
    match status {
        ModerationApplicationOperationStatus::Applied => Some(ModerationCaseStatus::Closed),
        ModerationApplicationOperationStatus::Rejected
        | ModerationApplicationOperationStatus::OperatorReview => {
            Some(ModerationCaseStatus::Escalated)
        }
        ModerationApplicationOperationStatus::Pending
        | ModerationApplicationOperationStatus::Applying
        | ModerationApplicationOperationStatus::Retryable => None,
    }
}

fn validate_consistent_terminal_case(
    case: &moderation_case::Model,
    target: ModerationCaseStatus,
) -> ModerationResult<()> {
    if target == ModerationCaseStatus::Closed
        && (case.closed_at.is_none() || case.active_deduplication_key.is_some())
    {
        return Err(ModerationError::Invariant(
            "closed moderation case has inconsistent terminal metadata".to_string(),
        ));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn terminal_recovery_mapping_never_requeues_applied() {
        assert_eq!(
            terminal_case_status(ModerationApplicationOperationStatus::Applied),
            Some(ModerationCaseStatus::Closed)
        );
        assert_eq!(
            terminal_case_status(ModerationApplicationOperationStatus::Rejected),
            Some(ModerationCaseStatus::Escalated)
        );
        assert_eq!(
            terminal_case_status(ModerationApplicationOperationStatus::OperatorReview),
            Some(ModerationCaseStatus::Escalated)
        );
        assert_eq!(
            terminal_case_status(ModerationApplicationOperationStatus::Retryable),
            None
        );
    }
}
