use std::sync::Arc;
use std::time::Duration;

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
use crate::status::FulfillmentStatusKind;

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
        if !matches!(
            current.status_kind(),
            FulfillmentStatusKind::Pending | FulfillmentStatusKind::Shipped
        ) {
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
            &context,
            tenant_id,
            request.fulfillment_id,
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
            )?,
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
            .ship_fulfillment_with_provider_result(
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
                    metadata,
                },
                journaled.result.metadata.clone(),
                journaled.operation_id,
            )
            .await;
        match updated {
            Ok(updated) => {
                self.ensure_committed(
                    &context,
                    tenant_id,
                    OPERATION,
                    journaled.operation_id,
                    "ship",
                )
                .await?;
                Ok(updated)
            }
            Err(error) => {
                self.mark_local_persistence_reconciliation(
                    &context,
                    tenant_id,
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
        request.input.validate().map_err(|_| {
            PortError::validation(
                "fulfillment.validation",
                "fulfillment delivery request is invalid",
            )
        })?;
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
        if current.status_kind() == FulfillmentStatusKind::Shipped
            && current
                .metadata
                .get("provider_operation")
                .and_then(|value| value.get("operation"))
                .and_then(Value::as_str)
                == Some("reship")
        {
            return Ok(current);
        }
        if current.status_kind() != FulfillmentStatusKind::Delivered {
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
            &context,
            tenant_id,
            request.fulfillment_id,
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
            )?,
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
            .reship_fulfillment_with_provider_result(
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
                    metadata,
                },
                journaled.result.metadata.clone(),
                journaled.operation_id,
            )
            .await;
        match updated {
            Ok(updated) => {
                self.ensure_committed(
                    &context,
                    tenant_id,
                    OPERATION,
                    journaled.operation_id,
                    "reship",
                )
                .await?;
                Ok(updated)
            }
            Err(error) => {
                self.mark_local_persistence_reconciliation(
                    &context,
                    tenant_id,
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
        request.input.validate().map_err(|_| {
            PortError::validation(
                "fulfillment.validation",
                "fulfillment cancellation request is invalid",
            )
        })?;
        let current = self
            .service
            .get_fulfillment(tenant_id, request.fulfillment_id)
            .await
            .map_err(|error| map_fulfillment_error(&context, OPERATION, error))?;
        if current.status_kind() == FulfillmentStatusKind::Cancelled {
            return Ok(current);
        }
        if current.status_kind() == FulfillmentStatusKind::Delivered {
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
            &context,
            tenant_id,
            request.fulfillment_id,
            merge_metadata(
                metadata.clone(),
                serde_json::json!({
                    "commerce_orchestration": {
                        "operation": "cancel",
                        "reason": reason
                    }
                }),
            )?,
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
            .cancel_fulfillment_with_provider_result(
                tenant_id,
                request.fulfillment_id,
                CancelFulfillmentInput { reason, metadata },
                journaled.result.metadata.clone(),
                journaled.operation_id,
            )
            .await;
        match updated {
            Ok(updated) => {
                self.ensure_committed(
                    &context,
                    tenant_id,
                    OPERATION,
                    journaled.operation_id,
                    "cancel",
                )
                .await?;
                Ok(updated)
            }
            Err(error) => {
                self.mark_local_persistence_reconciliation(
                    &context,
                    tenant_id,
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
            if journal_operation.status == PROVIDER_OPERATION_RECONCILIATION_REQUIRED {
                return Err(PortError::conflict(
                    "fulfillment.reconciliation_required",
                    "fulfillment provider operation requires reconciliation",
                ));
            }
            let result = deserialize_provider_result(&journal_operation)?;
            return Ok(JournaledProviderResult {
                operation_id: journal_operation.id,
                result,
                committed: journal_operation.status == PROVIDER_OPERATION_COMMITTED,
            });
        }
        if journal_operation.status == PROVIDER_OPERATION_EXECUTING {
            return Err(PortError::conflict(
                "fulfillment.provider_operation_in_progress",
                "fulfillment provider operation is already executing",
            ));
        }

        if self
            .operation_journal
            .claim_execution(request.tenant_id, journal_operation.id)
            .await
            .map_err(|error| map_fulfillment_error(context, owner_operation, error))?
            .is_none()
        {
            let current = self
                .operation_journal
                .get(request.tenant_id, journal_operation.id)
                .await
                .map_err(|error| map_fulfillment_error(context, owner_operation, error))?;
            if matches!(
                current.status.as_str(),
                PROVIDER_OPERATION_COMMITTED
                    | PROVIDER_OPERATION_SUCCEEDED
                    | PROVIDER_OPERATION_RECONCILIATION_REQUIRED
            ) {
                if current.status == PROVIDER_OPERATION_RECONCILIATION_REQUIRED {
                    return Err(PortError::conflict(
                        "fulfillment.reconciliation_required",
                        "fulfillment provider operation requires reconciliation",
                    ));
                }
                let result = deserialize_provider_result(&current)?;
                return Ok(JournaledProviderResult {
                    operation_id: current.id,
                    result,
                    committed: current.status == PROVIDER_OPERATION_COMMITTED,
                });
            }
            return Err(PortError::conflict(
                "fulfillment.provider_operation_in_progress",
                "fulfillment provider operation is already executing",
            ));
        }

        let tenant_id = request.tenant_id;
        let deadline = Duration::from_millis(context.deadline_ms.unwrap_or_default());
        let provider_future = match operation {
            "ship" | "reship" => {
                self.provider_registry.execute_ship(provider_id, request)
            }
            "cancel" => self.provider_registry.execute_cancel(provider_id, request),
            _ => {
                return Err(PortError::validation(
                    "fulfillment.provider_operation_invalid",
                    "unsupported fulfillment provider operation",
                ));
            }
        };
        let provider_result = match tokio::time::timeout(deadline, provider_future).await {
            Ok(result) => result,
            Err(_) => {
                if let Err(checkpoint_error) = self
                    .operation_journal
                    .mark_execution_reconciliation_required(
                        tenant_id,
                        journal_operation.id,
                        None,
                        None,
                        "fulfillment provider operation exceeded its declared deadline; external outcome is unknown",
                    )
                    .await
                {
                    tracing::error!(
                        boundary = ADMIN_COMMAND_BOUNDARY,
                        owner_operation,
                        operation,
                        provider_operation_id_non_nil = !journal_operation.id.is_nil(),
                        checkpoint_failed = true,
                        internal_code = %map_fulfillment_error_without_context(checkpoint_error).code,
                        "provider deadline reconciliation could not be checkpointed"
                    );
                    return Err(PortError::unavailable(
                        "fulfillment.provider_journal_failed",
                        "fulfillment provider operation could not be safely checkpointed",
                    ));
                }
                return Err(PortError::conflict(
                    "fulfillment.reconciliation_required",
                    "fulfillment provider operation exceeded its deadline and requires reconciliation",
                ));
            }
        };
        let provider_result = match provider_result {
            Ok(result) => result,
            Err(FulfillmentError::ProviderResultInvalid(reason)) => {
                if let Err(checkpoint_error) = self
                    .operation_journal
                    .mark_execution_reconciliation_required(
                        tenant_id,
                        journal_operation.id,
                        None,
                        None,
                        format!(
                            "fulfillment provider returned an invalid result after execution: {reason}"
                        ),
                    )
                    .await
                {
                    tracing::error!(
                        boundary = ADMIN_COMMAND_BOUNDARY,
                        owner_operation,
                        operation,
                        provider_operation_id_non_nil = !journal_operation.id.is_nil(),
                        checkpoint_failed = true,
                        internal_code = %map_fulfillment_error_without_context(checkpoint_error).code,
                        "fulfillment provider invalid-result reconciliation could not be checkpointed"
                    );
                    return Err(PortError::unavailable(
                        "fulfillment.provider_journal_failed",
                        "fulfillment provider operation could not be safely checkpointed",
                    ));
                }
                return Err(PortError::conflict(
                    "fulfillment.reconciliation_required",
                    "fulfillment provider returned an invalid result and requires reconciliation",
                ));
            }
            Err(error) => {
                if let Err(checkpoint_error) = self
                    .operation_journal
                    .mark_provider_error(
                        tenant_id,
                        journal_operation.id,
                        "fulfillment.provider_operation_failed",
                    )
                    .await
                {
                    tracing::error!(
                        boundary = ADMIN_COMMAND_BOUNDARY,
                        owner_operation,
                        operation,
                        provider_operation_id_non_nil = !journal_operation.id.is_nil(),
                        checkpoint_failed = true,
                        internal_code = %map_fulfillment_error_without_context(checkpoint_error).code,
                        "fulfillment provider-error checkpoint could not be persisted"
                    );
                    return Err(PortError::unavailable(
                        "fulfillment.provider_journal_failed",
                        "fulfillment provider operation could not be safely checkpointed",
                    ));
                }
                return Err(map_fulfillment_error(context, owner_operation, error));
            }
        };

        let result_payload = serde_json::to_value(&provider_result).map_err(|_| {
            PortError::invariant_violation(
                "fulfillment.provider_result_invalid",
                "fulfillment provider result could not be normalized",
            )
        })?;
        self.operation_journal
            .mark_provider_succeeded(
                tenant_id,
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
        tenant_id: Uuid,
        owner_operation: &'static str,
        operation_id: Uuid,
        operation: &'static str,
    ) {
        let checkpoint = self
            .operation_journal
            .mark_reconciliation_required(
                tenant_id,
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
        tenant_id: Uuid,
        owner_operation: &'static str,
        operation_id: Uuid,
        operation: &'static str,
    ) -> Result<(), PortError> {
        let current = self
            .operation_journal
            .get(tenant_id, operation_id)
            .await
            .map_err(|error| map_fulfillment_error(context, owner_operation, error))?;
        if current.status == PROVIDER_OPERATION_COMMITTED {
            return Ok(());
        }
        if let Err(commit_error) = self
            .operation_journal
            .mark_committed(tenant_id, operation_id)
            .await
        {
            tracing::error!(
                boundary = ADMIN_COMMAND_BOUNDARY,
                owner_operation,
                operation,
                provider_operation_id_non_nil = !operation_id.is_nil(),
                commit_failed = true,
                internal_code = %map_fulfillment_error_without_context(commit_error).code,
                "fulfillment provider operation journal commit failed"
            );
            if let Err(checkpoint_error) = self
                .operation_journal
                .mark_reconciliation_required(
                    tenant_id,
                    operation_id,
                    format!("fulfillment.local_{operation}_journal_commit_failed"),
                )
                .await
            {
                tracing::error!(
                    boundary = ADMIN_COMMAND_BOUNDARY,
                    owner_operation,
                    operation,
                    provider_operation_id_non_nil = !operation_id.is_nil(),
                    checkpoint_failed = true,
                    internal_code = %map_fulfillment_error_without_context(checkpoint_error).code,
                    "fulfillment provider journal reconciliation marker could not be persisted"
                );
            }
            return Err(PortError::conflict(
                "fulfillment.reconciliation_required",
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
        FulfillmentError::ProviderResultInvalid(_) => PortError::conflict(
            "fulfillment.reconciliation_required",
            "fulfillment provider result requires reconciliation",
        ),
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
        FulfillmentError::ShippingOptionTranslationRevisionConflict(_) => PortError::conflict(
            "fulfillment.shipping_option_translation_revision_conflict",
            "shipping option translation revision conflicts with the current state",
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
        FulfillmentError::ProviderResultInvalid(_) => "provider_result_invalid",
        FulfillmentError::ShippingOptionNotFound(_) => "shipping_option_not_found",
        FulfillmentError::FulfillmentNotFound(_) => "fulfillment_not_found",
        FulfillmentError::ShippingOptionTranslationRevisionConflict(_) => {
            "shipping_option_translation_revision_conflict"
        }
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
    context: &PortContext,
    tenant_id: Uuid,
    fulfillment_id: Uuid,
    metadata: Value,
) -> Result<FulfillmentProviderOperationRequest, PortError> {
    validate_provider_operation_metadata(&metadata)?;

    let idempotency_key = context
        .idempotency_key
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .ok_or_else(|| {
            PortError::validation(
                "fulfillment.provider_idempotency_key_missing",
                "fulfillment provider operation requires caller-owned idempotency identity",
            )
        })?;

    if idempotency_key.len() > 191 {
        return Err(PortError::validation(
            "fulfillment.provider_idempotency_key_invalid",
            "fulfillment provider idempotency identity is too long",
        ));
    }

    Ok(FulfillmentProviderOperationRequest {
        tenant_id,
        fulfillment_id,
        idempotency_key: Some(idempotency_key.to_string()),
        metadata,
    })
}

fn deserialize_provider_result(
    operation: &provider_operation::Model,
) -> Result<FulfillmentProviderOperationResult, PortError> {
    let value = operation.provider_result.clone().ok_or_else(|| {
        PortError::invariant_violation(
            "fulfillment.provider_result_missing",
            "fulfillment provider operation has no persisted provider result",
        )
    })?;
    serde_json::from_value(value).map_err(|_| {
        PortError::invariant_violation(
            "fulfillment.provider_result_invalid",
            "fulfillment provider operation has an invalid persisted provider result",
        )
    })
}

fn validate_provider_operation_metadata(metadata: &Value) -> Result<(), PortError> {
    if !metadata.is_object() {
        return Err(PortError::validation(
            "fulfillment.provider_metadata_invalid",
            "fulfillment provider metadata must be a JSON object",
        ));
    }
    validate_provider_metadata_safety(metadata).map_err(|_| {
        PortError::validation(
            "fulfillment.provider_metadata_invalid",
            "fulfillment provider metadata contains restricted sensitive fields",
        )
    })?;
    Ok(())
}

fn merge_metadata(current: Value, patch: Value) -> Result<Value, PortError> {
    validate_provider_operation_metadata(&current)?;
    validate_provider_operation_metadata(&patch)?;

    match (current, patch) {
        (Value::Object(mut current), Value::Object(patch)) => {
            for (key, value) in patch {
                current.insert(key, value);
            }
            Ok(Value::Object(current))
        }
        _ => unreachable!("provider metadata was validated as object"),
    }
}

#[cfg(test)]
mod tests {
    use rustok_api::{PortActor, PortErrorKind};

    use super::*;


    #[test]
    fn invalid_provider_result_maps_to_reconciliation_conflict() {
        let context = PortContext::new(
            "tenant-1",
            PortActor::user("actor-1"),
            "en",
            "commerce-admin-fulfillment:ship:test",
        );

        let error = map_fulfillment_error(
            &context,
            "ship_admin_fulfillment",
            FulfillmentError::ProviderResultInvalid(
                "tracking number exceeds 100 characters".into(),
            ),
        );

        assert!(matches!(error.kind, PortErrorKind::Conflict));
        assert_eq!(error.code, "fulfillment.reconciliation_required");
        assert!(!error.retryable);
    }

    #[test]
    fn provider_operation_uses_caller_owned_idempotency_key() {
        let context = PortContext::new(
            "tenant-1",
            PortActor::user("actor-1"),
            "en",
            "commerce-admin-fulfillment:ship:test",
        )
        .with_idempotency_key("caller-owned-key");

        let request = operation_request(
            &context,
            Uuid::new_v4(),
            Uuid::new_v4(),
            serde_json::json!({"example": "value"}),
        )
        .expect("caller-owned key should produce provider request");

        assert_eq!(request.idempotency_key.as_deref(), Some("caller-owned-key"));
    }

    #[test]
    fn provider_operation_rejects_missing_idempotency_key() {
        let context = PortContext::new(
            "tenant-1",
            PortActor::user("actor-1"),
            "en",
            "commerce-admin-fulfillment:ship:test",
        );

        let error = operation_request(
            &context,
            Uuid::new_v4(),
            Uuid::new_v4(),
            serde_json::json!({"example": "value"}),
        )
        .expect_err("missing caller-owned key must fail closed");

        assert!(matches!(error.kind, PortErrorKind::Validation));
        assert_eq!(error.code, "fulfillment.provider_idempotency_key_missing");
    }

    #[test]
    fn provider_operation_rejects_non_object_metadata_before_execution() {
        let context = PortContext::new(
            "tenant-1",
            PortActor::user("actor-1"),
            "en",
            "commerce-admin-fulfillment:ship:test",
        )
        .with_idempotency_key("caller-owned-key");

        let error = operation_request(
            &context,
            Uuid::new_v4(),
            Uuid::new_v4(),
            Value::Null,
        )
        .expect_err("provider metadata must be structured before execution");

        assert!(matches!(error.kind, PortErrorKind::Validation));
        assert_eq!(error.code, "fulfillment.provider_metadata_invalid");
    }

    #[test]
    fn merge_metadata_rejects_scalar_inputs_instead_of_replacing_them() {
        let error = merge_metadata(
            serde_json::json!("legacy scalar"),
            serde_json::json!({
                "commerce_orchestration": {
                    "operation": "ship"
                }
            }),
        )
        .expect_err("scalar metadata must not be silently replaced");

        assert!(matches!(error.kind, PortErrorKind::Validation));
        assert_eq!(error.code, "fulfillment.provider_metadata_invalid");
    }
}
