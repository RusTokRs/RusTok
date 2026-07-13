use rustok_fulfillment::providers::{
    FulfillmentProviderOperationRequest, FulfillmentProviderOperationResult,
    FulfillmentProviderRegistry, MANUAL_FULFILLMENT_PROVIDER_ID,
};
use rustok_fulfillment::{
    BeginProviderOperation, FulfillmentError, FulfillmentProviderOperationJournal,
    FulfillmentService, PROVIDER_OPERATION_COMMITTED, PROVIDER_OPERATION_EXECUTING,
    PROVIDER_OPERATION_RECONCILIATION_REQUIRED, PROVIDER_OPERATION_SUCCEEDED,
};
use sea_orm::DatabaseConnection;
use serde::Serialize;
use serde_json::Value;
use uuid::Uuid;
use validator::Validate;

use crate::dto::{
    CancelFulfillmentInput, FulfillmentResponse, ReshipFulfillmentInput, ShipFulfillmentInput,
};

use super::fulfillment_orchestration::{
    FulfillmentOrchestrationError, FulfillmentOrchestrationResult,
};

pub struct JournaledFulfillmentOrchestrationService {
    db: DatabaseConnection,
    fulfillment_provider_registry: FulfillmentProviderRegistry,
}

impl JournaledFulfillmentOrchestrationService {
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

    pub async fn ship_fulfillment(
        &self,
        tenant_id: Uuid,
        fulfillment_id: Uuid,
        input: ShipFulfillmentInput,
    ) -> FulfillmentOrchestrationResult<FulfillmentResponse> {
        input
            .validate()
            .map_err(|error| FulfillmentOrchestrationError::Validation(error.to_string()))?;
        let service = FulfillmentService::new(self.db.clone());
        let current = service.get_fulfillment(tenant_id, fulfillment_id).await?;
        if !matches!(current.status.as_str(), "pending" | "shipped") {
            return Err(FulfillmentError::InvalidTransition {
                from: current.status,
                to: "shipped".to_string(),
            }
            .into());
        }

        let provider_id = self.provider_id_for_fulfillment(tenant_id, &current).await?;
        let request_metadata = merge_metadata(
            input.metadata.clone(),
            serde_json::json!({
                "commerce_orchestration": {
                    "operation": "ship",
                    "carrier": input.carrier,
                    "tracking_number": input.tracking_number,
                    "items": input.items
                }
            }),
        );
        let request = operation_request(
            tenant_id,
            fulfillment_id,
            "ship",
            provider_id.as_str(),
            request_metadata,
        )?;
        let journaled = self
            .execute_provider_operation(provider_id.as_str(), "ship", request)
            .await?;
        if journaled.committed {
            return Ok(current);
        }

        let local_input = ShipFulfillmentInput {
            carrier: input.carrier,
            tracking_number: journaled
                .result
                .tracking_number
                .clone()
                .unwrap_or(input.tracking_number),
            items: input.items,
            metadata: local_commit_metadata(
                input.metadata,
                journaled.result.metadata.clone(),
                journaled.operation_id,
                "ship",
            ),
        };
        let updated = match service
            .ship_fulfillment(tenant_id, fulfillment_id, local_input)
            .await
        {
            Ok(updated) => updated,
            Err(source) => {
                self.mark_reconciliation_required(
                    journaled.operation_id,
                    "ship",
                    &source,
                )
                .await;
                return Err(FulfillmentOrchestrationError::PersistenceAfterProvider {
                    fulfillment_id,
                    operation: "ship",
                    source,
                });
            }
        };
        self.ensure_committed(journaled.operation_id, "ship").await?;
        Ok(updated)
    }

