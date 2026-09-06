use async_trait::async_trait;
use rust_decimal::Decimal;
use rustok_api::{PortCallPolicy, PortContext, PortError, PortErrorKind};
use std::{collections::HashSet, sync::Arc};
use uuid::Uuid;

use crate::{CalculatedTaxLine, TaxCalculationInput, TaxCalculationResult, TaxError};

const TAX_CALCULATION_PORT_BOUNDARY: &str = "tax_calculation_port";

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

/// Transport-neutral owner boundary for tax calculation.
#[async_trait]
pub trait TaxCalculationPort: Send + Sync {
    async fn calculate_tax(
        &self,
        context: PortContext,
        request: TaxCalculationInput,
    ) -> Result<TaxCalculationResult, PortError>;
}

/// Builds the owner-managed in-process provider for consumers that do not
/// supply a separately composed tax runtime.
pub fn in_process_tax_calculation_port() -> Arc<dyn TaxCalculationPort> {
    Arc::new(crate::TaxService::new())
}

#[async_trait]
impl TaxCalculationPort for crate::TaxService {
    async fn calculate_tax(
        &self,
        context: PortContext,
        request: TaxCalculationInput,
    ) -> Result<TaxCalculationResult, PortError> {
        let owner_operation = "calculate_tax";
        require_tax_calculation_policy(&context, owner_operation)?;
        let expected_currency = validate_tax_request(&context, owner_operation, &request)?;
        let customer_tax_exempt = request.customer_tax_exempt;
        let taxable_targets = request
            .taxable_amounts
            .iter()
            .map(|amount| (amount.line_item_id, amount.shipping_option_id))
            .collect::<HashSet<_>>();

        let result = self
            .calculate(request)
            .await
            .map_err(|error| tax_error_to_port_error(&context, owner_operation, error))?;
        validate_tax_result(
            &context,
            owner_operation,
            expected_currency.as_str(),
            customer_tax_exempt,
            &taxable_targets,
            &result,
        )?;
        Ok(result)
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

fn tax_port_error_kind(kind: &PortErrorKind) -> &'static str {
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

fn require_tax_calculation_policy(
    context: &PortContext,
    owner_operation: &'static str,
) -> Result<(), PortError> {
    context
        .require_policy(PortCallPolicy::read())
        .inspect_err(|error| {
            log_tax_calculation_policy_rejection(context, owner_operation, error);
        })
}

fn log_tax_calculation_policy_rejection(
    context: &PortContext,
    owner_operation: &'static str,
    error: &PortError,
) {
    let facts = tax_calculation_context_facts(context);
    match &error.kind {
        PortErrorKind::Unavailable | PortErrorKind::Timeout | PortErrorKind::InvariantViolation => {
            tracing::error!(
                owner = "rustok_tax",
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
                operation = owner_operation,
                code = %error.code,
                error_kind = tax_port_error_kind(&error.kind),
                error_message_present = !error.message.is_empty(),
                error_message_length = error.message.chars().count(),
                retryable = error.retryable,
                boundary = TAX_CALCULATION_PORT_BOUNDARY,
                "tax calculation policy admission failed with bounded diagnostics"
            );
        }
        _ => {
            tracing::warn!(
                owner = "rustok_tax",
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
                operation = owner_operation,
                code = %error.code,
                error_kind = tax_port_error_kind(&error.kind),
                error_message_present = !error.message.is_empty(),
                error_message_length = error.message.chars().count(),
                retryable = error.retryable,
                boundary = TAX_CALCULATION_PORT_BOUNDARY,
                "tax calculation policy admission was rejected with bounded diagnostics"
            );
        }
    }
}

fn validate_tax_request(
    context: &PortContext,
    owner_operation: &'static str,
    request: &TaxCalculationInput,
) -> Result<String, PortError> {
    let currency_code = normalize_currency_code(&request.currency_code).ok_or_else(|| {
        tax_request_error(
            context,
            owner_operation,
            "tax.currency_code_invalid",
            "request currency_code is not a three-letter alphabetic code",
        )
    })?;
    if request.policy.tax_rate < Decimal::ZERO {
        return Err(tax_request_error(
            context,
            owner_operation,
            "tax.negative_policy_rate",
            "tax policy rate is negative",
        ));
    }

    let mut country_codes = HashSet::new();
    for rule in &request.policy.country_rules {
        let country_code = rule.country_code.trim().to_ascii_uppercase();
        if country_code.len() != 2 || !country_code.chars().all(|ch| ch.is_ascii_alphabetic()) {
            return Err(tax_request_error(
                context,
                owner_operation,
                "tax.country_code_invalid",
                "tax country rule contains an invalid country code",
            ));
        }
        if rule.tax_rate < Decimal::ZERO {
            return Err(tax_request_error(
                context,
                owner_operation,
                "tax.negative_country_rate",
                "tax country rule rate is negative",
            ));
        }
        if !country_codes.insert(country_code.clone()) {
            return Err(tax_request_error(
                context,
                owner_operation,
                "tax.duplicate_country_rule",
                format!("duplicate tax country rule for {country_code}"),
            ));
        }
    }

    if request
        .taxable_amounts
        .iter()
        .any(|amount| amount.amount < Decimal::ZERO)
    {
        return Err(tax_request_error(
            context,
            owner_operation,
            "tax.negative_taxable_amount",
            "taxable amount is negative",
        ));
    }

    Ok(currency_code)
}

fn validate_tax_result(
    context: &PortContext,
    owner_operation: &'static str,
    expected_currency: &str,
    customer_tax_exempt: bool,
    taxable_targets: &HashSet<(Option<Uuid>, Option<Uuid>)>,
    result: &TaxCalculationResult,
) -> Result<(), PortError> {
    if result.tax_total < Decimal::ZERO {
        return Err(tax_result_error(
            context,
            owner_operation,
            "tax.negative_total",
            format!("tax provider returned negative total {}", result.tax_total),
        ));
    }
    if customer_tax_exempt && (result.tax_total != Decimal::ZERO || !result.lines.is_empty()) {
        return Err(tax_result_error(
            context,
            owner_operation,
            "tax.exempt_customer_charged",
            "tax provider returned charges for a tax-exempt customer",
        ));
    }

    let mut calculated_total = Decimal::ZERO;
    for line in &result.lines {
        validate_tax_line(
            context,
            owner_operation,
            expected_currency,
            taxable_targets,
            line,
        )?;
        calculated_total = calculated_total.checked_add(line.amount).ok_or_else(|| {
            tax_result_error(
                context,
                owner_operation,
                "tax.total_overflow",
                "tax provider line total overflowed Decimal",
            )
        })?;
    }

    if calculated_total != result.tax_total {
        return Err(tax_result_error(
            context,
            owner_operation,
            "tax.total_mismatch",
            format!(
                "tax provider total {} does not match line total {}",
                result.tax_total, calculated_total
            ),
        ));
    }

    Ok(())
}

fn validate_tax_line(
    context: &PortContext,
    owner_operation: &'static str,
    expected_currency: &str,
    taxable_targets: &HashSet<(Option<Uuid>, Option<Uuid>)>,
    line: &CalculatedTaxLine,
) -> Result<(), PortError> {
    if line.provider_id.trim().is_empty() || line.provider_id.len() > 64 {
        return Err(tax_result_error(
            context,
            owner_operation,
            "tax.provider_id_invalid",
            format!(
                "tax provider returned invalid provider_id {:?}",
                line.provider_id
            ),
        ));
    }
    if line.rate < Decimal::ZERO || line.amount < Decimal::ZERO {
        return Err(tax_result_error(
            context,
            owner_operation,
            "tax.negative_line",
            format!(
                "tax provider returned negative line rate {} or amount {}",
                line.rate, line.amount
            ),
        ));
    }
    let line_currency = normalize_currency_code(&line.currency_code).ok_or_else(|| {
        tax_result_error(
            context,
            owner_operation,
            "tax.currency_code_invalid",
            format!(
                "tax provider returned invalid currency {:?}",
                line.currency_code
            ),
        )
    })?;
    if line_currency != expected_currency {
        return Err(tax_result_error(
            context,
            owner_operation,
            "tax.currency_mismatch",
            format!("tax provider returned currency {line_currency}, expected {expected_currency}"),
        ));
    }
    if !taxable_targets.contains(&(line.line_item_id, line.shipping_option_id)) {
        return Err(tax_result_error(
            context,
            owner_operation,
            "tax.unknown_taxable_target",
            format!(
                "tax provider returned unknown line_item_id {:?} and shipping_option_id {:?}",
                line.line_item_id, line.shipping_option_id
            ),
        ));
    }
    Ok(())
}

fn normalize_currency_code(value: &str) -> Option<String> {
    let normalized = value.trim().to_ascii_uppercase();
    (normalized.len() == 3 && normalized.chars().all(|ch| ch.is_ascii_alphabetic()))
        .then_some(normalized)
}

fn tax_request_error(
    context: &PortContext,
    owner_operation: &'static str,
    code: &'static str,
    detail: impl std::fmt::Display,
) -> PortError {
    let detail = detail.to_string();
    let facts = tax_calculation_context_facts(context);
    tracing::warn!(
        owner = "rustok_tax",
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
        operation = owner_operation,
        code,
        detail_present = !detail.trim().is_empty(),
        detail_length = detail.chars().count(),
        boundary = TAX_CALCULATION_PORT_BOUNDARY,
        "tax request validation failed"
    );
    PortError::validation(code, detail)
}

fn tax_result_error(
    context: &PortContext,
    owner_operation: &'static str,
    code: &'static str,
    detail: impl std::fmt::Display,
) -> PortError {
    let detail = detail.to_string();
    let facts = tax_calculation_context_facts(context);
    tracing::error!(
        owner = "rustok_tax",
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
        operation = owner_operation,
        code,
        detail_present = !detail.trim().is_empty(),
        detail_length = detail.chars().count(),
        boundary = TAX_CALCULATION_PORT_BOUNDARY,
        "tax provider result violated the owner contract"
    );
    PortError::invariant_violation(code, detail)
}

fn tax_error_to_port_error(
    context: &PortContext,
    owner_operation: &'static str,
    error: TaxError,
) -> PortError {
    match error {
        TaxError::Validation(message) => {
            let facts = tax_calculation_context_facts(context);
            tracing::warn!(
                owner = "rustok_tax",
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
                operation = owner_operation,
                code = "tax.validation",
                validation_message_present = !message.trim().is_empty(),
                validation_message_length = message.chars().count(),
                boundary = TAX_CALCULATION_PORT_BOUNDARY,
                "tax owner validation failed"
            );
            PortError::validation("tax.validation", message)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{CalculatedTaxLine, TaxCalculationResult};
    use serde_json::Value;

    fn test_context() -> PortContext {
        PortContext::new(
            Uuid::new_v4().to_string(),
            rustok_api::PortActor::service("tax-port-test"),
            "en",
            "tax-port-test",
        )
    }

    #[test]
    fn rejects_symbolic_currency_and_negative_taxable_amount() {
        let request = TaxCalculationInput {
            currency_code: "12$".to_string(),
            channel_id: None,
            customer_tax_exempt: false,
            policy: crate::TaxPolicySnapshot {
                provider_id: None,
                channel_provider_id: None,
                country_code: None,
                tax_rate: Decimal::ZERO,
                tax_included: false,
                country_rules: Vec::new(),
            },
            taxable_amounts: vec![crate::TaxableAmount {
                line_item_id: None,
                shipping_option_id: None,
                item_tax_class: None,
                shipping_tax_class: None,
                description: None,
                amount: -Decimal::ONE,
            }],
        };
        assert!(validate_tax_request(&test_context(), "test", &request).is_err());
    }

    #[test]
    fn rejects_result_total_and_currency_mismatch() {
        let target = (Some(Uuid::new_v4()), None);
        let targets = HashSet::from([target]);
        let result = TaxCalculationResult {
            tax_total: Decimal::new(2, 0),
            tax_included: false,
            lines: vec![CalculatedTaxLine {
                line_item_id: target.0,
                shipping_option_id: target.1,
                description: None,
                provider_id: "provider".to_string(),
                rate: Decimal::new(10, 0),
                amount: Decimal::ONE,
                currency_code: "EUR".to_string(),
                metadata: Value::Null,
            }],
        };
        assert!(
            validate_tax_result(&test_context(), "test", "USD", false, &targets, &result).is_err()
        );
    }

    #[test]
    fn accepts_consistent_result() {
        let target = (Some(Uuid::new_v4()), None);
        let targets = HashSet::from([target]);
        let result = TaxCalculationResult {
            tax_total: Decimal::ONE,
            tax_included: false,
            lines: vec![CalculatedTaxLine {
                line_item_id: target.0,
                shipping_option_id: target.1,
                description: None,
                provider_id: "provider".to_string(),
                rate: Decimal::new(10, 0),
                amount: Decimal::ONE,
                currency_code: "USD".to_string(),
                metadata: Value::Null,
            }],
        };
        assert!(
            validate_tax_result(&test_context(), "test", "USD", false, &targets, &result).is_ok()
        );
    }
}
