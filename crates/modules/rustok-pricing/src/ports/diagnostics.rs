use rustok_api::{PortContext, PortError, PortErrorKind};
use rustok_commerce_foundation::error::CommerceError;
use uuid::Uuid;

pub(crate) const PRICING_OWNER: &str = "rustok_pricing";
pub(crate) const PRICING_PORT_BOUNDARY: &str = "pricing_owner_port";

pub(crate) struct PricingPortContextFacts {
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

pub(crate) struct PricingOwnerErrorFacts {
    pub error_variant: &'static str,
    pub text_field_count: usize,
    pub text_total_length: usize,
    pub uuid_field_count: usize,
    pub uuid_non_nil_count: usize,
    pub numeric_field_count: usize,
    pub numeric_nonzero_count: usize,
    pub numeric_negative_count: usize,
    pub opaque_payload_present: bool,
}

impl PricingOwnerErrorFacts {
    pub fn empty(error_variant: &'static str) -> Self {
        Self {
            error_variant,
            text_field_count: 0,
            text_total_length: 0,
            uuid_field_count: 0,
            uuid_non_nil_count: 0,
            numeric_field_count: 0,
            numeric_nonzero_count: 0,
            numeric_negative_count: 0,
            opaque_payload_present: false,
        }
    }

    pub fn text(error_variant: &'static str, values: &[&str]) -> Self {
        Self {
            text_field_count: values.len(),
            text_total_length: values.iter().map(|value| value.chars().count()).sum(),
            ..Self::empty(error_variant)
        }
    }

    pub fn uuids(error_variant: &'static str, values: &[Uuid]) -> Self {
        Self {
            uuid_field_count: values.len(),
            uuid_non_nil_count: values.iter().filter(|value| !value.is_nil()).count(),
            ..Self::empty(error_variant)
        }
    }

    pub fn numbers(error_variant: &'static str, values: &[i32]) -> Self {
        Self {
            numeric_field_count: values.len(),
            numeric_nonzero_count: values.iter().filter(|value| **value != 0).count(),
            numeric_negative_count: values.iter().filter(|value| **value < 0).count(),
            ..Self::empty(error_variant)
        }
    }

