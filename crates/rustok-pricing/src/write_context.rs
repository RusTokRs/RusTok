use std::sync::Arc;

use async_trait::async_trait;
use rustok_api::{PortContext, PortError, PortErrorKind};
use rustok_outbox::TransactionalEventBus;
use sea_orm::DatabaseConnection;
use uuid::Uuid;

use crate::ports::{
    ApplyVariantDiscountRequest, PricingWritePort, SetPriceListPercentageRuleRequest,
    SetPriceListScopeRequest, UpsertVariantPriceRequest,
};
use crate::{ActivePriceListOption, AdminPricingPrice, PriceAdjustmentPreview, PricingService};

const PRICING_OWNER: &str = "rustok_pricing";
const PRICING_WRITE_BOUNDARY: &str = "pricing_write_port";
const UPSERT_VARIANT_PRICE_OPERATION: &str = "upsert_variant_price";
const SET_PRICE_LIST_SCOPE_OPERATION: &str = "set_price_list_scope";
const APPLY_VARIANT_DISCOUNT_OPERATION: &str = "apply_variant_discount";
const SET_PRICE_LIST_PERCENTAGE_RULE_OPERATION: &str = "set_price_list_percentage_rule";

#[derive(Debug, Clone, Default)]
struct PricingWriteDiagnosticFacts {
    variant_id: Option<Uuid>,
    price_list_id: Option<Uuid>,
    channel_id: Option<Uuid>,
    min_quantity: Option<i32>,
    max_quantity: Option<i32>,
    currency_code_length: Option<usize>,
    channel_slug_length: Option<usize>,
    fallback_locale_length: Option<usize>,
    compare_at_amount_present: Option<bool>,
    adjustment_percent_present: Option<bool>,
}

#[derive(Debug, Clone, Copy)]
struct PricingWriteLocalOutcome {
    local_operation: &'static str,
    sanitized_message: Option<&'static str>,
}

/// Canonical in-process pricing write provider with retained local outcome context.
pub struct InProcessPricingWritePort {
    inner: PricingService,
}

impl InProcessPricingWritePort {
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

/// Builds the canonical owner-managed in-process pricing write provider.
pub fn in_process_pricing_write_port(
    db: DatabaseConnection,
    event_bus: TransactionalEventBus,
) -> Arc<dyn PricingWritePort> {
    Arc::new(InProcessPricingWritePort::new(db, event_bus))
}

#[async_trait]
impl PricingWritePort for InProcessPricingWritePort {
    async fn upsert_variant_price(
        &self,
        context: PortContext,
        request: UpsertVariantPriceRequest,
    ) -> Result<AdminPricingPrice, PortError> {
        let diagnostic_context = context.clone();
        let diagnostic_facts = PricingWriteDiagnosticFacts {
            variant_id: Some(request.variant_id),
            price_list_id: request.price_list_id,
            channel_id: request.channel_id,
            min_quantity: request.min_quantity,
            max_quantity: request.max_quantity,
            currency_code_length: Some(request.currency_code.chars().count()),
            channel_slug_length: request
                .channel_slug
                .as_ref()
                .map(|value| value.chars().count()),
            compare_at_amount_present: Some(request.compare_at_amount.is_some()),
            ..PricingWriteDiagnosticFacts::default()
        };
        let result = PricingWritePort::upsert_variant_price(&self.inner, context, request).await;
        result.map_err(|error| {
            map_pricing_write_local_port_error(
                &diagnostic_context,
                UPSERT_VARIANT_PRICE_OPERATION,
                &diagnostic_facts,
                error,
            )
        })
    }

