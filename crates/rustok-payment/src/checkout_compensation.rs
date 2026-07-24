use std::sync::Arc;

use async_trait::async_trait;
use rust_decimal::Decimal;
use rustok_api::{PortCallPolicy, PortContext, PortError, PortErrorKind};
use sea_orm::DatabaseConnection;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use uuid::Uuid;

use crate::dto::CancelPaymentInput;
use crate::providers::{
    MANUAL_PAYMENT_PROVIDER_ID, PaymentProviderOperationRequest, PaymentProviderOperationResult,
    PaymentProviderRegistry,
};
use crate::{
    BeginProviderOperation, PROVIDER_OPERATION_COMMITTED, PROVIDER_OPERATION_ERROR,
    PROVIDER_OPERATION_EXECUTING, PROVIDER_OPERATION_PENDING,
    PROVIDER_OPERATION_RECONCILIATION_REQUIRED, PROVIDER_OPERATION_SUCCEEDED, PaymentError,
    PaymentProviderOperationJournal, PaymentService,
};
use crate::{
    PaymentCollectionResponse, PaymentCollectionStatusKind, PaymentCollectionStatusSnapshot,
};

const COMPENSATE_CHECKOUT_PAYMENT_OPERATION: &str = "compensate_checkout_payment";

#[async_trait]
pub trait CheckoutPaymentCompensationPort: Send + Sync {
    async fn compensate_checkout_payment(
        &self,
        context: PortContext,
        request: CheckoutPaymentCompensationRequest,
    ) -> Result<Option<PaymentCollectionStatusSnapshot>, PortError>;
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct CheckoutPaymentCompensationRequest {
    pub checkout_operation_id: Uuid,
    pub collection_id: Option<Uuid>,
    pub reason: Option<String>,
    pub metadata: Value,
}

pub struct InProcessCheckoutPaymentCompensationPort {
    payment_service: PaymentService,
    operation_journal: PaymentProviderOperationJournal,
    provider_registry: PaymentProviderRegistry,
}

impl InProcessCheckoutPaymentCompensationPort {
    pub fn new(db: DatabaseConnection) -> Self {
        Self::with_provider_registry(db, PaymentProviderRegistry::with_manual_provider())
    }

    pub fn with_provider_registry(
        db: DatabaseConnection,
        provider_registry: PaymentProviderRegistry,
    ) -> Self {
        Self {
            payment_service: PaymentService::new(db.clone()),
            operation_journal: PaymentProviderOperationJournal::new(db),
            provider_registry,
        }
    }

    async fn reject_unsafe_provider_operations(
        &self,
        context: &PortContext,
        owner_operation: &'static str,
        tenant_id: Uuid,
        collection_id: Uuid,
    ) -> Result<(), PortError> {
        let operations = self
            .operation_journal
            .list_by_collection(tenant_id, collection_id)
            .await
            .map_err(|error| payment_error_to_port_error(context, owner_operation, error))?;
        for operation in operations {
            let unsafe_status = match operation.operation.as_str() {
                "cancel" => matches!(
                    operation.status.as_str(),
                    PROVIDER_OPERATION_EXECUTING | PROVIDER_OPERATION_RECONCILIATION_REQUIRED
                ),
                _ => matches!(
                    operation.status.as_str(),
                    PROVIDER_OPERATION_EXECUTING
                        | PROVIDER_OPERATION_SUCCEEDED
                        | PROVIDER_OPERATION_RECONCILIATION_REQUIRED
                ),
            };
            if unsafe_status {
                return Err(manual_reconciliation(
                    context,
                    owner_operation,
                    "payment provider operation has an unresolved external outcome",
                ));
            }
        }
        Ok(())
    }

    async fn commit_completed_cancel_if_needed(
        &self,
        context: &PortContext,
        owner_operation: &'static str,
        tenant_id: Uuid,
        collection_id: Uuid,
    ) -> Result<(), PortError> {
        let operations = self
            .operation_journal
            .list_by_collection(tenant_id, collection_id)
            .await
            .map_err(|error| payment_error_to_port_error(context, owner_operation, error))?;
        for operation in operations {
            if operation.operation == "cancel" && operation.status == PROVIDER_OPERATION_SUCCEEDED {
                self.operation_journal
                    .mark_committed(operation.id)
                    .await
                    .map_err(|error| {
                        tracing::error!(
                            operation_id = %operation.id,
                            error = ?error,
                            correlation_id = %context.correlation_id,
                            tenant_id = %context.tenant_id,
                            operation = owner_operation,
                            code = "payment.checkout_compensation_commit_checkpoint_failed",
                            "payment compensation could not commit recovered cancel operation"
                        );
                        manual_reconciliation(
                            context,
                            owner_operation,
                            "payment provider cancellation succeeded but its local checkpoint is incomplete",
                        )
                    })?;
            }
        }
        Ok(())
    }

