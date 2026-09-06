use std::sync::Arc;

use async_trait::async_trait;
use rustok_api::{PortCallPolicy, PortContext, PortError};
use sea_orm::DatabaseConnection;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use uuid::Uuid;
use validator::Validate;

use crate::dto::{
    CancelFulfillmentInput, DeliverFulfillmentInput, FulfillmentResponse, ReopenFulfillmentInput,
    ReshipFulfillmentInput, ShipFulfillmentInput,
};
use crate::entities::provider_operation;
use crate::error::FulfillmentError;
use crate::providers::{
    FulfillmentProviderOperationRequest, FulfillmentProviderOperationResult,
    FulfillmentProviderRegistry, MANUAL_FULFILLMENT_PROVIDER_ID,
};
use crate::services::{
    BeginProviderOperation, FulfillmentProviderOperationJournal, FulfillmentService,
    PROVIDER_OPERATION_COMMITTED, PROVIDER_OPERATION_EXECUTING,
    PROVIDER_OPERATION_RECONCILIATION_REQUIRED, PROVIDER_OPERATION_SUCCEEDED,
};

const ADMIN_COMMAND_BOUNDARY: &str = "fulfillment_admin_command_port";

#[async_trait]
pub trait FulfillmentAdminCommandPort: Send + Sync {
    async fn ship_fulfillment(
        &self,
        context: PortContext,
        request: ShipAdminFulfillmentRequest,
    ) -> Result<FulfillmentResponse, PortError>;

    async fn deliver_fulfillment(
        &self,
        context: PortContext,
        request: DeliverAdminFulfillmentRequest,
    ) -> Result<FulfillmentResponse, PortError>;

    async fn reopen_fulfillment(
        &self,
        context: PortContext,
        request: ReopenAdminFulfillmentRequest,
    ) -> Result<FulfillmentResponse, PortError>;

    async fn reship_fulfillment(
        &self,
        context: PortContext,
        request: ReshipAdminFulfillmentRequest,
    ) -> Result<FulfillmentResponse, PortError>;

