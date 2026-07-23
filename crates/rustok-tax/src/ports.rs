use async_trait::async_trait;
use rust_decimal::Decimal;
use rustok_api::{PortCallPolicy, PortContext, PortError};
use std::{collections::HashSet, sync::Arc};
use uuid::Uuid;

use crate::{CalculatedTaxLine, TaxCalculationInput, TaxCalculationResult, TaxError};

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
        context.require_policy(PortCallPolicy::read())?;
        let owner_operation = "calculate_tax";
        let expected_currency = validate_tax_request(&context, owner_operation, &request)?;
        let customer_tax_exempt = request.customer_tax_exempt;
        let taxable_targets = request
            .taxable_amounts
            .iter()
            .map(|amount| (amount.line_item_id, amount.shipping_option_id))
            .collect::<HashSet<_>>();

        let result = self.calculate(request).await.map_err(|error| {
            tax_error_to_port_error(&context, owner_operation, error)
        })?;
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
            format!("tax provider returned invalid provider_id {:?}", line.provider_id),
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
            format!(
                "tax provider returned currency {line_currency}, expected {expected_currency}"
            ),
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
    tracing::warn!(
        detail = %detail,
        correlation_id = %context.correlation_id,
        tenant_id = %context.tenant_id,
        operation = owner_operation,
        code,
        "tax request validation failed"
    );
    PortError::validation(code, "tax request is invalid")
}

fn tax_result_error(
    context: &PortContext,
    owner_operation: &'static str,
    code: &'static str,
    detail: impl std::fmt::Display,
) -> PortError {
    tracing::error!(
        detail = %detail,
        correlation_id = %context.correlation_id,
        tenant_id = %context.tenant_id,
        operation = owner_operation,
        code,
        "tax provider result violated the owner contract"
    );
    PortError::invariant_violation(code, "tax calculation result is invalid")
}

fn tax_error_to_port_error(
    context: &PortContext,
    owner_operation: &'static str,
    error: TaxError,
) -> PortError {
    match error {
        TaxError::Validation(message) => {
            tracing::warn!(
                error = %message,
                correlation_id = %context.correlation_id,
                tenant_id = %context.tenant_id,
                operation = owner_operation,
                code = "tax.validation",
                "tax owner validation failed"
            );
            PortError::validation("tax.validation", "tax request is invalid")
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
            validate_tax_result(&test_context(), "test", "USD", false, &targets, &result)
                .is_err()
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
            validate_tax_result(&test_context(), "test", "USD", false, &targets, &result)
                .is_ok()
        );
    }
}
