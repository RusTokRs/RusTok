use chrono::{DateTime, Utc};
use sea_orm::{
    ColumnTrait, DatabaseConnection, EntityTrait, QueryFilter, QueryOrder, QuerySelect,
    sea_query::Expr,
};
use serde_json::Value;
use uuid::Uuid;

use crate::entities::provider_operation;
use crate::error::{FulfillmentError, FulfillmentResult};
use crate::providers::FulfillmentProviderOperationResult;

use super::provider_operation::{
    PROVIDER_OPERATION_ERROR, PROVIDER_OPERATION_EXECUTING,
    PROVIDER_OPERATION_RECONCILIATION_REQUIRED, PROVIDER_OPERATION_SUCCEEDED,
};

#[derive(Clone)]
pub struct FulfillmentProviderOperationRecovery {
    db: DatabaseConnection,
}

impl FulfillmentProviderOperationRecovery {
    pub fn new(db: DatabaseConnection) -> Self {
        Self { db }
    }

    pub async fn list_reconciliation_required(
        &self,
        tenant_id: Uuid,
        limit: u64,
    ) -> FulfillmentResult<Vec<provider_operation::Model>> {
        provider_operation::Entity::find()
            .filter(provider_operation::Column::TenantId.eq(tenant_id))
            .filter(
                provider_operation::Column::Status.eq(PROVIDER_OPERATION_RECONCILIATION_REQUIRED),
            )
            .order_by_asc(provider_operation::Column::UpdatedAt)
            .limit(limit.clamp(1, 500))
            .all(&self.db)
            .await
            .map_err(Into::into)
    }

    /// Move stale executions for one tenant into a fail-closed reconciliation state.
    ///
    /// The provider may have completed the side effect before the process crashed,
    /// so stale executions are never made retryable automatically.
    pub async fn quarantine_stale_executing(
        &self,
        tenant_id: Uuid,
        stale_before: DateTime<Utc>,
        limit: u64,
    ) -> FulfillmentResult<u64> {
        let stale_before = stale_before.fixed_offset();
        let ids = provider_operation::Entity::find()
            .select_only()
            .column(provider_operation::Column::Id)
            .filter(provider_operation::Column::TenantId.eq(tenant_id))
            .filter(provider_operation::Column::Status.eq(PROVIDER_OPERATION_EXECUTING))
            .filter(provider_operation::Column::UpdatedAt.lt(stale_before))
            .order_by_asc(provider_operation::Column::UpdatedAt)
            .limit(limit.clamp(1, 500))
            .into_tuple::<Uuid>()
            .all(&self.db)
            .await?;
        if ids.is_empty() {
            return Ok(0);
        }

        let result = provider_operation::Entity::update_many()
            .col_expr(
                provider_operation::Column::Status,
                Expr::value(PROVIDER_OPERATION_RECONCILIATION_REQUIRED),
            )
            .col_expr(
                provider_operation::Column::ErrorMessage,
                Expr::value(Some(
                    "provider execution lease expired; external outcome is unknown and requires reconciliation"
                        .to_string(),
                )),
            )
            .col_expr(
                provider_operation::Column::UpdatedAt,
                Expr::current_timestamp().into(),
            )
            .col_expr(
                provider_operation::Column::ProviderCompletedAt,
                Expr::current_timestamp().into(),
            )
            .filter(provider_operation::Column::TenantId.eq(tenant_id))
            .filter(provider_operation::Column::Id.is_in(ids))
            .filter(provider_operation::Column::Status.eq(PROVIDER_OPERATION_EXECUTING))
            .filter(provider_operation::Column::UpdatedAt.lt(stale_before))
            .exec(&self.db)
            .await?;
        Ok(result.rows_affected)
    }

    /// Confirm that an unknown external execution did not happen, making the
    /// operation retryable under the same idempotency key.
    pub async fn resolve_unknown_as_failed(
        &self,
        tenant_id: Uuid,
        operation_id: Uuid,
        reason: impl Into<String>,
    ) -> FulfillmentResult<provider_operation::Model> {
        let result = provider_operation::Entity::update_many()
            .col_expr(
                provider_operation::Column::Status,
                Expr::value(PROVIDER_OPERATION_ERROR),
            )
            .col_expr(
                provider_operation::Column::ErrorMessage,
                Expr::value(Some(normalize_error(reason.into()))),
            )
            .col_expr(
                provider_operation::Column::UpdatedAt,
                Expr::current_timestamp().into(),
            )
            .col_expr(
                provider_operation::Column::ProviderCompletedAt,
                Expr::value(Option::<chrono::DateTime<chrono::FixedOffset>>::None),
            )
            .filter(provider_operation::Column::TenantId.eq(tenant_id))
            .filter(provider_operation::Column::Id.eq(operation_id))
            .filter(
                provider_operation::Column::Status.eq(PROVIDER_OPERATION_RECONCILIATION_REQUIRED),
            )
            .filter(provider_operation::Column::ProviderResult.is_null())
            .exec(&self.db)
            .await?;
        if result.rows_affected != 1 {
            return Err(FulfillmentError::Validation(format!(
                "fulfillment provider operation {operation_id} is not an unresolved unknown execution for tenant {tenant_id}"
            )));
        }
        self.get(tenant_id, operation_id).await
    }

