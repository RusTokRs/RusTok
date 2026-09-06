use std::sync::Arc;

use async_trait::async_trait;
use rustok_api::{PortCallPolicy, PortContext, PortError};
use sea_orm::DatabaseConnection;
use serde::{Deserialize, Serialize};
use uuid::Uuid;
use validator::Validate;

use crate::dto::{CreateFulfillmentInput, FulfillmentResponse};
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

const ADMIN_CREATE_BOUNDARY: &str = "fulfillment_admin_create_command_port";

#[async_trait]
pub trait FulfillmentAdminCreateCommandPort: Send + Sync {
    async fn create_fulfillment(
        &self,
        context: PortContext,
        request: CreateAdminFulfillmentRequest,
    ) -> Result<FulfillmentResponse, PortError>;
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreateAdminFulfillmentRequest {
    pub input: CreateFulfillmentInput,
    pub provider_id: String,
}

pub struct InProcessFulfillmentAdminCreateCommandPort {
    service: FulfillmentService,
    operation_journal: FulfillmentProviderOperationJournal,
    provider_registry: FulfillmentProviderRegistry,
}

impl InProcessFulfillmentAdminCreateCommandPort {
    pub fn new(db: DatabaseConnection, provider_registry: FulfillmentProviderRegistry) -> Self {
        Self {
            service: FulfillmentService::new(db.clone()),
            operation_journal: FulfillmentProviderOperationJournal::new(db),
            provider_registry,
        }
    }
}

pub fn in_process_fulfillment_admin_create_command_port(
    db: DatabaseConnection,
    provider_registry: FulfillmentProviderRegistry,
) -> Arc<dyn FulfillmentAdminCreateCommandPort> {
    Arc::new(InProcessFulfillmentAdminCreateCommandPort::new(
        db,
        provider_registry,
    ))
}

#[derive(Clone)]
pub struct FulfillmentAdminCreateCommandRuntime {
    command_port: Arc<dyn FulfillmentAdminCreateCommandPort>,
}

impl FulfillmentAdminCreateCommandRuntime {
    pub fn new(command_port: Arc<dyn FulfillmentAdminCreateCommandPort>) -> Self {
        Self { command_port }
    }

    pub fn in_process(
        db: DatabaseConnection,
        provider_registry: FulfillmentProviderRegistry,
    ) -> Self {
        Self::new(in_process_fulfillment_admin_create_command_port(
            db,
            provider_registry,
        ))
    }

