use async_trait::async_trait;
use rust_decimal::Decimal;
use serde_json::{json, Value};
use std::{collections::HashMap, sync::Arc};
use uuid::Uuid;

use crate::error::{TaxError, TaxResult};

pub const REGION_DEFAULT_TAX_PROVIDER_ID: &str = "region_default";

#[derive(Clone, Debug)]
pub struct TaxPolicySnapshot {
    pub provider_id: Option<String>,
    pub country_code: Option<String>,
    pub tax_rate: Decimal,
    pub tax_included: bool,
    pub country_rules: Vec<TaxPolicyCountryRule>,
}

#[derive(Clone, Debug)]
pub struct TaxPolicyCountryRule {
    pub country_code: String,
    pub tax_rate: Decimal,
    pub tax_included: bool,
}

#[derive(Clone, Debug)]
pub struct TaxableAmount {
    pub line_item_id: Option<Uuid>,
    pub shipping_option_id: Option<Uuid>,
    pub description: Option<String>,
    pub amount: Decimal,
}

#[derive(Clone, Debug)]
pub struct TaxCalculationInput {
    pub currency_code: String,
    pub policy: TaxPolicySnapshot,
    pub taxable_amounts: Vec<TaxableAmount>,
}

#[derive(Clone, Debug)]
pub struct CalculatedTaxLine {
    pub line_item_id: Option<Uuid>,
    pub shipping_option_id: Option<Uuid>,
    pub description: Option<String>,
    pub provider_id: String,
    pub rate: Decimal,
    pub amount: Decimal,
    pub currency_code: String,
    pub metadata: Value,
}

#[derive(Clone, Debug)]
pub struct TaxCalculationResult {
    pub tax_total: Decimal,
    pub tax_included: bool,
    pub lines: Vec<CalculatedTaxLine>,
}

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
            );
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
        let provider_id = normalize_provider_id(input.policy.provider_id.as_deref())?
            .unwrap_or_else(|| REGION_DEFAULT_TAX_PROVIDER_ID.to_string());
        let provider = self.providers.get(&provider_id).ok_or_else(|| {
            TaxError::Validation(format!("unknown tax provider_id: {provider_id}"))
        })?;
        provider.calculate(input).await
    }
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

fn normalize_description(value: Option<String>) -> Option<String> {
    let normalized = value?.trim().to_ascii_lowercase();
    if normalized.is_empty() {
        None
    } else {
        Some(normalized)
    }
}

#[derive(Clone, Debug)]
struct ResolvedTaxPolicy {
    country_code: Option<String>,
    tax_rate: Decimal,
    tax_included: bool,
    policy_scope: &'static str,
}

fn resolve_effective_policy(policy: &TaxPolicySnapshot) -> TaxResult<ResolvedTaxPolicy> {
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

fn calculate_tax_amount(amount: Decimal, rate: Decimal, tax_included: bool) -> Decimal {
    let hundred = Decimal::from(100);
    if tax_included {
        (amount - (amount / (Decimal::ONE + rate / hundred))).round_dp(2)
    } else {
        (amount * rate / hundred).round_dp(2)
    }
}

#[cfg(test)]
mod tests {
    use rust_decimal::Decimal;
    use serde_json::json;
    use uuid::Uuid;

    use super::{
        RegionTaxProvider, TaxCalculationInput, TaxPolicyCountryRule, TaxPolicySnapshot,
        TaxProvider, TaxableAmount, REGION_DEFAULT_TAX_PROVIDER_ID,
    };

    #[tokio::test]
    async fn region_provider_returns_provider_id_and_tax_lines() {
        let provider = RegionTaxProvider;
        let line_item_id = Uuid::new_v4();
        let shipping_option_id = Uuid::new_v4();
        let result = provider
            .calculate(TaxCalculationInput {
                currency_code: "eur".to_string(),
                policy: TaxPolicySnapshot {
                    provider_id: None,
                    country_code: None,
                    tax_rate: Decimal::from(20),
                    tax_included: false,
                    country_rules: Vec::new(),
                },
                taxable_amounts: vec![
                    TaxableAmount {
                        line_item_id: Some(line_item_id),
                        shipping_option_id: None,
                        description: Some("line_item".to_string()),
                        amount: Decimal::from(100),
                    },
                    TaxableAmount {
                        line_item_id: None,
                        shipping_option_id: Some(shipping_option_id),
                        description: Some("shipping".to_string()),
                        amount: Decimal::from(10),
                    },
                ],
            })
            .await
            .expect("tax calculation should succeed");

        assert_eq!(result.tax_total, Decimal::from(22));
        assert_eq!(result.lines.len(), 2);
        assert!(result
            .lines
            .iter()
            .all(|line| line.provider_id == REGION_DEFAULT_TAX_PROVIDER_ID));
        assert!(result
            .lines
            .iter()
            .any(|line| line.line_item_id == Some(line_item_id)));
        assert!(result
            .lines
            .iter()
            .any(|line| line.shipping_option_id == Some(shipping_option_id)));
    }

    #[tokio::test]
    async fn tax_service_rejects_unknown_provider() {
        let service = super::TaxService::new();
        let error = service
            .calculate(TaxCalculationInput {
                currency_code: "usd".to_string(),
                policy: TaxPolicySnapshot {
                    provider_id: Some("external_tax".to_string()),
                    country_code: None,
                    tax_rate: Decimal::from(10),
                    tax_included: false,
                    country_rules: Vec::new(),
                },
                taxable_amounts: vec![TaxableAmount {
                    line_item_id: None,
                    shipping_option_id: None,
                    description: Some("line_item".to_string()),
                    amount: Decimal::from(10),
                }],
            })
            .await
            .expect_err("unknown provider should be rejected");

        assert!(error
            .to_string()
            .contains("unknown tax provider_id: external_tax"));
    }

    #[tokio::test]
    async fn region_provider_prefers_country_rule_over_region_baseline() {
        let provider = RegionTaxProvider;
        let result = provider
            .calculate(TaxCalculationInput {
                currency_code: "eur".to_string(),
                policy: TaxPolicySnapshot {
                    provider_id: None,
                    country_code: Some("de".to_string()),
                    tax_rate: Decimal::from(20),
                    tax_included: false,
                    country_rules: vec![TaxPolicyCountryRule {
                        country_code: "DE".to_string(),
                        tax_rate: Decimal::from(7),
                        tax_included: true,
                    }],
                },
                taxable_amounts: vec![TaxableAmount {
                    line_item_id: None,
                    shipping_option_id: None,
                    description: Some("line_item".to_string()),
                    amount: Decimal::from(107),
                }],
            })
            .await
            .expect("tax calculation should succeed");

        assert_eq!(result.tax_total, Decimal::from(7));
        assert!(result.tax_included);
        assert_eq!(result.lines[0].metadata["country_code"], json!("DE"));
        assert_eq!(result.lines[0].metadata["policy_scope"], json!("country"));
    }
}
