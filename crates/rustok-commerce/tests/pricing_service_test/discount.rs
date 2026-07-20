use super::*;

#[tokio::test]
async fn test_apply_discount_10_percent() {
    let (_db, service, catalog) = setup().await;
    let tenant_id = Uuid::new_v4();
    let actor_id = Uuid::new_v4();
    let (_product_id, variant_id) = create_test_product(&catalog, tenant_id).await;

    service
        .set_price(tenant_id, actor_id, variant_id, "USD", dec!(100.00), None)
        .await
        .unwrap();

    let result = service
        .apply_discount(tenant_id, actor_id, variant_id, "USD", dec!(10))
        .await;

    assert!(result.is_ok());
    let new_amount = result.unwrap();
    assert_eq!(new_amount, dec!(90.00));

    let price = service.get_price(variant_id, "USD").await.unwrap();
    assert_eq!(price, Some(dec!(90.00)));
}

#[tokio::test]
async fn test_apply_discount_25_percent() {
    let (_db, service, catalog) = setup().await;
    let tenant_id = Uuid::new_v4();
    let actor_id = Uuid::new_v4();
    let (_product_id, variant_id) = create_test_product(&catalog, tenant_id).await;

    service
        .set_price(tenant_id, actor_id, variant_id, "USD", dec!(80.00), None)
        .await
        .unwrap();

    let result = service
        .apply_discount(tenant_id, actor_id, variant_id, "USD", dec!(25))
        .await;

    assert!(result.is_ok());
    let new_amount = result.unwrap();
    assert_eq!(new_amount, dec!(60.00));
}

#[tokio::test]
async fn test_apply_discount_50_percent() {
    let (_db, service, catalog) = setup().await;
    let tenant_id = Uuid::new_v4();
    let actor_id = Uuid::new_v4();
    let (_product_id, variant_id) = create_test_product(&catalog, tenant_id).await;

    service
        .set_price(tenant_id, actor_id, variant_id, "USD", dec!(100.00), None)
        .await
        .unwrap();

    let result = service
        .apply_discount(tenant_id, actor_id, variant_id, "USD", dec!(50))
        .await;

    assert!(result.is_ok());
    let new_amount = result.unwrap();
    assert_eq!(new_amount, dec!(50.00));
}

#[tokio::test]
async fn test_apply_discount_with_compare_at() {
    let (_db, service, catalog) = setup().await;
    let tenant_id = Uuid::new_v4();
    let actor_id = Uuid::new_v4();
    let (_product_id, variant_id) = create_test_product(&catalog, tenant_id).await;

    service
        .set_price(
            tenant_id,
            actor_id,
            variant_id,
            "USD",
            dec!(80.00),
            Some(dec!(100.00)),
        )
        .await
        .unwrap();

    let result = service
        .apply_discount(tenant_id, actor_id, variant_id, "USD", dec!(20))
        .await;

    assert!(result.is_ok());
    let new_amount = result.unwrap();
    assert_eq!(new_amount, dec!(80.00));
}

#[tokio::test]
async fn test_apply_discount_rounding() {
    let (_db, service, catalog) = setup().await;
    let tenant_id = Uuid::new_v4();
    let actor_id = Uuid::new_v4();
    let (_product_id, variant_id) = create_test_product(&catalog, tenant_id).await;

    service
        .set_price(tenant_id, actor_id, variant_id, "USD", dec!(99.99), None)
        .await
        .unwrap();

    let result = service
        .apply_discount(tenant_id, actor_id, variant_id, "USD", dec!(15))
        .await;

    assert!(result.is_ok());
    let new_amount = result.unwrap();
    assert_eq!(new_amount, dec!(84.99));
}

