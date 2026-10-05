use std::sync::Arc;

use async_trait::async_trait;
use rustok_api::{PortCallPolicy, PortContext, PortError};
use rustok_outbox::idempotency::{self, Admission};
use sea_orm::{DatabaseConnection, TransactionTrait};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::dto::{CreateShippingOptionInput, ShippingOptionResponse, UpdateShippingOptionInput};
use crate::error::FulfillmentError;
use crate::services::FulfillmentService;

const SHIPPING_OPTION_ADMIN_COMMAND_BOUNDARY: &str =
    "fulfillment_shipping_option_admin_command_port";

#[async_trait]
pub trait ShippingOptionAdminCommandPort: Send + Sync {
    async fn create_shipping_option(
        &self,
        context: PortContext,
        request: CreateAdminShippingOptionRequest,
    ) -> Result<ShippingOptionResponse, PortError>;

    async fn update_shipping_option(
        &self,
        context: PortContext,
        request: UpdateAdminShippingOptionRequest,
    ) -> Result<ShippingOptionResponse, PortError>;

    async fn deactivate_shipping_option(
        &self,
        context: PortContext,
        request: DeactivateAdminShippingOptionRequest,
    ) -> Result<ShippingOptionResponse, PortError>;

    async fn reactivate_shipping_option(
        &self,
        context: PortContext,
        request: ReactivateAdminShippingOptionRequest,
    ) -> Result<ShippingOptionResponse, PortError>;
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreateAdminShippingOptionRequest {
    pub input: CreateShippingOptionInput,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UpdateAdminShippingOptionRequest {
    pub shipping_option_id: Uuid,
    pub input: UpdateShippingOptionInput,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct DeactivateAdminShippingOptionRequest {
    pub shipping_option_id: Uuid,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ReactivateAdminShippingOptionRequest {
    pub shipping_option_id: Uuid,
}

pub struct InProcessShippingOptionAdminCommandPort {
    service: FulfillmentService,
}

impl InProcessShippingOptionAdminCommandPort {
    pub fn new(db: DatabaseConnection) -> Self {
        Self {
            service: FulfillmentService::new(db),
        }
    }
}

pub fn in_process_shipping_option_admin_command_port(
    db: DatabaseConnection,
) -> Arc<dyn ShippingOptionAdminCommandPort> {
    Arc::new(InProcessShippingOptionAdminCommandPort::new(db))
}

#[derive(Clone)]
pub struct ShippingOptionAdminCommandRuntime {
    command_port: Arc<dyn ShippingOptionAdminCommandPort>,
}

impl ShippingOptionAdminCommandRuntime {
    pub fn new(command_port: Arc<dyn ShippingOptionAdminCommandPort>) -> Self {
        Self { command_port }
    }

    pub fn in_process(db: DatabaseConnection) -> Self {
        Self::new(in_process_shipping_option_admin_command_port(db))
    }

    pub fn command_port(&self) -> Arc<dyn ShippingOptionAdminCommandPort> {
        self.command_port.clone()
    }
}

#[async_trait]
impl ShippingOptionAdminCommandPort for InProcessShippingOptionAdminCommandPort {
    async fn create_shipping_option(
        &self,
        context: PortContext,
        request: CreateAdminShippingOptionRequest,
    ) -> Result<ShippingOptionResponse, PortError> {
        const OPERATION: &str = "create_admin_shipping_option";
        require_write_admission(&context, OPERATION)?;
        let tenant_id = parse_tenant_id(&context, OPERATION)?;
        let idempotency_key = context.idempotency_key.as_deref().unwrap_or_default();

        let lease = match idempotency::admit(
            self.service.database(),
            idempotency::OwnerOperationScope::Tenant(tenant_id),
            "fulfillment",
            idempotency_key,
            OPERATION,
            &request,
        )
        .await?
        {
            Admission::Run(lease) => lease,
            Admission::Replay(value) => {
                return serde_json::from_value(value).map_err(|error| {
                    PortError::invariant_violation(
                        "fulfillment.shipping_option_create_receipt_corrupt",
                        error.to_string(),
                    )
                });
            }
            Admission::ReplayError(error) => return Err(error),
        };

        let result = async {
            let txn = self.service.database().begin().await.map_err(|error| {
                map_fulfillment_error(
                    &context,
                    OPERATION,
                    FulfillmentError::Database(error),
                )
            })?;
            let created = match self
                .service
                .create_shipping_option_in_txn(
                    &txn,
                    tenant_id,
                    request.input.clone(),
                    lease.operation_id,
                )
                .await
            {
                Ok(created) => created,
                Err(error) => return Err(map_fulfillment_error(&context, OPERATION, error)),
            };

            if let Err(error) = idempotency::complete(&txn, lease, &created).await {
                return Err(error);
            }

            txn.commit().await.map_err(|error| {
                map_fulfillment_error(
                    &context,
                    OPERATION,
                    FulfillmentError::Database(error),
                )
            })?;

            Ok(created)
        }
        .await;

        if let Err(error) = &result {
            fail_receipt(&self.service, lease, error, &context, OPERATION).await;
        }
        result
    }

    async fn update_shipping_option(
        &self,
        context: PortContext,
        request: UpdateAdminShippingOptionRequest,
    ) -> Result<ShippingOptionResponse, PortError> {
        const OPERATION: &str = "update_admin_shipping_option";
        require_write_admission(&context, OPERATION)?;
        let tenant_id = parse_tenant_id(&context, OPERATION)?;
        self.service
            .update_shipping_option(tenant_id, request.shipping_option_id, request.input)
            .await
            .map_err(|error| map_fulfillment_error(&context, OPERATION, error))
    }

    async fn deactivate_shipping_option(
        &self,
        context: PortContext,
        request: DeactivateAdminShippingOptionRequest,
    ) -> Result<ShippingOptionResponse, PortError> {
        const OPERATION: &str = "deactivate_admin_shipping_option";
        require_write_admission(&context, OPERATION)?;
        let tenant_id = parse_tenant_id(&context, OPERATION)?;
        self.service
            .deactivate_shipping_option(tenant_id, request.shipping_option_id)
            .await
            .map_err(|error| map_fulfillment_error(&context, OPERATION, error))
    }

    async fn reactivate_shipping_option(
        &self,
        context: PortContext,
        request: ReactivateAdminShippingOptionRequest,
    ) -> Result<ShippingOptionResponse, PortError> {
        const OPERATION: &str = "reactivate_admin_shipping_option";
        require_write_admission(&context, OPERATION)?;
        let tenant_id = parse_tenant_id(&context, OPERATION)?;
        self.service
            .reactivate_shipping_option(tenant_id, request.shipping_option_id)
            .await
            .map_err(|error| map_fulfillment_error(&context, OPERATION, error))
    }
}

async fn fail_receipt(
    service: &FulfillmentService,
    lease: idempotency::Lease,
    error: &PortError,
    context: &PortContext,
    operation: &'static str,
) {
    if let Err(receipt_error) = idempotency::fail(service.database(), lease, error).await {
        tracing::error!(
            owner = "rustok_fulfillment",
            operation,
            correlation_id = %context.correlation_id,
            tenant_id_length = context.tenant_id.chars().count(),
            actor_id_length = context.actor.id.chars().count(),
            receipt_error = %receipt_error.message,
            "failed to persist shipping-option create failure receipt"
        );
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
        FulfillmentError::Validation(_) => PortError::validation(
            "fulfillment.validation",
            "shipping option request is invalid",
        ),
        FulfillmentError::ProviderResultInvalid(_) => PortError::conflict(
            "fulfillment.reconciliation_required",
            "fulfillment provider result requires reconciliation",
        ),
        FulfillmentError::ShippingOptionNotFound(_) => PortError::not_found(
            "fulfillment.shipping_option_not_found",
            "shipping option was not found",
        ),
        FulfillmentError::FulfillmentNotFound(_) => PortError::not_found(
            "fulfillment.not_found",
            "fulfillment resource was not found",
        ),
        FulfillmentError::ShippingOptionTranslationRevisionConflict(_) => PortError::conflict(
            "fulfillment.shipping_option_translation_revision_conflict",
            "shipping option translation revision conflicts with the current state",
        ),
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
        boundary = SHIPPING_OPTION_ADMIN_COMMAND_BOUNDARY,
        "fulfillment shipping option admin command returned a bounded owner error"
    );
    mapped
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
        boundary = SHIPPING_OPTION_ADMIN_COMMAND_BOUNDARY,
        "fulfillment shipping option admin command admission failed"
    );
}
