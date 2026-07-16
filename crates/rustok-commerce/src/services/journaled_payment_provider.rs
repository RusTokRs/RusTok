use rustok_payment::providers::{
    PaymentProviderOperationRequest, PaymentProviderOperationResult, PaymentProviderRegistry,
};
use rustok_payment::{
    BeginProviderOperation, PaymentError, PaymentProviderOperationJournal,
    PROVIDER_OPERATION_COMMITTED, PROVIDER_OPERATION_EXECUTING,
    PROVIDER_OPERATION_RECONCILIATION_REQUIRED, PROVIDER_OPERATION_SUCCEEDED,
};
use serde_json::Value;
use uuid::Uuid;

use super::payment_orchestration::{PaymentOrchestrationError, PaymentOrchestrationResult};

const MANUAL_PROVIDER_ID: &str = "manual";

pub(crate) struct JournaledProviderResult {
    pub operation_id: Uuid,
    pub result: PaymentProviderOperationResult,
}

pub(crate) async fn execute_journaled_provider_operation(
    journal: &PaymentProviderOperationJournal,
    registry: &PaymentProviderRegistry,
    operation: &'static str,
    refund_id: Option<Uuid>,
    provider_id: &str,
    request: PaymentProviderOperationRequest,
) -> PaymentOrchestrationResult<JournaledProviderResult> {
    let request = enrich_provider_request(
        journal,
        operation,
        refund_id,
        provider_id,
        request,
    )
    .await?;
    let idempotency_key = request
        .idempotency_key
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .ok_or_else(|| {
            PaymentError::Validation(format!(
                "journaled payment provider operation `{operation}` requires idempotency_key"
            ))
        })?
        .to_string();
    let request_payload = serde_json::to_value(&request).map_err(|error| {
        PaymentError::Validation(format!(
            "failed to serialize {operation} provider request: {error}"
        ))
    })?;
    let journal_operation = journal
        .begin(BeginProviderOperation {
            tenant_id: request.tenant_id,
            payment_collection_id: request.collection_id,
            refund_id,
            operation: operation.to_string(),
            provider_id: provider_id.to_string(),
            idempotency_key,
            request_payload,
        })
        .await?;

    if let Some(result) = persisted_provider_result(&journal_operation)? {
        return Ok(JournaledProviderResult {
            operation_id: journal_operation.id,
            result,
        });
    }

    let claimed = journal.claim_execution(journal_operation.id).await?;
    if claimed.is_none() {
        let current = journal.get(journal_operation.id).await?;
        if let Some(result) = persisted_provider_result(&current)? {
            return Ok(JournaledProviderResult {
                operation_id: current.id,
                result,
            });
        }
        return Err(PaymentError::Validation(format!(
            "payment provider operation {} is already `{}`; retry after the active execution finishes",
            current.id, current.status
        ))
        .into());
    }

    let provider_result = match operation {
        "authorize" => registry.execute_authorize(provider_id, request).await,
        "capture" => registry.execute_capture(provider_id, request).await,
        "cancel" => registry.execute_cancel(provider_id, request).await,
        "refund" => registry.execute_refund(provider_id, request).await,
        _ => {
            return Err(PaymentError::Validation(format!(
                "unsupported journaled provider operation `{operation}`"
            ))
            .into())
        }
    };
    let provider_result = match provider_result {
        Ok(result) => result,
        Err(source) => {
            if let Err(journal_error) = journal
                .mark_provider_error(journal_operation.id, source.to_string())
                .await
            {
                return Err(PaymentOrchestrationError::Provider(PaymentError::Validation(
                    format!(
                        "provider {operation} failed and operation {} could not record the failure: provider error: {source}; journal error: {journal_error}",
                        journal_operation.id
                    ),
                )));
            }
            return match refund_id {
                Some(refund_id) => Err(PaymentOrchestrationError::ProviderAfterRefundReservation {
                    refund_id,
                    source,
                }),
                None => Err(PaymentOrchestrationError::Provider(source)),
            };
        }
    };
    let result_payload = serde_json::to_value(&provider_result).map_err(|error| {
        PaymentError::Validation(format!(
            "failed to serialize {operation} provider result: {error}"
        ))
    })?;
    if let Err(source) = journal
        .mark_provider_succeeded(
            journal_operation.id,
            provider_result.external_reference.clone(),
            result_payload,
        )
        .await
    {
        let source = reconciliation_error(journal_operation.id, "record provider success", source);
        return match refund_id {
            Some(refund_id) => {
                Err(PaymentOrchestrationError::ProviderAfterRefundReservation { refund_id, source })
            }
            None => Err(PaymentOrchestrationError::Provider(source)),
        };
    }

    Ok(JournaledProviderResult {
        operation_id: journal_operation.id,
        result: provider_result,
    })
}

