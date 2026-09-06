use chrono::Utc;
use rustok_fulfillment::entities::{fulfillment, provider_operation};
use rustok_fulfillment::providers::{
    FulfillmentProviderOperationRequest, FulfillmentProviderOperationResult,
    FulfillmentProviderRegistry,
};
use rustok_fulfillment::{
    FulfillmentProviderOperationJournal, FulfillmentService, PROVIDER_OPERATION_COMMITTED,
    PROVIDER_OPERATION_ERROR, PROVIDER_OPERATION_EXECUTING,
    PROVIDER_OPERATION_RECONCILIATION_REQUIRED, PROVIDER_OPERATION_SUCCEEDED,
};
use sea_orm::{ActiveModelTrait, ColumnTrait, DatabaseConnection, EntityTrait, QueryFilter, Set};
use serde_json::Value;
use uuid::Uuid;

use super::fulfillment_orchestration::{
    FulfillmentOrchestrationError, FulfillmentOrchestrationResult,
};

pub struct FulfillmentCreateLabelRecoveryService {
    db: DatabaseConnection,
    fulfillment_provider_registry: FulfillmentProviderRegistry,
}

impl FulfillmentCreateLabelRecoveryService {
    pub fn new(db: DatabaseConnection) -> Self {
        Self {
            db,
            fulfillment_provider_registry: FulfillmentProviderRegistry::with_manual_provider(),
        }
    }

    pub fn with_provider_registry(
        mut self,
        fulfillment_provider_registry: FulfillmentProviderRegistry,
    ) -> Self {
        self.fulfillment_provider_registry = fulfillment_provider_registry;
        self
    }

    /// Retry only the carrier label side effect for an already persisted
    /// fulfillment. The original immutable request and idempotency key are reused.
    pub async fn retry(
        &self,
        tenant_id: Uuid,
        operation_id: Uuid,
    ) -> FulfillmentOrchestrationResult<crate::dto::FulfillmentResponse> {
        let journal = FulfillmentProviderOperationJournal::new(self.db.clone());
        let operation = journal.get(operation_id).await?;
        if operation.tenant_id != tenant_id || operation.operation != "create_label" {
            return Err(FulfillmentOrchestrationError::Validation(format!(
                "fulfillment provider operation {operation_id} is not a create_label operation for tenant {tenant_id}"
            )));
        }

        match operation.status.as_str() {
            PROVIDER_OPERATION_COMMITTED => {
                return FulfillmentService::new(self.db.clone())
                    .get_fulfillment(tenant_id, operation.fulfillment_id)
                    .await
                    .map_err(Into::into);
            }
            PROVIDER_OPERATION_SUCCEEDED => {
                return self.commit_provider_result(&journal, operation).await;
            }
            PROVIDER_OPERATION_RECONCILIATION_REQUIRED => {
                if operation.provider_result.is_none() {
                    return Err(FulfillmentOrchestrationError::Validation(format!(
                        "create_label operation {operation_id} has an unknown provider outcome; resolve it before retrying"
                    )));
                }
                return self.commit_provider_result(&journal, operation).await;
            }
            PROVIDER_OPERATION_EXECUTING => {
                return Err(FulfillmentOrchestrationError::Validation(format!(
                    "create_label operation {operation_id} is already executing"
                )));
            }
            PROVIDER_OPERATION_ERROR | "pending" => {}
            other => {
                return Err(FulfillmentOrchestrationError::Validation(format!(
                    "create_label operation {operation_id} cannot be retried from status `{other}`"
                )));
            }
        }

        if journal.claim_execution(operation_id).await?.is_none() {
            let current = journal.get(operation_id).await?;
            return Err(FulfillmentOrchestrationError::Validation(format!(
                "create_label operation {operation_id} is now `{}` and was not claimed for retry",
                current.status
            )));
        }

        let request: FulfillmentProviderOperationRequest = serde_json::from_value(
            operation.request_payload.clone(),
        )
        .map_err(|error| {
            FulfillmentOrchestrationError::Validation(format!(
                "create_label operation {operation_id} contains invalid request_payload: {error}"
            ))
        })?;
        if request.tenant_id != tenant_id || request.fulfillment_id != operation.fulfillment_id {
            return Err(FulfillmentOrchestrationError::Validation(format!(
                "create_label operation {operation_id} request identity does not match the journal"
            )));
        }

        let result = match self
            .fulfillment_provider_registry
            .execute_create_label(operation.provider_id.as_str(), request)
            .await
        {
            Ok(result) => result,
            Err(source) => {
                if let Err(journal_error) = journal
                    .mark_provider_error(operation_id, source.to_string())
                    .await
                {
                    return Err(FulfillmentOrchestrationError::Validation(format!(
                        "create_label retry failed for operation {operation_id}, and the journal could not record the failure: provider={source}; journal={journal_error}"
                    )));
                }
                return Err(source.into());
            }
        };
        let payload = serde_json::to_value(&result).map_err(|error| {
            FulfillmentOrchestrationError::Validation(format!(
                "failed to serialize create_label retry result for operation {operation_id}: {error}"
            ))
        })?;
        let operation = journal
            .mark_provider_succeeded(operation_id, result.external_reference.clone(), payload)
            .await?;
        self.commit_provider_result(&journal, operation).await
    }

