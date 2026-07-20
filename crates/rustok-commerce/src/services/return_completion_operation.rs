use chrono::{Duration, Utc};
use sea_orm::{
    ActiveModelTrait, ColumnTrait, Condition, DatabaseConnection, EntityTrait, QueryFilter, Set,
    sea_query::Expr,
};
use serde::{Deserialize, Serialize};
use thiserror::Error;
use uuid::Uuid;

use rustok_core::generate_id;

use crate::entities::return_completion_operation;

pub const DEFAULT_RETURN_COMPLETION_LEASE_SECONDS: i64 = 60;
pub const MAX_RETURN_COMPLETION_LEASE_SECONDS: i64 = 900;

#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq, Eq, Hash)]
#[serde(rename_all = "snake_case")]
pub enum ReturnCompletionOperationStatus {
    Pending,
    Executing,
    RetryableError,
    ReconciliationRequired,
    Completed,
    Failed,
}

impl ReturnCompletionOperationStatus {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Pending => "pending",
            Self::Executing => "executing",
            Self::RetryableError => "retryable_error",
            Self::ReconciliationRequired => "reconciliation_required",
            Self::Completed => "completed",
            Self::Failed => "failed",
        }
    }
}

#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq, Eq, Hash)]
#[serde(rename_all = "snake_case")]
pub enum ReturnCompletionOperationStage {
    Created,
    ResolutionCreated,
    ReturnCompleted,
    Completed,
}

impl ReturnCompletionOperationStage {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Created => "created",
            Self::ResolutionCreated => "resolution_created",
            Self::ReturnCompleted => "return_completed",
            Self::Completed => "completed",
        }
    }

    pub fn from_str(value: &str) -> ReturnCompletionOperationResult<Self> {
        match value {
            "created" => Ok(Self::Created),
            "resolution_created" => Ok(Self::ResolutionCreated),
            "return_completed" => Ok(Self::ReturnCompleted),
            "completed" => Ok(Self::Completed),
            other => Err(ReturnCompletionOperationError::Conflict(format!(
                "unknown return completion stage `{other}`"
            ))),
        }
    }
}

#[derive(Debug, Error)]
pub enum ReturnCompletionOperationError {
    #[error("validation failed: {0}")]
    Validation(String),
    #[error("return completion operation {0} not found")]
    NotFound(Uuid),
    #[error("return completion operation conflict: {0}")]
    Conflict(String),
    #[error(transparent)]
    Database(#[from] sea_orm::DbErr),
}

pub type ReturnCompletionOperationResult<T> = Result<T, ReturnCompletionOperationError>;

#[derive(Clone, Debug)]
pub struct BeginReturnCompletionOperation {
    pub tenant_id: Uuid,
    pub return_id: Uuid,
    pub request_hash: String,
}

#[derive(Clone, Debug)]
pub struct ReturnCompletionOperationCheckpoint {
    pub tenant_id: Uuid,
    pub operation_id: Uuid,
    pub lease_owner: String,
    pub expected_stage: ReturnCompletionOperationStage,
    pub next_stage: ReturnCompletionOperationStage,
    pub refund_id: Option<Uuid>,
    pub order_change_id: Option<Uuid>,
    pub lease_seconds: i64,
}

#[derive(Clone)]
pub struct ReturnCompletionOperationJournal {
    db: DatabaseConnection,
}

impl ReturnCompletionOperationJournal {
    pub fn new(db: DatabaseConnection) -> Self {
        Self { db }
    }

    pub async fn begin(
        &self,
        input: BeginReturnCompletionOperation,
    ) -> ReturnCompletionOperationResult<return_completion_operation::Model> {
        let request_hash = normalize_hash(input.request_hash)?;
        if let Some(existing) = self
            .find_by_return(input.tenant_id, input.return_id)
            .await?
        {
            ensure_same_request(&existing, request_hash.as_str())?;
            return Ok(existing);
        }

        let now = Utc::now();
        let insert = return_completion_operation::ActiveModel {
            id: Set(generate_id()),
            tenant_id: Set(input.tenant_id),
            return_id: Set(input.return_id),
            request_hash: Set(request_hash.clone()),
            status: Set(ReturnCompletionOperationStatus::Pending
                .as_str()
                .to_string()),
            stage: Set(ReturnCompletionOperationStage::Created.as_str().to_string()),
            refund_id: Set(None),
            order_change_id: Set(None),
            attempt_count: Set(0),
            lease_owner: Set(None),
            lease_expires_at: Set(None),
            last_error_code: Set(None),
            last_error_message: Set(None),
            created_at: Set(now.into()),
            updated_at: Set(now.into()),
            completed_at: Set(None),
        }
        .insert(&self.db)
        .await;

        match insert {
            Ok(model) => Ok(model),
            Err(error) if is_unique_constraint(&error) => {
                let existing = self
                    .find_by_return(input.tenant_id, input.return_id)
                    .await?
                    .ok_or(error)?;
                ensure_same_request(&existing, request_hash.as_str())?;
                Ok(existing)
            }
            Err(error) => Err(error.into()),
        }
    }

