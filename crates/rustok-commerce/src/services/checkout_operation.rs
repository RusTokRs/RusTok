use chrono::{DateTime, Duration, FixedOffset, Utc};
use sea_orm::{
    ActiveModelTrait, ColumnTrait, Condition, DatabaseConnection, EntityTrait, QueryFilter,
    QueryOrder, Set, sea_query::Expr,
};
use serde::{Deserialize, Serialize};
use thiserror::Error;
use uuid::Uuid;

use rustok_core::generate_id;

use crate::entities::checkout_operation;

pub const DEFAULT_CHECKOUT_LEASE_SECONDS: i64 = 60;
pub const MAX_CHECKOUT_LEASE_SECONDS: i64 = 900;

#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq, Eq, Hash)]
#[serde(rename_all = "snake_case")]
pub enum CheckoutOperationStatus {
    Pending,
    Executing,
    RetryableError,
    CompensationRequired,
    Compensating,
    Completed,
    Compensated,
    Failed,
}

impl CheckoutOperationStatus {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Pending => "pending",
            Self::Executing => "executing",
            Self::RetryableError => "retryable_error",
            Self::CompensationRequired => "compensation_required",
            Self::Compensating => "compensating",
            Self::Completed => "completed",
            Self::Compensated => "compensated",
            Self::Failed => "failed",
        }
    }
}

#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq, Eq, Hash)]
#[serde(rename_all = "snake_case")]
pub enum CheckoutOperationStage {
    Created,
    CartLocked,
    OrderCreated,
    InventoryReserved,
    PaymentReady,
    PaymentAuthorized,
    PaymentCaptured,
    FulfillmentCreated,
    CartCompleted,
    Completed,
}

impl CheckoutOperationStage {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Created => "created",
            Self::CartLocked => "cart_locked",
            Self::OrderCreated => "order_created",
            Self::InventoryReserved => "inventory_reserved",
            Self::PaymentReady => "payment_ready",
            Self::PaymentAuthorized => "payment_authorized",
            Self::PaymentCaptured => "payment_captured",
            Self::FulfillmentCreated => "fulfillment_created",
            Self::CartCompleted => "cart_completed",
            Self::Completed => "completed",
        }
    }
}

#[derive(Debug, Error)]
pub enum CheckoutOperationError {
    #[error("validation failed: {0}")]
    Validation(String),
    #[error("checkout operation {0} not found")]
    NotFound(Uuid),
    #[error("checkout operation conflict: {0}")]
    Conflict(String),
    #[error(transparent)]
    Database(#[from] sea_orm::DbErr),
}

pub type CheckoutOperationResult<T> = Result<T, CheckoutOperationError>;

#[derive(Clone, Debug)]
pub struct BeginCheckoutOperation {
    pub tenant_id: Uuid,
    pub cart_id: Uuid,
    pub idempotency_key: String,
    pub request_hash: String,
    pub snapshot_hash: Option<String>,
}

#[derive(Clone, Debug)]
pub struct CheckoutOperationCheckpoint {
    pub tenant_id: Uuid,
    pub operation_id: Uuid,
    pub lease_owner: String,
    pub expected_stage: CheckoutOperationStage,
    pub next_stage: CheckoutOperationStage,
    pub snapshot_hash: Option<String>,
    pub order_id: Option<Uuid>,
    pub payment_collection_id: Option<Uuid>,
    pub lease_seconds: i64,
}

#[derive(Clone)]
pub struct CheckoutOperationJournal {
    db: DatabaseConnection,
}

struct LeaseErrorTransition {
    lease_owner: String,
    expected_status: CheckoutOperationStatus,
    next_status: CheckoutOperationStatus,
    error_code: String,
    error_message: String,
}

struct TerminalTransition {
    lease_owner: String,
    expected_status: CheckoutOperationStatus,
    next_status: CheckoutOperationStatus,
    next_stage: Option<CheckoutOperationStage>,
    error_code: Option<String>,
    error_message: Option<String>,
}

impl CheckoutOperationJournal {
    pub fn new(db: DatabaseConnection) -> Self {
        Self { db }
    }