    async fn cancel_fulfillment(
        &self,
        context: PortContext,
        request: CancelAdminFulfillmentRequest,
    ) -> Result<FulfillmentResponse, PortError>;
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ShipAdminFulfillmentRequest {
    pub fulfillment_id: Uuid,
    pub input: ShipFulfillmentInput,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DeliverAdminFulfillmentRequest {
    pub fulfillment_id: Uuid,
    pub input: DeliverFulfillmentInput,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReopenAdminFulfillmentRequest {
    pub fulfillment_id: Uuid,
    pub input: ReopenFulfillmentInput,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReshipAdminFulfillmentRequest {
    pub fulfillment_id: Uuid,
    pub input: ReshipFulfillmentInput,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CancelAdminFulfillmentRequest {
    pub fulfillment_id: Uuid,
    pub input: CancelFulfillmentInput,
}

pub struct InProcessFulfillmentAdminCommandPort {
    service: FulfillmentService,
    operation_journal: FulfillmentProviderOperationJournal,
    provider_registry: FulfillmentProviderRegistry,
}

impl InProcessFulfillmentAdminCommandPort {
    pub fn new(db: DatabaseConnection, provider_registry: FulfillmentProviderRegistry) -> Self {
        Self {
            service: FulfillmentService::new(db.clone()),
            operation_journal: FulfillmentProviderOperationJournal::new(db),
            provider_registry,
        }
    }
}

pub fn in_process_fulfillment_admin_command_port(
    db: DatabaseConnection,
    provider_registry: FulfillmentProviderRegistry,
) -> Arc<dyn FulfillmentAdminCommandPort> {
    Arc::new(InProcessFulfillmentAdminCommandPort::new(
        db,
        provider_registry,
    ))
}

#[derive(Clone)]
pub struct FulfillmentAdminCommandRuntime {
    command_port: Arc<dyn FulfillmentAdminCommandPort>,
}

impl FulfillmentAdminCommandRuntime {
    pub fn new(command_port: Arc<dyn FulfillmentAdminCommandPort>) -> Self {
        Self { command_port }
    }

    pub fn in_process(
        db: DatabaseConnection,
        provider_registry: FulfillmentProviderRegistry,
    ) -> Self {
        Self::new(in_process_fulfillment_admin_command_port(
            db,
            provider_registry,
        ))
    }

    pub fn command_port(&self) -> Arc<dyn FulfillmentAdminCommandPort> {
        self.command_port.clone()
    }
}

#[async_trait]
impl FulfillmentAdminCommandPort for InProcessFulfillmentAdminCommandPort {
    async fn ship_fulfillment(
        &self,
        context: PortContext,
        request: ShipAdminFulfillmentRequest,
    ) -> Result<FulfillmentResponse, PortError> {
        const OPERATION: &str = "ship_admin_fulfillment";
        require_write_admission(&context, OPERATION)?;
        let tenant_id = parse_tenant_id(&context, OPERATION)?;
        request.input.validate().map_err(|_| {
            PortError::validation(
                "fulfillment.validation",
                "fulfillment shipment request is invalid",
            )
        })?;

        let current = self
            .service
            .get_fulfillment(tenant_id, request.fulfillment_id)
            .await
            .map_err(|error| map_fulfillment_error(&context, OPERATION, error))?;
        if !matches!(current.status.as_str(), "pending" | "shipped") {
            return Err(map_fulfillment_error(
                &context,
                OPERATION,
                FulfillmentError::InvalidTransition {
                    from: current.status,
                    to: "shipped".to_string(),
                },
            ));
        }

        let provider_id = self
            .provider_id_for_fulfillment(&context, OPERATION, tenant_id, &current)
            .await?;
        let ShipFulfillmentInput {
            carrier,
            tracking_number,
            items,
            metadata,
        } = request.input;
        let provider_request = operation_request(
            tenant_id,
            request.fulfillment_id,
            "ship",
            provider_id.as_str(),
            merge_metadata(
                metadata.clone(),
                serde_json::json!({
                    "commerce_orchestration": {
                        "operation": "ship",
                        "carrier": carrier,
                        "tracking_number": tracking_number,
                        "items": items
                    }
                }),
            ),
        )?;
        let journaled = self
            .execute_provider_operation(
                &context,
                OPERATION,
                provider_id.as_str(),
                "ship",
                provider_request,
            )
            .await?;
        if journaled.committed {
            return Ok(current);
        }

        let updated = self
            .service
            .ship_fulfillment(
                tenant_id,
                request.fulfillment_id,
                ShipFulfillmentInput {
                    carrier,
                    tracking_number: journaled
                        .result
                        .tracking_number
                        .clone()
                        .unwrap_or(tracking_number),
                    items,
                    metadata: local_commit_metadata(
                        metadata,
                        journaled.result.metadata.clone(),
                        journaled.operation_id,
                        "ship",
                    ),
                },
            )
            .await;
        match updated {
            Ok(updated) => {
                self.ensure_committed(&context, OPERATION, journaled.operation_id, "ship")
                    .await?;
                Ok(updated)
            }
            Err(error) => {
                self.mark_local_persistence_reconciliation(
                    &context,
                    OPERATION,
                    journaled.operation_id,
                    "ship",
                )
                .await;
                Err(reconciliation_error(&context, OPERATION, "ship", &error))
            }
        }
    }

    async fn deliver_fulfillment(
        &self,
        context: PortContext,
        request: DeliverAdminFulfillmentRequest,
    ) -> Result<FulfillmentResponse, PortError> {
        const OPERATION: &str = "deliver_admin_fulfillment";
        require_write_admission(&context, OPERATION)?;
        let tenant_id = parse_tenant_id(&context, OPERATION)?;
        self.service
            .deliver_fulfillment(tenant_id, request.fulfillment_id, request.input)
            .await
            .map_err(|error| map_fulfillment_error(&context, OPERATION, error))
    }

    async fn reopen_fulfillment(
        &self,
        context: PortContext,
        request: ReopenAdminFulfillmentRequest,
    ) -> Result<FulfillmentResponse, PortError> {
        const OPERATION: &str = "reopen_admin_fulfillment";
        require_write_admission(&context, OPERATION)?;
        let tenant_id = parse_tenant_id(&context, OPERATION)?;
        self.service
            .reopen_fulfillment(tenant_id, request.fulfillment_id, request.input)
            .await
            .map_err(|error| map_fulfillment_error(&context, OPERATION, error))
    }

    async fn reship_fulfillment(
        &self,
        context: PortContext,
        request: ReshipAdminFulfillmentRequest,
    ) -> Result<FulfillmentResponse, PortError> {
        const OPERATION: &str = "reship_admin_fulfillment";
        require_write_admission(&context, OPERATION)?;
        let tenant_id = parse_tenant_id(&context, OPERATION)?;
        request.input.validate().map_err(|_| {
            PortError::validation(
                "fulfillment.validation",
                "fulfillment reship request is invalid",
            )
        })?;

        let current = self
            .service
            .get_fulfillment(tenant_id, request.fulfillment_id)
            .await
            .map_err(|error| map_fulfillment_error(&context, OPERATION, error))?;
        if current.status == "shipped"
            && current
                .metadata
                .get("provider_operation")
                .and_then(|value| value.get("operation"))
                .and_then(Value::as_str)
                == Some("reship")
        {
            return Ok(current);
        }
        if current.status != "delivered" {
            return Err(map_fulfillment_error(
                &context,
                OPERATION,
                FulfillmentError::InvalidTransition {
                    from: current.status,
                    to: "shipped".to_string(),
                },
            ));
        }

        let provider_id = self
            .provider_id_for_fulfillment(&context, OPERATION, tenant_id, &current)
            .await?;
        let ReshipFulfillmentInput {
            carrier,
            tracking_number,
            items,
            metadata,
        } = request.input;
        let provider_request = operation_request(
            tenant_id,
            request.fulfillment_id,
            "reship",
            provider_id.as_str(),
            merge_metadata(
                metadata.clone(),
                serde_json::json!({
                    "commerce_orchestration": {
                        "operation": "reship",
                        "carrier": carrier,
                        "tracking_number": tracking_number,
                        "items": items
                    }
                }),
            ),
        )?;
        let journaled = self
            .execute_provider_operation(
                &context,
                OPERATION,
                provider_id.as_str(),
                "reship",
                provider_request,
            )
            .await?;
        if journaled.committed {
            return Ok(current);
        }

        let updated = self
            .service
            .reship_fulfillment(
                tenant_id,
                request.fulfillment_id,
                ReshipFulfillmentInput {
                    carrier,
                    tracking_number: journaled
                        .result
                        .tracking_number
                        .clone()
                        .unwrap_or(tracking_number),
                    items,
                    metadata: local_commit_metadata(
                        metadata,
                        journaled.result.metadata.clone(),
                        journaled.operation_id,
                        "reship",
                    ),
                },
            )
            .await;
        match updated {
            Ok(updated) => {
                self.ensure_committed(&context, OPERATION, journaled.operation_id, "reship")
                    .await?;
                Ok(updated)
            }
            Err(error) => {
                self.mark_local_persistence_reconciliation(
                    &context,
                    OPERATION,
                    journaled.operation_id,
                    "reship",
                )
                .await;
                Err(reconciliation_error(&context, OPERATION, "reship", &error))
            }
        }
    }

    async fn cancel_fulfillment(
        &self,
        context: PortContext,
        request: CancelAdminFulfillmentRequest,
    ) -> Result<FulfillmentResponse, PortError> {
        const OPERATION: &str = "cancel_admin_fulfillment";
        require_write_admission(&context, OPERATION)?;
        let tenant_id = parse_tenant_id(&context, OPERATION)?;
        let current = self
            .service
            .get_fulfillment(tenant_id, request.fulfillment_id)
            .await
            .map_err(|error| map_fulfillment_error(&context, OPERATION, error))?;
        if current.status == "cancelled" {
            return Ok(current);
        }
        if current.status == "delivered" {
            return Err(map_fulfillment_error(
                &context,
                OPERATION,
                FulfillmentError::InvalidTransition {
                    from: current.status,
                    to: "cancelled".to_string(),
                },
            ));
        }

        let provider_id = self
            .provider_id_for_fulfillment(&context, OPERATION, tenant_id, &current)
            .await?;
        let CancelFulfillmentInput { reason, metadata } = request.input;
        let provider_request = operation_request(
            tenant_id,
            request.fulfillment_id,
            "cancel",
            provider_id.as_str(),
            merge_metadata(
                metadata.clone(),
                serde_json::json!({
                    "commerce_orchestration": {
                        "operation": "cancel",
                        "reason": reason
                    }
                }),
            ),
        )?;
        let journaled = self
            .execute_provider_operation(
                &context,
                OPERATION,
                provider_id.as_str(),
                "cancel",
                provider_request,
            )
            .await?;
        if journaled.committed {
            return Ok(current);
        }

        let updated = self
            .service
            .cancel_fulfillment(
                tenant_id,
                request.fulfillment_id,
                CancelFulfillmentInput {
                    reason,
                    metadata: local_commit_metadata(
                        metadata,
                        journaled.result.metadata.clone(),
                        journaled.operation_id,
                        "cancel",
                    ),
                },
            )
            .await;
        match updated {
            Ok(updated) => {
                self.ensure_committed(&context, OPERATION, journaled.operation_id, "cancel")
                    .await?;
                Ok(updated)
            }
            Err(error) => {
                self.mark_local_persistence_reconciliation(
                    &context,
                    OPERATION,
                    journaled.operation_id,
                    "cancel",
                )
                .await;
                Err(reconciliation_error(&context, OPERATION, "cancel", &error))
            }
        }
    }
}

struct JournaledProviderResult {
    operation_id: Uuid,
    result: FulfillmentProviderOperationResult,
    committed: bool,
}

impl InProcessFulfillmentAdminCommandPort {
    async fn execute_provider_operation(
        &self,
        context: &PortContext,
        owner_operation: &'static str,
        provider_id: &str,
        operation: &'static str,
        request: FulfillmentProviderOperationRequest,
    ) -> Result<JournaledProviderResult, PortError> {
        let idempotency_key = request
            .idempotency_key
            .as_deref()
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .ok_or_else(|| {
                PortError::validation(
                    "fulfillment.provider_idempotency_key_missing",
                    "fulfillment provider operation requires idempotency identity",
                )
            })?
            .to_string();
        let request_payload = serde_json::to_value(&request).map_err(|_| {
            PortError::validation(
                "fulfillment.provider_request_invalid",
                "fulfillment provider request could not be normalized",
            )
        })?;
        let journal_operation = self
            .operation_journal
            .begin(BeginProviderOperation {
                tenant_id: request.tenant_id,
                fulfillment_id: request.fulfillment_id,
                operation: operation.to_string(),
                provider_id: provider_id.to_string(),
                idempotency_key,
                request_payload,
            })
            .await
            .map_err(|error| map_fulfillment_error(context, owner_operation, error))?;

        if matches!(
            journal_operation.status.as_str(),
            PROVIDER_OPERATION_COMMITTED
                | PROVIDER_OPERATION_SUCCEEDED
                | PROVIDER_OPERATION_RECONCILIATION_REQUIRED
        ) {
            let result = deserialize_provider_result(&journal_operation)
                .map_err(|error| map_fulfillment_error(context, owner_operation, error))?;
            if journal_operation.status == PROVIDER_OPERATION_RECONCILIATION_REQUIRED {
                return Err(PortError::validation(
                    "fulfillment.provider_reconciliation_pending",
                    "fulfillment provider operation requires reconciliation",
                ));
            }
            return Ok(JournaledProviderResult {
                operation_id: journal_operation.id,
                result,
                committed: journal_operation.status == PROVIDER_OPERATION_COMMITTED,
            });
        }
        if journal_operation.status == PROVIDER_OPERATION_EXECUTING {
            return Err(PortError::validation(
                "fulfillment.provider_operation_in_progress",
                "fulfillment provider operation is already executing",
            ));
        }

        if self
            .operation_journal
            .claim_execution(journal_operation.id)
            .await
            .map_err(|error| map_fulfillment_error(context, owner_operation, error))?
            .is_none()
        {
            let current = self
                .operation_journal
                .get(journal_operation.id)
                .await
                .map_err(|error| map_fulfillment_error(context, owner_operation, error))?;
            if matches!(
                current.status.as_str(),
                PROVIDER_OPERATION_COMMITTED
                    | PROVIDER_OPERATION_SUCCEEDED
                    | PROVIDER_OPERATION_RECONCILIATION_REQUIRED
            ) {
                let result = deserialize_provider_result(&current)
                    .map_err(|error| map_fulfillment_error(context, owner_operation, error))?;
                if current.status == PROVIDER_OPERATION_RECONCILIATION_REQUIRED {
                    return Err(PortError::validation(
                        "fulfillment.provider_reconciliation_pending",
                        "fulfillment provider operation requires reconciliation",
                    ));
                }
                return Ok(JournaledProviderResult {
                    operation_id: current.id,
                    result,
                    committed: current.status == PROVIDER_OPERATION_COMMITTED,
                });
            }
            return Err(PortError::validation(
                "fulfillment.provider_operation_in_progress",
                "fulfillment provider operation is already executing",
            ));
        }

        let provider_result = match operation {
            "ship" | "reship" => {
                self.provider_registry
                    .execute_ship(provider_id, request)
                    .await
            }
            "cancel" => {
                self.provider_registry
                    .execute_cancel(provider_id, request)
                    .await
            }
            _ => Err(FulfillmentError::Validation(
                "unsupported fulfillment provider operation".to_string(),
            )),
        };
        let provider_result = match provider_result {
            Ok(result) => result,
            Err(error) => {
                if self
                    .operation_journal
                    .mark_provider_error(
                        journal_operation.id,
                        "fulfillment.provider_operation_failed",
                    )
                    .await
                    .is_err()
                {
                    return Err(PortError::validation(
                        "fulfillment.provider_journal_failed",
                        "fulfillment provider operation failed and could not be checkpointed",
                    ));
                }
                return Err(map_fulfillment_error(context, owner_operation, error));
            }
        };

        let result_payload = serde_json::to_value(&provider_result).map_err(|_| {
            PortError::validation(
                "fulfillment.provider_result_invalid",
                "fulfillment provider result could not be normalized",
            )
        })?;
        self.operation_journal
            .mark_provider_succeeded(
                journal_operation.id,
                provider_result.external_reference.clone(),
                result_payload,
            )
            .await
            .map_err(|error| map_fulfillment_error(context, owner_operation, error))?;

        Ok(JournaledProviderResult {
            operation_id: journal_operation.id,
            result: provider_result,
            committed: false,
        })
    }

    async fn provider_id_for_fulfillment(
        &self,
        context: &PortContext,
        owner_operation: &'static str,
        tenant_id: Uuid,
        fulfillment: &FulfillmentResponse,
    ) -> Result<String, PortError> {
        match fulfillment.shipping_option_id {
            Some(shipping_option_id) => self
                .service
                .get_shipping_option(tenant_id, shipping_option_id, None, None)
                .await
                .map(|option| option.provider_id)
                .map_err(|error| map_fulfillment_error(context, owner_operation, error)),
            None => Ok(MANUAL_FULFILLMENT_PROVIDER_ID.to_string()),
        }
    }

    async fn mark_local_persistence_reconciliation(
        &self,
        context: &PortContext,
        owner_operation: &'static str,
        operation_id: Uuid,
        operation: &'static str,
    ) {
        let checkpoint = self
            .operation_journal
            .mark_reconciliation_required(
                operation_id,
                format!("fulfillment.local_{operation}_persistence_failed"),
            )
            .await;
        tracing::error!(
            owner = "rustok_fulfillment",
            operation = owner_operation,
            provider_operation = operation,
            correlation_id = %context.correlation_id,
            operation_id_non_nil = !operation_id.is_nil(),
            checkpoint_failed = checkpoint.is_err(),
            boundary = ADMIN_COMMAND_BOUNDARY,
            "fulfillment provider operation succeeded but local persistence did not complete"
        );
    }

    async fn ensure_committed(
        &self,
        context: &PortContext,
        owner_operation: &'static str,
        operation_id: Uuid,
        operation: &'static str,
    ) -> Result<(), PortError> {
        let current = self
            .operation_journal
            .get(operation_id)
            .await
            .map_err(|error| map_fulfillment_error(context, owner_operation, error))?;
        if current.status == PROVIDER_OPERATION_COMMITTED {
            return Ok(());
        }
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
                    format!("fulfillment.local_{operation}_journal_commit_failed"),
                )
                .await;
            return Err(PortError::validation(
                "fulfillment.journal_commit_failed",
                "fulfillment operation completed but its journal could not be committed",
            ));
        }
        Ok(())
    }
}

fn require_write_admission(
    context: &PortContext,
    operation: &'static str,
) -> Result<(), PortError> {
    context
        .require_policy(PortCallPolicy::write())
        .inspect_err(|error| {
            log_port_error(context, operation, "policy", error);
        })?;
    context.require_write_semantics().inspect_err(|error| {
        log_port_error(context, operation, "write_semantics", error);
    })
}

fn parse_tenant_id(context: &PortContext, operation: &'static str) -> Result<Uuid, PortError> {
    Uuid::parse_str(&context.tenant_id).map_err(|_| {
        let error = PortError::validation(
            "fulfillment.tenant_id_invalid",
            "fulfillment command tenant context is invalid",
        );
        log_port_error(context, operation, "tenant_id", &error);
        error
    })
}

fn map_fulfillment_error(
    context: &PortContext,
    operation: &'static str,
    error: FulfillmentError,
) -> PortError {
    let error_variant = fulfillment_error_variant(&error);
    let mapped = match error {
        FulfillmentError::Validation(_) => {
            PortError::validation("fulfillment.validation", "fulfillment request is invalid")
        }
        FulfillmentError::ShippingOptionNotFound(_) => PortError::not_found(
            "fulfillment.shipping_option_not_found",
            "shipping option was not found",
        ),
        FulfillmentError::FulfillmentNotFound(_) => {
            PortError::not_found("fulfillment.not_found", "fulfillment was not found")
        }
        FulfillmentError::InvalidTransition { .. } => PortError::conflict(
            "fulfillment.invalid_transition",
            "fulfillment lifecycle conflicts with the requested operation",
        ),
        FulfillmentError::Database(_) => PortError::unavailable(
            "fulfillment.database_unavailable",
            "fulfillment storage is temporarily unavailable",
        ),
    };
    tracing::warn!(
        owner = "rustok_fulfillment",
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
        boundary = ADMIN_COMMAND_BOUNDARY,
        "fulfillment admin command returned a bounded owner error"
    );
    mapped
}

fn reconciliation_error(
    context: &PortContext,
    owner_operation: &'static str,
    provider_operation: &'static str,
    error: &FulfillmentError,
) -> PortError {
    tracing::error!(
        owner = "rustok_fulfillment",
        operation = owner_operation,
        provider_operation,
        correlation_id = %context.correlation_id,
        error_variant = fulfillment_error_variant(error),
        boundary = ADMIN_COMMAND_BOUNDARY,
        "fulfillment provider operation requires reconciliation after local persistence failure"
    );
    PortError::conflict(
        "fulfillment.reconciliation_required",
        "fulfillment operation requires reconciliation",
    )
}

fn fulfillment_error_variant(error: &FulfillmentError) -> &'static str {
    match error {
        FulfillmentError::Validation(_) => "validation",
        FulfillmentError::ShippingOptionNotFound(_) => "shipping_option_not_found",
        FulfillmentError::FulfillmentNotFound(_) => "fulfillment_not_found",
        FulfillmentError::InvalidTransition { .. } => "invalid_transition",
        FulfillmentError::Database(_) => "database",
    }
}

fn log_port_error(
    context: &PortContext,
    operation: &'static str,
    admission: &'static str,
    error: &PortError,
) {
    tracing::warn!(
        owner = "rustok_fulfillment",
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
        boundary = ADMIN_COMMAND_BOUNDARY,
        "fulfillment admin command admission failed"
    );
}

fn operation_request(
    tenant_id: Uuid,
    fulfillment_id: Uuid,
    operation: &'static str,
    provider_id: &str,
    metadata: Value,
) -> Result<FulfillmentProviderOperationRequest, PortError> {
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
) -> Result<String, PortError> {
    let bytes = serde_json::to_vec(payload).map_err(|_| {
        PortError::validation(
            "fulfillment.provider_identity_invalid",
            "fulfillment provider identity could not be normalized",
        )
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
    operation: &provider_operation::Model,
) -> Result<FulfillmentProviderOperationResult, FulfillmentError> {
    let value = operation.provider_result.clone().ok_or_else(|| {
        FulfillmentError::Validation(
            "fulfillment provider operation has no persisted provider result".to_string(),
        )
    })?;
    serde_json::from_value(value).map_err(|_| {
        FulfillmentError::Validation(
            "fulfillment provider operation has an invalid persisted provider result".to_string(),
        )
    })
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
