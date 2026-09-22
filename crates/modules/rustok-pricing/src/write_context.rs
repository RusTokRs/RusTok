use std::sync::Arc;

use async_trait::async_trait;
use rustok_api::{PortContext, PortError, PortErrorKind};
use rustok_outbox::TransactionalEventBus;
use sea_orm::DatabaseConnection;

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
    variant_id_present: bool,
    variant_id_non_nil: bool,
    price_list_id_present: bool,
    price_list_id_non_nil: bool,
    channel_id_present: bool,
    channel_id_non_nil: bool,
    min_quantity_present: bool,
    min_quantity_nonzero: bool,
    min_quantity_negative: bool,
    max_quantity_present: bool,
    max_quantity_nonzero: bool,
    max_quantity_negative: bool,
    currency_code_length: Option<usize>,
    channel_slug_length: Option<usize>,
    fallback_locale_length: Option<usize>,
    compare_at_amount_present: bool,
    adjustment_percent_present: bool,
}

struct PricingWriteContextFacts {
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
            variant_id_present: true,
            variant_id_non_nil: !request.variant_id.is_nil(),
            price_list_id_present: request.price_list_id.is_some(),
            price_list_id_non_nil: request
                .price_list_id
                .map(|value| !value.is_nil())
                .unwrap_or(false),
            channel_id_present: request.channel_id.is_some(),
            channel_id_non_nil: request
                .channel_id
                .map(|value| !value.is_nil())
                .unwrap_or(false),
            min_quantity_present: request.min_quantity.is_some(),
            min_quantity_nonzero: request
                .min_quantity
                .map(|value| value != 0)
                .unwrap_or(false),
            min_quantity_negative: request.min_quantity.map(|value| value < 0).unwrap_or(false),
            max_quantity_present: request.max_quantity.is_some(),
            max_quantity_nonzero: request
                .max_quantity
                .map(|value| value != 0)
                .unwrap_or(false),
            max_quantity_negative: request.max_quantity.map(|value| value < 0).unwrap_or(false),
            currency_code_length: Some(request.currency_code.chars().count()),
            channel_slug_length: request
                .channel_slug
                .as_ref()
                .map(|value| value.chars().count()),
            compare_at_amount_present: request.compare_at_amount.is_some(),
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
            price_list_id_present: true,
            price_list_id_non_nil: !request.price_list_id.is_nil(),
            channel_id_present: request.channel_id.is_some(),
            channel_id_non_nil: request
                .channel_id
                .map(|value| !value.is_nil())
                .unwrap_or(false),
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
            variant_id_present: true,
            variant_id_non_nil: !request.variant_id.is_nil(),
            price_list_id_present: request.price_list_id.is_some(),
            price_list_id_non_nil: request
                .price_list_id
                .map(|value| !value.is_nil())
                .unwrap_or(false),
            channel_id_present: request.channel_id.is_some(),
            channel_id_non_nil: request
                .channel_id
                .map(|value| !value.is_nil())
                .unwrap_or(false),
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
            price_list_id_present: true,
            price_list_id_non_nil: !request.price_list_id.is_nil(),
            fallback_locale_length: request
                .fallback_locale
                .as_ref()
                .map(|value| value.chars().count()),
            adjustment_percent_present: request.adjustment_percent.is_some(),
            ..PricingWriteDiagnosticFacts::default()
        };
        let result =
            PricingWritePort::set_price_list_percentage_rule(&self.inner, context, request).await;
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
        (PortErrorKind::Unavailable, "pricing.database_unavailable") => PricingWriteLocalOutcome {
            local_operation: "owner_storage",
            sanitized_message: None,
        },
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
        (PortErrorKind::Conflict, "pricing.insufficient_inventory") => PricingWriteLocalOutcome {
            local_operation: "validate_inventory_requirement",
            sanitized_message: Some("inventory is insufficient for the pricing operation"),
        },
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
        (PortErrorKind::Conflict, "pricing.cannot_delete_published") => PricingWriteLocalOutcome {
            local_operation: "validate_published_product_state",
            sanitized_message: None,
        },
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

fn pricing_write_context_facts(context: &PortContext) -> PricingWriteContextFacts {
    let actor_kind = match &context.actor.kind {
        rustok_api::PortActorKind::User => "user",
        rustok_api::PortActorKind::Service => "service",
        rustok_api::PortActorKind::System => "system",
    };
    PricingWriteContextFacts {
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

fn pricing_write_port_error_kind(kind: &PortErrorKind) -> &'static str {
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

fn log_pricing_write_local_outcome(
    context: &PortContext,
    owner_operation: &'static str,
    outcome: PricingWriteLocalOutcome,
    facts: &PricingWriteDiagnosticFacts,
    error: &PortError,
    mapped_error: &PortError,
    technical_failure: bool,
) {
    let context_facts = pricing_write_context_facts(context);
    let public_message_present = !mapped_error.message.is_empty();
    let public_message_length = mapped_error.message.chars().count();
    let original_message_length = error.message.chars().count();
    let error_kind = pricing_write_port_error_kind(&mapped_error.kind);

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
            variant_id_present = facts.variant_id_present,
            variant_id_non_nil = facts.variant_id_non_nil,
            price_list_id_present = facts.price_list_id_present,
            price_list_id_non_nil = facts.price_list_id_non_nil,
            channel_id_present = facts.channel_id_present,
            channel_id_non_nil = facts.channel_id_non_nil,
            min_quantity_present = facts.min_quantity_present,
            min_quantity_nonzero = facts.min_quantity_nonzero,
            min_quantity_negative = facts.min_quantity_negative,
            max_quantity_present = facts.max_quantity_present,
            max_quantity_nonzero = facts.max_quantity_nonzero,
            max_quantity_negative = facts.max_quantity_negative,
            currency_code_length = ?facts.currency_code_length,
            channel_slug_length = ?facts.channel_slug_length,
            fallback_locale_length = ?facts.fallback_locale_length,
            compare_at_amount_present = facts.compare_at_amount_present,
            adjustment_percent_present = facts.adjustment_percent_present,
            internal_code = %error.code,
            public_message_present,
            public_message_length,
            original_message_length,
            error_kind,
            retryable = mapped_error.retryable,
            boundary = PRICING_WRITE_BOUNDARY,
            "pricing write local technical outcome retained bounded delegated context"
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
            variant_id_present = facts.variant_id_present,
            variant_id_non_nil = facts.variant_id_non_nil,
            price_list_id_present = facts.price_list_id_present,
            price_list_id_non_nil = facts.price_list_id_non_nil,
            channel_id_present = facts.channel_id_present,
            channel_id_non_nil = facts.channel_id_non_nil,
            min_quantity_present = facts.min_quantity_present,
            min_quantity_nonzero = facts.min_quantity_nonzero,
            min_quantity_negative = facts.min_quantity_negative,
            max_quantity_present = facts.max_quantity_present,
            max_quantity_nonzero = facts.max_quantity_nonzero,
            max_quantity_negative = facts.max_quantity_negative,
            currency_code_length = ?facts.currency_code_length,
            channel_slug_length = ?facts.channel_slug_length,
            fallback_locale_length = ?facts.fallback_locale_length,
            compare_at_amount_present = facts.compare_at_amount_present,
            adjustment_percent_present = facts.adjustment_percent_present,
            internal_code = %error.code,
            public_message_present,
            public_message_length,
            original_message_length,
            error_kind,
            retryable = mapped_error.retryable,
            boundary = PRICING_WRITE_BOUNDARY,
            "pricing write local outcome retained bounded delegated context"
        );
    }
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
    log_pricing_write_local_outcome(
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