    pub async fn reship_fulfillment(
        &self,
        tenant_id: Uuid,
        fulfillment_id: Uuid,
        input: ReshipFulfillmentInput,
    ) -> FulfillmentOrchestrationResult<FulfillmentResponse> {
        input
            .validate()
            .map_err(|error| FulfillmentOrchestrationError::Validation(error.to_string()))?;
        let service = FulfillmentService::new(self.db.clone());
        let current = service.get_fulfillment(tenant_id, fulfillment_id).await?;
        if current.status != "delivered" {
            return Err(FulfillmentError::InvalidTransition {
                from: current.status,
                to: "shipped".to_string(),
            }
            .into());
        }

        let provider_id = self.provider_id_for_fulfillment(tenant_id, &current).await?;
        let request_metadata = merge_metadata(
            input.metadata.clone(),
            serde_json::json!({
                "commerce_orchestration": {
                    "operation": "reship",
                    "carrier": input.carrier,
                    "tracking_number": input.tracking_number,
                    "items": input.items
                }
            }),
        );
        let request = operation_request(
            tenant_id,
            fulfillment_id,
            "reship",
            provider_id.as_str(),
            request_metadata,
        )?;
        let journaled = self
            .execute_provider_operation(provider_id.as_str(), "reship", request)
            .await?;
        if journaled.committed {
            return Ok(current);
        }

        let local_input = ReshipFulfillmentInput {
            carrier: input.carrier,
            tracking_number: journaled
                .result
                .tracking_number
                .clone()
                .unwrap_or(input.tracking_number),
            items: input.items,
            metadata: local_commit_metadata(
                input.metadata,
                journaled.result.metadata.clone(),
                journaled.operation_id,
                "reship",
            ),
        };
        let updated = match service
            .reship_fulfillment(tenant_id, fulfillment_id, local_input)
            .await
        {
            Ok(updated) => updated,
            Err(source) => {
                self.mark_reconciliation_required(
                    journaled.operation_id,
                    "reship",
                    &source,
                )
                .await;
                return Err(FulfillmentOrchestrationError::PersistenceAfterProvider {
                    fulfillment_id,
                    operation: "reship",
                    source,
                });
            }
        };
        self.ensure_committed(journaled.operation_id, "reship").await?;
        Ok(updated)
    }

    pub async fn cancel_fulfillment(
        &self,
        tenant_id: Uuid,
        fulfillment_id: Uuid,
        input: CancelFulfillmentInput,
    ) -> FulfillmentOrchestrationResult<FulfillmentResponse> {
        let service = FulfillmentService::new(self.db.clone());
        let current = service.get_fulfillment(tenant_id, fulfillment_id).await?;
        if current.status == "cancelled" {
            return Ok(current);
        }
        if current.status == "delivered" {
            return Err(FulfillmentError::InvalidTransition {
                from: current.status,
                to: "cancelled".to_string(),
            }
            .into());
        }

        let provider_id = self.provider_id_for_fulfillment(tenant_id, &current).await?;
        let request_metadata = merge_metadata(
            input.metadata.clone(),
            serde_json::json!({
                "commerce_orchestration": {
                    "operation": "cancel",
                    "reason": input.reason
                }
            }),
        );
        let request = operation_request(
            tenant_id,
            fulfillment_id,
            "cancel",
            provider_id.as_str(),
            request_metadata,
        )?;
        let journaled = self
            .execute_provider_operation(provider_id.as_str(), "cancel", request)
            .await?;
        if journaled.committed {
            return Ok(current);
        }

        let local_input = CancelFulfillmentInput {
            reason: input.reason,
            metadata: local_commit_metadata(
                input.metadata,
                journaled.result.metadata.clone(),
                journaled.operation_id,
                "cancel",
            ),
        };
        let updated = match service
            .cancel_fulfillment(tenant_id, fulfillment_id, local_input)
            .await
        {
            Ok(updated) => updated,
            Err(source) => {
                self.mark_reconciliation_required(
                    journaled.operation_id,
                    "cancel",
                    &source,
                )
                .await;
                return Err(FulfillmentOrchestrationError::PersistenceAfterProvider {
                    fulfillment_id,
                    operation: "cancel",
                    source,
                });
            }
        };
        self.ensure_committed(journaled.operation_id, "cancel").await?;
        Ok(updated)
    }