    pub async fn get(
        &self,
        tenant_id: Uuid,
        operation_id: Uuid,
    ) -> ReturnCompletionOperationResult<return_completion_operation::Model> {
        return_completion_operation::Entity::find_by_id(operation_id)
            .filter(return_completion_operation::Column::TenantId.eq(tenant_id))
            .one(&self.db)
            .await?
            .ok_or(ReturnCompletionOperationError::NotFound(operation_id))
    }

    pub async fn find_by_return(
        &self,
        tenant_id: Uuid,
        return_id: Uuid,
    ) -> ReturnCompletionOperationResult<Option<return_completion_operation::Model>> {
        return_completion_operation::Entity::find()
            .filter(return_completion_operation::Column::TenantId.eq(tenant_id))
            .filter(return_completion_operation::Column::ReturnId.eq(return_id))
            .one(&self.db)
            .await
            .map_err(Into::into)
    }

    pub async fn claim_execution(
        &self,
        tenant_id: Uuid,
        operation_id: Uuid,
        lease_owner: impl Into<String>,
        lease_seconds: i64,
    ) -> ReturnCompletionOperationResult<Option<return_completion_operation::Model>> {
        let lease_owner = normalize_lease_owner(lease_owner.into())?;
        let lease_seconds = normalize_lease_seconds(lease_seconds)?;
        let now = Utc::now().fixed_offset();
        let lease_expires_at = now + Duration::seconds(lease_seconds);
        let claimable = Condition::any()
            .add(return_completion_operation::Column::Status.is_in([
                ReturnCompletionOperationStatus::Pending.as_str(),
                ReturnCompletionOperationStatus::RetryableError.as_str(),
            ]))
            .add(
                Condition::all()
                    .add(
                        return_completion_operation::Column::Status
                            .eq(ReturnCompletionOperationStatus::Executing.as_str()),
                    )
                    .add(return_completion_operation::Column::LeaseExpiresAt.lte(now)),
            );

        let result = return_completion_operation::Entity::update_many()
            .col_expr(
                return_completion_operation::Column::Status,
                Expr::value(ReturnCompletionOperationStatus::Executing.as_str()),
            )
            .col_expr(
                return_completion_operation::Column::LeaseOwner,
                Expr::value(Some(lease_owner)),
            )
            .col_expr(
                return_completion_operation::Column::LeaseExpiresAt,
                Expr::value(Some(lease_expires_at)),
            )
            .col_expr(
                return_completion_operation::Column::AttemptCount,
                Expr::col(return_completion_operation::Column::AttemptCount).add(1),
            )
            .col_expr(
                return_completion_operation::Column::LastErrorCode,
                Expr::value(Option::<String>::None),
            )
            .col_expr(
                return_completion_operation::Column::LastErrorMessage,
                Expr::value(Option::<String>::None),
            )
            .col_expr(
                return_completion_operation::Column::UpdatedAt,
                Expr::current_timestamp().into(),
            )
            .filter(return_completion_operation::Column::TenantId.eq(tenant_id))
            .filter(return_completion_operation::Column::Id.eq(operation_id))
            .filter(claimable)
            .exec(&self.db)
            .await?;

        if result.rows_affected == 0 {
            return Ok(None);
        }
        self.get(tenant_id, operation_id).await.map(Some)
    }

