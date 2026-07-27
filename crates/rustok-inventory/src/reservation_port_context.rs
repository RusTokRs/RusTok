use std::sync::Arc;

use async_trait::async_trait;
use rustok_api::{PortCallPolicy, PortContext, PortError, PortErrorKind};
use rustok_outbox::TransactionalEventBus;
use sea_orm::DatabaseConnection;
use uuid::Uuid;

use crate::ports_impl::{
    InventoryAvailabilityRequest, InventoryAvailabilitySnapshot, InventoryReservationPort,
    InventoryReservationReleaseRequest, InventoryReservationReleaseSnapshot,
    InventoryReservationRequest, InventoryReservationSnapshot,
};

const INVENTORY_OWNER: &str = "rustok_inventory";
const INVENTORY_RESERVATION_BOUNDARY: &str = "inventory_reservation_port";
const AVAILABILITY_OPERATION: &str = "check_availability";
const RESERVE_OPERATION: &str = "reserve_inventory";
const RELEASE_OPERATION: &str = "release_inventory_reservation";

#[derive(Clone)]
pub struct InProcessInventoryReservationPort {
    inner: Arc<dyn InventoryReservationPort>,
}

impl InProcessInventoryReservationPort {
    pub fn new(db: DatabaseConnection, event_bus: TransactionalEventBus) -> Self {
        Self {
            inner: Arc::new(crate::InventoryService::new(db, event_bus)),
        }
    }
}

pub fn in_process_inventory_reservation_port(
    db: DatabaseConnection,
    event_bus: TransactionalEventBus,
) -> Arc<dyn InventoryReservationPort> {
    Arc::new(InProcessInventoryReservationPort::new(db, event_bus))
}

#[async_trait]
impl InventoryReservationPort for InProcessInventoryReservationPort {
    async fn check_availability(
        &self,
        context: PortContext,
        request: InventoryAvailabilityRequest,
    ) -> Result<InventoryAvailabilitySnapshot, PortError> {
        require_inventory_reservation_read_admission(&context, AVAILABILITY_OPERATION)?;
        parse_inventory_reservation_tenant_id(&context, AVAILABILITY_OPERATION)?;
        let diagnostic_context = context.clone();
        let variant_id = request.variant_id;
        let quantity = request.requested_quantity;
        let result = self.inner.check_availability(context, request).await;
        result.map_err(|error| {
            map_inventory_reservation_local_port_error(
                &diagnostic_context,
                AVAILABILITY_OPERATION,
                variant_id,
                quantity,
                error,
            )
        })
    }

    #[allow(deprecated)]
    async fn reserve_inventory(
        &self,
        context: PortContext,
        request: InventoryReservationRequest,
    ) -> Result<InventoryReservationSnapshot, PortError> {
        require_inventory_reservation_write_admission(&context, RESERVE_OPERATION)?;
        parse_inventory_reservation_tenant_id(&context, RESERVE_OPERATION)?;
        let diagnostic_context = context.clone();
        let variant_id = request.variant_id;
        let quantity = request.quantity;
        let result = self.inner.reserve_inventory(context, request).await;
        result.map_err(|error| {
            map_inventory_reservation_local_port_error(
                &diagnostic_context,
                RESERVE_OPERATION,
                variant_id,
                quantity,
                error,
            )
        })
    }

    #[allow(deprecated)]
    async fn release_inventory_reservation(
        &self,
        context: PortContext,
        request: InventoryReservationReleaseRequest,
    ) -> Result<InventoryReservationReleaseSnapshot, PortError> {
        require_inventory_reservation_write_admission(&context, RELEASE_OPERATION)?;
        parse_inventory_reservation_tenant_id(&context, RELEASE_OPERATION)?;
        let diagnostic_context = context.clone();
        let variant_id = request.variant_id;
        let quantity = request.quantity;
        let result = self
            .inner
            .release_inventory_reservation(context, request)
            .await;
        result.map_err(|error| {
            map_inventory_reservation_local_port_error(
                &diagnostic_context,
                RELEASE_OPERATION,
                variant_id,
                quantity,
                error,
            )
        })
    }
}