#[tokio::test]
async fn test_apply_discount_no_existing_price() {
    let (_db, service, catalog) = setup().await;
    let tenant_id = Uuid::new_v4();
    let actor_id = Uuid::new_v4();
    let (_product_id, variant_id) = create_test_product(&catalog, tenant_id).await;

    let result = service
        .apply_discount(tenant_id, actor_id, variant_id, "EUR", dec!(10))
        .await;

    assert!(result.is_err());
    match result.unwrap_err() {
        CommerceError::InvalidPrice(msg) => {
            assert!(msg.contains("No canonical price found"));
        }
        _ => panic!("Expected InvalidPrice error"),
    }
}

#[tokio::test]
async fn test_price_precision() {
    let (_db, service, catalog) = setup().await;
    let tenant_id = Uuid::new_v4();
    let actor_id = Uuid::new_v4();
    let (_product_id, variant_id) = create_test_product(&catalog, tenant_id).await;

    service
        .set_price(tenant_id, actor_id, variant_id, "USD", dec!(19.99), None)
        .await
        .unwrap();

    let price = service.get_price(variant_id, "USD").await.unwrap();
    assert_eq!(price, Some(dec!(19.99)));
}

#[tokio::test]
async fn test_price_with_many_decimal_places() {
    let (_db, service, catalog) = setup().await;
    let tenant_id = Uuid::new_v4();
    let actor_id = Uuid::new_v4();
    let (_product_id, variant_id) = create_test_product(&catalog, tenant_id).await;

    service
        .set_price(
            tenant_id,
            actor_id,
            variant_id,
            "USD",
            dec!(19.999999),
            None,
        )
        .await
        .unwrap();

    let price = service.get_price(variant_id, "USD").await.unwrap();
    assert!(price.is_some());
}

#[tokio::test]
async fn test_multiple_currencies_independence() {
    let (_db, service, catalog) = setup().await;
    let tenant_id = Uuid::new_v4();
    let actor_id = Uuid::new_v4();
    let (_product_id, variant_id) = create_test_product(&catalog, tenant_id).await;

    service
        .set_price(tenant_id, actor_id, variant_id, "USD", dec!(100.00), None)
        .await
        .unwrap();
    service
        .set_price(tenant_id, actor_id, variant_id, "EUR", dec!(90.00), None)
        .await
        .unwrap();

    service
        .apply_discount(tenant_id, actor_id, variant_id, "USD", dec!(10))
        .await
        .unwrap();

    let usd_price = service.get_price(variant_id, "USD").await.unwrap();
    let eur_price = service.get_price(variant_id, "EUR").await.unwrap();

    assert_eq!(usd_price, Some(dec!(90.00)));
    assert_eq!(eur_price, Some(dec!(90.00)));
}

#[tokio::test]
async fn test_currency_code_case_sensitive() {
    let (_db, service, catalog) = setup().await;
    let tenant_id = Uuid::new_v4();
    let actor_id = Uuid::new_v4();
    let (_product_id, variant_id) = create_test_product(&catalog, tenant_id).await;

    service
        .set_price(tenant_id, actor_id, variant_id, "USD", dec!(100.00), None)
        .await
        .unwrap();

    let usd_upper = service.get_price(variant_id, "USD").await.unwrap();
    let usd_lower = service.get_price(variant_id, "usd").await.unwrap();

    assert_eq!(usd_upper, Some(dec!(100.00)));
    assert_eq!(usd_lower, None);
}

#[tokio::test]
async fn test_price_workflow() {
    let (_db, service, catalog) = setup().await;
    let tenant_id = Uuid::new_v4();
    let actor_id = Uuid::new_v4();
    let (_product_id, variant_id) = create_test_product(&catalog, tenant_id).await;

    service
        .set_price(tenant_id, actor_id, variant_id, "USD", dec!(100.00), None)
        .await
        .unwrap();

    let prices = service.get_variant_prices(variant_id).await.unwrap();
    assert_eq!(prices.len(), 1);

    service
        .set_price(
            tenant_id,
            actor_id,
            variant_id,
            "USD",
            dec!(80.00),
            Some(dec!(100.00)),
        )
        .await
        .unwrap();

    service
        .apply_discount(tenant_id, actor_id, variant_id, "USD", dec!(25))
        .await
        .unwrap();

    let final_price = service.get_price(variant_id, "USD").await.unwrap();
    assert_eq!(final_price, Some(dec!(75.00)));
}

