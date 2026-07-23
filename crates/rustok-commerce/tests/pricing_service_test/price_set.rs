use super::*;

#[tokio::test]
async fn test_set_price_success() {
    let (_db, service, catalog) = setup().await;
    let tenant_id = Uuid::new_v4();
    let actor_id = Uuid::new_v4();
    let (_product_id, variant_id) = create_test_product(&catalog, tenant_id).await;

    let result = service
        .set_price(tenant_id, actor_id, variant_id, "USD", dec!(99.99), None)
        .await;

    assert!(result.is_ok());
}

#[tokio::test]
async fn test_set_price_with_compare_at() {
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
            dec!(79.99),
            Some(dec!(99.99)),
        )
        .await;

    assert!(result.is_ok());
}

#[tokio::test]
async fn test_set_price_persists_decimal_and_legacy_cents() {
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
            dec!(79.994),
            Some(dec!(99.996)),
        )
        .await
        .unwrap();

    let price = entities::price::Entity::find()
        .filter(entities::price::Column::VariantId.eq(variant_id))
        .filter(entities::price::Column::CurrencyCode.eq("USD"))
        .filter(entities::price::Column::PriceListId.is_null())
        .filter(entities::price::Column::ChannelId.is_null())
        .filter(entities::price::Column::MinQuantity.is_null())
        .filter(entities::price::Column::MaxQuantity.is_null())
        .one(&_db)
        .await
        .unwrap()
        .expect("base row should exist");

    assert_eq!(price.amount, dec!(79.994));
    assert_eq!(price.compare_at_amount, Some(dec!(99.996)));
    assert_eq!(price.legacy_amount, Some(7999));
    assert_eq!(price.legacy_compare_at_amount, Some(10000));
}

#[tokio::test]
async fn test_set_price_clears_compare_at_and_legacy_compare_at_on_existing_row() {
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
            dec!(79.99),
            Some(dec!(99.99)),
        )
        .await
        .unwrap();

    service
        .set_price(tenant_id, actor_id, variant_id, "USD", dec!(89.99), None)
        .await
        .unwrap();

    let price = entities::price::Entity::find()
        .filter(entities::price::Column::VariantId.eq(variant_id))
        .filter(entities::price::Column::CurrencyCode.eq("USD"))
        .filter(entities::price::Column::PriceListId.is_null())
        .filter(entities::price::Column::ChannelId.is_null())
        .filter(entities::price::Column::MinQuantity.is_null())
        .filter(entities::price::Column::MaxQuantity.is_null())
        .one(&_db)
        .await
        .unwrap()
        .expect("base row should exist");

    assert_eq!(price.amount, dec!(89.99));
    assert_eq!(price.compare_at_amount, None);
    assert_eq!(price.legacy_amount, Some(8999));
    assert_eq!(price.legacy_compare_at_amount, None);
}

#[tokio::test]
async fn test_set_price_publishes_price_updated_event_with_rounded_cents() {
    let (_db, service, catalog, transport) = setup_with_transport().await;
    let tenant_id = Uuid::new_v4();
    let actor_id = Uuid::new_v4();
    let (product_id, variant_id) = create_test_product(&catalog, tenant_id).await;

    transport.clear();

    service
        .set_price(
            tenant_id,
            actor_id,
            variant_id,
            "USD",
            dec!(79.994),
            Some(dec!(99.996)),
        )
        .await
        .unwrap();

    assert_eq!(transport.event_count(), 1);
    let events = transport.events_of_type("PriceUpdated");
    assert_eq!(events.len(), 1);

    match &events[0] {
        DomainEvent::PriceUpdated {
            variant_id: event_variant_id,
            product_id: event_product_id,
            currency,
            old_amount,
            new_amount,
        } => {
            assert_eq!(*event_variant_id, variant_id);
            assert_eq!(*event_product_id, product_id);
            assert_eq!(currency, "USD");
            assert_eq!(*old_amount, Some(9999));
            assert_eq!(*new_amount, 7999);
        }
        other => panic!("Expected PriceUpdated event, got {other:?}"),
    }
}

