use chrono::Utc;
use rustok_fulfillment::entities::provider_operation;
use rustok_fulfillment::providers::{
    FulfillmentProviderOperationRequest, FulfillmentProviderOperationResult,
    FulfillmentProviderRegistry,
};
use rustok_fulfillment::{
    FulfillmentProviderOperationJournal, FulfillmentService, PROVIDER_OPERATION_COMMITTED,
    PROVIDER_OPERATION_ERROR, PROVIDER_OPERATION_EXECUTING,
    PROVIDER_OPERATION_RECONCILIATION_REQUIRED, PROVIDER_OPERATION_SUCCEEDED,
};
use sea_orm::DatabaseConnection;
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
        let operation = journal.get(tenant_id, operation_id).await?;
        if operation.tenant_id != tenant_id || operation.operation != "create_label" {
            return Err(FulfillmentOrchestrationError::Validation(format!(
                "fulfillment provider operation {operation_id} is not a create_label operation for tenant {tenant_id}"
            )));
        }

        match operation.status.as_str() {
            PROVIDER_OPERATION_COMMITTED => {
                return self.commit_provider_result(&journal, operation).await;
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

        let request: FulfillmentProviderOperationRequest =
            serde_json::from_value(operation.request_payload.clone()).map_err(|error| {
                FulfillmentOrchestrationError::Validation(format!(
                    "create_label operation {operation_id} contains invalid request_payload: {error}"
                ))
            })?;
        if request.tenant_id != tenant_id
            || request.fulfillment_id != operation.fulfillment_id
            || request.idempotency_key.as_deref().map(str::trim)
                != Some(operation.idempotency_key.as_str())
        {
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
                    .mark_provider_error(
                        tenant_id,
                        operation_id,
                        "create_label provider execution returned an error",
                    )
                    .await
                {
                    tracing::error!(
                        boundary = "commerce_fulfillment_create_label_recovery",
                        operation_id_non_nil = !operation_id.is_nil(),
                        fulfillment_id_non_nil = !operation.fulfillment_id.is_nil(),
                        checkpoint_failed = true,
                        provider_outcome = "error",
                        checkpoint_error = %journal_error,
                        "create-label recovery could not checkpoint provider error"
                    );
                }
                return Err(FulfillmentOrchestrationError::ProviderAfterPersistence {
                    fulfillment_id: operation.fulfillment_id,
                    operation: "create_label",
                    source,
                });
            }
        };
        let payload = match serde_json::to_value(&result) {
            Ok(payload) => payload,
            Err(_) => {
                if let Err(checkpoint_error) = journal
                    .mark_execution_reconciliation_required(
                        tenant_id,
                        operation_id,
                        result.external_reference.clone(),
                        None,
                        "create_label provider result could not be serialized",
                    )
                    .await
                {
                    tracing::error!(
                        boundary = "commerce_fulfillment_create_label_recovery",
                        operation_id_non_nil = !operation_id.is_nil(),
                        fulfillment_id_non_nil = !operation.fulfillment_id.is_nil(),
                        checkpoint_failed = true,
                        provider_outcome = "success",
                        checkpoint_error = %checkpoint_error,
                        "create-label recovery could not checkpoint an unserializable provider result"
                    );
                }
                return Err(FulfillmentOrchestrationError::ProviderAfterPersistence {
                    fulfillment_id: operation.fulfillment_id,
                    operation: "create_label",
                    source: rustok_fulfillment::error::FulfillmentError::Validation(
                        "provider result could not be serialized",
                    ),
                });
            }
        };
        let operation = journal
            .mark_provider_succeeded(
                tenant_id,
                operation_id,
                result.external_reference.clone(),
                payload,
            )
            .await?;
        self.commit_provider_result(&journal, operation).await
    }

    async fn commit_provider_result(
        &self,
        journal: &FulfillmentProviderOperationJournal,
        operation: provider_operation::Model,
    ) -> FulfillmentOrchestrationResult<crate::dto::FulfillmentResponse> {
        let result = validate_result(&operation)?;
        match FulfillmentService::new(self.db.clone())
            .commit_create_label_provider_result(
                operation.tenant_id,
                operation.fulfillment_id,
                operation.id,
                &result,
            )
            .await
        {
            Ok(fulfillment) => Ok(fulfillment),
            Err(source) => {
                if operation.status == PROVIDER_OPERATION_SUCCEEDED {
                    if let Err(checkpoint_error) = journal
                        .mark_reconciliation_required(
                            operation.tenant_id,
                            operation.id,
                            "create_label provider result could not be committed locally",
                        )
                        .await
                    {
                        tracing::error!(
                            boundary = "commerce_fulfillment_create_label_recovery",
                            operation_id_non_nil = !operation.id.is_nil(),
                            fulfillment_id_non_nil = !operation.fulfillment_id.is_nil(),
                            checkpoint_failed = true,
                            checkpoint_error = %checkpoint_error,
                            "create-label recovery reconciliation checkpoint failed after owner persistence error"
                        );
                    }
                }
                Err(FulfillmentOrchestrationError::PersistenceAfterProvider {
                    fulfillment_id: operation.fulfillment_id,
                    operation: "create_label",
                    source,
                })
            }
        }
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


#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn create_label_result_replay_requires_valid_provider_result() {
        let operation = provider_operation::Model {
            id: Uuid::new_v4(),
            tenant_id: Uuid::new_v4(),
            fulfillment_id: Uuid::new_v4(),
            operation: "create_label".to_string(),
            provider_id: "carrier".to_string(),
            idempotency_key: "key".to_string(),
            status: PROVIDER_OPERATION_SUCCEEDED.to_string(),
            request_payload: serde_json::json!({}),
            provider_reference: None,
            provider_result: Some(serde_json::json!({
                "provider_id": "carrier",
                "external_reference": "label-1",
                "tracking_number": "track-1",
                "metadata": {"provider_fact": true}
            })),
            error_message: None,
            created_at: Utc::now().into(),
            updated_at: Utc::now().into(),
            provider_completed_at: Some(Utc::now().into()),
            committed_at: None,
        };
        let result = validate_result(&operation).expect("provider result should validate");
        assert_eq!(result.provider_id, "carrier");
        assert_eq!(result.tracking_number.as_deref(), Some("track-1"));
    }

    #[test]
    fn create_label_result_replay_rejects_wrong_provider() {
        let operation = provider_operation::Model {
            id: Uuid::new_v4(),
            tenant_id: Uuid::new_v4(),
            fulfillment_id: Uuid::new_v4(),
            operation: "create_label".to_string(),
            provider_id: "carrier".to_string(),
            idempotency_key: "key".to_string(),
            status: PROVIDER_OPERATION_SUCCEEDED.to_string(),
            request_payload: serde_json::json!({}),
            provider_reference: None,
            provider_result: Some(serde_json::json!({
                "provider_id": "other",
                "external_reference": null,
                "tracking_number": null,
                "metadata": {}
            })),
            error_message: None,
            created_at: Utc::now().into(),
            updated_at: Utc::now().into(),
            provider_completed_at: Some(Utc::now().into()),
            committed_at: None,
        };
        assert!(validate_result(&operation).is_err());
    }
}