async fn enrich_provider_request(
    journal: &PaymentProviderOperationJournal,
    operation: &str,
    refund_id: Option<Uuid>,
    provider_id: &str,
    mut request: PaymentProviderOperationRequest,
) -> Result<PaymentProviderOperationRequest, PaymentError> {
    if let Some(refund_id) = refund_id {
        insert_metadata_string(&mut request.metadata, "refund_id", refund_id.to_string())?;
    }
    if provider_id == MANUAL_PROVIDER_ID || operation == "authorize" {
        return Ok(request);
    }
    if metadata_string(&request.metadata, "provider_payment_id").is_some() {
        return Ok(request);
    }

    let authorize_key = format!("payment_collection:{}:authorize", request.collection_id);
    let authorize = journal
        .find_by_key(request.tenant_id, provider_id, authorize_key.as_str())
        .await?
        .ok_or_else(|| {
            PaymentError::Validation(format!(
                "provider `{provider_id}` {operation} requires a completed authorize operation for payment collection {}",
                request.collection_id
            ))
        })?;
    if !matches!(
        authorize.status.as_str(),
        PROVIDER_OPERATION_COMMITTED
            | PROVIDER_OPERATION_SUCCEEDED
            | PROVIDER_OPERATION_RECONCILIATION_REQUIRED
    ) {
        return Err(PaymentError::Validation(format!(
            "provider `{provider_id}` {operation} cannot use authorize operation {} in status `{}`",
            authorize.id, authorize.status
        )));
    }
    let provider_payment_id = authorize
        .provider_reference
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_string)
        .or_else(|| {
            authorize
                .provider_result
                .as_ref()
                .and_then(|result| result.get("external_reference"))
                .and_then(Value::as_str)
                .map(str::trim)
                .filter(|value| !value.is_empty())
                .map(str::to_string)
        })
        .ok_or_else(|| {
            PaymentError::Validation(format!(
                "provider authorize operation {} has no external payment identity",
                authorize.id
            ))
        })?;
    insert_metadata_string(
        &mut request.metadata,
        "provider_payment_id",
        provider_payment_id,
    )?;
    Ok(request)
}

fn insert_metadata_string(
    metadata: &mut Value,
    key: &str,
    value: String,
) -> Result<(), PaymentError> {
    if !metadata.is_object() {
        if metadata.is_null() {
            *metadata = serde_json::json!({});
        } else {
            return Err(PaymentError::Validation(
                "payment provider operation metadata must be an object".to_string(),
            ));
        }
    }
    let object = metadata.as_object_mut().ok_or_else(|| {
        PaymentError::Validation("payment provider operation metadata must be an object".to_string())
    })?;
    if let Some(existing) = object.get(key).and_then(Value::as_str) {
        if existing != value {
            return Err(PaymentError::Validation(format!(
                "payment provider operation metadata `{key}` conflicts with owner identity"
            )));
        }
        return Ok(());
    }
    object.insert(key.to_string(), Value::String(value));
    Ok(())
}

fn metadata_string<'a>(metadata: &'a Value, key: &str) -> Option<&'a str> {
    metadata
        .get(key)
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
}

fn persisted_provider_result(
    journal_operation: &rustok_payment::entities::provider_operation::Model,
) -> Result<Option<PaymentProviderOperationResult>, PaymentError> {
    if !matches!(
        journal_operation.status.as_str(),
        PROVIDER_OPERATION_COMMITTED
            | PROVIDER_OPERATION_SUCCEEDED
            | PROVIDER_OPERATION_RECONCILIATION_REQUIRED
    ) {
        if journal_operation.status == PROVIDER_OPERATION_EXECUTING {
            return Ok(None);
        }
        return Ok(None);
    }

    let result = journal_operation
        .provider_result
        .clone()
        .ok_or_else(|| {
            PaymentError::Validation(format!(
                "provider operation {} is `{}` but has no persisted provider_result",
                journal_operation.id, journal_operation.status
            ))
        })
        .and_then(|value| {
            serde_json::from_value(value).map_err(|error| {
                PaymentError::Validation(format!(
                    "provider operation {} contains invalid provider_result: {error}",
                    journal_operation.id
                ))
            })
        })?;
    Ok(Some(result))
}

pub(crate) async fn mark_journal_committed(
    journal: &PaymentProviderOperationJournal,
    operation_id: Uuid,
    operation: &'static str,
) -> PaymentOrchestrationResult<()> {
    if let Err(source) = journal.mark_committed(operation_id).await {
        let _ = journal
            .mark_reconciliation_required(operation_id, source.to_string())
            .await;
        return Err(PaymentOrchestrationError::Provider(reconciliation_error(
            operation_id,
            &format!("commit {operation} journal"),
            source,
        )));
    }
    Ok(())
}

pub(crate) async fn mark_local_persistence_failed(
    journal: &PaymentProviderOperationJournal,
    operation_id: Uuid,
    operation: &'static str,
    source: &PaymentError,
) {
    let _ = journal
        .mark_reconciliation_required(
            operation_id,
            format!("local {operation} persistence failed: {source}"),
        )
        .await;
}

pub(crate) fn local_persistence_after_provider_error(
    operation_id: Uuid,
    operation: &'static str,
    source: PaymentError,
) -> PaymentOrchestrationError {
    PaymentOrchestrationError::Provider(PaymentError::Validation(format!(
        "provider {operation} succeeded, but local persistence failed for operation {operation_id}: {source}"
    )))
}

fn reconciliation_error(operation_id: Uuid, stage: &str, source: PaymentError) -> PaymentError {
    PaymentError::Validation(format!(
        "provider side effect succeeded, but failed to {stage} for operation {operation_id}: {source}"
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn inserts_owner_provider_identity_without_overwriting_conflicts() {
        let mut metadata = serde_json::json!({});
        insert_metadata_string(
            &mut metadata,
            "provider_payment_id",
            "pi_123".to_string(),
        )
        .unwrap();
        assert_eq!(
            metadata.get("provider_payment_id").and_then(Value::as_str),
            Some("pi_123")
        );
        assert!(insert_metadata_string(
            &mut metadata,
            "provider_payment_id",
            "pi_other".to_string(),
        )
        .is_err());
    }
}
