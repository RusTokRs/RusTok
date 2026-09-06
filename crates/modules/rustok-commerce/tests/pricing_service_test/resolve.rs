use super::*;

#[tokio::test]
async fn test_resolve_variant_price_prefers_exact_region_over_global() {
    let (_db, service, catalog) = setup().await;
    let tenant_id = Uuid::new_v4();
    let actor_id = Uuid::new_v4();
    let (_product_id, variant_id) = create_test_product(&catalog, tenant_id).await;
    let region_id = Uuid::new_v4();

    service
        .set_price(tenant_id, actor_id, variant_id, "USD", dec!(99.99), None)
        .await
        .unwrap();

    entities::price::ActiveModel {
        id: Set(Uuid::new_v4()),
        variant_id: Set(variant_id),
        price_list_id: Set(None),
        channel_id: Set(None),
        channel_slug: Set(None),
        currency_code: Set("USD".to_string()),
        region_id: Set(Some(region_id)),
        amount: Set(dec!(79.99)),
        compare_at_amount: Set(Some(dec!(99.99))),
        legacy_amount: Set(Some(7999)),
        legacy_compare_at_amount: Set(Some(9999)),
        min_quantity: Set(None),
        max_quantity: Set(None),
    }
    .insert(&_db)
    .await
    .unwrap();

    let resolved = service
        .resolve_variant_price(
            tenant_id,
            variant_id,
            rustok_pricing::PriceResolutionContext {
                currency_code: "usd".to_string(),
                region_id: Some(region_id),
                price_list_id: None,
                channel_id: None,
                channel_slug: None,
                quantity: Some(1),
            },
        )
        .await
        .unwrap()
        .expect("region price should resolve");

    assert_eq!(resolved.amount, dec!(79.99));
    assert_eq!(resolved.region_id, Some(region_id));
    assert!(resolved.on_sale);
}

#[tokio::test]
async fn test_resolve_variant_price_prefers_more_specific_quantity_tier() {
    let (_db, service, catalog) = setup().await;
    let tenant_id = Uuid::new_v4();
    let actor_id = Uuid::new_v4();
    let (_product_id, variant_id) = create_test_product(&catalog, tenant_id).await;

    service
        .set_price(tenant_id, actor_id, variant_id, "USD", dec!(100.00), None)
        .await
        .unwrap();

    entities::price::ActiveModel {
        id: Set(Uuid::new_v4()),
        variant_id: Set(variant_id),
        price_list_id: Set(None),
        channel_id: Set(None),
        channel_slug: Set(None),
        currency_code: Set("USD".to_string()),
        region_id: Set(None),
        amount: Set(dec!(90.00)),
        compare_at_amount: Set(None),
        legacy_amount: Set(Some(9000)),
        legacy_compare_at_amount: Set(None),
        min_quantity: Set(Some(5)),
        max_quantity: Set(Some(9)),
    }
    .insert(&_db)
    .await
    .unwrap();

    entities::price::ActiveModel {
        id: Set(Uuid::new_v4()),
        variant_id: Set(variant_id),
        price_list_id: Set(None),
        channel_id: Set(None),
        channel_slug: Set(None),
        currency_code: Set("USD".to_string()),
        region_id: Set(None),
        amount: Set(dec!(85.00)),
        compare_at_amount: Set(None),
        legacy_amount: Set(Some(8500)),
        legacy_compare_at_amount: Set(None),
        min_quantity: Set(Some(10)),
        max_quantity: Set(None),
    }
    .insert(&_db)
    .await
    .unwrap();

    let resolved = service
        .resolve_variant_price(
            tenant_id,
            variant_id,
            rustok_pricing::PriceResolutionContext {
                currency_code: "USD".to_string(),
                region_id: None,
                price_list_id: None,
                channel_id: None,
                channel_slug: None,
                quantity: Some(12),
            },
        )
        .await
        .unwrap()
        .expect("tiered price should resolve");

    assert_eq!(resolved.amount, dec!(85.00));
    assert_eq!(resolved.min_quantity, Some(10));
    assert_eq!(resolved.max_quantity, None);
}