#[tokio::test]
async fn test_set_price_tier_persists_quantity_window_and_resolves() {
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

    let prices = service.get_variant_prices(variant_id).await.unwrap();
    let tier = prices
        .into_iter()
        .find(|price| price.min_quantity == Some(10) && price.max_quantity.is_none())
        .expect("tier row should exist");
    assert_eq!(tier.amount, dec!(85.00));
    assert_eq!(tier.compare_at_amount, Some(dec!(100.00)));

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
async fn test_set_price_tier_rejects_invalid_quantity_window() {
    let (_db, service, catalog) = setup().await;
    let tenant_id = Uuid::new_v4();
    let actor_id = Uuid::new_v4();
    let (_product_id, variant_id) = create_test_product(&catalog, tenant_id).await;

    let result = service
        .set_price_tier(
            tenant_id,
            actor_id,
            variant_id,
            "USD",
            dec!(85.00),
            None,
            Some(10),
            Some(5),
        )
        .await;

    assert!(result.is_err());
    match result.unwrap_err() {
        CommerceError::InvalidPrice(message) => {
            assert!(message.contains("Maximum quantity"));
        }
        other => panic!("Expected InvalidPrice error, got {other:?}"),
    }
}

#[tokio::test]
async fn test_set_price_multiple_currencies() {
    let (_db, service, catalog) = setup().await;
    let tenant_id = Uuid::new_v4();
    let actor_id = Uuid::new_v4();
    let (_product_id, variant_id) = create_test_product(&catalog, tenant_id).await;

    service
        .set_price(tenant_id, actor_id, variant_id, "USD", dec!(99.99), None)
        .await
        .unwrap();
    service
        .set_price(tenant_id, actor_id, variant_id, "EUR", dec!(89.99), None)
        .await
        .unwrap();
    service
        .set_price(tenant_id, actor_id, variant_id, "GBP", dec!(79.99), None)
        .await
        .unwrap();

    let usd_price = service.get_price(variant_id, "USD").await.unwrap();
    let eur_price = service.get_price(variant_id, "EUR").await.unwrap();
    let gbp_price = service.get_price(variant_id, "GBP").await.unwrap();

    assert_eq!(usd_price, Some(dec!(99.99)));
    assert_eq!(eur_price, Some(dec!(89.99)));
    assert_eq!(gbp_price, Some(dec!(79.99)));
}

#[tokio::test]
async fn test_set_price_negative_amount() {
    let (_db, service, catalog) = setup().await;
    let tenant_id = Uuid::new_v4();
    let actor_id = Uuid::new_v4();
    let (_product_id, variant_id) = create_test_product(&catalog, tenant_id).await;

    let result = service
        .set_price(tenant_id, actor_id, variant_id, "USD", dec!(-10.00), None)
        .await;

    assert!(result.is_err());
    match result.unwrap_err() {
        CommerceError::InvalidPrice(msg) => {
            assert!(msg.contains("negative"));
        }
        _ => panic!("Expected InvalidPrice error"),
    }
}

#[tokio::test]
async fn test_set_price_invalid_compare_at() {
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
            dec!(99.99),
            Some(dec!(79.99)),
        )
        .await;

    assert!(result.is_err());
    match result.unwrap_err() {
        CommerceError::InvalidPrice(msg) => {
            assert!(msg.contains("greater"));
        }
        _ => panic!("Expected InvalidPrice error"),
    }
}

#[tokio::test]
async fn test_set_price_zero_amount() {
    let (_db, service, catalog) = setup().await;
    let tenant_id = Uuid::new_v4();
    let actor_id = Uuid::new_v4();
    let (_product_id, variant_id) = create_test_product(&catalog, tenant_id).await;

    let result = service
        .set_price(tenant_id, actor_id, variant_id, "USD", dec!(0.00), None)
        .await;

    assert!(result.is_ok());
}

#[tokio::test]
async fn test_set_price_update_existing() {
    let (_db, service, catalog) = setup().await;
    let tenant_id = Uuid::new_v4();
    let actor_id = Uuid::new_v4();
    let (_product_id, variant_id) = create_test_product(&catalog, tenant_id).await;

    service
        .set_price(tenant_id, actor_id, variant_id, "USD", dec!(99.99), None)
        .await
        .unwrap();

    service
        .set_price(tenant_id, actor_id, variant_id, "USD", dec!(79.99), None)
        .await
        .unwrap();

    let price = service.get_price(variant_id, "USD").await.unwrap();
    assert_eq!(price, Some(dec!(79.99)));
}

#[tokio::test]
async fn test_set_price_nonexistent_variant() {
    let (_db, service, _catalog) = setup().await;
    let tenant_id = Uuid::new_v4();
    let actor_id = Uuid::new_v4();
    let fake_variant_id = Uuid::new_v4();

    let result = service
        .set_price(
            tenant_id,
            actor_id,
            fake_variant_id,
            "USD",
            dec!(99.99),
            None,
        )
        .await;

    assert!(result.is_err());
    match result.unwrap_err() {
        CommerceError::VariantNotFound(_) => {}
        _ => panic!("Expected VariantNotFound error"),
    }
}