fn map_inventory_reservation_local_port_error(
    context: &PortContext,
    operation: &'static str,
    variant_id: Uuid,
    quantity: i32,
    error: PortError,
) -> PortError {
    let local_operation = match (error.code.as_str(), error.message.as_str()) {
        ("inventory.validation", "inventory request is invalid") => {
            if operation == AVAILABILITY_OPERATION {
                "validate_availability_request"
            } else if operation == RESERVE_OPERATION {
                "validate_reservation_request"
            } else if operation == RELEASE_OPERATION {
                "validate_reservation_release_request"
            } else {
                return error;
            }
        }
        ("inventory.variant_not_found", "inventory variant was not found") => "load_variant",
        (
            "inventory.insufficient_inventory",
            "inventory reservation conflicts with available stock",
        ) if operation == RESERVE_OPERATION => "reserve_available_stock",
        (
            "inventory.database_unavailable",
            "inventory storage is temporarily unavailable",
        ) => "owner_storage",
        (
            "inventory.invariant_violation",
            "inventory operation violated an owner invariant",
        ) => "owner_invariant",
        _ => return error,
    };
    let technical_failure = matches!(
        &error.kind,
        PortErrorKind::Unavailable | PortErrorKind::Timeout | PortErrorKind::InvariantViolation
    );
    if technical_failure {
        tracing::error!(
            error = ?error,
            owner = INVENTORY_OWNER,
            operation,
            local_operation,
            correlation_id = %context.correlation_id,
            tenant_id = %context.tenant_id,
            actor = ?context.actor,
            channel = ?context.channel,
            locale = %context.locale,
            causation_id = ?context.causation_id,
            traceparent = ?context.traceparent,
            idempotency_key = ?context.idempotency_key,
            deadline_ms = ?context.deadline_ms,
            variant_id = %variant_id,
            request_quantity = quantity,
            internal_code = %error.code,
            internal_message = %error.message,
            error_kind = ?error.kind,
            retryable = error.retryable,
            boundary = INVENTORY_RESERVATION_BOUNDARY,
            "inventory availability or quantity reservation local technical outcome retained delegated context"
        );
    } else {
        tracing::warn!(
            error = ?error,
            owner = INVENTORY_OWNER,
            operation,
            local_operation,
            correlation_id = %context.correlation_id,
            tenant_id = %context.tenant_id,
            actor = ?context.actor,
            channel = ?context.channel,
            locale = %context.locale,
            causation_id = ?context.causation_id,
            traceparent = ?context.traceparent,
            idempotency_key = ?context.idempotency_key,
            deadline_ms = ?context.deadline_ms,
            variant_id = %variant_id,
            request_quantity = quantity,
            internal_code = %error.code,
            internal_message = %error.message,
            error_kind = ?error.kind,
            retryable = error.retryable,
            boundary = INVENTORY_RESERVATION_BOUNDARY,
            "inventory availability or quantity reservation local outcome retained delegated context"
        );
    }
    error
}

fn require_inventory_reservation_read_admission(
    context: &PortContext,
    operation: &'static str,
) -> Result<(), PortError> {
    context.require_policy(PortCallPolicy::read()).map_err(|error| {
        log_inventory_reservation_admission_rejection(context, operation, "policy", &error);
        error
    })
}

fn require_inventory_reservation_write_admission(
    context: &PortContext,
    operation: &'static str,
) -> Result<(), PortError> {
    context.require_policy(PortCallPolicy::write()).map_err(|error| {
        log_inventory_reservation_admission_rejection(context, operation, "policy", &error);
        error
    })?;
    context.require_write_semantics().map_err(|error| {
        log_inventory_reservation_admission_rejection(
            context,
            operation,
            "write_semantics",
            &error,
        );
        error
    })
}

fn log_inventory_reservation_admission_rejection(
    context: &PortContext,
    operation: &'static str,
    admission_phase: &'static str,
    error: &PortError,
) {
    match &error.kind {
        PortErrorKind::Unavailable | PortErrorKind::Timeout | PortErrorKind::InvariantViolation => {
            tracing::error!(
                error = ?error,
                owner = INVENTORY_OWNER,
                operation,
                admission_phase,
                correlation_id = %context.correlation_id,
                tenant_id = %context.tenant_id,
                actor = ?context.actor,
                channel = ?context.channel,
                locale = %context.locale,
                causation_id = ?context.causation_id,
                traceparent = ?context.traceparent,
                idempotency_key = ?context.idempotency_key,
                deadline_ms = ?context.deadline_ms,
                internal_code = %error.code,
                internal_message = %error.message,
                error_kind = ?error.kind,
                retryable = error.retryable,
                boundary = INVENTORY_RESERVATION_BOUNDARY,
                "inventory availability or quantity reservation owner admission failed"
            );
        }
        _ => {
            tracing::warn!(
                error = ?error,
                owner = INVENTORY_OWNER,
                operation,
                admission_phase,
                correlation_id = %context.correlation_id,
                tenant_id = %context.tenant_id,
                actor = ?context.actor,
                channel = ?context.channel,
                locale = %context.locale,
                causation_id = ?context.causation_id,
                traceparent = ?context.traceparent,
                idempotency_key = ?context.idempotency_key,
                deadline_ms = ?context.deadline_ms,
                internal_code = %error.code,
                internal_message = %error.message,
                error_kind = ?error.kind,
                retryable = error.retryable,
                boundary = INVENTORY_RESERVATION_BOUNDARY,
                "inventory availability or quantity reservation owner admission was rejected"
            );
        }
    }
}

fn parse_inventory_reservation_tenant_id(
    context: &PortContext,
    operation: &'static str,
) -> Result<Uuid, PortError> {
    Uuid::parse_str(context.tenant_id.trim()).map_err(|cause| {
        let error = PortError::validation(
            "inventory.context_invalid",
            "inventory request context is invalid",
        );
        tracing::warn!(
            parse_cause = ?cause,
            error = ?error,
            owner = INVENTORY_OWNER,
            operation,
            validation_phase = "tenant_id",
            correlation_id = %context.correlation_id,
            tenant_id = %context.tenant_id,
            actor = ?context.actor,
            channel = ?context.channel,
            locale = %context.locale,
            causation_id = ?context.causation_id,
            traceparent = ?context.traceparent,
            idempotency_key = ?context.idempotency_key,
            deadline_ms = ?context.deadline_ms,
            internal_code = %error.code,
            internal_message = %error.message,
            error_kind = ?error.kind,
            retryable = error.retryable,
            boundary = INVENTORY_RESERVATION_BOUNDARY,
            "inventory availability or quantity reservation owner context validation was rejected"
        );
        error
    })
}