    pub async fn begin(
        &self,
        input: BeginCheckoutOperation,
    ) -> CheckoutOperationResult<checkout_operation::Model> {
        let input = normalize_begin_input(input)?;
        if let Some(existing) = self
            .find_by_key(
                input.tenant_id,
                input.cart_id,
                input.idempotency_key.as_str(),
            )
            .await?
        {
            ensure_same_request(&existing, &input)?;
            return Ok(existing);
        }
        if let Some(active) = self
            .find_active_by_cart(input.tenant_id, input.cart_id)
            .await?
        {
            return Err(active_cart_conflict(input.cart_id, active.id));
        }

        let id = generate_id();
        let now = Utc::now();
        let insert = checkout_operation::ActiveModel {
            id: Set(id),
            tenant_id: Set(input.tenant_id),
            cart_id: Set(input.cart_id),
            idempotency_key: Set(input.idempotency_key.clone()),
            request_hash: Set(input.request_hash.clone()),
            snapshot_hash: Set(input.snapshot_hash.clone()),
            status: Set(CheckoutOperationStatus::Pending.as_str().to_string()),
            stage: Set(CheckoutOperationStage::Created.as_str().to_string()),
            order_id: Set(None),
            payment_collection_id: Set(None),
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
            Err(insert_error) => {
                if let Some(existing) = self
                    .find_by_key(
                        input.tenant_id,
                        input.cart_id,
                        input.idempotency_key.as_str(),
                    )
                    .await?
                {
                    ensure_same_request(&existing, &input)?;
                    return Ok(existing);
                }
                if let Some(active) = self
                    .find_active_by_cart(input.tenant_id, input.cart_id)
                    .await?
                {
                    return Err(active_cart_conflict(input.cart_id, active.id));
                }
                Err(insert_error.into())
            }
        }
    }

    pub async fn get(
        &self,
        tenant_id: Uuid,
        id: Uuid,
    ) -> CheckoutOperationResult<checkout_operation::Model> {
        checkout_operation::Entity::find_by_id(id)
            .filter(checkout_operation::Column::TenantId.eq(tenant_id))
            .one(&self.db)
            .await?
            .ok_or(CheckoutOperationError::NotFound(id))
    }

    pub async fn find_by_key(
        &self,
        tenant_id: Uuid,
        cart_id: Uuid,
        idempotency_key: &str,
    ) -> CheckoutOperationResult<Option<checkout_operation::Model>> {
        checkout_operation::Entity::find()
            .filter(checkout_operation::Column::TenantId.eq(tenant_id))
            .filter(checkout_operation::Column::CartId.eq(cart_id))
            .filter(checkout_operation::Column::IdempotencyKey.eq(idempotency_key))
            .one(&self.db)
            .await
            .map_err(Into::into)
    }

    pub async fn find_latest_by_cart(
        &self,
        tenant_id: Uuid,
        cart_id: Uuid,
    ) -> CheckoutOperationResult<Option<checkout_operation::Model>> {
        checkout_operation::Entity::find()
            .filter(checkout_operation::Column::TenantId.eq(tenant_id))
            .filter(checkout_operation::Column::CartId.eq(cart_id))
            .order_by_desc(checkout_operation::Column::CreatedAt)
            .one(&self.db)
            .await
            .map_err(Into::into)
    }

    pub async fn find_active_by_cart(
        &self,
        tenant_id: Uuid,
        cart_id: Uuid,
    ) -> CheckoutOperationResult<Option<checkout_operation::Model>> {
        checkout_operation::Entity::find()
            .filter(checkout_operation::Column::TenantId.eq(tenant_id))
            .filter(checkout_operation::Column::CartId.eq(cart_id))
            .filter(checkout_operation::Column::Status.is_in(active_statuses()))
            .order_by_desc(checkout_operation::Column::CreatedAt)
            .one(&self.db)
            .await
            .map_err(Into::into)
    }

