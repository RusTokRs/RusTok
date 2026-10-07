use chrono::Utc;
use sea_orm::{
    ActiveModelTrait, ColumnTrait, ConnectionTrait, DatabaseConnection, DatabaseTransaction,
    EntityTrait, QueryFilter, Set, sea_query::Expr,
};
use serde_json::Value;
use uuid::Uuid;

use rustok_core::generate_id;

use crate::entities::provider_operation;
use crate::error::{FulfillmentError, FulfillmentResult};
use crate::providers::{
    FULFILLMENT_TRACKING_NUMBER_MAX_LEN, FulfillmentProviderOperationResult,
    validate_durable_provider_payload, validate_optional_boundary_text, validate_provider_id,
    validate_provider_metadata_safety,
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
        self.begin_on_connection(&self.db, input).await
    }

    pub(crate) async fn begin_in_txn(
        &self,
        txn: &DatabaseTransaction,
        input: BeginProviderOperation,
    ) -> FulfillmentResult<provider_operation::Model> {
        self.begin_on_connection(txn, input).await
    }

    async fn begin_on_connection<C>(
        &self,
        conn: &C,
        input: BeginProviderOperation,
    ) -> FulfillmentResult<provider_operation::Model>
    where
        C: ConnectionTrait,
    {
        let input = normalize_begin_input(input)?;

        let fulfillment_exists =
            crate::entities::fulfillment::Entity::find_by_id(input.fulfillment_id)
                .filter(crate::entities::fulfillment::Column::TenantId.eq(input.tenant_id))
                .one(conn)
                .await?
                .is_some();
        if !fulfillment_exists {
            return Err(FulfillmentError::FulfillmentNotFound(input.fulfillment_id));
        }

        if let Some(existing) = provider_operation::Entity::find()
            .filter(provider_operation::Column::TenantId.eq(input.tenant_id))
            .filter(provider_operation::Column::ProviderId.eq(&input.provider_id))
            .filter(provider_operation::Column::IdempotencyKey.eq(&input.idempotency_key))
            .one(conn)
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
        .insert(conn)
        .await;

        match insert {
            Ok(model) => Ok(model),
            Err(insert_error) => Err(insert_error.into()),
        }
    }

    pub async fn get(
        &self,
        tenant_id: Uuid,
        operation_id: Uuid,
    ) -> FulfillmentResult<provider_operation::Model> {
        validate_operation_identity(tenant_id, operation_id)?;
        provider_operation::Entity::find_by_id(operation_id)
            .filter(provider_operation::Column::TenantId.eq(tenant_id))
            .one(&self.db)
            .await?
            .ok_or_else(|| {
                FulfillmentError::Validation(format!(
                    "fulfillment provider operation {operation_id} not found for tenant {tenant_id}"
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
        tenant_id: Uuid,
        operation_id: Uuid,
    ) -> FulfillmentResult<Option<provider_operation::Model>> {
        validate_operation_identity(tenant_id, operation_id)?;
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
            .filter(provider_operation::Column::Id.eq(operation_id))
            .filter(
                provider_operation::Column::Status
                    .is_in([PROVIDER_OPERATION_PENDING, PROVIDER_OPERATION_ERROR]),
            )
            .exec(&self.db)
            .await?;

        if update.rows_affected == 0 {
            return Ok(None);
        }
        self.get(tenant_id, operation_id).await.map(Some)
    }

    pub async fn mark_provider_succeeded(
        &self,
        tenant_id: Uuid,
        operation_id: Uuid,
        provider_reference: Option<String>,
        provider_result: Value,
    ) -> FulfillmentResult<provider_operation::Model> {
        validate_operation_identity(tenant_id, operation_id)?;
        validate_durable_provider_payload(&provider_result, "provider_result")?;
        let current = self.get(tenant_id, operation_id).await?;
        let provider_reference =
            validate_provider_result_for_operation(&current, provider_reference, &provider_result)?;
        let now = Utc::now();
        let update = provider_operation::Entity::update_many()
            .col_expr(
                provider_operation::Column::Status,
                Expr::value(PROVIDER_OPERATION_SUCCEEDED),
            )
            .col_expr(
                provider_operation::Column::ProviderReference,
                Expr::value(provider_reference.clone()),
            )
            .col_expr(
                provider_operation::Column::ProviderResult,
                Expr::value(Some(provider_result.clone())),
            )
            .col_expr(
                provider_operation::Column::ErrorMessage,
                Expr::value(Option::<String>::None),
            )
            .col_expr(provider_operation::Column::UpdatedAt, Expr::value(now))
            .col_expr(
                provider_operation::Column::ProviderCompletedAt,
                Expr::value(Some(now)),
            )
            .filter(provider_operation::Column::TenantId.eq(tenant_id))
            .filter(provider_operation::Column::Id.eq(operation_id))
            .filter(provider_operation::Column::Status.eq(PROVIDER_OPERATION_EXECUTING))
            .exec(&self.db)
            .await;

        let update = match update {
            Ok(update) => update,
            Err(source) => {
                let fallback = self
                    .mark_execution_reconciliation_required(
                        tenant_id,
                        operation_id,
                        provider_reference,
                        Some(provider_result),
                        format!(
                            "provider succeeded, but the journal could not persist the success state: {source}"
                        ),
                    )
                    .await;
                return match fallback {
                    Ok(model) => Ok(model),
                    Err(fallback_error) => {
                        tracing::error!(
                            operation_id_non_nil = !operation_id.is_nil(),
                            original_error = %source,
                            fallback_error = %fallback_error,
                            "fulfillment provider success checkpoint failed"
                        );
                        Err(FulfillmentError::Database(source))
                    }
                };
            }
        };

        if update.rows_affected == 0 {
            let current = self.get(tenant_id, operation_id).await?;
            if matches!(
                current.status.as_str(),
                PROVIDER_OPERATION_SUCCEEDED
                    | PROVIDER_OPERATION_RECONCILIATION_REQUIRED
                    | PROVIDER_OPERATION_COMMITTED
            ) {
                return Ok(current);
            }
            return Err(FulfillmentError::InvalidTransition {
                from: current.status,
                to: PROVIDER_OPERATION_SUCCEEDED.to_string(),
            });
        }

        self.get(tenant_id, operation_id).await
    }

    pub async fn mark_provider_error(
        &self,
        tenant_id: Uuid,
        operation_id: Uuid,
        error_message: impl Into<String>,
    ) -> FulfillmentResult<provider_operation::Model> {
        validate_operation_identity(tenant_id, operation_id)?;
        let error_message = normalize_error(error_message.into());
        let current = self.get(tenant_id, operation_id).await?;
        if current.status == PROVIDER_OPERATION_EXECUTING {
            return self
                .mark_execution_reconciliation_required(
                    tenant_id,
                    operation_id,
                    None,
                    None,
                    error_message,
                )
                .await;
        }
        if current.status == PROVIDER_OPERATION_RECONCILIATION_REQUIRED
            || current.status == PROVIDER_OPERATION_ERROR
        {
            return Ok(current);
        }
        Err(FulfillmentError::InvalidTransition {
            from: current.status,
            to: PROVIDER_OPERATION_RECONCILIATION_REQUIRED.to_string(),
        })
    }

    /// Record an ambiguous provider outcome directly from an executing claim.
    ///
    /// This is used when the adapter returned an error after invocation, or when
    /// persisting a successful result failed. Such operations are never made
    /// retryable automatically because the external side effect may have happened.
    pub async fn mark_execution_reconciliation_required(
        &self,
        tenant_id: Uuid,
        operation_id: Uuid,
        provider_reference: Option<String>,
        provider_result: Option<Value>,
        error_message: impl Into<String>,
    ) -> FulfillmentResult<provider_operation::Model> {
        validate_operation_identity(tenant_id, operation_id)?;
        validate_optional_boundary_text("provider_reference", provider_reference.as_deref(), 191)?;
        let mut provider_reference = normalize_optional(provider_reference);
        if let Some(provider_result) = provider_result.as_ref() {
            validate_durable_provider_payload(provider_result, "provider_result")?;
            let current = self.get(tenant_id, operation_id).await?;
            // The validated contract also recovers the reference from the provider payload when the
            // caller supplied none (the adapter errored after the call, or persisting the success
            // result failed); persisting the raw argument would strand the reconciliation row
            // without the reference an operator needs.
            provider_reference = validate_provider_result_for_operation(
                &current,
                provider_reference,
                provider_result,
            )?;
        }
        let update = provider_operation::Entity::update_many()
            .col_expr(
                provider_operation::Column::Status,
                Expr::value(PROVIDER_OPERATION_RECONCILIATION_REQUIRED),
            )
            .col_expr(
                provider_operation::Column::ProviderReference,
                Expr::value(provider_reference),
            )
            .col_expr(
                provider_operation::Column::ProviderResult,
                Expr::value(provider_result),
            )
            .col_expr(
                provider_operation::Column::ErrorMessage,
                Expr::value(Some(normalize_error(error_message.into()))),
            )
            .col_expr(
                provider_operation::Column::UpdatedAt,
                Expr::current_timestamp(),
            )
            .col_expr(
                provider_operation::Column::ProviderCompletedAt,
                Expr::current_timestamp(),
            )
            .filter(provider_operation::Column::TenantId.eq(tenant_id))
            .filter(provider_operation::Column::Id.eq(operation_id))
            .filter(provider_operation::Column::Status.eq(PROVIDER_OPERATION_EXECUTING))
            .exec(&self.db)
            .await?;

        if update.rows_affected == 0 {
            let current = self.get(tenant_id, operation_id).await?;
            if current.status == PROVIDER_OPERATION_RECONCILIATION_REQUIRED {
                return Ok(current);
            }
            return Err(FulfillmentError::InvalidTransition {
                from: current.status,
                to: PROVIDER_OPERATION_RECONCILIATION_REQUIRED.to_string(),
            });
        }

        self.get(tenant_id, operation_id).await
    }

    pub async fn mark_reconciliation_required(
        &self,
        tenant_id: Uuid,
        operation_id: Uuid,
        error_message: impl Into<String>,
    ) -> FulfillmentResult<provider_operation::Model> {
        validate_operation_identity(tenant_id, operation_id)?;
        let current = self.get(tenant_id, operation_id).await?;
        if current.status == PROVIDER_OPERATION_RECONCILIATION_REQUIRED {
            return Ok(current);
        }
        ensure_transition(&current.status, PROVIDER_OPERATION_RECONCILIATION_REQUIRED)?;

        let provider_completed_at = if current.provider_completed_at.is_none() {
            Expr::value(Some(Utc::now()))
        } else {
            Expr::col(provider_operation::Column::ProviderCompletedAt)
        };
        let update = provider_operation::Entity::update_many()
            .col_expr(
                provider_operation::Column::Status,
                Expr::value(PROVIDER_OPERATION_RECONCILIATION_REQUIRED),
            )
            .col_expr(
                provider_operation::Column::ErrorMessage,
                Expr::value(Some(normalize_error(error_message.into()))),
            )
            .col_expr(
                provider_operation::Column::UpdatedAt,
                Expr::current_timestamp(),
            )
            .col_expr(
                provider_operation::Column::ProviderCompletedAt,
                provider_completed_at,
            )
            .filter(provider_operation::Column::TenantId.eq(tenant_id))
            .filter(provider_operation::Column::Id.eq(operation_id))
            .filter(
                provider_operation::Column::Status
                    .is_in([PROVIDER_OPERATION_EXECUTING, PROVIDER_OPERATION_SUCCEEDED]),
            )
            .exec(&self.db)
            .await?;

        if update.rows_affected == 0 {
            let current = self.get(tenant_id, operation_id).await?;
            if current.status == PROVIDER_OPERATION_RECONCILIATION_REQUIRED {
                return Ok(current);
            }
            return Err(FulfillmentError::InvalidTransition {
                from: current.status,
                to: PROVIDER_OPERATION_RECONCILIATION_REQUIRED.to_string(),
            });
        }

        self.get(tenant_id, operation_id).await
    }

    pub(crate) async fn mark_committed_in_txn(
        &self,
        txn: &DatabaseTransaction,
        tenant_id: Uuid,
        operation_id: Uuid,
    ) -> FulfillmentResult<provider_operation::Model> {
        validate_operation_identity(tenant_id, operation_id)?;
        let current = provider_operation::Entity::find_by_id(operation_id)
            .filter(provider_operation::Column::TenantId.eq(tenant_id))
            .one(txn)
            .await?
            .ok_or_else(|| {
                FulfillmentError::Validation(format!(
                    "provider operation {operation_id} was not found for tenant {tenant_id}"
                ))
            })?;

        if current.status == PROVIDER_OPERATION_COMMITTED {
            return Ok(current);
        }
        ensure_transition(&current.status, PROVIDER_OPERATION_COMMITTED)?;

        let now = Utc::now();
        let provider_completed_at = if current.provider_completed_at.is_none() {
            Expr::value(Some(now))
        } else {
            Expr::col(provider_operation::Column::ProviderCompletedAt)
        };
        let update = provider_operation::Entity::update_many()
            .col_expr(
                provider_operation::Column::Status,
                Expr::value(PROVIDER_OPERATION_COMMITTED),
            )
            .col_expr(
                provider_operation::Column::ErrorMessage,
                Expr::value(Option::<String>::None),
            )
            .col_expr(provider_operation::Column::UpdatedAt, Expr::value(now))
            .col_expr(
                provider_operation::Column::ProviderCompletedAt,
                provider_completed_at,
            )
            .col_expr(
                provider_operation::Column::CommittedAt,
                Expr::value(Some(now)),
            )
            .filter(provider_operation::Column::TenantId.eq(tenant_id))
            .filter(provider_operation::Column::Id.eq(operation_id))
            .filter(provider_operation::Column::Status.is_in([
                PROVIDER_OPERATION_SUCCEEDED,
                PROVIDER_OPERATION_RECONCILIATION_REQUIRED,
            ]))
            .exec(txn)
            .await?;

        if update.rows_affected == 0 {
            let current = provider_operation::Entity::find_by_id(operation_id)
                .filter(provider_operation::Column::TenantId.eq(tenant_id))
                .one(txn)
                .await?
                .ok_or_else(|| {
                    FulfillmentError::Validation(format!(
                        "provider operation {operation_id} was not found for tenant {tenant_id}"
                    ))
                })?;
            if current.status == PROVIDER_OPERATION_COMMITTED {
                return Ok(current);
            }
            return Err(FulfillmentError::InvalidTransition {
                from: current.status,
                to: PROVIDER_OPERATION_COMMITTED.to_string(),
            });
        }

        provider_operation::Entity::find_by_id(operation_id)
            .filter(provider_operation::Column::TenantId.eq(tenant_id))
            .one(txn)
            .await?
            .ok_or_else(|| {
                FulfillmentError::Validation(format!(
                    "provider operation {operation_id} disappeared after commit"
                ))
            })
    }

    pub async fn mark_committed(
        &self,
        tenant_id: Uuid,
        operation_id: Uuid,
    ) -> FulfillmentResult<provider_operation::Model> {
        validate_operation_identity(tenant_id, operation_id)?;
        let current = self.get(tenant_id, operation_id).await?;
        if current.status == PROVIDER_OPERATION_COMMITTED {
            return Ok(current);
        }
        ensure_transition(&current.status, PROVIDER_OPERATION_COMMITTED)?;

        let now = Utc::now();
        let provider_completed_at = if current.provider_completed_at.is_none() {
            Expr::value(Some(now))
        } else {
            Expr::col(provider_operation::Column::ProviderCompletedAt)
        };
        let update = provider_operation::Entity::update_many()
            .col_expr(
                provider_operation::Column::Status,
                Expr::value(PROVIDER_OPERATION_COMMITTED),
            )
            .col_expr(
                provider_operation::Column::ErrorMessage,
                Expr::value(Option::<String>::None),
            )
            .col_expr(provider_operation::Column::UpdatedAt, Expr::value(now))
            .col_expr(
                provider_operation::Column::ProviderCompletedAt,
                provider_completed_at,
            )
            .col_expr(
                provider_operation::Column::CommittedAt,
                Expr::value(Some(now)),
            )
            .filter(provider_operation::Column::TenantId.eq(tenant_id))
            .filter(provider_operation::Column::Id.eq(operation_id))
            .filter(provider_operation::Column::Status.is_in([
                PROVIDER_OPERATION_SUCCEEDED,
                PROVIDER_OPERATION_RECONCILIATION_REQUIRED,
            ]))
            .exec(&self.db)
            .await?;

        if update.rows_affected == 0 {
            let current = self.get(tenant_id, operation_id).await?;
            if current.status == PROVIDER_OPERATION_COMMITTED {
                return Ok(current);
            }
            return Err(FulfillmentError::InvalidTransition {
                from: current.status,
                to: PROVIDER_OPERATION_COMMITTED.to_string(),
            });
        }

        self.get(tenant_id, operation_id).await
    }
}

fn validate_provider_result_for_operation(
    current: &provider_operation::Model,
    provider_reference: Option<String>,
    provider_result: &Value,
) -> FulfillmentResult<Option<String>> {
    let typed_result: FulfillmentProviderOperationResult =
        serde_json::from_value(provider_result.clone()).map_err(|error| {
            FulfillmentError::ProviderResultInvalid(format!(
                "provider_result does not match the fulfillment provider contract: {error}"
            ))
        })?;

    validate_provider_id(&typed_result.provider_id).map_err(provider_result_invalid)?;
    validate_optional_boundary_text(
        "external_reference",
        typed_result.external_reference.as_deref(),
        191,
    )
    .map_err(provider_result_invalid)?;
    validate_optional_boundary_text(
        "tracking_number",
        typed_result.tracking_number.as_deref(),
        FULFILLMENT_TRACKING_NUMBER_MAX_LEN,
    )
    .map_err(provider_result_invalid)?;

    if typed_result.provider_id != current.provider_id {
        return Err(FulfillmentError::ProviderResultInvalid(format!(
            "provider_result provider_id {} does not match journal provider {}",
            typed_result.provider_id, current.provider_id
        )));
    }

    if !typed_result.metadata.is_object() {
        return Err(FulfillmentError::ProviderResultInvalid(
            "provider_result metadata must be a JSON object".to_string(),
        ));
    }
    validate_provider_metadata_safety(&typed_result.metadata)?;
    validate_optional_boundary_text("provider_reference", provider_reference.as_deref(), 191)
        .map_err(provider_result_invalid)?;

    let result_reference = normalize_optional(typed_result.external_reference.clone());
    let supplied_reference = normalize_optional(provider_reference);
    if let (Some(supplied), Some(result)) = (&supplied_reference, &result_reference)
        && supplied != result
    {
        return Err(FulfillmentError::ProviderResultInvalid(
            "provider_reference does not match provider_result.external_reference".to_string(),
        ));
    }

    Ok(supplied_reference.or(result_reference))
}

fn provider_result_invalid(error: FulfillmentError) -> FulfillmentError {
    match error {
        FulfillmentError::Validation(message) => FulfillmentError::ProviderResultInvalid(message),
        other => other,
    }
}
pub(crate) fn validate_provider_operation_tenant(tenant_id: Uuid) -> FulfillmentResult<()> {
    if tenant_id.is_nil() {
        return Err(FulfillmentError::Validation(
            "tenant_id must not be nil".to_string(),
        ));
    }
    Ok(())
}

pub(crate) fn validate_operation_identity(tenant_id: Uuid, operation_id: Uuid) -> FulfillmentResult<()> {
    if tenant_id.is_nil() || operation_id.is_nil() {
        return Err(FulfillmentError::Validation(
            "provider operation requires non-nil tenant_id and operation_id".to_string(),
        ));
    }
    Ok(())
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
    validate_durable_provider_payload(&input.request_payload, "request_payload")?;
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
        PROVIDER_OPERATION_RECONCILIATION_REQUIRED => {
            matches!(
                from,
                PROVIDER_OPERATION_EXECUTING | PROVIDER_OPERATION_SUCCEEDED
            )
        }
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
    let value = if value.is_empty() {
        "provider operation failed"
    } else {
        value
    };
    if value.len() <= 2000 {
        value.to_string()
    } else {
        value.chars().take(2000).collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::providers::MAX_PROVIDER_OPERATION_PAYLOAD_BYTES;

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
    fn provider_operation_identity_rejects_nil_ids() {
        assert!(validate_provider_operation_tenant(Uuid::nil()).is_err());
        assert!(validate_operation_identity(Uuid::new_v4(), Uuid::nil()).is_err());
        assert!(validate_operation_identity(Uuid::nil(), Uuid::new_v4()).is_err());
        assert!(validate_operation_identity(Uuid::new_v4(), Uuid::new_v4()).is_ok());
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

    #[test]
    fn provider_operation_allows_executing_to_reconciliation() {
        assert!(
            ensure_transition(
                PROVIDER_OPERATION_EXECUTING,
                PROVIDER_OPERATION_RECONCILIATION_REQUIRED,
            )
            .is_ok()
        );
    }

    #[test]
    fn provider_operation_payloads_are_bounded() {
        let oversized =
            serde_json::json!({"metadata": "x".repeat(MAX_PROVIDER_OPERATION_PAYLOAD_BYTES)});
        assert!(validate_durable_provider_payload(&oversized, "provider_result").is_err());
    }

    #[test]
    fn provider_operation_payload_limit_accepts_small_json() {
        let payload = serde_json::json!({"provider": "carrier", "tracking_number": "track-1"});
        assert!(validate_durable_provider_payload(&payload, "provider_result").is_ok());
    }

    #[test]
    fn empty_provider_error_is_normalized_to_safe_default() {
        assert_eq!(
            normalize_error("   ".to_string()),
            "provider operation failed"
        );
    }
}