#[tokio::test]
async fn test_resolve_variant_price_prefers_narrower_max_quantity_when_min_quantity_matches() {
    let (_db, service, catalog) = setup().await;
    let tenant_id = Uuid::new_v4();
    let actor_id = Uuid::new_v4();
    let (_product_id, variant_id) = create_test_product(&catalog, tenant_id).await;

    service
        .set_price(tenant_id, actor_id, variant_id, "USD", dec!(100.00), None)
        .await
        .unwrap();

    entities::price::ActiveModel {
        id: Set(Uuid::new_v4()),
        variant_id: Set(variant_id),
        price_list_id: Set(None),
        channel_id: Set(None),
        channel_slug: Set(None),
        currency_code: Set("USD".to_string()),
        region_id: Set(None),
        amount: Set(dec!(88.00)),
        compare_at_amount: Set(None),
        legacy_amount: Set(Some(8800)),
        legacy_compare_at_amount: Set(None),
        min_quantity: Set(Some(10)),
        max_quantity: Set(Some(20)),
    }
    .insert(&_db)
    .await
    .unwrap();

    entities::price::ActiveModel {
        id: Set(Uuid::new_v4()),
        variant_id: Set(variant_id),
        price_list_id: Set(None),
        channel_id: Set(None),
        channel_slug: Set(None),
        currency_code: Set("USD".to_string()),
        region_id: Set(None),
        amount: Set(dec!(86.00)),
        compare_at_amount: Set(None),
        legacy_amount: Set(Some(8600)),
        legacy_compare_at_amount: Set(None),
        min_quantity: Set(Some(10)),
        max_quantity: Set(Some(15)),
    }
    .insert(&_db)
    .await
    .unwrap();

    let resolved = service
        .resolve_variant_price(
            tenant_id,
            variant_id,
            rustok_pricing::PriceResolutionContext {
                currency_code: "USD".to_string(),
                region_id: None,
                price_list_id: None,
                channel_id: None,
                channel_slug: None,
                quantity: Some(12),
            },
        )
        .await
        .unwrap()
        .expect("narrower quantity window should win");

    assert_eq!(resolved.amount, dec!(86.00));
    assert_eq!(resolved.min_quantity, Some(10));
    assert_eq!(resolved.max_quantity, Some(15));
}

#[tokio::test]
async fn test_resolve_variant_price_falls_back_to_global_price_without_region_specific_match() {
    let (_db, service, catalog) = setup().await;
    let tenant_id = Uuid::new_v4();
    let actor_id = Uuid::new_v4();
    let (_product_id, variant_id) = create_test_product(&catalog, tenant_id).await;

    service
        .set_price(tenant_id, actor_id, variant_id, "USD", dec!(100.00), None)
        .await
        .unwrap();

    let resolved = service
        .resolve_variant_price(
            tenant_id,
            variant_id,
            rustok_pricing::PriceResolutionContext {
                currency_code: "USD".to_string(),
                region_id: Some(Uuid::new_v4()),
                price_list_id: None,
                channel_id: None,
                channel_slug: None,
                quantity: None,
            },
        )
        .await
        .unwrap()
        .expect("global price should resolve");

    assert_eq!(resolved.amount, dec!(100.00));
    assert_eq!(resolved.region_id, None);
}

#[tokio::test]
async fn test_resolve_variant_price_matches_channel_slug_without_channel_id() {
    let (_db, service, catalog) = setup().await;
    let tenant_id = Uuid::new_v4();
    let actor_id = Uuid::new_v4();
    let (_product_id, variant_id) = create_test_product(&catalog, tenant_id).await;

    service
        .set_price(tenant_id, actor_id, variant_id, "USD", dec!(100.00), None)
        .await
        .unwrap();
    service
        .set_prices(
            tenant_id,
            actor_id,
            variant_id,
            vec![rustok_commerce_foundation::dto::PriceInput {
                currency_code: "USD".to_string(),
                channel_id: None,
                channel_slug: Some("web-store".to_string()),
                amount: dec!(78.00),
                compare_at_amount: Some(dec!(100.00)),
            }],
        )
        .await
        .unwrap();

    let resolved = service
        .resolve_variant_price(
            tenant_id,
            variant_id,
            rustok_pricing::PriceResolutionContext {
                currency_code: "USD".to_string(),
                region_id: None,
                price_list_id: None,
                channel_id: None,
                channel_slug: Some("WEB-STORE".to_string()),
                quantity: Some(1),
            },
        )
        .await
        .unwrap()
        .expect("slug-scoped price should resolve without channel_id");

    assert_eq!(resolved.amount, dec!(78.00));
    assert_eq!(resolved.channel_id, None);
    assert_eq!(resolved.channel_slug.as_deref(), Some("web-store"));
    assert!(resolved.on_sale);
}