    async fn execute_provider_cancel(
        &self,
        context: &PortContext,
        owner_operation: &'static str,
        tenant_id: Uuid,
        collection: &PaymentCollectionResponse,
        reason: Option<&str>,
        metadata: Value,
    ) -> Result<ProviderCancelOutcome, PortError> {
        let provider_id = collection
            .provider_id
            .clone()
            .unwrap_or_else(|| MANUAL_PAYMENT_PROVIDER_ID.to_string());
        // This key is intentionally identical to the pre-port
        // PaymentOrchestrationService key. Upgraded retries must adopt the
        // existing provider journal row instead of executing a second cancel.
        let idempotency_key = format!("payment_collection:{}:cancel", collection.id);
        // This request metadata is intentionally identical to the legacy
        // journaled cancel payload for the same reason.
        let mut provider_metadata = merge_metadata(
            metadata,
            serde_json::json!({
                "commerce_orchestration": {
                    "operation": "cancel_payment_collection",
                    "reason": reason,
                }
            }),
        );
        self.attach_provider_payment_id(
            context,
            owner_operation,
            tenant_id,
            collection.id,
            provider_id.as_str(),
            &mut provider_metadata,
        )
        .await?;

        let provider_request = PaymentProviderOperationRequest {
            tenant_id,
            collection_id: collection.id,
            amount: executable_payment_amount(collection),
            currency_code: collection.currency_code.clone(),
            idempotency_key: Some(idempotency_key.clone()),
            metadata: provider_metadata,
        };
        let request_payload = serde_json::to_value(&provider_request).map_err(|error| {
            tracing::error!(
                error = ?error,
                correlation_id = %context.correlation_id,
                tenant_id = %context.tenant_id,
                operation = owner_operation,
                code = "payment.checkout_compensation_encoding_failed",
                "payment compensation request encoding failed"
            );
            PortError::invariant_violation(
                "payment.checkout_compensation_encoding_failed",
                "payment compensation request could not be encoded",
            )
        })?;
        let operation = self
            .operation_journal
            .begin(BeginProviderOperation {
                tenant_id,
                payment_collection_id: collection.id,
                refund_id: None,
                operation: "cancel".to_string(),
                provider_id: provider_id.clone(),
                idempotency_key,
                request_payload,
            })
            .await
            .map_err(|error| payment_error_to_port_error(context, owner_operation, error))?;

        if let Some(result) = persisted_cancel_result(context, owner_operation, &operation)? {
            return Ok(ProviderCancelOutcome {
                operation_id: operation.id,
                metadata: result.metadata,
            });
        }
        if matches!(
            operation.status.as_str(),
            PROVIDER_OPERATION_EXECUTING | PROVIDER_OPERATION_RECONCILIATION_REQUIRED
        ) {
            return Err(manual_reconciliation(
                context,
                owner_operation,
                "payment provider cancellation has an unresolved external outcome",
            ));
        }

        let claimed = self
            .operation_journal
            .claim_execution(operation.id)
            .await
            .map_err(|error| payment_error_to_port_error(context, owner_operation, error))?;
        if claimed.is_none() {
            let current = self
                .operation_journal
                .get(operation.id)
                .await
                .map_err(|error| payment_error_to_port_error(context, owner_operation, error))?;
            if let Some(result) = persisted_cancel_result(context, owner_operation, &current)? {
                return Ok(ProviderCancelOutcome {
                    operation_id: current.id,
                    metadata: result.metadata,
                });
            }
            return Err(manual_reconciliation(
                context,
                owner_operation,
                "payment provider cancellation is already executing or requires reconciliation",
            ));
        }

        let provider_result = match self
            .provider_registry
            .execute_cancel(provider_id.as_str(), provider_request)
            .await
        {
            Ok(result) => result,
            Err(error) => {
                let code = stable_payment_error_code(&error);
                let checkpoint = if error.requires_provider_reconciliation() {
                    self.operation_journal
                        .mark_reconciliation_required(operation.id, code)
                        .await
                } else {
                    self.operation_journal
                        .mark_provider_error(operation.id, code)
                        .await
                };
                if let Err(checkpoint_error) = checkpoint {
                    tracing::error!(
                        operation_id = %operation.id,
                        error = ?checkpoint_error,
                        correlation_id = %context.correlation_id,
                        tenant_id = %context.tenant_id,
                        operation = owner_operation,
                        code = "payment.checkout_compensation_provider_failure_checkpoint_failed",
                        "payment compensation provider failure checkpoint failed"
                    );
                    return Err(manual_reconciliation(
                        context,
                        owner_operation,
                        "payment provider cancellation failed without a durable outcome checkpoint",
                    ));
                }
                return Err(payment_error_to_port_error(context, owner_operation, error));
            }
        };
        let result_payload = serde_json::to_value(&provider_result).map_err(|error| {
            tracing::error!(
                operation_id = %operation.id,
                error = ?error,
                correlation_id = %context.correlation_id,
                tenant_id = %context.tenant_id,
                operation = owner_operation,
                code = "payment.checkout_compensation_provider_result_encoding_failed",
                "payment compensation provider result encoding failed"
            );
            manual_reconciliation(
                context,
                owner_operation,
                "payment provider cancellation succeeded but its result could not be persisted",
            )
        })?;
        self.operation_journal
            .mark_provider_succeeded(
                operation.id,
                provider_result.external_reference.clone(),
                result_payload,
            )
            .await
            .map_err(|error| {
                tracing::error!(
                    operation_id = %operation.id,
                    error = ?error,
                    correlation_id = %context.correlation_id,
                    tenant_id = %context.tenant_id,
                    operation = owner_operation,
                    code = "payment.checkout_compensation_provider_checkpoint_failed",
                    "payment compensation provider success checkpoint failed"
                );
                manual_reconciliation(
                    context,
                    owner_operation,
                    "payment provider cancellation succeeded but its durable checkpoint failed",
                )
            })?;
        Ok(ProviderCancelOutcome {
            operation_id: operation.id,
            metadata: provider_result.metadata,
        })
    }

