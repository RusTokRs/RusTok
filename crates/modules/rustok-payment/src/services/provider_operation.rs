use chrono::Utc;
use sea_orm::{
    ActiveModelTrait, ColumnTrait, DatabaseConnection, EntityTrait, QueryFilter, QueryOrder, Set,
    sea_query::Expr,
};
use serde_json::Value;
use uuid::Uuid;

use rustok_core::generate_id;

use crate::entities::{payment_collection, provider_operation, refund};
use crate::error::{PaymentError, PaymentResult};
use crate::providers::{
    MAX_EXTERNAL_REFERENCE_LENGTH, PaymentProviderOperationResult,
    validate_provider_operation_payload,
};

pub const PROVIDER_OPERATION_PENDING: &str = "pending";
pub const PROVIDER_OPERATION_EXECUTING: &str = "executing";
pub const PROVIDER_OPERATION_SUCCEEDED: &str = "provider_succeeded";
pub const PROVIDER_OPERATION_ERROR: &str = "provider_error";
pub const PROVIDER_OPERATION_RECONCILIATION_REQUIRED: &str = "reconciliation_required";
pub const PROVIDER_OPERATION_COMMITTED: &str = "committed";

#[derive(Clone, Debug)]
pub struct BeginProviderOperation {
    pub tenant_id: Uuid,
    pub payment_collection_id: Uuid,
    pub refund_id: Option<Uuid>,
    pub operation: String,
    pub provider_id: String,
    pub idempotency_key: String,
    pub request_payload: Value,
}

#[derive(Clone)]
pub struct PaymentProviderOperationJournal {
    db: DatabaseConnection,
}

impl PaymentProviderOperationJournal {
    pub fn new(db: DatabaseConnection) -> Self {
        Self { db }
    }

    /// Create an operation journal row or return the existing row for the same
    /// provider idempotency key. A key collision with a different immutable
    /// request is rejected instead of silently reusing the wrong operation.
    pub async fn begin(
        &self,
        input: BeginProviderOperation,
    ) -> PaymentResult<provider_operation::Model> {
        let input = normalize_begin_input(input)?;
        validate_begin_identity(&self.db, &input).await?;
        if let Some(existing) = self
            .find_by_key(input.tenant_id, &input.provider_id, &input.idempotency_key)
            .await?
        {
            ensure_same_request(&existing, &input)?;
            return Ok(existing);
        }

        let id = generate_id();
        let now = Utc::now();
        let insert = provider_operation::ActiveModel {
            id: Set(id),
            tenant_id: Set(input.tenant_id),
            payment_collection_id: Set(input.payment_collection_id),
            refund_id: Set(input.refund_id),
            operation: Set(input.operation.clone()),
            provider_id: Set(input.provider_id.clone()),
            idempotency_key: Set(input.idempotency_key.clone()),
            status: Set(PROVIDER_OPERATION_PENDING.to_string()),
            request_payload: Set(input.request_payload.clone()),
            provider_reference: Set(None),
            provider_result: Set(None),
            error_message: Set(None),
            created_at: Set(now.into()),
            updated_at: Set(now.into()),
            provider_completed_at: Set(None),
            committed_at: Set(None),
        }
        .insert(&self.db)
        .await;

        match insert {
            Ok(model) => Ok(model),
            Err(insert_error) => {
                if let Some(existing) = self
                    .find_by_key(input.tenant_id, &input.provider_id, &input.idempotency_key)
                    .await?
                {
                    ensure_same_request(&existing, &input)?;
                    Ok(existing)
                } else {
                    Err(insert_error.into())
                }
            }
        }
    }

    pub async fn get(
        &self,
        tenant_id: Uuid,
        id: Uuid,
    ) -> PaymentResult<provider_operation::Model> {
        validate_operation_identity(tenant_id, id)?;
        provider_operation::Entity::find_by_id(id)
            .filter(provider_operation::Column::TenantId.eq(tenant_id))
            .one(&self.db)
            .await?
            .ok_or_else(|| {
                PaymentError::Validation(format!(
                    "payment provider operation {id} not found for tenant {tenant_id}"
                ))
            })
    }

    pub async fn find_by_key(
        &self,
        tenant_id: Uuid,
        provider_id: &str,
        idempotency_key: &str,
    ) -> PaymentResult<Option<provider_operation::Model>> {
        provider_operation::Entity::find()
            .filter(provider_operation::Column::TenantId.eq(tenant_id))
            .filter(provider_operation::Column::ProviderId.eq(provider_id))
            .filter(provider_operation::Column::IdempotencyKey.eq(idempotency_key))
            .one(&self.db)
            .await
            .map_err(Into::into)
    }