    pub async fn checkpoint(
        &self,
        input: ReturnCompletionOperationCheckpoint,
    ) -> ReturnCompletionOperationResult<return_completion_operation::Model> {
        let lease_owner = normalize_lease_owner(input.lease_owner)?;
        let lease_seconds = normalize_lease_seconds(input.lease_seconds)?;
        let now = Utc::now().fixed_offset();
        let lease_expires_at = now + Duration::seconds(lease_seconds);

        let mut update = return_completion_operation::Entity::update_many()
            .col_expr(
                return_completion_operation::Column::Stage,
                Expr::value(input.next_stage.as_str()),
            )
            .col_expr(
                return_completion_operation::Column::LeaseExpiresAt,
                Expr::value(Some(lease_expires_at)),
            )
            .col_expr(
                return_completion_operation::Column::UpdatedAt,
                Expr::current_timestamp().into(),
            )
            .filter(return_completion_operation::Column::TenantId.eq(input.tenant_id))
            .filter(return_completion_operation::Column::Id.eq(input.operation_id))
            .filter(
                return_completion_operation::Column::Status
                    .eq(ReturnCompletionOperationStatus::Executing.as_str()),
            )
            .filter(return_completion_operation::Column::Stage.eq(input.expected_stage.as_str()))
            .filter(return_completion_operation::Column::LeaseOwner.eq(lease_owner))
            .filter(return_completion_operation::Column::LeaseExpiresAt.gt(now));

        if let Some(refund_id) = input.refund_id {
            update = update.col_expr(
                return_completion_operation::Column::RefundId,
                Expr::value(Some(refund_id)),
            );
        }
        if let Some(order_change_id) = input.order_change_id {
            update = update.col_expr(
                return_completion_operation::Column::OrderChangeId,
                Expr::value(Some(order_change_id)),
            );
        }

        let result = update.exec(&self.db).await?;
        if result.rows_affected == 0 {
            return Err(self
                .cas_conflict(input.tenant_id, input.operation_id, "checkpoint")
                .await?);
        }
        self.get(input.tenant_id, input.operation_id).await
    }

    pub async fn mark_retryable(
        &self,
        tenant_id: Uuid,
        operation_id: Uuid,
        lease_owner: impl Into<String>,
        error_code: impl Into<String>,
        error_message: impl Into<String>,
    ) -> ReturnCompletionOperationResult<return_completion_operation::Model> {
        self.finish_execution(
            tenant_id,
            operation_id,
            lease_owner.into(),
            ReturnCompletionOperationStatus::RetryableError,
            error_code.into(),
            error_message.into(),
            false,
        )
        .await
    }

    pub async fn mark_reconciliation_required(
        &self,
        tenant_id: Uuid,
        operation_id: Uuid,
        lease_owner: impl Into<String>,
        error_code: impl Into<String>,
        error_message: impl Into<String>,
    ) -> ReturnCompletionOperationResult<return_completion_operation::Model> {
        self.finish_execution(
            tenant_id,
            operation_id,
            lease_owner.into(),
            ReturnCompletionOperationStatus::ReconciliationRequired,
            error_code.into(),
            error_message.into(),
            false,
        )
        .await
    }

    pub async fn mark_failed(
        &self,
        tenant_id: Uuid,
        operation_id: Uuid,
        lease_owner: impl Into<String>,
        error_code: impl Into<String>,
        error_message: impl Into<String>,
    ) -> ReturnCompletionOperationResult<return_completion_operation::Model> {
        self.finish_execution(
            tenant_id,
            operation_id,
            lease_owner.into(),
            ReturnCompletionOperationStatus::Failed,
            error_code.into(),
            error_message.into(),
            true,
        )
        .await
    }

    pub async fn mark_completed(
        &self,
        tenant_id: Uuid,
        operation_id: Uuid,
        lease_owner: impl Into<String>,
    ) -> ReturnCompletionOperationResult<return_completion_operation::Model> {
        let lease_owner = normalize_lease_owner(lease_owner.into())?;
        let now = Utc::now().fixed_offset();
        let result = return_completion_operation::Entity::update_many()
            .col_expr(
                return_completion_operation::Column::Status,
                Expr::value(ReturnCompletionOperationStatus::Completed.as_str()),
            )
            .col_expr(
                return_completion_operation::Column::Stage,
                Expr::value(ReturnCompletionOperationStage::Completed.as_str()),
            )
            .col_expr(
                return_completion_operation::Column::LeaseOwner,
                Expr::value(Option::<String>::None),
            )
            .col_expr(
                return_completion_operation::Column::LeaseExpiresAt,
                Expr::value(Option::<chrono::DateTime<chrono::FixedOffset>>::None),
            )
            .col_expr(
                return_completion_operation::Column::CompletedAt,
                Expr::value(Some(now)),
            )
            .col_expr(
                return_completion_operation::Column::UpdatedAt,
                Expr::current_timestamp().into(),
            )
            .filter(return_completion_operation::Column::TenantId.eq(tenant_id))
            .filter(return_completion_operation::Column::Id.eq(operation_id))
            .filter(
                return_completion_operation::Column::Status
                    .eq(ReturnCompletionOperationStatus::Executing.as_str()),
            )
            .filter(return_completion_operation::Column::LeaseOwner.eq(lease_owner))
            .filter(return_completion_operation::Column::LeaseExpiresAt.gt(now))
            .exec(&self.db)
            .await?;
        if result.rows_affected == 0 {
            return Err(self
                .cas_conflict(tenant_id, operation_id, "complete")
                .await?);
        }
        self.get(tenant_id, operation_id).await
    }