    async fn attach_provider_payment_id(
        &self,
        context: &PortContext,
        owner_operation: &'static str,
        tenant_id: Uuid,
        collection_id: Uuid,
        provider_id: &str,
        metadata: &mut Value,
    ) -> Result<(), PortError> {
        if provider_id == MANUAL_PAYMENT_PROVIDER_ID
            || metadata
                .get("provider_payment_id")
                .and_then(Value::as_str)
                .is_some_and(|value| !value.trim().is_empty())
        {
            return Ok(());
        }
        let authorize_key = format!("payment_collection:{collection_id}:authorize");
        let authorize = self
            .operation_journal
            .find_by_key(tenant_id, provider_id, authorize_key.as_str())
            .await
            .map_err(|error| payment_error_to_port_error(context, owner_operation, error))?
            .ok_or_else(|| {
                manual_reconciliation(
                    context,
                    owner_operation,
                    "payment provider cancellation has no durable authorize identity",
                )
            })?;
        if authorize.status != PROVIDER_OPERATION_COMMITTED {
            return Err(manual_reconciliation(
                context,
                owner_operation,
                "payment authorization is not durably committed",
            ));
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
                    .and_then(|value| value.get("external_reference"))
                    .and_then(Value::as_str)
                    .map(str::trim)
                    .filter(|value| !value.is_empty())
                    .map(str::to_string)
            })
            .ok_or_else(|| {
                manual_reconciliation(
                    context,
                    owner_operation,
                    "payment authorization has no durable provider payment identity",
                )
            })?;
        insert_metadata_string(metadata, "provider_payment_id", provider_payment_id)
    }