    pub async fn list_by_collection(
        &self,
        tenant_id: Uuid,
        payment_collection_id: Uuid,
    ) -> PaymentResult<Vec<provider_operation::Model>> {
        provider_operation::Entity::find()
            .filter(provider_operation::Column::TenantId.eq(tenant_id))
            .filter(provider_operation::Column::PaymentCollectionId.eq(payment_collection_id))
            .order_by_asc(provider_operation::Column::CreatedAt)
            .all(&self.db)
            .await
            .map_err(Into::into)
    }

    pub async fn claim_execution(
        &self,
        tenant_id: Uuid,
        id: Uuid,
    ) -> PaymentResult<Option<provider_operation::Model>> {
        validate_operation_identity(tenant_id, id)?;
        let update = provider_operation::Entity::update_many()
            .col_expr(
                provider_operation::Column::Status,
                Expr::value(PROVIDER_OPERATION_EXECUTING),
            )
            .col_expr(
                provider_operation::Column::UpdatedAt,
                Expr::current_timestamp(),
            )
            .filter(provider_operation::Column::TenantId.eq(tenant_id))
            .filter(provider_operation::Column::Id.eq(id))
            .filter(
                provider_operation::Column::Status
                    .is_in([PROVIDER_OPERATION_PENDING, PROVIDER_OPERATION_ERROR]),
            )
            .exec(&self.db)
            .await?;

        if update.rows_affected == 0 {
            return Ok(None);
        }
        self.get(tenant_id, id).await.map(Some)
    }

    pub async fn mark_provider_succeeded(
        &self,
        tenant_id: Uuid,
        id: Uuid,
        provider_reference: Option<String>,
        provider_result: Value,
    ) -> PaymentResult<provider_operation::Model> {
        validate_operation_identity(tenant_id, id)?;
        let model = self.get(tenant_id, id).await?;
        let provider_reference =
            validate_provider_result_for_operation(&model, provider_reference, &provider_result)?;
        if matches!(
            model.status.as_str(),
            PROVIDER_OPERATION_SUCCEEDED
                | PROVIDER_OPERATION_RECONCILIATION_REQUIRED
                | PROVIDER_OPERATION_COMMITTED
        ) {
            return Ok(model);
        }
        ensure_transition(&model.status, PROVIDER_OPERATION_SUCCEEDED)?;

        let now = Utc::now();
        let update = provider_operation::Entity::update_many()
            .col_expr(
                provider_operation::Column::Status,
                Expr::value(PROVIDER_OPERATION_SUCCEEDED),
            )
            .col_expr(
                provider_operation::Column::ProviderReference,
                Expr::value(provider_reference),
            )
            .col_expr(
                provider_operation::Column::ProviderResult,
                Expr::value(Some(provider_result)),
            )
            .col_expr(
                provider_operation::Column::ErrorMessage,
                Expr::value(Option::<String>::None),
            )
            .col_expr(
                provider_operation::Column::UpdatedAt,
                Expr::value(now),
            )
            .col_expr(
                provider_operation::Column::ProviderCompletedAt,
                Expr::value(Some(now)),
            )
            .filter(provider_operation::Column::Id.eq(id))
            .filter(provider_operation::Column::Status.eq(PROVIDER_OPERATION_EXECUTING))
            .exec(&self.db)
            .await?;

        if update.rows_affected == 0 {
            let current = self.get(tenant_id, id).await?;
            if matches!(
                current.status.as_str(),
                PROVIDER_OPERATION_SUCCEEDED
                    | PROVIDER_OPERATION_RECONCILIATION_REQUIRED
                    | PROVIDER_OPERATION_COMMITTED
            ) {
                return Ok(current);
            }
            return Err(PaymentError::InvalidTransition {
                from: current.status,
                to: PROVIDER_OPERATION_SUCCEEDED.to_string(),
            });
        }

        self.get(tenant_id, id).await
    }

