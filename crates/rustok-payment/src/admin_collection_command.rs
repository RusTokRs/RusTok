use std::sync::Arc;

use async_trait::async_trait;
use rust_decimal::Decimal;
use rustok_api::{PortCallPolicy, PortContext, PortError, PortErrorKind};
use sea_orm::DatabaseConnection;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use uuid::Uuid;
use validator::Validate;

use crate::dto::{
    AuthorizePaymentInput, CancelPaymentInput, CapturePaymentInput, PaymentCollectionResponse,
    PaymentCollectionStatusKind,
};
use crate::providers::{
    MANUAL_PAYMENT_PROVIDER_ID, PaymentProviderOperationRequest, PaymentProviderOperationResult,
    PaymentProviderRegistry,
};
use crate::{
    BeginProviderOperation, PROVIDER_OPERATION_COMMITTED, PROVIDER_OPERATION_EXECUTING,
    PROVIDER_OPERATION_RECONCILIATION_REQUIRED, PROVIDER_OPERATION_SUCCEEDED, PaymentError,
    PaymentProviderOperationJournal, PaymentService,
};

const ADMIN_COLLECTION_COMMAND_BOUNDARY: &str = "payment_admin_collection_command_port";
const UNKNOWN_PROVIDER_ID: &str = "payment-provider";

#[async_trait]
pub trait PaymentAdminCollectionCommandPort: Send + Sync {
    async fn authorize_payment_collection(
        &self,
        context: PortContext,
        request: AuthorizeAdminPaymentCollectionRequest,
    ) -> Result<PaymentCollectionResponse, PortError>;

    async fn capture_payment_collection(
        &self,
        context: PortContext,
        request: CaptureAdminPaymentCollectionRequest,
    ) -> Result<PaymentCollectionResponse, PortError>;