    async fn cancel_local_collection(
        &self,
        context: &PortContext,
        owner_operation: &'static str,
        tenant_id: Uuid,
        collection: PaymentCollectionResponse,
        reason: Option<String>,
        metadata: Value,
    ) -> Result<PaymentCollectionResponse, PortError> {
        match self
            .payment_service
            .cancel_collection(
                tenant_id,
                collection.id,
                CancelPaymentInput { reason, metadata },
            )
            .await
        {
            Ok(cancelled) => Ok(cancelled),
            Err(PaymentError::InvalidTransition { .. }) => {
                let current = self
                    .payment_service
                    .get_collection(tenant_id, collection.id)
                    .await
                    .map_err(|error| {
                        payment_error_to_port_error(context, owner_operation, error)
                    })?;
                if current.status_kind() == PaymentCollectionStatusKind::Cancelled {
                    Ok(current)
                } else {
                    Err(PortError::conflict(
                        "payment.checkout_compensation_state_conflict",
                        "payment collection changed while compensation was being applied",
                    ))
                }
            }
            Err(error) => Err(payment_error_to_port_error(context, owner_operation, error)),
        }
    }
}

pub fn in_process_checkout_payment_compensation_port(
    db: DatabaseConnection,
) -> Arc<dyn CheckoutPaymentCompensationPort> {
    Arc::new(InProcessCheckoutPaymentCompensationPort::new(db))
}

#[async_trait]
impl CheckoutPaymentCompensationPort for InProcessCheckoutPaymentCompensationPort {
    async fn compensate_checkout_payment(
        &self,
        context: PortContext,
        request: CheckoutPaymentCompensationRequest,
    ) -> Result<Option<PaymentCollectionStatusSnapshot>, PortError> {
        let owner_operation = COMPENSATE_CHECKOUT_PAYMENT_OPERATION;
        context.require_policy(PortCallPolicy::write())?;
        context.require_write_semantics()?;
        let tenant_id = parse_tenant_id(&context, owner_operation)?;
        require_operation_context(&context, owner_operation, request.checkout_operation_id)?;
        let Some(collection_id) = request.collection_id else {
            return Ok(None);
        };
        if request.checkout_operation_id.is_nil() || collection_id.is_nil() {
            return Err(PortError::validation(
                "payment.checkout_compensation_identity_invalid",
                "checkout operation and payment collection identity must be non-nil UUIDs",
            ));
        }

        let collection = self
            .payment_service
            .get_collection(tenant_id, collection_id)
            .await
            .map_err(|error| payment_error_to_port_error(&context, owner_operation, error))?;
        self.reject_unsafe_provider_operations(&context, owner_operation, tenant_id, collection_id)
            .await?;
        match collection.status_kind() {
            PaymentCollectionStatusKind::Cancelled => {
                self.commit_completed_cancel_if_needed(
                    &context,
                    owner_operation,
                    tenant_id,
                    collection_id,
                )
                .await?;
                return Ok(Some(PaymentCollectionStatusSnapshot::from_response(
                    &collection,
                )));
            }
            PaymentCollectionStatusKind::Captured => {
                return Err(manual_reconciliation(
                    &context,
                    owner_operation,
                    "captured payment collection must be reconciled through refund policy",
                ));
            }
            PaymentCollectionStatusKind::Pending | PaymentCollectionStatusKind::Authorized => {}
            PaymentCollectionStatusKind::Unknown => {
                return Err(manual_reconciliation(
                    &context,
                    owner_operation,
                    "payment collection lifecycle is unknown and requires manual reconciliation",
                ));
            }
        }

        let reason = request
            .reason
            .filter(|value| !value.trim().is_empty())
            .or_else(|| Some("checkout_compensation".to_string()));
        let provider_cancel = if should_cancel_provider(&collection) {
            Some(
                self.execute_provider_cancel(
                    &context,
                    owner_operation,
                    tenant_id,
                    &collection,
                    reason.as_deref(),
                    request.metadata.clone(),
                )
                .await?,
            )
        } else {
            None
        };
        let local_metadata = provider_cancel
            .as_ref()
            .map(|outcome| merge_metadata(request.metadata.clone(), outcome.metadata.clone()))
            .unwrap_or(request.metadata);
        let cancelled = self
            .cancel_local_collection(
                &context,
                owner_operation,
                tenant_id,
                collection,
                reason,
                local_metadata,
            )
            .await?;
        if let Some(outcome) = provider_cancel {
            self.operation_journal
                .mark_committed(outcome.operation_id)
                .await
                .map_err(|error| {
                    tracing::error!(
                        operation_id = %outcome.operation_id,
                        error = ?error,
                        correlation_id = %context.correlation_id,
                        tenant_id = %context.tenant_id,
                        operation = owner_operation,
                        code = "payment.checkout_compensation_commit_checkpoint_failed",
                        "payment compensation local commit checkpoint failed"
                    );
                    manual_reconciliation(
                        &context,
                        owner_operation,
                        "payment collection was cancelled but its provider operation checkpoint is incomplete",
                    )
                })?;
        }
        Ok(Some(PaymentCollectionStatusSnapshot::from_response(
            &cancelled,
        )))
    }
}

