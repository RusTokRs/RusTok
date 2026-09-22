use chrono::{Duration, Utc};
use sea_orm::{
    ActiveModelTrait, ColumnTrait, Condition, ConnectionTrait, DatabaseTransaction, EntityTrait,
    QueryFilter, QueryOrder, QuerySelect, Set, TransactionTrait,
    sea_query::{Expr, ExprTrait},
};
use uuid::Uuid;

use crate::domain::{
    ModerationApplicationOperationRecord, ModerationApplicationOperationStatus,
    ModerationCaseStatus, ModerationDecisionApplication, ModerationSubjectKind,
};
use crate::entities::{moderation_application_operation, moderation_case, moderation_decision};
use crate::error::{ModerationError, ModerationResult};
use crate::service::{ModerationService, append_event, find_case};

pub const DEFAULT_APPLICATION_LEASE_SECONDS: i64 = 60;
pub const MAX_APPLICATION_LEASE_SECONDS: i64 = 900;
pub const MAX_APPLICATION_RETRY_SECONDS: i64 = 86_400;
pub const MAX_DUE_APPLICATION_OPERATIONS: u32 = 100;
const MAX_LEASE_OWNER_BYTES: usize = 120;
const MAX_ERROR_CODE_BYTES: usize = 120;
const MAX_ERROR_MESSAGE_BYTES: usize = 2_000;

pub(crate) async fn enqueue_application_operation_in_transaction(
    transaction: &DatabaseTransaction,
    tenant_id: Uuid,
    case: &moderation_case::Model,
    decision: &moderation_decision::Model,
) -> ModerationResult<()> {
    if case.tenant_id != tenant_id
        || decision.tenant_id != tenant_id
        || decision.case_id != case.id
        || decision.subject_revision != case.subject_revision
    {
        return Err(ModerationError::Invariant(
            "decision application identity does not match the decided case".to_string(),
        ));
    }
    if decision.decision_hash.len() != 64
        || !decision
            .decision_hash
            .bytes()
            .all(|byte| byte.is_ascii_hexdigit() && !byte.is_ascii_uppercase())
    {
        return Err(ModerationError::Invariant(
            "decision application requires the canonical lowercase SHA-256 decision hash"
                .to_string(),
        ));
    }

    let now = Utc::now();
    moderation_application_operation::ActiveModel {
        decision_id: Set(decision.id),
        tenant_id: Set(tenant_id),
        case_id: Set(case.id),
        decision_hash: Set(decision.decision_hash.clone()),
        subject_module: Set(case.subject_module.clone()),
        subject_kind: Set(case.subject_kind.clone()),
        subject_id: Set(case.subject_id),
        subject_revision: Set(case.subject_revision),
        status: Set(ModerationApplicationOperationStatus::Pending
            .as_str()
            .to_string()),
        attempt_count: Set(0),
        next_attempt_at: Set(now.into()),
        lease_token: Set(None),
        lease_owner: Set(None),
        lease_expires_at: Set(None),
        last_error_code: Set(None),
        last_error_message: Set(None),
        applied_revision: Set(None),
        applied_at: Set(None),
        created_at: Set(now.into()),
        updated_at: Set(now.into()),
    }
    .insert(transaction)
    .await?;
    Ok(())
}

impl ModerationService {
    pub async fn get_application_operation(
        &self,
        tenant_id: Uuid,
        decision_id: Uuid,
    ) -> ModerationResult<Option<ModerationApplicationOperationRecord>> {
        find_application_operation_model(self.database(), tenant_id, decision_id)
            .await?
            .map(map_application_operation)
            .transpose()
    }

    pub async fn list_due_application_operations(
        &self,
        tenant_id: Uuid,
        limit: u32,
    ) -> ModerationResult<Vec<ModerationApplicationOperationRecord>> {
        let now = Utc::now().fixed_offset();
        let due = due_condition(now);
        let models = moderation_application_operation::Entity::find()
            .filter(moderation_application_operation::Column::TenantId.eq(tenant_id))
            .filter(due)
            .order_by_asc(moderation_application_operation::Column::NextAttemptAt)
            .order_by_asc(moderation_application_operation::Column::CreatedAt)
            .limit(limit.clamp(1, MAX_DUE_APPLICATION_OPERATIONS) as u64)
            .all(self.database())
            .await?;
        models.into_iter().map(map_application_operation).collect()
    }

