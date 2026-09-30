use std::sync::Arc;

use async_trait::async_trait;
use rustok_api::{PortCallPolicy, PortContext, PortError};
use sea_orm::DatabaseConnection;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use uuid::Uuid;
use validator::Validate;

use crate::dto::{CreateFulfillmentInput, FulfillmentResponse};
use crate::entities::provider_operation;
use crate::error::{FulfillmentError, FulfillmentResult};
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
    async fn begin_create_label_operation(
        &self,
        tenant_id: Uuid,
        provider_id: &str,
        idempotency_key: &str,
        request_payload: Value,
    ) -> Result<provider_operation::Model, PortError> {
        const OPERATION: &str = "create_admin_fulfillment";
        if let Some(existing) = self
            .operation_journal
            .find_by_key(tenant_id, provider_id, idempotency_key)
            .await
            .map_err(|error| map_fulfillment_error_without_context(error))?
        {
            ensure_create_label_request_unchanged(&existing, &request_payload)?;
            return Ok(existing);
        }

        let candidate_fulfillment_id = rustok_core::generate_id();
        match self
            .operation_journal
            .begin(BeginProviderOperation {
                tenant_id,
                fulfillment_id: candidate_fulfillment_id,
                operation: "create_label".to_string(),
                provider_id: provider_id.to_string(),
                idempotency_key: idempotency_key.to_string(),
                request_payload: request_payload.clone(),
            })
            .await
        {
            Ok(operation) => Ok(operation),
            Err(first_error) => {
                let existing = self
                    .operation_journal
                    .find_by_key(tenant_id, provider_id, idempotency_key)
                    .await
                    .map_err(|error| map_fulfillment_error_without_context(error))?;
                match existing {
                    Some(existing) => {
                        ensure_create_label_request_unchanged(&existing, &request_payload)?;
                        Ok(existing)
                    }
                    None => {
                        tracing::error!(
                            boundary = ADMIN_CREATE_BOUNDARY,
                            owner_operation = OPERATION,
                            tenant_id_non_nil = !tenant_id.is_nil(),
                            provider_id_length = provider_id.len(),
                            idempotency_key_length = idempotency_key.len(),
                            "create-label journal begin failed without an existing operation"
                        );
                        Err(map_fulfillment_error_without_context(first_error))
                    }
                }
            }
        }
    }

    async fn ensure_local_fulfillment(
        &self,
        tenant_id: Uuid,
        fulfillment_id: Uuid,
        input: CreateFulfillmentInput,
    ) -> FulfillmentResult<FulfillmentResponse> {
        match self.service.get_fulfillment(tenant_id, fulfillment_id).await {
            Ok(existing) => Ok(existing),
            Err(FulfillmentError::FulfillmentNotFound(_)) => {
                match self
                    .service
                    .create_fulfillment_with_id(tenant_id, fulfillment_id, input.clone())
                    .await
                {
                    Ok(created) => Ok(created),
                    Err(create_error) => match self
                        .service
                        .get_fulfillment(tenant_id, fulfillment_id)
                        .await
                    {
                        Ok(existing) => Ok(existing),
                        Err(FulfillmentError::FulfillmentNotFound(_)) => Err(create_error),
                        Err(read_error) => Err(read_error),
                    },
                }
            }
            Err(error) => Err(error),
        }
    }
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
        let idempotency_key = context
            .idempotency_key
            .as_deref()
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .ok_or_else(|| {
                PortError::validation(
                    "fulfillment.provider_idempotency_key_missing",
                    "fulfillment create requires caller-owned idempotency identity",
                )
            })?;
        if idempotency_key.len() > 191 {
            return Err(PortError::validation(
                "fulfillment.provider_idempotency_key_invalid",
                "fulfillment create idempotency identity is too long",
            ));
        }

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

        let request_payload = serde_json::to_value(&request).map_err(|_| {
            PortError::validation(
                "fulfillment.provider_request_invalid",
                "fulfillment create request could not be normalized",
            )
        })?;
        let operation = self
            .begin_create_label_operation(
                tenant_id,
                provider_id.as_str(),
                idempotency_key,
                request_payload,
            )
            .await?;

        if operation.status == PROVIDER_OPERATION_EXECUTING {
            return Err(PortError::conflict(
                "fulfillment.provider_operation_in_progress",
                "fulfillment create-label operation is already in progress",
            ));
        }

        let fulfillment = if matches!(
            operation.status.as_str(),
            PROVIDER_OPERATION_COMMITTED
                | PROVIDER_OPERATION_SUCCEEDED
                | PROVIDER_OPERATION_RECONCILIATION_REQUIRED
        ) {
            self.service
                .get_fulfillment(tenant_id, operation.fulfillment_id)
                .await
                .map_err(|error| match error {
                    FulfillmentError::FulfillmentNotFound(_) => PortError::conflict(
                        "fulfillment.reconciliation_required",
                        "fulfillment provider operation exists but its local fulfillment is missing",
                    ),
                    other => map_fulfillment_error(&context, OPERATION, other),
                })?
        } else {
            self.ensure_local_fulfillment(
                tenant_id,
                operation.fulfillment_id,
                request.input.clone(),
            )
            .await
            .map_err(|error| map_fulfillment_error(&context, OPERATION, error))?
        };

        let provider_request = FulfillmentProviderOperationRequest {
            tenant_id,
            fulfillment_id: fulfillment.id,
            idempotency_key: Some(idempotency_key.to_string()),
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
            .execute_create_label(&context, OPERATION, provider_id.as_str(), operation.clone(), provider_request)
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

        self.service
            .get_fulfillment(tenant_id, fulfillment.id)
            .await
            .map_err(|error| match error {
                FulfillmentError::FulfillmentNotFound(_) => PortError::conflict(
                    "fulfillment.reconciliation_required",
                    "fulfillment create-label operation completed but its local fulfillment is missing",
                ),
                other => map_fulfillment_error(&context, OPERATION, other),
            })
    }
}