    pub async fn claim_execution(
        &self,
        tenant_id: Uuid,
        id: Uuid,
        lease_owner: impl Into<String>,
        lease_seconds: i64,
    ) -> CheckoutOperationResult<Option<checkout_operation::Model>> {
        let lease_owner = normalize_lease_owner(lease_owner.into())?;
        let lease_seconds = normalize_lease_seconds(lease_seconds)?;
        let now = Utc::now().fixed_offset();
        let lease_expires_at = now + Duration::seconds(lease_seconds);
        let claimable = Condition::any()
            .add(checkout_operation::Column::Status.is_in([
                CheckoutOperationStatus::Pending.as_str(),
                CheckoutOperationStatus::RetryableError.as_str(),
            ]))
            .add(
                Condition::all()
                    .add(
                        checkout_operation::Column::Status
                            .eq(CheckoutOperationStatus::Executing.as_str()),
                    )
                    .add(checkout_operation::Column::LeaseExpiresAt.lte(now)),
            );

        let update = checkout_operation::Entity::update_many()
            .col_expr(
                checkout_operation::Column::Status,
                Expr::value(CheckoutOperationStatus::Executing.as_str()),
            )
            .col_expr(
                checkout_operation::Column::LeaseOwner,
                Expr::value(Some(lease_owner)),
            )
            .col_expr(
                checkout_operation::Column::LeaseExpiresAt,
                Expr::value(Some(lease_expires_at)),
            )
            .col_expr(
                checkout_operation::Column::AttemptCount,
                Expr::col(checkout_operation::Column::AttemptCount).add(1),
            )
            .col_expr(
                checkout_operation::Column::LastErrorCode,
                Expr::value(Option::<String>::None),
            )
            .col_expr(
                checkout_operation::Column::LastErrorMessage,
                Expr::value(Option::<String>::None),
            )
            .col_expr(
                checkout_operation::Column::UpdatedAt,
                Expr::current_timestamp().into(),
            )
            .filter(checkout_operation::Column::TenantId.eq(tenant_id))
            .filter(checkout_operation::Column::Id.eq(id))
            .filter(claimable)
            .exec(&self.db)
            .await?;

        if update.rows_affected == 0 {
            return Ok(None);
        }
        self.get(tenant_id, id).await.map(Some)
    }