    pub fn opaque(error_variant: &'static str) -> Self {
        Self {
            opaque_payload_present: true,
            ..Self::empty(error_variant)
        }
    }
}

pub(crate) fn pricing_port_context_facts(context: &PortContext) -> PricingPortContextFacts {
    let actor_kind = match &context.actor.kind {
        rustok_api::PortActorKind::User => "user",
        rustok_api::PortActorKind::Service => "service",
        rustok_api::PortActorKind::System => "system",
    };
    PricingPortContextFacts {
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

pub(crate) fn pricing_owner_error_facts(error: &CommerceError) -> PricingOwnerErrorFacts {
    match error {
        CommerceError::Database(_) => PricingOwnerErrorFacts::opaque("database"),
        CommerceError::ProductNotFound(value) => {
            PricingOwnerErrorFacts::uuids("product_not_found", &[*value])
        }
        CommerceError::VariantNotFound(value) => {
            PricingOwnerErrorFacts::uuids("variant_not_found", &[*value])
        }
        CommerceError::DuplicateHandle { handle, locale } => {
            PricingOwnerErrorFacts::text("duplicate_handle", &[handle.as_str(), locale.as_str()])
        }
        CommerceError::DuplicateSku(value) => {
            PricingOwnerErrorFacts::text("duplicate_sku", &[value.as_str()])
        }
        CommerceError::InvalidPrice(value) => {
            PricingOwnerErrorFacts::text("invalid_price", &[value.as_str()])
        }
        CommerceError::InsufficientInventory {
            requested,
            available,
        } => PricingOwnerErrorFacts::numbers("insufficient_inventory", &[*requested, *available]),
        CommerceError::InvalidOptionCombination => {
            PricingOwnerErrorFacts::empty("invalid_option_combination")
        }
        CommerceError::Validation(value) => {
            PricingOwnerErrorFacts::text("validation", &[value.as_str()])
        }
        CommerceError::ShippingProfileNotFound(value) => {
            PricingOwnerErrorFacts::uuids("shipping_profile_not_found", &[*value])
        }
        CommerceError::DuplicateShippingProfileSlug(value) => {
            PricingOwnerErrorFacts::text("duplicate_shipping_profile_slug", &[value.as_str()])
        }
        CommerceError::NoVariants => PricingOwnerErrorFacts::empty("no_variants"),
        CommerceError::CannotDeletePublished => {
            PricingOwnerErrorFacts::empty("cannot_delete_published")
        }
        CommerceError::Rich(_) => PricingOwnerErrorFacts::opaque("rich"),
        CommerceError::Core(_) => PricingOwnerErrorFacts::opaque("core"),
    }
}

pub(crate) fn log_pricing_port_entry(context: &PortContext, operation: &'static str) {
    let facts = pricing_port_context_facts(context);
    tracing::debug!(
        owner = PRICING_OWNER,
        boundary = PRICING_PORT_BOUNDARY,
        operation,
        tenant_id_length = facts.tenant_id_length,
        actor_kind = facts.actor_kind,
        claim_count = facts.claim_count,
        role_count = facts.role_count,
        correlation_present = !context.correlation_id.is_empty(),
        "pricing owner port entered"
    );
}

pub(crate) fn log_pricing_port_failure(
    context: &PortContext,
    operation: &'static str,
    code: &'static str,
    error_facts: &PricingOwnerErrorFacts,
    technical_failure: bool,
) {
    let context_facts = pricing_port_context_facts(context);
    if technical_failure {
        tracing::error!(
            owner = PRICING_OWNER,
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
            operation,
            code,
            error_variant = error_facts.error_variant,
            text_field_count = error_facts.text_field_count,
            text_total_length = error_facts.text_total_length,
            uuid_field_count = error_facts.uuid_field_count,
            uuid_non_nil_count = error_facts.uuid_non_nil_count,
            numeric_field_count = error_facts.numeric_field_count,
            numeric_nonzero_count = error_facts.numeric_nonzero_count,
            numeric_negative_count = error_facts.numeric_negative_count,
            opaque_payload_present = error_facts.opaque_payload_present,
            boundary = PRICING_PORT_BOUNDARY,
            "pricing owner operation failed with bounded diagnostics"
        );
    } else {
        tracing::warn!(
            owner = PRICING_OWNER,
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
            operation,
            code,
            error_variant = error_facts.error_variant,
            text_field_count = error_facts.text_field_count,
            text_total_length = error_facts.text_total_length,
            uuid_field_count = error_facts.uuid_field_count,
            uuid_non_nil_count = error_facts.uuid_non_nil_count,
            numeric_field_count = error_facts.numeric_field_count,
            numeric_nonzero_count = error_facts.numeric_nonzero_count,
            numeric_negative_count = error_facts.numeric_negative_count,
            opaque_payload_present = error_facts.opaque_payload_present,
            boundary = PRICING_PORT_BOUNDARY,
            "pricing owner operation was rejected with bounded diagnostics"
        );
    }
}

pub(crate) fn log_pricing_context_rejection(
    context: &PortContext,
    operation: &'static str,
    code: &'static str,
    parse_target: &'static str,
) {
    let context_facts = pricing_port_context_facts(context);
    tracing::warn!(
        owner = PRICING_OWNER,
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
        operation,
        code,
        parse_target,
        parse_failed = true,
        boundary = PRICING_PORT_BOUNDARY,
        "pricing port context was rejected with bounded diagnostics"
    );
}

pub(crate) fn validate_non_nil_uuid(id: Uuid, field: &'static str) -> Result<(), PortError> {
    if id.is_nil() {
        return Err(PortError::validation(
            format!("pricing.{field}_nil"),
            format!("{field} must not be a nil UUID"),
        ));
    }
    Ok(())
}

pub(crate) fn parse_port_tenant_id(
    context: &PortContext,
    operation: &'static str,
) -> Result<Uuid, PortError> {
    let tenant_id = Uuid::parse_str(&context.tenant_id).map_err(|_| {
        log_pricing_context_rejection(context, operation, "pricing.tenant_id_invalid", "tenant_id");
        PortError::validation(
            "pricing.tenant_id_invalid",
            "pricing request context is invalid",
        )
    })?;
    if tenant_id.is_nil() {
        log_pricing_context_rejection(context, operation, "pricing.tenant_id_nil", "tenant_id");
        return Err(PortError::validation(
            "pricing.tenant_id_nil",
            "tenant_id must not be a nil UUID",
        ));
    }
    Ok(tenant_id)
}

pub(crate) fn parse_port_actor_id(
    context: &PortContext,
    operation: &'static str,
) -> Result<Uuid, PortError> {
    let actor_id = Uuid::parse_str(context.actor.id.as_str()).map_err(|_| {
        log_pricing_context_rejection(context, operation, "pricing.actor_id_invalid", "actor_id");
        PortError::validation("pricing.actor_id_invalid", "pricing write actor is invalid")
    })?;
    if actor_id.is_nil() {
        log_pricing_context_rejection(context, operation, "pricing.actor_id_nil", "actor_id");
        return Err(PortError::validation(
            "pricing.actor_id_nil",
            "actor_id must not be a nil UUID",
        ));
    }
    Ok(actor_id)
}

pub(crate) struct PricingErrorPolicy {
    pub(crate) kind: PortErrorKind,
    pub(crate) code: &'static str,
    message: std::borrow::Cow<'static, str>,
    pub(crate) retryable: bool,
}

impl PricingErrorPolicy {
    fn new(
        kind: PortErrorKind,
        code: &'static str,
        message: impl Into<std::borrow::Cow<'static, str>>,
        retryable: bool,
    ) -> Self {
        Self {
            kind,
            code,
            message: message.into(),
            retryable,
        }
    }

    fn into_port_error(self) -> PortError {
        PortError::new(self.kind, self.code, self.message, self.retryable)
    }
}

pub(crate) fn pricing_error_policy(error: &CommerceError) -> PricingErrorPolicy {
    match error {
        CommerceError::Database(_) => PricingErrorPolicy::new(
            PortErrorKind::Unavailable,
            "pricing.database_unavailable",
            "pricing storage is temporarily unavailable",
            true,
        ),
        CommerceError::ProductNotFound(_) => PricingErrorPolicy::new(
            PortErrorKind::NotFound,
            "pricing.product_not_found",
            "product was not found",
            false,
        ),
        CommerceError::VariantNotFound(_) => PricingErrorPolicy::new(
            PortErrorKind::NotFound,
            "pricing.variant_not_found",
            "variant was not found",
            false,
        ),
        CommerceError::DuplicateHandle { .. } => PricingErrorPolicy::new(
            PortErrorKind::Conflict,
            "pricing.duplicate_handle",
            "pricing handle is already in use",
            false,
        ),
        CommerceError::DuplicateSku(_) => PricingErrorPolicy::new(
            PortErrorKind::Conflict,
            "pricing.duplicate_sku",
            "pricing SKU is already in use",
            false,
        ),
        CommerceError::InvalidPrice(msg) => PricingErrorPolicy::new(
            PortErrorKind::Validation,
            "pricing.validation",
            msg.clone(),
            false,
        ),
        CommerceError::Validation(msg) => PricingErrorPolicy::new(
            PortErrorKind::Validation,
            "pricing.validation",
            msg.clone(),
            false,
        ),
        CommerceError::InsufficientInventory { .. } => PricingErrorPolicy::new(
            PortErrorKind::Conflict,
            "pricing.insufficient_inventory",
            "inventory is insufficient for the pricing operation",
            false,
        ),
        CommerceError::InvalidOptionCombination => PricingErrorPolicy::new(
            PortErrorKind::Validation,
            "pricing.invalid_option_combination",
            "invalid option combination",
            false,
        ),
        CommerceError::ShippingProfileNotFound(_) => PricingErrorPolicy::new(
            PortErrorKind::NotFound,
            "pricing.shipping_profile_not_found",
            "shipping profile was not found",
            false,
        ),
        CommerceError::DuplicateShippingProfileSlug(_) => PricingErrorPolicy::new(
            PortErrorKind::Conflict,
            "pricing.duplicate_shipping_profile_slug",
            "shipping profile slug is already in use",
            false,
        ),
        CommerceError::NoVariants => PricingErrorPolicy::new(
            PortErrorKind::Validation,
            "pricing.no_variants",
            "product must have at least one variant",
            false,
        ),
        CommerceError::CannotDeletePublished => PricingErrorPolicy::new(
            PortErrorKind::Conflict,
            "pricing.cannot_delete_published",
            "cannot delete published product",
            false,
        ),
        CommerceError::Core(error) => pricing_core_error_policy(error),
        CommerceError::Rich(error) => pricing_rich_error_policy(&error.kind),
    }
}

fn pricing_core_error_policy(error: &rustok_core::Error) -> PricingErrorPolicy {
    use rustok_core::Error as CoreError;
    match error {
        CoreError::Database(_) => PricingErrorPolicy::new(
            PortErrorKind::Unavailable,
            "pricing.database_unavailable",
            "pricing storage is temporarily unavailable",
            true,
        ),
        CoreError::Cache(_) | CoreError::External(_) => PricingErrorPolicy::new(
            PortErrorKind::Unavailable,
            "pricing.dependency_unavailable",
            "pricing dependency is temporarily unavailable",
            true,
        ),
        CoreError::Auth(_) | CoreError::Forbidden(_) => PricingErrorPolicy::new(
            PortErrorKind::Forbidden,
            "pricing.access_denied",
            "pricing operation is not permitted",
            false,
        ),
        CoreError::InvalidIdFormat(msg) | CoreError::Validation(msg) => PricingErrorPolicy::new(
            PortErrorKind::Validation,
            "pricing.validation",
            msg.clone(),
            false,
        ),
        CoreError::NotFound(_) => PricingErrorPolicy::new(
            PortErrorKind::NotFound,
            "pricing.resource_not_found",
            "pricing resource was not found",
            false,
        ),
        CoreError::Serialization(_) | CoreError::Scripting(_) | CoreError::Internal(_) => {
            PricingErrorPolicy::new(
                PortErrorKind::InvariantViolation,
                "pricing.core_error",
                "pricing operation failed an internal invariant",
                false,
            )
        }
    }
}

fn pricing_rich_error_policy(kind: &rustok_core::error::ErrorKind) -> PricingErrorPolicy {
    use rustok_core::error::ErrorKind;
    match kind {
        ErrorKind::Validation | ErrorKind::BusinessLogic => PricingErrorPolicy::new(
            PortErrorKind::Validation,
            "pricing.validation",
            "pricing request is invalid",
            false,
        ),
        ErrorKind::Unauthenticated | ErrorKind::Forbidden => PricingErrorPolicy::new(
            PortErrorKind::Forbidden,
            "pricing.access_denied",
            "pricing operation is not permitted",
            false,
        ),
        ErrorKind::NotFound => PricingErrorPolicy::new(
            PortErrorKind::NotFound,
            "pricing.resource_not_found",
            "pricing resource was not found",
            false,
        ),
        ErrorKind::Conflict => PricingErrorPolicy::new(
            PortErrorKind::Conflict,
            "pricing.state_conflict",
            "pricing operation conflicts with the current state",
            false,
        ),
        ErrorKind::RateLimited => PricingErrorPolicy::new(
            PortErrorKind::Unavailable,
            "pricing.rate_limited",
            "pricing is temporarily rate limited",
            true,
        ),
        ErrorKind::Database => PricingErrorPolicy::new(
            PortErrorKind::Unavailable,
            "pricing.database_unavailable",
            "pricing storage is temporarily unavailable",
            true,
        ),
        ErrorKind::ExternalService => PricingErrorPolicy::new(
            PortErrorKind::Unavailable,
            "pricing.dependency_unavailable",
            "pricing dependency is temporarily unavailable",
            true,
        ),
        ErrorKind::Timeout => PricingErrorPolicy::new(
            PortErrorKind::Timeout,
            "pricing.timeout",
            "pricing operation timed out",
            true,
        ),
        ErrorKind::Internal => PricingErrorPolicy::new(
            PortErrorKind::InvariantViolation,
            "pricing.rich_error",
            "pricing operation failed an internal invariant",
            false,
        ),
    }
}

pub(crate) fn pricing_error_to_port_error(
    context: &PortContext,
    operation: &'static str,
    error: CommerceError,
) -> PortError {
    let error_facts = pricing_owner_error_facts(&error);
    let mapped = pricing_error_policy(&error);
    let technical = matches!(
        mapped.kind,
        PortErrorKind::Unavailable | PortErrorKind::Timeout | PortErrorKind::InvariantViolation
    );
    log_pricing_port_failure(context, operation, mapped.code, &error_facts, technical);
    mapped.into_port_error()
}

pub(crate) fn log_pricing_service_failure(
    error: &CommerceError,
    operation: &'static str,
    boundary: &'static str,
    public: &crate::error::PricingPublicError,
) {
    let facts = pricing_owner_error_facts(error);
    tracing::error!(
        owner = PRICING_OWNER,
        operation,
        boundary,
        public_code = public.code,
        retryable = public.retryable,
        correlation_id = %public.correlation_id,
        error_variant = facts.error_variant,
        text_field_count = facts.text_field_count,
        text_total_length = facts.text_total_length,
        uuid_field_count = facts.uuid_field_count,
        uuid_non_nil_count = facts.uuid_non_nil_count,
        numeric_field_count = facts.numeric_field_count,
        numeric_nonzero_count = facts.numeric_nonzero_count,
        numeric_negative_count = facts.numeric_negative_count,
        opaque_payload_present = facts.opaque_payload_present,
        "pricing service operation failed"
    );
}

pub(crate) async fn read_with_deadline<T>(
    context: &PortContext,
    read: impl std::future::Future<Output = Result<T, PortError>>,
) -> Result<T, PortError> {
    let deadline_ms = context
        .deadline_ms
        .filter(|millis| *millis > 0)
        .ok_or_else(|| {
            PortError::timeout("port.deadline_required", "pricing reads require a deadline")
        })?;
    if deadline_ms > 30_000 {
        return Err(PortError::validation(
            "pricing.deadline_invalid",
            "pricing read deadline must not exceed 30 seconds",
        ));
    }
    tokio::time::timeout(std::time::Duration::from_millis(deadline_ms), read)
        .await
        .unwrap_or_else(|_| {
            Err(PortError::timeout(
                "pricing.resolution_timeout",
                "pricing exceeded its read deadline",
            ))
        })
}

pub(crate) fn resolution_scope_error(
    error: PortError,
    code: &'static str,
    message: &'static str,
) -> PortError {
    if matches!(
        error.kind,
        PortErrorKind::NotFound | PortErrorKind::Validation
    ) {
        PortError::validation(code, message)
    } else {
        PortError::new(
            error.kind,
            "pricing.scope_unavailable",
            "pricing scope could not be verified",
            error.retryable,
        )
    }
}

pub(crate) fn resolution_error_to_port_error(
    context: &PortContext,
    operation: &'static str,
    error: crate::services::pricing::ResolutionError,
) -> PortError {
    use crate::services::pricing::ResolutionError;
    match error {
        ResolutionError::Ambiguous => PortError::conflict(
            "pricing.price_ambiguous",
            "Several price rows have winning priority; a pricing operator must correct their scopes",
        ),
        ResolutionError::InvalidData(_) => PortError::invariant_violation(
            "pricing.stored_data_invalid",
            "Stored pricing data is unsupported or inconsistent; contact a pricing operator",
        ),
        ResolutionError::RowBudget => PortError::conflict(
            "pricing.price_row_limit",
            "Pricing exceeded 4096 scoped rows; reduce the SKU batch or narrow its scope",
        ),
        ResolutionError::Domain(error) => pricing_error_to_port_error(context, operation, *error),
        ResolutionError::Storage(error) => {
            pricing_error_to_port_error(context, operation, CommerceError::Database(*error))
        }
    }
}