#[tokio::test]
async fn test_very_large_price() {
    let (_db, service, catalog) = setup().await;
    let tenant_id = Uuid::new_v4();
    let actor_id = Uuid::new_v4();
    let (_product_id, variant_id) = create_test_product(&catalog, tenant_id).await;

    let result = service
        .set_price(
            tenant_id,
            actor_id,
            variant_id,
            "USD",
            dec!(999999999.99),
            None,
        )
        .await;

    assert!(result.is_ok());

    let price = service.get_price(variant_id, "USD").await.unwrap();
    assert_eq!(price, Some(dec!(999999999.99)));
}

#[tokio::test]
async fn test_very_small_price() {
    let (_db, service, catalog) = setup().await;
    let tenant_id = Uuid::new_v4();
    let actor_id = Uuid::new_v4();
    let (_product_id, variant_id) = create_test_product(&catalog, tenant_id).await;

    let result = service
        .set_price(tenant_id, actor_id, variant_id, "USD", dec!(0.01), None)
        .await;

    assert!(result.is_ok());

    let price = service.get_price(variant_id, "USD").await.unwrap();
    assert_eq!(price, Some(dec!(0.01)));
}

#[tokio::test]
async fn test_discount_chain() {
    let (_db, service, catalog) = setup().await;
    let tenant_id = Uuid::new_v4();
    let actor_id = Uuid::new_v4();
    let (_product_id, variant_id) = create_test_product(&catalog, tenant_id).await;

    service
        .set_price(tenant_id, actor_id, variant_id, "USD", dec!(100.00), None)
        .await
        .unwrap();

    service
        .apply_discount(tenant_id, actor_id, variant_id, "USD", dec!(10))
        .await
        .unwrap();

    service
        .apply_discount(tenant_id, actor_id, variant_id, "USD", dec!(10))
        .await
        .unwrap();

    let final_price = service.get_price(variant_id, "USD").await.unwrap();
    assert_eq!(final_price, Some(dec!(90.00)));
}

#[tokio::test]
async fn test_preview_percentage_discount_returns_typed_adjustment() {
    let (_db, service, catalog) = setup().await;
    let tenant_id = Uuid::new_v4();
    let actor_id = Uuid::new_v4();
    let (_product_id, variant_id) = create_test_product(&catalog, tenant_id).await;

    service
        .set_price(
            tenant_id,
            actor_id,
            variant_id,
            "USD",
            dec!(80.00),
            Some(dec!(100.00)),
        )
        .await
        .unwrap();

    let preview = service
        .preview_percentage_discount(variant_id, "USD", dec!(25))
        .await
        .unwrap();

    assert_eq!(preview.kind, PriceAdjustmentKind::PercentageDiscount);
    assert_eq!(preview.currency_code, "USD");
    assert_eq!(preview.current_amount, dec!(80.00));
    assert_eq!(preview.base_amount, dec!(100.00));
    assert_eq!(preview.adjustment_percent, dec!(25));
    assert_eq!(preview.adjusted_amount, dec!(75.00));
    assert_eq!(preview.compare_at_amount, Some(dec!(100.00)));
    assert_eq!(preview.price_list_id, None);
}

