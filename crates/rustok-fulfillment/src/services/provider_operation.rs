use chrono::Utc;
use sea_orm::{
    ActiveModelTrait, ColumnTrait, DatabaseConnection, EntityTrait, QueryFilter, Set,
    sea_query::Expr,
};
use serde_json::Value;
use uuid::Uuid;

use rustok_core::generate_id;

use crate::entities::provider_operation;
use crate::error::{FulfillmentError, FulfillmentResult};

pub const PROVIDER_OPERATION_PENDING: &str = "pending";
pub const PROVIDER_OPERATION_EXECUTING: &str = "executing";
pub const PROVIDER_OPERATION_SUCCEEDED: &str = "provider_succeeded";
pub const PROVIDER_OPERATION_ERROR: &str = "provider_error";
pub const PROVIDER_OPERATION_RECONCILIATION_REQUIRED: &str = "reconciliation_required";
pub const PROVIDER_OPERATION_COMMITTED: &str = "committed";

#[derive(Clone, Debug)]
pub struct BeginProviderOperation {
    pub tenant_id: Uuid,
    pub fulfillment_id: Uuid,
    pub operation: String,
    pub provider_id: String,
    pub idempotency_key: String,
    pub request_payload: Value,
}

#[derive(Clone)]
pub struct FulfillmentProviderOperationJournal {
    db: DatabaseConnection,
}

impl FulfillmentProviderOperationJournal {
    pub fn new(db: DatabaseConnection) -> Self {
        Self { db }
    }