    async fn cancel_payment_collection(
        &self,
        context: PortContext,
        request: CancelAdminPaymentCollectionRequest,
    ) -> Result<PaymentCollectionResponse, PortError>;
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuthorizeAdminPaymentCollectionRequest {
    pub collection_id: Uuid,
    pub input: AuthorizePaymentInput,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CaptureAdminPaymentCollectionRequest {
    pub collection_id: Uuid,
    pub input: CapturePaymentInput,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CancelAdminPaymentCollectionRequest {
    pub collection_id: Uuid,
    pub input: CancelPaymentInput,
}

pub struct InProcessPaymentAdminCollectionCommandPort {
    payment_service: PaymentService,
    operation_journal: PaymentProviderOperationJournal,
    provider_registry: PaymentProviderRegistry,
}

impl InProcessPaymentAdminCollectionCommandPort {
    pub fn new(db: DatabaseConnection, provider_registry: PaymentProviderRegistry) -> Self {
        Self {
            payment_service: PaymentService::new(db.clone()),
            operation_journal: PaymentProviderOperationJournal::new(db),
            provider_registry,
        }
    }
}

pub fn in_process_payment_admin_collection_command_port(
    db: DatabaseConnection,
    provider_registry: PaymentProviderRegistry,
) -> Arc<dyn PaymentAdminCollectionCommandPort> {
    Arc::new(InProcessPaymentAdminCollectionCommandPort::new(
        db,
        provider_registry,
    ))
}

#[derive(Clone)]
pub struct PaymentAdminCollectionCommandRuntime {
    command_port: Arc<dyn PaymentAdminCollectionCommandPort>,
}

impl PaymentAdminCollectionCommandRuntime {
    pub fn new(command_port: Arc<dyn PaymentAdminCollectionCommandPort>) -> Self {
        Self { command_port }
    }

    pub fn in_process(
        db: DatabaseConnection,
        provider_registry: PaymentProviderRegistry,
    ) -> Self {
        Self::new(in_process_payment_admin_collection_command_port(
            db,
            provider_registry,
        ))
    }

    pub fn command_port(&self) -> Arc<dyn PaymentAdminCollectionCommandPort> {
        self.command_port.clone()
    }
}

#[async_trait]
impl PaymentAdminCollectionCommandPort for InProcessPaymentAdminCollectionCommandPort {
    async fn authorize_payment_collection(
        &self,
        context: PortContext,
        request: AuthorizeAdminPaymentCollectionRequest,
    ) -> Result<PaymentCollectionResponse, PortError> {
        const OPERATION: &str = "authorize_admin_payment_collection";
        require_admin_collection_write_admission(&context, OPERATION)?;
        let tenant_id = parse_admin_collection_tenant_id(&context, OPERATION)?;
        request.input.validate().map_err(|_| {
            PortError::validation(
                "payment.validation",
                "payment authorization request is invalid",
            )
        })?;

        let collection = self
            .payment_service
            .get_collection(tenant_id, request.collection_id)
            .await
            .map_err(|error| map_payment_error(&context, OPERATION, error))?;
        let provider_id = request
            .input
            .provider_id
            .clone()
            .or_else(|| collection.provider_id.clone())
            .unwrap_or_else(|| MANUAL_PAYMENT_PROVIDER_ID.to_string());
        let idempotency_key = format!("payment_collection:{}:authorize", collection.id);

        match collection.status_kind() {
            PaymentCollectionStatusKind::Authorized | PaymentCollectionStatusKind::Captured => {
                self.commit_existing_provider_operation(
                    &context,
                    OPERATION,
                    tenant_id,
                    provider_id.as_str(),
                    idempotency_key.as_str(),
                    "authorize",
                )
                .await?;
                return Ok(collection);
            }
            PaymentCollectionStatusKind::Pending => {}
            PaymentCollectionStatusKind::Cancelled | PaymentCollectionStatusKind::Unknown => {
                return Err(map_payment_error(
                    &context,
                    OPERATION,
                    PaymentError::InvalidTransition {
                        from: collection.status,
                        to: "authorized".to_string(),
                    },
                ));
            }
        }

        let AuthorizePaymentInput {
            provider_id: _,
            provider_payment_id,
            amount,
            metadata,
        } = request.input;
        let requested_amount = amount.unwrap_or(collection.amount);
        let provider_request = PaymentProviderOperationRequest {
            tenant_id,
            collection_id: collection.id,
            amount: requested_amount,
            currency_code: collection.currency_code.clone(),
            idempotency_key: Some(idempotency_key),
            metadata: merge_provider_context(
                metadata.clone(),
                serde_json::json!({
                    "commerce_orchestration": {
                        "operation": "authorize_payment_collection",
                        "requested_provider_payment_id": provider_payment_id.clone(),
                    }
                }),
            ),
        };
        let journaled = self
            .execute_journaled_provider_operation(
                &context,
                OPERATION,
                "authorize",
                provider_id.as_str(),
                provider_request,
            )
            .await?;
        let provider_result = journaled.result;

        match self
            .payment_service
            .authorize_collection(
                tenant_id,
                collection.id,
                AuthorizePaymentInput {
                    provider_id: Some(provider_result.provider_id),
                    provider_payment_id: provider_result.external_reference.or(provider_payment_id),
                    amount: Some(provider_result.authorized_amount),
                    metadata: merge_provider_context(metadata, provider_result.metadata),
                },
            )
            .await
        {
            Ok(collection) => {
                self.mark_journal_committed(
                    &context,
                    OPERATION,
                    journaled.operation_id,
                    "authorize",
                )
                .await?;
                Ok(collection)
            }
            Err(error) => {
                self.mark_local_persistence_failed(
                    &context,
                    OPERATION,
                    journaled.operation_id,
                    "authorize",
                    &error,
                )
                .await;
                Err(self.local_persistence_after_provider_error(
                    &context,
                    OPERATION,
                    "authorize",
                    error,
                ))
            }
        }
    }

    async fn capture_payment_collection(
        &self,
        context: PortContext,
        request: CaptureAdminPaymentCollectionRequest,
    ) -> Result<PaymentCollectionResponse, PortError> {
        const OPERATION: &str = "capture_admin_payment_collection";
        require_admin_collection_write_admission(&context, OPERATION)?;
        let tenant_id = parse_admin_collection_tenant_id(&context, OPERATION)?;
        let collection = self
            .payment_service
            .get_collection(tenant_id, request.collection_id)
            .await
            .map_err(|error| map_payment_error(&context, OPERATION, error))?;
        let provider_id = provider_id_for_collection(&collection);
        let idempotency_key = format!("payment_collection:{}:capture", collection.id);

        match collection.status_kind() {
            PaymentCollectionStatusKind::Captured => {
                self.commit_existing_provider_operation(
                    &context,
                    OPERATION,
                    tenant_id,
                    provider_id.as_str(),
                    idempotency_key.as_str(),
                    "capture",
                )
                .await?;
                return Ok(collection);
            }
            PaymentCollectionStatusKind::Authorized => {}
            PaymentCollectionStatusKind::Pending
            | PaymentCollectionStatusKind::Cancelled
            | PaymentCollectionStatusKind::Unknown => {
                return Err(map_payment_error(
                    &context,
                    OPERATION,
                    PaymentError::InvalidTransition {
                        from: collection.status,
                        to: "captured".to_string(),
                    },
                ));
            }
        }

        let CapturePaymentInput { amount, metadata } = request.input;
        let requested_amount = amount.unwrap_or(collection.authorized_amount);
        let provider_request = PaymentProviderOperationRequest {
            tenant_id,
            collection_id: collection.id,
            amount: requested_amount,
            currency_code: collection.currency_code.clone(),
            idempotency_key: Some(idempotency_key),
            metadata: merge_provider_context(
                metadata.clone(),
                serde_json::json!({
                    "commerce_orchestration": {
                        "operation": "capture_payment_collection"
                    }
                }),
            ),
        };
        let journaled = self
            .execute_journaled_provider_operation(
                &context,
                OPERATION,
                "capture",
                provider_id.as_str(),
                provider_request,
            )
            .await?;
        let provider_result = journaled.result;

        match self
            .payment_service
            .capture_collection(
                tenant_id,
                collection.id,
                CapturePaymentInput {
                    amount: Some(provider_result.captured_amount),
                    metadata: merge_provider_context(metadata, provider_result.metadata),
                },
            )
            .await
        {
            Ok(collection) => {
                self.mark_journal_committed(
                    &context,
                    OPERATION,
                    journaled.operation_id,
                    "capture",
                )
                .await?;
                Ok(collection)
            }
            Err(error) => {
                self.mark_local_persistence_failed(
                    &context,
                    OPERATION,
                    journaled.operation_id,
                    "capture",
                    &error,
                )
                .await;
                Err(self.local_persistence_after_provider_error(
                    &context,
                    OPERATION,
                    "capture",
                    error,
                ))
            }
        }
    }

    async fn cancel_payment_collection(
        &self,
        context: PortContext,
        request: CancelAdminPaymentCollectionRequest,
    ) -> Result<PaymentCollectionResponse, PortError> {
        const OPERATION: &str = "cancel_admin_payment_collection";
        require_admin_collection_write_admission(&context, OPERATION)?;
        let tenant_id = parse_admin_collection_tenant_id(&context, OPERATION)?;
        let mut input = request.input;
        let collection = self
            .payment_service
            .get_collection(tenant_id, request.collection_id)
            .await
            .map_err(|error| map_payment_error(&context, OPERATION, error))?;
        let provider_id = provider_id_for_collection(&collection);
        let idempotency_key = format!("payment_collection:{}:cancel", collection.id);

        match collection.status_kind() {
            PaymentCollectionStatusKind::Cancelled => {
                self.commit_existing_provider_operation(
                    &context,
                    OPERATION,
                    tenant_id,
                    provider_id.as_str(),
                    idempotency_key.as_str(),
                    "cancel",
                )
                .await?;
                return Ok(collection);
            }
            PaymentCollectionStatusKind::Pending | PaymentCollectionStatusKind::Authorized => {}
            PaymentCollectionStatusKind::Captured | PaymentCollectionStatusKind::Unknown => {
                return Err(map_payment_error(
                    &context,
                    OPERATION,
                    PaymentError::InvalidTransition {
                        from: collection.status,
                        to: "cancelled".to_string(),
                    },
                ));
            }
        }

        let provider_operation_id = if should_cancel_provider(&collection) {
            let provider_request = PaymentProviderOperationRequest {
                tenant_id,
                collection_id: collection.id,
                amount: executable_payment_amount(&collection),
                currency_code: collection.currency_code.clone(),
                idempotency_key: Some(idempotency_key),
                metadata: merge_provider_context(
                    input.metadata.clone(),
                    serde_json::json!({
                        "commerce_orchestration": {
                            "operation": "cancel_payment_collection",
                            "reason": input.reason.clone(),
                        }
                    }),
                ),
            };
            let journaled = self
                .execute_journaled_provider_operation(
                    &context,
                    OPERATION,
                    "cancel",
                    provider_id.as_str(),
                    provider_request,
                )
                .await?;
            input.metadata = merge_provider_context(input.metadata, journaled.result.metadata);
            Some(journaled.operation_id)
        } else {
            None
        };

        match self
            .payment_service
            .cancel_collection(tenant_id, collection.id, input)
            .await
        {
            Ok(collection) => {
                if let Some(operation_id) = provider_operation_id {
                    self.mark_journal_committed(
                        &context,
                        OPERATION,
                        operation_id,
                        "cancel",
                    )
                    .await?;
                }
                Ok(collection)
            }
            Err(error) => {
                if let Some(operation_id) = provider_operation_id {
                    self.mark_local_persistence_failed(
                        &context,
                        OPERATION,
                        operation_id,
                        "cancel",
                        &error,
                    )
                    .await;
                    Err(self.local_persistence_after_provider_error(
                        &context,
                        OPERATION,
                        "cancel",
                        error,
                    ))
                } else {
                    Err(map_payment_error(&context, OPERATION, error))
                }
            }
        }
    }
}

struct JournaledProviderResult {
    operation_id: Uuid,
    result: PaymentProviderOperationResult,
}

impl InProcessPaymentAdminCollectionCommandPort {
    async fn execute_journaled_provider_operation(
        &self,
        context: &PortContext,
        owner_operation: &'static str,
        provider_operation: &'static str,
        provider_id: &str,
        request: PaymentProviderOperationRequest,
    ) -> Result<JournaledProviderResult, PortError> {
        let request = self
            .enrich_provider_request(
                context,
                owner_operation,
                provider_operation,
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
                refund_id: None,
                operation: provider_operation.to_string(),
                provider_id: provider_id.to_string(),
                idempotency_key,
                request_payload,
            })
            .await
            .map_err(|error| map_payment_error(context, owner_operation, error))?;

        if let Some(result) = persisted_provider_result(&journal_operation)
            .map_err(|error| map_payment_error(context, owner_operation, error))?
        {
            return Ok(JournaledProviderResult {
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
                return Ok(JournaledProviderResult {
                    operation_id: current.id,
                    result,
                });
            }
            return Err(PortError::validation(
                "payment.provider_operation_in_progress",
                "payment provider operation is already in progress",
            ));
        }

        let provider_result = match provider_operation {
            "authorize" => self.provider_registry.execute_authorize(provider_id, request).await,
            "capture" => self.provider_registry.execute_capture(provider_id, request).await,
            "cancel" => self.provider_registry.execute_cancel(provider_id, request).await,
            _ => {
                return Err(PortError::validation(
                    "payment.provider_operation_invalid",
                    "payment provider operation is not supported",
                ));
            }
        };
        let provider_result = match provider_result {
            Ok(result) => result,
            Err(error) => {
                let checkpoint = if error.requires_provider_reconciliation() {
                    self.operation_journal
                        .mark_reconciliation_required(
                            journal_operation.id,
                            "payment.provider_outcome_requires_reconciliation",
                        )
                        .await
                } else {
                    self.operation_journal
                        .mark_provider_error(journal_operation.id, "payment.provider_operation_failed")
                        .await
                };
                if checkpoint.is_err() {
                    return Err(map_payment_error(
                        context,
                        owner_operation,
                        PaymentError::provider_outcome_unknown(provider_id, provider_operation),
                    ));
                }
                return Err(map_payment_error(context, owner_operation, error));
            }
        };

        let result_payload = match serde_json::to_value(&provider_result) {
            Ok(payload) => payload,
            Err(_) => {
                let _ = self
                    .operation_journal
                    .mark_reconciliation_required(
                        journal_operation.id,
                        "payment.provider_result_serialization_failed",
                    )
                    .await;
                return Err(map_payment_error(
                    context,
                    owner_operation,
                    PaymentError::provider_outcome_unknown(provider_id, provider_operation),
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
                    "payment.provider_success_checkpoint_failed",
                )
                .await;
            return Err(map_payment_error(
                context,
                owner_operation,
                PaymentError::provider_outcome_unknown(provider_id, provider_operation),
            ));
        }

        Ok(JournaledProviderResult {
            operation_id: journal_operation.id,
            result: provider_result,
        })
    }

    async fn enrich_provider_request(
        &self,
        context: &PortContext,
        owner_operation: &'static str,
        provider_operation: &str,
        provider_id: &str,
        mut request: PaymentProviderOperationRequest,
    ) -> Result<PaymentProviderOperationRequest, PortError> {
        if provider_id == MANUAL_PAYMENT_PROVIDER_ID || provider_operation == "authorize" {
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
                        "provider operation requires a completed authorize operation".to_string(),
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
                    "provider operation requires a completed authorize operation".to_string(),
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

    async fn commit_existing_provider_operation(
        &self,
        context: &PortContext,
        owner_operation: &'static str,
        tenant_id: Uuid,
        provider_id: &str,
        idempotency_key: &str,
        provider_operation: &'static str,
    ) -> Result<(), PortError> {
        if let Some(existing) = self
            .operation_journal
            .find_by_key(tenant_id, provider_id, idempotency_key)
            .await
            .map_err(|error| map_payment_error(context, owner_operation, error))?
            && matches!(
                existing.status.as_str(),
                PROVIDER_OPERATION_SUCCEEDED | PROVIDER_OPERATION_RECONCILIATION_REQUIRED
            )
        {
            self.mark_journal_committed(
                context,
                owner_operation,
                existing.id,
                provider_operation,
            )
            .await?;
        }
        Ok(())
    }

    async fn mark_journal_committed(
        &self,
        context: &PortContext,
        owner_operation: &'static str,
        operation_id: Uuid,
        provider_operation: &'static str,
    ) -> Result<(), PortError> {
        if self.operation_journal.mark_committed(operation_id).await.is_err() {
            let _ = self
                .operation_journal
                .mark_reconciliation_required(
                    operation_id,
                    format!("payment.local_{provider_operation}_commit_checkpoint_failed"),
                )
                .await;
            return Err(map_payment_error(
                context,
                owner_operation,
                PaymentError::provider_outcome_unknown(UNKNOWN_PROVIDER_ID, provider_operation),
            ));
        }
        Ok(())
    }

    async fn mark_local_persistence_failed(
        &self,
        context: &PortContext,
        owner_operation: &'static str,
        operation_id: Uuid,
        provider_operation: &'static str,
        error: &PaymentError,
    ) {
        let _ = self
            .operation_journal
            .mark_reconciliation_required(
                operation_id,
                format!("payment.local_{provider_operation}_persistence_failed"),
            )
            .await;
        log_local_persistence_failure(context, owner_operation, provider_operation, error);
    }

    fn local_persistence_after_provider_error(
        &self,
        context: &PortContext,
        owner_operation: &'static str,
        provider_operation: &'static str,
        error: PaymentError,
    ) -> PortError {
        log_local_persistence_failure(context, owner_operation, provider_operation, &error);
        map_payment_error(
            context,
            owner_operation,
            PaymentError::provider_outcome_unknown(UNKNOWN_PROVIDER_ID, provider_operation),
        )
    }
}

fn require_admin_collection_write_admission(
    context: &PortContext,
    operation: &'static str,
) -> Result<(), PortError> {
    context.require_policy(PortCallPolicy::write()).inspect_err(|error| {
        log_port_error(context, operation, "policy", error);
    })?;
    context.require_write_semantics().inspect_err(|error| {
        log_port_error(context, operation, "write_semantics", error);
    })
}

fn parse_admin_collection_tenant_id(
    context: &PortContext,
    operation: &'static str,
) -> Result<Uuid, PortError> {
    Uuid::parse_str(&context.tenant_id).map_err(|_| {
        let error = PortError::validation(
            "payment.tenant_id_invalid",
            "payment command tenant context is invalid",
        );
        log_port_error(context, operation, "tenant_id", &error);
        error
    })
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
        PaymentError::ProviderOutcomeUnknown { .. } => PortError::new(
            PortErrorKind::Conflict,
            "payment.provider_outcome_unknown",
            "payment provider outcome requires reconciliation",
            false,
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
        public_code = %mapped.code,
        retryable = mapped.retryable,
        boundary = ADMIN_COLLECTION_COMMAND_BOUNDARY,
        "payment admin collection command returned a bounded owner error"
    );
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
        boundary = ADMIN_COLLECTION_COMMAND_BOUNDARY,
        "payment admin collection command admission failed"
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
    serde_json::from_value(value)
        .map(Some)
        .map_err(|_| {
            PaymentError::provider_outcome_unknown(
                journal_operation.provider_id.as_str(),
                journal_operation.operation.as_str(),
            )
        })
}

fn insert_metadata_string(metadata: &mut Value, key: &str, value: String) -> Result<(), PaymentError> {
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

fn log_local_persistence_failure(
    context: &PortContext,
    owner_operation: &'static str,
    provider_operation: &'static str,
    error: &PaymentError,
) {
    tracing::error!(
        owner = "rustok_payment",
        operation = owner_operation,
        provider_operation,
        correlation_id = %context.correlation_id,
        tenant_id_length = context.tenant_id.chars().count(),
        actor_id_length = context.actor.id.chars().count(),
        channel_present = context.channel.is_some(),
        locale_length = context.locale.chars().count(),
        deadline_ms = ?context.deadline_ms,
        error_variant = payment_error_variant(error),
        boundary = ADMIN_COLLECTION_COMMAND_BOUNDARY,
        "payment provider operation succeeded but local persistence did not complete"
    );
}