#[tokio::test]
async fn test_apply_percentage_discount_targets_base_row_only() {
    let (_db, service, catalog) = setup().await;
    let tenant_id = Uuid::new_v4();
    let actor_id = Uuid::new_v4();
    let (_product_id, variant_id) = create_test_product(&catalog, tenant_id).await;

    service
        .set_price(tenant_id, actor_id, variant_id, "USD", dec!(100.00), None)
        .await
        .unwrap();
    service
        .set_price_tier(
            tenant_id,
            actor_id,
            variant_id,
            "USD",
            dec!(85.00),
            Some(dec!(100.00)),
            Some(10),
            None,
        )
        .await
        .unwrap();

    let preview = service
        .apply_percentage_discount(tenant_id, actor_id, variant_id, "USD", dec!(10))
        .await
        .unwrap();

    assert_eq!(preview.adjusted_amount, dec!(90.00));

    let resolved_base = service
        .resolve_variant_price(
            tenant_id,
            variant_id,
            rustok_pricing::PriceResolutionContext {
                currency_code: "USD".to_string(),
                region_id: None,
                price_list_id: None,
                channel_id: None,
                channel_slug: None,
                quantity: Some(1),
            },
        )
        .await
        .unwrap()
        .unwrap();
    let resolved_tier = service
        .resolve_variant_price(
            tenant_id,
            variant_id,
            rustok_pricing::PriceResolutionContext {
                currency_code: "USD".to_string(),
                region_id: None,
                price_list_id: None,
                channel_id: None,
                channel_slug: None,
                quantity: Some(10),
            },
        )
        .await
        .unwrap()
        .unwrap();

    assert_eq!(resolved_base.amount, dec!(90.00));
    assert_eq!(resolved_tier.amount, dec!(85.00));
}

#[tokio::test]
async fn test_preview_percentage_discount_supports_channel_scoped_base_row() {
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
        .set_price_tier_with_channel(
            tenant_id,
            actor_id,
            variant_id,
            "USD",
            dec!(80.00),
            Some(dec!(100.00)),
            Some(channel_id),
            Some("web-store".to_string()),
            None,
            None,
        )
        .await
        .unwrap();

    let preview = service
        .preview_percentage_discount_with_channel(
            variant_id,
            "USD",
            dec!(10),
            Some(channel_id),
            Some("web-store".to_string()),
        )
        .await
        .unwrap();

    assert_eq!(preview.current_amount, dec!(80.00));
    assert_eq!(preview.base_amount, dec!(100.00));
    assert_eq!(preview.adjusted_amount, dec!(90.00));
    assert_eq!(preview.price_list_id, None);
    assert_eq!(preview.channel_id, Some(channel_id));
    assert_eq!(preview.channel_slug.as_deref(), Some("web-store"));
}

#[tokio::test]
async fn test_apply_percentage_discount_targets_channel_scoped_base_row_only() {
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
        .set_price_tier_with_channel(
            tenant_id,
            actor_id,
            variant_id,
            "USD",
            dec!(80.00),
            Some(dec!(100.00)),
            Some(channel_id),
            Some("web-store".to_string()),
            None,
            None,
        )
        .await
        .unwrap();

    let preview = service
        .apply_percentage_discount_with_channel(
            tenant_id,
            actor_id,
            variant_id,
            "USD",
            dec!(10),
            Some(channel_id),
            Some("web-store".to_string()),
        )
        .await
        .unwrap();

    assert_eq!(preview.adjusted_amount, dec!(90.00));
    assert_eq!(preview.channel_id, Some(channel_id));
    assert_eq!(preview.channel_slug.as_deref(), Some("web-store"));

    let resolved_global = service
        .resolve_variant_price(
            tenant_id,
            variant_id,
            rustok_pricing::PriceResolutionContext {
                currency_code: "USD".to_string(),
                region_id: None,
                price_list_id: None,
                channel_id: None,
                channel_slug: None,
                quantity: Some(1),
            },
        )
        .await
        .unwrap()
        .unwrap();
    let resolved_channel = service
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
        .unwrap();

    assert_eq!(resolved_global.amount, dec!(100.00));
    assert_eq!(resolved_channel.amount, dec!(90.00));
    assert_eq!(resolved_channel.channel_id, Some(channel_id));
    assert_eq!(resolved_channel.channel_slug.as_deref(), Some("web-store"));
}

