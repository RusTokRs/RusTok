use std::sync::Arc;

use async_trait::async_trait;
use rustok_api::{PortContext, PortError, PortErrorKind};

use crate::ports::TaxCalculationPort;
use crate::services::{TaxCalculationInput, TaxCalculationResult, TaxService};

const TAX_OWNER: &str = "rustok_tax";
const CALCULATE_TAX_OPERATION: &str = "calculate_tax";
const TAX_CALCULATION_BOUNDARY: &str = "tax_calculation_port";

#[derive(Debug, Clone)]
struct TaxCalculationContextFacts {
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

#[derive(Debug, Clone)]
struct TaxCalculationDiagnosticFacts {
    currency_code_length: usize,
    channel_id_present: bool,
    channel_id_non_nil: Option<bool>,
    customer_tax_exempt: bool,
    taxable_amount_count: usize,
    line_item_target_count: usize,
    shipping_target_count: usize,
    dual_target_count: usize,
    country_rule_count: usize,
    provider_id_length: Option<usize>,
    channel_provider_id_length: Option<usize>,
    country_code_length: Option<usize>,
}

/// Canonical in-process tax provider that retains safe local outcome context.
pub struct InProcessTaxCalculationPort {
    inner: TaxService,
}

impl InProcessTaxCalculationPort {
    pub fn new() -> Self {
        Self {
            inner: TaxService::new(),
        }
    }

