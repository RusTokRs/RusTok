use super::*;

#[tokio::test]
async fn admin_shipping_profiles_transport_supports_create_update_and_list() {
    let db = setup_test_db().await;
    support::ensure_commerce_schema(&db).await;
    let tenant_id = Uuid::new_v4();
    let actor_id = Uuid::new_v4();
    seed_tenant_context(&db, tenant_id).await;
    let tenant = TenantContext {
        id: tenant_id,
        name: "Admin Test Tenant".to_string(),
        slug: format!("admin-test-{tenant_id}"),
        domain: None,
        settings: json!({}),
        default_locale: "en".to_string(),
        is_active: true,
    };
    let auth = AuthContext {
        user_id: actor_id,
        session_id: Uuid::new_v4(),
        tenant_id,
        permissions: vec![
            Permission::FULFILLMENTS_READ,
            Permission::FULFILLMENTS_CREATE,
            Permission::FULFILLMENTS_UPDATE,
        ],
        client_id: None,
        scopes: vec![],
        grant_type: "direct".to_string(),
    };

    let app = admin_transport_router(test_app_context(db), tenant, auth);
    let create_response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/admin/shipping-profiles")
                .header("content-type", "application/json")
                .header("X-Tenant-ID", tenant_id.to_string())
                .body(Body::from(
                    json!({
                        "slug": " bulky-freight ",
                        "translations": [{
                            "locale": "en",
                            "name": "Bulky Freight",
                            "description": "Large parcel handling"
                        }],
                        "metadata": { "source": "admin-shipping-profiles" }
                    })
                    .to_string(),
                ))
                .expect("request"),
        )
        .await
        .expect("create request should succeed");
    let create_status = create_response.status();
    let create_body = to_bytes(create_response.into_body(), usize::MAX)
        .await
        .expect("create response should read");
    assert_eq!(
        create_status,
        StatusCode::CREATED,
        "unexpected create body: {}",
        String::from_utf8_lossy(&create_body)
    );

    let created: serde_json::Value =
        serde_json::from_slice(&create_body).expect("create response should be JSON");
    let profile_id = created["id"]
        .as_str()
        .expect("created shipping profile id should be present");
    assert_eq!(created["slug"], json!("bulky-freight"));

    let list_response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("GET")
                .uri("/admin/shipping-profiles?search=bulky&page=1&per_page=10")
                .header("X-Tenant-ID", tenant_id.to_string())
                .body(Body::empty())
                .expect("request"),
        )
        .await
        .expect("list request should succeed");
    let list_status = list_response.status();
    let list_body = to_bytes(list_response.into_body(), usize::MAX)
        .await
        .expect("list response should read");
    assert_eq!(
        list_status,
        StatusCode::OK,
        "unexpected list body: {}",
        String::from_utf8_lossy(&list_body)
    );
    let listed: serde_json::Value =
        serde_json::from_slice(&list_body).expect("list response should be JSON");
    assert_eq!(listed["meta"]["total"], json!(1));
    assert_eq!(listed["data"][0]["id"], json!(profile_id));

    let update_response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri(format!("/admin/shipping-profiles/{profile_id}"))
                .header("content-type", "application/json")
                .header("X-Tenant-ID", tenant_id.to_string())
                .body(Body::from(
                    serde_json::to_string(&crate::dto::UpdateShippingProfileInput {
                        slug: None,
                        translations: Some(vec![crate::dto::ShippingProfileTranslationInput {
                            locale: "en".to_string(),
                            name: "Oversize Freight".to_string(),
                            description: Some("Updated profile".to_string()),
                        }]),
                        metadata: Some(json!({ "updated": true })),
                    })
                    .expect("update payload should serialize"),
                ))
                .expect("request"),
        )
        .await
        .expect("update request should succeed");
    let update_status = update_response.status();
    let update_body = to_bytes(update_response.into_body(), usize::MAX)
        .await
        .expect("update response should read");
    assert_eq!(
        update_status,
        StatusCode::OK,
        "unexpected update body: {}",
        String::from_utf8_lossy(&update_body)
    );
    let updated: serde_json::Value =
        serde_json::from_slice(&update_body).expect("update response should be JSON");
    assert_eq!(updated["name"], json!("Oversize Freight"));
    assert_eq!(updated["metadata"]["updated"], json!(true));

    let show_response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("GET")
                .uri(format!("/admin/shipping-profiles/{profile_id}"))
                .header("X-Tenant-ID", tenant_id.to_string())
                .body(Body::empty())
                .expect("request"),
        )
        .await
        .expect("show request should succeed");
    let show_status = show_response.status();
    let show_body = to_bytes(show_response.into_body(), usize::MAX)
        .await
        .expect("show response should read");
    assert_eq!(
        show_status,
        StatusCode::OK,
        "unexpected show body: {}",
        String::from_utf8_lossy(&show_body)
    );
    let shown: serde_json::Value =
        serde_json::from_slice(&show_body).expect("show response should be JSON");
    assert_eq!(shown["id"], json!(profile_id));
    assert_eq!(shown["slug"], json!("bulky-freight"));

    let deactivate_response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri(format!("/admin/shipping-profiles/{profile_id}/deactivate"))
                .header("X-Tenant-ID", tenant_id.to_string())
                .body(Body::empty())
                .expect("request"),
        )
        .await
        .expect("deactivate request should succeed");
    let deactivate_body = to_bytes(deactivate_response.into_body(), usize::MAX)
        .await
        .expect("deactivate response should read");
    let deactivated: serde_json::Value =
        serde_json::from_slice(&deactivate_body).expect("deactivate response should be JSON");
    assert_eq!(deactivated["active"], json!(false));

    let reactivate_response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri(format!("/admin/shipping-profiles/{profile_id}/reactivate"))
                .header("X-Tenant-ID", tenant_id.to_string())
                .body(Body::empty())
                .expect("request"),
        )
        .await
        .expect("reactivate request should succeed");
    let reactivate_body = to_bytes(reactivate_response.into_body(), usize::MAX)
        .await
        .expect("reactivate response should read");
    let reactivated: serde_json::Value =
        serde_json::from_slice(&reactivate_body).expect("reactivate response should be JSON");
    assert_eq!(reactivated["active"], json!(true));
}