#[tokio::test]
async fn test_preview_price_list_percentage_discount_returns_typed_adjustment() {
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
        .set_price_list_tier(
            tenant_id,
            actor_id,
            variant_id,
            price_list_id,
            "USD",
            dec!(80.00),
            Some(dec!(100.00)),
            None,
            None,
        )
        .await
        .unwrap();

    let preview = service
        .preview_price_list_percentage_discount(
            tenant_id,
            variant_id,
            price_list_id,
            "USD",
            dec!(10),
        )
        .await
        .unwrap();

    assert_eq!(preview.kind, PriceAdjustmentKind::PercentageDiscount);
    assert_eq!(preview.current_amount, dec!(80.00));
    assert_eq!(preview.base_amount, dec!(100.00));
    assert_eq!(preview.adjusted_amount, dec!(90.00));
    assert_eq!(preview.price_list_id, Some(price_list_id));
}

#[tokio::test]
async fn test_apply_price_list_percentage_discount_targets_override_only() {
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
        .set_price_list_tier(
            tenant_id,
            actor_id,
            variant_id,
            price_list_id,
            "USD",
            dec!(80.00),
            Some(dec!(100.00)),
            None,
            None,
        )
        .await
        .unwrap();

    let preview = service
        .apply_price_list_percentage_discount(
            tenant_id,
            actor_id,
            variant_id,
            price_list_id,
            "USD",
            dec!(10),
        )
        .await
        .unwrap();

    assert_eq!(preview.adjusted_amount, dec!(90.00));
    assert_eq!(preview.price_list_id, Some(price_list_id));

    let resolved_base = service
        .resolve_variant_price(
            tenant_id,
            variant_id,
            rustok_pricing::PriceResolutionContext {
                currency_code: "USD".to_string(),
                region_id: None,
                price_list_id: None,
                channel_id: None,
                channel_slug: None,
                quantity: Some(1),
            },
        )
        .await
        .unwrap()
        .unwrap();
    let resolved_price_list = service
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
        .unwrap();

    assert_eq!(resolved_base.amount, dec!(100.00));
    assert_eq!(resolved_price_list.amount, dec!(90.00));
    assert_eq!(resolved_price_list.price_list_id, Some(price_list_id));
}

#[tokio::test]
async fn test_apply_price_list_percentage_discount_targets_channel_scoped_override_only() {
    let (db, service, catalog) = setup().await;
    let tenant_id = Uuid::new_v4();
    let actor_id = Uuid::new_v4();
    let (_product_id, variant_id) = create_test_product(&catalog, tenant_id).await;
    let channel_id = Uuid::new_v4();
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
        .set_price_list_tier_with_channel(
            tenant_id,
            actor_id,
            variant_id,
            price_list_id,
            "USD",
            dec!(80.00),
            Some(dec!(100.00)),
            Some(channel_id),
            Some("web-store".to_string()),
            None,
            None,
        )
        .await
        .unwrap();

    let preview = service
        .apply_price_list_percentage_discount_with_channel(
            tenant_id,
            actor_id,
            variant_id,
            price_list_id,
            "USD",
            dec!(10),
            Some(channel_id),
            Some("web-store".to_string()),
        )
        .await
        .unwrap();

    assert_eq!(preview.adjusted_amount, dec!(90.00));
    assert_eq!(preview.price_list_id, Some(price_list_id));
    assert_eq!(preview.channel_id, Some(channel_id));
    assert_eq!(preview.channel_slug.as_deref(), Some("web-store"));

    let resolved_base = service
        .resolve_variant_price(
            tenant_id,
            variant_id,
            rustok_pricing::PriceResolutionContext {
                currency_code: "USD".to_string(),
                region_id: None,
                price_list_id: None,
                channel_id: None,
                channel_slug: None,
                quantity: Some(1),
            },
        )
        .await
        .unwrap()
        .unwrap();
    let resolved_channel_override = service
        .resolve_variant_price(
            tenant_id,
            variant_id,
            rustok_pricing::PriceResolutionContext {
                currency_code: "USD".to_string(),
                region_id: None,
                price_list_id: Some(price_list_id),
                channel_id: Some(channel_id),
                channel_slug: Some("web-store".to_string()),
                quantity: Some(1),
            },
        )
        .await
        .unwrap()
        .unwrap();

    assert_eq!(resolved_base.amount, dec!(100.00));
    assert_eq!(resolved_channel_override.amount, dec!(90.00));
    assert_eq!(resolved_channel_override.channel_id, Some(channel_id));
    assert_eq!(
        resolved_channel_override.channel_slug.as_deref(),
        Some("web-store")
    );
}

