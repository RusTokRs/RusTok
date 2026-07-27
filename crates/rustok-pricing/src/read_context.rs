use std::sync::Arc;

use async_trait::async_trait;
use rustok_api::{PortContext, PortError, PortErrorKind};
use rustok_outbox::TransactionalEventBus;
use sea_orm::DatabaseConnection;
use uuid::Uuid;

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
const LIST_ACTIVE_PRICE_LIST_PROJECTIONS_OPERATION: &str =
    "list_active_price_list_projections";
const READ_ADMIN_PRODUCT_PRICING_PROJECTION_OPERATION: &str =
    "read_admin_product_pricing_projection";
const READ_STOREFRONT_PRODUCT_PRICING_PROJECTION_OPERATION: &str =
    "read_storefront_product_pricing_projection";
const PREVIEW_VARIANT_DISCOUNT_OPERATION: &str = "preview_variant_discount";

#[derive(Debug, Clone, Default)]
struct PricingReadDiagnosticFacts {
    product_id: Option<Uuid>,
    variant_id: Option<Uuid>,
    region_id: Option<Uuid>,
    channel_id: Option<Uuid>,
    price_list_id: Option<Uuid>,
    selected_price_list_id: Option<Uuid>,
    quantity: Option<i32>,
    currency_code_length: Option<usize>,
    channel_slug_length: Option<usize>,
    locale_length: Option<usize>,
    fallback_locale_length: Option<usize>,
    handle_length: Option<usize>,
    public_channel_slug_length: Option<usize>,
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
            product_id: request.product_id,
            variant_id: Some(request.variant_id),
            region_id: request.region_id,
            channel_id: request.channel_id,
            price_list_id: request.price_list_id,
            quantity: request.quantity,
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
            price_list_id: Some(request.price_list_id),
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
            channel_id: request.channel_id,
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
        let result = PricingReadPort::list_active_price_list_projections(
            &self.inner,
            context,
            request,
        )
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
            product_id: Some(request.product_id),
            selected_price_list_id: request.selected_price_list_id,
            fallback_locale_length: request
                .fallback_locale
                .as_ref()
                .map(|value| value.chars().count()),
            ..PricingReadDiagnosticFacts::default()
        };
        let result = PricingReadPort::read_admin_product_pricing_projection(
            &self.inner,
            context,
            request,
        )
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
            variant_id: Some(request.variant_id),
            channel_id: request.channel_id,
            price_list_id: request.price_list_id,
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
        (
            RESOLVE_PRODUCT_PRICE_OPERATION,
            PortErrorKind::NotFound,
            "pricing.price_not_found",
        ) => PricingReadLocalOutcome {
            local_operation: "resolve_variant_price",
            sanitized_message: Some("price was not found"),
        },
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
        (_, PortErrorKind::Conflict, "pricing.insufficient_inventory") => {
            PricingReadLocalOutcome {
                local_operation: "validate_inventory_requirement",
                sanitized_message: Some("inventory is insufficient for the pricing operation"),
            }
        }
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
        (_, PortErrorKind::InvariantViolation, "pricing.rich_error") => {
            PricingReadLocalOutcome {
                local_operation: "owner_rich_invariant",
                sanitized_message: None,
            }
        }
        (_, PortErrorKind::InvariantViolation, "pricing.core_error") => {
            PricingReadLocalOutcome {
                local_operation: "owner_core_invariant",
                sanitized_message: None,
            }
        }
        _ => return None,
    };
    Some(outcome)
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
    let original_message_length = error.message.chars().count();

    if technical_failure {
        tracing::error!(
            owner = PRICING_OWNER,
            operation = owner_operation,
            local_operation = outcome.local_operation,
            correlation_id = %context.correlation_id,
            tenant_id = %context.tenant_id,
            actor = ?context.actor,
            claim_count = context.claims.len(),
            role_count = context.roles.len(),
            channel = ?context.channel,
            locale = %context.locale,
            causation_id = ?context.causation_id,
            traceparent = ?context.traceparent,
            idempotency_key = ?context.idempotency_key,
            deadline_ms = ?context.deadline_ms,
            product_id = ?facts.product_id,
            variant_id = ?facts.variant_id,
            region_id = ?facts.region_id,
            channel_id = ?facts.channel_id,
            price_list_id = ?facts.price_list_id,
            selected_price_list_id = ?facts.selected_price_list_id,
            quantity = ?facts.quantity,
            currency_code_length = ?facts.currency_code_length,
            channel_slug_length = ?facts.channel_slug_length,
            locale_length = ?facts.locale_length,
            fallback_locale_length = ?facts.fallback_locale_length,
            handle_length = ?facts.handle_length,
            public_channel_slug_length = ?facts.public_channel_slug_length,
            internal_code = %error.code,
            public_message = %mapped_error.message,
            original_message_length,
            error_kind = ?mapped_error.kind,
            retryable = mapped_error.retryable,
            boundary = PRICING_READ_BOUNDARY,
            "pricing read local technical outcome retained delegated context"
        );
    } else {
        tracing::warn!(
            owner = PRICING_OWNER,
            operation = owner_operation,
            local_operation = outcome.local_operation,
            correlation_id = %context.correlation_id,
            tenant_id = %context.tenant_id,
            actor = ?context.actor,
            claim_count = context.claims.len(),
            role_count = context.roles.len(),
            channel = ?context.channel,
            locale = %context.locale,
            causation_id = ?context.causation_id,
            traceparent = ?context.traceparent,
            idempotency_key = ?context.idempotency_key,
            deadline_ms = ?context.deadline_ms,
            product_id = ?facts.product_id,
            variant_id = ?facts.variant_id,
            region_id = ?facts.region_id,
            channel_id = ?facts.channel_id,
            price_list_id = ?facts.price_list_id,
            selected_price_list_id = ?facts.selected_price_list_id,
            quantity = ?facts.quantity,
            currency_code_length = ?facts.currency_code_length,
            channel_slug_length = ?facts.channel_slug_length,
            locale_length = ?facts.locale_length,
            fallback_locale_length = ?facts.fallback_locale_length,
            handle_length = ?facts.handle_length,
            public_channel_slug_length = ?facts.public_channel_slug_length,
            internal_code = %error.code,
            public_message = %mapped_error.message,
            original_message_length,
            error_kind = ?mapped_error.kind,
            retryable = mapped_error.retryable,
            boundary = PRICING_READ_BOUNDARY,
            "pricing read local outcome retained delegated context"
        );
    }

    mapped_error
}
