use async_trait::async_trait;
use rust_decimal::Decimal;
use serde_json::json;
use std::{collections::HashMap, sync::Arc};
use uuid::Uuid;

use crate::domain::calculation::{
    calculate_tax_amount, normalize_description, resolve_effective_policy,
};
use crate::dto::{CalculatedTaxLine, TaxCalculationInput, TaxCalculationResult, TaxPolicySnapshot};

use crate::error::{TaxError, TaxResult};

pub const REGION_DEFAULT_TAX_PROVIDER_ID: &str = "region_default";

#[async_trait]
pub trait TaxProvider: Send + Sync {
    fn provider_id(&self) -> &'static str;

    async fn calculate(&self, input: TaxCalculationInput) -> TaxResult<TaxCalculationResult>;
}

#[derive(Clone, Default)]
pub struct RegionTaxProvider;

#[async_trait]
impl TaxProvider for RegionTaxProvider {
    fn provider_id(&self) -> &'static str {
        REGION_DEFAULT_TAX_PROVIDER_ID
    }

    async fn calculate(&self, input: TaxCalculationInput) -> TaxResult<TaxCalculationResult> {
        if input.currency_code.trim().len() != 3 {
            return Err(TaxError::Validation(
                "currency_code must be a 3-letter code".to_string(),
            ));
        }

        if input.policy.tax_rate < Decimal::ZERO {
            return Err(TaxError::Validation(
                "tax_rate must be zero or greater".to_string(),
            ));
        }
        for rule in &input.policy.country_rules {
            if rule.tax_rate < Decimal::ZERO {
                return Err(TaxError::Validation(
                    "country tax policy tax_rate must be zero or greater".to_string(),
                ));
            }
        }

        let resolved_policy = resolve_effective_policy(&input.policy)?;
        if input.customer_tax_exempt {
            return Ok(TaxCalculationResult {
                tax_total: Decimal::ZERO,
                tax_included: false,
                lines: Vec::new(),
            });
        }

        let currency_code = input.currency_code.trim().to_ascii_uppercase();
        if resolved_policy.tax_rate <= Decimal::ZERO {
            return Ok(TaxCalculationResult {
                tax_total: Decimal::ZERO,
                tax_included: resolved_policy.tax_included,
                lines: Vec::new(),
            });
        }

        let mut tax_total = Decimal::ZERO;
        let mut lines = Vec::new();
        for amount in input.taxable_amounts {
            if amount.amount <= Decimal::ZERO {
                continue;
            }
            let line_tax = calculate_tax_amount(
                amount.amount,
                resolved_policy.tax_rate,
                resolved_policy.tax_included,
                currency_code.as_str(),
            )?;
            if line_tax <= Decimal::ZERO {
                continue;
            }
            tax_total += line_tax;
            lines.push(CalculatedTaxLine {
                line_item_id: amount.line_item_id,
                shipping_option_id: amount.shipping_option_id,
                description: normalize_description(amount.description),
                provider_id: self.provider_id().to_string(),
                rate: resolved_policy.tax_rate,
                amount: line_tax,
                currency_code: currency_code.clone(),
                metadata: json!({
                    "tax_included": resolved_policy.tax_included,
                    "country_code": resolved_policy.country_code,
                    "policy_scope": resolved_policy.policy_scope,
                    "channel_id": input.channel_id.map(|value| value.to_string()),
                    "customer_tax_exempt": input.customer_tax_exempt,
                    "item_tax_class": amount.item_tax_class,
                    "shipping_tax_class": amount.shipping_tax_class,
                }),
            });
        }

        Ok(TaxCalculationResult {
            tax_total,
            tax_included: resolved_policy.tax_included,
            lines,
        })
    }
}

#[derive(Clone, Default)]
pub struct TaxService {
    providers: HashMap<String, Arc<dyn TaxProvider>>,
}

impl TaxService {
    pub fn new() -> Self {
        Self::default().with_provider(RegionTaxProvider)
    }

    pub fn with_provider<P>(mut self, provider: P) -> Self
    where
        P: TaxProvider + 'static,
    {
        self.providers
            .insert(provider.provider_id().to_string(), Arc::new(provider));
        self
    }

    pub async fn calculate(&self, input: TaxCalculationInput) -> TaxResult<TaxCalculationResult> {
        let provider_id = resolve_provider_id(input.channel_id, &input.policy)?;
        let provider = self.providers.get(&provider_id).ok_or_else(|| {
            TaxError::Validation(format!("unknown tax provider_id: {provider_id}"))
        })?;
        provider.calculate(input).await
    }
}

fn resolve_provider_id(channel_id: Option<Uuid>, policy: &TaxPolicySnapshot) -> TaxResult<String> {
    if channel_id.is_some_and(|id| !id.is_nil())
        && let Some(channel_provider_id) =
            normalize_provider_id(policy.channel_provider_id.as_deref())?
    {
        return Ok(channel_provider_id);
    }
    if let Some(provider_id) = normalize_provider_id(policy.provider_id.as_deref())? {
        return Ok(provider_id);
    }
    Ok(REGION_DEFAULT_TAX_PROVIDER_ID.to_string())
}

fn normalize_provider_id(value: Option<&str>) -> TaxResult<Option<String>> {
    let Some(value) = value.map(str::trim).filter(|value| !value.is_empty()) else {
        return Ok(None);
    };
    let normalized = value.to_ascii_lowercase();
    if normalized.len() > 64 {
        return Err(TaxError::Validation(
            "tax provider_id must be at most 64 characters".to_string(),
        ));
    }
    if !normalized
        .chars()
        .all(|ch| ch.is_ascii_lowercase() || ch.is_ascii_digit() || ch == '_' || ch == '-')
    {
        return Err(TaxError::Validation(
            "tax provider_id must use lowercase ASCII, digits, underscore, or hyphen".to_string(),
        ));
    }
    Ok(Some(normalized))
}

#[cfg(test)]
mod tests;
