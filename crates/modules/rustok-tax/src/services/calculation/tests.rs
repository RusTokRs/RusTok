use rust_decimal::Decimal;
use serde_json::json;
use uuid::Uuid;

use crate::dto::{TaxPolicyCountryRule, TaxableAmount};

use super::{
    REGION_DEFAULT_TAX_PROVIDER_ID, RegionTaxProvider, TaxCalculationInput, TaxPolicySnapshot,
    TaxProvider, TaxService,
};

#[tokio::test]
async fn region_provider_returns_provider_id_and_tax_lines() {
    let provider = RegionTaxProvider;
    let line_item_id = Uuid::new_v4();
    let shipping_option_id = Uuid::new_v4();
    let result = provider
        .calculate(TaxCalculationInput {
            currency_code: "eur".to_string(),
            channel_id: None,
            policy: TaxPolicySnapshot {
                provider_id: None,
                channel_provider_id: None,
                country_code: None,
                tax_rate: Decimal::from(20),
                tax_included: false,
                country_rules: Vec::new(),
            },
            taxable_amounts: vec![
                TaxableAmount {
                    line_item_id: Some(line_item_id),
                    shipping_option_id: None,
                    item_tax_class: Some("standard".to_string()),
                    shipping_tax_class: None,
                    description: Some("line_item".to_string()),
                    amount: Decimal::from(100),
                },
                TaxableAmount {
                    line_item_id: None,
                    shipping_option_id: Some(shipping_option_id),
                    item_tax_class: None,
                    shipping_tax_class: Some("default_shipping".to_string()),
                    description: Some("shipping".to_string()),
                    amount: Decimal::from(10),
                },
            ],
            customer_tax_exempt: false,
        })
        .await
        .expect("tax calculation should succeed");

    assert_eq!(result.tax_total, Decimal::from(22));
    assert_eq!(result.lines.len(), 2);
    assert!(
        result
            .lines
            .iter()
            .all(|line| line.provider_id == REGION_DEFAULT_TAX_PROVIDER_ID)
    );
    assert!(
        result
            .lines
            .iter()
            .any(|line| line.line_item_id == Some(line_item_id))
    );
    assert!(
        result
            .lines
            .iter()
            .any(|line| line.shipping_option_id == Some(shipping_option_id))
    );
}

#[tokio::test]
async fn tax_service_rejects_unknown_provider() {
    let service = super::TaxService::new();
    let error = service
        .calculate(TaxCalculationInput {
            currency_code: "usd".to_string(),
            channel_id: None,
            policy: TaxPolicySnapshot {
                provider_id: Some("external_tax".to_string()),
                channel_provider_id: None,
                country_code: None,
                tax_rate: Decimal::from(10),
                tax_included: false,
                country_rules: Vec::new(),
            },
            taxable_amounts: vec![TaxableAmount {
                line_item_id: None,
                shipping_option_id: None,
                item_tax_class: None,
                shipping_tax_class: None,
                description: Some("line_item".to_string()),
                amount: Decimal::from(10),
            }],
            customer_tax_exempt: false,
        })
        .await
        .expect_err("unknown provider should be rejected");

    assert!(
        error
            .to_string()
            .contains("unknown tax provider_id: external_tax")
    );
}

#[tokio::test]
async fn tax_service_normalizes_channel_provider_id_before_resolution() {
    let service = super::TaxService::new();
    let result = service
        .calculate(TaxCalculationInput {
            currency_code: "usd".to_string(),
            channel_id: Some(Uuid::new_v4()),
            policy: TaxPolicySnapshot {
                provider_id: Some("external_tax".to_string()),
                channel_provider_id: Some("  REGION_DEFAULT  ".to_string()),
                country_code: None,
                tax_rate: Decimal::from(10),
                tax_included: false,
                country_rules: Vec::new(),
            },
            taxable_amounts: vec![TaxableAmount {
                line_item_id: None,
                shipping_option_id: None,
                item_tax_class: None,
                shipping_tax_class: None,
                description: Some("line_item".to_string()),
                amount: Decimal::from(10),
            }],
            customer_tax_exempt: false,
        })
        .await
        .expect("normalized channel provider should be used");

    assert_eq!(result.lines.len(), 1);
    assert_eq!(result.lines[0].provider_id, REGION_DEFAULT_TAX_PROVIDER_ID);
}

