use std::sync::Arc;

use async_trait::async_trait;
use rustok_api::{PortContext, PortError, PortErrorKind};
use rustok_outbox::TransactionalEventBus;
use sea_orm::DatabaseConnection;

use crate::ports::{
    ActivePriceListProjectionRequest, ActivePriceListProjectionSnapshot,
    AdminProductPricingProjectionRequest, PreviewVariantDiscountRequest,
    PriceListProjectionRequest, PriceListProjectionSnapshot, PricingReadPort,
    ResolveProductPriceRequest, ResolvedProductPriceSnapshot,
    StorefrontProductPricingProjectionRequest,
};
use crate::{
    AdminPricingProductDetail, PriceAdjustmentPreview, PricingService,
    StorefrontPricingProductDetail,
};

const PRICING_OWNER: &str = "rustok_pricing";
const PRICING_READ_BOUNDARY: &str = "pricing_read_port";
const RESOLVE_PRODUCT_PRICE_OPERATION: &str = "resolve_product_price";
const READ_PRICE_LIST_PROJECTION_OPERATION: &str = "read_price_list_projection";
const LIST_ACTIVE_PRICE_LIST_PROJECTIONS_OPERATION: &str = "list_active_price_list_projections";
const READ_ADMIN_PRODUCT_PRICING_PROJECTION_OPERATION: &str =
    "read_admin_product_pricing_projection";
const READ_STOREFRONT_PRODUCT_PRICING_PROJECTION_OPERATION: &str =
    "read_storefront_product_pricing_projection";
const PREVIEW_VARIANT_DISCOUNT_OPERATION: &str = "preview_variant_discount";

#[derive(Debug, Clone, Default)]
struct PricingReadDiagnosticFacts {
    product_id_present: bool,
    product_id_non_nil: bool,
    variant_id_present: bool,
    variant_id_non_nil: bool,
    region_id_present: bool,
    region_id_non_nil: bool,
    channel_id_present: bool,
    channel_id_non_nil: bool,
    price_list_id_present: bool,
    price_list_id_non_nil: bool,
    selected_price_list_id_present: bool,
    selected_price_list_id_non_nil: bool,
    quantity_present: bool,
    quantity_nonzero: bool,
    quantity_negative: bool,
    currency_code_length: Option<usize>,
    channel_slug_length: Option<usize>,
    locale_length: Option<usize>,
    fallback_locale_length: Option<usize>,
    handle_length: Option<usize>,
    public_channel_slug_length: Option<usize>,
}

struct PricingReadContextFacts {
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

#[derive(Debug, Clone, Copy)]
struct PricingReadLocalOutcome {
    local_operation: &'static str,
    sanitized_message: Option<&'static str>,
}

/// Canonical in-process pricing read provider with retained local outcome context.
pub struct InProcessPricingReadPort {
    inner: PricingService,
}

impl InProcessPricingReadPort {
    pub fn new(db: DatabaseConnection, event_bus: TransactionalEventBus) -> Self {
        Self {
            inner: PricingService::new(db, event_bus),
        }
    }