    /// Wraps a host-composed tax service, including custom provider registries.
    pub fn from_service(inner: TaxService) -> Self {
        Self { inner }
    }
}

impl Default for InProcessTaxCalculationPort {
    fn default() -> Self {
        Self::new()
    }
}

pub fn in_process_tax_calculation_port() -> Arc<dyn TaxCalculationPort> {
    Arc::new(InProcessTaxCalculationPort::new())
}

#[async_trait]
impl TaxCalculationPort for InProcessTaxCalculationPort {
    async fn calculate_tax(
        &self,
        context: PortContext,
        request: TaxCalculationInput,
    ) -> Result<TaxCalculationResult, PortError> {
        let diagnostic_context = context.clone();
        let diagnostic_facts = tax_calculation_diagnostic_facts(&request);
        let result = self.inner.calculate_tax(context, request).await;
        result.map_err(|error| {
            map_tax_calculation_local_port_error(&diagnostic_context, &diagnostic_facts, error)
        })
    }
}

fn tax_calculation_context_facts(context: &PortContext) -> TaxCalculationContextFacts {
    let actor_kind = match &context.actor.kind {
        rustok_api::PortActorKind::User => "user",
        rustok_api::PortActorKind::Service => "service",
        rustok_api::PortActorKind::System => "system",
    };
    TaxCalculationContextFacts {
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

fn tax_calculation_diagnostic_facts(
    request: &TaxCalculationInput,
) -> TaxCalculationDiagnosticFacts {
    let line_item_target_count = request
        .taxable_amounts
        .iter()
        .filter(|amount| amount.line_item_id.is_some())
        .count();
    let shipping_target_count = request
        .taxable_amounts
        .iter()
        .filter(|amount| amount.shipping_option_id.is_some())
        .count();
    let dual_target_count = request
        .taxable_amounts
        .iter()
        .filter(|amount| amount.line_item_id.is_some() && amount.shipping_option_id.is_some())
        .count();

    TaxCalculationDiagnosticFacts {
        currency_code_length: request.currency_code.chars().count(),
        channel_id_present: request.channel_id.is_some(),
        channel_id_non_nil: request.channel_id.map(|value| !value.is_nil()),
        customer_tax_exempt: request.customer_tax_exempt,
        taxable_amount_count: request.taxable_amounts.len(),
        line_item_target_count,
        shipping_target_count,
        dual_target_count,
        country_rule_count: request.policy.country_rules.len(),
        provider_id_length: request
            .policy
            .provider_id
            .as_ref()
            .map(|value| value.chars().count()),
        channel_provider_id_length: request
            .policy
            .channel_provider_id
            .as_ref()
            .map(|value| value.chars().count()),
        country_code_length: request
            .policy
            .country_code
            .as_ref()
            .map(|value| value.chars().count()),
    }
}

fn tax_calculation_local_operation(code: &str) -> Option<&'static str> {
    match code {
        "tax.currency_code_invalid" => Some("validate_currency_code"),
        "tax.negative_policy_rate" => Some("validate_policy_rate"),
        "tax.country_code_invalid" => Some("validate_country_rule_code"),
        "tax.negative_country_rate" => Some("validate_country_rule_rate"),
        "tax.duplicate_country_rule" => Some("validate_country_rule_uniqueness"),
        "tax.negative_taxable_amount" => Some("validate_taxable_amount"),
        "tax.validation" => Some("calculate_provider"),
        "tax.negative_total" => Some("validate_result_total"),
        "tax.exempt_customer_charged" => Some("validate_tax_exempt_result"),
        "tax.total_overflow" => Some("sum_result_lines"),
        "tax.total_mismatch" => Some("validate_result_total"),
        "tax.provider_id_invalid" => Some("validate_provider_identity"),
        "tax.negative_line" => Some("validate_result_line"),
        "tax.currency_mismatch" => Some("validate_result_currency"),
        "tax.unknown_taxable_target" => Some("validate_result_target"),
        _ => None,
    }
}

fn map_tax_calculation_local_port_error(
    context: &PortContext,
    facts: &TaxCalculationDiagnosticFacts,
    error: PortError,
) -> PortError {
    let Some(local_operation) = tax_calculation_local_operation(error.code.as_str()) else {
        return error;
    };
    let context_facts = tax_calculation_context_facts(context);
    let technical_failure = matches!(
        &error.kind,
        PortErrorKind::Unavailable | PortErrorKind::Timeout | PortErrorKind::InvariantViolation
    );

    if technical_failure {
        tracing::error!(
            error = ?error,
            owner = TAX_OWNER,
            operation = CALCULATE_TAX_OPERATION,
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
            currency_code_length = facts.currency_code_length,
            channel_id_present = facts.channel_id_present,
            channel_id_non_nil = ?facts.channel_id_non_nil,
            customer_tax_exempt = facts.customer_tax_exempt,
            taxable_amount_count = facts.taxable_amount_count,
            line_item_target_count = facts.line_item_target_count,
            shipping_target_count = facts.shipping_target_count,
            dual_target_count = facts.dual_target_count,
            country_rule_count = facts.country_rule_count,
            provider_id_length = ?facts.provider_id_length,
            channel_provider_id_length = ?facts.channel_provider_id_length,
            country_code_length = ?facts.country_code_length,
            internal_code = %error.code,
            error_kind = ?error.kind,
            retryable = error.retryable,
            boundary = TAX_CALCULATION_BOUNDARY,
            "tax calculation local technical outcome retained safe delegated context"
        );
    } else {
        tracing::warn!(
            error = ?error,
            owner = TAX_OWNER,
            operation = CALCULATE_TAX_OPERATION,
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
            currency_code_length = facts.currency_code_length,
            channel_id_present = facts.channel_id_present,
            channel_id_non_nil = ?facts.channel_id_non_nil,
            customer_tax_exempt = facts.customer_tax_exempt,
            taxable_amount_count = facts.taxable_amount_count,
            line_item_target_count = facts.line_item_target_count,
            shipping_target_count = facts.shipping_target_count,
            dual_target_count = facts.dual_target_count,
            country_rule_count = facts.country_rule_count,
            provider_id_length = ?facts.provider_id_length,
            channel_provider_id_length = ?facts.channel_provider_id_length,
            country_code_length = ?facts.country_code_length,
            internal_code = %error.code,
            error_kind = ?error.kind,
            retryable = error.retryable,
            boundary = TAX_CALCULATION_BOUNDARY,
            "tax calculation local outcome retained safe delegated context"
        );
    }

    error
}