#[tokio::test]
async fn test_resolve_variant_price_prefers_channel_scoped_base_price() {
    let (_db, service, catalog) = setup().await;
    let tenant_id = Uuid::new_v4();
    let actor_id = Uuid::new_v4();
    let (_product_id, variant_id) = create_test_product(&catalog, tenant_id).await;
    let channel_id = Uuid::new_v4();

    service
        .set_price(tenant_id, actor_id, variant_id, "USD", dec!(100.00), None)
        .await
        .unwrap();
    service
        .set_prices(
            tenant_id,
            actor_id,
            variant_id,
            vec![rustok_commerce_foundation::dto::PriceInput {
                currency_code: "USD".to_string(),
                channel_id: Some(channel_id),
                channel_slug: Some("web-store".to_string()),
                amount: dec!(79.00),
                compare_at_amount: Some(dec!(100.00)),
            }],
        )
        .await
        .unwrap();

    let resolved = service
        .resolve_variant_price(
            tenant_id,
            variant_id,
            rustok_pricing::PriceResolutionContext {
                currency_code: "USD".to_string(),
                region_id: None,
                price_list_id: None,
                channel_id: Some(channel_id),
                channel_slug: Some("web-store".to_string()),
                quantity: Some(1),
            },
        )
        .await
        .unwrap()
        .expect("channel scoped price should resolve");

    assert_eq!(resolved.amount, dec!(79.00));
    assert_eq!(resolved.channel_id, Some(channel_id));
    assert_eq!(resolved.channel_slug.as_deref(), Some("web-store"));
    assert!(resolved.on_sale);
}

#[tokio::test]
async fn test_resolve_variant_price_does_not_leak_channel_scoped_price() {
    let (_db, service, catalog) = setup().await;
    let tenant_id = Uuid::new_v4();
    let actor_id = Uuid::new_v4();
    let (_product_id, variant_id) = create_test_product(&catalog, tenant_id).await;
    let channel_id = Uuid::new_v4();

    service
        .set_price(tenant_id, actor_id, variant_id, "USD", dec!(100.00), None)
        .await
        .unwrap();
    service
        .set_prices(
            tenant_id,
            actor_id,
            variant_id,
            vec![rustok_commerce_foundation::dto::PriceInput {
                currency_code: "USD".to_string(),
                channel_id: Some(channel_id),
                channel_slug: Some("web-store".to_string()),
                amount: dec!(79.00),
                compare_at_amount: Some(dec!(100.00)),
            }],
        )
        .await
        .unwrap();

    let resolved = service
        .resolve_variant_price(
            tenant_id,
            variant_id,
            rustok_pricing::PriceResolutionContext {
                currency_code: "USD".to_string(),
                region_id: None,
                price_list_id: None,
                channel_id: Some(Uuid::new_v4()),
                channel_slug: Some("mobile-app".to_string()),
                quantity: Some(1),
            },
        )
        .await
        .unwrap()
        .expect("global price should still resolve");

    assert_eq!(resolved.amount, dec!(100.00));
    assert_eq!(resolved.channel_id, None);
    assert_eq!(resolved.channel_slug, None);
}

#[tokio::test]
async fn test_resolve_variant_price_prefers_active_price_list_over_base_price() {
    let (db, service, catalog) = setup().await;
    let tenant_id = Uuid::new_v4();
    let actor_id = Uuid::new_v4();
    let (_product_id, variant_id) = create_test_product(&catalog, tenant_id).await;
    let price_list_id = create_price_list(&db, tenant_id, "active", None, None).await;

    service
        .set_price(tenant_id, actor_id, variant_id, "USD", dec!(100.00), None)
        .await
        .unwrap();

    entities::price::ActiveModel {
        id: Set(Uuid::new_v4()),
        variant_id: Set(variant_id),
        price_list_id: Set(Some(price_list_id)),
        channel_id: Set(None),
        channel_slug: Set(None),
        currency_code: Set("USD".to_string()),
        region_id: Set(None),
        amount: Set(dec!(80.00)),
        compare_at_amount: Set(Some(dec!(100.00))),
        legacy_amount: Set(Some(8000)),
        legacy_compare_at_amount: Set(Some(10000)),
        min_quantity: Set(None),
        max_quantity: Set(None),
    }
    .insert(&db)
    .await
    .unwrap();

    let resolved = service
        .resolve_variant_price(
            tenant_id,
            variant_id,
            rustok_pricing::PriceResolutionContext {
                currency_code: "USD".to_string(),
                region_id: None,
                price_list_id: Some(price_list_id),
                channel_id: None,
                channel_slug: None,
                quantity: Some(1),
            },
        )
        .await
        .unwrap()
        .expect("price list price should resolve");

    assert_eq!(resolved.amount, dec!(80.00));
    assert_eq!(resolved.price_list_id, Some(price_list_id));
    assert!(resolved.on_sale);
}

