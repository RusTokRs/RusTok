use std::sync::Arc;

use async_trait::async_trait;
use rustok_api::{PortCallPolicy, PortContext, PortError, PortErrorKind};
use sea_orm::DatabaseConnection;
use uuid::Uuid;

use crate::ports_impl::{
    InventoryIdentityReservationReleaseRequest, InventoryIdentityReservationReleaseSnapshot,
    InventoryIdentityReservationRequest, InventoryIdentityReservationSnapshot,
    InventoryReservationIdentityPort,
};

const INVENTORY_OWNER: &str = "rustok_inventory";
const INVENTORY_RESERVATION_BOUNDARY: &str = "inventory_reservation_identity_port";
const RESERVE_OPERATION: &str = "reserve_inventory_by_identity";
const RELEASE_OPERATION: &str = "release_inventory_by_identity";

struct InventoryReservationIdentityDiagnostic {
    reservation_id: Uuid,
    variant_id: Option<Uuid>,
    quantity: Option<i32>,
    line_item_id: Option<Uuid>,
    external_id_length: usize,
}

#[derive(Clone)]
pub struct PersistentInventoryReservationIdentityPort {
    inner: Arc<dyn InventoryReservationIdentityPort>,
}

impl PersistentInventoryReservationIdentityPort {
    pub fn new(db: DatabaseConnection) -> Self {
        Self {
            inner: Arc::new(crate::ports_impl::PersistentInventoryReservationIdentityPort::new(db)),
        }
    }
}

pub fn in_process_inventory_reservation_identity_port(
    db: DatabaseConnection,
) -> Arc<dyn InventoryReservationIdentityPort> {
    Arc::new(PersistentInventoryReservationIdentityPort::new(db))
}

#[async_trait]
impl InventoryReservationIdentityPort for PersistentInventoryReservationIdentityPort {
    async fn reserve_inventory_by_identity(
        &self,
        context: PortContext,
        request: InventoryIdentityReservationRequest,
    ) -> Result<InventoryIdentityReservationSnapshot, PortError> {
        require_inventory_reservation_write_admission(&context, RESERVE_OPERATION)?;
        parse_inventory_reservation_tenant_id(&context, RESERVE_OPERATION)?;
        let diagnostic_context = context.clone();
        let identity = InventoryReservationIdentityDiagnostic {
            reservation_id: request.reservation_id,
            variant_id: Some(request.variant_id),
            quantity: Some(request.quantity),
            line_item_id: request.line_item_id,
            external_id_length: request.external_id.chars().count(),
        };
        let result = self
            .inner
            .reserve_inventory_by_identity(context, request)
            .await;
        result.map_err(|error| {
            map_inventory_reservation_identity_local_port_error(
                &diagnostic_context,
                RESERVE_OPERATION,
                &identity,
                error,
            )
        })
    }

    async fn release_inventory_by_identity(
        &self,
        context: PortContext,
        request: InventoryIdentityReservationReleaseRequest,
    ) -> Result<InventoryIdentityReservationReleaseSnapshot, PortError> {
        require_inventory_reservation_write_admission(&context, RELEASE_OPERATION)?;
        parse_inventory_reservation_tenant_id(&context, RELEASE_OPERATION)?;
        let diagnostic_context = context.clone();
        let identity = InventoryReservationIdentityDiagnostic {
            reservation_id: request.reservation_id,
            variant_id: None,
            quantity: None,
            line_item_id: None,
            external_id_length: request.external_id.chars().count(),
        };
        let result = self
            .inner
            .release_inventory_by_identity(context, request)
            .await;
        result.map_err(|error| {
            map_inventory_reservation_identity_local_port_error(
                &diagnostic_context,
                RELEASE_OPERATION,
                &identity,
                error,
            )
        })
    }
}

fn map_inventory_reservation_identity_local_port_error(
    context: &PortContext,
    operation: &'static str,
    identity: &InventoryReservationIdentityDiagnostic,
    error: PortError,
) -> PortError {
    let local_operation = match (error.code.as_str(), error.message.as_str()) {
        (
            "inventory.reservation_external_id_invalid",
            "reservation external_id must contain 1 to 191 characters",
        ) => "normalize_external_id",
        ("inventory.reservation_quantity_invalid", "reservation quantity must be positive")
            if operation == RESERVE_OPERATION =>
        {
            "validate_reservation_quantity"
        }
        ("inventory.variant_not_found", "inventory variant was not found") => "load_variant",
        ("inventory.state_not_found", "variant has no configured inventory state")
            if operation == RESERVE_OPERATION =>
        {
            "load_inventory_state"
        }
        (
            "inventory.reservation_identity_conflict",
            "reservation identity is already bound to different reservation data",
        ) if operation == RESERVE_OPERATION => "validate_existing_reservation_identity",
        ("inventory.insufficient_inventory", "insufficient inventory for reservation")
            if operation == RESERVE_OPERATION =>
        {
            "reserve_available_stock"
        }
        ("inventory.reservation_not_found", "inventory reservation was not found")
            if operation == RELEASE_OPERATION =>
        {
            "load_reservation"
        }
        (
            "inventory.reservation_identity_conflict",
            "reservation id is bound to another external identity",
        ) if operation == RELEASE_OPERATION => "validate_release_external_identity",
        ("inventory.reservation_item_missing", "reservation inventory item is missing")
            if operation == RELEASE_OPERATION =>
        {
            "load_reservation_inventory_item"
        }
        (
            "inventory.reservation_identity_conflict",
            "reservation identity changed while acquiring the owner lock",
        ) if operation == RELEASE_OPERATION => "revalidate_release_identity",
        (
            "inventory.reservation_ledger_inconsistent",
            "inventory reservation ledger is inconsistent",
        ) if operation == RELEASE_OPERATION => "release_reserved_quantity",
        (
            "inventory.available_quantity_overflow",
            "inventory available quantity is outside the supported range",
        ) => "calculate_available_quantity",
        ("inventory.database_unavailable", "inventory storage is temporarily unavailable") => {
            "owner_storage"
        }
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
            reservation_id = %identity.reservation_id,
            variant_id = ?identity.variant_id,
            request_quantity = ?identity.quantity,
            line_item_id = ?identity.line_item_id,
            external_id_length = identity.external_id_length,
            internal_code = %error.code,
            internal_message = %error.message,
            error_kind = ?error.kind,
            retryable = error.retryable,
            boundary = INVENTORY_RESERVATION_BOUNDARY,
            "durable inventory reservation local technical outcome retained delegated context"
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
            reservation_id = %identity.reservation_id,
            variant_id = ?identity.variant_id,
            request_quantity = ?identity.quantity,
            line_item_id = ?identity.line_item_id,
            external_id_length = identity.external_id_length,
            internal_code = %error.code,
            internal_message = %error.message,
            error_kind = ?error.kind,
            retryable = error.retryable,
            boundary = INVENTORY_RESERVATION_BOUNDARY,
            "durable inventory reservation local outcome retained delegated context"
        );
    }
    error
}

fn require_inventory_reservation_write_admission(
    context: &PortContext,
    operation: &'static str,
) -> Result<(), PortError> {
    context
        .require_policy(PortCallPolicy::write())
        .inspect_err(|error| {
            log_inventory_reservation_admission_rejection(context, operation, "policy", error);
        })?;
    context.require_write_semantics().inspect_err(|error| {
        log_inventory_reservation_admission_rejection(context, operation, "write_semantics", error);
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
                "inventory reservation owner admission failed"
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
                "inventory reservation owner admission was rejected"
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
            "inventory reservation owner context validation was rejected"
        );
        error
    })
}