    pub fn command_port(&self) -> Arc<dyn FulfillmentAdminCreateCommandPort> {
        self.command_port.clone()
    }
}

#[async_trait]
impl FulfillmentAdminCreateCommandPort for InProcessFulfillmentAdminCreateCommandPort {
    async fn create_fulfillment(
        &self,
        context: PortContext,
        request: CreateAdminFulfillmentRequest,
    ) -> Result<FulfillmentResponse, PortError> {
        const OPERATION: &str = "create_admin_fulfillment";
        context.require_policy(PortCallPolicy::write())?;
        let tenant_id = parse_tenant_id(&context, OPERATION)?;

        request.input.validate().map_err(|_| {
            PortError::validation(
                "fulfillment.validation",
                "fulfillment create request is invalid",
            )
        })?;
        let provider_id = request.provider_id.trim().to_string();
        if provider_id.is_empty() || provider_id.len() > 100 {
            return Err(PortError::validation(
                "fulfillment.provider_invalid",
                "fulfillment provider identity is invalid",
            ));
        }

        if let Some(shipping_option_id) = request.input.shipping_option_id {
            let option = self
                .service
                .get_shipping_option(tenant_id, shipping_option_id, None, None)
                .await
                .map_err(|error| map_fulfillment_error(&context, OPERATION, error))?;
            if option.provider_id != provider_id {
                return Err(PortError::validation(
                    "fulfillment.provider_mismatch",
                    "fulfillment provider does not match the selected shipping option",
                ));
            }
        } else if provider_id != MANUAL_FULFILLMENT_PROVIDER_ID {
            return Err(PortError::validation(
                "fulfillment.provider_mismatch",
                "manual fulfillment requires the manual provider",
            ));
        }

        let fulfillment = self
            .service
            .create_fulfillment(tenant_id, request.input)
            .await
            .map_err(|error| map_fulfillment_error(&context, OPERATION, error))?;

        let provider_request = FulfillmentProviderOperationRequest {
            tenant_id,
            fulfillment_id: fulfillment.id,
            idempotency_key: Some(format!("fulfillment:{}:create_label", fulfillment.id)),
            metadata: merge_metadata(
                fulfillment.metadata.clone(),
                serde_json::json!({
                    "commerce_orchestration": {
                        "operation": "create_label"
                    }
                }),
            ),
        };

        if let Err(error) = self
            .execute_create_label(&context, OPERATION, provider_id.as_str(), provider_request)
            .await
        {
            tracing::error!(
                boundary = ADMIN_CREATE_BOUNDARY,
                owner_operation = OPERATION,
                fulfillment_id_non_nil = !fulfillment.id.is_nil(),
                internal_code = %error.code,
                retryable = error.retryable,
                "fulfillment create-label execution requires reconciliation"
            );
            return Err(PortError::conflict(
                "fulfillment.reconciliation_required",
                "fulfillment create-label operation requires reconciliation",
            ));
        }

        Ok(fulfillment)
    }
}

impl InProcessFulfillmentAdminCreateCommandPort {
    async fn execute_create_label(
        &self,
        context: &PortContext,
        owner_operation: &'static str,
        provider_id: &str,
        request: FulfillmentProviderOperationRequest,
    ) -> Result<FulfillmentProviderOperationResult, PortError> {
        let idempotency_key = request
            .idempotency_key
            .as_deref()
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .ok_or_else(|| {
                PortError::validation(
                    "fulfillment.provider_idempotency_key_missing",
                    "create-label operation requires idempotency identity",
                )
            })?
            .to_string();
        let request_payload = serde_json::to_value(&request).map_err(|_| {
            PortError::validation(
                "fulfillment.provider_request_invalid",
                "create-label provider request is invalid",
            )
        })?;
        let operation = self
            .operation_journal
            .begin(BeginProviderOperation {
                tenant_id: request.tenant_id,
                fulfillment_id: request.fulfillment_id,
                operation: "create_label".to_string(),
                provider_id: provider_id.to_string(),
                idempotency_key,
                request_payload,
            })
            .await
            .map_err(|error| map_fulfillment_error(context, owner_operation, error))?;

        if matches!(
            operation.status.as_str(),
            PROVIDER_OPERATION_COMMITTED
                | PROVIDER_OPERATION_SUCCEEDED
                | PROVIDER_OPERATION_RECONCILIATION_REQUIRED
        ) {
            let result = deserialize_create_label_result(context, owner_operation, &operation)?;
            if operation.status != PROVIDER_OPERATION_COMMITTED {
                self.commit_create_label(context, owner_operation, operation.id)
                    .await?;
            }
            return Ok(result);
        }
        if operation.status == PROVIDER_OPERATION_EXECUTING {
            return Err(PortError::conflict(
                "fulfillment.provider_operation_in_progress",
                "create-label provider operation is already in progress",
            ));
        }

        if self
            .operation_journal
            .claim_execution(operation.id)
            .await
            .map_err(|error| map_fulfillment_error(context, owner_operation, error))?
            .is_none()
        {
            let current = self
                .operation_journal
                .get(operation.id)
                .await
                .map_err(|error| map_fulfillment_error(context, owner_operation, error))?;
            if matches!(
                current.status.as_str(),
                PROVIDER_OPERATION_COMMITTED
                    | PROVIDER_OPERATION_SUCCEEDED
                    | PROVIDER_OPERATION_RECONCILIATION_REQUIRED
            ) {
                let result = deserialize_create_label_result(context, owner_operation, &current)?;
                if current.status != PROVIDER_OPERATION_COMMITTED {
                    self.commit_create_label(context, owner_operation, current.id)
                        .await?;
                }
                return Ok(result);
            }
            return Err(PortError::conflict(
                "fulfillment.provider_operation_in_progress",
                "create-label provider operation is already in progress",
            ));
        }

        let result = match self
            .provider_registry
            .execute_create_label(provider_id, request)
            .await
        {
            Ok(result) => result,
            Err(error) => {
                let _ = self
                    .operation_journal
                    .mark_provider_error(operation.id, "create_label provider execution failed")
                    .await;
                return Err(map_fulfillment_error(context, owner_operation, error));
            }
        };

        let result_payload = match serde_json::to_value(&result) {
            Ok(payload) => payload,
            Err(_) => {
                let _ = self
                    .operation_journal
                    .mark_execution_reconciliation_required(
                        operation.id,
                        result.external_reference.clone(),
                        None,
                        "create_label provider result could not be serialized",
                    )
                    .await;
                return Err(PortError::conflict(
                    "fulfillment.reconciliation_required",
                    "create-label provider result requires reconciliation",
                ));
            }
        };

        self.operation_journal
            .mark_provider_succeeded(
                operation.id,
                result.external_reference.clone(),
                result_payload,
            )
            .await
            .map_err(|error| map_fulfillment_error(context, owner_operation, error))?;
        self.commit_create_label(context, owner_operation, operation.id)
            .await?;
        Ok(result)
    }