#[tokio::test]
async fn tax_service_prefers_channel_provider_id_over_region_provider_id() {
    let service = super::TaxService::new();
    let error = service
        .calculate(TaxCalculationInput {
            currency_code: "usd".to_string(),
            channel_id: Some(Uuid::new_v4()),
            policy: TaxPolicySnapshot {
                provider_id: Some("region_default".to_string()),
                channel_provider_id: Some("external_tax".to_string()),
                country_code: None,
                tax_rate: Decimal::from(10),
                tax_included: false,
                country_rules: Vec::new(),
            },
            taxable_amounts: vec![TaxableAmount {
                line_item_id: None,
                shipping_option_id: None,
                item_tax_class: None,
                shipping_tax_class: None,
                description: Some("line_item".to_string()),
                amount: Decimal::from(10),
            }],
            customer_tax_exempt: false,
        })
        .await
        .expect_err("unknown channel provider should be rejected");

    assert!(
        error
            .to_string()
            .contains("unknown tax provider_id: external_tax")
    );
}

#[tokio::test]
async fn tax_service_ignores_channel_provider_without_channel_context() {
    let service = super::TaxService::new();
    let result = service
        .calculate(TaxCalculationInput {
            currency_code: "usd".to_string(),
            channel_id: None,
            policy: TaxPolicySnapshot {
                provider_id: Some("region_default".to_string()),
                channel_provider_id: Some("external_tax".to_string()),
                country_code: None,
                tax_rate: Decimal::from(10),
                tax_included: false,
                country_rules: Vec::new(),
            },
            taxable_amounts: vec![TaxableAmount {
                line_item_id: None,
                shipping_option_id: None,
                item_tax_class: None,
                shipping_tax_class: None,
                description: Some("line_item".to_string()),
                amount: Decimal::from(10),
            }],
            customer_tax_exempt: false,
        })
        .await
        .expect("region provider should be used when channel context is absent");

    assert_eq!(result.lines.len(), 1);
    assert_eq!(result.lines[0].provider_id, REGION_DEFAULT_TAX_PROVIDER_ID);
}

#[tokio::test]
async fn region_provider_snapshots_channel_id_metadata() {
    let provider = RegionTaxProvider;
    let channel_id = Uuid::new_v4();
    let result = provider
        .calculate(TaxCalculationInput {
            currency_code: "usd".to_string(),
            channel_id: Some(channel_id),
            policy: TaxPolicySnapshot {
                provider_id: None,
                channel_provider_id: None,
                country_code: None,
                tax_rate: Decimal::from(10),
                tax_included: false,
                country_rules: Vec::new(),
            },
            taxable_amounts: vec![TaxableAmount {
                line_item_id: None,
                shipping_option_id: None,
                item_tax_class: None,
                shipping_tax_class: None,
                description: Some("line_item".to_string()),
                amount: Decimal::from(10),
            }],
            customer_tax_exempt: false,
        })
        .await
        .expect("tax calculation should succeed");

    assert_eq!(result.lines.len(), 1);
    assert_eq!(
        result.lines[0].metadata["channel_id"],
        json!(channel_id.to_string())
    );

    let without_channel = provider
        .calculate(TaxCalculationInput {
            currency_code: "usd".to_string(),
            channel_id: None,
            policy: TaxPolicySnapshot {
                provider_id: None,
                channel_provider_id: None,
                country_code: None,
                tax_rate: Decimal::from(10),
                tax_included: false,
                country_rules: Vec::new(),
            },
            taxable_amounts: vec![TaxableAmount {
                line_item_id: None,
                shipping_option_id: None,
                item_tax_class: None,
                shipping_tax_class: None,
                description: Some("line_item".to_string()),
                amount: Decimal::from(10),
            }],
            customer_tax_exempt: false,
        })
        .await
        .expect("tax calculation should succeed without channel");

    assert_eq!(without_channel.lines.len(), 1);
    assert_eq!(without_channel.lines[0].metadata["channel_id"], json!(null));
}