    /// Wraps a host-composed pricing service without changing owner execution.
    pub fn from_service(inner: PricingService) -> Self {
        Self { inner }
    }
}

/// Builds the canonical owner-managed in-process pricing read provider.
pub fn in_process_pricing_read_port(
    db: DatabaseConnection,
    event_bus: TransactionalEventBus,
) -> Arc<dyn PricingReadPort> {
    Arc::new(InProcessPricingReadPort::new(db, event_bus))
}

#[async_trait]
impl PricingReadPort for InProcessPricingReadPort {
    async fn resolve_product_price(
        &self,
        context: PortContext,
        request: ResolveProductPriceRequest,
    ) -> Result<ResolvedProductPriceSnapshot, PortError> {
        let diagnostic_context = context.clone();
        let diagnostic_facts = PricingReadDiagnosticFacts {
            product_id_present: request.product_id.is_some(),
            product_id_non_nil: request
                .product_id
                .map(|value| !value.is_nil())
                .unwrap_or(false),
            variant_id_present: true,
            variant_id_non_nil: !request.variant_id.is_nil(),
            region_id_present: request.region_id.is_some(),
            region_id_non_nil: request
                .region_id
                .map(|value| !value.is_nil())
                .unwrap_or(false),
            channel_id_present: request.channel_id.is_some(),
            channel_id_non_nil: request
                .channel_id
                .map(|value| !value.is_nil())
                .unwrap_or(false),
            price_list_id_present: request.price_list_id.is_some(),
            price_list_id_non_nil: request
                .price_list_id
                .map(|value| !value.is_nil())
                .unwrap_or(false),
            quantity_present: request.quantity.is_some(),
            quantity_nonzero: request.quantity.map(|value| value != 0).unwrap_or(false),
            quantity_negative: request.quantity.map(|value| value < 0).unwrap_or(false),
            currency_code_length: Some(request.currency_code.chars().count()),
            channel_slug_length: request
                .channel_slug
                .as_ref()
                .map(|value| value.chars().count()),
            ..PricingReadDiagnosticFacts::default()
        };
        let result = PricingReadPort::resolve_product_price(&self.inner, context, request).await;
        result.map_err(|error| {
            map_pricing_read_local_port_error(
                &diagnostic_context,
                RESOLVE_PRODUCT_PRICE_OPERATION,
                &diagnostic_facts,
                error,
            )
        })
    }

    async fn read_price_list_projection(
        &self,
        context: PortContext,
        request: PriceListProjectionRequest,
    ) -> Result<PriceListProjectionSnapshot, PortError> {
        let diagnostic_context = context.clone();
        let diagnostic_facts = PricingReadDiagnosticFacts {
            price_list_id_present: true,
            price_list_id_non_nil: !request.price_list_id.is_nil(),
            locale_length: request.locale.as_ref().map(|value| value.chars().count()),
            ..PricingReadDiagnosticFacts::default()
        };
        let result =
            PricingReadPort::read_price_list_projection(&self.inner, context, request).await;
        result.map_err(|error| {
            map_pricing_read_local_port_error(
                &diagnostic_context,
                READ_PRICE_LIST_PROJECTION_OPERATION,
                &diagnostic_facts,
                error,
            )
        })
    }

    async fn list_active_price_list_projections(
        &self,
        context: PortContext,
        request: ActivePriceListProjectionRequest,
    ) -> Result<Vec<ActivePriceListProjectionSnapshot>, PortError> {
        let diagnostic_context = context.clone();
        let diagnostic_facts = PricingReadDiagnosticFacts {
            channel_id_present: request.channel_id.is_some(),
            channel_id_non_nil: request
                .channel_id
                .map(|value| !value.is_nil())
                .unwrap_or(false),
            channel_slug_length: request
                .channel_slug
                .as_ref()
                .map(|value| value.chars().count()),
            fallback_locale_length: request
                .fallback_locale
                .as_ref()
                .map(|value| value.chars().count()),
            ..PricingReadDiagnosticFacts::default()
        };
        let result =
            PricingReadPort::list_active_price_list_projections(&self.inner, context, request)
                .await;
        result.map_err(|error| {
            map_pricing_read_local_port_error(
                &diagnostic_context,
                LIST_ACTIVE_PRICE_LIST_PROJECTIONS_OPERATION,
                &diagnostic_facts,
                error,
            )
        })
    }

    async fn read_admin_product_pricing_projection(
        &self,
        context: PortContext,
        request: AdminProductPricingProjectionRequest,
    ) -> Result<AdminPricingProductDetail, PortError> {
        let diagnostic_context = context.clone();
        let diagnostic_facts = PricingReadDiagnosticFacts {
            product_id_present: true,
            product_id_non_nil: !request.product_id.is_nil(),
            selected_price_list_id_present: request.selected_price_list_id.is_some(),
            selected_price_list_id_non_nil: request
                .selected_price_list_id
                .map(|value| !value.is_nil())
                .unwrap_or(false),
            fallback_locale_length: request
                .fallback_locale
                .as_ref()
                .map(|value| value.chars().count()),
            ..PricingReadDiagnosticFacts::default()
        };
        let result =
            PricingReadPort::read_admin_product_pricing_projection(&self.inner, context, request)
                .await;
        result.map_err(|error| {
            map_pricing_read_local_port_error(
                &diagnostic_context,
                READ_ADMIN_PRODUCT_PRICING_PROJECTION_OPERATION,
                &diagnostic_facts,
                error,
            )
        })
    }