    async fn commit_provider_result(
        &self,
        journal: &FulfillmentProviderOperationJournal,
        operation: provider_operation::Model,
    ) -> FulfillmentOrchestrationResult<crate::dto::FulfillmentResponse> {
        let result = validate_result(&operation)?;
        let fulfillment = self
            .persist_provider_result(&operation, &result)
            .await
            .map_err(|source| {
                FulfillmentOrchestrationError::Validation(format!(
                    "create_label provider result for operation {} could not be persisted locally: {source}",
                    operation.id
                ))
            });

        let fulfillment = match fulfillment {
            Ok(fulfillment) => fulfillment,
            Err(error) => {
                if operation.status == PROVIDER_OPERATION_SUCCEEDED {
                    let _ = journal
                        .mark_reconciliation_required(
                            operation.id,
                            format!(
                                "create_label provider succeeded, but local fulfillment projection failed: {error}"
                            ),
                        )
                        .await;
                }
                return Err(error);
            }
        };

        if let Err(source) = journal.mark_committed(operation.id).await {
            if operation.status == PROVIDER_OPERATION_SUCCEEDED {
                let _ = journal
                    .mark_reconciliation_required(
                        operation.id,
                        format!(
                            "create_label provider result was persisted locally, but journal commit failed: {source}"
                        ),
                    )
                    .await;
            }
            return Err(FulfillmentOrchestrationError::Validation(format!(
                "create_label operation {} could not be committed after local persistence: {source}",
                operation.id
            )));
        }

        Ok(fulfillment)
    }

    async fn persist_provider_result(
        &self,
        operation: &provider_operation::Model,
        result: &FulfillmentProviderOperationResult,
    ) -> Result<crate::dto::FulfillmentResponse, rustok_fulfillment::FulfillmentError> {
        let current = fulfillment::Entity::find_by_id(operation.fulfillment_id)
            .filter(fulfillment::Column::TenantId.eq(operation.tenant_id))
            .one(&self.db)
            .await?
            .ok_or(rustok_fulfillment::FulfillmentError::FulfillmentNotFound(
                operation.fulfillment_id,
            ))?;

        if label_operation_id(&current.metadata) != Some(operation.id) {
            let carrier_missing = current
                .carrier
                .as_deref()
                .map(|carrier| carrier.trim().is_empty())
                .unwrap_or(true);
            let mut active: fulfillment::ActiveModel = current.into();
            let current_metadata = active.metadata.clone().take().unwrap_or_default();
            active.metadata = Set(label_result_metadata(
                current_metadata,
                result,
                operation.id,
            ));
            if carrier_missing {
                active.carrier = Set(Some(result.provider_id.clone()));
            }
            if let Some(tracking_number) = result
                .tracking_number
                .as_deref()
                .map(str::trim)
                .filter(|value| !value.is_empty())
            {
                active.tracking_number = Set(Some(tracking_number.to_string()));
            }
            active.updated_at = Set(Utc::now().into());
            active.update(&self.db).await?;
        }

        FulfillmentService::new(self.db.clone())
            .get_fulfillment(operation.tenant_id, operation.fulfillment_id)
            .await
    }
}

