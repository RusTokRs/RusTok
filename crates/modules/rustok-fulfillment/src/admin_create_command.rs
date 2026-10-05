use std::sync::Arc;

use async_trait::async_trait;
use rustok_api::{PortCallPolicy, PortContext, PortError};
use sea_orm::{DatabaseConnection, TransactionTrait};
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
    async fn create_local_fulfillment_and_begin_operation(
        &self,
        tenant_id: Uuid,
        provider_id: &str,
        idempotency_key: &str,
        request_payload: Value,
        input: CreateFulfillmentInput,
        context: &PortContext,
        owner_operation: &'static str,
    ) -> Result<(provider_operation::Model, FulfillmentResponse), PortError> {
        let fulfillment_id = rustok_core::generate_id();
        let txn = self.service.database().begin().await.map_err(|error| {
            map_fulfillment_error(context, owner_operation, FulfillmentError::Database(error))
        })?;

        if let Err(error) = self
            .service
            .create_fulfillment_in_txn(&txn, tenant_id, fulfillment_id, input)
            .await
        {
            drop(txn);
            return Err(map_fulfillment_error(
                context,
                owner_operation,
                error,
            ));
        }

        let operation = match self
            .operation_journal
            .begin_in_txn(
                &txn,
                BeginProviderOperation {
                    tenant_id,
                    fulfillment_id,
                    operation: "create_label".to_string(),
                    provider_id: provider_id.to_string(),
                    idempotency_key: idempotency_key.to_string(),
                    request_payload: request_payload.clone(),
                },
            )
            .await
        {
            Ok(operation) => operation,
            Err(error) => {
                drop(txn);
                if let Some(existing) = self
                    .operation_journal
                    .find_by_key(tenant_id, provider_id, idempotency_key)
                    .await
                    .map_err(map_fulfillment_error_without_context)?
                {
                    ensure_create_label_request_unchanged(&existing, &request_payload)?;
                    let fulfillment = self
                        .service
                        .get_fulfillment(tenant_id, existing.fulfillment_id)
                        .await
                        .map_err(|read_error| match read_error {
                            FulfillmentError::FulfillmentNotFound(_) => PortError::conflict(
                                "fulfillment.reconciliation_required",
                                "fulfillment provider operation exists but its local fulfillment is missing",
                            ),
                            other => map_fulfillment_error(context, owner_operation, other),
                        })?;
                    return Ok((existing, fulfillment));
                }
                return Err(map_fulfillment_error_without_context(error));
            }
        };

        if operation.fulfillment_id != fulfillment_id {
            drop(txn);
            ensure_create_label_request_unchanged(&operation, &request_payload)?;
            let fulfillment = self
                .service
                .get_fulfillment(tenant_id, operation.fulfillment_id)
                .await
                .map_err(|error| match error {
                    FulfillmentError::FulfillmentNotFound(_) => PortError::conflict(
                        "fulfillment.reconciliation_required",
                        "fulfillment provider operation exists but its local fulfillment is missing",
                    ),
                    other => map_fulfillment_error(context, owner_operation, other),
                })?;
            return Ok((operation, fulfillment));
        }

        txn.commit().await.map_err(|error| {
            map_fulfillment_error(
                context,
                owner_operation,
                FulfillmentError::Database(error),
            )
        })?;

        let fulfillment = self
            .service
            .get_fulfillment(tenant_id, fulfillment_id)
            .await
            .map_err(|error| match error {
                FulfillmentError::FulfillmentNotFound(_) => PortError::conflict(
                    "fulfillment.reconciliation_required",
                    "fulfillment operation committed but its local fulfillment is missing",
                ),
                other => map_fulfillment_error(context, owner_operation, other),
            })?;

        Ok((operation, fulfillment))
    }

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
        let existing_operation = self
            .operation_journal
            .find_by_key(tenant_id, provider_id.as_str(), idempotency_key)
            .await
            .map_err(map_fulfillment_error_without_context)?;

        let (operation, fulfillment) = if let Some(existing) = existing_operation {
            ensure_create_label_request_unchanged(&existing, &request_payload)?;
            let fulfillment = self
                .service
                .get_fulfillment(tenant_id, existing.fulfillment_id)
                .await
                .map_err(|error| match error {
                    FulfillmentError::FulfillmentNotFound(_) => PortError::conflict(
                        "fulfillment.reconciliation_required",
                        "fulfillment provider operation exists but its local fulfillment is missing",
                    ),
                    other => map_fulfillment_error(&context, OPERATION, other),
                })?;
            (existing, fulfillment)
        } else {
            self
                .create_local_fulfillment_and_begin_operation(
                    tenant_id,
                    provider_id.as_str(),
                    idempotency_key,
                    request_payload.clone(),
                    request.input.clone(),
                    &context,
                    OPERATION,
                )
                .await?
        };

        if operation.status == PROVIDER_OPERATION_EXECUTING {
            return Err(PortError::conflict(
                "fulfillment.provider_operation_in_progress",
                "fulfillment create-label operation is already in progress",
            ));
        }

        if matches!(
            operation.status.as_str(),
            PROVIDER_OPERATION_COMMITTED
                | PROVIDER_OPERATION_SUCCEEDED
                | PROVIDER_OPERATION_RECONCILIATION_REQUIRED
        ) {
            return Ok(fulfillment);
        }

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
            .execute_create_label(
                &context,
                OPERATION,
                provider_id.as_str(),
                operation.clone(),
                provider_request,
            )
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
            self.commit_create_label_provider_result(
                context,
                owner_operation,
                &operation,
                &result,
            )
            .await?;
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
                self.commit_create_label_provider_result(
                    context,
                    owner_operation,
                    &current,
                    &result,
                )
                .await?;
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
            Err(FulfillmentError::ProviderResultInvalid(reason)) => {
                if let Err(checkpoint_error) = self
                    .operation_journal
                    .mark_execution_reconciliation_required(
                        operation.tenant_id,
                        operation.id,
                        None,
                        None,
                        format!(
                            "create_label provider returned an invalid result after execution: {reason}"
                        ),
                    )
                    .await
                {
                    tracing::error!(
                        boundary = ADMIN_CREATE_BOUNDARY,
                        owner_operation,
                        provider_operation_id_non_nil = !operation.id.is_nil(),
                        checkpoint_failed = true,
                        internal_code = %map_fulfillment_error_without_context(checkpoint_error).code,
                        "create-label invalid-result reconciliation could not be checkpointed"
                    );
                }
                return Err(PortError::conflict(
                    "fulfillment.reconciliation_required",
                    "create-label provider returned an invalid result and requires reconciliation",
                ));
            }
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
        self.commit_create_label_provider_result(
            context,
            owner_operation,
            &operation,
            &result,
        )
        .await?;
        Ok(result)
    }

    async fn commit_create_label_provider_result(
        &self,
        context: &PortContext,
        owner_operation: &'static str,
        operation: &provider_operation::Model,
        result: &FulfillmentProviderOperationResult,
    ) -> Result<(), PortError> {
        match self
            .service
            .commit_create_label_provider_result(
                operation.tenant_id,
                operation.fulfillment_id,
                operation.id,
                result,
            )
            .await
        {
            Ok(_) => Ok(()),
            Err(error) => {
                if let Err(checkpoint_error) = self
                    .operation_journal
                    .mark_reconciliation_required(
                        operation.tenant_id,
                        operation.id,
                        "create_label provider result could not be committed locally",
                    )
                    .await
                {
                    tracing::error!(
                        boundary = ADMIN_CREATE_BOUNDARY,
                        owner_operation,
                        provider_operation_id_non_nil = !operation.id.is_nil(),
                        fulfillment_id_non_nil = !operation.fulfillment_id.is_nil(),
                        checkpoint_failed = true,
                        internal_code = %map_fulfillment_error_without_context(checkpoint_error).code,
                        "create-label local persistence reconciliation marker could not be persisted"
                    );
                }
                tracing::error!(
                    boundary = ADMIN_CREATE_BOUNDARY,
                    owner_operation,
                    provider_operation_id_non_nil = !operation.id.is_nil(),
                    fulfillment_id_non_nil = !operation.fulfillment_id.is_nil(),
                    local_commit_failed = true,
                    internal_code = %map_fulfillment_error_without_context(error).code,
                    "create-label provider result could not be committed locally"
                );
                Err(PortError::conflict(
                    "fulfillment.reconciliation_required",
                    "fulfillment create-label operation requires reconciliation",
                ))
            }
        }
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
        FulfillmentError::Validation(_) => {
            PortError::validation("fulfillment.validation", "fulfillment request is invalid")
        }
        FulfillmentError::ProviderResultInvalid(_) => PortError::conflict(
            "fulfillment.reconciliation_required",
            "fulfillment provider result requires reconciliation",
        ),
        FulfillmentError::ShippingOptionNotFound(_) | FulfillmentError::FulfillmentNotFound(_) => {
            PortError::not_found(
                "fulfillment.not_found",
                "fulfillment resource was not found",
            )
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
        FulfillmentError::ProviderResultInvalid(_) => (
            "provider_result_invalid",
            PortError::conflict(
                "fulfillment.reconciliation_required",
                "fulfillment provider result requires reconciliation",
            ),
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
    use crate::services::PROVIDER_OPERATION_PENDING;

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
    fn invalid_provider_result_maps_to_reconciliation_conflict() {
        let error = map_fulfillment_error_without_context(
            FulfillmentError::ProviderResultInvalid(
                "tracking number exceeds 100 characters".to_string(),
            ),
        );

        assert!(matches!(error.kind, rustok_api::PortErrorKind::Conflict));
        assert_eq!(error.code, "fulfillment.reconciliation_required");
        assert!(!error.retryable);
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

        assert!(
            ensure_create_label_request_unchanged(&existing, &existing.request_payload).is_err()
        );
    }
}
