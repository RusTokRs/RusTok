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

        let fulfillment = FulfillmentService::new(self.db.clone())
            .get_fulfillment(tenant_id, operation.fulfillment_id)
            .await?;
        match operation.status.as_str() {
            PROVIDER_OPERATION_COMMITTED => return Ok(fulfillment),
            PROVIDER_OPERATION_SUCCEEDED => {
                validate_result(&operation)?;
                journal.mark_committed(operation_id).await?;
                return Ok(fulfillment);
            }
            PROVIDER_OPERATION_RECONCILIATION_REQUIRED => {
                if operation.provider_result.is_none() {
                    return Err(FulfillmentOrchestrationError::Validation(format!(
                        "create_label operation {operation_id} has an unknown provider outcome; resolve it before retrying"
                    )));
                }
                validate_result(&operation)?;
                journal.mark_committed(operation_id).await?;
                return Ok(fulfillment);
            }
            PROVIDER_OPERATION_EXECUTING => {
                return Err(FulfillmentOrchestrationError::Validation(format!(
                    "create_label operation {operation_id} is already executing"
                )))
            }
            PROVIDER_OPERATION_ERROR | "pending" => {}
            other => {
                return Err(FulfillmentOrchestrationError::Validation(format!(
                    "create_label operation {operation_id} cannot be retried from status `{other}`"
                )))
            }
        }

        if journal.claim_execution(operation_id).await?.is_none() {
            let current = journal.get(operation_id).await?;
            return Err(FulfillmentOrchestrationError::Validation(format!(
                "create_label operation {operation_id} is now `{}` and was not claimed for retry",
                current.status
            )));
        }

        let request: FulfillmentProviderOperationRequest =
            serde_json::from_value(operation.request_payload.clone()).map_err(|error| {
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
        journal
            .mark_provider_succeeded(
                operation_id,
                result.external_reference.clone(),
                payload,
            )
            .await?;
        if let Err(source) = journal.mark_committed(operation_id).await {
            let _ = journal
                .mark_reconciliation_required(
                    operation_id,
                    format!("create_label retry succeeded, but journal commit failed: {source}"),
                )
                .await;
            return Err(FulfillmentOrchestrationError::Validation(format!(
                "create_label retry succeeded, but operation {operation_id} could not be committed: {source}"
            )));
        }
        Ok(fulfillment)
    }
}

fn validate_result(
    operation: &rustok_fulfillment::entities::provider_operation::Model,
) -> FulfillmentOrchestrationResult<FulfillmentProviderOperationResult> {
    let value = operation.provider_result.clone().ok_or_else(|| {
        FulfillmentOrchestrationError::Validation(format!(
            "create_label operation {} is `{}` but has no provider_result",
            operation.id, operation.status
        ))
    })?;
    serde_json::from_value(value).map_err(|error| {
        FulfillmentOrchestrationError::Validation(format!(
            "create_label operation {} contains invalid provider_result: {error}",
            operation.id
        ))
    })
}