fn validate_result(
    operation: &provider_operation::Model,
) -> FulfillmentOrchestrationResult<FulfillmentProviderOperationResult> {
    let value = operation.provider_result.clone().ok_or_else(|| {
        FulfillmentOrchestrationError::Validation(format!(
            "create_label operation {} is `{}` but has no provider_result",
            operation.id, operation.status
        ))
    })?;
    let result: FulfillmentProviderOperationResult =
        serde_json::from_value(value).map_err(|error| {
            FulfillmentOrchestrationError::Validation(format!(
                "create_label operation {} contains invalid provider_result: {error}",
                operation.id
            ))
        })?;
    if result.provider_id != operation.provider_id {
        return Err(FulfillmentOrchestrationError::Validation(format!(
            "create_label operation {} provider result `{}` does not match journal provider `{}`",
            operation.id, result.provider_id, operation.provider_id
        )));
    }
    Ok(result)
}

fn label_operation_id(metadata: &Value) -> Option<Uuid> {
    metadata
        .get("provider_operation")
        .and_then(|operation| operation.get("id"))
        .and_then(Value::as_str)
        .and_then(|value| Uuid::parse_str(value).ok())
        .or_else(|| {
            metadata
                .get("label")
                .and_then(|label| label.get("provider_operation_id"))
                .and_then(Value::as_str)
                .and_then(|value| Uuid::parse_str(value).ok())
        })
}

fn label_result_metadata(
    current: Value,
    result: &FulfillmentProviderOperationResult,
    operation_id: Uuid,
) -> Value {
    merge_metadata(
        current,
        serde_json::json!({
            "provider_operation": {
                "id": operation_id,
                "operation": "create_label",
            },
            "label": {
                "provider_operation_id": operation_id,
                "provider_id": result.provider_id,
                "external_reference": result.external_reference,
                "tracking_number": result.tracking_number,
                "provider_metadata": result.metadata,
            }
        }),
    )
}

fn merge_metadata(current: Value, patch: Value) -> Value {
    match (current, patch) {
        (Value::Object(mut current), Value::Object(patch)) => {
            for (key, value) in patch {
                current.insert(key, value);
            }
            Value::Object(current)
        }
        (_, patch) => patch,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn label_result_metadata_preserves_existing_fields_and_marks_operation() {
        let operation_id = Uuid::new_v4();
        let result = FulfillmentProviderOperationResult {
            provider_id: "carrier".to_string(),
            external_reference: Some("label-1".to_string()),
            tracking_number: Some("track-1".to_string()),
            metadata: serde_json::json!({"provider_fact": true}),
        };

        let metadata = label_result_metadata(
            serde_json::json!({"delivery_group": {"seller_id": "seller-1"}}),
            &result,
            operation_id,
        );

        assert_eq!(label_operation_id(&metadata), Some(operation_id));
        assert_eq!(metadata["provider_operation"]["operation"], "create_label");
        assert_eq!(metadata["delivery_group"]["seller_id"], "seller-1");
        assert_eq!(metadata["label"]["tracking_number"], "track-1");
        assert_eq!(
            metadata["label"]["provider_metadata"]["provider_fact"],
            true
        );
    }

    #[test]
    fn non_object_provider_metadata_cannot_replace_owner_metadata() {
        let operation_id = Uuid::new_v4();
        let result = FulfillmentProviderOperationResult {
            provider_id: "carrier".to_string(),
            external_reference: None,
            tracking_number: None,
            metadata: serde_json::json!(["opaque", "facts"]),
        };

        let metadata = label_result_metadata(
            serde_json::json!({"delivery_group": {"seller_id": "seller-1"}}),
            &result,
            operation_id,
        );

        assert_eq!(metadata["delivery_group"]["seller_id"], "seller-1");
        assert_eq!(metadata["label"]["provider_metadata"][0], "opaque");
    }
}
