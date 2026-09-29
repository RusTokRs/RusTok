        json!("promo-shipping-store")
    );
    assert_eq!(cart["adjustments"][0]["amount"], json!("4.99"));
    assert_eq!(cart["adjustments"][0]["currency_code"], json!("EUR"));
    assert_eq!(
        cart["adjustments"][0]["metadata"],
        json!({ "campaign": "shipping-half-off", "kind": "fixed_discount", "scope": "shipping", "fixed_amount": "4.99" })
    );
}

#[tokio::test]
async fn store_cart_transport_rejects_customer_owned_cart_for_another_customer() {
    let db = setup_test_db().await;
    support::ensure_commerce_schema(&db).await;
    let tenant_id = Uuid::new_v4();
    let owner_user_id = Uuid::new_v4();
    let other_user_id = Uuid::new_v4();
    seed_store_tenant_context(&db, tenant_id).await;
    let tenant = TenantContext {
        id: tenant_id,
        name: "Store Test Tenant".to_string(),
        slug: format!("store-test-{tenant_id}"),
        domain: None,
        settings: json!({}),
        default_locale: "en".to_string(),
        is_active: true,
    };
    let owner_auth = AuthContext {
        user_id: owner_user_id,
        session_id: Uuid::new_v4(),
        tenant_id,
        permissions: vec![Permission::ORDERS_READ],
        client_id: None,
        scopes: vec![],
        grant_type: "direct".to_string(),
    };
    let other_auth = AuthContext {
        user_id: other_user_id,
        session_id: Uuid::new_v4(),
        tenant_id,
        permissions: vec![Permission::ORDERS_READ],
        client_id: None,
        scopes: vec![],
        grant_type: "direct".to_string(),
    };
    let owner_customer_id =
        create_customer_for_user(&db, tenant_id, owner_user_id, "cart-owner@example.com").await;
    create_customer_for_user(&db, tenant_id, other_user_id, "cart-other@example.com").await;
    let owner_app = commerce_transport_router_with_auth(
        test_app_context(db.clone()),
        tenant.clone(),
        Some(owner_auth),
    );
    let other_app =
        commerce_transport_router_with_auth(test_app_context(db), tenant, Some(other_auth));

    let create_cart_response = owner_app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/store/carts")
                .header("content-type", "application/json")
                .header("X-Tenant-ID", tenant_id.to_string())
                .body(Body::from(
                    json!({