use async_trait::async_trait;
use rustok_fulfillment::providers::{
    FulfillmentProvider, FulfillmentProviderDescriptor, FulfillmentProviderOperationRequest,
    FulfillmentProviderOperationResult, FulfillmentProviderRegistry,
    FulfillmentProviderWebhookRequest, FulfillmentProviderWebhookResult, FulfillmentRateQuote,
    FulfillmentRateQuoteRequest,
};
use rustok_fulfillment::{
    BeginProviderOperation, FulfillmentError, FulfillmentProviderOperationJournal,
    FulfillmentResult, PROVIDER_OPERATION_COMMITTED, PROVIDER_OPERATION_EXECUTING,
    PROVIDER_OPERATION_RECONCILIATION_REQUIRED, PROVIDER_OPERATION_SUCCEEDED,
};
use sea_orm::DatabaseConnection;
use std::sync::Arc;

pub fn wrap_create_label_providers(
    db: DatabaseConnection,
    source_registry: FulfillmentProviderRegistry,
) -> FulfillmentResult<FulfillmentProviderRegistry> {
    let mut wrapped_registry = FulfillmentProviderRegistry::new();
    for descriptor in source_registry.descriptors() {
        let provider_id = descriptor.provider_id.clone();
        let registration = source_registry
            .registration(provider_id.as_str())
            .cloned()
            .ok_or_else(|| {
                FulfillmentError::Validation(format!(
                    "fulfillment provider `{provider_id}` has no registration metadata"
                ))
            })?;
        let wrapped = Arc::new(JournaledCreateLabelProvider {
            db: db.clone(),
            descriptor,
            provider_id: provider_id.clone(),
            source_registry: source_registry.clone(),
        });
        wrapped_registry.register_external(provider_id.as_str(), wrapped, registration)?;
    }
    Ok(wrapped_registry)
}

struct JournaledCreateLabelProvider {
    db: DatabaseConnection,
    descriptor: FulfillmentProviderDescriptor,
    provider_id: String,
    source_registry: FulfillmentProviderRegistry,
}

#[async_trait]
impl FulfillmentProvider for JournaledCreateLabelProvider {
    fn descriptor(&self) -> FulfillmentProviderDescriptor {
        self.descriptor.clone()
    }

    async fn quote_rates(
        &self,
        request: FulfillmentRateQuoteRequest,
    ) -> FulfillmentResult<Vec<FulfillmentRateQuote>> {
        self.source_registry
            .execute_quote_rates(self.provider_id.as_str(), request)
            .await
    }

