use chrono::{Duration, Utc};
use sea_orm::{
    ActiveModelTrait, ColumnTrait, Condition, DatabaseTransaction, EntityTrait, QueryFilter,
    QueryOrder, QuerySelect, Set, sea_query::Expr,
};
use uuid::Uuid;

use crate::domain::{
    ModerationApplicationOperationRecord, ModerationApplicationOperationStatus,
    ModerationDecisionApplication, ModerationSubjectKind,
};
use crate::entities::{moderation_application_operation, moderation_case, moderation_decision};
use crate::error::{ModerationError, ModerationResult};
use crate::service::ModerationService;

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
        moderation_application_operation::Entity::find_by_id(decision_id)
            .filter(moderation_application_operation::Column::TenantId.eq(tenant_id))
            .one(self.database())
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
        let lease_owner = normalize_text(
            lease_owner.into(),
            "lease_owner",
            MAX_LEASE_OWNER_BYTES,
        )?;
        let lease_seconds = normalize_seconds(
            lease_seconds,
            DEFAULT_APPLICATION_LEASE_SECONDS,
            MAX_APPLICATION_LEASE_SECONDS,
            "lease_seconds",
        )?;
        let now = Utc::now().fixed_offset();
        let lease_expires_at = now + Duration::seconds(lease_seconds);
        let lease_token = Uuid::new_v4();
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
                Expr::value(Some(lease_owner)),
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
                Expr::current_timestamp().into(),
            )
            .filter(moderation_application_operation::Column::TenantId.eq(tenant_id))
            .filter(moderation_application_operation::Column::DecisionId.eq(decision_id))
            .filter(due_condition(now))
            .exec(self.database())
            .await?;
        if result.rows_affected == 0 {
            return Ok(None);
        }
        self.get_application_operation(tenant_id, decision_id).await
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
        let current = moderation_application_operation::Entity::find_by_id(decision_id)
            .filter(moderation_application_operation::Column::TenantId.eq(tenant_id))
            .one(self.database())
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
                Expr::current_timestamp().into(),
            )
            .filter(moderation_application_operation::Column::TenantId.eq(tenant_id))
            .filter(moderation_application_operation::Column::DecisionId.eq(decision_id))
            .filter(
                moderation_application_operation::Column::Status
                    .eq(ModerationApplicationOperationStatus::Applying.as_str()),
            )
            .filter(moderation_application_operation::Column::LeaseToken.eq(lease_token))
            .filter(moderation_application_operation::Column::LeaseExpiresAt.gt(now))
            .exec(self.database())
            .await?;
        if result.rows_affected != 1 {
            return Err(self.application_cas_conflict(tenant_id, decision_id).await?);
        }
        self.get_application_operation(tenant_id, decision_id)
            .await?
            .ok_or(ModerationError::ApplicationOperationNotFound(decision_id))
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
        let error_message = normalize_text(error_message, "error_message", MAX_ERROR_MESSAGE_BYTES)?;
        let now = Utc::now().fixed_offset();
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
                Expr::value(Some(error_code)),
            )
            .col_expr(
                moderation_application_operation::Column::LastErrorMessage,
                Expr::value(Some(error_message)),
            )
            .col_expr(
                moderation_application_operation::Column::UpdatedAt,
                Expr::current_timestamp().into(),
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
        let result = update.exec(self.database()).await?;
        if result.rows_affected != 1 {
            return Err(self.application_cas_conflict(tenant_id, decision_id).await?);
        }
        self.get_application_operation(tenant_id, decision_id)
            .await?
            .ok_or(ModerationError::ApplicationOperationNotFound(decision_id))
    }

    async fn application_cas_conflict(
        &self,
        tenant_id: Uuid,
        decision_id: Uuid,
    ) -> ModerationResult<ModerationError> {
        if moderation_application_operation::Entity::find_by_id(decision_id)
            .filter(moderation_application_operation::Column::TenantId.eq(tenant_id))
            .one(self.database())
            .await?
            .is_some()
        {
            Ok(ModerationError::ApplicationLeaseConflict(decision_id))
        } else {
            Ok(ModerationError::ApplicationOperationNotFound(decision_id))
        }
    }
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
    let stored_kind = ModerationSubjectKind::parse(operation.subject_kind.as_str()).ok_or_else(|| {
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
    let subject_kind = ModerationSubjectKind::parse(model.subject_kind.as_str()).ok_or_else(|| {
        ModerationError::Invariant("unknown stored application subject kind".to_string())
    })?;
    let status = ModerationApplicationOperationStatus::parse(model.status.as_str()).ok_or_else(|| {
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
        lease_expires_at: model.lease_expires_at.map(|value| value.with_timezone(&Utc)),
        last_error_code: model.last_error_code,
        last_error_message: model.last_error_message,
        applied_revision: model.applied_revision,
        applied_at: model.applied_at.map(|value| value.with_timezone(&Utc)),
        created_at: model.created_at.with_timezone(&Utc),
        updated_at: model.updated_at.with_timezone(&Utc),
    })
}

fn normalize_seconds(
    value: i64,
    default: i64,
    max: i64,
    field: &str,
) -> ModerationResult<i64> {
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