    pub async fn begin(
        &self,
        input: BeginProviderOperation,
    ) -> FulfillmentResult<provider_operation::Model> {
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
            fulfillment_id: Set(input.fulfillment_id),
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

    pub async fn get(&self, id: Uuid) -> FulfillmentResult<provider_operation::Model> {
        provider_operation::Entity::find_by_id(id)
            .one(&self.db)
            .await?
            .ok_or_else(|| {
                FulfillmentError::Validation(format!(
                    "fulfillment provider operation {id} not found"
                ))
            })
    }

    pub async fn find_by_key(
        &self,
        tenant_id: Uuid,
        provider_id: &str,
        idempotency_key: &str,
    ) -> FulfillmentResult<Option<provider_operation::Model>> {
        provider_operation::Entity::find()
            .filter(provider_operation::Column::TenantId.eq(tenant_id))
            .filter(provider_operation::Column::ProviderId.eq(provider_id))
            .filter(provider_operation::Column::IdempotencyKey.eq(idempotency_key))
            .one(&self.db)
            .await
            .map_err(Into::into)
    }

    pub async fn claim_execution(
        &self,
        id: Uuid,
    ) -> FulfillmentResult<Option<provider_operation::Model>> {
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
    ) -> FulfillmentResult<provider_operation::Model> {
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

        let provider_reference = normalize_optional(provider_reference);
        let now = Utc::now();
        let mut active: provider_operation::ActiveModel = model.into();
        active.status = Set(PROVIDER_OPERATION_SUCCEEDED.to_string());
        active.provider_reference = Set(provider_reference.clone());
        active.provider_result = Set(Some(provider_result.clone()));
        active.error_message = Set(None);
        active.updated_at = Set(now.into());
        active.provider_completed_at = Set(Some(now.into()));
        match active.update(&self.db).await {
            Ok(model) => Ok(model),
            Err(source) => {
                let message = format!(
                    "provider succeeded, but the journal could not persist the success state: {source}"
                );
                self.mark_execution_reconciliation_required(
                    id,
                    provider_reference,
                    Some(provider_result),
                    message,
                )
                .await
                .map_err(|fallback| {
                    FulfillmentError::Validation(format!(
                        "failed to persist provider success for operation {id}: {source}; fallback reconciliation write also failed: {fallback}"
                    ))
                })
            }
        }
    }

    pub async fn mark_provider_error(
        &self,
        id: Uuid,
        error_message: impl Into<String>,
    ) -> FulfillmentResult<provider_operation::Model> {
        let message = normalize_error(error_message.into());
        let model = self.get(id).await?;
        if model.status == PROVIDER_OPERATION_EXECUTING {
            return self
                .mark_execution_reconciliation_required(id, None, None, message)
                .await;
        }
        if model.status != PROVIDER_OPERATION_ERROR {
            return Err(FulfillmentError::InvalidTransition {
                from: model.status,
                to: PROVIDER_OPERATION_ERROR.to_string(),
            });
        }

        let mut active: provider_operation::ActiveModel = model.into();
        active.error_message = Set(Some(message));
        active.updated_at = Set(Utc::now().into());
        active.update(&self.db).await.map_err(Into::into)
    }

    /// Record an ambiguous provider outcome directly from an executing claim.
    ///
    /// This is used when the adapter returned an error after invocation, or when
    /// persisting a successful result failed. Such operations are never made
    /// retryable automatically because the external side effect may have happened.
    pub async fn mark_execution_reconciliation_required(
        &self,
        id: Uuid,
        provider_reference: Option<String>,
        provider_result: Option<Value>,
        error_message: impl Into<String>,
    ) -> FulfillmentResult<provider_operation::Model> {
        let model = self.get(id).await?;
        if model.status == PROVIDER_OPERATION_RECONCILIATION_REQUIRED {
            return Ok(model);
        }
        if model.status != PROVIDER_OPERATION_EXECUTING {
            return Err(FulfillmentError::InvalidTransition {
                from: model.status,
                to: PROVIDER_OPERATION_RECONCILIATION_REQUIRED.to_string(),
            });
        }

        let now = Utc::now();
        let mut active: provider_operation::ActiveModel = model.into();
        active.status = Set(PROVIDER_OPERATION_RECONCILIATION_REQUIRED.to_string());
        active.provider_reference = Set(normalize_optional(provider_reference));
        active.provider_result = Set(provider_result);
        active.error_message = Set(Some(normalize_error(error_message.into())));
        active.updated_at = Set(now.into());
        active.provider_completed_at = Set(Some(now.into()));
        active.update(&self.db).await.map_err(Into::into)
    }

    pub async fn mark_reconciliation_required(
        &self,
        id: Uuid,
        error_message: impl Into<String>,
    ) -> FulfillmentResult<provider_operation::Model> {
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

    pub async fn mark_committed(&self, id: Uuid) -> FulfillmentResult<provider_operation::Model> {
        let model = self.get(id).await?;
        if model.status == PROVIDER_OPERATION_COMMITTED {
            return Ok(model);
        }
        ensure_transition(&model.status, PROVIDER_OPERATION_COMMITTED)?;

        let now = Utc::now();
        let mut active: provider_operation::ActiveModel = model.into();
        active.status = Set(PROVIDER_OPERATION_COMMITTED.to_string());
        active.error_message = Set(None);
        active.updated_at = Set(now.into());
        active.committed_at = Set(Some(now.into()));
        active.update(&self.db).await.map_err(Into::into)
    }
}

fn normalize_begin_input(
    mut input: BeginProviderOperation,
) -> FulfillmentResult<BeginProviderOperation> {
    input.operation = input.operation.trim().to_ascii_lowercase();
    if !matches!(
        input.operation.as_str(),
        "create_label" | "ship" | "reship" | "cancel"
    ) {
        return Err(FulfillmentError::Validation(format!(
            "unsupported fulfillment provider operation `{}`",
            input.operation
        )));
    }

    input.provider_id = input.provider_id.trim().to_string();
    input.idempotency_key = input.idempotency_key.trim().to_string();
    if input.tenant_id.is_nil() || input.fulfillment_id.is_nil() {
        return Err(FulfillmentError::Validation(
            "provider operation requires non-nil tenant_id and fulfillment_id".to_string(),
        ));
    }
    if input.provider_id.is_empty() || input.provider_id.len() > 100 {
        return Err(FulfillmentError::Validation(
            "provider_id must contain 1 to 100 characters".to_string(),
        ));
    }
    if input.idempotency_key.is_empty() || input.idempotency_key.len() > 191 {
        return Err(FulfillmentError::Validation(
            "idempotency_key must contain 1 to 191 characters".to_string(),
        ));
    }
    if !input.request_payload.is_object() {
        return Err(FulfillmentError::Validation(
            "provider operation request_payload must be a JSON object".to_string(),
        ));
    }
    Ok(input)
}

fn ensure_same_request(
    existing: &provider_operation::Model,
    input: &BeginProviderOperation,
) -> FulfillmentResult<()> {
    if existing.fulfillment_id != input.fulfillment_id
        || existing.operation != input.operation
        || existing.request_payload != input.request_payload
    {
        return Err(FulfillmentError::Validation(format!(
            "idempotency key `{}` is already bound to a different fulfillment provider request",
            input.idempotency_key
        )));
    }
    Ok(())
}

fn ensure_transition(from: &str, to: &str) -> FulfillmentResult<()> {
    let allowed = match to {
        PROVIDER_OPERATION_EXECUTING => {
            matches!(from, PROVIDER_OPERATION_PENDING | PROVIDER_OPERATION_ERROR)
        }
        PROVIDER_OPERATION_SUCCEEDED => from == PROVIDER_OPERATION_EXECUTING,
        PROVIDER_OPERATION_ERROR => {
            matches!(
                from,
                PROVIDER_OPERATION_EXECUTING | PROVIDER_OPERATION_ERROR
            )
        }
        PROVIDER_OPERATION_RECONCILIATION_REQUIRED => from == PROVIDER_OPERATION_SUCCEEDED,
        PROVIDER_OPERATION_COMMITTED => matches!(
            from,
            PROVIDER_OPERATION_SUCCEEDED | PROVIDER_OPERATION_RECONCILIATION_REQUIRED
        ),
        _ => false,
    };
    if allowed {
        Ok(())
    } else {
        Err(FulfillmentError::InvalidTransition {
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
    if value.len() <= 2000 {
        value.to_string()
    } else {
        value.chars().take(2000).collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn provider_operation_transition_matrix_is_fail_closed() {
        assert!(
            ensure_transition(PROVIDER_OPERATION_PENDING, PROVIDER_OPERATION_EXECUTING).is_ok()
        );
        assert!(ensure_transition(PROVIDER_OPERATION_ERROR, PROVIDER_OPERATION_EXECUTING).is_ok());
        assert!(ensure_transition(PROVIDER_OPERATION_EXECUTING, PROVIDER_OPERATION_ERROR).is_ok());
        assert!(
            ensure_transition(PROVIDER_OPERATION_EXECUTING, PROVIDER_OPERATION_SUCCEEDED).is_ok()
        );
        assert!(
            ensure_transition(PROVIDER_OPERATION_SUCCEEDED, PROVIDER_OPERATION_COMMITTED).is_ok()
        );
        assert!(
            ensure_transition(
                PROVIDER_OPERATION_SUCCEEDED,
                PROVIDER_OPERATION_RECONCILIATION_REQUIRED
            )
            .is_ok()
        );
        assert!(
            ensure_transition(PROVIDER_OPERATION_PENDING, PROVIDER_OPERATION_SUCCEEDED).is_err()
        );
        assert!(
            ensure_transition(PROVIDER_OPERATION_PENDING, PROVIDER_OPERATION_COMMITTED).is_err()
        );
    }

    #[test]
    fn provider_operation_input_requires_object_payload() {
        let error = normalize_begin_input(BeginProviderOperation {
            tenant_id: Uuid::new_v4(),
            fulfillment_id: Uuid::new_v4(),
            operation: "ship".to_string(),
            provider_id: "manual".to_string(),
            idempotency_key: "key".to_string(),
            request_payload: Value::Null,
        })
        .expect_err("non-object payload must be rejected");
        assert!(matches!(error, FulfillmentError::Validation(_)));
    }
}
