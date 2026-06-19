use super::*;

#[tokio::test]
async fn cart_add_line_item_rejects_unknown_tax_provider_id_on_region() {
    let (db, cart_service, _, _) = setup().await;
    let tenant_id = Uuid::new_v4();
    seed_tenant_context(&db, tenant_id).await;

    let region = RegionService::new(db.clone())
        .create_region(
            tenant_id,
            CreateRegionInput {
                translations: vec![RegionTranslationInput {
                    locale: "en".to_string(),
                    name: "Tax Provider Region".to_string(),
                }],
                currency_code: "usd".to_string(),
                tax_provider_id: Some("external_tax".to_string()),
                tax_rate: Decimal::from_str("20.00").expect("valid decimal"),
                tax_included: false,
                country_tax_policies: None,
                countries: vec!["us".to_string()],
                metadata: serde_json::json!({ "source": "unknown-tax-provider-test" }),
            },
        )
        .await
        .unwrap();

    let cart = cart_service
        .create_cart(
            tenant_id,
            CreateCartInput {
                customer_id: None,
                email: Some("buyer@example.com".to_string()),
                region_id: Some(region.id),
                country_code: Some("us".to_string()),
                locale_code: Some("en".to_string()),
                selected_shipping_option_id: None,
                currency_code: "usd".to_string(),
                metadata: serde_json::json!({ "source": "unknown-tax-provider-test" }),
            },
        )
        .await
        .unwrap();

    let error = cart_service
        .add_line_item(
            tenant_id,
            cart.id,
            AddCartLineItemInput {
                product_id: None,
                variant_id: None,
                shipping_profile_slug: None,
                sku: Some("CHK-TAX-1".to_string()),
                title: "Tax Provider Product".to_string(),
                quantity: 1,
                unit_price: Decimal::from_str("25.00").expect("valid decimal"),
                metadata: serde_json::json!({}),
            },
        )
        .await
        .expect_err("unknown tax provider should be rejected");

    match error {
        rustok_cart::CartError::Tax(inner) => {
            assert!(inner
                .to_string()
                .contains("unknown tax provider_id: external_tax"));
        }
        other => panic!("unexpected error: {other}"),
    }
}

#[tokio::test]
async fn cart_add_line_item_prefers_country_tax_policy_over_region_baseline() {
    let (db, cart_service, _, _) = setup().await;
    let tenant_id = Uuid::new_v4();
    seed_tenant_context(&db, tenant_id).await;

    let region = RegionService::new(db.clone())
        .create_region(
            tenant_id,
            CreateRegionInput {
                translations: vec![RegionTranslationInput {
                    locale: "en".to_string(),
                    name: "Country Tax Region".to_string(),
                }],
                currency_code: "usd".to_string(),
                tax_provider_id: None,
                tax_rate: Decimal::from_str("20.00").expect("valid decimal"),
                tax_included: false,
                country_tax_policies: Some(vec![RegionCountryTaxPolicyInput {
                    country_code: "de".to_string(),
                    tax_rate: Decimal::from_str("7.00").expect("valid decimal"),
                    tax_included: true,
                }]),
                countries: vec!["de".to_string(), "fr".to_string()],
                metadata: serde_json::json!({ "source": "country-tax-policy-test" }),
            },
        )
        .await
        .unwrap();

    let cart = cart_service
        .create_cart(
            tenant_id,
            CreateCartInput {
                customer_id: None,
                email: Some("buyer@example.com".to_string()),
                region_id: Some(region.id),
                country_code: Some("de".to_string()),
                locale_code: Some("de".to_string()),
                selected_shipping_option_id: None,
                currency_code: "usd".to_string(),
                metadata: serde_json::json!({ "source": "country-tax-policy-test" }),
            },
        )
        .await
        .unwrap();

    let cart = cart_service
        .add_line_item(
            tenant_id,
            cart.id,
            AddCartLineItemInput {
                product_id: None,
                variant_id: None,
                shipping_profile_slug: None,
                sku: Some("CHK-TAX-DE".to_string()),
                title: "Country Tax Product".to_string(),
                quantity: 1,
                unit_price: Decimal::from_str("107.00").expect("valid decimal"),
                metadata: serde_json::json!({}),
            },
        )
        .await
        .expect("country-specific tax policy should be applied");

    assert_eq!(cart.tax_total, Decimal::from_str("7.00").unwrap());
    assert_eq!(cart.tax_lines.len(), 1);
    assert_eq!(cart.tax_lines[0].rate, Decimal::from_str("7.00").unwrap());
    assert_eq!(
        cart.tax_lines[0].metadata["country_code"],
        serde_json::json!("DE")
    );
    assert_eq!(
        cart.tax_lines[0].metadata["policy_scope"],
        serde_json::json!("country")
    );
}