#[tokio::test]
async fn test_apply_price_list_percentage_discount_rejects_price_list_not_active_yet_without_writing()
 {
    let (db, service, catalog) = setup().await;
    let tenant_id = Uuid::new_v4();
    let actor_id = Uuid::new_v4();
    let (_product_id, variant_id) = create_test_product(&catalog, tenant_id).await;
    let price_list_id = create_price_list(
        &db,
        tenant_id,
        "active",
        Some(chrono::Utc::now() + chrono::Duration::days(1)),
        None,
    )
    .await;

    service
        .set_price(tenant_id, actor_id, variant_id, "USD", dec!(100.00), None)
        .await
        .unwrap();

    let count_before = entities::price::Entity::find()
        .filter(entities::price::Column::VariantId.eq(variant_id))
        .filter(entities::price::Column::PriceListId.eq(price_list_id))
        .count(&db)
        .await
        .unwrap();

    let error = service
        .apply_price_list_percentage_discount(
            tenant_id,
            actor_id,
            variant_id,
            price_list_id,
            "USD",
            dec!(10),
        )
        .await
        .unwrap_err();

    match error {
        CommerceError::Validation(message) => {
            assert!(message.contains("not active yet"));
        }
        other => panic!("Expected Validation error, got {other:?}"),
    }

    let count_after = entities::price::Entity::find()
        .filter(entities::price::Column::VariantId.eq(variant_id))
        .filter(entities::price::Column::PriceListId.eq(price_list_id))
        .count(&db)
        .await
        .unwrap();

    assert_eq!(count_before, 0);
    assert_eq!(count_after, 0);
}

#[tokio::test]
async fn test_apply_price_list_percentage_discount_rejects_expired_list_without_writing() {
    let (db, service, catalog) = setup().await;
    let tenant_id = Uuid::new_v4();
    let actor_id = Uuid::new_v4();
    let (_product_id, variant_id) = create_test_product(&catalog, tenant_id).await;
    let price_list_id = create_price_list(
        &db,
        tenant_id,
        "active",
        None,
        Some(chrono::Utc::now() - chrono::Duration::days(1)),
    )
    .await;

    service
        .set_price(tenant_id, actor_id, variant_id, "USD", dec!(100.00), None)
        .await
        .unwrap();

    let count_before = entities::price::Entity::find()
        .filter(entities::price::Column::VariantId.eq(variant_id))
        .filter(entities::price::Column::PriceListId.eq(price_list_id))
        .count(&db)
        .await
        .unwrap();

    let error = service
        .apply_price_list_percentage_discount(
            tenant_id,
            actor_id,
            variant_id,
            price_list_id,
            "USD",
            dec!(10),
        )
        .await
        .unwrap_err();

    match error {
        CommerceError::Validation(message) => {
            assert!(message.contains("already expired"));
        }
        other => panic!("Expected Validation error, got {other:?}"),
    }

    let count_after = entities::price::Entity::find()
        .filter(entities::price::Column::VariantId.eq(variant_id))
        .filter(entities::price::Column::PriceListId.eq(price_list_id))
        .count(&db)
        .await
        .unwrap();

    assert_eq!(count_before, 0);
    assert_eq!(count_after, 0);
}

