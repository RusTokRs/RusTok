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
        ("inventory.database_unavailable", "inventory storage is temporarily unavailable") => {
            "owner_storage"
        }
        ("inventory.invariant_violation", "inventory operation violated an owner invariant") => {
            "owner_invariant"
        }
        _ => return error,
    };
    let technical_failure = inventory_reservation_error_is_technical(&error);
    log_inventory_reservation_local_outcome(
        context,
        operation,
        local_operation,
        variant_id,
        quantity,
        &error,
        technical_failure,
    );
    error
}

#[derive(Clone, Copy, Debug)]
struct InventoryReservationContextFacts {
    tenant_id_length: usize,
    actor_kind: &'static str,
    actor_id_length: usize,
    claim_count: usize,
    role_count: usize,
    channel_present: bool,
    channel_length: Option<usize>,
    locale_length: usize,
    causation_id_present: bool,
    causation_id_length: Option<usize>,
    traceparent_present: bool,
    traceparent_length: Option<usize>,
    idempotency_key_present: bool,
    idempotency_key_length: Option<usize>,
    deadline_ms: Option<u64>,
}

fn inventory_reservation_context_facts(context: &PortContext) -> InventoryReservationContextFacts {
    let actor_kind = match &context.actor.kind {
        rustok_api::PortActorKind::User => "user",
        rustok_api::PortActorKind::Service => "service",
        rustok_api::PortActorKind::System => "system",
    };
    InventoryReservationContextFacts {
        tenant_id_length: context.tenant_id.chars().count(),
        actor_kind,
        actor_id_length: context.actor.id.chars().count(),
        claim_count: context.claims.len(),
        role_count: context.roles.len(),
        channel_present: context.channel.is_some(),
        channel_length: context.channel.as_ref().map(|value| value.chars().count()),
        locale_length: context.locale.chars().count(),
        causation_id_present: context.causation_id.is_some(),
        causation_id_length: context
            .causation_id
            .as_ref()
            .map(|value| value.chars().count()),
        traceparent_present: context.traceparent.is_some(),
        traceparent_length: context
            .traceparent
            .as_ref()
            .map(|value| value.chars().count()),
        idempotency_key_present: context.idempotency_key.is_some(),
        idempotency_key_length: context
            .idempotency_key
            .as_ref()
            .map(|value| value.chars().count()),
        deadline_ms: context.deadline_ms,
    }
}

fn inventory_reservation_error_is_technical(error: &PortError) -> bool {
    matches!(
        &error.kind,
        PortErrorKind::Unavailable | PortErrorKind::Timeout | PortErrorKind::InvariantViolation
    )
}

#[allow(clippy::too_many_arguments)]
fn log_inventory_reservation_local_outcome(
    context: &PortContext,
    operation: &'static str,
    local_operation: &'static str,
    variant_id: Uuid,
    quantity: i32,
    error: &PortError,
    technical_failure: bool,
) {
    let context_facts = inventory_reservation_context_facts(context);
    if technical_failure {
        tracing::error!(
            owner = INVENTORY_OWNER,
            operation,
            local_operation,
            correlation_id = %context.correlation_id,
            tenant_id_length = context_facts.tenant_id_length,
            actor_kind = context_facts.actor_kind,
            actor_id_length = context_facts.actor_id_length,
            claim_count = context_facts.claim_count,
            role_count = context_facts.role_count,
            channel_present = context_facts.channel_present,
            channel_length = ?context_facts.channel_length,
            locale_length = context_facts.locale_length,
            causation_id_present = context_facts.causation_id_present,
            causation_id_length = ?context_facts.causation_id_length,
            traceparent_present = context_facts.traceparent_present,
            traceparent_length = ?context_facts.traceparent_length,
            idempotency_key_present = context_facts.idempotency_key_present,
            idempotency_key_length = ?context_facts.idempotency_key_length,
            deadline_ms = ?context_facts.deadline_ms,
            variant_id_non_nil = !variant_id.is_nil(),
            request_quantity_zero = quantity == 0,
            request_quantity_negative = quantity < 0,
            code = error.code.as_str(),
            error_message_length = error.message.chars().count(),
            retryable = error.retryable,
            technical_failure,
            boundary = INVENTORY_RESERVATION_BOUNDARY,
            "inventory availability or quantity reservation local technical outcome retained bounded diagnostics"
        );
    } else {
        tracing::warn!(
            owner = INVENTORY_OWNER,
            operation,
            local_operation,
            correlation_id = %context.correlation_id,
            tenant_id_length = context_facts.tenant_id_length,
            actor_kind = context_facts.actor_kind,
            actor_id_length = context_facts.actor_id_length,
            claim_count = context_facts.claim_count,
            role_count = context_facts.role_count,
            channel_present = context_facts.channel_present,
            channel_length = ?context_facts.channel_length,
            locale_length = context_facts.locale_length,
            causation_id_present = context_facts.causation_id_present,
            causation_id_length = ?context_facts.causation_id_length,
            traceparent_present = context_facts.traceparent_present,
            traceparent_length = ?context_facts.traceparent_length,
            idempotency_key_present = context_facts.idempotency_key_present,
            idempotency_key_length = ?context_facts.idempotency_key_length,
            deadline_ms = ?context_facts.deadline_ms,
            variant_id_non_nil = !variant_id.is_nil(),
            request_quantity_zero = quantity == 0,
            request_quantity_negative = quantity < 0,
            code = error.code.as_str(),
            error_message_length = error.message.chars().count(),
            retryable = error.retryable,
            technical_failure,
            boundary = INVENTORY_RESERVATION_BOUNDARY,
            "inventory availability or quantity reservation local outcome retained bounded diagnostics"
        );
    }
}

