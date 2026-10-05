use chrono::{DateTime, Utc};
use sea_orm::{
    ColumnTrait, DatabaseConnection, EntityTrait, QueryFilter, QueryOrder, QuerySelect,
    sea_query::Expr,
};
use serde_json::Value;
use uuid::Uuid;

use crate::entities::provider_operation;
use crate::error::{FulfillmentError, FulfillmentResult};
use crate::providers::{
    FULFILLMENT_TRACKING_NUMBER_MAX_LEN, FulfillmentProviderOperationResult,
    validate_optional_boundary_text,
};

use super::provider_operation::{
    PROVIDER_OPERATION_ERROR, PROVIDER_OPERATION_EXECUTING,
    PROVIDER_OPERATION_RECONCILIATION_REQUIRED, PROVIDER_OPERATION_SUCCEEDED,
    validate_durable_json_payload,
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
                Expr::current_timestamp(),
            )
            .col_expr(
                provider_operation::Column::ProviderCompletedAt,
                Expr::current_timestamp(),
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
                Expr::current_timestamp(),
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
        validate_durable_json_payload(&provider_result, "provider_result")?;
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
        validate_provider_result_metadata(&typed_result.metadata)?;
        validate_optional_boundary_text(
            "external_reference",
            typed_result.external_reference.as_deref(),
            191,
        )?;
        validate_optional_boundary_text(
            "tracking_number",
            typed_result.tracking_number.as_deref(),
            FULFILLMENT_TRACKING_NUMBER_MAX_LEN,
        )?;
        let result_reference = normalize_optional(typed_result.external_reference.clone());
        let supplied_reference = normalize_optional(provider_reference);
        if let (Some(supplied), Some(result)) = (&supplied_reference, &result_reference)
            && supplied != result
        {
            return Err(FulfillmentError::Validation(
                "provider_reference does not match provider_result.external_reference".to_string(),
            ));
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
                Expr::current_timestamp(),
            )
            .col_expr(
                provider_operation::Column::ProviderCompletedAt,
                Expr::current_timestamp(),
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

fn validate_provider_result_metadata(metadata: &Value) -> FulfillmentResult<()> {
    if !metadata.is_object() {
        return Err(FulfillmentError::Validation(
            "provider_result metadata must be a JSON object".to_string(),
        ));
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

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[tokio::test]
    async fn oversized_unknown_success_result_is_rejected_before_persistence() {
        let db = setup_test_db().await;
        support::ensure_fulfillment_schema(&db).await;
        let journal = FulfillmentProviderOperationJournal::new(db.clone());
        let tenant_id = Uuid::new_v4();
        let fulfillment_id = Uuid::new_v4();
        insert_test_fulfillment(&db, tenant_id, fulfillment_id).await;

        let operation = journal
            .begin(BeginProviderOperation {
                tenant_id,
                fulfillment_id,
                operation: "ship".to_string(),
                provider_id: "carrier".to_string(),
                idempotency_key: "oversized-recovery-result".to_string(),
                request_payload: serde_json::json!({
                    "tenant_id": tenant_id,
                    "fulfillment_id": fulfillment_id,
                    "idempotency_key": "oversized-recovery-result",
                    "metadata": {}
                }),
            })
            .await
            .expect("provider operation should begin");
        journal
            .claim_execution(tenant_id, operation.id)
            .await
            .expect("claim should succeed")
            .expect("operation should be claimable");
        journal
            .mark_provider_error(tenant_id, operation.id, "outcome unknown")
            .await
            .expect("ambiguous outcome should be quarantined");

        let oversized = serde_json::json!({
            "provider_id": "carrier",
            "external_reference": "shipment-1",
            "tracking_number": "TRACK-1",
            "metadata": "x".repeat(rustok_fulfillment::MAX_PROVIDER_OPERATION_PAYLOAD_BYTES)
        });
        let recovery = FulfillmentProviderOperationRecovery::new(db.clone());
        let error = recovery
            .resolve_unknown_as_succeeded(tenant_id, operation.id, Some("shipment-1".to_string()), oversized)
            .await
            .expect_err("oversized provider result must be rejected");

        assert!(matches!(
            error,
            rustok_fulfillment::error::FulfillmentError::Validation(_)
        ));
        let current = recovery
            .resolve_unknown_as_failed(tenant_id, operation.id, "confirmed no shipment")
            .await
            .expect("recovery state must remain unchanged after rejected oversized result");
        assert_eq!(current.status, PROVIDER_OPERATION_ERROR);
    }

    #[test]
    fn provider_result_metadata_requires_object_shape() {
        assert!(
            validate_provider_result_metadata(&Value::Null).is_err(),
            "non-object provider metadata must be rejected"
        );
        assert!(
            validate_provider_result_metadata(&json!({"provider": "carrier"})).is_ok(),
            "object provider metadata should be accepted"
        );
    }
}