#[tokio::test]
async fn test_apply_price_list_percentage_discount_rejects_channel_mismatch_without_mutating_override()
 {
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
    service
        .set_price_list_tier_with_channel(
            tenant_id,
            actor_id,
            variant_id,
            price_list_id,
            "USD",
            dec!(80.00),
            Some(dec!(100.00)),
            Some(web_channel_id),
            Some("web-store".to_string()),
            None,
            None,
        )
        .await
        .unwrap();

    let count_before = entities::price::Entity::find()
        .filter(entities::price::Column::VariantId.eq(variant_id))
        .filter(entities::price::Column::PriceListId.eq(price_list_id))
        .count(&db)
        .await
        .unwrap();

    let error = service
        .apply_price_list_percentage_discount_with_channel(
            tenant_id,
            actor_id,
            variant_id,
            price_list_id,
            "USD",
            dec!(10),
            Some(Uuid::new_v4()),
            Some("mobile-app".to_string()),
        )
        .await
        .unwrap_err();

    match error {
        CommerceError::Validation(message) => {
            assert!(message.contains("price_list_id is not available for the requested channel"));
        }
        other => panic!("Expected Validation error, got {other:?}"),
    }

    let count_after = entities::price::Entity::find()
        .filter(entities::price::Column::VariantId.eq(variant_id))
        .filter(entities::price::Column::PriceListId.eq(price_list_id))
        .count(&db)
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
                channel_id: Some(web_channel_id),
                channel_slug: Some("web-store".to_string()),
                quantity: Some(1),
            },
        )
        .await
        .unwrap()
        .expect("existing scoped override should stay intact");

    assert_eq!(count_before, 1);
    assert_eq!(count_after, 1);
    assert_eq!(resolved.amount, dec!(80.00));
    assert_eq!(resolved.compare_at_amount, Some(dec!(100.00)));
}

#[tokio::test]
async fn test_preview_price_list_percentage_discount_rejects_inactive_list() {
    let (db, service, catalog) = setup().await;
    let tenant_id = Uuid::new_v4();
    let actor_id = Uuid::new_v4();
    let (_product_id, variant_id) = create_test_product(&catalog, tenant_id).await;
    let price_list_id = create_price_list(&db, tenant_id, "draft", None, None).await;

    service
        .set_price(tenant_id, actor_id, variant_id, "USD", dec!(100.00), None)
        .await
        .unwrap();

    let error = service
        .preview_price_list_percentage_discount(
            tenant_id,
            variant_id,
            price_list_id,
            "USD",
            dec!(10),
        )
        .await
        .unwrap_err();

    match error {
        CommerceError::Validation(message) => {
            assert!(message.contains("active price list"));
        }
        other => panic!("Expected Validation error, got {other:?}"),
    }
}

#[tokio::test]
async fn test_preview_price_list_percentage_discount_rejects_price_list_not_active_yet() {
    let (db, service, catalog) = setup().await;
    let tenant_id = Uuid::new_v4();
    let actor_id = Uuid::new_v4();
    let (_product_id, variant_id) = create_test_product(&catalog, tenant_id).await;
    let price_list_id = create_price_list(
        &db,
        tenant_id,
        "active",
        Some(chrono::Utc::now() + chrono::Duration::days(1)),
        None,
    )
    .await;

    service
        .set_price(tenant_id, actor_id, variant_id, "USD", dec!(100.00), None)
        .await
        .unwrap();

    let error = service
        .preview_price_list_percentage_discount(
            tenant_id,
            variant_id,
            price_list_id,
            "USD",
            dec!(10),
        )
        .await
        .unwrap_err();

    match error {
        CommerceError::Validation(message) => {
            assert!(message.contains("not active yet"));
        }
        other => panic!("Expected Validation error, got {other:?}"),
    }
}

#[tokio::test]
async fn test_preview_price_list_percentage_discount_rejects_expired_list() {
    let (db, service, catalog) = setup().await;
    let tenant_id = Uuid::new_v4();
    let actor_id = Uuid::new_v4();
    let (_product_id, variant_id) = create_test_product(&catalog, tenant_id).await;
    let price_list_id = create_price_list(
        &db,
        tenant_id,
        "active",
        None,
        Some(chrono::Utc::now() - chrono::Duration::days(1)),
    )
    .await;

    service
        .set_price(tenant_id, actor_id, variant_id, "USD", dec!(100.00), None)
        .await
        .unwrap();

    let error = service
        .preview_price_list_percentage_discount(
            tenant_id,
            variant_id,
            price_list_id,
            "USD",
            dec!(10),
        )
        .await
        .unwrap_err();

    match error {
        CommerceError::Validation(message) => {
            assert!(message.contains("already expired"));
        }
        other => panic!("Expected Validation error, got {other:?}"),
    }
}

