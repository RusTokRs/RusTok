use std::collections::HashMap;

use rust_decimal::Decimal;
use rustok_core::money;
use serde::{Deserialize, Serialize};

use crate::error::{TaxError, TaxResult};

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct TaxPolicySnapshot {
    pub provider_id: Option<String>,
    pub channel_provider_id: Option<String>,
    pub country_code: Option<String>,
    pub tax_rate: Decimal,
    pub tax_included: bool,
    pub country_rules: Vec<TaxPolicyCountryRule>,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct TaxPolicyCountryRule {
    pub country_code: String,
    pub tax_rate: Decimal,
    pub tax_included: bool,
}

pub(crate) fn normalize_description(value: Option<String>) -> Option<String> {
    let normalized = value?.trim().to_ascii_lowercase();
    if normalized.is_empty() {
        None
    } else {
        Some(normalized)
    }
}

#[derive(Clone, Debug)]
pub(crate) struct ResolvedTaxPolicy {
    pub(crate) country_code: Option<String>,
    pub(crate) tax_rate: Decimal,
    pub(crate) tax_included: bool,
    pub(crate) policy_scope: &'static str,
}

pub(crate) fn resolve_effective_policy(policy: &TaxPolicySnapshot) -> TaxResult<ResolvedTaxPolicy> {
    let requested_country_code = normalize_country_code(policy.country_code.as_deref())?;
    let mut rules = HashMap::new();
    for rule in &policy.country_rules {
        let country_code =
            normalize_country_code(Some(rule.country_code.as_str()))?.ok_or_else(|| {
                TaxError::Validation("country tax policy country_code is required".to_string())
            })?;
        if rules
            .insert(
                country_code.clone(),
                ResolvedTaxPolicy {
                    country_code: Some(country_code),
                    tax_rate: rule.tax_rate,
                    tax_included: rule.tax_included,
                    policy_scope: "country",
                },
            )
            .is_some()
        {
            return Err(TaxError::Validation(
                "duplicate country_code in tax policy".to_string(),
            ));
        }
    }
    if let Some(country_code) = requested_country_code {
        if let Some(policy) = rules.remove(&country_code) {
            return Ok(policy);
        }
        return Ok(ResolvedTaxPolicy {
            country_code: Some(country_code),
            tax_rate: policy.tax_rate,
            tax_included: policy.tax_included,
            policy_scope: "region",
        });
    }
    Ok(ResolvedTaxPolicy {
        country_code: None,
        tax_rate: policy.tax_rate,
        tax_included: policy.tax_included,
        policy_scope: "region",
    })
}

fn normalize_country_code(value: Option<&str>) -> TaxResult<Option<String>> {
    let Some(value) = value.map(str::trim).filter(|value| !value.is_empty()) else {
        return Ok(None);
    };
    let normalized = value.to_ascii_uppercase();
    if normalized.len() != 2 || !normalized.chars().all(|ch| ch.is_ascii_alphabetic()) {
        return Err(TaxError::Validation(
            "country_code must be a 2-letter code".to_string(),
        ));
    }
    Ok(Some(normalized))
}

/// Tax is rounded with the canonical currency rule (`rustok_core::money::round_to_currency`,
/// half away from zero at the currency's minor-unit exponent) so tax amounts stay convertible to
/// provider minor units for zero- and three-decimal currencies, not only for 2-decimal ones.
pub(crate) fn calculate_tax_amount(
    amount: Decimal,
    rate: Decimal,
    tax_included: bool,
    currency_code: &str,
) -> TaxResult<Decimal> {
    let hundred = Decimal::from(100);
    let raw_tax = if tax_included {
        rate.checked_div(hundred)
            .and_then(|fraction| Decimal::ONE.checked_add(fraction))
            .and_then(|divisor| amount.checked_div(divisor))
            .and_then(|net| amount.checked_sub(net))
    } else {
        amount
            .checked_mul(rate)
            .and_then(|product| product.checked_div(hundred))
    }
    .ok_or_else(|| TaxError::Validation("tax line exceeds the decimal range".to_string()))?;
    money::round_to_currency(raw_tax, currency_code)
        .map_err(|error| TaxError::Validation(error.to_string()))
}