    async fn execute_provider_operation(
        &self,
        provider_id: &str,
        operation: &'static str,
        request: FulfillmentProviderOperationRequest,
    ) -> FulfillmentOrchestrationResult<JournaledProviderResult> {
        let idempotency_key = request
            .idempotency_key
            .clone()
            .ok_or_else(|| FulfillmentOrchestrationError::Validation(
                format!("journaled fulfillment provider operation `{operation}` requires idempotency_key"),
            ))?;
        let request_payload = serde_json::to_value(&request).map_err(|error| {
            FulfillmentOrchestrationError::Validation(format!(
                "failed to serialize fulfillment {operation} request: {error}"
            ))
        })?;
        let journal = FulfillmentProviderOperationJournal::new(self.db.clone());
        let journal_operation = journal
            .begin(BeginProviderOperation {
                tenant_id: request.tenant_id,
                fulfillment_id: request.fulfillment_id,
                operation: operation.to_string(),
                provider_id: provider_id.to_string(),
                idempotency_key,
                request_payload,
            })
            .await?;

        if matches!(
            journal_operation.status.as_str(),
            PROVIDER_OPERATION_COMMITTED
                | PROVIDER_OPERATION_SUCCEEDED
                | PROVIDER_OPERATION_RECONCILIATION_REQUIRED
        ) {
            let result = deserialize_provider_result(&journal_operation)?;
            if journal_operation.status == PROVIDER_OPERATION_RECONCILIATION_REQUIRED {
                return Err(FulfillmentOrchestrationError::Validation(format!(
                    "fulfillment provider operation {} requires reconciliation: {}",
                    journal_operation.id,
                    journal_operation.error_message.as_deref().unwrap_or("unknown local persistence failure")
                )));
            }
            return Ok(JournaledProviderResult {
                operation_id: journal_operation.id,
                result,
                committed: journal_operation.status == PROVIDER_OPERATION_COMMITTED,
            });
        }
        if journal_operation.status == PROVIDER_OPERATION_EXECUTING {
            return Err(operation_in_progress(journal_operation.id, operation));
        }

        if journal.claim_execution(journal_operation.id).await?.is_none() {
            let current = journal.get(journal_operation.id).await?;
            if matches!(
                current.status.as_str(),
                PROVIDER_OPERATION_COMMITTED
                    | PROVIDER_OPERATION_SUCCEEDED
                    | PROVIDER_OPERATION_RECONCILIATION_REQUIRED
            ) {
                let result = deserialize_provider_result(&current)?;
                if current.status == PROVIDER_OPERATION_RECONCILIATION_REQUIRED {
                    return Err(FulfillmentOrchestrationError::Validation(format!(
                        "fulfillment provider operation {} requires reconciliation: {}",
                        current.id,
                        current.error_message.as_deref().unwrap_or("unknown local persistence failure")
                    )));
                }
                return Ok(JournaledProviderResult {
                    operation_id: current.id,
                    result,
                    committed: current.status == PROVIDER_OPERATION_COMMITTED,
                });
            }
            return Err(operation_in_progress(current.id, operation));
        }

        let provider_result = match operation {
            "ship" | "reship" => self
                .fulfillment_provider_registry
                .execute_ship(provider_id, request)
                .await,
            "cancel" => self
                .fulfillment_provider_registry
                .execute_cancel(provider_id, request)
                .await,
            _ => Err(FulfillmentError::Validation(format!(
                "unsupported journaled fulfillment provider operation `{operation}`"
            ))),
        };
        let provider_result = match provider_result {
            Ok(result) => result,
            Err(source) => {
                if let Err(journal_error) = journal
                    .mark_provider_error(journal_operation.id, source.to_string())
                    .await
                {
                    return Err(FulfillmentOrchestrationError::Validation(format!(
                        "fulfillment provider {operation} failed for operation {}, and the journal could not record the failure: provider={source}; journal={journal_error}",
                        journal_operation.id
                    )));
                }
                return Err(source.into());
            }
        };
        let result_payload = serde_json::to_value(&provider_result).map_err(|error| {
            FulfillmentOrchestrationError::Validation(format!(
                "failed to serialize fulfillment {operation} provider result: {error}"
            ))
        })?;
        journal
            .mark_provider_succeeded(
                journal_operation.id,
                provider_result.external_reference.clone(),
                result_payload,
            )
            .await
            .map_err(|source| {
                FulfillmentOrchestrationError::Validation(format!(
                    "fulfillment provider {operation} succeeded, but operation {} could not record provider success: {source}",
                    journal_operation.id
                ))
            })?;

        Ok(JournaledProviderResult {
            operation_id: journal_operation.id,
            result: provider_result,
            committed: false,
        })
    }

    async fn provider_id_for_fulfillment(
        &self,
        tenant_id: Uuid,
        fulfillment: &FulfillmentResponse,
    ) -> FulfillmentOrchestrationResult<String> {
        match fulfillment.shipping_option_id {
            Some(shipping_option_id) => Ok(FulfillmentService::new(self.db.clone())
                .get_shipping_option(tenant_id, shipping_option_id, None, None)
                .await?
                .provider_id),
            None => Ok(MANUAL_FULFILLMENT_PROVIDER_ID.to_string()),
        }
    }

    async fn mark_reconciliation_required(
        &self,
        operation_id: Uuid,
        operation: &'static str,
        source: &FulfillmentError,
    ) {
        let _ = FulfillmentProviderOperationJournal::new(self.db.clone())
            .mark_reconciliation_required(
                operation_id,
                format!("local {operation} persistence failed: {source}"),
            )
            .await;
    }