    pub async fn checkpoint(
        &self,
        input: CheckoutOperationCheckpoint,
    ) -> CheckoutOperationResult<checkout_operation::Model> {
        let lease_owner = normalize_lease_owner(input.lease_owner)?;
        let lease_seconds = normalize_lease_seconds(input.lease_seconds)?;
        let snapshot_hash = input.snapshot_hash.map(normalize_hash).transpose()?;
        let now = Utc::now().fixed_offset();
        let lease_expires_at = now + Duration::seconds(lease_seconds);

        let mut update = checkout_operation::Entity::update_many()
            .col_expr(
                checkout_operation::Column::Stage,
                Expr::value(input.next_stage.as_str()),
            )
            .col_expr(
                checkout_operation::Column::LeaseExpiresAt,
                Expr::value(Some(lease_expires_at)),
            )
            .col_expr(
                checkout_operation::Column::UpdatedAt,
                Expr::current_timestamp().into(),
            )
            .filter(checkout_operation::Column::TenantId.eq(input.tenant_id))
            .filter(checkout_operation::Column::Id.eq(input.operation_id))
            .filter(
                checkout_operation::Column::Status.eq(CheckoutOperationStatus::Executing.as_str()),
            )
            .filter(checkout_operation::Column::Stage.eq(input.expected_stage.as_str()))
            .filter(checkout_operation::Column::LeaseOwner.eq(lease_owner))
            .filter(checkout_operation::Column::LeaseExpiresAt.gt(now));

        if let Some(snapshot_hash) = snapshot_hash {
            update = update.col_expr(
                checkout_operation::Column::SnapshotHash,
                Expr::value(Some(snapshot_hash)),
            );
        }
        if let Some(order_id) = input.order_id {
            update = update.col_expr(
                checkout_operation::Column::OrderId,
                Expr::value(Some(order_id)),
            );
        }
        if let Some(payment_collection_id) = input.payment_collection_id {
            update = update.col_expr(
                checkout_operation::Column::PaymentCollectionId,
                Expr::value(Some(payment_collection_id)),
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

    pub async fn mark_retryable_error(
        &self,
        tenant_id: Uuid,
        id: Uuid,
        lease_owner: impl Into<String>,
        error_code: impl Into<String>,
        error_message: impl Into<String>,
    ) -> CheckoutOperationResult<checkout_operation::Model> {
        self.release_lease_with_error(
            tenant_id,
            id,
            LeaseErrorTransition {
                lease_owner: lease_owner.into(),
                expected_status: CheckoutOperationStatus::Executing,
                next_status: CheckoutOperationStatus::RetryableError,
                error_code: error_code.into(),
                error_message: error_message.into(),
            },
        )
        .await
    }

    pub async fn mark_compensation_required(
        &self,
        tenant_id: Uuid,
        id: Uuid,
        lease_owner: impl Into<String>,
        error_code: impl Into<String>,
        error_message: impl Into<String>,
    ) -> CheckoutOperationResult<checkout_operation::Model> {
        self.release_lease_with_error(
            tenant_id,
            id,
            LeaseErrorTransition {
                lease_owner: lease_owner.into(),
                expected_status: CheckoutOperationStatus::Executing,
                next_status: CheckoutOperationStatus::CompensationRequired,
                error_code: error_code.into(),
                error_message: error_message.into(),
            },
        )
        .await
    }

    pub async fn claim_compensation(
        &self,
        tenant_id: Uuid,
        id: Uuid,
        lease_owner: impl Into<String>,
        lease_seconds: i64,
    ) -> CheckoutOperationResult<Option<checkout_operation::Model>> {
        let lease_owner = normalize_lease_owner(lease_owner.into())?;
        let lease_seconds = normalize_lease_seconds(lease_seconds)?;
        let now = Utc::now().fixed_offset();
        let lease_expires_at = now + Duration::seconds(lease_seconds);
        let claimable = Condition::any()
            .add(
                checkout_operation::Column::Status
                    .eq(CheckoutOperationStatus::CompensationRequired.as_str()),
            )
            .add(
                Condition::all()
                    .add(
                        checkout_operation::Column::Status
                            .eq(CheckoutOperationStatus::Compensating.as_str()),
                    )
                    .add(checkout_operation::Column::LeaseExpiresAt.lte(now)),
            );
        let update = checkout_operation::Entity::update_many()
            .col_expr(
                checkout_operation::Column::Status,
                Expr::value(CheckoutOperationStatus::Compensating.as_str()),
            )
            .col_expr(
                checkout_operation::Column::LeaseOwner,
                Expr::value(Some(lease_owner)),
            )
            .col_expr(
                checkout_operation::Column::LeaseExpiresAt,
                Expr::value(Some(lease_expires_at)),
            )
            .col_expr(
                checkout_operation::Column::AttemptCount,
                Expr::col(checkout_operation::Column::AttemptCount).add(1),
            )
            .col_expr(
                checkout_operation::Column::UpdatedAt,
                Expr::current_timestamp().into(),
            )
            .filter(checkout_operation::Column::TenantId.eq(tenant_id))
            .filter(checkout_operation::Column::Id.eq(id))
            .filter(claimable)
            .exec(&self.db)
            .await?;
        if update.rows_affected == 0 {
            return Ok(None);
        }
        self.get(tenant_id, id).await.map(Some)
    }

    pub async fn mark_compensation_retryable(
        &self,
        tenant_id: Uuid,
        id: Uuid,
        lease_owner: impl Into<String>,
        error_code: impl Into<String>,
        error_message: impl Into<String>,
    ) -> CheckoutOperationResult<checkout_operation::Model> {
        self.release_lease_with_error(
            tenant_id,
            id,
            LeaseErrorTransition {
                lease_owner: lease_owner.into(),
                expected_status: CheckoutOperationStatus::Compensating,
                next_status: CheckoutOperationStatus::CompensationRequired,
                error_code: error_code.into(),
                error_message: error_message.into(),
            },
        )
        .await
    }

    pub async fn mark_completed(
        &self,
        tenant_id: Uuid,
        id: Uuid,
        lease_owner: impl Into<String>,
    ) -> CheckoutOperationResult<checkout_operation::Model> {
        self.mark_terminal(
            tenant_id,
            id,
            TerminalTransition {
                lease_owner: lease_owner.into(),
                expected_status: CheckoutOperationStatus::Executing,
                next_status: CheckoutOperationStatus::Completed,
                next_stage: Some(CheckoutOperationStage::Completed),
                error_code: None,
                error_message: None,
            },
        )
        .await
    }

    pub async fn mark_compensated(
        &self,
        tenant_id: Uuid,
        id: Uuid,
        lease_owner: impl Into<String>,
    ) -> CheckoutOperationResult<checkout_operation::Model> {
        self.mark_terminal(
            tenant_id,
            id,
            TerminalTransition {
                lease_owner: lease_owner.into(),
                expected_status: CheckoutOperationStatus::Compensating,
                next_status: CheckoutOperationStatus::Compensated,
                next_stage: None,
                error_code: None,
                error_message: None,
            },
        )
        .await
    }

    pub async fn mark_failed(
        &self,
        tenant_id: Uuid,
        id: Uuid,
        lease_owner: impl Into<String>,
        error_code: impl Into<String>,
        error_message: impl Into<String>,
    ) -> CheckoutOperationResult<checkout_operation::Model> {
        let current = self.get(tenant_id, id).await?;
        let expected_status = match current.status.as_str() {
            "executing" => CheckoutOperationStatus::Executing,
            "compensating" => CheckoutOperationStatus::Compensating,
            other => {
                return Err(CheckoutOperationError::Conflict(format!(
                    "checkout operation {id} cannot fail from status `{other}`"
                )));
            }
        };
        self.mark_terminal(
            tenant_id,
            id,
            TerminalTransition {
                lease_owner: lease_owner.into(),
                expected_status,
                next_status: CheckoutOperationStatus::Failed,
                next_stage: None,
                error_code: Some(error_code.into()),
                error_message: Some(error_message.into()),
            },
        )
        .await
    }

    async fn release_lease_with_error(
        &self,
        tenant_id: Uuid,
        id: Uuid,
        transition: LeaseErrorTransition,
    ) -> CheckoutOperationResult<checkout_operation::Model> {
        let lease_owner = normalize_lease_owner(transition.lease_owner)?;
        let error_code = normalize_error_code(transition.error_code)?;
        let error_message = normalize_error_message(transition.error_message)?;
        let now = Utc::now().fixed_offset();
        let update = checkout_operation::Entity::update_many()
            .col_expr(
                checkout_operation::Column::Status,
                Expr::value(transition.next_status.as_str()),
            )
            .col_expr(
                checkout_operation::Column::LeaseOwner,
                Expr::value(Option::<String>::None),
            )
            .col_expr(
                checkout_operation::Column::LeaseExpiresAt,
                Expr::value(Option::<DateTime<FixedOffset>>::None),
            )
            .col_expr(
                checkout_operation::Column::LastErrorCode,
                Expr::value(Some(error_code)),
            )
            .col_expr(
                checkout_operation::Column::LastErrorMessage,
                Expr::value(Some(error_message)),
            )
            .col_expr(
                checkout_operation::Column::UpdatedAt,
                Expr::current_timestamp().into(),
            )
            .filter(checkout_operation::Column::TenantId.eq(tenant_id))
            .filter(checkout_operation::Column::Id.eq(id))
            .filter(checkout_operation::Column::Status.eq(transition.expected_status.as_str()))
            .filter(checkout_operation::Column::LeaseOwner.eq(lease_owner))
            .filter(checkout_operation::Column::LeaseExpiresAt.gt(now))
            .exec(&self.db)
            .await?;
        if update.rows_affected == 0 {
            return Err(self
                .cas_conflict(tenant_id, id, transition.next_status.as_str())
                .await?);
        }
        self.get(tenant_id, id).await
    }

    async fn mark_terminal(
        &self,
        tenant_id: Uuid,
        id: Uuid,
        transition: TerminalTransition,
    ) -> CheckoutOperationResult<checkout_operation::Model> {
        let lease_owner = normalize_lease_owner(transition.lease_owner)?;
        let error_code = transition
            .error_code
            .map(normalize_error_code)
            .transpose()?;
        let error_message = transition
            .error_message
            .map(normalize_error_message)
            .transpose()?;
        let now = Utc::now().fixed_offset();
        let mut update = checkout_operation::Entity::update_many()
            .col_expr(
                checkout_operation::Column::Status,
                Expr::value(transition.next_status.as_str()),
            )
            .col_expr(
                checkout_operation::Column::LeaseOwner,
                Expr::value(Option::<String>::None),
            )
            .col_expr(
                checkout_operation::Column::LeaseExpiresAt,
                Expr::value(Option::<DateTime<FixedOffset>>::None),
            )
            .col_expr(
                checkout_operation::Column::LastErrorCode,
                Expr::value(error_code),
            )
            .col_expr(
                checkout_operation::Column::LastErrorMessage,
                Expr::value(error_message),
            )
            .col_expr(
                checkout_operation::Column::CompletedAt,
                Expr::value(Some(now)),
            )
            .col_expr(
                checkout_operation::Column::UpdatedAt,
                Expr::current_timestamp().into(),
            )
            .filter(checkout_operation::Column::TenantId.eq(tenant_id))
            .filter(checkout_operation::Column::Id.eq(id))
            .filter(checkout_operation::Column::Status.eq(transition.expected_status.as_str()))
            .filter(checkout_operation::Column::LeaseOwner.eq(lease_owner))
            .filter(checkout_operation::Column::LeaseExpiresAt.gt(now));
        if let Some(next_stage) = transition.next_stage {
            update = update.col_expr(
                checkout_operation::Column::Stage,
                Expr::value(next_stage.as_str()),
            );
        }
        let result = update.exec(&self.db).await?;
        if result.rows_affected == 0 {
            return Err(self
                .cas_conflict(tenant_id, id, transition.next_status.as_str())
                .await?);
        }
        self.get(tenant_id, id).await
    }

    async fn cas_conflict(
        &self,
        tenant_id: Uuid,
        id: Uuid,
        action: &str,
    ) -> CheckoutOperationResult<CheckoutOperationError> {
        let current = self.get(tenant_id, id).await?;
        Ok(CheckoutOperationError::Conflict(format!(
            "cannot {action} checkout operation {id}; current status={}, stage={}, lease_owner={}",
            current.status,
            current.stage,
            current.lease_owner.as_deref().unwrap_or("none")
        )))
    }
}

fn active_statuses() -> [&'static str; 5] {
    [
        CheckoutOperationStatus::Pending.as_str(),
        CheckoutOperationStatus::Executing.as_str(),
        CheckoutOperationStatus::RetryableError.as_str(),
        CheckoutOperationStatus::CompensationRequired.as_str(),
        CheckoutOperationStatus::Compensating.as_str(),
    ]
}

fn active_cart_conflict(cart_id: Uuid, operation_id: Uuid) -> CheckoutOperationError {
    CheckoutOperationError::Conflict(format!(
        "cart {cart_id} already has active checkout operation {operation_id}"
    ))
}

fn normalize_begin_input(
    mut input: BeginCheckoutOperation,
) -> CheckoutOperationResult<BeginCheckoutOperation> {
    input.idempotency_key = normalize_bounded("idempotency_key", input.idempotency_key, 191)?;
    input.request_hash = normalize_hash(input.request_hash)?;
    input.snapshot_hash = input.snapshot_hash.map(normalize_hash).transpose()?;
    Ok(input)
}

fn ensure_same_request(
    existing: &checkout_operation::Model,
    input: &BeginCheckoutOperation,
) -> CheckoutOperationResult<()> {
    if existing.request_hash != input.request_hash || existing.snapshot_hash != input.snapshot_hash
    {
        return Err(CheckoutOperationError::Conflict(format!(
            "idempotency key `{}` is already bound to a different checkout request",
            input.idempotency_key
        )));
    }
    Ok(())
}

fn normalize_hash(value: String) -> CheckoutOperationResult<String> {
    let value = normalize_bounded("hash", value, 128)?;
    if !value.chars().all(|character| character.is_ascii_hexdigit()) {
        return Err(CheckoutOperationError::Validation(
            "checkout hashes must contain only hexadecimal characters".to_string(),
        ));
    }
    Ok(value.to_ascii_lowercase())
}

fn normalize_lease_owner(value: String) -> CheckoutOperationResult<String> {
    normalize_bounded("lease_owner", value, 191)
}

fn normalize_lease_seconds(value: i64) -> CheckoutOperationResult<i64> {
    if (1..=MAX_CHECKOUT_LEASE_SECONDS).contains(&value) {
        Ok(value)
    } else {
        Err(CheckoutOperationError::Validation(format!(
            "lease_seconds must be between 1 and {MAX_CHECKOUT_LEASE_SECONDS}"
        )))
    }
}

fn normalize_error_code(value: String) -> CheckoutOperationResult<String> {
    normalize_bounded("error_code", value, 100)
}

fn normalize_error_message(value: String) -> CheckoutOperationResult<String> {
    normalize_bounded("error_message", value, 2000)
}

fn normalize_bounded(
    field: &str,
    value: String,
    maximum_length: usize,
) -> CheckoutOperationResult<String> {
    let value = value.trim().to_string();
    if value.is_empty() || value.chars().count() > maximum_length {
        return Err(CheckoutOperationError::Validation(format!(
            "{field} must contain 1 to {maximum_length} characters"
        )));
    }
    Ok(value)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn checkout_hashes_are_normalized_and_fail_closed() {
        assert_eq!(
            normalize_hash("A0ff".to_string()).expect("hex hash"),
            "a0ff"
        );
        assert!(normalize_hash("not-a-hash".to_string()).is_err());
        assert!(normalize_hash(String::new()).is_err());
    }

    #[test]
    fn checkout_lease_duration_is_bounded() {
        assert!(normalize_lease_seconds(1).is_ok());
        assert!(normalize_lease_seconds(MAX_CHECKOUT_LEASE_SECONDS).is_ok());
        assert!(normalize_lease_seconds(0).is_err());
        assert!(normalize_lease_seconds(MAX_CHECKOUT_LEASE_SECONDS + 1).is_err());
    }
}
