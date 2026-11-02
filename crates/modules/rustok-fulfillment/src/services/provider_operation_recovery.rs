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
    validate_durable_provider_payload, validate_optional_boundary_text,
    validate_provider_metadata_safety,
};

use super::provider_operation::{
    PROVIDER_OPERATION_ERROR, PROVIDER_OPERATION_EXECUTING, PROVIDER_OPERATION_PENDING,
    PROVIDER_OPERATION_RECONCILIATION_REQUIRED, PROVIDER_OPERATION_SUCCEEDED,
    validate_operation_identity, validate_provider_operation_tenant,
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
        validate_provider_operation_tenant(tenant_id)?;
        provider_operation::Entity::find()
            .filter(provider_operation::Column::TenantId.eq(tenant_id))
            .filter(super::provider_operation::unresolved_provider_effects())
            .filter(provider_operation::Column::Status.ne(PROVIDER_OPERATION_EXECUTING))
            .order_by_asc(provider_operation::Column::UpdatedAt)
            .order_by_asc(provider_operation::Column::Id)
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
        validate_provider_operation_tenant(tenant_id)?;
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
                // Historical Executing rows may already retain an outcome timestamp.
                // Quarantine is not permission to reinterpret that saved fact.
                Expr::cust("COALESCE(provider_completed_at, CURRENT_TIMESTAMP)"),
            )
            .filter(provider_operation::Column::TenantId.eq(tenant_id))
            .filter(provider_operation::Column::Id.is_in(ids))
            .filter(provider_operation::Column::Status.eq(PROVIDER_OPERATION_EXECUTING))
            .filter(provider_operation::Column::UpdatedAt.lt(stale_before))
            .exec(&self.db)
            .await?;
        Ok(result.rows_affected)
    }

    /// Explicit operator authority confirms that an unknown/legacy execution had no effect.
    /// Requeue the same immutable identity as Pending, never infer safety from provider_error.
    /// Retained references/results/commit evidence cannot be erased by this command.
    pub async fn resolve_unknown_as_failed(
        &self,
        tenant_id: Uuid,
        operation_id: Uuid,
        reason: impl Into<String>,
    ) -> FulfillmentResult<provider_operation::Model> {
        validate_operation_identity(tenant_id, operation_id)?;
        let result = provider_operation::Entity::update_many()
            .col_expr(
                provider_operation::Column::Status,
                Expr::value(PROVIDER_OPERATION_PENDING),
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
            .filter(provider_operation::Column::Status.is_in([
                PROVIDER_OPERATION_RECONCILIATION_REQUIRED,
                PROVIDER_OPERATION_ERROR,
            ]))
            .filter(provider_operation::Column::ProviderResult.is_null())
            .filter(provider_operation::Column::ProviderReference.is_null())
            .filter(provider_operation::Column::CommittedAt.is_null())
            .exec(&self.db)
            .await?;
        if result.rows_affected != 1 {
            return Err(FulfillmentError::Validation(format!(
                "fulfillment provider operation {operation_id} cannot be requeued without unresolved state and absent reference/result/commit evidence for tenant {tenant_id}"
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
        validate_operation_identity(tenant_id, operation_id)?;
        if !provider_result.is_object() {
            return Err(FulfillmentError::Validation(
                "provider_result must be a JSON object".to_string(),
            ));
        }
        validate_durable_provider_payload(&provider_result, "provider_result")?;
        let existing = self.get(tenant_id, operation_id).await?;
        if !matches!(
            existing.status.as_str(),
            PROVIDER_OPERATION_RECONCILIATION_REQUIRED | PROVIDER_OPERATION_ERROR
        ) || existing.committed_at.is_some()
        {
            return Err(FulfillmentError::Validation(format!(
                "fulfillment provider operation {operation_id} is not an unresolved unknown execution for tenant {tenant_id}"
            )));
        }
        let typed_result: FulfillmentProviderOperationResult =
            serde_json::from_value(provider_result.clone()).map_err(|error| {
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
        validate_provider_metadata_safety(&typed_result.metadata)?;
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
        validate_optional_boundary_text("provider_reference", provider_reference.as_deref(), 191)?;
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
        if existing
            .provider_reference
            .as_ref()
            .is_some_and(|retained| Some(retained) != provider_reference.as_ref())
        {
            return Err(FulfillmentError::Validation(
                "operator resolution contradicts the retained provider reference".into(),
            ));
        }
        // Late validated outcomes may already be retained in quarantine. Explicit operator
        // resolution can promote identical facts, but cannot canonicalize/replace stored JSON.
        let result_match = match existing.provider_result.as_ref() {
            Some(saved) if saved == &provider_result => {
                provider_operation::Column::ProviderResult.is_not_null()
            }
            Some(_) => {
                return Err(FulfillmentError::OwnerPort(
                    rustok_api::PortError::invariant_violation(
                        "fulfillment.provider_outcome_conflict",
                        "operator success contradicts the retained provider result",
                    ),
                ));
            }
            None => provider_operation::Column::ProviderResult.is_null(),
        };
        let reference_match = match existing.provider_reference {
            Some(value) => provider_operation::Column::ProviderReference.eq(value),
            None => provider_operation::Column::ProviderReference.is_null(),
        };
        let canonical_result = serde_json::to_value(typed_result).map_err(|error| {
            FulfillmentError::Validation(format!(
                "failed to canonicalize fulfillment provider result: {error}"
            ))
        })?;

        let result_value: sea_orm::sea_query::SimpleExpr = if existing.provider_result.is_some() {
            Expr::col(provider_operation::Column::ProviderResult)
        } else {
            Expr::value(canonical_result)
        };
        let completed: sea_orm::sea_query::SimpleExpr = if existing.provider_completed_at.is_some()
        {
            Expr::col(provider_operation::Column::ProviderCompletedAt)
        } else {
            Expr::current_timestamp()
        };
        let result = provider_operation::Entity::update_many()
            .col_expr(
                provider_operation::Column::Status,
                Expr::value(PROVIDER_OPERATION_SUCCEEDED),
            )
            .col_expr(
                provider_operation::Column::ProviderReference,
                Expr::value(provider_reference),
            )
            .col_expr(provider_operation::Column::ProviderResult, result_value)
            .col_expr(
                provider_operation::Column::ErrorMessage,
                Expr::value(Option::<String>::None),
            )
            .col_expr(
                provider_operation::Column::UpdatedAt,
                Expr::current_timestamp(),
            )
            .col_expr(provider_operation::Column::ProviderCompletedAt, completed)
            .filter(provider_operation::Column::TenantId.eq(tenant_id))
            .filter(provider_operation::Column::Id.eq(operation_id))
            .filter(provider_operation::Column::Status.eq(existing.status))
            .filter(result_match)
            .filter(reference_match)
            .filter(provider_operation::Column::CommittedAt.is_null())
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
    use crate::services::provider_operation::{
        BeginProviderOperation, FulfillmentProviderOperationJournal,
    };
    use rustok_test_utils::db::setup_test_db;
    use sea_orm::{ActiveModelTrait, ConnectionTrait, Schema, Set};
    use serde_json::json;

    async fn ensure_schema(db: &DatabaseConnection) {
        let builder = db.get_database_backend();
        let schema = Schema::new(builder);
        let statement1 = schema
            .create_table_from_entity(crate::entities::fulfillment::Entity)
            .if_not_exists()
            .to_owned();
        db.execute_raw(builder.build(&statement1))
            .await
            .expect("fulfillment table should be created");
        let statement2 = schema
            .create_table_from_entity(crate::entities::provider_operation::Entity)
            .if_not_exists()
            .to_owned();
        db.execute_raw(builder.build(&statement2))
            .await
            .expect("provider operation table should be created");
        crate::migrations::migrations()
            .into_iter()
            .find(|migration| {
                migration.name() == "m20261101_000121_enforce_provider_effect_admission"
            })
            .expect("provider admission migration is registered")
            .up(&sea_orm_migration::SchemaManager::new(db))
            .await
            .expect("provider effect guard");
    }

    async fn insert_test_fulfillment(
        db: &DatabaseConnection,
        tenant_id: Uuid,
        fulfillment_id: Uuid,
    ) {
        let now = Utc::now().fixed_offset();
        crate::entities::fulfillment::ActiveModel {
            id: Set(fulfillment_id),
            tenant_id: Set(tenant_id),
            order_id: Set(Uuid::new_v4()),
            shipping_option_id: Set(None),
            customer_id: Set(None),
            checkout_operation_id: Set(None),
            checkout_fulfillment_index: Set(None),
            checkout_plan_hash: Set(None),
            status: Set("pending".to_string()),
            carrier: Set(None),
            tracking_number: Set(None),
            delivered_note: Set(None),
            cancellation_reason: Set(None),
            metadata: Set(serde_json::json!({})),
            created_at: Set(now),
            updated_at: Set(now),
            shipped_at: Set(None),
            delivered_at: Set(None),
            cancelled_at: Set(None),
        }
        .insert(db)
        .await
        .expect("test fulfillment should be inserted");
    }

    #[tokio::test]
    async fn oversized_unknown_success_result_is_rejected_before_persistence() {
        let db = setup_test_db().await;
        ensure_schema(&db).await;
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
            "metadata": "x".repeat(MAX_PROVIDER_OPERATION_PAYLOAD_BYTES)
        });
        let recovery = FulfillmentProviderOperationRecovery::new(db.clone());
        let error = recovery
            .resolve_unknown_as_succeeded(
                tenant_id,
                operation.id,
                Some("shipment-1".to_string()),
                oversized,
            )
            .await
            .expect_err("oversized provider result must be rejected");

        assert!(matches!(error, FulfillmentError::Validation(_)));
        let current = recovery
            .resolve_unknown_as_failed(tenant_id, operation.id, "confirmed no shipment")
            .await
            .expect("recovery state must remain unchanged after rejected oversized result");
        assert_eq!(current.status, PROVIDER_OPERATION_PENDING);
    }

    #[tokio::test]
    async fn oversized_recovery_provider_reference_is_rejected_before_persistence() {
        let db = setup_test_db().await;
        ensure_schema(&db).await;
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
                idempotency_key: "oversized-recovery-reference".to_string(),
                request_payload: serde_json::json!({
                    "tenant_id": tenant_id,
                    "fulfillment_id": fulfillment_id,
                    "idempotency_key": "oversized-recovery-reference",
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

        let provider_result = serde_json::json!({
            "provider_id": "carrier",
            "external_reference": "shipment-1",
            "tracking_number": "TRACK-1",
            "metadata": {}
        });
        let recovery = FulfillmentProviderOperationRecovery::new(db.clone());
        let error = recovery
            .resolve_unknown_as_succeeded(
                tenant_id,
                operation.id,
                Some("x".repeat(192)),
                provider_result,
            )
            .await
            .expect_err("oversized provider reference must be rejected");

        assert!(matches!(error, FulfillmentError::Validation(_)));

        let current = recovery
            .resolve_unknown_as_failed(tenant_id, operation.id, "confirmed no shipment")
            .await
            .expect("recovery state must remain unchanged after rejected provider reference");
        assert_eq!(current.status, PROVIDER_OPERATION_PENDING);
        assert!(current.provider_reference.is_none());
        assert!(current.provider_result.is_none());
    }

    #[test]
    fn empty_recovery_error_is_normalized_to_safe_default() {
        assert_eq!(
            normalize_error("   ".to_string()),
            "provider operation failed"
        );
    }

    #[test]
    fn recovery_identity_rejects_nil_ids_before_db_access() {
        assert!(validate_provider_operation_tenant(Uuid::nil()).is_err());
        assert!(validate_operation_identity(Uuid::new_v4(), Uuid::nil()).is_err());
        assert!(validate_operation_identity(Uuid::nil(), Uuid::new_v4()).is_err());
        assert!(validate_operation_identity(Uuid::new_v4(), Uuid::new_v4()).is_ok());
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
    #[tokio::test]
    async fn uninvoked_claim_release_uses_current_schema_and_cannot_reopen_unknown_outcomes() {
        let db = setup_test_db().await;
        ensure_schema(&db).await;
        let tenant = Uuid::new_v4();
        let fulfillment = Uuid::new_v4();
        insert_test_fulfillment(&db, tenant, fulfillment).await;
        let journal = FulfillmentProviderOperationJournal::new(db.clone());
        let request = json!({"tenant_id":tenant,"fulfillment_id":fulfillment,"idempotency_key":"original-uninvoked-intent","metadata":{}});
        let original = journal
            .begin(BeginProviderOperation {
                tenant_id: tenant,
                fulfillment_id: fulfillment,
                operation: "ship".into(),
                provider_id: "manual".into(),
                idempotency_key: "original-uninvoked-intent".into(),
                request_payload: request.clone(),
            })
            .await
            .expect("pending intent");
        journal
            .claim_execution(tenant, original.id)
            .await
            .expect("claim")
            .expect("one claimant");
        journal
            .release_uninvoked_claim(tenant, original.id)
            .await
            .expect("no carrier was invoked");
        let released = journal.get(tenant, original.id).await.expect("released");
        assert_eq!(released.status, PROVIDER_OPERATION_PENDING);
        assert_eq!(released.idempotency_key, original.idempotency_key);
        assert_eq!(released.request_payload, request);
        journal
            .claim_execution(tenant, original.id)
            .await
            .expect("same identity")
            .expect("known no effect");
        journal
            .mark_provider_error(
                tenant,
                original.id,
                "simulated unknown outcome after invocation",
            )
            .await
            .expect("quarantine");
        assert!(
            journal
                .release_uninvoked_claim(tenant, original.id)
                .await
                .is_err()
        );
        assert!(
            journal
                .claim_execution(tenant, original.id)
                .await
                .expect("not automatically claimable")
                .is_none()
        );
        let current = journal.get(tenant, original.id).await.expect("retained");
        assert_eq!(current.status, PROVIDER_OPERATION_RECONCILIATION_REQUIRED);
        assert_eq!(current.idempotency_key, original.idempotency_key);
        assert_eq!(current.request_payload, request);
    }
    async fn retained_review_fixture(
        key: &str,
    ) -> (
        DatabaseConnection,
        Uuid,
        provider_operation::Model,
        FulfillmentProviderOperationJournal,
    ) {
        let db = setup_test_db().await;
        ensure_schema(&db).await;
        let tenant = Uuid::new_v4();
        let fulfillment = Uuid::new_v4();
        insert_test_fulfillment(&db, tenant, fulfillment).await;
        let journal = FulfillmentProviderOperationJournal::new(db.clone());
        let operation = journal.begin(BeginProviderOperation { tenant_id: tenant, fulfillment_id: fulfillment,
            operation: "ship".into(), provider_id: "manual".into(), idempotency_key: key.into(),
            request_payload: json!({"tenant_id":tenant,"fulfillment_id":fulfillment,"idempotency_key":key,"metadata":{}}),
        }).await.expect("retained review intent");
        (db, tenant, operation, journal)
    }

    fn retained_review_result(tracking: &str) -> Value {
        json!({"provider_id":"manual", "external_reference":"RETAINED-REF", "tracking_number":tracking, "metadata":{"safe":true}})
    }

    #[tokio::test]
    async fn late_provider_success_is_retained_in_quarantine_and_promoted_only_explicitly() {
        let (db, tenant, operation, journal) =
            retained_review_fixture("late-success-original").await;
        journal
            .claim_execution(tenant, operation.id)
            .await
            .expect("claim")
            .expect("one claimant");
        let quarantine = journal
            .mark_provider_error(tenant, operation.id, "stale unknown outcome")
            .await
            .expect("quarantine");
        let payload = retained_review_result("ORIGINAL");
        let late = journal
            .mark_provider_succeeded(
                tenant,
                operation.id,
                Some("RETAINED-REF".into()),
                payload.clone(),
            )
            .await
            .expect("retain late actual success");
        assert_eq!(late.status, PROVIDER_OPERATION_RECONCILIATION_REQUIRED);
        assert_eq!(late.provider_result, Some(payload.clone()));
        assert_eq!(late.provider_reference.as_deref(), Some("RETAINED-REF"));
        assert_eq!(late.provider_completed_at, quarantine.provider_completed_at);
        let replay = journal
            .mark_provider_succeeded(
                tenant,
                operation.id,
                Some("RETAINED-REF".into()),
                payload.clone(),
            )
            .await
            .expect("same evidence is read-only");
        assert_eq!(replay, late);
        journal
            .mark_provider_succeeded(
                tenant,
                operation.id,
                Some("RETAINED-REF".into()),
                retained_review_result("CONTRADICTORY"),
            )
            .await
            .expect_err("never replace retained result");
        assert_eq!(
            journal.get(tenant, operation.id).await.expect("unchanged"),
            late
        );
        let resolved = FulfillmentProviderOperationRecovery::new(db)
            .resolve_unknown_as_succeeded(
                tenant,
                operation.id,
                Some("RETAINED-REF".into()),
                payload,
            )
            .await
            .expect("explicit identical success resolution");
        assert_eq!(resolved.status, PROVIDER_OPERATION_SUCCEEDED);
        assert_eq!(resolved.provider_result, late.provider_result);
        assert_eq!(resolved.provider_reference, late.provider_reference);
        assert_eq!(resolved.provider_completed_at, late.provider_completed_at);
        assert_eq!(resolved.idempotency_key, operation.idempotency_key);
        assert_eq!(resolved.request_payload, operation.request_payload);
    }

    #[tokio::test]
    async fn retained_fulfillment_key_refuses_cross_provider_admission_and_historical_ambiguity() {
        let (db, tenant, operation, journal) =
            retained_review_fixture("provider-collision-key").await;
        journal
            .begin(BeginProviderOperation {
                tenant_id: tenant,
                fulfillment_id: operation.fulfillment_id,
                operation: operation.operation.clone(),
                provider_id: "changed-provider".into(),
                idempotency_key: operation.idempotency_key.clone(),
                request_payload: operation.request_payload.clone(),
            })
            .await
            .expect_err("same Fulfillment literal key cannot become another provider intent");
        let mut historical: provider_operation::ActiveModel = operation.clone().into();
        historical.id = Set(Uuid::new_v4());
        historical.provider_id = Set("historical-other-provider".into());
        historical
            .insert(&db)
            .await
            .expect("simulate pre-existing cross-provider Pending rows without rewriting history");
        journal
            .find_fulfillment_key(tenant, operation.fulfillment_id, &operation.idempotency_key)
            .await
            .expect_err("bounded lookup refuses ambiguity");
        journal
            .claim_execution(tenant, operation.id)
            .await
            .expect_err("Pending duplicate also fences claim");
        assert_eq!(
            journal
                .get(tenant, operation.id)
                .await
                .expect("unchanged original"),
            operation
        );
    }

    #[tokio::test]
    async fn duplicate_success_and_current_schema_cannot_erase_or_replace_retained_outcomes() {
        use sea_orm::QueryFilter;
        let (db, tenant, operation, journal) =
            retained_review_fixture("monotonic-provider-outcome").await;
        journal
            .claim_execution(tenant, operation.id)
            .await
            .expect("claim")
            .expect("one claimant");
        let payload = retained_review_result("ORIGINAL");
        let succeeded = journal
            .mark_provider_succeeded(
                tenant,
                operation.id,
                Some("RETAINED-REF".into()),
                payload.clone(),
            )
            .await
            .expect("actual success");
        assert_eq!(
            journal
                .mark_provider_succeeded(tenant, operation.id, Some("RETAINED-REF".into()), payload)
                .await
                .expect("same success is read-only"),
            succeeded
        );
        journal
            .mark_provider_succeeded(
                tenant,
                operation.id,
                Some("RETAINED-REF".into()),
                retained_review_result("CONTRADICTORY"),
            )
            .await
            .expect_err("contradictory duplicate is not a success ACK");
        provider_operation::Entity::update_many()
            .col_expr(
                provider_operation::Column::ProviderReference,
                Expr::value(Option::<String>::None),
            )
            .filter(provider_operation::Column::TenantId.eq(tenant))
            .filter(provider_operation::Column::Id.eq(operation.id))
            .exec(&db)
            .await
            .expect_err("new guard prevents erasing reference even in Succeeded");
        provider_operation::Entity::update_many()
            .col_expr(
                provider_operation::Column::ProviderResult,
                Expr::value(retained_review_result("CONTRADICTORY")),
            )
            .filter(provider_operation::Column::TenantId.eq(tenant))
            .filter(provider_operation::Column::Id.eq(operation.id))
            .exec(&db)
            .await
            .expect_err("new guard prevents replacing the retained result");
        assert_eq!(
            journal
                .get(tenant, operation.id)
                .await
                .expect("unchanged after refusals"),
            succeeded
        );
    }
}