    pub async fn mark_provider_error(
        &self,
        tenant_id: Uuid,
        id: Uuid,
        error_message: impl Into<String>,
    ) -> PaymentResult<provider_operation::Model> {
        validate_operation_identity(tenant_id, id)?;
        let model = self.get(tenant_id, id).await?;
        ensure_transition(&model.status, PROVIDER_OPERATION_ERROR)?;

        let now = Utc::now();
        let error_message = normalize_error(error_message.into());
        let update = provider_operation::Entity::update_many()
            .col_expr(
                provider_operation::Column::Status,
                Expr::value(PROVIDER_OPERATION_ERROR),
            )
            .col_expr(
                provider_operation::Column::ErrorMessage,
                Expr::value(Some(error_message)),
            )
            .col_expr(
                provider_operation::Column::UpdatedAt,
                Expr::value(now),
            )
            .filter(provider_operation::Column::Id.eq(id))
            .filter(provider_operation::Column::Status.eq(PROVIDER_OPERATION_EXECUTING))
            .exec(&self.db)
            .await?;

        if update.rows_affected == 0 {
            let current = self.get(tenant_id, id).await?;
            if current.status == PROVIDER_OPERATION_ERROR {
                return Ok(current);
            }
            return Err(PaymentError::InvalidTransition {
                from: current.status,
                to: PROVIDER_OPERATION_ERROR.to_string(),
            });
        }

        self.get(tenant_id, id).await
    }

    /// Record an operation whose external outcome cannot be safely retried.
    /// This transition is valid both after a persisted provider success and
    /// directly from `executing` when the provider may have accepted the request
    /// but the response or local success checkpoint is uncertain.
    pub async fn mark_reconciliation_required(
        &self,
        tenant_id: Uuid,
        id: Uuid,
        error_message: impl Into<String>,
    ) -> PaymentResult<provider_operation::Model> {
        validate_operation_identity(tenant_id, id)?;
        let model = self.get(tenant_id, id).await?;
        if model.status == PROVIDER_OPERATION_RECONCILIATION_REQUIRED {
            return Ok(model);
        }
        ensure_transition(&model.status, PROVIDER_OPERATION_RECONCILIATION_REQUIRED)?;

        let error_message = normalize_error(error_message.into());
        let update = provider_operation::Entity::update_many()
            .col_expr(
                provider_operation::Column::Status,
                Expr::value(PROVIDER_OPERATION_RECONCILIATION_REQUIRED),
            )
            .col_expr(
                provider_operation::Column::ErrorMessage,
                Expr::value(Some(error_message)),
            )
            .col_expr(
                provider_operation::Column::UpdatedAt,
                Expr::current_timestamp(),
            )
            .filter(provider_operation::Column::Id.eq(id))
            .filter(
                provider_operation::Column::Status.is_in([
                    PROVIDER_OPERATION_EXECUTING,
                    PROVIDER_OPERATION_SUCCEEDED,
                ]),
            )
            .exec(&self.db)
            .await?;

        if update.rows_affected == 0 {
            let current = self.get(tenant_id, id).await?;
            if current.status == PROVIDER_OPERATION_RECONCILIATION_REQUIRED {
                return Ok(current);
            }
            return Err(PaymentError::InvalidTransition {
                from: current.status,
                to: PROVIDER_OPERATION_RECONCILIATION_REQUIRED.to_string(),
            });
        }

        self.get(tenant_id, id).await
    }

    pub async fn mark_committed(
        &self,
        tenant_id: Uuid,
        id: Uuid,
    ) -> PaymentResult<provider_operation::Model> {
        validate_operation_identity(tenant_id, id)?;
        let model = self.get(tenant_id, id).await?;
        if model.status == PROVIDER_OPERATION_COMMITTED {
            return Ok(model);
        }
        ensure_transition(&model.status, PROVIDER_OPERATION_COMMITTED)?;

        let provider_completion_missing = model.provider_completed_at.is_none();
        let now = Utc::now();
        let update = provider_operation::Entity::update_many()
            .col_expr(
                provider_operation::Column::Status,
                Expr::value(PROVIDER_OPERATION_COMMITTED),
            )
            .col_expr(
                provider_operation::Column::ErrorMessage,
                Expr::value(Option::<String>::None),
            )
            .col_expr(
                provider_operation::Column::UpdatedAt,
                Expr::value(now),
            )
            .col_expr(
                provider_operation::Column::ProviderCompletedAt,
                if provider_completion_missing {
                    Expr::value(Some(now))
                } else {
                    Expr::col(provider_operation::Column::ProviderCompletedAt)
                },
            )
            .col_expr(
                provider_operation::Column::CommittedAt,
                Expr::value(Some(now)),
            )
            .filter(provider_operation::Column::Id.eq(id))
            .filter(
                provider_operation::Column::Status.is_in([
                    PROVIDER_OPERATION_SUCCEEDED,
                    PROVIDER_OPERATION_RECONCILIATION_REQUIRED,
                ]),
            )
            .exec(&self.db)
            .await?;

        if update.rows_affected == 0 {
            let current = self.get(tenant_id, id).await?;
            if current.status == PROVIDER_OPERATION_COMMITTED {
                return Ok(current);
            }
            return Err(PaymentError::InvalidTransition {
                from: current.status,
                to: PROVIDER_OPERATION_COMMITTED.to_string(),
            });
        }

        self.get(tenant_id, id).await
    }
}