    async fn ensure_committed(
        &self,
        operation_id: Uuid,
        operation: &'static str,
    ) -> FulfillmentOrchestrationResult<()> {
        let journal = FulfillmentProviderOperationJournal::new(self.db.clone());
        let current = journal.get(operation_id).await?;
        if current.status == PROVIDER_OPERATION_COMMITTED {
            return Ok(());
        }
        if let Err(source) = journal.mark_committed(operation_id).await {
            let _ = journal
                .mark_reconciliation_required(
                    operation_id,
                    format!("local {operation} succeeded, but journal commit failed: {source}"),
                )
                .await;
            return Err(FulfillmentOrchestrationError::Validation(format!(
                "fulfillment {operation} succeeded locally, but operation {operation_id} could not be committed: {source}"
            )));
        }
        Ok(())
    }
}

struct JournaledProviderResult {
    operation_id: Uuid,
    result: FulfillmentProviderOperationResult,
    committed: bool,
}

fn operation_request(
    tenant_id: Uuid,
    fulfillment_id: Uuid,
    operation: &'static str,
    provider_id: &str,
    metadata: Value,
) -> FulfillmentOrchestrationResult<FulfillmentProviderOperationRequest> {
    let immutable_payload = serde_json::json!({
        "tenant_id": tenant_id,
        "fulfillment_id": fulfillment_id,
        "operation": operation,
        "provider_id": provider_id,
        "metadata": metadata,
    });
    let key = stable_operation_key(fulfillment_id, operation, &immutable_payload)?;
    Ok(FulfillmentProviderOperationRequest {
        tenant_id,
        fulfillment_id,
        idempotency_key: Some(key),
        metadata,
    })
}

fn stable_operation_key(
    fulfillment_id: Uuid,
    operation: &str,
    payload: &Value,
) -> FulfillmentOrchestrationResult<String> {
    let bytes = serde_json::to_vec(payload).map_err(|error| {
        FulfillmentOrchestrationError::Validation(format!(
            "failed to serialize fulfillment idempotency payload: {error}"
        ))
    })?;
    let first = fnv1a64(&bytes, 0xcbf29ce484222325);
    let second = fnv1a64(&bytes, 0x84222325cbf29ce4);
    Ok(format!(
        "fulfillment:{fulfillment_id}:{operation}:{first:016x}{second:016x}"
    ))
}

fn fnv1a64(bytes: &[u8], offset_basis: u64) -> u64 {
    bytes.iter().fold(offset_basis, |hash, byte| {
        (hash ^ u64::from(*byte)).wrapping_mul(0x100000001b3)
    })
}

fn deserialize_provider_result(
    operation: &rustok_fulfillment::entities::provider_operation::Model,
) -> FulfillmentOrchestrationResult<FulfillmentProviderOperationResult> {
    let value = operation.provider_result.clone().ok_or_else(|| {
        FulfillmentOrchestrationError::Validation(format!(
            "fulfillment provider operation {} is `{}` but has no provider_result",
            operation.id, operation.status
        ))
    })?;
    serde_json::from_value(value).map_err(|error| {
        FulfillmentOrchestrationError::Validation(format!(
            "fulfillment provider operation {} contains invalid provider_result: {error}",
            operation.id
        ))
    })
}

fn operation_in_progress(
    operation_id: Uuid,
    operation: &'static str,
) -> FulfillmentOrchestrationError {
    FulfillmentOrchestrationError::Validation(format!(
        "fulfillment provider {operation} operation {operation_id} is already executing"
    ))
}

fn local_commit_metadata(
    input_metadata: Value,
    provider_metadata: Value,
    operation_id: Uuid,
    operation: &'static str,
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
    fn operation_key_is_stable_and_payload_sensitive() {
        let fulfillment_id = Uuid::new_v4();
        let first = stable_operation_key(
            fulfillment_id,
            "ship",
            &serde_json::json!({"items": [{"id": "a", "quantity": 1}]}),
        )
        .expect("key");
        let retry = stable_operation_key(
            fulfillment_id,
            "ship",
            &serde_json::json!({"items": [{"id": "a", "quantity": 1}]}),
        )
        .expect("retry key");
        let changed = stable_operation_key(
            fulfillment_id,
            "ship",
            &serde_json::json!({"items": [{"id": "a", "quantity": 2}]}),
        )
        .expect("changed key");
        assert_eq!(first, retry);
        assert_ne!(first, changed);
        assert!(first.len() <= 191);
    }
}