    async fn set_price_list_scope(
        &self,
        context: PortContext,
        request: SetPriceListScopeRequest,
    ) -> Result<ActivePriceListOption, PortError> {
        let diagnostic_context = context.clone();
        let diagnostic_facts = PricingWriteDiagnosticFacts {
            price_list_id: Some(request.price_list_id),
            channel_id: request.channel_id,
            channel_slug_length: request
                .channel_slug
                .as_ref()
                .map(|value| value.chars().count()),
            ..PricingWriteDiagnosticFacts::default()
        };
        let result = PricingWritePort::set_price_list_scope(&self.inner, context, request).await;
        result.map_err(|error| {
            map_pricing_write_local_port_error(
                &diagnostic_context,
                SET_PRICE_LIST_SCOPE_OPERATION,
                &diagnostic_facts,
                error,
            )
        })
    }

    async fn apply_variant_discount(
        &self,
        context: PortContext,
        request: ApplyVariantDiscountRequest,
    ) -> Result<PriceAdjustmentPreview, PortError> {
        let diagnostic_context = context.clone();
        let diagnostic_facts = PricingWriteDiagnosticFacts {
            variant_id: Some(request.variant_id),
            price_list_id: request.price_list_id,
            channel_id: request.channel_id,
            currency_code_length: Some(request.currency_code.chars().count()),
            channel_slug_length: request
                .channel_slug
                .as_ref()
                .map(|value| value.chars().count()),
            ..PricingWriteDiagnosticFacts::default()
        };
        let result = PricingWritePort::apply_variant_discount(&self.inner, context, request).await;
        result.map_err(|error| {
            map_pricing_write_local_port_error(
                &diagnostic_context,
                APPLY_VARIANT_DISCOUNT_OPERATION,
                &diagnostic_facts,
                error,
            )
        })
    }