struct ProviderCancelOutcome {
    operation_id: Uuid,
    metadata: Value,
}

fn persisted_cancel_result(
    context: &PortContext,
    owner_operation: &'static str,
    operation: &crate::entities::provider_operation::Model,
) -> Result<Option<PaymentProviderOperationResult>, PortError> {
    match operation.status.as_str() {
        PROVIDER_OPERATION_COMMITTED | PROVIDER_OPERATION_SUCCEEDED => {
            let value = operation.provider_result.clone().ok_or_else(|| {
                manual_reconciliation(
                    context,
                    owner_operation,
                    "payment provider cancellation checkpoint has no normalized result",
                )
            })?;
            serde_json::from_value(value).map(Some).map_err(|error| {
                tracing::error!(
                    operation_id = %operation.id,
                    error = ?error,
                    correlation_id = %context.correlation_id,
                    tenant_id = %context.tenant_id,
                    operation = owner_operation,
                    code = "payment.provider_invalid_response",
                    "payment compensation provider checkpoint is malformed"
                );
                manual_reconciliation(
                    context,
                    owner_operation,
                    "payment provider cancellation checkpoint is malformed",
                )
            })
        }
        PROVIDER_OPERATION_RECONCILIATION_REQUIRED | PROVIDER_OPERATION_EXECUTING => {
            Err(manual_reconciliation(
                context,
                owner_operation,
                "payment provider cancellation has an unresolved external outcome",
            ))
        }
        PROVIDER_OPERATION_PENDING | PROVIDER_OPERATION_ERROR => Ok(None),
        _ => Err(PortError::conflict(
            "payment.checkout_compensation_provider_state_conflict",
            "payment provider cancellation is in an unsupported state",
        )),
    }
}

fn should_cancel_provider(collection: &PaymentCollectionResponse) -> bool {
    collection.status_kind() == PaymentCollectionStatusKind::Authorized
        || collection.authorized_amount > Decimal::ZERO
        || collection.provider_id.is_some()
}