    /// Confirm provider success for an unknown execution. A subsequent retry of
    /// the owner operation will reuse this persisted result and apply only the
    /// local fulfillment transition.
    pub async fn resolve_unknown_as_succeeded(
        &self,
        tenant_id: Uuid,
        operation_id: Uuid,
        provider_reference: Option<String>,
        provider_result: Value,
    ) -> FulfillmentResult<provider_operation::Model> {
        if !provider_result.is_object() {
            return Err(FulfillmentError::Validation(
                "provider_result must be a JSON object".to_string(),
            ));
        }
        let existing = self.get(tenant_id, operation_id).await?;
        if existing.status != PROVIDER_OPERATION_RECONCILIATION_REQUIRED
            || existing.provider_result.is_some()
        {
            return Err(FulfillmentError::Validation(format!(
                "fulfillment provider operation {operation_id} is not an unresolved unknown execution for tenant {tenant_id}"
            )));
        }
        let typed_result: FulfillmentProviderOperationResult =
            serde_json::from_value(provider_result).map_err(|error| {
                FulfillmentError::Validation(format!(
                    "provider_result does not match the fulfillment provider contract: {error}"
                ))
            })?;
        if typed_result.provider_id != existing.provider_id {
            return Err(FulfillmentError::Validation(format!(
                "provider_result provider_id `{}` does not match journal provider `{}`",
                typed_result.provider_id, existing.provider_id
            )));
        }
        validate_optional_boundary_text(
            "external_reference",
            typed_result.external_reference.as_deref(),
            191,
        )?;
        validate_optional_boundary_text(
            "tracking_number",
            typed_result.tracking_number.as_deref(),
            191,
        )?;
        let result_reference = normalize_optional(typed_result.external_reference.clone());
        let supplied_reference = normalize_optional(provider_reference);
        if let (Some(supplied), Some(result)) = (&supplied_reference, &result_reference) {
            if supplied != result {
                return Err(FulfillmentError::Validation(
                    "provider_reference does not match provider_result.external_reference"
                        .to_string(),
                ));
            }
        }
        let provider_reference = supplied_reference.or(result_reference);
        let canonical_result = serde_json::to_value(typed_result).map_err(|error| {
            FulfillmentError::Validation(format!(
                "failed to canonicalize fulfillment provider result: {error}"
            ))
        })?;

        let result = provider_operation::Entity::update_many()
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
                Expr::value(canonical_result),
            )
            .col_expr(
                provider_operation::Column::ErrorMessage,
                Expr::value(Option::<String>::None),
            )
            .col_expr(
                provider_operation::Column::UpdatedAt,
                Expr::current_timestamp().into(),
            )
            .col_expr(
                provider_operation::Column::ProviderCompletedAt,
                Expr::current_timestamp().into(),
            )
            .filter(provider_operation::Column::TenantId.eq(tenant_id))
            .filter(provider_operation::Column::Id.eq(operation_id))
            .filter(
                provider_operation::Column::Status.eq(PROVIDER_OPERATION_RECONCILIATION_REQUIRED),
            )
            .filter(provider_operation::Column::ProviderResult.is_null())
            .exec(&self.db)
            .await?;
        if result.rows_affected != 1 {
            return Err(FulfillmentError::Validation(format!(
                "fulfillment provider operation {operation_id} changed while it was being reconciled"
            )));
        }
        self.get(tenant_id, operation_id).await
    }

    async fn get(
        &self,
        tenant_id: Uuid,
        operation_id: Uuid,
    ) -> FulfillmentResult<provider_operation::Model> {
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
}

fn validate_optional_boundary_text(
    field: &str,
    value: Option<&str>,
    max: usize,
) -> FulfillmentResult<()> {
    if let Some(value) = value {
        if value.trim().is_empty() || value.len() > max {
            return Err(FulfillmentError::Validation(format!(
                "{field} must be non-empty and at most {max} characters when provided"
            )));
        }
    }
    Ok(())
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