#[tokio::test]
async fn test_set_prices_bulk() {
    let (_db, service, catalog) = setup().await;
    let tenant_id = Uuid::new_v4();
    let actor_id = Uuid::new_v4();
    let (_product_id, variant_id) = create_test_product(&catalog, tenant_id).await;

    let prices = vec![
        rustok_commerce_foundation::dto::PriceInput {
            currency_code: "USD".to_string(),
            channel_id: None,
            channel_slug: None,
            amount: dec!(99.99),
            compare_at_amount: None,
        },
        rustok_commerce_foundation::dto::PriceInput {
            currency_code: "EUR".to_string(),
            channel_id: None,
            channel_slug: None,
            amount: dec!(89.99),
            compare_at_amount: None,
        },
        rustok_commerce_foundation::dto::PriceInput {
            currency_code: "GBP".to_string(),
            channel_id: None,
            channel_slug: None,
            amount: dec!(79.99),
            compare_at_amount: None,
        },
    ];

    let result = service
        .set_prices(tenant_id, actor_id, variant_id, prices)
        .await;

    assert!(result.is_ok());

    let usd_price = service.get_price(variant_id, "USD").await.unwrap();
    let eur_price = service.get_price(variant_id, "EUR").await.unwrap();
    let gbp_price = service.get_price(variant_id, "GBP").await.unwrap();

    assert_eq!(usd_price, Some(dec!(99.99)));
    assert_eq!(eur_price, Some(dec!(89.99)));
    assert_eq!(gbp_price, Some(dec!(79.99)));
}

#[tokio::test]
async fn test_set_prices_persists_decimal_and_legacy_cents_for_new_scoped_row() {
    let (_db, service, catalog) = setup().await;
    let tenant_id = Uuid::new_v4();
    let actor_id = Uuid::new_v4();
    let (_product_id, variant_id) = create_test_product(&catalog, tenant_id).await;
    let channel_id = Uuid::new_v4();

    service
        .set_prices(
            tenant_id,
            actor_id,
            variant_id,
            vec![rustok_commerce_foundation::dto::PriceInput {
                currency_code: "USD".to_string(),
                channel_id: Some(channel_id),
                channel_slug: Some("WEB-STORE".to_string()),
                amount: dec!(79.994),
                compare_at_amount: Some(dec!(99.996)),
            }],
        )
        .await
        .unwrap();

    let price = entities::price::Entity::find()
        .filter(entities::price::Column::VariantId.eq(variant_id))
        .filter(entities::price::Column::CurrencyCode.eq("USD"))
        .filter(entities::price::Column::ChannelId.eq(channel_id))
        .filter(entities::price::Column::PriceListId.is_null())
        .filter(entities::price::Column::MinQuantity.is_null())
        .filter(entities::price::Column::MaxQuantity.is_null())
        .one(&_db)
        .await
        .unwrap()
        .expect("scoped bulk row should exist");

    assert_eq!(price.amount, dec!(79.994));
    assert_eq!(price.compare_at_amount, Some(dec!(99.996)));
    assert_eq!(price.channel_slug.as_deref(), Some("web-store"));
    assert_eq!(price.legacy_amount, Some(7999));
    assert_eq!(price.legacy_compare_at_amount, Some(10000));
}