    async fn finish_execution(
        &self,
        tenant_id: Uuid,
        operation_id: Uuid,
        lease_owner: String,
        next_status: ReturnCompletionOperationStatus,
        error_code: String,
        error_message: String,
        terminal: bool,
    ) -> ReturnCompletionOperationResult<return_completion_operation::Model> {
        let lease_owner = normalize_lease_owner(lease_owner)?;
        let now = Utc::now().fixed_offset();
        let mut update = return_completion_operation::Entity::update_many()
            .col_expr(
                return_completion_operation::Column::Status,
                Expr::value(next_status.as_str()),
            )
            .col_expr(
                return_completion_operation::Column::LeaseOwner,
                Expr::value(Option::<String>::None),
            )
            .col_expr(
                return_completion_operation::Column::LeaseExpiresAt,
                Expr::value(Option::<chrono::DateTime<chrono::FixedOffset>>::None),
            )
            .col_expr(
                return_completion_operation::Column::LastErrorCode,
                Expr::value(Some(normalize_error_code(error_code))),
            )
            .col_expr(
                return_completion_operation::Column::LastErrorMessage,
                Expr::value(Some(normalize_error_message(error_message))),
            )
            .col_expr(
                return_completion_operation::Column::UpdatedAt,
                Expr::current_timestamp().into(),
            )
            .filter(return_completion_operation::Column::TenantId.eq(tenant_id))
            .filter(return_completion_operation::Column::Id.eq(operation_id))
            .filter(
                return_completion_operation::Column::Status
                    .eq(ReturnCompletionOperationStatus::Executing.as_str()),
            )
            .filter(return_completion_operation::Column::LeaseOwner.eq(lease_owner))
            .filter(return_completion_operation::Column::LeaseExpiresAt.gt(now));
        if terminal {
            update = update.col_expr(
                return_completion_operation::Column::CompletedAt,
                Expr::value(Some(now)),
            );
        }
        let result = update.exec(&self.db).await?;
        if result.rows_affected == 0 {
            return Err(self.cas_conflict(tenant_id, operation_id, "finish").await?);
        }
        self.get(tenant_id, operation_id).await
    }

    async fn cas_conflict(
        &self,
        tenant_id: Uuid,
        operation_id: Uuid,
        action: &str,
    ) -> ReturnCompletionOperationResult<ReturnCompletionOperationError> {
        let current = self.get(tenant_id, operation_id).await?;
        Ok(ReturnCompletionOperationError::Conflict(format!(
            "cannot {action} operation {operation_id} from status `{}` stage `{}`",
            current.status, current.stage
        )))
    }
}

fn ensure_same_request(
    existing: &return_completion_operation::Model,
    request_hash: &str,
) -> ReturnCompletionOperationResult<()> {
    if existing.request_hash != request_hash {
        return Err(ReturnCompletionOperationError::Conflict(format!(
            "return {} is already bound to another completion request",
            existing.return_id
        )));
    }
    Ok(())
}

fn normalize_hash(value: String) -> ReturnCompletionOperationResult<String> {
    let value = value.trim().to_ascii_lowercase();
    if value.len() != 64 || !value.chars().all(|ch| ch.is_ascii_hexdigit()) {
        return Err(ReturnCompletionOperationError::Validation(
            "request_hash must be a 64-character hexadecimal SHA-256 digest".to_string(),
        ));
    }
    Ok(value)
}

fn normalize_lease_owner(value: String) -> ReturnCompletionOperationResult<String> {
    let value = value.trim().to_string();
    if value.is_empty() || value.len() > 191 {
        return Err(ReturnCompletionOperationError::Validation(
            "lease_owner must contain 1 to 191 bytes".to_string(),
        ));
    }
    Ok(value)
}

fn normalize_lease_seconds(value: i64) -> ReturnCompletionOperationResult<i64> {
    if !(1..=MAX_RETURN_COMPLETION_LEASE_SECONDS).contains(&value) {
        return Err(ReturnCompletionOperationError::Validation(format!(
            "lease_seconds must be between 1 and {MAX_RETURN_COMPLETION_LEASE_SECONDS}"
        )));
    }
    Ok(value)
}

fn normalize_error_code(value: String) -> String {
    let value = value.trim();
    if value.is_empty() {
        "return_completion_failed".to_string()
    } else {
        value.chars().take(100).collect()
    }
}

fn normalize_error_message(value: String) -> String {
    let value = value.trim();
    if value.is_empty() {
        "Return completion failed".to_string()
    } else {
        value.chars().take(2000).collect()
    }
}

fn is_unique_constraint(error: &sea_orm::DbErr) -> bool {
    matches!(
        error.sql_err(),
        Some(sea_orm::SqlErr::UniqueConstraintViolation(_))
    )
}
