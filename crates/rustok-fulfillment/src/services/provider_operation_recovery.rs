use chrono::{DateTime, Utc};
use sea_orm::{
    sea_query::Expr, ColumnTrait, DatabaseConnection, EntityTrait, QueryFilter, QueryOrder,
    QuerySelect,
};
use serde_json::Value;
use uuid::Uuid;

use crate::entities::provider_operation;
use crate::error::{FulfillmentError, FulfillmentResult};

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
                provider_operation::Column::Status
                    .eq(PROVIDER_OPERATION_RECONCILIATION_REQUIRED),
            )
            .order_by_asc(provider_operation::Column::UpdatedAt)
            .limit(limit.clamp(1, 500))
            .all(&self.db)
            .await
            .map_err(Into::into)
    }

    /// Move stale executions into a fail-closed reconciliation state.
    ///
    /// The provider may have completed the side effect before the process crashed,
    /// so stale executions are never made retryable automatically.
    pub async fn quarantine_stale_executing(
        &self,
        stale_before: DateTime<Utc>,
        limit: u64,
    ) -> FulfillmentResult<u64> {
        let stale_before = stale_before.fixed_offset();
        let ids = provider_operation::Entity::find()
            .select_only()
            .column(provider_operation::Column::Id)
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
            .filter(provider_operation::Column::Id.eq(operation_id))
            .filter(
                provider_operation::Column::Status
                    .eq(PROVIDER_OPERATION_RECONCILIATION_REQUIRED),
            )
            .filter(provider_operation::Column::ProviderResult.is_null())
            .exec(&self.db)
            .await?;
        if result.rows_affected != 1 {
            return Err(FulfillmentError::Validation(format!(
                "fulfillment provider operation {operation_id} is not an unresolved unknown execution"
            )));
        }
        self.get(operation_id).await
    }

    /// Confirm provider success for an unknown execution. A subsequent retry of
    /// the owner operation will reuse this persisted result and apply only the
    /// local fulfillment transition.
    pub async fn resolve_unknown_as_succeeded(
        &self,
        operation_id: Uuid,
        provider_reference: Option<String>,
        provider_result: Value,
    ) -> FulfillmentResult<provider_operation::Model> {
        if !provider_result.is_object() {
            return Err(FulfillmentError::Validation(
                "provider_result must be a JSON object".to_string(),
            ));
        }
        let result = provider_operation::Entity::update_many()
            .col_expr(
                provider_operation::Column::Status,
                Expr::value(PROVIDER_OPERATION_SUCCEEDED),
            )
            .col_expr(
                provider_operation::Column::ProviderReference,
                Expr::value(normalize_optional(provider_reference)),
            )
            .col_expr(
                provider_operation::Column::ProviderResult,
                Expr::value(provider_result),
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
            .filter(provider_operation::Column::Id.eq(operation_id))
            .filter(
                provider_operation::Column::Status
                    .eq(PROVIDER_OPERATION_RECONCILIATION_REQUIRED),
            )
            .filter(provider_operation::Column::ProviderResult.is_null())
            .exec(&self.db)
            .await?;
        if result.rows_affected != 1 {
            return Err(FulfillmentError::Validation(format!(
                "fulfillment provider operation {operation_id} is not an unresolved unknown execution"
            )));
        }
        self.get(operation_id).await
    }

    async fn get(&self, operation_id: Uuid) -> FulfillmentResult<provider_operation::Model> {
        provider_operation::Entity::find_by_id(operation_id)
            .one(&self.db)
            .await?
            .ok_or_else(|| {
                FulfillmentError::Validation(format!(
                    "fulfillment provider operation {operation_id} not found"
                ))
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