async fn validate_begin_identity(
    db: &DatabaseConnection,
    input: &BeginProviderOperation,
) -> PaymentResult<()> {
    validate_operation_identity(input.tenant_id, input.payment_collection_id)?;

    let collection = payment_collection::Entity::find_by_id(input.payment_collection_id)
        .one(db)
        .await?
        .ok_or(PaymentError::PaymentCollectionNotFound(
            input.payment_collection_id,
        ))?;
    if collection.tenant_id != input.tenant_id {
        return Err(PaymentError::Validation(
            "payment provider operation tenant does not match payment collection tenant"
                .to_string(),
        ));
    }

    if let Some(refund_id) = input.refund_id {
        if refund_id.is_nil() {
            return Err(PaymentError::Validation(
                "payment provider operation refund_id must not be nil".to_string(),
            ));
        }
        let refund = refund::Entity::find_by_id(refund_id)
            .one(db)
            .await?
            .ok_or(PaymentError::RefundNotFound(refund_id))?;
        if refund.tenant_id != input.tenant_id
            || refund.payment_collection_id != input.payment_collection_id
        {
            return Err(PaymentError::Validation(
                "payment provider operation refund does not belong to the payment collection and tenant"
                    .to_string(),
            ));
        }
    }

    Ok(())
}

fn validate_operation_identity(tenant_id: Uuid, operation_id: Uuid) -> PaymentResult<()> {
    if tenant_id.is_nil() || operation_id.is_nil() {
        return Err(PaymentError::Validation(
            "payment provider operation requires non-nil tenant_id and operation_id".to_string(),
        ));
    }
    Ok(())
}

fn normalize_begin_input(
    mut input: BeginProviderOperation,
) -> PaymentResult<BeginProviderOperation> {
    input.operation = input.operation.trim().to_ascii_lowercase();
    if !matches!(
        input.operation.as_str(),
        "authorize" | "capture" | "cancel" | "refund"
    ) {
        return Err(PaymentError::Validation(format!(
            "unsupported payment provider operation `{}`",
            input.operation
        )));
    }

    input.provider_id = input.provider_id.trim().to_string();
    input.idempotency_key = input.idempotency_key.trim().to_string();
    if input.provider_id.is_empty() || input.provider_id.len() > 100 {
        return Err(PaymentError::Validation(
            "provider_id must contain 1 to 100 characters".to_string(),
        ));
    }
    if input.idempotency_key.is_empty() || input.idempotency_key.len() > 191 {
        return Err(PaymentError::Validation(
            "idempotency_key must contain 1 to 191 characters".to_string(),
        ));
    }

    Ok(input)
}

fn ensure_same_request(
    existing: &provider_operation::Model,
    input: &BeginProviderOperation,
) -> PaymentResult<()> {
    if existing.tenant_id != input.tenant_id
        || existing.payment_collection_id != input.payment_collection_id
        || existing.refund_id != input.refund_id
        || existing.operation != input.operation
        || existing.provider_id != input.provider_id
        || existing.request_payload != input.request_payload
    {
        return Err(PaymentError::Validation(format!(
            "provider idempotency key `{}` is already bound to another request",
            input.idempotency_key
        )));
    }
    Ok(())
}