#[tokio::test]
async fn region_provider_prefers_country_rule_over_region_baseline() {
    let provider = RegionTaxProvider;
    let result = provider
        .calculate(TaxCalculationInput {
            currency_code: "eur".to_string(),
            channel_id: None,
            policy: TaxPolicySnapshot {
                provider_id: None,
                channel_provider_id: None,
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
                item_tax_class: None,
                shipping_tax_class: None,
                description: Some("line_item".to_string()),
                amount: Decimal::from(107),
            }],
            customer_tax_exempt: false,
        })
        .await
        .expect("tax calculation should succeed");

    assert_eq!(result.tax_total, Decimal::from(7));
    assert!(result.tax_included);
    assert_eq!(result.lines[0].metadata["country_code"], json!("DE"));
    assert_eq!(result.lines[0].metadata["policy_scope"], json!("country"));
}

#[tokio::test]
async fn region_provider_returns_empty_result_for_tax_exempt_customer() {
    let provider = RegionTaxProvider;
    let result = provider
        .calculate(TaxCalculationInput {
            currency_code: "usd".to_string(),
            channel_id: None,
            customer_tax_exempt: true,
            policy: TaxPolicySnapshot {
                provider_id: None,
                channel_provider_id: None,
                country_code: None,
                tax_rate: Decimal::from(20),
                tax_included: true,
                country_rules: Vec::new(),
            },
            taxable_amounts: vec![TaxableAmount {
                line_item_id: Some(Uuid::new_v4()),
                shipping_option_id: None,
                item_tax_class: Some("standard".to_string()),
                shipping_tax_class: None,
                description: Some("line_item".to_string()),
                amount: Decimal::from(100),
            }],
        })
        .await
        .expect("tax-exempt calculation should succeed");

    assert_eq!(result.tax_total, Decimal::ZERO);
    assert!(!result.tax_included);
    assert!(result.lines.is_empty());
}

async fn calculate_single_line(currency_code: &str, amount: Decimal) -> Decimal {
    RegionTaxProvider
        .calculate(TaxCalculationInput {
            currency_code: currency_code.to_string(),
            channel_id: None,
            policy: TaxPolicySnapshot {
                provider_id: None,
                channel_provider_id: None,
                country_code: None,
                tax_rate: Decimal::from(10),
                tax_included: false,
                country_rules: Vec::new(),
            },
            taxable_amounts: vec![TaxableAmount {
                line_item_id: Some(Uuid::new_v4()),
                shipping_option_id: None,
                item_tax_class: None,
                shipping_tax_class: None,
                description: None,
                amount,
            }],
            customer_tax_exempt: false,
        })
        .await
        .expect("tax calculation should succeed")
        .tax_total
}

#[tokio::test]
async fn tax_amount_is_rounded_at_currency_precision_half_away_from_zero() {
    // 10% of 105 JPY is 10.5: a zero-decimal currency must round to whole yen, half away from zero.
    assert_eq!(
        calculate_single_line("JPY", Decimal::from(105)).await,
        Decimal::from(11)
    );
    // 10% of 1.25 USD is 0.125: half away from zero gives 0.13 (banker's rounding gave 0.12).
    assert_eq!(
        calculate_single_line("USD", Decimal::new(125, 2)).await,
        Decimal::new(13, 2)
    );
}

#[tokio::test]
async fn tax_service_ignores_nil_channel_id_when_selecting_provider() {
    let service = TaxService::new();
    let result = service
        .calculate(TaxCalculationInput {
            currency_code: "usd".to_string(),
            channel_id: Some(Uuid::nil()),
            policy: TaxPolicySnapshot {
                provider_id: None,
                channel_provider_id: Some("custom_channel_provider".to_string()),
                country_code: None,
                tax_rate: Decimal::from(10),
                tax_included: false,
                country_rules: Vec::new(),
            },
            taxable_amounts: vec![TaxableAmount {
                line_item_id: Some(Uuid::new_v4()),
                shipping_option_id: None,
                item_tax_class: None,
                shipping_tax_class: None,
                description: None,
                amount: Decimal::from(100),
            }],
            customer_tax_exempt: false,
        })
        .await
        .expect("should fall back to region_default instead of unknown channel provider");

    assert_eq!(result.lines[0].provider_id, REGION_DEFAULT_TAX_PROVIDER_ID);
    assert_eq!(result.lines[0].metadata["channel_id"], json!(null));
}
