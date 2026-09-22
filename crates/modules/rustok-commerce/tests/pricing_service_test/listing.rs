use super::*;

#[tokio::test]
async fn test_list_admin_product_pricing_uses_locale_fallback_and_preserves_seller_id() {
    let (_db, service, catalog) = setup().await;
    let tenant_id = Uuid::new_v4();
    let product_id = create_test_product_with_seller(&catalog, tenant_id, "seller-alpha").await;

    let list = service
        .list_admin_product_pricing_with_locale_fallback(
            tenant_id,
            "ru",
            Some("en"),
            Some("Seller Product"),
            None,
            1,
            24,
        )
        .await
        .unwrap();

    let item = list
        .items
        .into_iter()
        .find(|item| item.id == product_id)
        .expect("admin pricing item should be present");

    assert_eq!(item.seller_id.as_deref(), Some("seller-alpha"));
    assert_eq!(item.title, "Seller Product");
    assert_eq!(item.handle, item.handle.to_lowercase());
    assert_eq!(item.shipping_profile_slug.as_deref(), Some("default"));
}

#[tokio::test]
async fn test_list_active_price_lists_only_returns_currently_active_lists() {
    let (db, service, _catalog) = setup().await;
    let tenant_id = Uuid::new_v4();
    let now = chrono::Utc::now();

    let active_id = create_price_list(&db, tenant_id, "active", None, None).await;
    let future_id = create_price_list(
        &db,
        tenant_id,
        "active",
        Some(now + chrono::Duration::days(1)),
        None,
    )
    .await;
    let expired_id = create_price_list(
        &db,
        tenant_id,
        "active",
        None,
        Some(now - chrono::Duration::days(1)),
    )
    .await;
    let draft_id = create_price_list(&db, tenant_id, "draft", None, None).await;

    let lists = service
        .list_active_price_lists(tenant_id, Some("en"), Some("en"))
        .await
        .unwrap();

    assert!(lists.iter().any(|list| list.id == active_id));
    assert!(!lists.iter().any(|list| list.id == future_id));
    assert!(!lists.iter().any(|list| list.id == expired_id));
    assert!(!lists.iter().any(|list| list.id == draft_id));
}

#[tokio::test]
async fn test_list_active_price_lists_exposes_rule_metadata() {
    let (db, service, _catalog) = setup().await;
    let tenant_id = Uuid::new_v4();
    let actor_id = Uuid::new_v4();
    let price_list_id = create_price_list(&db, tenant_id, "active", None, None).await;

    service
        .set_price_list_percentage_rule(tenant_id, actor_id, price_list_id, Some(dec!(12.5)))
        .await
        .unwrap();

    let lists = service
        .list_active_price_lists(tenant_id, Some("en"), Some("en"))
        .await
        .unwrap();
    let option = lists
        .into_iter()
        .find(|list| list.id == price_list_id)
        .expect("active price list should be present");

    assert_eq!(option.rule_kind.as_deref(), Some("percentage_discount"));
    assert_eq!(option.adjustment_percent, Some(dec!(12.5)));
}

#[tokio::test]
async fn test_list_active_price_lists_filters_by_channel_scope() {
    let (db, service, _catalog) = setup().await;
    let tenant_id = Uuid::new_v4();
    let channel_id = Uuid::new_v4();
    let global_id = create_price_list(&db, tenant_id, "active", None, None).await;
    let scoped_id = create_price_list_with_channel(
        &db,
        tenant_id,
        "active",
        None,
        None,
        Some(channel_id),
        Some("web-store"),
    )
    .await;

    let web_lists = service
        .list_active_price_lists_for_channel(
            tenant_id,
            Some(channel_id),
            Some("web-store"),
            Some("en"),
            Some("en"),
        )
        .await
        .unwrap();
    let mobile_lists = service
        .list_active_price_lists_for_channel(
            tenant_id,
            Some(Uuid::new_v4()),
            Some("mobile-app"),
            Some("en"),
            Some("en"),
        )
        .await
        .unwrap();

    assert!(web_lists.iter().any(|list| list.id == global_id));
    assert!(web_lists.iter().any(|list| list.id == scoped_id));
    assert!(mobile_lists.iter().any(|list| list.id == global_id));
    assert!(!mobile_lists.iter().any(|list| list.id == scoped_id));
}