fn ensure_transition(from: &str, to: &str) -> PaymentResult<()> {
    let allowed = matches!(
        (from, to),
        (PROVIDER_OPERATION_PENDING, PROVIDER_OPERATION_EXECUTING)
            | (PROVIDER_OPERATION_ERROR, PROVIDER_OPERATION_EXECUTING)
            | (PROVIDER_OPERATION_EXECUTING, PROVIDER_OPERATION_SUCCEEDED)
            | (PROVIDER_OPERATION_EXECUTING, PROVIDER_OPERATION_ERROR)
            | (
                PROVIDER_OPERATION_EXECUTING,
                PROVIDER_OPERATION_RECONCILIATION_REQUIRED
            )
            | (
                PROVIDER_OPERATION_SUCCEEDED,
                PROVIDER_OPERATION_RECONCILIATION_REQUIRED
            )
            | (PROVIDER_OPERATION_SUCCEEDED, PROVIDER_OPERATION_COMMITTED)
            | (
                PROVIDER_OPERATION_RECONCILIATION_REQUIRED,
                PROVIDER_OPERATION_COMMITTED
            )
    );
    if allowed {
        Ok(())
    } else {
        Err(PaymentError::InvalidTransition {
            from: from.to_string(),
            to: to.to_string(),
        })
    }
}

fn normalize_optional(value: Option<String>) -> Option<String> {
    value
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
}

fn validate_provider_result_for_operation(
    current: &provider_operation::Model,
    provider_reference: Option<String>,
    provider_result: &Value,
) -> PaymentResult<Option<String>> {
    if !provider_result.is_object() {
        return Err(PaymentError::ProviderInvalidResponse {
            provider_id: current.provider_id.clone(),
            operation: current.operation.clone(),
        });
    }
    validate_provider_operation_payload(provider_result, "result").map_err(|_| {
        PaymentError::ProviderInvalidResponse {
            provider_id: current.provider_id.clone(),
            operation: current.operation.clone(),
        }
    })?;

    let typed_result: PaymentProviderOperationResult =
        serde_json::from_value(provider_result.clone()).map_err(|_| {
            PaymentError::ProviderInvalidResponse {
                provider_id: current.provider_id.clone(),
                operation: current.operation.clone(),
            }
        })?;

    if typed_result.provider_id != current.provider_id
        || typed_result
            .external_reference
            .as_deref()
            .map(str::trim)
            .is_some_and(|value| value.is_empty() || value.len() > MAX_EXTERNAL_REFERENCE_LENGTH)
        || !typed_result.metadata.is_object()
    {
        return Err(PaymentError::ProviderInvalidResponse {
            provider_id: current.provider_id.clone(),
            operation: current.operation.clone(),
        });
    }

    validate_provider_operation_payload(&serde_json::to_value(&typed_result).map_err(|_| {
        PaymentError::ProviderInvalidResponse {
            provider_id: current.provider_id.clone(),
            operation: current.operation.clone(),
        }
    })?, "canonical result").map_err(|_| PaymentError::ProviderInvalidResponse {
        provider_id: current.provider_id.clone(),
        operation: current.operation.clone(),
    })?;

    validate_provider_reference(provider_reference.as_deref())?;

    let result_reference = normalize_optional(typed_result.external_reference);
    let supplied_reference = normalize_optional(provider_reference);
    if let (Some(supplied), Some(result)) = (&supplied_reference, &result_reference)
        && supplied != result
    {
        return Err(PaymentError::ProviderInvalidResponse {
            provider_id: current.provider_id.clone(),
            operation: current.operation.clone(),
        });
    }

    Ok(supplied_reference.or(result_reference))
}

fn validate_provider_reference(value: Option<&str>) -> PaymentResult<()> {
    if let Some(value) = value {
        let value = value.trim();
        if value.is_empty() || value.len() > MAX_EXTERNAL_REFERENCE_LENGTH {
            return Err(PaymentError::ProviderInvalidResponse {
                provider_id: "unknown".to_string(),
                operation: "unknown".to_string(),
            });
        }
    }
    Ok(())
}

fn normalize_error(value: String) -> String {
    let value = value.trim();
    let value = if value.is_empty() {
        "provider operation failed"
    } else {
        value
    };
    value.chars().take(2000).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn provider_reference_boundary_is_enforced() {
        assert!(validate_provider_reference(None).is_ok());
        assert!(validate_provider_reference(Some("reference-1")).is_ok());
        assert!(validate_provider_reference(Some("   ")).is_err());
        assert!(validate_provider_reference(Some(&"r".repeat(192))).is_err());
    }

    #[test]
    fn uncertain_executing_outcome_requires_reconciliation() {
        assert!(
            ensure_transition(
                PROVIDER_OPERATION_EXECUTING,
                PROVIDER_OPERATION_RECONCILIATION_REQUIRED,
            )
            .is_ok()
        );
        assert!(
            ensure_transition(
                PROVIDER_OPERATION_PENDING,
                PROVIDER_OPERATION_RECONCILIATION_REQUIRED,
            )
            .is_err()
        );
    }
}
