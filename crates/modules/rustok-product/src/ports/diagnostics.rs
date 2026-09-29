//! Port-boundary diagnostics: structured logging, error mapping, context
//! validation and telemetry guards for product catalog owner operations.
//!
//! All helpers are `pub(super)` so they stay internal to `ports/` while being
//! shared by port submodules without exposing telemetry internals.

use rustok_api::{PortContext, PortError, PortErrorKind};
use uuid::Uuid;

// ── Context fact extraction ─────────────────────────────────────────

pub(super) struct ProductPortContextFacts {
    pub correlation_id_length: usize,
    pub tenant_id_length: usize,
    pub actor_kind: &'static str,
    pub actor_id_length: usize,
    pub claim_count: usize,
    pub role_count: usize,
    pub channel_present: bool,
    pub channel_length: Option<usize>,
    pub locale_length: usize,
    pub causation_id_present: bool,
    pub causation_id_length: Option<usize>,
    pub traceparent_present: bool,
    pub traceparent_length: Option<usize>,
    pub idempotency_key_present: bool,
    pub idempotency_key_length: Option<usize>,
    pub deadline_ms: Option<u64>,
}

pub(super) fn product_port_context_facts(context: &PortContext) -> ProductPortContextFacts {
    let actor_kind = match &context.actor.kind {
        rustok_api::PortActorKind::User => "user",
        rustok_api::PortActorKind::Service => "service",
        rustok_api::PortActorKind::System => "system",
    };
    ProductPortContextFacts {
        correlation_id_length: context.correlation_id.chars().count(),
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

// ── Owner error fact extraction ─────────────────────────────────────

pub(super) struct ProductOwnerErrorFacts {
    pub error_variant: &'static str,
    pub text_field_count: usize,
    pub text_total_length: usize,
    pub uuid_field_count: usize,
    pub uuid_non_nil_count: usize,
    pub opaque_payload_present: bool,
}

impl ProductOwnerErrorFacts {
    fn empty(error_variant: &'static str) -> Self {
        Self {
            error_variant,
            text_field_count: 0,
            text_total_length: 0,
            uuid_field_count: 0,
            uuid_non_nil_count: 0,
            opaque_payload_present: false,
        }
    }

    fn text(error_variant: &'static str, values: &[&str]) -> Self {
        Self {
            text_field_count: values.len(),
            text_total_length: values.iter().map(|value| value.chars().count()).sum(),
            ..Self::empty(error_variant)
        }
    }

    fn uuids(error_variant: &'static str, values: &[Uuid]) -> Self {
        Self {
            uuid_field_count: values.len(),
            uuid_non_nil_count: values.iter().filter(|value| !value.is_nil()).count(),
            ..Self::empty(error_variant)
        }
    }

    fn opaque(error_variant: &'static str) -> Self {
        Self {
            opaque_payload_present: true,
            ..Self::empty(error_variant)
        }
    }
}

pub(super) fn product_owner_error_facts(
    error: &crate::error::CommerceError,
) -> ProductOwnerErrorFacts {
    use crate::error::CommerceError;

    match error {
        CommerceError::Database(_) => ProductOwnerErrorFacts::opaque("database"),
        CommerceError::ProductNotFound(value) => {
            ProductOwnerErrorFacts::uuids("product_not_found", &[*value])
        }
        CommerceError::DuplicateHandle { handle, locale } => {
            ProductOwnerErrorFacts::text("duplicate_handle", &[handle.as_str(), locale.as_str()])
        }
        CommerceError::DuplicateSku(value) => {
            ProductOwnerErrorFacts::text("duplicate_sku", &[value.as_str()])
        }
        CommerceError::Validation(value) => {
            ProductOwnerErrorFacts::text("validation", &[value.as_str()])
        }
        CommerceError::NoVariants => ProductOwnerErrorFacts::empty("no_variants"),
        CommerceError::VariantNotFound(value) => {
            ProductOwnerErrorFacts::uuids("variant_not_found", &[*value])
        }
        CommerceError::ImageNotFound(value) => {
            ProductOwnerErrorFacts::uuids("image_not_found", &[*value])
        }
        CommerceError::CannotDeleteOnlyVariant => {
            ProductOwnerErrorFacts::empty("cannot_delete_only_variant")
        }
        CommerceError::CannotDeletePublished => {
            ProductOwnerErrorFacts::empty("cannot_delete_published")
        }
        CommerceError::Core(_) => ProductOwnerErrorFacts::opaque("core"),
    }
}

// ── Structured logging ──────────────────────────────────────────────

pub(super) fn log_product_port_failure(
    context: &PortContext,
    owner_operation: &'static str,
    code: &'static str,
    error_facts: &ProductOwnerErrorFacts,
    technical_failure: bool,
) {
    let context_facts = product_port_context_facts(context);
    if technical_failure {
        tracing::error!(
            owner = "rustok_product",
            correlation_id = %context.correlation_id,
            correlation_id_length = context_facts.correlation_id_length,
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
            operation = owner_operation,
            code,
            error_variant = error_facts.error_variant,
            text_field_count = error_facts.text_field_count,
            text_total_length = error_facts.text_total_length,
            uuid_field_count = error_facts.uuid_field_count,
            uuid_non_nil_count = error_facts.uuid_non_nil_count,
            opaque_payload_present = error_facts.opaque_payload_present,
            boundary = "product_catalog_read_port",
            "product catalog owner operation failed with bounded diagnostics"
        );
    } else {
        tracing::warn!(
            owner = "rustok_product",
            correlation_id = %context.correlation_id,
            correlation_id_length = context_facts.correlation_id_length,
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
            operation = owner_operation,
            code,
            error_variant = error_facts.error_variant,
            text_field_count = error_facts.text_field_count,
            text_total_length = error_facts.text_total_length,
            uuid_field_count = error_facts.uuid_field_count,
            uuid_non_nil_count = error_facts.uuid_non_nil_count,
            opaque_payload_present = error_facts.opaque_payload_present,
            boundary = "product_catalog_read_port",
            "product catalog owner operation was rejected with bounded diagnostics"
        );
    }
}

fn log_product_context_rejection(
    context: &PortContext,
    operation: &'static str,
    code: &'static str,
    parse_target: &'static str,
) {
    let context_facts = product_port_context_facts(context);
    tracing::warn!(
        owner = "rustok_product",
        correlation_id = %context.correlation_id,
        correlation_id_length = context_facts.correlation_id_length,
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
        operation,
        code,
        parse_target,
        parse_failed = true,
        boundary = "product_catalog_read_port",
        "product catalog port context was rejected with bounded diagnostics"
    );
}

// ── Context/tenant parsing ──────────────────────────────────────────

pub(super) fn parse_port_tenant_id(
    context: &PortContext,
    owner_operation: &'static str,
) -> Result<Uuid, PortError> {
    Uuid::parse_str(&context.tenant_id).map_err(|_| {
        log_product_context_rejection(
            context,
            owner_operation,
            "product.tenant_id_invalid",
            "tenant_id",
        );
        PortError::validation(
            "product.tenant_id_invalid",
            "product request context is invalid",
        )
    })
}

pub(super) fn product_context_error(
    context: &PortContext,
    owner_operation: &'static str,
    error: PortError,
) -> PortError {
    let error_kind = product_port_error_kind(&error.kind);
    let error_code_length = error.code.chars().count();
    let error_message_length = error.message.chars().count();
    let context_facts = product_port_context_facts(context);
    tracing::warn!(
        owner = "rustok_product",
        correlation_id = %context.correlation_id,
        correlation_id_length = context_facts.correlation_id_length,
        tenant_id_length = context_facts.tenant_id_length,
        operation = owner_operation,
        code = "product.context_invalid",
        error_kind,
        error_code_length,
        error_message_present = !error.message.trim().is_empty(),
        error_message_length,
        retryable = error.retryable,
        boundary = "product_catalog_read_port",
        "product catalog call context was rejected"
    );

    let PortError {
        kind,
        code,
        retryable,
        ..
    } = error;
    match kind {
        PortErrorKind::Timeout => PortError::timeout(code, "product request context is invalid"),
        PortErrorKind::Validation => {
            PortError::validation(code, "product request context is invalid")
        }
        kind => PortError::new(
            kind,
            "product.context_invalid",
            "product request context is invalid",
            retryable,
        ),
    }
}

// ── Storage/variant error helpers ───────────────────────────────────

pub(super) fn product_storage_error(
    context: &PortContext,
    owner_operation: &'static str,
    _error: sea_orm::DbErr,
) -> PortError {
    let context_facts = product_port_context_facts(context);
    tracing::error!(
        owner = "rustok_product",
        correlation_id = %context.correlation_id,
        correlation_id_length = context_facts.correlation_id_length,
        tenant_id_length = context_facts.tenant_id_length,
        actor_kind = context_facts.actor_kind,
        actor_id_length = context_facts.actor_id_length,
        operation = owner_operation,
        code = "product.database_unavailable",
        error_variant = "database",
        boundary = "product_catalog_read_port",
        "product catalog storage failed with bounded diagnostics"
    );
    PortError::unavailable(
        "product.database_unavailable",
        "product storage is temporarily unavailable",
    )
}

pub(super) fn product_variant_not_found(
    context: &PortContext,
    owner_operation: &'static str,
    variant_id: Uuid,
) -> PortError {
    let context_facts = product_port_context_facts(context);
    tracing::warn!(
        owner = "rustok_product",
        correlation_id = %context.correlation_id,
        correlation_id_length = context_facts.correlation_id_length,
        tenant_id_length = context_facts.tenant_id_length,
        operation = owner_operation,
        variant_id_non_nil = !variant_id.is_nil(),
        code = "product.variant_not_found",
        boundary = "product_catalog_read_port",
        "product variant projection was not found"
    );
    PortError::not_found("product.variant_not_found", "product variant was not found")
}

// ── Domain error → port error mapping ───────────────────────────────

fn product_port_error_kind(kind: &PortErrorKind) -> &'static str {
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

fn product_error_code(error: &crate::error::CommerceError) -> &'static str {
    use crate::error::CommerceError;

    match error {
        CommerceError::Database(_) => "product.database_unavailable",
        CommerceError::ProductNotFound(_) => "product.product_not_found",
        CommerceError::VariantNotFound(_) => "product.variant_not_found",
        CommerceError::ImageNotFound(_) => "product.image_not_found",
        CommerceError::CannotDeleteOnlyVariant => "product.cannot_delete_only_variant",
        CommerceError::DuplicateHandle { .. } => "product.duplicate_handle",
        CommerceError::DuplicateSku(_) => "product.duplicate_sku",
        CommerceError::Validation(_) => "product.validation",
        CommerceError::NoVariants => "product.no_variants",
        CommerceError::CannotDeletePublished => "product.cannot_delete_published",
        CommerceError::Core(_) => "product.invariant_violation",
    }
}

pub(super) fn product_error_to_port_error(
    context: &PortContext,
    owner_operation: &'static str,
    error: crate::error::CommerceError,
) -> PortError {
    use crate::error::CommerceError;

    let code = product_error_code(&error);
    let error_facts = product_owner_error_facts(&error);
    let technical_failure = matches!(&error, CommerceError::Database(_) | CommerceError::Core(_));
    log_product_port_failure(
        context,
        owner_operation,
        code,
        &error_facts,
        technical_failure,
    );

    match error {
        CommerceError::Database(_) => PortError::unavailable(
            "product.database_unavailable",
            "product storage is temporarily unavailable",
        ),
        CommerceError::ProductNotFound(_) => {
            PortError::not_found("product.product_not_found", "product was not found")
        }
        CommerceError::VariantNotFound(_) => {
            PortError::not_found("product.variant_not_found", "product variant was not found")
        }
        CommerceError::ImageNotFound(_) => {
            PortError::not_found("product.image_not_found", "product image was not found")
        }
        CommerceError::CannotDeleteOnlyVariant => PortError::conflict(
            "product.cannot_delete_only_variant",
            "cannot delete the only variant of a product",
        ),
        CommerceError::DuplicateHandle { .. } => PortError::conflict(
            "product.duplicate_handle",
            "product handle conflicts with an existing product",
        ),
        CommerceError::DuplicateSku(_) => PortError::conflict(
            "product.duplicate_sku",
            "product SKU conflicts with an existing product",
        ),
        CommerceError::Validation(_) => {
            PortError::validation("product.validation", "product request is invalid")
        }
        CommerceError::NoVariants => PortError::conflict(
            "product.no_variants",
            "product must have at least one variant",
        ),
        CommerceError::CannotDeletePublished => PortError::conflict(
            "product.cannot_delete_published",
            "cannot delete a published product",
        ),
        CommerceError::Core(_) => PortError::invariant_violation(
            "product.invariant_violation",
            "product operation could not be completed safely",
        ),
    }
}