    pub async fn claim_application_operation(
        &self,
        tenant_id: Uuid,
        decision_id: Uuid,
        lease_owner: impl Into<String>,
        lease_seconds: i64,
    ) -> ModerationResult<Option<ModerationApplicationOperationRecord>> {
        let lease_owner = normalize_text(lease_owner.into(), "lease_owner", MAX_LEASE_OWNER_BYTES)?;
        let lease_seconds = normalize_seconds(
            lease_seconds,
            DEFAULT_APPLICATION_LEASE_SECONDS,
            MAX_APPLICATION_LEASE_SECONDS,
            "lease_seconds",
        )?;
        let now = Utc::now().fixed_offset();
        let lease_expires_at = now + Duration::seconds(lease_seconds);
        let lease_token = Uuid::new_v4();
        let transaction = self.database().begin().await?;
        let result = moderation_application_operation::Entity::update_many()
            .col_expr(
                moderation_application_operation::Column::Status,
                Expr::value(ModerationApplicationOperationStatus::Applying.as_str()),
            )
            .col_expr(
                moderation_application_operation::Column::AttemptCount,
                Expr::col(moderation_application_operation::Column::AttemptCount).add(1),
            )
            .col_expr(
                moderation_application_operation::Column::LeaseToken,
                Expr::value(Some(lease_token)),
            )
            .col_expr(
                moderation_application_operation::Column::LeaseOwner,
                Expr::value(Some(lease_owner.clone())),
            )
            .col_expr(
                moderation_application_operation::Column::LeaseExpiresAt,
                Expr::value(Some(lease_expires_at)),
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
            .filter(moderation_application_operation::Column::DecisionId.eq(decision_id))
            .filter(due_condition(now))
            .exec(&transaction)
            .await?;
        if result.rows_affected == 0 {
            transaction.rollback().await?;
            return Ok(None);
        }

        let operation = find_application_operation_model(&transaction, tenant_id, decision_id)
            .await?
            .ok_or(ModerationError::ApplicationOperationNotFound(decision_id))?;
        let case = find_case(&transaction, tenant_id, operation.case_id).await?;
        let case_status = parse_case_status(case.status.as_str())?;
        let case_revision = match case_status {
            ModerationCaseStatus::Decided => {
                let revision = transition_case_status_in_transaction(
                    &transaction,
                    tenant_id,
                    &case,
                    ModerationCaseStatus::Decided,
                    ModerationCaseStatus::ApplyingDecision,
                    now,
                )
                .await?;
                append_event(
                    &transaction,
                    tenant_id,
                    "case",
                    case.id,
                    "case_application_started",
                    serde_json::json!({
                        "decision_id": decision_id,
                        "attempt_count": operation.attempt_count,
                        "revision": revision,
                    }),
                )
                .await?;
                revision
            }
            ModerationCaseStatus::ApplyingDecision => case.revision,
            _ => {
                transaction.rollback().await?;
                return Err(ModerationError::LifecycleConflict {
                    from: case.status,
                    to: ModerationCaseStatus::ApplyingDecision.as_str().to_string(),
                });
            }
        };

        append_event(
            &transaction,
            tenant_id,
            "application",
            decision_id,
            "application_attempt_claimed",
            serde_json::json!({
                "case_id": operation.case_id,
                "attempt_count": operation.attempt_count,
                "lease_owner": lease_owner,
                "lease_expires_at": operation.lease_expires_at.clone(),
                "case_revision": case_revision,
            }),
        )
        .await?;

        let operation = map_application_operation(operation)?;
        transaction.commit().await?;
        Ok(Some(operation))
    }

    pub async fn mark_application_retryable(
        &self,
        tenant_id: Uuid,
        decision_id: Uuid,
        lease_token: Uuid,
        error_code: impl Into<String>,
        error_message: impl Into<String>,
        retry_after_seconds: i64,
    ) -> ModerationResult<ModerationApplicationOperationRecord> {
        let retry_after_seconds = normalize_seconds(
            retry_after_seconds,
            1,
            MAX_APPLICATION_RETRY_SECONDS,
            "retry_after_seconds",
        )?;
        let now = Utc::now().fixed_offset();
        let next_attempt_at = now + Duration::seconds(retry_after_seconds);
        self.finish_with_error(
            tenant_id,
            decision_id,
            lease_token,
            ModerationApplicationOperationStatus::Retryable,
            error_code.into(),
            error_message.into(),
            Some(next_attempt_at),
        )
        .await
    }

    pub async fn mark_application_rejected(
        &self,
        tenant_id: Uuid,
        decision_id: Uuid,
        lease_token: Uuid,
        error_code: impl Into<String>,
        error_message: impl Into<String>,
    ) -> ModerationResult<ModerationApplicationOperationRecord> {
        self.finish_with_error(
            tenant_id,
            decision_id,
            lease_token,
            ModerationApplicationOperationStatus::Rejected,
            error_code.into(),
            error_message.into(),
            None,
        )
        .await
    }

    pub async fn mark_application_operator_review(
        &self,
        tenant_id: Uuid,
        decision_id: Uuid,
        lease_token: Uuid,
        error_code: impl Into<String>,
        error_message: impl Into<String>,
    ) -> ModerationResult<ModerationApplicationOperationRecord> {
        self.finish_with_error(
            tenant_id,
            decision_id,
            lease_token,
            ModerationApplicationOperationStatus::OperatorReview,
            error_code.into(),
            error_message.into(),
            None,
        )
        .await
    }

    pub async fn mark_application_applied(
        &self,
        tenant_id: Uuid,
        decision_id: Uuid,
        lease_token: Uuid,
        application: ModerationDecisionApplication,
    ) -> ModerationResult<ModerationApplicationOperationRecord> {
        let transaction = self.database().begin().await?;
        let current = find_application_operation_model(&transaction, tenant_id, decision_id)
            .await?
            .ok_or(ModerationError::ApplicationOperationNotFound(decision_id))?;
        validate_application_evidence(&current, &application)?;

        let now = Utc::now().fixed_offset();
        let result = moderation_application_operation::Entity::update_many()
            .col_expr(
                moderation_application_operation::Column::Status,
                Expr::value(ModerationApplicationOperationStatus::Applied.as_str()),
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
                moderation_application_operation::Column::AppliedRevision,
                Expr::value(Some(application.applied_revision)),
            )
            .col_expr(
                moderation_application_operation::Column::AppliedAt,
                Expr::value(Some(application.applied_at.fixed_offset())),
            )
            .col_expr(
                moderation_application_operation::Column::UpdatedAt,
                Expr::value(now),
            )
            .filter(moderation_application_operation::Column::TenantId.eq(tenant_id))
            .filter(moderation_application_operation::Column::DecisionId.eq(decision_id))
            .filter(
                moderation_application_operation::Column::Status
                    .eq(ModerationApplicationOperationStatus::Applying.as_str()),
            )
            .filter(moderation_application_operation::Column::LeaseToken.eq(lease_token))
            .filter(moderation_application_operation::Column::LeaseExpiresAt.gt(now))
            .exec(&transaction)
            .await?;
        if result.rows_affected != 1 {
            transaction.rollback().await?;
            return Err(self
                .application_cas_conflict(tenant_id, decision_id)
                .await?);
        }

        let operation = find_application_operation_model(&transaction, tenant_id, decision_id)
            .await?
            .ok_or(ModerationError::ApplicationOperationNotFound(decision_id))?;
        let case = find_case(&transaction, tenant_id, operation.case_id).await?;
        let case_target = terminal_case_status(ModerationApplicationOperationStatus::Applied)
            .ok_or_else(|| {
                ModerationError::Invariant(
                    "applied moderation operation has no terminal case state".to_string(),
                )
            })?;
        let case_revision = transition_case_status_in_transaction(
            &transaction,
            tenant_id,
            &case,
            ModerationCaseStatus::ApplyingDecision,
            case_target,
            now,
        )
        .await?;

        append_event(
            &transaction,
            tenant_id,
            "application",
            decision_id,
            "application_applied",
            serde_json::json!({
                "case_id": operation.case_id,
                "attempt_count": operation.attempt_count,
                "applied_revision": application.applied_revision,
                "applied_at": application.applied_at,
            }),
        )
        .await?;
        append_event(
            &transaction,
            tenant_id,
            "case",
            operation.case_id,
            "case_closed",
            serde_json::json!({
                "decision_id": decision_id,
                "application_status": ModerationApplicationOperationStatus::Applied.as_str(),
                "applied_revision": application.applied_revision,
                "revision": case_revision,
            }),
        )
        .await?;

        let operation = map_application_operation(operation)?;
        transaction.commit().await?;
        Ok(operation)
    }

    async fn finish_with_error(
        &self,
        tenant_id: Uuid,
        decision_id: Uuid,
        lease_token: Uuid,
        next_status: ModerationApplicationOperationStatus,
        error_code: String,
        error_message: String,
        next_attempt_at: Option<chrono::DateTime<chrono::FixedOffset>>,
    ) -> ModerationResult<ModerationApplicationOperationRecord> {
        let error_code = normalize_text(error_code, "error_code", MAX_ERROR_CODE_BYTES)?;
        let error_message =
            normalize_text(error_message, "error_message", MAX_ERROR_MESSAGE_BYTES)?;
        match next_status {
            ModerationApplicationOperationStatus::Retryable if next_attempt_at.is_none() => {
                return Err(ModerationError::Invariant(
                    "retryable moderation application requires next_attempt_at".to_string(),
                ));
            }
            ModerationApplicationOperationStatus::Rejected
            | ModerationApplicationOperationStatus::OperatorReview
                if next_attempt_at.is_some() =>
            {
                return Err(ModerationError::Invariant(
                    "terminal moderation application must not schedule a retry".to_string(),
                ));
            }
            ModerationApplicationOperationStatus::Retryable
            | ModerationApplicationOperationStatus::Rejected
            | ModerationApplicationOperationStatus::OperatorReview => {}
            _ => {
                return Err(ModerationError::Invariant(
                    "invalid moderation application error transition".to_string(),
                ));
            }
        }

        let now = Utc::now().fixed_offset();
        let transaction = self.database().begin().await?;
        let mut update = moderation_application_operation::Entity::update_many()
            .col_expr(
                moderation_application_operation::Column::Status,
                Expr::value(next_status.as_str()),
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
                Expr::value(Some(error_code.clone())),
            )
            .col_expr(
                moderation_application_operation::Column::LastErrorMessage,
                Expr::value(Some(error_message)),
            )
            .col_expr(
                moderation_application_operation::Column::UpdatedAt,
                Expr::value(now),
            )
            .filter(moderation_application_operation::Column::TenantId.eq(tenant_id))
            .filter(moderation_application_operation::Column::DecisionId.eq(decision_id))
            .filter(
                moderation_application_operation::Column::Status
                    .eq(ModerationApplicationOperationStatus::Applying.as_str()),
            )
            .filter(moderation_application_operation::Column::LeaseToken.eq(lease_token))
            .filter(moderation_application_operation::Column::LeaseExpiresAt.gt(now));
        if let Some(next_attempt_at) = next_attempt_at {
            update = update.col_expr(
                moderation_application_operation::Column::NextAttemptAt,
                Expr::value(next_attempt_at),
            );
        }
        let result = update.exec(&transaction).await?;
        if result.rows_affected != 1 {
            transaction.rollback().await?;
            return Err(self
                .application_cas_conflict(tenant_id, decision_id)
                .await?);
        }

        let operation = find_application_operation_model(&transaction, tenant_id, decision_id)
            .await?
            .ok_or(ModerationError::ApplicationOperationNotFound(decision_id))?;
        let case = find_case(&transaction, tenant_id, operation.case_id).await?;
        match next_status {
            ModerationApplicationOperationStatus::Retryable => {
                require_case_status(&case, ModerationCaseStatus::ApplyingDecision)?;
                append_event(
                    &transaction,
                    tenant_id,
                    "application",
                    decision_id,
                    "application_retry_scheduled",
                    serde_json::json!({
                        "case_id": operation.case_id,
                        "attempt_count": operation.attempt_count,
                        "error_code": error_code,
                        "next_attempt_at": operation.next_attempt_at.clone(),
                    }),
                )
                .await?;
            }
            ModerationApplicationOperationStatus::Rejected
            | ModerationApplicationOperationStatus::OperatorReview => {
                let case_target = terminal_case_status(next_status).ok_or_else(|| {
                    ModerationError::Invariant(
                        "terminal moderation application has no terminal case state".to_string(),
                    )
                })?;
                let case_revision = transition_case_status_in_transaction(
                    &transaction,
                    tenant_id,
                    &case,
                    ModerationCaseStatus::ApplyingDecision,
                    case_target,
                    now,
                )
                .await?;
                let application_event_type =
                    if next_status == ModerationApplicationOperationStatus::Rejected {
                        "application_rejected"
                    } else {
                        "application_operator_review"
                    };
                append_event(
                    &transaction,
                    tenant_id,
                    "application",
                    decision_id,
                    application_event_type,
                    serde_json::json!({
                        "case_id": operation.case_id,
                        "attempt_count": operation.attempt_count,
                        "error_code": error_code,
                    }),
                )
                .await?;
                append_event(
                    &transaction,
                    tenant_id,
                    "case",
                    operation.case_id,
                    "case_escalated",
                    serde_json::json!({
                        "decision_id": decision_id,
                        "application_status": next_status.as_str(),
                        "error_code": operation.last_error_code.clone(),
                        "revision": case_revision,
                    }),
                )
                .await?;
            }
            _ => unreachable!("validated application error status"),
        }

        let operation = map_application_operation(operation)?;
        transaction.commit().await?;
        Ok(operation)
    }

    async fn application_cas_conflict(
        &self,
        tenant_id: Uuid,
        decision_id: Uuid,
    ) -> ModerationResult<ModerationError> {
        if find_application_operation_model(self.database(), tenant_id, decision_id)
            .await?
            .is_some()
        {
            Ok(ModerationError::ApplicationLeaseConflict(decision_id))
        } else {
            Ok(ModerationError::ApplicationOperationNotFound(decision_id))
        }
    }
}

async fn find_application_operation_model<C>(
    connection: &C,
    tenant_id: Uuid,
    decision_id: Uuid,
) -> ModerationResult<Option<moderation_application_operation::Model>>
where
    C: ConnectionTrait,
{
    moderation_application_operation::Entity::find_by_id(decision_id)
        .filter(moderation_application_operation::Column::TenantId.eq(tenant_id))
        .one(connection)
        .await
        .map_err(Into::into)
}

async fn transition_case_status_in_transaction(
    transaction: &DatabaseTransaction,
    tenant_id: Uuid,
    case: &moderation_case::Model,
    from: ModerationCaseStatus,
    to: ModerationCaseStatus,
    now: chrono::DateTime<chrono::FixedOffset>,
) -> ModerationResult<i64> {
    require_case_status(case, from)?;
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
    let result = update.exec(transaction).await?;
    if result.rows_affected != 1 {
        return Err(ModerationError::RevisionConflict);
    }
    Ok(next_revision)
}

fn require_case_status(
    case: &moderation_case::Model,
    expected: ModerationCaseStatus,
) -> ModerationResult<()> {
    let actual = parse_case_status(case.status.as_str())?;
    if actual != expected {
        return Err(ModerationError::LifecycleConflict {
            from: case.status.clone(),
            to: expected.as_str().to_string(),
        });
    }
    Ok(())
}

fn parse_case_status(value: &str) -> ModerationResult<ModerationCaseStatus> {
    ModerationCaseStatus::parse(value)
        .ok_or_else(|| ModerationError::Invariant("unknown stored case status".to_string()))
}

fn due_condition(now: chrono::DateTime<chrono::FixedOffset>) -> Condition {
    Condition::any()
        .add(
            Condition::all()
                .add(moderation_application_operation::Column::Status.is_in([
                    ModerationApplicationOperationStatus::Pending.as_str(),
                    ModerationApplicationOperationStatus::Retryable.as_str(),
                ]))
                .add(moderation_application_operation::Column::NextAttemptAt.lte(now)),
        )
        .add(
            Condition::all()
                .add(
                    moderation_application_operation::Column::Status
                        .eq(ModerationApplicationOperationStatus::Applying.as_str()),
                )
                .add(moderation_application_operation::Column::LeaseExpiresAt.lte(now)),
        )
}

fn validate_application_evidence(
    operation: &moderation_application_operation::Model,
    application: &ModerationDecisionApplication,
) -> ModerationResult<()> {
    let stored_kind =
        ModerationSubjectKind::parse(operation.subject_kind.as_str()).ok_or_else(|| {
            ModerationError::Invariant("unknown stored application subject kind".to_string())
        })?;
    if application.decision_id != operation.decision_id
        || application.subject.module != operation.subject_module
        || application.subject.kind != stored_kind
        || application.subject.id != operation.subject_id
        || application.subject.revision != operation.subject_revision
        || application.applied_revision < operation.subject_revision
    {
        return Err(ModerationError::ApplicationEvidenceMismatch(
            operation.decision_id,
        ));
    }
    Ok(())
}

fn map_application_operation(
    model: moderation_application_operation::Model,
) -> ModerationResult<ModerationApplicationOperationRecord> {
    let subject_kind =
        ModerationSubjectKind::parse(model.subject_kind.as_str()).ok_or_else(|| {
            ModerationError::Invariant("unknown stored application subject kind".to_string())
        })?;
    let status =
        ModerationApplicationOperationStatus::parse(model.status.as_str()).ok_or_else(|| {
            ModerationError::Invariant("unknown stored application status".to_string())
        })?;
    Ok(ModerationApplicationOperationRecord {
        decision_id: model.decision_id,
        tenant_id: model.tenant_id,
        case_id: model.case_id,
        decision_hash: model.decision_hash,
        subject: crate::domain::ModerationSubjectRef {
            module: model.subject_module,
            kind: subject_kind,
            id: model.subject_id,
            revision: model.subject_revision,
        },
        status,
        attempt_count: model.attempt_count,
        next_attempt_at: model.next_attempt_at.with_timezone(&Utc),
        lease_token: model.lease_token,
        lease_owner: model.lease_owner,
        lease_expires_at: model
            .lease_expires_at
            .map(|value| value.with_timezone(&Utc)),
        last_error_code: model.last_error_code,
        last_error_message: model.last_error_message,
        applied_revision: model.applied_revision,
        applied_at: model.applied_at.map(|value| value.with_timezone(&Utc)),
        created_at: model.created_at.with_timezone(&Utc),
        updated_at: model.updated_at.with_timezone(&Utc),
    })
}

fn normalize_seconds(value: i64, default: i64, max: i64, field: &str) -> ModerationResult<i64> {
    let value = if value == 0 { default } else { value };
    if !(1..=max).contains(&value) {
        return Err(ModerationError::Validation(format!(
            "{field} must contain 1 to {max} seconds"
        )));
    }
    Ok(value)
}

fn normalize_text(value: String, field: &str, max_bytes: usize) -> ModerationResult<String> {
    let value = value.trim().to_string();
    if value.is_empty() || value.len() > max_bytes {
        return Err(ModerationError::Validation(format!(
            "{field} must contain 1 to {max_bytes} bytes"
        )));
    }
    Ok(value)
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn terminal_application_case_statuses_are_fail_closed() {
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