#[tokio::test]
async fn test_resolve_variant_price_applies_active_price_list_rule_without_override() {
    let (db, service, catalog) = setup().await;
    let tenant_id = Uuid::new_v4();
    let actor_id = Uuid::new_v4();
    let (_product_id, variant_id) = create_test_product(&catalog, tenant_id).await;
    let price_list_id = create_price_list(&db, tenant_id, "active", None, None).await;

    service
        .set_price(tenant_id, actor_id, variant_id, "USD", dec!(100.00), None)
        .await
        .unwrap();
    service
        .set_price_list_percentage_rule(tenant_id, actor_id, price_list_id, Some(dec!(15)))
        .await
        .unwrap();

    let resolved = service
        .resolve_variant_price(
            tenant_id,
            variant_id,
            rustok_pricing::PriceResolutionContext {
                currency_code: "USD".to_string(),
                region_id: None,
                price_list_id: Some(price_list_id),
                channel_id: None,
                channel_slug: None,
                quantity: Some(1),
            },
        )
        .await
        .unwrap()
        .expect("price list rule should resolve from base row");

    assert_eq!(resolved.amount, dec!(85.00));
    assert_eq!(resolved.compare_at_amount, Some(dec!(100.00)));
    assert_eq!(resolved.discount_percent, Some(dec!(15)));
    assert_eq!(resolved.price_list_id, Some(price_list_id));
    assert!(resolved.on_sale);
}

#[tokio::test]
async fn test_resolve_variant_price_rule_uses_channel_quantity_tier_and_rounds() {
    let (db, service, catalog) = setup().await;
    let tenant_id = Uuid::new_v4();
    let actor_id = Uuid::new_v4();
    let channel_id = Uuid::new_v4();
    let (_product_id, variant_id) = create_test_product(&catalog, tenant_id).await;
    let price_list_id = create_price_list_with_channel(
        &db,
        tenant_id,
        "active",
        None,
        None,
        Some(channel_id),
        Some("web-store"),
    )
    .await;

    service
        .set_price(tenant_id, actor_id, variant_id, "USD", dec!(100.00), None)
        .await
        .unwrap();
    service
        .set_price_tier_with_channel(
            tenant_id,
            actor_id,
            variant_id,
            "USD",
            dec!(19.99),
            None,
            Some(channel_id),
            Some("web-store".to_string()),
            Some(10),
            None,
        )
        .await
        .unwrap();
    service
        .set_price_list_percentage_rule(tenant_id, actor_id, price_list_id, Some(dec!(12.5)))
        .await
        .unwrap();

    let resolved = service
        .resolve_variant_price(
            tenant_id,
            variant_id,
            rustok_pricing::PriceResolutionContext {
                currency_code: "USD".to_string(),
                region_id: None,
                price_list_id: Some(price_list_id),
                channel_id: Some(channel_id),
                channel_slug: Some("web-store".to_string()),
                quantity: Some(12),
            },
        )
        .await
        .unwrap()
        .expect("price list rule should resolve from the channel quantity tier");

    assert_eq!(resolved.amount, dec!(17.49));
    assert_eq!(resolved.compare_at_amount, Some(dec!(19.99)));
    assert_eq!(resolved.discount_percent, Some(dec!(12.5)));
    assert_eq!(resolved.price_list_id, Some(price_list_id));
    assert_eq!(resolved.channel_id, Some(channel_id));
    assert_eq!(resolved.channel_slug.as_deref(), Some("web-store"));
    assert_eq!(resolved.min_quantity, Some(10));
    assert_eq!(resolved.max_quantity, None);
    assert!(resolved.on_sale);
}

