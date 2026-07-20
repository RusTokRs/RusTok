use super::*;

#[test]
fn guest_cart_allows_missing_customer_context() {
    let cart = sample_cart(None);
    assert!(ensure_store_cart_access(&cart, None).is_ok());
}

#[test]
fn customer_owned_cart_allows_matching_customer() {
    let customer_id = Uuid::new_v4();
    let cart = sample_cart(Some(customer_id));
    assert!(ensure_store_cart_access(&cart, Some(customer_id)).is_ok());
}

#[test]
fn customer_owned_cart_rejects_missing_customer_context() {
    let cart = sample_cart(Some(Uuid::new_v4()));
    let error = ensure_store_cart_access(&cart, None).expect_err("customer auth required");
    assert_eq!(error.status, StatusCode::UNAUTHORIZED);
    assert_eq!(error.code, "commerce_store_denied");
    assert_eq!(error.message, "Cart belongs to another customer");
}

#[test]
fn customer_owned_cart_rejects_different_customer() {
    let cart = sample_cart(Some(Uuid::new_v4()));
    let error = ensure_store_cart_access(&cart, Some(Uuid::new_v4()))
        .expect_err("foreign customer access must be rejected");
    assert_eq!(error.status, StatusCode::UNAUTHORIZED);
    assert_eq!(error.code, "commerce_store_denied");
    assert_eq!(error.message, "Cart belongs to another customer");
}

#[test]
fn payment_collection_allows_non_completed_cart() {
    let mut cart = sample_cart(None);
    cart.status = "open".to_string();
    assert!(super::super::ensure_cart_allows_payment_collection(&cart).is_ok());
}

#[test]
fn payment_collection_rejects_completed_cart() {
    let mut cart = sample_cart(None);
    cart.status = "completed".to_string();
    let error = super::super::ensure_cart_allows_payment_collection(&cart)
        .expect_err("completed carts must reject payment collection creation");
    assert_eq!(error.status, StatusCode::BAD_REQUEST);
    assert_eq!(error.code, "commerce_store_invalid");
    assert_eq!(
        error.message,
        "Cannot create payment collection for completed cart"
    );
}

#[test]
fn guest_checkout_uses_nil_actor_without_auth() {
    assert_eq!(checkout_actor_id(None), Uuid::nil());
}

#[test]
fn authenticated_checkout_uses_user_actor() {
    let user_id = Uuid::new_v4();
    let auth = AuthContext {
        user_id,
        session_id: Uuid::new_v4(),
        tenant_id: Uuid::new_v4(),
        permissions: vec![],
        client_id: None,
        scopes: vec![],
        grant_type: "direct".to_string(),
    };

    assert_eq!(checkout_actor_id(Some(&auth)), user_id);
}

#[test]
fn cart_context_patch_keeps_existing_values_when_fields_are_omitted() {
    let region_id = Uuid::new_v4();
    let shipping_option_id = Uuid::new_v4();
    let mut cart = sample_cart(None);
    cart.email = Some("keep@example.com".to_string());
    cart.region_id = Some(region_id);
    cart.country_code = Some("DE".to_string());
    cart.locale_code = Some("de".to_string());
    cart.selected_shipping_option_id = Some(shipping_option_id);

    let requested = requested_cart_context(
        &cart,
        &sample_request_context("en"),
        StoreCartContextPatch {
            email: None,
            region_id: None,
            country_code: None,
            locale: None,
            selected_shipping_option_id: None,
            shipping_selections: None,
        },
    );

    assert_eq!(
        requested,
        RequestedCartContext {
            email: Some("keep@example.com".to_string()),
            region_id: Some(region_id),
            country_code: Some("DE".to_string()),
            locale: Some("de".to_string()),
            selected_shipping_option_id: Some(shipping_option_id),
            shipping_selections: Vec::new(),
        }
    );
}

#[test]
fn cart_context_patch_applies_explicit_values() {
    let region_id = Uuid::new_v4();
    let shipping_option_id = Uuid::new_v4();
    let cart = sample_cart(None);

    let requested = requested_cart_context(
        &cart,
        &sample_request_context("en"),
        StoreCartContextPatch {
            email: Some(Some("set@example.com".to_string())),
            region_id: Some(Some(region_id)),
            country_code: Some(Some("fr".to_string())),
            locale: Some(Some("fr".to_string())),
            selected_shipping_option_id: Some(Some(shipping_option_id)),
            shipping_selections: None,
        },
    );

    assert_eq!(
        requested,
        RequestedCartContext {
            email: Some("set@example.com".to_string()),
            region_id: Some(region_id),
            country_code: Some("fr".to_string()),
            locale: Some("fr".to_string()),
            selected_shipping_option_id: Some(shipping_option_id),
            shipping_selections: Vec::new(),
        }
    );
}

