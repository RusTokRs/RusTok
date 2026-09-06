use super::*;

#[tokio::test]
async fn test_set_price_list_tier_resolves_active_price_list_override() {
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
            dec!(70.00),
            Some(dec!(100.00)),
            Some(5),
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
                quantity: Some(6),
            },
        )
        .await
        .unwrap()
        .expect("price list tier should resolve");

    assert_eq!(resolved.amount, dec!(70.00));
    assert_eq!(resolved.price_list_id, Some(price_list_id));
    assert_eq!(resolved.min_quantity, Some(5));
}

#[tokio::test]
async fn test_set_price_list_tier_rejects_inactive_price_list() {
    let (db, service, catalog) = setup().await;
    let tenant_id = Uuid::new_v4();
    let actor_id = Uuid::new_v4();
    let (_product_id, variant_id) = create_test_product(&catalog, tenant_id).await;
    let price_list_id = create_price_list(&db, tenant_id, "draft", None, None).await;

    let result = service
        .set_price_list_tier(
            tenant_id,
            actor_id,
            variant_id,
            price_list_id,
            "USD",
            dec!(70.00),
            None,
            None,
            None,
        )
        .await;

    assert!(result.is_err());
    match result.unwrap_err() {
        CommerceError::Validation(message) => {
            assert!(message.contains("active price list"));
        }
        other => panic!("Expected Validation error, got {other:?}"),
    }
}

#[tokio::test]
async fn test_set_price_list_tier_rejects_price_list_not_active_yet() {
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

    let result = service
        .set_price_list_tier(
            tenant_id,
            actor_id,
            variant_id,
            price_list_id,
            "USD",
            dec!(70.00),
            None,
            None,
            None,
        )
        .await;

    assert!(result.is_err());
    match result.unwrap_err() {
        CommerceError::Validation(message) => {
            assert!(message.contains("not active yet"));
        }
        other => panic!("Expected Validation error, got {other:?}"),
    }
}

#[tokio::test]
async fn test_set_price_list_scope_rejects_expired_price_list() {
    let (db, service, _catalog) = setup().await;
    let tenant_id = Uuid::new_v4();
    let actor_id = Uuid::new_v4();
    let price_list_id = create_price_list(
        &db,
        tenant_id,
        "active",
        None,
        Some(chrono::Utc::now() - chrono::Duration::days(1)),
    )
    .await;

    let result = service
        .set_price_list_scope(
            tenant_id,
            actor_id,
            price_list_id,
            Some(Uuid::new_v4()),
            Some("web-store".to_string()),
        )
        .await;

    assert!(result.is_err());
    match result.unwrap_err() {
        CommerceError::Validation(message) => {
            assert!(message.contains("already expired"));
        }
        other => panic!("Expected Validation error, got {other:?}"),
    }
}

#[tokio::test]
async fn test_set_price_list_tier_with_channel_inherits_price_list_scope() {
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
        .set_price_list_tier_with_channel(
            tenant_id,
            actor_id,
            variant_id,
            price_list_id,
            "USD",
            dec!(75.00),
            Some(dec!(100.00)),
            None,
            None,
            None,
            None,
        )
        .await
        .unwrap();

    let override_row = service
        .get_variant_prices(variant_id)
        .await
        .unwrap()
        .into_iter()
        .find(|price| price.price_list_id == Some(price_list_id))
        .expect("price-list override row should exist");

    assert_eq!(override_row.channel_id, Some(channel_id));
    assert_eq!(override_row.channel_slug.as_deref(), Some("web-store"));

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
                quantity: Some(1),
            },
        )
        .await
        .unwrap()
        .expect("scoped price-list override should resolve");

    assert_eq!(resolved.amount, dec!(75.00));
    assert_eq!(resolved.channel_id, Some(channel_id));
    assert_eq!(resolved.channel_slug.as_deref(), Some("web-store"));
}

#[tokio::test]
async fn test_set_price_list_tier_with_channel_rejects_scope_mismatch() {
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

    let error = service
        .set_price_list_tier_with_channel(
            tenant_id,
            actor_id,
            variant_id,
            price_list_id,
            "USD",
            dec!(75.00),
            None,
            Some(Uuid::new_v4()),
            Some("mobile-app".to_string()),
            None,
            None,
        )
        .await
        .unwrap_err();

    match error {
        CommerceError::Validation(message) => {
            assert!(message.contains("match the price list channel scope"));
        }
        other => panic!("Expected Validation error, got {other:?}"),
    }
}

#[tokio::test]
async fn test_set_price_list_scope_propagates_to_existing_override_rows() {
    let (db, service, catalog) = setup().await;
    let tenant_id = Uuid::new_v4();
    let actor_id = Uuid::new_v4();
    let channel_id = Uuid::new_v4();
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

    let updated_scope = service
        .set_price_list_scope(
            tenant_id,
            actor_id,
            price_list_id,
            Some(channel_id),
            Some("web-store".to_string()),
        )
        .await
        .unwrap();

    assert_eq!(updated_scope.channel_id, Some(channel_id));
    assert_eq!(updated_scope.channel_slug.as_deref(), Some("web-store"));

    let override_row = service
        .get_variant_prices(variant_id)
        .await
        .unwrap()
        .into_iter()
        .find(|price| price.price_list_id == Some(price_list_id))
        .expect("price-list override row should exist");

    assert_eq!(override_row.channel_id, Some(channel_id));
    assert_eq!(override_row.channel_slug.as_deref(), Some("web-store"));

    let visible_lists = service
        .list_active_price_lists_for_channel(
            tenant_id,
            Some(channel_id),
            Some("web-store"),
            Some("en"),
            Some("en"),
        )
        .await
        .unwrap();
    assert!(visible_lists.iter().any(|list| list.id == price_list_id));

    let hidden_lists = service
        .list_active_price_lists_for_channel(
            tenant_id,
            Some(Uuid::new_v4()),
            Some("mobile-app"),
            Some("en"),
            Some("en"),
        )
        .await
        .unwrap();
    assert!(!hidden_lists.iter().any(|list| list.id == price_list_id));

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
                quantity: Some(1),
            },
        )
        .await
        .unwrap()
        .expect("updated scoped override should resolve");

    assert_eq!(resolved.amount, dec!(80.00));
    assert_eq!(resolved.channel_id, Some(channel_id));
    assert_eq!(resolved.channel_slug.as_deref(), Some("web-store"));
}

#[tokio::test]
async fn test_set_price_list_percentage_rule_clears_rule_metadata() {
    let (db, service, _catalog) = setup().await;
    let tenant_id = Uuid::new_v4();
    let actor_id = Uuid::new_v4();
    let price_list_id = create_price_list(&db, tenant_id, "active", None, None).await;

    let applied = service
        .set_price_list_percentage_rule(tenant_id, actor_id, price_list_id, Some(dec!(12.5)))
        .await
        .unwrap()
        .expect("rule metadata should be returned");
    assert_eq!(applied.adjustment_percent, dec!(12.5));

    let cleared = service
        .set_price_list_percentage_rule(tenant_id, actor_id, price_list_id, None)
        .await
        .unwrap();
    assert!(cleared.is_none());

    let option = service
        .list_active_price_lists(tenant_id, Some("en"), Some("en"))
        .await
        .unwrap()
        .into_iter()
        .find(|list| list.id == price_list_id)
        .expect("active price list should stay visible");

    assert_eq!(option.rule_kind, None);
    assert_eq!(option.adjustment_percent, None);
}