#[tokio::test]
async fn admin_shipping_options_transport_supports_create_update_and_list() {
    let db = setup_test_db().await;
    support::ensure_commerce_schema(&db).await;
    let tenant_id = Uuid::new_v4();
    let actor_id = Uuid::new_v4();
    seed_tenant_context(&db, tenant_id).await;
    ShippingProfileService::new(db.clone())
        .create_shipping_profile(
            tenant_id,
            crate::dto::CreateShippingProfileInput {
                slug: "bulky".to_string(),
                translations: vec![crate::dto::ShippingProfileTranslationInput {
                    locale: "en".to_string(),
                    name: "Bulky".to_string(),
                    description: None,
                }],
                metadata: json!({}),
            },
        )
        .await
        .expect("bulky profile should be created");
    ShippingProfileService::new(db.clone())
        .create_shipping_profile(
            tenant_id,
            crate::dto::CreateShippingProfileInput {
                slug: "cold-chain".to_string(),
                translations: vec![crate::dto::ShippingProfileTranslationInput {
                    locale: "en".to_string(),
                    name: "Cold Chain".to_string(),
                    description: None,
                }],
                metadata: json!({}),
            },
        )
        .await
        .expect("cold-chain profile should be created");
    let tenant = TenantContext {
        id: tenant_id,
        name: "Admin Test Tenant".to_string(),
        slug: format!("admin-test-{tenant_id}"),
        domain: None,
        settings: json!({}),
        default_locale: "en".to_string(),
        is_active: true,
    };
    let auth = AuthContext {
        user_id: actor_id,
        session_id: Uuid::new_v4(),
        tenant_id,
        permissions: vec![
            Permission::FULFILLMENTS_READ,
            Permission::FULFILLMENTS_CREATE,
            Permission::FULFILLMENTS_UPDATE,
        ],
        client_id: None,
        scopes: vec![],
        grant_type: "direct".to_string(),
    };

    let app = admin_transport_router(test_app_context(db), tenant, auth);
    let create_response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/admin/shipping-options")
                .header("content-type", "application/json")
                .header("X-Tenant-ID", tenant_id.to_string())
                .body(Body::from(
                    json!({
                        "translations": [{
                            "locale": "en",
                            "name": "Bulky Freight"
                        }],
                        "currency_code": "eur",
                        "amount": "29.99",
                        "provider_id": " manual ",
                        "allowed_shipping_profile_slugs": [" bulky ", "cold-chain", "bulky"],
                        "metadata": { "source": "admin-shipping-options" }
                    })
                    .to_string(),
                ))
                .expect("request"),
        )
        .await
        .expect("create request should succeed");
    let create_status = create_response.status();
    let create_body = to_bytes(create_response.into_body(), usize::MAX)
        .await
        .expect("create response should read");
    assert_eq!(
        create_status,
        StatusCode::CREATED,
        "unexpected create body: {}",
        String::from_utf8_lossy(&create_body)
    );

    let created: serde_json::Value =
        serde_json::from_slice(&create_body).expect("create response should be JSON");
    let option_id = created["id"]
        .as_str()
        .expect("created shipping option id should be present");
    assert_eq!(
        created["allowed_shipping_profile_slugs"],
        json!(["bulky", "cold-chain"])
    );

    let list_response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("GET")
                .uri("/admin/shipping-options?search=freight&page=1&per_page=10")
                .header("X-Tenant-ID", tenant_id.to_string())
                .body(Body::empty())
                .expect("request"),
        )
        .await
        .expect("list request should succeed");
    let list_status = list_response.status();
    let list_body = to_bytes(list_response.into_body(), usize::MAX)
        .await
        .expect("list response should read");
    assert_eq!(
        list_status,
        StatusCode::OK,
        "unexpected list body: {}",
        String::from_utf8_lossy(&list_body)
    );
    let listed: serde_json::Value =
        serde_json::from_slice(&list_body).expect("list response should be JSON");
    assert_eq!(listed["meta"]["total"], json!(1));
    assert_eq!(listed["data"][0]["id"], json!(option_id));

    let update_response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri(format!("/admin/shipping-options/{option_id}"))
                .header("content-type", "application/json")
                .header("X-Tenant-ID", tenant_id.to_string())
                .body(Body::from(
                    serde_json::to_string(&UpdateShippingOptionInput {
                        translations: Some(vec![crate::dto::ShippingOptionTranslationInput {
                            locale: "en".to_string(),
                            name: "Cold Chain Freight".to_string(),
                        }]),
                        currency_code: Some("usd".to_string()),
                        amount: Some(Decimal::from_str("39.99").expect("valid decimal")),
                        provider_id: Some("custom-provider".to_string()),
                        allowed_shipping_profile_slugs: Some(vec!["cold-chain".to_string()]),
                        metadata: Some(json!({ "updated": true })),
                    })
                    .expect("update payload should serialize"),
                ))
                .expect("request"),
        )
        .await
        .expect("update request should succeed");
    let update_status = update_response.status();
    let update_body = to_bytes(update_response.into_body(), usize::MAX)
        .await
        .expect("update response should read");
    assert_eq!(
        update_status,
        StatusCode::OK,
        "unexpected update body: {}",
        String::from_utf8_lossy(&update_body)
    );
    let updated: serde_json::Value =
        serde_json::from_slice(&update_body).expect("update response should be JSON");
    assert_eq!(updated["name"], json!("Cold Chain Freight"));
    assert_eq!(updated["currency_code"], json!("USD"));
    assert_eq!(updated["provider_id"], json!("custom-provider"));
    assert_eq!(
        updated["allowed_shipping_profile_slugs"],
        json!(["cold-chain"])
    );

    let show_response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("GET")
                .uri(format!("/admin/shipping-options/{option_id}"))
                .header("X-Tenant-ID", tenant_id.to_string())
                .body(Body::empty())
                .expect("request"),
        )
        .await
        .expect("show request should succeed");
    let show_status = show_response.status();
    let show_body = to_bytes(show_response.into_body(), usize::MAX)
        .await
        .expect("show response should read");
    assert_eq!(
        show_status,
        StatusCode::OK,
        "unexpected show body: {}",
        String::from_utf8_lossy(&show_body)
    );
    let shown: serde_json::Value =
        serde_json::from_slice(&show_body).expect("show response should be JSON");
    assert_eq!(shown["id"], json!(option_id));
    assert_eq!(shown["metadata"]["updated"], json!(true));
    assert_eq!(
        shown["metadata"]["shipping_profiles"]["allowed_slugs"],
        json!(["cold-chain"])
    );

    let deactivate_response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri(format!("/admin/shipping-options/{option_id}/deactivate"))
                .header("X-Tenant-ID", tenant_id.to_string())
                .body(Body::empty())
                .expect("request"),
        )
        .await
        .expect("deactivate request should succeed");
    let deactivate_body = to_bytes(deactivate_response.into_body(), usize::MAX)
        .await
        .expect("deactivate response should read");
    let deactivated: serde_json::Value =
        serde_json::from_slice(&deactivate_body).expect("deactivate response should be JSON");
    assert_eq!(deactivated["active"], json!(false));

    let inactive_list_response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("GET")
                .uri("/admin/shipping-options?active=false&page=1&per_page=10")
                .header("X-Tenant-ID", tenant_id.to_string())
                .body(Body::empty())
                .expect("request"),
        )
        .await
        .expect("inactive list request should succeed");
    let inactive_list_body = to_bytes(inactive_list_response.into_body(), usize::MAX)
        .await
        .expect("inactive list response should read");
    let inactive_listed: serde_json::Value =
        serde_json::from_slice(&inactive_list_body).expect("inactive list should be JSON");
    assert_eq!(inactive_listed["meta"]["total"], json!(1));
    assert_eq!(inactive_listed["data"][0]["id"], json!(option_id));
    assert_eq!(inactive_listed["data"][0]["active"], json!(false));

    let reactivate_response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri(format!("/admin/shipping-options/{option_id}/reactivate"))
                .header("X-Tenant-ID", tenant_id.to_string())
                .body(Body::empty())
                .expect("request"),
        )
        .await
        .expect("reactivate request should succeed");
    let reactivate_body = to_bytes(reactivate_response.into_body(), usize::MAX)
        .await
        .expect("reactivate response should read");
    let reactivated: serde_json::Value =
        serde_json::from_slice(&reactivate_body).expect("reactivate response should be JSON");
    assert_eq!(reactivated["active"], json!(true));
}