#[tokio::test]
async fn test_resolve_variant_price_prefers_explicit_override_over_price_list_rule() {
    let (db, service, catalog) = setup().await;
    let tenant_id = Uuid::new_v4();
    let actor_id = Uuid::new_v4();
    let (_product_id, variant_id) = create_test_product(&catalog, tenant_id).await;
    let price_list_id = create_price_list(&db, tenant_id, "active", None, None).await;

    service
        .set_price(tenant_id, actor_id, variant_id, "USD", dec!(100.00), None)
        .await
        .unwrap();
    service
        .set_price_list_percentage_rule(tenant_id, actor_id, price_list_id, Some(dec!(15)))
        .await
        .unwrap();
    service
        .set_price_list_tier(
            tenant_id,
            actor_id,
            variant_id,
            price_list_id,
            "USD",
            dec!(70.00),
            Some(dec!(100.00)),
            None,
            None,
        )
        .await
        .unwrap();

    let resolved = service
        .resolve_variant_price(
            tenant_id,
            variant_id,
            rustok_pricing::PriceResolutionContext {
                currency_code: "USD".to_string(),
                region_id: None,
                price_list_id: Some(price_list_id),
                channel_id: None,
                channel_slug: None,
                quantity: Some(1),
            },
        )
        .await
        .unwrap()
        .expect("explicit override should win");

    assert_eq!(resolved.amount, dec!(70.00));
    assert_eq!(resolved.compare_at_amount, Some(dec!(100.00)));
    assert_eq!(resolved.price_list_id, Some(price_list_id));
}

#[tokio::test]
async fn test_resolve_variant_price_falls_back_to_base_after_clearing_price_list_rule() {
    let (db, service, catalog) = setup().await;
    let tenant_id = Uuid::new_v4();
    let actor_id = Uuid::new_v4();
    let (_product_id, variant_id) = create_test_product(&catalog, tenant_id).await;
    let price_list_id = create_price_list(&db, tenant_id, "active", None, None).await;

    service
        .set_price(tenant_id, actor_id, variant_id, "USD", dec!(100.00), None)
        .await
        .unwrap();
    service
        .set_price_list_percentage_rule(tenant_id, actor_id, price_list_id, Some(dec!(15)))
        .await
        .unwrap();
    service
        .set_price_list_percentage_rule(tenant_id, actor_id, price_list_id, None)
        .await
        .unwrap();

    let resolved = service
        .resolve_variant_price(
            tenant_id,
            variant_id,
            rustok_pricing::PriceResolutionContext {
                currency_code: "USD".to_string(),
                region_id: None,
                price_list_id: Some(price_list_id),
                channel_id: None,
                channel_slug: None,
                quantity: Some(1),
            },
        )
        .await
        .unwrap()
        .expect("base row should resolve after rule removal");

    assert_eq!(resolved.amount, dec!(100.00));
    assert_eq!(resolved.compare_at_amount, None);
    assert_eq!(resolved.discount_percent, None);
    assert_eq!(resolved.price_list_id, None);
    assert!(!resolved.on_sale);
}

#[tokio::test]
async fn test_resolve_variant_price_rejects_inactive_price_list_context() {
    let (db, service, catalog) = setup().await;
    let tenant_id = Uuid::new_v4();
    let (_product_id, variant_id) = create_test_product(&catalog, tenant_id).await;
    let price_list_id = create_price_list(&db, tenant_id, "draft", None, None).await;

    let result = service
        .resolve_variant_price(
            tenant_id,
            variant_id,
            rustok_pricing::PriceResolutionContext {
                currency_code: "USD".to_string(),
                region_id: None,
                price_list_id: Some(price_list_id),
                channel_id: None,
                channel_slug: None,
                quantity: Some(1),
            },
        )
        .await;

    assert!(result.is_err());
    match result.unwrap_err() {
        CommerceError::Validation(message) => {
            assert!(message.contains("active price list"));
        }
        other => panic!("Expected validation error, got {other:?}"),
    }
}

#[tokio::test]
async fn test_resolve_variant_price_rejects_price_list_not_active_yet() {
    let (db, service, catalog) = setup().await;
    let tenant_id = Uuid::new_v4();
    let (_product_id, variant_id) = create_test_product(&catalog, tenant_id).await;
    let price_list_id = create_price_list(
        &db,
        tenant_id,
        "active",
        Some(chrono::Utc::now() + chrono::Duration::days(1)),
        None,
    )
    .await;

    let result = service
        .resolve_variant_price(
            tenant_id,
            variant_id,
            rustok_pricing::PriceResolutionContext {
                currency_code: "USD".to_string(),
                region_id: None,
                price_list_id: Some(price_list_id),
                channel_id: None,
                channel_slug: None,
                quantity: Some(1),
            },
        )
        .await;

    assert!(result.is_err());
    match result.unwrap_err() {
        CommerceError::Validation(message) => {
            assert!(message.contains("not active yet"));
        }
        other => panic!("Expected validation error, got {other:?}"),
    }
}

