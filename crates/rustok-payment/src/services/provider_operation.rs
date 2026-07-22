use chrono::Utc;
use sea_orm::{
    ActiveModelTrait, ColumnTrait, DatabaseConnection, EntityTrait, QueryFilter, QueryOrder, Set,
    sea_query::Expr,
};
use serde_json::Value;
use uuid::Uuid;

use rustok_core::generate_id;

use crate::entities::provider_operation;
use crate::error::{PaymentError, PaymentResult};

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

    pub async fn get(&self, id: Uuid) -> PaymentResult<provider_operation::Model> {
        provider_operation::Entity::find_by_id(id)
            .one(&self.db)
            .await?
            .ok_or_else(|| {
                PaymentError::Validation(format!("payment provider operation {id} not found"))
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
        id: Uuid,
    ) -> PaymentResult<Option<provider_operation::Model>> {
        let update = provider_operation::Entity::update_many()
            .col_expr(
                provider_operation::Column::Status,
                Expr::value(PROVIDER_OPERATION_EXECUTING),
            )
            .col_expr(
                provider_operation::Column::UpdatedAt,
                Expr::current_timestamp().into(),
            )
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
        self.get(id).await.map(Some)
    }

    pub async fn mark_provider_succeeded(
        &self,
        id: Uuid,
        provider_reference: Option<String>,
        provider_result: Value,
    ) -> PaymentResult<provider_operation::Model> {
        let model = self.get(id).await?;
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
        let mut active: provider_operation::ActiveModel = model.into();
        active.status = Set(PROVIDER_OPERATION_SUCCEEDED.to_string());
        active.provider_reference = Set(normalize_optional(provider_reference));
        active.provider_result = Set(Some(provider_result));
        active.error_message = Set(None);
        active.updated_at = Set(now.into());
        active.provider_completed_at = Set(Some(now.into()));
        active.update(&self.db).await.map_err(Into::into)
    }

    pub async fn mark_provider_error(
        &self,
        id: Uuid,
        error_message: impl Into<String>,
    ) -> PaymentResult<provider_operation::Model> {
        let model = self.get(id).await?;
        ensure_transition(&model.status, PROVIDER_OPERATION_ERROR)?;

        let now = Utc::now();
        let mut active: provider_operation::ActiveModel = model.into();
        active.status = Set(PROVIDER_OPERATION_ERROR.to_string());
        active.error_message = Set(Some(normalize_error(error_message.into())));
        active.updated_at = Set(now.into());
        active.update(&self.db).await.map_err(Into::into)
    }

    /// Record an operation whose external outcome cannot be safely retried.
    /// This transition is valid both after a persisted provider success and
    /// directly from `executing` when the provider may have accepted the request
    /// but the response or local success checkpoint is uncertain.
    pub async fn mark_reconciliation_required(
        &self,
        id: Uuid,
        error_message: impl Into<String>,
    ) -> PaymentResult<provider_operation::Model> {
        let model = self.get(id).await?;
        if model.status == PROVIDER_OPERATION_RECONCILIATION_REQUIRED {
            return Ok(model);
        }
        ensure_transition(&model.status, PROVIDER_OPERATION_RECONCILIATION_REQUIRED)?;

        let mut active: provider_operation::ActiveModel = model.into();
        active.status = Set(PROVIDER_OPERATION_RECONCILIATION_REQUIRED.to_string());
        active.error_message = Set(Some(normalize_error(error_message.into())));
        active.updated_at = Set(Utc::now().into());
        active.update(&self.db).await.map_err(Into::into)
    }

    pub async fn mark_committed(&self, id: Uuid) -> PaymentResult<provider_operation::Model> {
        let model = self.get(id).await?;
        if model.status == PROVIDER_OPERATION_COMMITTED {
            return Ok(model);
        }
        ensure_transition(&model.status, PROVIDER_OPERATION_COMMITTED)?;

        let provider_completion_missing = model.provider_completed_at.is_none();
        let now = Utc::now();
        let mut active: provider_operation::ActiveModel = model.into();
        active.status = Set(PROVIDER_OPERATION_COMMITTED.to_string());
        active.error_message = Set(None);
        active.updated_at = Set(now.into());
        if provider_completion_missing {
            active.provider_completed_at = Set(Some(now.into()));
        }
        active.committed_at = Set(Some(now.into()));
        active.update(&self.db).await.map_err(Into::into)
    }
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