    async fn set_price_list_percentage_rule(
        &self,
        context: PortContext,
        request: SetPriceListPercentageRuleRequest,
    ) -> Result<ActivePriceListOption, PortError> {
        let diagnostic_context = context.clone();
        let diagnostic_facts = PricingWriteDiagnosticFacts {
            price_list_id: Some(request.price_list_id),
            fallback_locale_length: request
                .fallback_locale
                .as_ref()
                .map(|value| value.chars().count()),
            adjustment_percent_present: Some(request.adjustment_percent.is_some()),
            ..PricingWriteDiagnosticFacts::default()
        };
        let result = PricingWritePort::set_price_list_percentage_rule(
            &self.inner,
            context,
            request,
        )
        .await;
        result.map_err(|error| {
            map_pricing_write_local_port_error(
                &diagnostic_context,
                SET_PRICE_LIST_PERCENTAGE_RULE_OPERATION,
                &diagnostic_facts,
                error,
            )
        })
    }
}

fn classify_pricing_write_local_outcome(error: &PortError) -> Option<PricingWriteLocalOutcome> {
    let outcome = match (&error.kind, error.code.as_str()) {
        (PortErrorKind::Validation, "pricing.tenant_id_invalid") => PricingWriteLocalOutcome {
            local_operation: "validate_tenant_context",
            sanitized_message: Some("pricing request context is invalid"),
        },
        (PortErrorKind::Validation, "pricing.actor_id_invalid") => PricingWriteLocalOutcome {
            local_operation: "validate_actor_context",
            sanitized_message: Some("pricing write actor is invalid"),
        },
        (PortErrorKind::Unavailable, "pricing.database_unavailable") => {
            PricingWriteLocalOutcome {
                local_operation: "owner_storage",
                sanitized_message: None,
            }
        }
        (PortErrorKind::NotFound, "pricing.product_not_found") => PricingWriteLocalOutcome {
            local_operation: "load_product",
            sanitized_message: Some("product was not found"),
        },
        (PortErrorKind::NotFound, "pricing.variant_not_found") => PricingWriteLocalOutcome {
            local_operation: "load_variant",
            sanitized_message: Some("variant was not found"),
        },
        (PortErrorKind::Conflict, "pricing.duplicate_handle") => PricingWriteLocalOutcome {
            local_operation: "validate_handle_uniqueness",
            sanitized_message: Some("pricing handle is already in use"),
        },
        (PortErrorKind::Conflict, "pricing.duplicate_sku") => PricingWriteLocalOutcome {
            local_operation: "validate_sku_uniqueness",
            sanitized_message: Some("pricing SKU is already in use"),
        },
        (PortErrorKind::Validation, "pricing.validation") => PricingWriteLocalOutcome {
            local_operation: "validate_owner_request",
            sanitized_message: None,
        },
        (PortErrorKind::Conflict, "pricing.insufficient_inventory") => {
            PricingWriteLocalOutcome {
                local_operation: "validate_inventory_requirement",
                sanitized_message: Some("inventory is insufficient for the pricing operation"),
            }
        }
        (PortErrorKind::Validation, "pricing.invalid_option_combination") => {
            PricingWriteLocalOutcome {
                local_operation: "validate_option_combination",
                sanitized_message: None,
            }
        }
        (PortErrorKind::NotFound, "pricing.shipping_profile_not_found") => {
            PricingWriteLocalOutcome {
                local_operation: "load_shipping_profile",
                sanitized_message: Some("shipping profile was not found"),
            }
        }
        (PortErrorKind::Conflict, "pricing.duplicate_shipping_profile_slug") => {
            PricingWriteLocalOutcome {
                local_operation: "validate_shipping_profile_slug_uniqueness",
                sanitized_message: Some("shipping profile slug is already in use"),
            }
        }
        (PortErrorKind::Validation, "pricing.no_variants") => PricingWriteLocalOutcome {
            local_operation: "validate_product_variants",
            sanitized_message: None,
        },
        (PortErrorKind::Conflict, "pricing.cannot_delete_published") => {
            PricingWriteLocalOutcome {
                local_operation: "validate_published_product_state",
                sanitized_message: None,
            }
        }
        (PortErrorKind::InvariantViolation, "pricing.rich_error") => PricingWriteLocalOutcome {
            local_operation: "owner_rich_invariant",
            sanitized_message: None,
        },
        (PortErrorKind::InvariantViolation, "pricing.core_error") => PricingWriteLocalOutcome {
            local_operation: "owner_core_invariant",
            sanitized_message: None,
        },
        _ => return None,
    };
    Some(outcome)
}

fn map_pricing_write_local_port_error(
    context: &PortContext,
    owner_operation: &'static str,
    facts: &PricingWriteDiagnosticFacts,
    error: PortError,
) -> PortError {
    let Some(outcome) = classify_pricing_write_local_outcome(&error) else {
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
            variant_id = ?facts.variant_id,
            price_list_id = ?facts.price_list_id,
            channel_id = ?facts.channel_id,
            min_quantity = ?facts.min_quantity,
            max_quantity = ?facts.max_quantity,
            currency_code_length = ?facts.currency_code_length,
            channel_slug_length = ?facts.channel_slug_length,
            fallback_locale_length = ?facts.fallback_locale_length,
            compare_at_amount_present = ?facts.compare_at_amount_present,
            adjustment_percent_present = ?facts.adjustment_percent_present,
            internal_code = %error.code,
            public_message = %mapped_error.message,
            original_message_length,
            error_kind = ?mapped_error.kind,
            retryable = mapped_error.retryable,
            boundary = PRICING_WRITE_BOUNDARY,
            "pricing write local technical outcome retained delegated context"
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
            variant_id = ?facts.variant_id,
            price_list_id = ?facts.price_list_id,
            channel_id = ?facts.channel_id,
            min_quantity = ?facts.min_quantity,
            max_quantity = ?facts.max_quantity,
            currency_code_length = ?facts.currency_code_length,
            channel_slug_length = ?facts.channel_slug_length,
            fallback_locale_length = ?facts.fallback_locale_length,
            compare_at_amount_present = ?facts.compare_at_amount_present,
            adjustment_percent_present = ?facts.adjustment_percent_present,
            internal_code = %error.code,
            public_message = %mapped_error.message,
            original_message_length,
            error_kind = ?mapped_error.kind,
            retryable = mapped_error.retryable,
            boundary = PRICING_WRITE_BOUNDARY,
            "pricing write local outcome retained delegated context"
        );
    }

    mapped_error
}