    async fn read_storefront_product_pricing_projection(
        &self,
        context: PortContext,
        request: StorefrontProductPricingProjectionRequest,
    ) -> Result<Option<StorefrontPricingProductDetail>, PortError> {
        let diagnostic_context = context.clone();
        let diagnostic_facts = PricingReadDiagnosticFacts {
            handle_length: Some(request.handle.chars().count()),
            fallback_locale_length: request
                .fallback_locale
                .as_ref()
                .map(|value| value.chars().count()),
            public_channel_slug_length: request
                .public_channel_slug
                .as_ref()
                .map(|value| value.chars().count()),
            ..PricingReadDiagnosticFacts::default()
        };
        let result = PricingReadPort::read_storefront_product_pricing_projection(
            &self.inner,
            context,
            request,
        )
        .await;
        result.map_err(|error| {
            map_pricing_read_local_port_error(
                &diagnostic_context,
                READ_STOREFRONT_PRODUCT_PRICING_PROJECTION_OPERATION,
                &diagnostic_facts,
                error,
            )
        })
    }

    async fn preview_variant_discount(
        &self,
        context: PortContext,
        request: PreviewVariantDiscountRequest,
    ) -> Result<PriceAdjustmentPreview, PortError> {
        let diagnostic_context = context.clone();
        let diagnostic_facts = PricingReadDiagnosticFacts {
            variant_id_present: true,
            variant_id_non_nil: !request.variant_id.is_nil(),
            channel_id_present: request.channel_id.is_some(),
            channel_id_non_nil: request
                .channel_id
                .map(|value| !value.is_nil())
                .unwrap_or(false),
            price_list_id_present: request.price_list_id.is_some(),
            price_list_id_non_nil: request
                .price_list_id
                .map(|value| !value.is_nil())
                .unwrap_or(false),
            currency_code_length: Some(request.currency_code.chars().count()),
            channel_slug_length: request
                .channel_slug
                .as_ref()
                .map(|value| value.chars().count()),
            ..PricingReadDiagnosticFacts::default()
        };
        let result = PricingReadPort::preview_variant_discount(&self.inner, context, request).await;
        result.map_err(|error| {
            map_pricing_read_local_port_error(
                &diagnostic_context,
                PREVIEW_VARIANT_DISCOUNT_OPERATION,
                &diagnostic_facts,
                error,
            )
        })
    }
}

fn classify_pricing_read_local_outcome(
    owner_operation: &'static str,
    error: &PortError,
) -> Option<PricingReadLocalOutcome> {
    let outcome = match (owner_operation, &error.kind, error.code.as_str()) {
        (_, PortErrorKind::Validation, "pricing.tenant_id_invalid") => PricingReadLocalOutcome {
            local_operation: "validate_tenant_context",
            sanitized_message: Some("pricing request context is invalid"),
        },
        (
            RESOLVE_PRODUCT_PRICE_OPERATION,
            PortErrorKind::Validation,
            "pricing.variant_product_mismatch",
        ) => PricingReadLocalOutcome {
            local_operation: "validate_variant_product_identity",
            sanitized_message: Some("variant does not belong to the requested product"),
        },
        (RESOLVE_PRODUCT_PRICE_OPERATION, PortErrorKind::NotFound, "pricing.price_not_found") => {
            PricingReadLocalOutcome {
                local_operation: "resolve_variant_price",
                sanitized_message: Some("price was not found"),
            }
        }
        (
            READ_PRICE_LIST_PROJECTION_OPERATION,
            PortErrorKind::NotFound,
            "pricing.price_list_not_found",
        ) => PricingReadLocalOutcome {
            local_operation: "load_price_list_projection",
            sanitized_message: Some("price list was not found"),
        },
        (_, PortErrorKind::Unavailable, "pricing.database_unavailable") => {
            PricingReadLocalOutcome {
                local_operation: "owner_storage",
                sanitized_message: None,
            }
        }
        (_, PortErrorKind::NotFound, "pricing.product_not_found") => PricingReadLocalOutcome {
            local_operation: "load_product",
            sanitized_message: Some("product was not found"),
        },
        (_, PortErrorKind::NotFound, "pricing.variant_not_found") => PricingReadLocalOutcome {
            local_operation: "load_variant",
            sanitized_message: Some("variant was not found"),
        },
        (_, PortErrorKind::Conflict, "pricing.duplicate_handle") => PricingReadLocalOutcome {
            local_operation: "validate_handle_uniqueness",
            sanitized_message: Some("pricing handle is already in use"),
        },
        (_, PortErrorKind::Conflict, "pricing.duplicate_sku") => PricingReadLocalOutcome {
            local_operation: "validate_sku_uniqueness",
            sanitized_message: Some("pricing SKU is already in use"),
        },
        (_, PortErrorKind::Validation, "pricing.validation") => PricingReadLocalOutcome {
            local_operation: "validate_owner_request",
            sanitized_message: None,
        },
        (_, PortErrorKind::Conflict, "pricing.insufficient_inventory") => PricingReadLocalOutcome {
            local_operation: "validate_inventory_requirement",
            sanitized_message: Some("inventory is insufficient for the pricing operation"),
        },
        (_, PortErrorKind::Validation, "pricing.invalid_option_combination") => {
            PricingReadLocalOutcome {
                local_operation: "validate_option_combination",
                sanitized_message: None,
            }
        }
        (_, PortErrorKind::NotFound, "pricing.shipping_profile_not_found") => {
            PricingReadLocalOutcome {
                local_operation: "load_shipping_profile",
                sanitized_message: Some("shipping profile was not found"),
            }
        }
        (_, PortErrorKind::Conflict, "pricing.duplicate_shipping_profile_slug") => {
            PricingReadLocalOutcome {
                local_operation: "validate_shipping_profile_slug_uniqueness",
                sanitized_message: Some("shipping profile slug is already in use"),
            }
        }
        (_, PortErrorKind::Validation, "pricing.no_variants") => PricingReadLocalOutcome {
            local_operation: "validate_product_variants",
            sanitized_message: None,
        },
        (_, PortErrorKind::Conflict, "pricing.cannot_delete_published") => {
            PricingReadLocalOutcome {
                local_operation: "validate_published_product_state",
                sanitized_message: None,
            }
        }
        (_, PortErrorKind::InvariantViolation, "pricing.rich_error") => PricingReadLocalOutcome {
            local_operation: "owner_rich_invariant",
            sanitized_message: None,
        },
        (_, PortErrorKind::InvariantViolation, "pricing.core_error") => PricingReadLocalOutcome {
            local_operation: "owner_core_invariant",
            sanitized_message: None,
        },
        _ => return None,
    };
    Some(outcome)
}

fn pricing_read_context_facts(context: &PortContext) -> PricingReadContextFacts {
    let actor_kind = match &context.actor.kind {
        rustok_api::PortActorKind::User => "user",
        rustok_api::PortActorKind::Service => "service",
        rustok_api::PortActorKind::System => "system",
    };
    PricingReadContextFacts {
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

fn pricing_read_port_error_kind(kind: &PortErrorKind) -> &'static str {
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

fn log_pricing_read_local_outcome(
    context: &PortContext,
    owner_operation: &'static str,
    outcome: PricingReadLocalOutcome,
    facts: &PricingReadDiagnosticFacts,
    error: &PortError,
    mapped_error: &PortError,
    technical_failure: bool,
) {
    let context_facts = pricing_read_context_facts(context);
    let public_message_present = !mapped_error.message.is_empty();
    let public_message_length = mapped_error.message.chars().count();
    let original_message_length = error.message.chars().count();
    let error_kind = pricing_read_port_error_kind(&mapped_error.kind);

    if technical_failure {
        tracing::error!(
            owner = PRICING_OWNER,
            operation = owner_operation,
            local_operation = outcome.local_operation,
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
            product_id_present = facts.product_id_present,
            product_id_non_nil = facts.product_id_non_nil,
            variant_id_present = facts.variant_id_present,
            variant_id_non_nil = facts.variant_id_non_nil,
            region_id_present = facts.region_id_present,
            region_id_non_nil = facts.region_id_non_nil,
            channel_id_present = facts.channel_id_present,
            channel_id_non_nil = facts.channel_id_non_nil,
            price_list_id_present = facts.price_list_id_present,
            price_list_id_non_nil = facts.price_list_id_non_nil,
            selected_price_list_id_present = facts.selected_price_list_id_present,
            selected_price_list_id_non_nil = facts.selected_price_list_id_non_nil,
            quantity_present = facts.quantity_present,
            quantity_nonzero = facts.quantity_nonzero,
            quantity_negative = facts.quantity_negative,
            currency_code_length = ?facts.currency_code_length,
            channel_slug_length = ?facts.channel_slug_length,
            request_locale_length = ?facts.locale_length,
            fallback_locale_length = ?facts.fallback_locale_length,
            handle_length = ?facts.handle_length,
            public_channel_slug_length = ?facts.public_channel_slug_length,
            internal_code = %error.code,
            public_message_present,
            public_message_length,
            original_message_length,
            error_kind,
            retryable = mapped_error.retryable,
            boundary = PRICING_READ_BOUNDARY,
            "pricing read local technical outcome retained bounded delegated context"
        );
    } else {
        tracing::warn!(
            owner = PRICING_OWNER,
            operation = owner_operation,
            local_operation = outcome.local_operation,
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
            product_id_present = facts.product_id_present,
            product_id_non_nil = facts.product_id_non_nil,
            variant_id_present = facts.variant_id_present,
            variant_id_non_nil = facts.variant_id_non_nil,
            region_id_present = facts.region_id_present,
            region_id_non_nil = facts.region_id_non_nil,
            channel_id_present = facts.channel_id_present,
            channel_id_non_nil = facts.channel_id_non_nil,
            price_list_id_present = facts.price_list_id_present,
            price_list_id_non_nil = facts.price_list_id_non_nil,
            selected_price_list_id_present = facts.selected_price_list_id_present,
            selected_price_list_id_non_nil = facts.selected_price_list_id_non_nil,
            quantity_present = facts.quantity_present,
            quantity_nonzero = facts.quantity_nonzero,
            quantity_negative = facts.quantity_negative,
            currency_code_length = ?facts.currency_code_length,
            channel_slug_length = ?facts.channel_slug_length,
            request_locale_length = ?facts.locale_length,
            fallback_locale_length = ?facts.fallback_locale_length,
            handle_length = ?facts.handle_length,
            public_channel_slug_length = ?facts.public_channel_slug_length,
            internal_code = %error.code,
            public_message_present,
            public_message_length,
            original_message_length,
            error_kind,
            retryable = mapped_error.retryable,
            boundary = PRICING_READ_BOUNDARY,
            "pricing read local outcome retained bounded delegated context"
        );
    }
}

fn map_pricing_read_local_port_error(
    context: &PortContext,
    owner_operation: &'static str,
    facts: &PricingReadDiagnosticFacts,
    error: PortError,
) -> PortError {
    let Some(outcome) = classify_pricing_read_local_outcome(owner_operation, &error) else {
        return error;
    };

    let mapped_error = match outcome.sanitized_message {
        Some(message) => PortError::new(
            error.kind.clone(),
            error.code.clone(),
            message,
            error.retryable,
        ),
        None => error.clone(),
    };
    let technical_failure = matches!(
        &mapped_error.kind,
        PortErrorKind::Unavailable | PortErrorKind::Timeout | PortErrorKind::InvariantViolation
    );
    log_pricing_read_local_outcome(
        context,
        owner_operation,
        outcome,
        facts,
        &error,
        &mapped_error,
        technical_failure,
    );

    mapped_error
}