    async fn create_label(
        &self,
        request: FulfillmentProviderOperationRequest,
    ) -> FulfillmentResult<FulfillmentProviderOperationResult> {
        let idempotency_key = request.idempotency_key.clone().ok_or_else(|| {
            FulfillmentError::Validation(
                "journaled create_label requires idempotency_key".to_string(),
            )
        })?;
        let request_payload = serde_json::to_value(&request).map_err(|error| {
            FulfillmentError::Validation(format!(
                "failed to serialize create_label provider request: {error}"
            ))
        })?;
        let journal = FulfillmentProviderOperationJournal::new(self.db.clone());
        let operation = journal
            .begin(BeginProviderOperation {
                tenant_id: request.tenant_id,
                fulfillment_id: request.fulfillment_id,
                operation: "create_label".to_string(),
                provider_id: self.provider_id.clone(),
                idempotency_key,
                request_payload,
            })
            .await?;

        if matches!(
            operation.status.as_str(),
            PROVIDER_OPERATION_COMMITTED
                | PROVIDER_OPERATION_SUCCEEDED
                | PROVIDER_OPERATION_RECONCILIATION_REQUIRED
        ) {
            let result = deserialize_result(&operation)?;
            if operation.status != PROVIDER_OPERATION_COMMITTED {
                commit_create_label(&journal, operation.id).await?;
            }
            return Ok(result);
        }
        if operation.status == PROVIDER_OPERATION_EXECUTING {
            return Err(operation_in_progress(operation.id));
        }
        if journal.claim_execution(operation.id).await?.is_none() {
            let current = journal.get(operation.id).await?;
            if matches!(
                current.status.as_str(),
                PROVIDER_OPERATION_COMMITTED
                    | PROVIDER_OPERATION_SUCCEEDED
                    | PROVIDER_OPERATION_RECONCILIATION_REQUIRED
            ) {
                let result = deserialize_result(&current)?;
                if current.status != PROVIDER_OPERATION_COMMITTED {
                    commit_create_label(&journal, current.id).await?;
                }
                return Ok(result);
            }
            return Err(operation_in_progress(current.id));
        }

        let result = match self
            .source_registry
            .execute_create_label(self.provider_id.as_str(), request)
            .await
        {
            Ok(result) => result,
            Err(source) => {
                if let Err(journal_error) = journal
                    .mark_provider_error(operation.id, source.to_string())
                    .await
                {
                    return Err(FulfillmentError::Validation(format!(
                        "create_label failed for operation {}, and the journal could not record the failure: provider={source}; journal={journal_error}",
                        operation.id
                    )));
                }
                return Err(source);
            }
        };
        let result_payload = serde_json::to_value(&result).map_err(|error| {
            FulfillmentError::Validation(format!(
                "failed to serialize create_label provider result: {error}"
            ))
        })?;
        journal
            .mark_provider_succeeded(
                operation.id,
                result.external_reference.clone(),
                result_payload,
            )
            .await
            .map_err(|source| {
                FulfillmentError::Validation(format!(
                    "create_label succeeded, but operation {} could not record provider success: {source}",
                    operation.id
                ))
            })?;
        commit_create_label(&journal, operation.id).await?;
        Ok(result)
    }

    async fn ship(
        &self,
        request: FulfillmentProviderOperationRequest,
    ) -> FulfillmentResult<FulfillmentProviderOperationResult> {
        self.source_registry
            .execute_ship(self.provider_id.as_str(), request)
            .await
    }

    async fn cancel(
        &self,
        request: FulfillmentProviderOperationRequest,
    ) -> FulfillmentResult<FulfillmentProviderOperationResult> {
        self.source_registry
            .execute_cancel(self.provider_id.as_str(), request)
            .await
    }

    async fn handle_tracking_webhook(
        &self,
        request: FulfillmentProviderWebhookRequest,
    ) -> FulfillmentResult<FulfillmentProviderWebhookResult> {
        self.source_registry
            .execute_tracking_webhook(self.provider_id.as_str(), request)
            .await
    }
}

async fn commit_create_label(
    journal: &FulfillmentProviderOperationJournal,
    operation_id: uuid::Uuid,
) -> FulfillmentResult<()> {
    if let Err(source) = journal.mark_committed(operation_id).await {
        let _ = journal
            .mark_reconciliation_required(
                operation_id,
                format!("create_label provider succeeded, but journal commit failed: {source}"),
            )
            .await;
        return Err(FulfillmentError::Validation(format!(
            "create_label provider succeeded, but operation {operation_id} could not be committed: {source}"
        )));
    }
    Ok(())
}

fn deserialize_result(
    operation: &rustok_fulfillment::entities::provider_operation::Model,
) -> FulfillmentResult<FulfillmentProviderOperationResult> {
    let value = operation.provider_result.clone().ok_or_else(|| {
        FulfillmentError::Validation(format!(
            "create_label operation {} is `{}` but has no provider_result",
            operation.id, operation.status
        ))
    })?;
    serde_json::from_value(value).map_err(|error| {
        FulfillmentError::Validation(format!(
            "create_label operation {} contains invalid provider_result: {error}",
            operation.id
        ))
    })
}

fn operation_in_progress(operation_id: uuid::Uuid) -> FulfillmentError {
    FulfillmentError::Validation(format!(
        "create_label provider operation {operation_id} is already executing"
    ))
}