    async fn commit_create_label(
        &self,
        context: &PortContext,
        owner_operation: &'static str,
        operation_id: Uuid,
    ) -> Result<(), PortError> {
        if let Err(error) = self.operation_journal.mark_committed(operation_id).await {
            let _ = self
                .operation_journal
                .mark_reconciliation_required(
                    operation_id,
                    "create_label provider succeeded but journal commit failed",
                )
                .await;
            return Err(map_fulfillment_error(context, owner_operation, error));
        }
        Ok(())
    }
}

fn deserialize_create_label_result(
    _context: &PortContext,
    owner_operation: &'static str,
    operation: &provider_operation::Model,
) -> Result<FulfillmentProviderOperationResult, PortError> {
    let value = operation.provider_result.clone().ok_or_else(|| {
        PortError::conflict(
            "fulfillment.reconciliation_required",
            "create-label provider result is not available for safe replay",
        )
    })?;
    serde_json::from_value(value).map_err(|_| {
        tracing::error!(
            boundary = ADMIN_CREATE_BOUNDARY,
            owner_operation,
            provider_operation_id_non_nil = !operation.id.is_nil(),
            "fulfillment create-label journal contains invalid provider result"
        );
        PortError::conflict(
            "fulfillment.reconciliation_required",
            "create-label provider result is invalid and requires reconciliation",
        )
    })
}

fn parse_tenant_id(context: &PortContext, operation: &'static str) -> Result<Uuid, PortError> {
    Uuid::parse_str(context.tenant_id.as_str()).map_err(|_| {
        tracing::error!(
            boundary = ADMIN_CREATE_BOUNDARY,
            owner_operation = operation,
            tenant_id_length = context.tenant_id.len(),
            "fulfillment admin create command received invalid tenant identity"
        );
        PortError::validation(
            "fulfillment.tenant_invalid",
            "fulfillment tenant identity is invalid",
        )
    })
}

fn map_fulfillment_error(
    context: &PortContext,
    operation: &'static str,
    error: FulfillmentError,
) -> PortError {
    let (variant, mapped) = match error {
        FulfillmentError::Validation(_) => (
            "validation",
            PortError::validation("fulfillment.validation", "fulfillment request is invalid"),
        ),
        FulfillmentError::ShippingOptionNotFound(_) | FulfillmentError::FulfillmentNotFound(_) => (
            "not_found",
            PortError::not_found(
                "fulfillment.not_found",
                "fulfillment resource was not found",
            ),
        ),
        FulfillmentError::InvalidTransition { .. } => (
            "invalid_transition",
            PortError::conflict(
                "fulfillment.invalid_transition",
                "fulfillment operation conflicts with the current state",
            ),
        ),
        FulfillmentError::Database(_) => (
            "database",
            PortError::unavailable(
                "fulfillment.database_unavailable",
                "fulfillment storage is temporarily unavailable",
            ),
        ),
    };
    tracing::error!(
        boundary = ADMIN_CREATE_BOUNDARY,
        owner_operation = operation,
        error_variant = variant,
        correlation_id_present = !context.correlation_id.is_empty(),
        "fulfillment admin create owner operation failed"
    );
    mapped
}

fn merge_metadata(current: serde_json::Value, patch: serde_json::Value) -> serde_json::Value {
    match (current, patch) {
        (serde_json::Value::Object(mut current), serde_json::Value::Object(patch)) => {
            for (key, value) in patch {
                current.insert(key, value);
            }
            serde_json::Value::Object(current)
        }
        (_, patch) => patch,
    }
}