#[tokio::test]
async fn test_resolve_variant_price_rejects_expired_price_list_context() {
    let (db, service, catalog) = setup().await;
    let tenant_id = Uuid::new_v4();
    let (_product_id, variant_id) = create_test_product(&catalog, tenant_id).await;
    let price_list_id = create_price_list(
        &db,
        tenant_id,
        "active",
        None,
        Some(chrono::Utc::now() - chrono::Duration::days(1)),
    )
    .await;

    let result = service
        .resolve_variant_price(
            tenant_id,
            variant_id,
            rustok_pricing::PriceResolutionContext {
                currency_code: "USD".to_string(),
                region_id: None,
                price_list_id: Some(price_list_id),
                channel_id: None,
                channel_slug: None,
                quantity: Some(1),
            },
        )
        .await;

    assert!(result.is_err());
    match result.unwrap_err() {
        CommerceError::Validation(message) => {
            assert!(message.contains("already expired"));
        }
        other => panic!("Expected validation error, got {other:?}"),
    }
}

#[tokio::test]
async fn test_resolve_variant_price_rejects_price_list_outside_requested_channel() {
    let (db, service, catalog) = setup().await;
    let tenant_id = Uuid::new_v4();
    let actor_id = Uuid::new_v4();
    let web_channel_id = Uuid::new_v4();
    let (_product_id, variant_id) = create_test_product(&catalog, tenant_id).await;
    let price_list_id = create_price_list_with_channel(
        &db,
        tenant_id,
        "active",
        None,
        None,
        Some(web_channel_id),
        Some("web-store"),
    )
    .await;

    service
        .set_price(tenant_id, actor_id, variant_id, "USD", dec!(100.00), None)
        .await
        .unwrap();

    let result = service
        .resolve_variant_price(
            tenant_id,
            variant_id,
            rustok_pricing::PriceResolutionContext {
                currency_code: "USD".to_string(),
                region_id: None,
                price_list_id: Some(price_list_id),
                channel_id: Some(Uuid::new_v4()),
                channel_slug: Some("mobile-app".to_string()),
                quantity: Some(1),
            },
        )
        .await;

    assert!(result.is_err());
    match result.unwrap_err() {
        CommerceError::Validation(message) => {
            assert!(message.contains("price_list_id is not available for the requested channel"));
        }
        other => panic!("Expected validation error, got {other:?}"),
    }
}

#[tokio::test]
async fn test_resolve_variant_price_rejects_non_positive_quantity() {
    let (_db, service, catalog) = setup().await;
    let tenant_id = Uuid::new_v4();
    let actor_id = Uuid::new_v4();
    let (_product_id, variant_id) = create_test_product(&catalog, tenant_id).await;

    service
        .set_price(tenant_id, actor_id, variant_id, "USD", dec!(100.00), None)
        .await
        .unwrap();

    let result = service
        .resolve_variant_price(
            tenant_id,
            variant_id,
            rustok_pricing::PriceResolutionContext {
                currency_code: "USD".to_string(),
                region_id: None,
                price_list_id: None,
                channel_id: None,
                channel_slug: None,
                quantity: Some(0),
            },
        )
        .await;

    assert!(result.is_err());
    match result.unwrap_err() {
        CommerceError::Validation(message) => {
            assert!(message.contains("quantity must be at least 1"));
        }
        other => panic!("Expected validation error, got {other:?}"),
    }
}

#[tokio::test]
async fn test_resolve_variant_price_rejects_non_letter_currency_code() {
    let (_db, service, catalog) = setup().await;
    let tenant_id = Uuid::new_v4();
    let actor_id = Uuid::new_v4();
    let (_product_id, variant_id) = create_test_product(&catalog, tenant_id).await;

    service
        .set_price(tenant_id, actor_id, variant_id, "USD", dec!(100.00), None)
        .await
        .unwrap();

    let result = service
        .resolve_variant_price(
            tenant_id,
            variant_id,
            rustok_pricing::PriceResolutionContext {
                currency_code: "US1".to_string(),
                region_id: None,
                price_list_id: None,
                channel_id: None,
                channel_slug: None,
                quantity: Some(1),
            },
        )
        .await;

    assert!(result.is_err());
    match result.unwrap_err() {
        CommerceError::Validation(message) => {
            assert!(message.contains("currency_code must be a 3-letter code"));
        }
        other => panic!("Expected validation error, got {other:?}"),
    }
}
