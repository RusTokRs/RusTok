use std::sync::Arc;

use async_trait::async_trait;
use rustok_api::{PortCallPolicy, PortContext, PortError, PortErrorKind};
use sea_orm::DatabaseConnection;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use uuid::Uuid;

use crate::dto::{
    CancelRefundInput, CompleteRefundInput, CreateRefundInput, PaymentCollectionResponse,
    PaymentCollectionStatusKind, RefundResponse,
};
use crate::providers::{
    MANUAL_PAYMENT_PROVIDER_ID, PaymentProviderOperationRequest, PaymentProviderOperationResult,
    PaymentProviderRegistry,
};
use crate::{
    BeginProviderOperation, PROVIDER_OPERATION_COMMITTED, PROVIDER_OPERATION_EXECUTING,
    PROVIDER_OPERATION_RECONCILIATION_REQUIRED, PROVIDER_OPERATION_SUCCEEDED, PaymentError,
    PaymentProviderOperationJournal, PaymentRefundCreationService, PaymentService,
};

const ADMIN_REFUND_COMMAND_BOUNDARY: &str = "payment_admin_refund_command_port";
const UNKNOWN_PROVIDER_ID: &str = "payment-provider";

#[async_trait]
pub trait PaymentAdminRefundCommandPort: Send + Sync {
    async fn create_refund(
        &self,
        context: PortContext,
        request: CreateAdminRefundRequest,
    ) -> Result<RefundResponse, PortError>;

    async fn complete_refund(
        &self,
        context: PortContext,
        request: CompleteAdminRefundRequest,
    ) -> Result<RefundResponse, PortError>;

