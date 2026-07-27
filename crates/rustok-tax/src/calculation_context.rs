use std::sync::Arc;

use async_trait::async_trait;
use rustok_api::{PortContext, PortError, PortErrorKind};
use uuid::Uuid;

use crate::ports::TaxCalculationPort;
use crate::services::{TaxCalculationInput, TaxCalculationResult, TaxService};

const TAX_OWNER: &str = "rustok_tax";
const CALCULATE_TAX_OPERATION: &str = "calculate_tax";
const TAX_CALCULATION_BOUNDARY: &str = "tax_calculation_port";

#[derive(Debug, Clone)]
struct TaxCalculationDiagnosticFacts {
    currency_code_length: usize,
    channel_id: Option<Uuid>,
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
            map_tax_calculation_local_port_error(
                &diagnostic_context,
                &diagnostic_facts,
                error,
            )
        })
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
        channel_id: request.channel_id,
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

fn map_tax_calculation_local_port_error(
    context: &PortContext,
    facts: &TaxCalculationDiagnosticFacts,
    error: PortError,
) -> PortError {
    let local_operation = match (error.code.as_str(), error.message.as_str()) {
        ("tax.currency_code_invalid", "tax request is invalid") => "validate_currency_code",
        ("tax.negative_policy_rate", "tax request is invalid") => "validate_policy_rate",
        ("tax.country_code_invalid", "tax request is invalid") => {
            "validate_country_rule_code"
        }
        ("tax.negative_country_rate", "tax request is invalid") => {
            "validate_country_rule_rate"
        }
        ("tax.duplicate_country_rule", "tax request is invalid") => {
            "validate_country_rule_uniqueness"
        }
        ("tax.negative_taxable_amount", "tax request is invalid") => {
            "validate_taxable_amount"
        }
        ("tax.validation", "tax request is invalid") => "calculate_provider",
        ("tax.negative_total", "tax calculation result is invalid") => {
            "validate_result_total"
        }
        ("tax.exempt_customer_charged", "tax calculation result is invalid") => {
            "validate_tax_exempt_result"
        }
        ("tax.total_overflow", "tax calculation result is invalid") => "sum_result_lines",
        ("tax.total_mismatch", "tax calculation result is invalid") => {
            "validate_result_total"
        }
        ("tax.provider_id_invalid", "tax calculation result is invalid") => {
            "validate_provider_identity"
        }
        ("tax.negative_line", "tax calculation result is invalid") => {
            "validate_result_line"
        }
        ("tax.currency_code_invalid", "tax calculation result is invalid") => {
            "validate_result_currency"
        }
        ("tax.currency_mismatch", "tax calculation result is invalid") => {
            "validate_result_currency"
        }
        ("tax.unknown_taxable_target", "tax calculation result is invalid") => {
            "validate_result_target"
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
            owner = TAX_OWNER,
            operation = CALCULATE_TAX_OPERATION,
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
            currency_code_length = facts.currency_code_length,
            channel_id = ?facts.channel_id,
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
            internal_message = %error.message,
            error_kind = ?error.kind,
            retryable = error.retryable,
            boundary = TAX_CALCULATION_BOUNDARY,
            "tax calculation local technical outcome retained delegated context"
        );
    } else {
        tracing::warn!(
            error = ?error,
            owner = TAX_OWNER,
            operation = CALCULATE_TAX_OPERATION,
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
            currency_code_length = facts.currency_code_length,
            channel_id = ?facts.channel_id,
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
            internal_message = %error.message,
            error_kind = ?error.kind,
            retryable = error.retryable,
            boundary = TAX_CALCULATION_BOUNDARY,
            "tax calculation local outcome retained delegated context"
        );
    }

    error
}