#[tokio::test]
async fn test_preview_price_list_percentage_discount_rejects_requested_channel_mismatch() {
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

    let error = service
        .preview_price_list_percentage_discount_with_channel(
            tenant_id,
            variant_id,
            price_list_id,
            "USD",
            dec!(10),
            Some(Uuid::new_v4()),
            Some("mobile-app".to_string()),
        )
        .await
        .unwrap_err();

    match error {
        CommerceError::Validation(message) => {
            assert!(message.contains("price_list_id is not available for the requested channel"));
        }
        other => panic!("Expected Validation error, got {other:?}"),
    }
}

#[tokio::test]
async fn test_preview_percentage_discount_rejects_invalid_percent() {
    let (_db, service, catalog) = setup().await;
    let tenant_id = Uuid::new_v4();
    let actor_id = Uuid::new_v4();
    let (_product_id, variant_id) = create_test_product(&catalog, tenant_id).await;

    service
        .set_price(tenant_id, actor_id, variant_id, "USD", dec!(100.00), None)
        .await
        .unwrap();

    let error = service
        .preview_percentage_discount(variant_id, "USD", dec!(125))
        .await
        .unwrap_err();

    match error {
        CommerceError::InvalidPrice(message) => {
            assert!(message.contains("discount_percent"));
        }
        other => panic!("Expected InvalidPrice error, got {other:?}"),
    }
}

#[tokio::test]
async fn test_resolve_variant_price_reports_discount_percent() {
    let (_db, service, catalog) = setup().await;
    let tenant_id = Uuid::new_v4();
    let actor_id = Uuid::new_v4();
    let (_product_id, variant_id) = create_test_product(&catalog, tenant_id).await;

    service
        .set_price(
            tenant_id,
            actor_id,
            variant_id,
            "USD",
            dec!(84.99),
            Some(dec!(99.99)),
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
                channel_slug: None,
                quantity: Some(1),
            },
        )
        .await
        .expect("price resolution should succeed")
        .expect("sale price should resolve");

    assert_eq!(resolved.discount_percent, Some(dec!(15.00)));
    assert!(resolved.on_sale);
}

#[tokio::test]
async fn test_resolve_variant_price_rounds_fractional_discount_percent() {
    let (_db, service, catalog) = setup().await;
    let tenant_id = Uuid::new_v4();
    let actor_id = Uuid::new_v4();
    let (_product_id, variant_id) = create_test_product(&catalog, tenant_id).await;

    service
        .set_price(
            tenant_id,
            actor_id,
            variant_id,
            "USD",
            dec!(2.00),
            Some(dec!(3.00)),
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
                channel_slug: None,
                quantity: Some(1),
            },
        )
        .await
        .expect("price resolution should succeed")
        .expect("sale price should resolve");

    assert_eq!(resolved.amount, dec!(2.00));
    assert_eq!(resolved.compare_at_amount, Some(dec!(3.00)));
    assert_eq!(resolved.discount_percent, Some(dec!(33.33)));
    assert!(resolved.on_sale);
}

#[tokio::test]
async fn test_resolve_variant_price_omits_discount_percent_for_non_sale_price() {
    let (_db, service, catalog) = setup().await;
    let tenant_id = Uuid::new_v4();
    let actor_id = Uuid::new_v4();
    let (_product_id, variant_id) = create_test_product(&catalog, tenant_id).await;

    service
        .set_price(tenant_id, actor_id, variant_id, "USD", dec!(99.99), None)
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
                quantity: Some(1),
            },
        )
        .await
        .expect("price resolution should succeed")
        .expect("base price should resolve");

    assert_eq!(resolved.discount_percent, None);
    assert!(!resolved.on_sale);
}
