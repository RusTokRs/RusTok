use std::sync::Arc;

use async_trait::async_trait;
use rustok_api::{PortCallPolicy, PortContext, PortError, PortErrorKind};
use sea_orm::DatabaseConnection;
use uuid::Uuid;

use crate::ports::{
    InventoryIdentityReservationReleaseRequest, InventoryIdentityReservationReleaseSnapshot,
    InventoryIdentityReservationRequest, InventoryIdentityReservationSnapshot,
    InventoryReservationIdentityPort,
};

const INVENTORY_OWNER: &str = "rustok_inventory";
const INVENTORY_RESERVATION_BOUNDARY: &str = "inventory_reservation_identity_port";
const RESERVE_OPERATION: &str = "reserve_inventory_by_identity";
const RELEASE_OPERATION: &str = "release_inventory_by_identity";

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

struct InventoryReservationIdentityDiagnostic {
    reservation_id_present: bool,
    reservation_id_non_nil: bool,
    variant_id_present: bool,
    variant_id_non_nil: bool,
    quantity_present: bool,
    quantity_nonzero: bool,
    quantity_negative: bool,
    line_item_id_present: bool,
    line_item_id_non_nil: bool,
    external_id_length: usize,
}

impl InventoryReservationIdentityDiagnostic {
    fn reserve(request: &InventoryIdentityReservationRequest) -> Self {
        Self {
            reservation_id_present: true,
            reservation_id_non_nil: !request.reservation_id.is_nil(),
            variant_id_present: true,
            variant_id_non_nil: !request.variant_id.is_nil(),
            quantity_present: true,
            quantity_nonzero: request.quantity != 0,
            quantity_negative: request.quantity < 0,
            line_item_id_present: request.line_item_id.is_some(),
            line_item_id_non_nil: request.line_item_id.is_some_and(|value| !value.is_nil()),
            external_id_length: request.external_id.chars().count(),
        }
    }

    fn release(request: &InventoryIdentityReservationReleaseRequest) -> Self {
        Self {
            reservation_id_present: true,
            reservation_id_non_nil: !request.reservation_id.is_nil(),
            variant_id_present: false,
            variant_id_non_nil: false,
            quantity_present: false,
            quantity_nonzero: false,
            quantity_negative: false,
            line_item_id_present: false,
            line_item_id_non_nil: false,
            external_id_length: request.external_id.chars().count(),
        }
    }
}

#[derive(Clone)]
pub struct PersistentInventoryReservationIdentityPort {
    inner: Arc<dyn InventoryReservationIdentityPort>,
}