impl InProcessFulfillmentAdminCreateCommandPort {
    async fn execute_create_label(
        &self,
        context: &PortContext,
        owner_operation: &'static str,
        provider_id: &str,
        operation: provider_operation::Model,
        request: FulfillmentProviderOperationRequest,
    ) -> Result<FulfillmentProviderOperationResult, PortError> {

        if matches!(
            operation.status.as_str(),
            PROVIDER_OPERATION_COMMITTED
                | PROVIDER_OPERATION_SUCCEEDED
                | PROVIDER_OPERATION_RECONCILIATION_REQUIRED
        ) {
            let result = deserialize_create_label_result(context, owner_operation, &operation)?;
            if operation.status != PROVIDER_OPERATION_COMMITTED {
                self.commit_create_label(operation.tenant_id, owner_operation, operation.id)
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
            .claim_execution(operation.tenant_id, operation.id)
            .await
            .map_err(|error| map_fulfillment_error(context, owner_operation, error))?
            .is_none()
        {
            let current = self
                .operation_journal
                .get(operation.tenant_id, operation.id)
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
                    self.commit_create_label(operation.tenant_id, owner_operation, current.id)
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
                if let Err(checkpoint_error) = self
                    .operation_journal
                    .mark_provider_error(
                        operation.tenant_id,
                        operation.id,
                        "create_label provider execution failed",
                    )
                    .await
                {
                    tracing::error!(
                        boundary = ADMIN_CREATE_BOUNDARY,
                        owner_operation,
                        provider_operation_id_non_nil = !operation.id.is_nil(),
                        checkpoint_failed = true,
                        internal_code = %map_fulfillment_error_without_context(checkpoint_error).code,
                        "create-label provider error could not be checkpointed"
                    );
                }
                return Err(map_fulfillment_error(context, owner_operation, error));
            }
        };

        let result_payload = match serde_json::to_value(&result) {
            Ok(payload) => payload,
            Err(_) => {
                if let Err(checkpoint_error) = self
                    .operation_journal
                    .mark_execution_reconciliation_required(
                        operation.tenant_id,
                        operation.id,
                        result.external_reference.clone(),
                        None,
                        "create_label provider result could not be serialized",
                    )
                    .await
                {
                    tracing::error!(
                        boundary = ADMIN_CREATE_BOUNDARY,
                        owner_operation,
                        provider_operation_id_non_nil = !operation.id.is_nil(),
                        checkpoint_failed = true,
                        internal_code = %map_fulfillment_error_without_context(checkpoint_error).code,
                        "create-label serialization failure could not be checkpointed"
                    );
                }
                return Err(PortError::conflict(
                    "fulfillment.reconciliation_required",
                    "create-label provider result requires reconciliation",
                ));
            }
        };

        self.operation_journal
            .mark_provider_succeeded(
                operation.tenant_id,
                operation.id,
                result.external_reference.clone(),
                result_payload,
            )
            .await
            .map_err(|error| map_fulfillment_error(context, owner_operation, error))?;
        self.commit_create_label(operation.tenant_id, owner_operation, operation.id)
            .await?;
        Ok(result)
    }

    async fn commit_create_label(
        &self,
        tenant_id: Uuid,
        owner_operation: &'static str,
        operation_id: Uuid,
    ) -> Result<(), PortError> {
        if let Err(error) = self.operation_journal.mark_committed(tenant_id, operation_id).await {
            if let Err(checkpoint_error) = self
                .operation_journal
                .mark_reconciliation_required(
                    tenant_id,
                    operation_id,
                    "create_label provider succeeded but journal commit failed",
                )
                .await
            {
                tracing::error!(
                    boundary = ADMIN_CREATE_BOUNDARY,
                    owner_operation,
                    provider_operation_id_non_nil = !operation_id.is_nil(),
                    checkpoint_failed = true,
                    internal_code = %map_fulfillment_error_without_context(checkpoint_error).code,
                    "create-label journal reconciliation marker could not be persisted"
                );
            }
            let _ = error;
            return Err(PortError::conflict(
                "fulfillment.reconciliation_required",
                "fulfillment create-label operation requires reconciliation",
            ));
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

fn ensure_create_label_request_unchanged(
    existing: &provider_operation::Model,
    request_payload: &Value,
) -> Result<(), PortError> {
    if existing.operation != "create_label" || existing.request_payload != *request_payload {
        return Err(PortError::validation(
            "fulfillment.idempotency_key_reused",
            "idempotency key is already bound to a different fulfillment create request",
        ));
    }
    Ok(())
}

fn map_fulfillment_error_without_context(error: FulfillmentError) -> PortError {
    match error {
        FulfillmentError::Validation(_) => PortError::validation(
            "fulfillment.validation",
            "fulfillment request is invalid",
        ),
        FulfillmentError::ShippingOptionNotFound(_)
        | FulfillmentError::FulfillmentNotFound(_) => {
            PortError::not_found("fulfillment.not_found", "fulfillment resource was not found")
        }
        FulfillmentError::InvalidTransition { .. } => PortError::conflict(
            "fulfillment.invalid_transition",
            "fulfillment operation conflicts with the current state",
        ),
        FulfillmentError::ShippingOptionTranslationRevisionConflict(_) => PortError::conflict(
            "fulfillment.shipping_option_translation_revision_conflict",
            "shipping option translation revision conflicts with the current state",
        ),
        FulfillmentError::Database(_) => PortError::unavailable(
            "fulfillment.database_unavailable",
            "fulfillment storage is temporarily unavailable",
        ),
    }
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
        FulfillmentError::ShippingOptionTranslationRevisionConflict(_) => (
            "shipping_option_translation_revision_conflict",
            PortError::conflict(
                "fulfillment.shipping_option_translation_revision_conflict",
                "shipping option translation revision conflicts with the current state",
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

#[cfg(test)]
mod tests {
    use super::*;

    fn provider_operation(payload: Value) -> provider_operation::Model {
        let now = chrono::Utc::now().into();
        provider_operation::Model {
            id: Uuid::new_v4(),
            tenant_id: Uuid::new_v4(),
            fulfillment_id: Uuid::new_v4(),
            operation: "create_label".to_string(),
            provider_id: "manual".to_string(),
            idempotency_key: "caller-key".to_string(),
            status: PROVIDER_OPERATION_PENDING.to_string(),
            request_payload: payload,
            provider_reference: None,
            provider_result: None,
            error_message: None,
            created_at: now,
            updated_at: now,
            provider_completed_at: None,
            committed_at: None,
        }
    }

    #[test]
    fn create_label_replay_rejects_changed_request_payload() {
        let existing = provider_operation(serde_json::json!({
            "provider_id": "manual",
            "input": { "order_id": Uuid::new_v4() }
        }));
        let changed = serde_json::json!({
            "provider_id": "manual",
            "input": { "order_id": Uuid::new_v4() }
        });

        assert!(ensure_create_label_request_unchanged(&existing, &changed).is_err());
    }

    #[test]
    fn create_label_replay_accepts_identical_request_payload() {
        let payload = serde_json::json!({
            "provider_id": "manual",
            "input": { "order_id": Uuid::new_v4() }
        });
        let existing = provider_operation(payload.clone());

        assert!(ensure_create_label_request_unchanged(&existing, &payload).is_ok());
    }

    #[test]
    fn create_label_replay_rejects_non_create_operation() {
        let mut existing = provider_operation(serde_json::json!({
            "provider_id": "manual",
            "input": {}
        }));
        existing.operation = "ship".to_string();

        assert!(ensure_create_label_request_unchanged(
            &existing,
            &existing.request_payload
        )
        .is_err());
    }
}