    async fn cancel_refund(
        &self,
        context: PortContext,
        request: CancelAdminRefundRequest,
    ) -> Result<RefundResponse, PortError>;
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreateAdminRefundRequest {
    pub collection_id: Uuid,
    pub creation_key: String,
    pub input: CreateRefundInput,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CompleteAdminRefundRequest {
    pub refund_id: Uuid,
    pub input: CompleteRefundInput,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CancelAdminRefundRequest {
    pub refund_id: Uuid,
    pub input: CancelRefundInput,
}

pub struct InProcessPaymentAdminRefundCommandPort {
    payment_service: PaymentService,
    refund_creation_service: PaymentRefundCreationService,
    operation_journal: PaymentProviderOperationJournal,
    provider_registry: PaymentProviderRegistry,
}

impl InProcessPaymentAdminRefundCommandPort {
    pub fn new(db: DatabaseConnection, provider_registry: PaymentProviderRegistry) -> Self {
        Self {
            payment_service: PaymentService::new(db.clone()),
            refund_creation_service: PaymentRefundCreationService::new(db.clone()),
            operation_journal: PaymentProviderOperationJournal::new(db),
            provider_registry,
        }
    }
}

pub fn in_process_payment_admin_refund_command_port(
    db: DatabaseConnection,
    provider_registry: PaymentProviderRegistry,
) -> Arc<dyn PaymentAdminRefundCommandPort> {
    Arc::new(InProcessPaymentAdminRefundCommandPort::new(
        db,
        provider_registry,
    ))
}

#[derive(Clone)]
pub struct PaymentAdminRefundCommandRuntime {
    command_port: Arc<dyn PaymentAdminRefundCommandPort>,
}

impl PaymentAdminRefundCommandRuntime {
    pub fn new(command_port: Arc<dyn PaymentAdminRefundCommandPort>) -> Self {
        Self { command_port }
    }

    pub fn in_process(db: DatabaseConnection, provider_registry: PaymentProviderRegistry) -> Self {
        Self::new(in_process_payment_admin_refund_command_port(
            db,
            provider_registry,
        ))
    }

    pub fn command_port(&self) -> Arc<dyn PaymentAdminRefundCommandPort> {
        self.command_port.clone()
    }
}

#[async_trait]
impl PaymentAdminRefundCommandPort for InProcessPaymentAdminRefundCommandPort {
    async fn create_refund(
        &self,
        context: PortContext,
        request: CreateAdminRefundRequest,
    ) -> Result<RefundResponse, PortError> {
        const OPERATION: &str = "create_admin_refund";
        require_refund_write_admission(&context, OPERATION)?;
        let tenant_id = parse_tenant_id(&context, OPERATION)?;

        let collection = self
            .payment_service
            .get_collection(tenant_id, request.collection_id)
            .await
            .map_err(|error| map_payment_error(&context, OPERATION, error))?;
        if collection.status_kind() != PaymentCollectionStatusKind::Captured {
            return Err(map_payment_error(
                &context,
                OPERATION,
                PaymentError::InvalidTransition {
                    from: collection.status,
                    to: "pending".to_string(),
                },
            ));
        }
        let provider_id = provider_id_for_collection(&collection);
        let refund = self
            .refund_creation_service
            .create_or_replay(
                tenant_id,
                request.collection_id,
                request.creation_key,
                request.input.clone(),
            )
            .await
            .map_err(|error| map_payment_error(&context, OPERATION, error))?;

        let provider_request = PaymentProviderOperationRequest {
            tenant_id,
            collection_id: request.collection_id,
            amount: refund.amount,
            currency_code: refund.currency_code.clone(),
            idempotency_key: Some(format!("payment_refund:{}", refund.id)),
            metadata: merge_provider_context(
                request.input.metadata,
                serde_json::json!({
                    "commerce_orchestration": {
                        "operation": "create_refund",
                        "refund_id": refund.id,
                        "reason": request.input.reason,
                    }
                }),
            ),
        };
        let journaled = self
            .execute_refund_provider_operation(
                &context,
                OPERATION,
                refund.id,
                provider_id.as_str(),
                provider_request,
            )
            .await?;
        self.mark_refund_journal_committed(&context, OPERATION, refund.id, journaled.operation_id)
            .await?;
        Ok(refund)
    }

    async fn complete_refund(
        &self,
        context: PortContext,
        request: CompleteAdminRefundRequest,
    ) -> Result<RefundResponse, PortError> {
        const OPERATION: &str = "complete_admin_refund";
        require_refund_write_admission(&context, OPERATION)?;
        let tenant_id = parse_tenant_id(&context, OPERATION)?;
        self.payment_service
            .complete_refund(tenant_id, request.refund_id, request.input)
            .await
            .map_err(|error| map_payment_error(&context, OPERATION, error))
    }

    async fn cancel_refund(
        &self,
        context: PortContext,
        request: CancelAdminRefundRequest,
    ) -> Result<RefundResponse, PortError> {
        const OPERATION: &str = "cancel_admin_refund";
        require_refund_write_admission(&context, OPERATION)?;
        let tenant_id = parse_tenant_id(&context, OPERATION)?;
        self.payment_service
            .cancel_refund(tenant_id, request.refund_id, request.input)
            .await
            .map_err(|error| map_payment_error(&context, OPERATION, error))
    }
}

struct JournaledRefundProviderResult {
    operation_id: Uuid,
    #[allow(dead_code)]
    result: PaymentProviderOperationResult,
}

impl InProcessPaymentAdminRefundCommandPort {
    async fn execute_refund_provider_operation(
        &self,
        context: &PortContext,
        owner_operation: &'static str,
        refund_id: Uuid,
        provider_id: &str,
        request: PaymentProviderOperationRequest,
    ) -> Result<JournaledRefundProviderResult, PortError> {
        let request = self
            .enrich_refund_provider_request(
                context,
                owner_operation,
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
                PortError::validation(
                    "payment.provider_idempotency_key_missing",
                    "payment provider operation requires idempotency identity",
                )
            })?
            .to_string();
        let request_payload = serde_json::to_value(&request).map_err(|_| {
            PortError::validation(
                "payment.provider_request_invalid",
                "payment provider request could not be normalized",
            )
        })?;
        let journal_operation = self
            .operation_journal
            .begin(BeginProviderOperation {
                tenant_id: request.tenant_id,
                payment_collection_id: request.collection_id,
                refund_id: Some(refund_id),
                operation: "refund".to_string(),
                provider_id: provider_id.to_string(),
                idempotency_key,
                request_payload,
            })
            .await
            .map_err(|error| map_payment_error(context, owner_operation, error))?;

        if let Some(result) = persisted_provider_result(&journal_operation)
            .map_err(|error| map_payment_error(context, owner_operation, error))?
        {
            return Ok(JournaledRefundProviderResult {
                operation_id: journal_operation.id,
                result,
            });
        }

        let claimed = self
            .operation_journal
            .claim_execution(journal_operation.id)
            .await
            .map_err(|error| map_payment_error(context, owner_operation, error))?;
        if claimed.is_none() {
            let current = self
                .operation_journal
                .get(journal_operation.id)
                .await
                .map_err(|error| map_payment_error(context, owner_operation, error))?;
            if let Some(result) = persisted_provider_result(&current)
                .map_err(|error| map_payment_error(context, owner_operation, error))?
            {
                return Ok(JournaledRefundProviderResult {
                    operation_id: current.id,
                    result,
                });
            }
            return Err(PortError::validation(
                "payment.provider_operation_in_progress",
                "payment provider operation is already in progress",
            ));
        }

        let provider_result = match self
            .provider_registry
            .execute_refund(provider_id, request)
            .await
        {
            Ok(result) => result,
            Err(error) => {
                let checkpoint = if error.requires_provider_reconciliation() {
                    self.operation_journal
                        .mark_reconciliation_required(
                            journal_operation.id,
                            "payment.refund_provider_outcome_requires_reconciliation",
                        )
                        .await
                } else {
                    self.operation_journal
                        .mark_provider_error(
                            journal_operation.id,
                            "payment.refund_provider_operation_failed",
                        )
                        .await
                };
                if checkpoint.is_err() {
                    return Err(map_reserved_refund_error(
                        context,
                        owner_operation,
                        PaymentError::provider_outcome_unknown(provider_id, "refund"),
                    ));
                }
                return Err(map_reserved_refund_error(context, owner_operation, error));
            }
        };

        let result_payload = match serde_json::to_value(&provider_result) {
            Ok(payload) => payload,
            Err(_) => {
                let _ = self
                    .operation_journal
                    .mark_reconciliation_required(
                        journal_operation.id,
                        "payment.refund_provider_result_serialization_failed",
                    )
                    .await;
                return Err(map_reserved_refund_error(
                    context,
                    owner_operation,
                    PaymentError::provider_outcome_unknown(provider_id, "refund"),
                ));
            }
        };
        if self
            .operation_journal
            .mark_provider_succeeded(
                journal_operation.id,
                provider_result.external_reference.clone(),
                result_payload,
            )
            .await
            .is_err()
        {
            let _ = self
                .operation_journal
                .mark_reconciliation_required(
                    journal_operation.id,
                    "payment.refund_provider_success_checkpoint_failed",
                )
                .await;
            return Err(map_reserved_refund_error(
                context,
                owner_operation,
                PaymentError::provider_outcome_unknown(provider_id, "refund"),
            ));
        }

        Ok(JournaledRefundProviderResult {
            operation_id: journal_operation.id,
            result: provider_result,
        })
    }

    async fn enrich_refund_provider_request(
        &self,
        context: &PortContext,
        owner_operation: &'static str,
        refund_id: Uuid,
        provider_id: &str,
        mut request: PaymentProviderOperationRequest,
    ) -> Result<PaymentProviderOperationRequest, PortError> {
        insert_metadata_string(&mut request.metadata, "refund_id", refund_id.to_string())
            .map_err(|error| map_payment_error(context, owner_operation, error))?;
        if provider_id == MANUAL_PAYMENT_PROVIDER_ID {
            return Ok(request);
        }
        if metadata_string(&request.metadata, "provider_payment_id").is_some() {
            return Ok(request);
        }

        let authorize_key = format!("payment_collection:{}:authorize", request.collection_id);
        let authorize = self
            .operation_journal
            .find_by_key(request.tenant_id, provider_id, authorize_key.as_str())
            .await
            .map_err(|error| map_payment_error(context, owner_operation, error))?
            .ok_or_else(|| {
                map_payment_error(
                    context,
                    owner_operation,
                    PaymentError::Validation(
                        "provider refund requires a completed authorize operation".to_string(),
                    ),
                )
            })?;
        if !matches!(
            authorize.status.as_str(),
            PROVIDER_OPERATION_COMMITTED
                | PROVIDER_OPERATION_SUCCEEDED
                | PROVIDER_OPERATION_RECONCILIATION_REQUIRED
        ) {
            return Err(map_payment_error(
                context,
                owner_operation,
                PaymentError::Validation(
                    "provider refund requires a completed authorize operation".to_string(),
                ),
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
                    .and_then(|result| result.get("external_reference"))
                    .and_then(Value::as_str)
                    .map(str::trim)
                    .filter(|value| !value.is_empty())
                    .map(str::to_string)
            })
            .ok_or_else(|| {
                map_payment_error(
                    context,
                    owner_operation,
                    PaymentError::provider_outcome_unknown(provider_id, "authorize"),
                )
            })?;
        insert_metadata_string(
            &mut request.metadata,
            "provider_payment_id",
            provider_payment_id,
        )
        .map_err(|error| map_payment_error(context, owner_operation, error))?;
        Ok(request)
    }

    async fn mark_refund_journal_committed(
        &self,
        context: &PortContext,
        owner_operation: &'static str,
        refund_id: Uuid,
        operation_id: Uuid,
    ) -> Result<(), PortError> {
        if self
            .operation_journal
            .mark_committed(operation_id)
            .await
            .is_err()
        {
            let _ = self
                .operation_journal
                .mark_reconciliation_required(
                    operation_id,
                    "payment.refund_local_commit_checkpoint_failed",
                )
                .await;
            tracing::error!(
                owner = "rustok_payment",
                operation = owner_operation,
                refund_id_non_nil = !refund_id.is_nil(),
                operation_id_non_nil = !operation_id.is_nil(),
                correlation_id = %context.correlation_id,
                code = "payment.refund_commit_checkpoint_failed",
                boundary = ADMIN_REFUND_COMMAND_BOUNDARY,
                "refund provider success could not be committed locally"
            );
            return Err(map_reserved_refund_error(
                context,
                owner_operation,
                PaymentError::provider_outcome_unknown(UNKNOWN_PROVIDER_ID, "refund"),
            ));
        }
        Ok(())
    }
}

fn require_refund_write_admission(
    context: &PortContext,
    operation: &'static str,
) -> Result<(), PortError> {
    context
        .require_policy(PortCallPolicy::write())
        .inspect_err(|error| {
            log_port_error(context, operation, "policy", error);
        })
}

fn parse_tenant_id(context: &PortContext, operation: &'static str) -> Result<Uuid, PortError> {
    Uuid::parse_str(&context.tenant_id).map_err(|_| {
        let error = PortError::validation(
            "payment.tenant_id_invalid",
            "payment refund command tenant context is invalid",
        );
        log_port_error(context, operation, "tenant_id", &error);
        error
    })
}

fn map_reserved_refund_error(
    context: &PortContext,
    operation: &'static str,
    error: PaymentError,
) -> PortError {
    match error {
        PaymentError::ProviderOutcomeUnknown { .. }
        | PaymentError::ProviderInvalidResponse { .. } => {
            let mapped = PortError::conflict(
                "payment.refund_reserved_reconciliation_required",
                "refund remains reserved while provider outcome is reconciled",
            );
            log_mapped_error(
                context,
                operation,
                "reserved_refund_provider_outcome",
                &mapped,
            );
            mapped
        }
        PaymentError::ProviderUnavailable { .. } => {
            let mapped = PortError::unavailable(
                "payment.refund_reserved_provider_unavailable",
                "refund remains reserved and provider operation may be retried safely",
            );
            log_mapped_error(
                context,
                operation,
                "reserved_refund_provider_unavailable",
                &mapped,
            );
            mapped
        }
        other => map_payment_error(context, operation, other),
    }
}

fn map_payment_error(
    context: &PortContext,
    operation: &'static str,
    error: PaymentError,
) -> PortError {
    let error_variant = payment_error_variant(&error);
    let mapped = match error {
        PaymentError::Validation(_) => {
            PortError::validation("payment.validation", "payment request is invalid")
        }
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
            "payment.invalid_transition",
            "payment lifecycle conflicts with the requested operation",
        ),
        PaymentError::ProviderUnavailable { .. } => PortError::unavailable(
            "payment.provider_unavailable",
            "payment provider is temporarily unavailable",
        ),
        PaymentError::ProviderRejected { .. } => PortError::conflict(
            "payment.provider_rejected",
            "payment provider rejected the requested operation",
        ),
        PaymentError::ProviderInvalidResponse { .. } => PortError::new(
            PortErrorKind::InvariantViolation,
            "payment.provider_invalid_response",
            "payment provider response could not be applied safely",
            false,
        ),
        PaymentError::ProviderOutcomeUnknown { .. } => PortError::conflict(
            "payment.provider_outcome_unknown",
            "payment provider outcome requires reconciliation",
        ),
        PaymentError::ProviderConfiguration { .. } => PortError::new(
            PortErrorKind::InvariantViolation,
            "payment.provider_not_configured",
            "payment provider is not configured for the requested operation",
            false,
        ),
        PaymentError::Database(_) => PortError::unavailable(
            "payment.database_unavailable",
            "payment storage is temporarily unavailable",
        ),
    };
    log_mapped_error(context, operation, error_variant, &mapped);
    mapped
}

fn payment_error_variant(error: &PaymentError) -> &'static str {
    match error {
        PaymentError::Validation(_) => "validation",
        PaymentError::PaymentCollectionNotFound(_) => "payment_collection_not_found",
        PaymentError::PaymentNotFound(_) => "payment_not_found",
        PaymentError::RefundNotFound(_) => "refund_not_found",
        PaymentError::InvalidTransition { .. } => "invalid_transition",
        PaymentError::ProviderUnavailable { .. } => "provider_unavailable",
        PaymentError::ProviderRejected { .. } => "provider_rejected",
        PaymentError::ProviderInvalidResponse { .. } => "provider_invalid_response",
        PaymentError::ProviderOutcomeUnknown { .. } => "provider_outcome_unknown",
        PaymentError::ProviderConfiguration { .. } => "provider_configuration",
        PaymentError::Database(_) => "database",
    }
}

fn log_port_error(
    context: &PortContext,
    operation: &'static str,
    admission: &'static str,
    error: &PortError,
) {
    tracing::warn!(
        owner = "rustok_payment",
        operation,
        admission,
        correlation_id = %context.correlation_id,
        tenant_id_length = context.tenant_id.chars().count(),
        actor_id_length = context.actor.id.chars().count(),
        channel_present = context.channel.is_some(),
        locale_length = context.locale.chars().count(),
        deadline_ms = ?context.deadline_ms,
        code = %error.code,
        retryable = error.retryable,
        boundary = ADMIN_REFUND_COMMAND_BOUNDARY,
        "payment admin refund command admission failed"
    );
}

fn log_mapped_error(
    context: &PortContext,
    operation: &'static str,
    error_variant: &'static str,
    error: &PortError,
) {
    tracing::warn!(
        owner = "rustok_payment",
        operation,
        correlation_id = %context.correlation_id,
        tenant_id_length = context.tenant_id.chars().count(),
        actor_id_length = context.actor.id.chars().count(),
        channel_present = context.channel.is_some(),
        locale_length = context.locale.chars().count(),
        deadline_ms = ?context.deadline_ms,
        error_variant,
        public_code = %error.code,
        retryable = error.retryable,
        boundary = ADMIN_REFUND_COMMAND_BOUNDARY,
        "payment admin refund command returned a bounded owner error"
    );
}

fn persisted_provider_result(
    journal_operation: &crate::entities::provider_operation::Model,
) -> Result<Option<PaymentProviderOperationResult>, PaymentError> {
    if journal_operation.status == PROVIDER_OPERATION_EXECUTING {
        return Ok(None);
    }
    if !matches!(
        journal_operation.status.as_str(),
        PROVIDER_OPERATION_COMMITTED
            | PROVIDER_OPERATION_SUCCEEDED
            | PROVIDER_OPERATION_RECONCILIATION_REQUIRED
    ) {
        return Ok(None);
    }
    let Some(value) = journal_operation.provider_result.clone() else {
        return Err(PaymentError::provider_outcome_unknown(
            journal_operation.provider_id.as_str(),
            journal_operation.operation.as_str(),
        ));
    };
    serde_json::from_value(value).map(Some).map_err(|_| {
        PaymentError::provider_outcome_unknown(
            journal_operation.provider_id.as_str(),
            journal_operation.operation.as_str(),
        )
    })
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
        PaymentError::Validation(
            "payment provider operation metadata must be an object".to_string(),
        )
    })?;
    if let Some(existing) = object.get(key).and_then(Value::as_str) {
        if existing != value {
            return Err(PaymentError::Validation(
                "payment provider identity conflicts with owner identity".to_string(),
            ));
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

fn provider_id_for_collection(collection: &PaymentCollectionResponse) -> String {
    collection
        .provider_id
        .clone()
        .unwrap_or_else(|| MANUAL_PAYMENT_PROVIDER_ID.to_string())
}

fn merge_provider_context(current: Value, patch: Value) -> Value {
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