#[tokio::test]
async fn test_set_prices_rolls_back_existing_row_updates_when_any_price_is_invalid() {
    let (_db, service, catalog) = setup().await;
    let tenant_id = Uuid::new_v4();
    let actor_id = Uuid::new_v4();
    let (_product_id, variant_id) = create_test_product(&catalog, tenant_id).await;

    service
        .set_prices(
            tenant_id,
            actor_id,
            variant_id,
            vec![
                rustok_commerce_foundation::dto::PriceInput {
                    currency_code: "USD".to_string(),
                    channel_id: None,
                    channel_slug: None,
                    amount: dec!(79.99),
                    compare_at_amount: Some(dec!(99.99)),
                },
                rustok_commerce_foundation::dto::PriceInput {
                    currency_code: "EUR".to_string(),
                    channel_id: None,
                    channel_slug: None,
                    amount: dec!(89.99),
                    compare_at_amount: None,
                },
            ],
        )
        .await
        .unwrap();

    let count_before = entities::price::Entity::find()
        .filter(entities::price::Column::VariantId.eq(variant_id))
        .count(&_db)
        .await
        .unwrap();

    let error = service
        .set_prices(
            tenant_id,
            actor_id,
            variant_id,
            vec![
                rustok_commerce_foundation::dto::PriceInput {
                    currency_code: "USD".to_string(),
                    channel_id: None,
                    channel_slug: None,
                    amount: dec!(59.99),
                    compare_at_amount: Some(dec!(79.99)),
                },
                rustok_commerce_foundation::dto::PriceInput {
                    currency_code: "GBP".to_string(),
                    channel_id: None,
                    channel_slug: None,
                    amount: dec!(70.00),
                    compare_at_amount: Some(dec!(60.00)),
                },
            ],
        )
        .await
        .expect_err("invalid bulk input should roll back the transaction");

    match error {
        CommerceError::InvalidPrice(message) => {
            assert!(message.contains("Compare at price must be greater than amount"));
        }
        other => panic!("Expected InvalidPrice error, got {other:?}"),
    }

    let count_after = entities::price::Entity::find()
        .filter(entities::price::Column::VariantId.eq(variant_id))
        .count(&_db)
        .await
        .unwrap();
    assert_eq!(count_after, count_before);

    let usd_price = entities::price::Entity::find()
        .filter(entities::price::Column::VariantId.eq(variant_id))
        .filter(entities::price::Column::CurrencyCode.eq("USD"))
        .filter(entities::price::Column::PriceListId.is_null())
        .filter(entities::price::Column::ChannelId.is_null())
        .filter(entities::price::Column::MinQuantity.is_null())
        .filter(entities::price::Column::MaxQuantity.is_null())
        .one(&_db)
        .await
        .unwrap()
        .expect("usd row should still exist");

    assert_eq!(usd_price.amount, dec!(79.99));
    assert_eq!(usd_price.compare_at_amount, Some(dec!(99.99)));
    assert_eq!(usd_price.legacy_amount, Some(7999));
    assert_eq!(usd_price.legacy_compare_at_amount, Some(9999));

    let gbp_price = entities::price::Entity::find()
        .filter(entities::price::Column::VariantId.eq(variant_id))
        .filter(entities::price::Column::CurrencyCode.eq("GBP"))
        .one(&_db)
        .await
        .unwrap();
    assert!(gbp_price.is_none());
}

#[tokio::test]
async fn test_set_prices_empty_list() {
    let (_db, service, catalog) = setup().await;
    let tenant_id = Uuid::new_v4();
    let actor_id = Uuid::new_v4();
    let (_product_id, variant_id) = create_test_product(&catalog, tenant_id).await;

    let result = service
        .set_prices(tenant_id, actor_id, variant_id, vec![])
        .await;

    assert!(result.is_ok());
}

#[tokio::test]
async fn test_set_prices_publishes_price_updated_events_with_old_and_new_cents() {
    let (_db, service, catalog, transport) = setup_with_transport().await;
    let tenant_id = Uuid::new_v4();
    let actor_id = Uuid::new_v4();
    let (product_id, variant_id) = create_test_product(&catalog, tenant_id).await;

    transport.clear();

    service
        .set_prices(
            tenant_id,
            actor_id,
            variant_id,
            vec![
                rustok_commerce_foundation::dto::PriceInput {
                    currency_code: "USD".to_string(),
                    channel_id: None,
                    channel_slug: None,
                    amount: dec!(89.994),
                    compare_at_amount: Some(dec!(99.996)),
                },
                rustok_commerce_foundation::dto::PriceInput {
                    currency_code: "EUR".to_string(),
                    channel_id: None,
                    channel_slug: None,
                    amount: dec!(79.994),
                    compare_at_amount: Some(dec!(99.996)),
                },
            ],
        )
        .await
        .unwrap();

    let events = transport.events_of_type("PriceUpdated");
    assert_eq!(events.len(), 2);

    match &events[0] {
        DomainEvent::PriceUpdated {
            variant_id: event_variant_id,
            product_id: event_product_id,
            currency,
            old_amount,
            new_amount,
        } => {
            assert_eq!(*event_variant_id, variant_id);
            assert_eq!(*event_product_id, product_id);
            assert_eq!(currency, "USD");
            assert_eq!(*old_amount, Some(9999));
            assert_eq!(*new_amount, 8999);
        }
        other => panic!("Expected first PriceUpdated event, got {other:?}"),
    }

    match &events[1] {
        DomainEvent::PriceUpdated {
            variant_id: event_variant_id,
            product_id: event_product_id,
            currency,
            old_amount,
            new_amount,
        } => {
            assert_eq!(*event_variant_id, variant_id);
            assert_eq!(*event_product_id, product_id);
            assert_eq!(currency, "EUR");
            assert_eq!(*old_amount, None);
            assert_eq!(*new_amount, 7999);
        }
        other => panic!("Expected second PriceUpdated event, got {other:?}"),
    }
}

