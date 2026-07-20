use rustok_fulfillment::providers::{
    FulfillmentProviderOperationRequest, FulfillmentProviderOperationResult,
};
use rustok_fulfillment::{
    FulfillmentProviderOperationJournal, FulfillmentService, PROVIDER_OPERATION_COMMITTED,
    PROVIDER_OPERATION_RECONCILIATION_REQUIRED, PROVIDER_OPERATION_SUCCEEDED,
};
use sea_orm::DatabaseConnection;
use serde::de::DeserializeOwned;
use serde_json::Value;
use uuid::Uuid;

use crate::dto::{
    CancelFulfillmentInput, FulfillmentResponse, ReshipFulfillmentInput, ShipFulfillmentInput,
};

#[cfg(test)]
use crate::dto::FulfillmentItemQuantityInput;

use super::fulfillment_orchestration::{
    FulfillmentOrchestrationError, FulfillmentOrchestrationResult,
};

#[derive(Clone)]
pub struct FulfillmentReconciliationService {
    db: DatabaseConnection,
}

impl FulfillmentReconciliationService {
    pub fn new(db: DatabaseConnection) -> Self {
        Self { db }
    }

    /// Apply only the local owner transition for a provider operation whose
    /// external result was already persisted in the durable journal.
    pub async fn retry_local_persistence(
        &self,
        tenant_id: Uuid,
        operation_id: Uuid,
    ) -> FulfillmentOrchestrationResult<FulfillmentResponse> {
        let journal = FulfillmentProviderOperationJournal::new(self.db.clone());
        let operation = journal.get(operation_id).await?;
        if operation.tenant_id != tenant_id {
            return Err(FulfillmentOrchestrationError::Validation(format!(
                "fulfillment provider operation {operation_id} does not belong to tenant {tenant_id}"
            )));
        }
        if !matches!(
            operation.status.as_str(),
            PROVIDER_OPERATION_SUCCEEDED
                | PROVIDER_OPERATION_RECONCILIATION_REQUIRED
                | PROVIDER_OPERATION_COMMITTED
        ) {
            return Err(FulfillmentOrchestrationError::Validation(format!(
                "fulfillment provider operation {operation_id} is `{}` and has no confirmed provider result to reconcile",
                operation.status
            )));
        }

        let service = FulfillmentService::new(self.db.clone());
        let current = service
            .get_fulfillment(tenant_id, operation.fulfillment_id)
            .await?;
        if operation.status == PROVIDER_OPERATION_COMMITTED {
            return Ok(current);
        }
        if metadata_operation_id(&current.metadata) == Some(operation_id) {
            journal.mark_committed(operation_id).await?;
            return Ok(current);
        }

        let request: FulfillmentProviderOperationRequest =
            serde_json::from_value(operation.request_payload.clone()).map_err(|error| {
                FulfillmentOrchestrationError::Validation(format!(
                    "fulfillment provider operation {operation_id} contains invalid request_payload: {error}"
                ))
            })?;
        if request.tenant_id != tenant_id || request.fulfillment_id != operation.fulfillment_id {
            return Err(FulfillmentOrchestrationError::Validation(format!(
                "fulfillment provider operation {operation_id} request identity does not match the journal"
            )));
        }
        let provider_result: FulfillmentProviderOperationResult = operation
            .provider_result
            .clone()
            .ok_or_else(|| {
                FulfillmentOrchestrationError::Validation(format!(
                    "fulfillment provider operation {operation_id} has an unknown external outcome; resolve it before retrying local persistence"
                ))
            })
            .and_then(|value| {
                serde_json::from_value(value).map_err(|error| {
                    FulfillmentOrchestrationError::Validation(format!(
                        "fulfillment provider operation {operation_id} contains invalid provider_result: {error}"
                    ))
                })
            })?;

        let orchestration = request
            .metadata
            .get("commerce_orchestration")
            .and_then(Value::as_object)
            .ok_or_else(|| {
                FulfillmentOrchestrationError::Validation(format!(
                    "fulfillment provider operation {operation_id} lacks commerce_orchestration metadata"
                ))
            })?;
        let local_metadata = local_commit_metadata(
            request.metadata.clone(),
            provider_result.metadata,
            operation_id,
            operation.operation.as_str(),
        );

        let updated =
            match operation.operation.as_str() {
                "ship" => {
                    let input = ShipFulfillmentInput {
                        carrier: required_string(orchestration, "carrier", operation_id)?,
                        tracking_number: provider_result.tracking_number.unwrap_or(
                            required_string(orchestration, "tracking_number", operation_id)?,
                        ),
                        items: optional_field(orchestration, "items", operation_id)?,
                        metadata: local_metadata,
                    };
                    service
                        .ship_fulfillment(tenant_id, operation.fulfillment_id, input)
                        .await?
                }
                "reship" => {
                    let input = ReshipFulfillmentInput {
                        carrier: required_string(orchestration, "carrier", operation_id)?,
                        tracking_number: provider_result.tracking_number.unwrap_or(
                            required_string(orchestration, "tracking_number", operation_id)?,
                        ),
                        items: optional_field(orchestration, "items", operation_id)?,
                        metadata: local_metadata,
                    };
                    service
                        .reship_fulfillment(tenant_id, operation.fulfillment_id, input)
                        .await?
                }
                "cancel" => {
                    let input = CancelFulfillmentInput {
                        reason: optional_field(orchestration, "reason", operation_id)?,
                        metadata: local_metadata,
                    };
                    service
                        .cancel_fulfillment(tenant_id, operation.fulfillment_id, input)
                        .await?
                }
                "create_label" => {
                    journal.mark_committed(operation_id).await?;
                    return Ok(current);
                }
                other => {
                    return Err(FulfillmentOrchestrationError::Validation(format!(
                        "unsupported fulfillment reconciliation operation `{other}`"
                    )));
                }
            };

        let reconciled = journal.get(operation_id).await?;
        if reconciled.status != PROVIDER_OPERATION_COMMITTED {
            journal.mark_committed(operation_id).await?;
        }
        Ok(updated)
    }
}