fn require_inventory_reservation_read_admission(
    context: &PortContext,
    operation: &'static str,
) -> Result<(), PortError> {
    context
        .require_policy(PortCallPolicy::read())
        .inspect_err(|error| {
            log_inventory_reservation_admission_rejection(context, operation, "policy", error);
        })
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
    let technical_failure = inventory_reservation_error_is_technical(error);
    let context_facts = inventory_reservation_context_facts(context);
    if technical_failure {
        tracing::error!(
            owner = INVENTORY_OWNER,
            operation,
            admission_phase,
            correlation_id = %context.correlation_id,
            tenant_id_length = context_facts.tenant_id_length,
            actor_kind = context_facts.actor_kind,
            actor_id_length = context_facts.actor_id_length,
            claim_count = context_facts.claim_count,
            role_count = context_facts.role_count,
            channel_present = context_facts.channel_present,
            channel_length = ?context_facts.channel_length,
            locale_length = context_facts.locale_length,
            causation_id_present = context_facts.causation_id_present,
            causation_id_length = ?context_facts.causation_id_length,
            traceparent_present = context_facts.traceparent_present,
            traceparent_length = ?context_facts.traceparent_length,
            idempotency_key_present = context_facts.idempotency_key_present,
            idempotency_key_length = ?context_facts.idempotency_key_length,
            deadline_ms = ?context_facts.deadline_ms,
            code = error.code.as_str(),
            error_message_length = error.message.chars().count(),
            retryable = error.retryable,
            technical_failure,
            boundary = INVENTORY_RESERVATION_BOUNDARY,
            "inventory availability or quantity reservation owner admission failed with bounded diagnostics"
        );
    } else {
        tracing::warn!(
            owner = INVENTORY_OWNER,
            operation,
            admission_phase,
            correlation_id = %context.correlation_id,
            tenant_id_length = context_facts.tenant_id_length,
            actor_kind = context_facts.actor_kind,
            actor_id_length = context_facts.actor_id_length,
            claim_count = context_facts.claim_count,
            role_count = context_facts.role_count,
            channel_present = context_facts.channel_present,
            channel_length = ?context_facts.channel_length,
            locale_length = context_facts.locale_length,
            causation_id_present = context_facts.causation_id_present,
            causation_id_length = ?context_facts.causation_id_length,
            traceparent_present = context_facts.traceparent_present,
            traceparent_length = ?context_facts.traceparent_length,
            idempotency_key_present = context_facts.idempotency_key_present,
            idempotency_key_length = ?context_facts.idempotency_key_length,
            deadline_ms = ?context_facts.deadline_ms,
            code = error.code.as_str(),
            error_message_length = error.message.chars().count(),
            retryable = error.retryable,
            technical_failure,
            boundary = INVENTORY_RESERVATION_BOUNDARY,
            "inventory availability or quantity reservation owner admission was rejected with bounded diagnostics"
        );
    }
}

fn log_inventory_reservation_tenant_parse_rejection(
    context: &PortContext,
    operation: &'static str,
    error: &PortError,
) {
    let context_facts = inventory_reservation_context_facts(context);
    tracing::warn!(
        owner = INVENTORY_OWNER,
        operation,
        validation_phase = "tenant_id",
        correlation_id = %context.correlation_id,
        tenant_id_length = context_facts.tenant_id_length,
        tenant_id_trimmed_length = context.tenant_id.trim().chars().count(),
        tenant_id_parse_failed = true,
        actor_kind = context_facts.actor_kind,
        actor_id_length = context_facts.actor_id_length,
        claim_count = context_facts.claim_count,
        role_count = context_facts.role_count,
        channel_present = context_facts.channel_present,
        channel_length = ?context_facts.channel_length,
        locale_length = context_facts.locale_length,
        causation_id_present = context_facts.causation_id_present,
        causation_id_length = ?context_facts.causation_id_length,
        traceparent_present = context_facts.traceparent_present,
        traceparent_length = ?context_facts.traceparent_length,
        idempotency_key_present = context_facts.idempotency_key_present,
        idempotency_key_length = ?context_facts.idempotency_key_length,
        deadline_ms = ?context_facts.deadline_ms,
        code = error.code.as_str(),
        error_message_length = error.message.chars().count(),
        retryable = error.retryable,
        boundary = INVENTORY_RESERVATION_BOUNDARY,
        "inventory availability or quantity reservation owner context validation was rejected with bounded diagnostics"
    );
}

fn parse_inventory_reservation_tenant_id(
    context: &PortContext,
    operation: &'static str,
) -> Result<Uuid, PortError> {
    Uuid::parse_str(context.tenant_id.trim()).map_err(|_| {
        let error = PortError::validation(
            "inventory.context_invalid",
            "inventory request context is invalid",
        );
        log_inventory_reservation_tenant_parse_rejection(context, operation, &error);
        error
    })
}