fn executable_payment_amount(collection: &PaymentCollectionResponse) -> Decimal {
    if collection.captured_amount > Decimal::ZERO {
        collection.captured_amount
    } else if collection.authorized_amount > Decimal::ZERO {
        collection.authorized_amount
    } else {
        collection.amount
    }
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

fn insert_metadata_string(metadata: &mut Value, key: &str, value: String) -> Result<(), PortError> {
    if metadata.is_null() {
        *metadata = serde_json::json!({});
    }
    let object = metadata.as_object_mut().ok_or_else(|| {
        PortError::validation(
            "payment.checkout_compensation_metadata_invalid",
            "payment compensation metadata must be a JSON object",
        )
    })?;
    if let Some(existing) = object.get(key).and_then(Value::as_str) {
        if existing != value {
            return Err(PortError::conflict(
                "payment.checkout_compensation_provider_identity_conflict",
                "payment provider identity conflicts with the durable authorization",
            ));
        }
        return Ok(());
    }
    object.insert(key.to_string(), Value::String(value));
    Ok(())
}

fn require_operation_context(
    context: &PortContext,
    owner_operation: &'static str,
    checkout_operation_id: Uuid,
) -> Result<(), PortError> {
    let context_operation = context
        .causation_id
        .as_deref()
        .and_then(|value| Uuid::parse_str(value).ok());
    if context_operation != Some(checkout_operation_id) {
        tracing::warn!(
            causation_id = ?context.causation_id,
            checkout_operation_id = %checkout_operation_id,
            correlation_id = %context.correlation_id,
            tenant_id = %context.tenant_id,
            operation = owner_operation,
            code = "payment.checkout_compensation_causation_invalid",
            "payment checkout compensation causation context is invalid"
        );
        return Err(PortError::validation(
            "payment.checkout_compensation_causation_invalid",
            "payment request context is invalid",
        ));
    }
    Ok(())
}

fn parse_tenant_id(
    context: &PortContext,
    owner_operation: &'static str,
) -> Result<Uuid, PortError> {
    Uuid::parse_str(&context.tenant_id).map_err(|error| {
        tracing::warn!(
            error = ?error,
            internal_tenant_id = %context.tenant_id,
            correlation_id = %context.correlation_id,
            tenant_id = %context.tenant_id,
            operation = owner_operation,
            code = "payment.tenant_id_invalid",
            "payment checkout compensation tenant context is invalid"
        );
        PortError::validation(
            "payment.tenant_id_invalid",
            "payment request context is invalid",
        )
    })
}

fn manual_reconciliation(
    context: &PortContext,
    owner_operation: &'static str,
    internal_message: &'static str,
) -> PortError {
    tracing::error!(
        internal_message,
        correlation_id = %context.correlation_id,
        tenant_id = %context.tenant_id,
        operation = owner_operation,
        code = "payment.checkout_compensation_manual_reconciliation",
        "payment checkout compensation requires manual reconciliation"
    );
    PortError::new(
        PortErrorKind::Conflict,
        "payment.checkout_compensation_manual_reconciliation",
        "payment checkout compensation requires manual reconciliation",
        false,
    )
}

fn stable_payment_error_code(error: &PaymentError) -> &'static str {
    match error {
        PaymentError::Database(_) => "payment.database_unavailable",
        PaymentError::Validation(_) => "payment.validation",
        PaymentError::PaymentCollectionNotFound(_) => "payment.collection_not_found",
        PaymentError::PaymentNotFound(_) => "payment.payment_not_found",
        PaymentError::RefundNotFound(_) => "payment.refund_not_found",
        PaymentError::InvalidTransition { .. } => "payment.invalid_transition",
        PaymentError::ProviderUnavailable { .. } => "payment.provider_unavailable",
        PaymentError::ProviderRejected { .. } => "payment.provider_rejected",
        PaymentError::ProviderInvalidResponse { .. } => "payment.provider_invalid_response",
        PaymentError::ProviderOutcomeUnknown { .. } => "payment.provider_outcome_unknown",
        PaymentError::ProviderConfiguration { .. } => "payment.provider_not_configured",
    }
}

fn payment_error_to_port_error(
    context: &PortContext,
    owner_operation: &'static str,
    error: PaymentError,
) -> PortError {
    let code = stable_payment_error_code(&error);
    tracing::error!(
        error = ?error,
        correlation_id = %context.correlation_id,
        tenant_id = %context.tenant_id,
        operation = owner_operation,
        code,
        "payment checkout compensation owner operation failed"
    );
    match error {
        PaymentError::Database(_) => PortError::unavailable(
            "payment.database_unavailable",
            "payment storage is temporarily unavailable",
        ),
        PaymentError::Validation(_) => PortError::validation(
            "payment.checkout_compensation_validation",
            "payment compensation request is invalid",
        ),
        PaymentError::PaymentCollectionNotFound(_) => PortError::not_found(
            "payment.collection_not_found",
            "payment collection was not found",
        ),
        PaymentError::PaymentNotFound(_) => {
            PortError::not_found("payment.payment_not_found", "payment was not found")
        }
        PaymentError::RefundNotFound(_) => {
            PortError::not_found("payment.refund_not_found", "refund was not found")
        }
        PaymentError::InvalidTransition { .. } => PortError::conflict(
            "payment.checkout_compensation_state_conflict",
            "payment lifecycle conflicts with checkout compensation",
        ),
        PaymentError::ProviderUnavailable { .. } => PortError::unavailable(
            "payment.provider_unavailable",
            "payment provider is temporarily unavailable",
        ),
        PaymentError::ProviderRejected { .. } => PortError::conflict(
            "payment.provider_rejected",
            "payment provider rejected the requested operation",
        ),
        PaymentError::ProviderInvalidResponse { .. } => PortError::invariant_violation(
            "payment.provider_invalid_response",
            "payment provider response could not be applied safely",
        ),
        PaymentError::ProviderOutcomeUnknown { .. } => manual_reconciliation(
            context,
            owner_operation,
            "payment provider cancellation outcome is unknown",
        ),
        PaymentError::ProviderConfiguration { .. } => PortError::invariant_violation(
            "payment.provider_not_configured",
            "payment provider is not configured",
        ),
    }
}