fn required_string(
    object: &serde_json::Map<String, Value>,
    field: &str,
    operation_id: Uuid,
) -> FulfillmentOrchestrationResult<String> {
    object
        .get(field)
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(ToOwned::to_owned)
        .ok_or_else(|| {
            FulfillmentOrchestrationError::Validation(format!(
                "fulfillment provider operation {operation_id} lacks `{field}` in commerce_orchestration metadata"
            ))
        })
}

fn optional_field<T: DeserializeOwned>(
    object: &serde_json::Map<String, Value>,
    field: &str,
    operation_id: Uuid,
) -> FulfillmentOrchestrationResult<Option<T>> {
    match object.get(field) {
        None | Some(Value::Null) => Ok(None),
        Some(value) => serde_json::from_value(value.clone())
            .map(Some)
            .map_err(|error| {
                FulfillmentOrchestrationError::Validation(format!(
                    "fulfillment provider operation {operation_id} contains invalid `{field}` metadata: {error}"
                ))
            }),
    }
}

fn metadata_operation_id(metadata: &Value) -> Option<Uuid> {
    metadata
        .get("provider_operation")
        .and_then(|value| value.get("id"))
        .and_then(Value::as_str)
        .and_then(|value| Uuid::parse_str(value).ok())
}

fn local_commit_metadata(
    input_metadata: Value,
    provider_metadata: Value,
    operation_id: Uuid,
    operation: &str,
) -> Value {
    merge_metadata(
        merge_metadata(input_metadata, provider_metadata),
        serde_json::json!({
            "provider_operation": {
                "id": operation_id,
                "operation": operation
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
    fn reads_provider_operation_marker() {
        let operation_id = Uuid::new_v4();
        let metadata = serde_json::json!({
            "provider_operation": { "id": operation_id }
        });
        assert_eq!(metadata_operation_id(&metadata), Some(operation_id));
    }

    #[test]
    fn optional_items_metadata_round_trips() {
        let operation_id = Uuid::new_v4();
        let item = FulfillmentItemQuantityInput {
            fulfillment_item_id: Uuid::new_v4(),
            quantity: 2,
        };
        let object = serde_json::json!({"items": [item]});
        let parsed: Option<Vec<FulfillmentItemQuantityInput>> =
            optional_field(object.as_object().expect("object"), "items", operation_id)
                .expect("items");
        assert_eq!(parsed.expect("some items").len(), 1);
    }
}