impl PersistentInventoryReservationIdentityPort {
    pub fn new(db: DatabaseConnection) -> Self {
        Self {
            inner: Arc::new(crate::ports::PersistentInventoryReservationIdentityPort::new(db)),
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
        let identity = InventoryReservationIdentityDiagnostic::reserve(&request);
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
        let identity = InventoryReservationIdentityDiagnostic::release(&request);
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

fn inventory_reservation_port_error_kind(kind: &PortErrorKind) -> &'static str {
    match kind {
        PortErrorKind::Validation => "validation",
        PortErrorKind::NotFound => "not_found",
        PortErrorKind::Conflict => "conflict",
        PortErrorKind::Forbidden => "forbidden",
        PortErrorKind::Unavailable => "unavailable",
        PortErrorKind::Timeout => "timeout",
        PortErrorKind::InvariantViolation => "invariant_violation",
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

    log_inventory_reservation_local_outcome(context, operation, local_operation, identity, &error);
    error
}

fn log_inventory_reservation_local_outcome(
    context: &PortContext,
    operation: &'static str,
    local_operation: &'static str,
    identity: &InventoryReservationIdentityDiagnostic,
    error: &PortError,
) {
    let facts = inventory_reservation_context_facts(context);
    let technical_failure = matches!(
        &error.kind,
        PortErrorKind::Unavailable | PortErrorKind::Timeout | PortErrorKind::InvariantViolation
    );

    if technical_failure {
        tracing::error!(
            owner = INVENTORY_OWNER,
            operation,
            local_operation,
            correlation_id = %context.correlation_id,
            tenant_id_length = facts.tenant_id_length,
            actor_kind = facts.actor_kind,
            actor_id_length = facts.actor_id_length,
            claim_count = facts.claim_count,
            role_count = facts.role_count,
            channel_present = facts.channel_present,
            channel_length = ?facts.channel_length,
            locale_length = facts.locale_length,
            causation_id_present = facts.causation_id_present,
            causation_id_length = ?facts.causation_id_length,
            traceparent_present = facts.traceparent_present,
            traceparent_length = ?facts.traceparent_length,
            idempotency_key_present = facts.idempotency_key_present,
            idempotency_key_length = ?facts.idempotency_key_length,
            deadline_ms = ?facts.deadline_ms,
            reservation_id_present = identity.reservation_id_present,
            reservation_id_non_nil = identity.reservation_id_non_nil,
            variant_id_present = identity.variant_id_present,
            variant_id_non_nil = identity.variant_id_non_nil,
            quantity_present = identity.quantity_present,
            quantity_nonzero = identity.quantity_nonzero,
            quantity_negative = identity.quantity_negative,
            line_item_id_present = identity.line_item_id_present,
            line_item_id_non_nil = identity.line_item_id_non_nil,
            external_id_length = identity.external_id_length,
            code = %error.code,
            error_message_present = !error.message.is_empty(),
            error_message_length = error.message.chars().count(),
            error_kind = inventory_reservation_port_error_kind(&error.kind),
            retryable = error.retryable,
            boundary = INVENTORY_RESERVATION_BOUNDARY,
            "durable inventory reservation local technical outcome retained bounded delegated context"
        );
    } else {
        tracing::warn!(
            owner = INVENTORY_OWNER,
            operation,
            local_operation,
            correlation_id = %context.correlation_id,
            tenant_id_length = facts.tenant_id_length,
            actor_kind = facts.actor_kind,
            actor_id_length = facts.actor_id_length,
            claim_count = facts.claim_count,
            role_count = facts.role_count,
            channel_present = facts.channel_present,
            channel_length = ?facts.channel_length,
            locale_length = facts.locale_length,
            causation_id_present = facts.causation_id_present,
            causation_id_length = ?facts.causation_id_length,
            traceparent_present = facts.traceparent_present,
            traceparent_length = ?facts.traceparent_length,
            idempotency_key_present = facts.idempotency_key_present,
            idempotency_key_length = ?facts.idempotency_key_length,
            deadline_ms = ?facts.deadline_ms,
            reservation_id_present = identity.reservation_id_present,
            reservation_id_non_nil = identity.reservation_id_non_nil,
            variant_id_present = identity.variant_id_present,
            variant_id_non_nil = identity.variant_id_non_nil,
            quantity_present = identity.quantity_present,
            quantity_nonzero = identity.quantity_nonzero,
            quantity_negative = identity.quantity_negative,
            line_item_id_present = identity.line_item_id_present,
            line_item_id_non_nil = identity.line_item_id_non_nil,
            external_id_length = identity.external_id_length,
            code = %error.code,
            error_message_present = !error.message.is_empty(),
            error_message_length = error.message.chars().count(),
            error_kind = inventory_reservation_port_error_kind(&error.kind),
            retryable = error.retryable,
            boundary = INVENTORY_RESERVATION_BOUNDARY,
            "durable inventory reservation local outcome retained bounded delegated context"
        );
    }
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
    let facts = inventory_reservation_context_facts(context);
    let technical_failure = matches!(
        &error.kind,
        PortErrorKind::Unavailable | PortErrorKind::Timeout | PortErrorKind::InvariantViolation
    );

    if technical_failure {
        tracing::error!(
            owner = INVENTORY_OWNER,
            operation,
            admission_phase,
            correlation_id = %context.correlation_id,
            tenant_id_length = facts.tenant_id_length,
            actor_kind = facts.actor_kind,
            actor_id_length = facts.actor_id_length,
            claim_count = facts.claim_count,
            role_count = facts.role_count,
            channel_present = facts.channel_present,
            channel_length = ?facts.channel_length,
            locale_length = facts.locale_length,
            causation_id_present = facts.causation_id_present,
            causation_id_length = ?facts.causation_id_length,
            traceparent_present = facts.traceparent_present,
            traceparent_length = ?facts.traceparent_length,
            idempotency_key_present = facts.idempotency_key_present,
            idempotency_key_length = ?facts.idempotency_key_length,
            deadline_ms = ?facts.deadline_ms,
            code = %error.code,
            error_message_present = !error.message.is_empty(),
            error_message_length = error.message.chars().count(),
            error_kind = inventory_reservation_port_error_kind(&error.kind),
            retryable = error.retryable,
            boundary = INVENTORY_RESERVATION_BOUNDARY,
            "inventory reservation owner admission failed with bounded diagnostics"
        );
    } else {
        tracing::warn!(
            owner = INVENTORY_OWNER,
            operation,
            admission_phase,
            correlation_id = %context.correlation_id,
            tenant_id_length = facts.tenant_id_length,
            actor_kind = facts.actor_kind,
            actor_id_length = facts.actor_id_length,
            claim_count = facts.claim_count,
            role_count = facts.role_count,
            channel_present = facts.channel_present,
            channel_length = ?facts.channel_length,
            locale_length = facts.locale_length,
            causation_id_present = facts.causation_id_present,
            causation_id_length = ?facts.causation_id_length,
            traceparent_present = facts.traceparent_present,
            traceparent_length = ?facts.traceparent_length,
            idempotency_key_present = facts.idempotency_key_present,
            idempotency_key_length = ?facts.idempotency_key_length,
            deadline_ms = ?facts.deadline_ms,
            code = %error.code,
            error_message_present = !error.message.is_empty(),
            error_message_length = error.message.chars().count(),
            error_kind = inventory_reservation_port_error_kind(&error.kind),
            retryable = error.retryable,
            boundary = INVENTORY_RESERVATION_BOUNDARY,
            "inventory reservation owner admission was rejected with bounded diagnostics"
        );
    }
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
        log_inventory_reservation_tenant_rejection(context, operation, &error);
        error
    })
}

fn log_inventory_reservation_tenant_rejection(
    context: &PortContext,
    operation: &'static str,
    error: &PortError,
) {
    let facts = inventory_reservation_context_facts(context);
    tracing::warn!(
        owner = INVENTORY_OWNER,
        operation,
        validation_phase = "tenant_id",
        correlation_id = %context.correlation_id,
        tenant_id_length = facts.tenant_id_length,
        actor_kind = facts.actor_kind,
        actor_id_length = facts.actor_id_length,
        claim_count = facts.claim_count,
        role_count = facts.role_count,
        channel_present = facts.channel_present,
        channel_length = ?facts.channel_length,
        locale_length = facts.locale_length,
        causation_id_present = facts.causation_id_present,
        causation_id_length = ?facts.causation_id_length,
        traceparent_present = facts.traceparent_present,
        traceparent_length = ?facts.traceparent_length,
        idempotency_key_present = facts.idempotency_key_present,
        idempotency_key_length = ?facts.idempotency_key_length,
        deadline_ms = ?facts.deadline_ms,
        tenant_id_parse_failed = true,
        code = %error.code,
        error_message_present = !error.message.is_empty(),
        error_message_length = error.message.chars().count(),
        error_kind = inventory_reservation_port_error_kind(&error.kind),
        retryable = error.retryable,
        boundary = INVENTORY_RESERVATION_BOUNDARY,
        "inventory reservation owner context validation was rejected with bounded diagnostics"
    );
}