#[test]
fn cart_context_patch_clears_country_when_region_is_explicitly_cleared() {
    let mut cart = sample_cart(None);
    cart.region_id = Some(Uuid::new_v4());
    cart.country_code = Some("DE".to_string());
    cart.locale_code = Some("de".to_string());

    let requested = requested_cart_context(
        &cart,
        &sample_request_context("en"),
        StoreCartContextPatch {
            email: None,
            region_id: Some(None),
            country_code: None,
            locale: None,
            selected_shipping_option_id: None,
            shipping_selections: None,
        },
    );

    assert_eq!(
        requested,
        RequestedCartContext {
            email: Some("buyer@example.com".to_string()),
            region_id: None,
            country_code: None,
            locale: Some("de".to_string()),
            selected_shipping_option_id: None,
            shipping_selections: Vec::new(),
        }
    );
}

#[test]
fn cart_context_patch_can_clear_individual_fields_and_falls_back_to_request_locale() {
    let region_id = Uuid::new_v4();
    let shipping_option_id = Uuid::new_v4();
    let mut cart = sample_cart(None);
    cart.region_id = Some(region_id);
    cart.country_code = Some("DE".to_string());
    cart.locale_code = Some("de".to_string());
    cart.selected_shipping_option_id = Some(shipping_option_id);

    let requested = requested_cart_context(
        &cart,
        &sample_request_context("en"),
        StoreCartContextPatch {
            email: Some(None),
            region_id: None,
            country_code: Some(None),
            locale: Some(None),
            selected_shipping_option_id: Some(None),
            shipping_selections: None,
        },
    );

    assert_eq!(
        requested,
        RequestedCartContext {
            email: None,
            region_id: Some(region_id),
            country_code: None,
            locale: Some("en".to_string()),
            selected_shipping_option_id: None,
            shipping_selections: Vec::new(),
        }
    );
}

#[test]
fn merge_metadata_keeps_existing_fields_and_overrides_conflicts() {
    let merged = merge_metadata(
        json!({
            "source": "request",
            "cart_context": { "locale": "de", "currency_code": "EUR" }
        }),
        json!({
            "cart_context": { "locale": "en" },
            "attempt": 2
        }),
    );

    assert_eq!(
        merged,
        json!({
            "source": "request",
            "cart_context": { "locale": "en" },
            "attempt": 2
        })
    );
}

#[test]
fn cart_context_metadata_embeds_storefront_context_for_payment_collection() {
    let tenant_id = Uuid::new_v4();
    let customer_id = Uuid::new_v4();
    let region_id = Uuid::new_v4();
    let shipping_option_id = Uuid::new_v4();
    let channel_id = Uuid::new_v4();
    let mut cart = sample_cart(Some(customer_id));
    cart.channel_id = Some(channel_id);
    cart.channel_slug = Some("web-store".to_string());
    cart.region_id = Some(region_id);
    cart.country_code = Some("DE".to_string());
    cart.locale_code = Some("de".to_string());
    cart.selected_shipping_option_id = Some(shipping_option_id);

    let metadata = cart_context_metadata(
        &cart,
        &StoreContextResponse {
            region: Some(RegionResponse {
                id: region_id,
                tenant_id,
                name: "Europe".to_string(),
                currency_code: "EUR".to_string(),
                tax_provider_id: None,
                tax_rate: Decimal::from(20),
                tax_included: true,
                country_tax_policies: vec![],
                countries: vec!["DE".to_string()],
                metadata: json!({}),
                created_at: chrono::Utc::now(),
                updated_at: chrono::Utc::now(),
                requested_locale: Some("de".to_string()),
                effective_locale: Some("de".to_string()),
                available_locales: vec!["en".to_string(), "de".to_string()],
                translations: Vec::new(),
            }),
            locale: "de".to_string(),
            default_locale: "en".to_string(),
            available_locales: vec!["en".to_string(), "de".to_string()],
            currency_code: Some("EUR".to_string()),
        },
    );

    assert_eq!(
        metadata,
        json!({
            "cart_context": {
                "channel_id": channel_id,
                "channel_slug": "web-store",
                "region_id": region_id,
                "country_code": "DE",
                "locale": "de",
                "currency_code": "USD",
                "selected_shipping_option_id": shipping_option_id,
                "shipping_selections": [],
                "customer_id": customer_id,
                "email": "buyer@example.com"
            }
        })
    );
}