#[tokio::test]
async fn test_get_price_existing() {
    let (_db, service, catalog) = setup().await;
    let tenant_id = Uuid::new_v4();
    let actor_id = Uuid::new_v4();
    let (_product_id, variant_id) = create_test_product(&catalog, tenant_id).await;

    service
        .set_price(tenant_id, actor_id, variant_id, "USD", dec!(99.99), None)
        .await
        .unwrap();

    let result = service.get_price(variant_id, "USD").await;

    assert!(result.is_ok());
    let price = result.unwrap();
    assert_eq!(price, Some(dec!(99.99)));
}

#[tokio::test]
async fn test_get_price_nonexistent() {
    let (_db, service, catalog) = setup().await;
    let tenant_id = Uuid::new_v4();
    let (_product_id, variant_id) = create_test_product(&catalog, tenant_id).await;

    let result = service.get_price(variant_id, "EUR").await;

    assert!(result.is_ok());
    let price = result.unwrap();
    assert_eq!(price, None);
}

#[tokio::test]
async fn test_get_price_after_update() {
    let (_db, service, catalog) = setup().await;
    let tenant_id = Uuid::new_v4();
    let actor_id = Uuid::new_v4();
    let (_product_id, variant_id) = create_test_product(&catalog, tenant_id).await;

    service
        .set_price(tenant_id, actor_id, variant_id, "USD", dec!(99.99), None)
        .await
        .unwrap();

    service
        .set_price(tenant_id, actor_id, variant_id, "USD", dec!(79.99), None)
        .await
        .unwrap();

    let price = service.get_price(variant_id, "USD").await.unwrap();
    assert_eq!(price, Some(dec!(79.99)));
}

#[tokio::test]
async fn test_get_variant_prices_multiple() {
    let (_db, service, catalog) = setup().await;
    let tenant_id = Uuid::new_v4();
    let actor_id = Uuid::new_v4();
    let (_product_id, variant_id) = create_test_product(&catalog, tenant_id).await;

    service
        .set_price(tenant_id, actor_id, variant_id, "USD", dec!(99.99), None)
        .await
        .unwrap();
    service
        .set_price(tenant_id, actor_id, variant_id, "EUR", dec!(89.99), None)
        .await
        .unwrap();
    service
        .set_price(tenant_id, actor_id, variant_id, "GBP", dec!(79.99), None)
        .await
        .unwrap();

    let result = service.get_variant_prices(variant_id).await;

    assert!(result.is_ok());
    let prices = result.unwrap();
    assert_eq!(prices.len(), 3);

    let currency_codes: Vec<String> = prices.iter().map(|p| p.currency_code.clone()).collect();
    assert!(currency_codes.contains(&"USD".to_string()));
    assert!(currency_codes.contains(&"EUR".to_string()));
    assert!(currency_codes.contains(&"GBP".to_string()));
}

#[tokio::test]
async fn test_get_variant_prices_empty() {
    let (_db, service, catalog) = setup().await;
    let tenant_id = Uuid::new_v4();
    let actor_id = Uuid::new_v4();
    let input = CreateProductInput {
        translations: vec![ProductTranslationInput {
            locale: "en".to_string(),
            title: "No Price Product".to_string(),
            description: Some("Variant without prices".to_string()),
            handle: Some(unique_slug("no-price-product")),
            meta_title: None,
            meta_description: None,
        }],
        options: vec![],
        variants: vec![CreateVariantInput {
            sku: Some(format!(
                "SKU-{}",
                Uuid::new_v4().to_string().split('-').next().unwrap()
            )),
            barcode: None,
            shipping_profile_slug: None,
            option1: Some("Default".to_string()),
            option2: None,
            option3: None,
            prices: vec![],
            inventory_quantity: 0,
            inventory_policy: "deny".to_string(),
            weight: Some(dec!(1.5)),
            weight_unit: Some("kg".to_string()),
        }],
        seller_id: None,
        vendor: Some("Test Vendor".to_string()),
        product_type: Some("Physical".to_string()),
        shipping_profile_slug: None,
        primary_category_id: None,
        tags: vec![],
        publish: false,
        metadata: serde_json::json!({}),
    };

    let product = catalog
        .create_product(tenant_id, actor_id, input)
        .await
        .unwrap();
    let variant_id = product.variants[0].id;

    let result = service.get_variant_prices(variant_id).await;

    assert!(result.is_ok());
    let prices = result.unwrap();
    assert_eq!(prices.len(), 0);
}
