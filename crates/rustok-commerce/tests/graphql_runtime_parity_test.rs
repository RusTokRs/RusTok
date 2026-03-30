use async_graphql::{EmptySubscription, Request, Schema};
use rust_decimal::Decimal;
use rustok_api::{AuthContext, RequestContext, TenantContext};
use rustok_commerce::dto::{
    AddCartLineItemInput, CompleteCheckoutInput, CreateCartInput, CreateCustomerInput,
    CreateFulfillmentInput, CreateOrderInput, CreateOrderLineItemInput,
    CreatePaymentCollectionInput, CreateProductInput, CreateShippingOptionInput,
    CreateVariantInput, PriceInput, ProductTranslationInput,
};
use rustok_commerce::graphql::{CommerceMutation, CommerceQuery};
use rustok_commerce::{
    CartService, CatalogService, CheckoutService, CustomerService, FulfillmentService,
    OrderService, PaymentService,
};
use rustok_core::Permission;
use rustok_region::dto::CreateRegionInput;
use rustok_region::services::RegionService;
use rustok_test_utils::{db::setup_test_db, helpers::unique_slug, mock_transactional_event_bus};
use sea_orm::{ConnectionTrait, DatabaseBackend, DatabaseConnection, Statement};
use serde_json::Value;
use std::str::FromStr;
use uuid::Uuid;

mod support;

type CommerceSchema = Schema<CommerceQuery, CommerceMutation, EmptySubscription>;

const STOREFRONT_QUERY_TEMPLATE: &str = r#"
query {
  storefrontProducts(locale: "de") {
    total
    items { title handle }
  }
  storefrontProduct(locale: "de", handle: "__HANDLE__") {
    translations { locale title handle }
  }
}
"#;

async fn setup() -> (DatabaseConnection, CatalogService, CartService) {
    let db = setup_test_db().await;
    support::ensure_commerce_schema(&db).await;
    let event_bus = mock_transactional_event_bus();
    (
        db.clone(),
        CatalogService::new(db.clone(), event_bus),
        CartService::new(db),
    )
}

async fn setup_checkout() -> (
    DatabaseConnection,
    CatalogService,
    CartService,
    CheckoutService,
    FulfillmentService,
) {
    let db = setup_test_db().await;
    support::ensure_commerce_schema(&db).await;
    let event_bus = mock_transactional_event_bus();
    (
        db.clone(),
        CatalogService::new(db.clone(), event_bus.clone()),
        CartService::new(db.clone()),
        CheckoutService::new(db.clone(), event_bus),
        FulfillmentService::new(db),
    )
}

fn create_product_input() -> CreateProductInput {
    CreateProductInput {
        translations: vec![
            ProductTranslationInput {
                locale: "en".to_string(),
                title: "Parity Product".to_string(),
                description: Some("English description".to_string()),
                handle: Some(unique_slug("parity-product")),
                meta_title: None,
                meta_description: None,
            },
            ProductTranslationInput {
                locale: "de".to_string(),
                title: "Paritaet Produkt".to_string(),
                description: Some("German description".to_string()),
                handle: Some(unique_slug("paritaet-produkt")),
                meta_title: None,
                meta_description: None,
            },
        ],
        options: vec![],
        variants: vec![CreateVariantInput {
            sku: Some("PARITY-SKU-1".to_string()),
            barcode: None,
            option1: Some("Default".to_string()),
            option2: None,
            option3: None,
            prices: vec![PriceInput {
                currency_code: "EUR".to_string(),
                amount: Decimal::from_str("19.99").expect("valid decimal"),
                compare_at_amount: None,
            }],
            inventory_quantity: 0,
            inventory_policy: "deny".to_string(),
            weight: None,
            weight_unit: None,
        }],
        vendor: Some("Parity Vendor".to_string()),
        product_type: Some("physical".to_string()),
        tags: vec![],
        publish: false,
        metadata: serde_json::json!({}),
    }
}

fn tenant_context(tenant_id: Uuid) -> TenantContext {
    TenantContext {
        id: tenant_id,
        name: "Parity Tenant".to_string(),
        slug: "parity-tenant".to_string(),
        domain: None,
        settings: serde_json::json!({}),
        default_locale: "en".to_string(),
        is_active: true,
    }
}

fn request_context(tenant_id: Uuid, locale: &str) -> RequestContext {
    RequestContext {
        tenant_id,
        user_id: None,
        channel_id: None,
        channel_slug: None,
        channel_resolution_source: None,
        locale: locale.to_string(),
    }
}

fn auth_context(tenant_id: Uuid) -> AuthContext {
    AuthContext {
        user_id: Uuid::new_v4(),
        session_id: Uuid::new_v4(),
        tenant_id,
        permissions: vec![Permission::PRODUCTS_READ, Permission::PRODUCTS_LIST],
        client_id: None,
        scopes: vec![],
        grant_type: "direct".to_string(),
    }
}

fn admin_order_auth_context(tenant_id: Uuid) -> AuthContext {
    AuthContext {
        user_id: Uuid::new_v4(),
        session_id: Uuid::new_v4(),
        tenant_id,
        permissions: vec![
            Permission::ORDERS_READ,
            Permission::ORDERS_LIST,
            Permission::ORDERS_UPDATE,
            Permission::PAYMENTS_READ,
            Permission::PAYMENTS_UPDATE,
            Permission::FULFILLMENTS_READ,
            Permission::FULFILLMENTS_UPDATE,
        ],
        client_id: None,
        scopes: vec![],
        grant_type: "direct".to_string(),
    }
}

fn customer_auth_context(tenant_id: Uuid, user_id: Uuid) -> AuthContext {
    AuthContext {
        user_id,
        session_id: Uuid::new_v4(),
        tenant_id,
        permissions: vec![],
        client_id: None,
        scopes: vec![],
        grant_type: "direct".to_string(),
    }
}

fn build_schema(
    db: &DatabaseConnection,
    tenant: TenantContext,
    request_context: RequestContext,
    auth: Option<AuthContext>,
) -> CommerceSchema {
    let event_bus = mock_transactional_event_bus();
    let mut builder = Schema::build(
        CommerceQuery::default(),
        CommerceMutation::default(),
        EmptySubscription,
    )
    .data(db.clone())
    .data(event_bus)
    .data(tenant)
    .data(request_context);

    if let Some(auth) = auth {
        builder = builder.data(auth);
    }

    builder.finish()
}

fn storefront_query(handle: &str) -> String {
    STOREFRONT_QUERY_TEMPLATE.replace("__HANDLE__", handle)
}

fn admin_query(tenant_id: Uuid, product_id: Uuid) -> String {
    format!(
        r#"
        query {{
          products(tenantId: "{tenant_id}", locale: "en", filter: {{ page: 1, perPage: 20 }}) {{
            total
            items {{ title handle }}
          }}
          product(tenantId: "{tenant_id}", id: "{product_id}", locale: "en") {{
            translations {{ locale title handle }}
          }}
        }}
        "#
    )
}

fn admin_order_mutation(
    tenant_id: Uuid,
    actor_id: Uuid,
    order_id: Uuid,
    payment_collection_id: Uuid,
    fulfillment_id: Uuid,
) -> String {
    format!(
        r#"
        mutation {{
          authorizePaymentCollection(
            tenantId: "{tenant_id}",
            id: "{payment_collection_id}",
            input: {{
              providerId: "manual"
              providerPaymentId: "graphql-pay-1"
              amount: "25.00"
              metadata: "{{\"source\":\"graphql-admin-order-parity\",\"step\":\"authorize\"}}"
            }}
          ) {{
            status
            authorizedAmount
          }}
          capturePaymentCollection(
            tenantId: "{tenant_id}",
            id: "{payment_collection_id}",
            input: {{
              amount: "25.00"
              metadata: "{{\"source\":\"graphql-admin-order-parity\",\"step\":\"capture\"}}"
            }}
          ) {{
            status
            capturedAmount
          }}
          markOrderPaid(
            tenantId: "{tenant_id}",
            userId: "{actor_id}",
            id: "{order_id}",
            input: {{
              paymentId: "graphql-pay-1"
              paymentMethod: "manual"
            }}
          ) {{
            status
            paymentId
            paymentMethod
          }}
          shipFulfillment(
            tenantId: "{tenant_id}",
            id: "{fulfillment_id}",
            input: {{
              carrier: "manual"
              trackingNumber: "TRACK-789"
              metadata: "{{\"source\":\"graphql-admin-order-parity\",\"step\":\"ship-fulfillment\"}}"
            }}
          ) {{
            status
            trackingNumber
          }}
          deliverFulfillment(
            tenantId: "{tenant_id}",
            id: "{fulfillment_id}",
            input: {{
              deliveredNote: "Left at reception"
              metadata: "{{\"source\":\"graphql-admin-order-parity\",\"step\":\"deliver-fulfillment\"}}"
            }}
          ) {{
            status
            deliveredNote
          }}
          shipOrder(
            tenantId: "{tenant_id}",
            userId: "{actor_id}",
            id: "{order_id}",
            input: {{
              trackingNumber: "TRACK-789"
              carrier: "manual"
            }}
          ) {{
            status
            trackingNumber
            carrier
          }}
          deliverOrder(
            tenantId: "{tenant_id}",
            userId: "{actor_id}",
            id: "{order_id}",
            input: {{
              deliveredSignature: "signed-by-customer"
            }}
          ) {{
            status
            deliveredSignature
          }}
        }}
        "#
    )
}

fn admin_order_parity_query(
    tenant_id: Uuid,
    order_id: Uuid,
    payment_collection_id: Uuid,
    fulfillment_id: Uuid,
) -> String {
    format!(
        r#"
        query {{
          order(tenantId: "{tenant_id}", id: "{order_id}") {{
            order {{
              id
              status
              paymentId
              paymentMethod
              trackingNumber
              carrier
              deliveredSignature
            }}
            paymentCollection {{
              id
              status
              authorizedAmount
              capturedAmount
            }}
            fulfillment {{
              id
              status
              trackingNumber
              deliveredNote
            }}
          }}
          orders(tenantId: "{tenant_id}", filter: {{ page: 1, perPage: 20, status: "delivered" }}) {{
            total
            items {{
              id
              status
              trackingNumber
              deliveredSignature
            }}
          }}
          paymentCollection(tenantId: "{tenant_id}", id: "{payment_collection_id}") {{
            id
            status
            providerId
            authorizedAmount
            capturedAmount
            payments {{
              providerPaymentId
              status
              capturedAmount
            }}
          }}
          fulfillment(tenantId: "{tenant_id}", id: "{fulfillment_id}") {{
            id
            status
            trackingNumber
            deliveredNote
          }}
          paymentCollections(
            tenantId: "{tenant_id}",
            filter: {{ page: 1, perPage: 20, orderId: "{order_id}", status: "captured" }}
          ) {{
            total
            items {{
              id
              status
              orderId
            }}
          }}
          fulfillments(
            tenantId: "{tenant_id}",
            filter: {{ page: 1, perPage: 20, orderId: "{order_id}", status: "delivered" }}
          ) {{
            total
            items {{
              id
              status
              orderId
              trackingNumber
            }}
          }}
        }}
        "#
    )
}

fn storefront_customer_order_query(tenant_id: Uuid, order_id: Uuid) -> String {
    format!(
        r#"
        query {{
          storefrontMe(tenantId: "{tenant_id}") {{
            id
            email
            locale
          }}
          storefrontOrder(tenantId: "{tenant_id}", id: "{order_id}") {{
            id
            customerId
            status
            currencyCode
            totalAmount
            lineItems {{
              title
              quantity
              currencyCode
            }}
          }}
        }}
        "#
    )
}

fn storefront_checkout_mutation(tenant_id: Uuid, cart_id: Uuid) -> String {
    format!(
        r#"
        mutation {{
          createStorefrontPaymentCollection(
            tenantId: "{tenant_id}",
            input: {{
              cartId: "{cart_id}"
              metadata: "{{\"source\":\"storefront-graphql-checkout\",\"step\":\"payment\"}}"
            }}
          ) {{
            id
            status
            amount
          }}
          completeStorefrontCheckout(
            tenantId: "{tenant_id}",
            input: {{
              cartId: "{cart_id}"
              createFulfillment: true
              metadata: "{{\"source\":\"storefront-graphql-checkout\",\"step\":\"complete\"}}"
            }}
          ) {{
            cart {{ id status }}
            order {{ id status }}
            paymentCollection {{ id status cartId orderId }}
            fulfillment {{ id status }}
            context {{ locale currencyCode }}
          }}
        }}
        "#
    )
}

fn storefront_cart_flow_mutation(tenant_id: Uuid) -> String {
    format!(
        r#"
        mutation {{
          createStorefrontCart(
            tenantId: "{tenant_id}",
            input: {{
              email: "guest-cart@example.com"
              currencyCode: "eur"
              countryCode: "de"
              locale: "de"
              metadata: "{{\"source\":\"storefront-graphql-cart\",\"step\":\"create\"}}"
            }}
          ) {{
            cart {{ id status currencyCode email }}
            context {{ locale currencyCode }}
          }}
        }}
        "#,
    )
}

fn storefront_cart_add_line_item_mutation(
    tenant_id: Uuid,
    cart_id: Uuid,
    variant_id: Uuid,
) -> String {
    format!(
        r#"
        mutation {{
          addStorefrontCartLineItem(
            tenantId: "{tenant_id}",
            cartId: "{cart_id}",
            input: {{
              variantId: "{variant_id}"
              quantity: 2
              metadata: "{{\"source\":\"storefront-graphql-cart\",\"step\":\"add\"}}"
            }}
          ) {{
            id
            status
            totalAmount
            lineItems {{ id title quantity totalPrice currencyCode }}
          }}
        }}
        "#
    )
}

fn storefront_cart_update_line_item_mutation(
    tenant_id: Uuid,
    cart_id: Uuid,
    line_id: Uuid,
) -> String {
    format!(
        r#"
        mutation {{
          updateStorefrontCartLineItem(
            tenantId: "{tenant_id}",
            cartId: "{cart_id}",
            lineId: "{line_id}",
            input: {{ quantity: 3 }}
          ) {{
            id
            totalAmount
            lineItems {{ id quantity totalPrice }}
          }}
        }}
        "#
    )
}

fn storefront_cart_remove_line_item_mutation(
    tenant_id: Uuid,
    cart_id: Uuid,
    line_id: Uuid,
) -> String {
    format!(
        r#"
        mutation {{
          removeStorefrontCartLineItem(
            tenantId: "{tenant_id}",
            cartId: "{cart_id}",
            lineId: "{line_id}"
          ) {{
            id
            totalAmount
            lineItems {{ id }}
          }}
        }}
        "#
    )
}

fn storefront_cart_query(tenant_id: Uuid, cart_id: Uuid) -> String {
    format!(
        r#"
        query {{
          storefrontCart(tenantId: "{tenant_id}", id: "{cart_id}") {{
            id
            email
            status
            currencyCode
            totalAmount
            lineItems {{ id title quantity totalPrice currencyCode }}
          }}
        }}
        "#
    )
}

fn storefront_cart_context_update_mutation(
    tenant_id: Uuid,
    cart_id: Uuid,
    region_id: Uuid,
    shipping_option_id: Uuid,
) -> String {
    format!(
        r#"
        mutation {{
          updateStorefrontCartContext(
            tenantId: "{tenant_id}",
            cartId: "{cart_id}",
            input: {{
              email: null
              regionId: "{region_id}"
              selectedShippingOptionId: "{shipping_option_id}"
            }}
          ) {{
            cart {{
              id
              email
              regionId
              countryCode
              localeCode
              selectedShippingOptionId
            }}
            context {{
              locale
              currencyCode
              region {{ id }}
            }}
          }}
        }}
        "#
    )
}

fn storefront_discovery_query(tenant_id: Uuid, cart_id: Uuid) -> String {
    format!(
        r#"
        query {{
          storefrontRegions(tenantId: "{tenant_id}") {{
            id
            name
            currencyCode
          }}
          storefrontShippingOptions(
            tenantId: "{tenant_id}",
            filter: {{
              cartId: "{cart_id}"
              currencyCode: "usd"
            }}
          ) {{
            id
            name
            currencyCode
            amount
          }}
        }}
        "#
    )
}

async fn seed_tenant_context(db: &DatabaseConnection, tenant_id: Uuid) {
    db.execute(Statement::from_sql_and_values(
        DatabaseBackend::Sqlite,
        "INSERT INTO tenants (id, name, slug, domain, settings, default_locale, is_active, created_at, updated_at)
         VALUES (?, ?, ?, ?, ?, ?, ?, CURRENT_TIMESTAMP, CURRENT_TIMESTAMP)",
        vec![
            tenant_id.into(),
            "Parity Tenant".into(),
            "parity-tenant".into(),
            sea_orm::Value::String(None),
            serde_json::json!({}).to_string().into(),
            "en".into(),
            true.into(),
        ],
    ))
    .await
    .unwrap();

    for (locale, name, native_name, is_default) in [
        ("en", "English", "English", true),
        ("de", "German", "Deutsch", false),
    ] {
        db.execute(Statement::from_sql_and_values(
            DatabaseBackend::Sqlite,
            "INSERT INTO tenant_locales (id, tenant_id, locale, name, native_name, is_default, is_enabled, fallback_locale, created_at)
             VALUES (?, ?, ?, ?, ?, ?, ?, ?, CURRENT_TIMESTAMP)",
            vec![
                Uuid::new_v4().into(),
                tenant_id.into(),
                locale.into(),
                name.into(),
                native_name.into(),
                is_default.into(),
                true.into(),
                sea_orm::Value::String(None),
            ],
        ))
        .await
        .unwrap();
    }

    db.execute(Statement::from_sql_and_values(
        DatabaseBackend::Sqlite,
        "INSERT INTO tenant_modules (id, tenant_id, module_slug, enabled, settings, created_at, updated_at)
         VALUES (?, ?, ?, ?, ?, CURRENT_TIMESTAMP, CURRENT_TIMESTAMP)",
        vec![
            Uuid::new_v4().into(),
            tenant_id.into(),
            "commerce".into(),
            true.into(),
            serde_json::json!({}).to_string().into(),
        ],
    ))
    .await
    .unwrap();
}

#[tokio::test]
async fn storefront_graphql_read_path_is_stable_after_cart_snapshot_creation() {
    let (db, catalog, cart_service) = setup().await;
    let tenant_id = Uuid::new_v4();
    let actor_id = Uuid::new_v4();
    seed_tenant_context(&db, tenant_id).await;

    let created = catalog
        .create_product(tenant_id, actor_id, create_product_input())
        .await
        .unwrap();
    let published = catalog
        .publish_product(tenant_id, actor_id, created.id)
        .await
        .unwrap();
    let handle = published
        .translations
        .iter()
        .find(|translation| translation.locale == "de")
        .map(|translation| translation.handle.clone())
        .expect("published product must keep de handle");

    let schema = build_schema(
        &db,
        tenant_context(tenant_id),
        request_context(tenant_id, "de"),
        None,
    );

    let before = schema
        .execute(Request::new(storefront_query(&handle)))
        .await;
    assert!(
        before.errors.is_empty(),
        "unexpected GraphQL errors before cart snapshot: {:?}",
        before.errors
    );

    let products_before = before
        .data
        .into_json()
        .expect("GraphQL response must serialize");

    cart_service
        .create_cart(
            tenant_id,
            CreateCartInput {
                customer_id: Some(Uuid::new_v4()),
                email: Some("buyer@example.com".to_string()),
                region_id: Some(Uuid::new_v4()),
                country_code: Some("de".to_string()),
                locale_code: Some("de".to_string()),
                selected_shipping_option_id: Some(Uuid::new_v4()),
                currency_code: "eur".to_string(),
                metadata: serde_json::json!({ "source": "graphql-parity-test" }),
            },
        )
        .await
        .unwrap();

    let after = schema
        .execute(Request::new(storefront_query(&handle)))
        .await;
    assert!(
        after.errors.is_empty(),
        "unexpected GraphQL errors after cart snapshot: {:?}",
        after.errors
    );

    let products_after = after
        .data
        .into_json()
        .expect("GraphQL response must serialize");

    assert_eq!(products_before, products_after);
    assert_eq!(
        products_after["storefrontProducts"]["total"],
        Value::from(1)
    );
    assert_eq!(
        products_after["storefrontProducts"]["items"][0]["title"],
        Value::from("Paritaet Produkt")
    );
}

#[tokio::test]
async fn admin_graphql_catalog_query_is_stable_after_cart_snapshot_creation() {
    let (db, catalog, cart_service) = setup().await;
    let tenant_id = Uuid::new_v4();
    let actor_id = Uuid::new_v4();
    seed_tenant_context(&db, tenant_id).await;

    let created = catalog
        .create_product(tenant_id, actor_id, create_product_input())
        .await
        .unwrap();

    let schema = build_schema(
        &db,
        tenant_context(tenant_id),
        request_context(tenant_id, "en"),
        Some(auth_context(tenant_id)),
    );
    let query = admin_query(tenant_id, created.id);

    let before = schema.execute(Request::new(query.clone())).await;
    assert!(
        before.errors.is_empty(),
        "unexpected admin GraphQL errors before cart snapshot: {:?}",
        before.errors
    );
    let before_json = before
        .data
        .into_json()
        .expect("GraphQL response must serialize");

    cart_service
        .create_cart(
            tenant_id,
            CreateCartInput {
                customer_id: Some(Uuid::new_v4()),
                email: Some("buyer@example.com".to_string()),
                region_id: Some(Uuid::new_v4()),
                country_code: Some("de".to_string()),
                locale_code: Some("de".to_string()),
                selected_shipping_option_id: Some(Uuid::new_v4()),
                currency_code: "eur".to_string(),
                metadata: serde_json::json!({ "source": "graphql-admin-parity-test" }),
            },
        )
        .await
        .unwrap();

    let after = schema.execute(Request::new(query)).await;
    assert!(
        after.errors.is_empty(),
        "unexpected admin GraphQL errors after cart snapshot: {:?}",
        after.errors
    );
    let after_json = after
        .data
        .into_json()
        .expect("GraphQL response must serialize");

    assert_eq!(before_json, after_json);
    assert_eq!(after_json["products"]["total"], Value::from(1));
    assert_eq!(
        after_json["product"]["translations"][0]["title"],
        Value::from("Parity Product")
    );
}

#[tokio::test]
async fn storefront_graphql_read_path_is_stable_after_complete_checkout() {
    let (db, catalog, cart_service, checkout, fulfillment) = setup_checkout().await;
    let tenant_id = Uuid::new_v4();
    let actor_id = Uuid::new_v4();
    seed_tenant_context(&db, tenant_id).await;

    let created = catalog
        .create_product(tenant_id, actor_id, create_product_input())
        .await
        .unwrap();
    let published = catalog
        .publish_product(tenant_id, actor_id, created.id)
        .await
        .unwrap();
    let published_variant = published
        .variants
        .first()
        .expect("published product must have variant");
    let handle = published
        .translations
        .iter()
        .find(|translation| translation.locale == "de")
        .map(|translation| translation.handle.clone())
        .expect("published product must keep de handle");

    let schema = build_schema(
        &db,
        tenant_context(tenant_id),
        request_context(tenant_id, "de"),
        None,
    );

    let before = schema
        .execute(Request::new(storefront_query(&handle)))
        .await;
    assert!(
        before.errors.is_empty(),
        "unexpected GraphQL errors before checkout: {:?}",
        before.errors
    );
    let before_json = before
        .data
        .into_json()
        .expect("GraphQL response must serialize");

    let region = RegionService::new(db.clone())
        .create_region(
            tenant_id,
            CreateRegionInput {
                name: "Europe".to_string(),
                currency_code: "eur".to_string(),
                tax_rate: Decimal::from_str("20.00").expect("valid decimal"),
                tax_included: true,
                countries: vec!["de".to_string()],
                metadata: serde_json::json!({ "source": "graphql-checkout-parity" }),
            },
        )
        .await
        .unwrap();
    let shipping_option = fulfillment
        .create_shipping_option(
            tenant_id,
            CreateShippingOptionInput {
                name: "Standard".to_string(),
                currency_code: "eur".to_string(),
                amount: Decimal::from_str("9.99").expect("valid decimal"),
                provider_id: None,
                metadata: serde_json::json!({ "source": "graphql-checkout-parity" }),
            },
        )
        .await
        .unwrap();
    let cart = cart_service
        .create_cart(
            tenant_id,
            CreateCartInput {
                customer_id: Some(Uuid::new_v4()),
                email: Some("buyer@example.com".to_string()),
                region_id: Some(region.id),
                country_code: Some("de".to_string()),
                locale_code: Some("de".to_string()),
                selected_shipping_option_id: Some(shipping_option.id),
                currency_code: "eur".to_string(),
                metadata: serde_json::json!({ "source": "graphql-checkout-parity" }),
            },
        )
        .await
        .unwrap();
    let cart = cart_service
        .add_line_item(
            tenant_id,
            cart.id,
            AddCartLineItemInput {
                product_id: Some(published.id),
                variant_id: Some(published_variant.id),
                sku: published_variant.sku.clone(),
                title: "Parity Product".to_string(),
                quantity: 1,
                unit_price: Decimal::from_str("19.99").expect("valid decimal"),
                metadata: serde_json::json!({ "source": "graphql-checkout-parity" }),
            },
        )
        .await
        .unwrap();

    let completed = checkout
        .complete_checkout(
            tenant_id,
            actor_id,
            CompleteCheckoutInput {
                cart_id: cart.id,
                shipping_option_id: None,
                region_id: None,
                country_code: None,
                locale: None,
                create_fulfillment: true,
                metadata: serde_json::json!({ "source": "graphql-checkout-parity" }),
            },
        )
        .await
        .unwrap();
    assert_eq!(completed.cart.status, "completed");
    assert_eq!(completed.order.status, "paid");

    let after = schema
        .execute(Request::new(storefront_query(&handle)))
        .await;
    assert!(
        after.errors.is_empty(),
        "unexpected GraphQL errors after checkout: {:?}",
        after.errors
    );
    let after_json = after
        .data
        .into_json()
        .expect("GraphQL response must serialize");

    assert_eq!(before_json, after_json);
    assert_eq!(after_json["storefrontProducts"]["total"], Value::from(1));
    assert_eq!(
        after_json["storefrontProducts"]["items"][0]["title"],
        Value::from("Paritaet Produkt")
    );
}

#[tokio::test]
async fn admin_graphql_catalog_query_is_stable_after_complete_checkout() {
    let (db, catalog, cart_service, checkout, fulfillment) = setup_checkout().await;
    let tenant_id = Uuid::new_v4();
    let actor_id = Uuid::new_v4();
    seed_tenant_context(&db, tenant_id).await;

    let created = catalog
        .create_product(tenant_id, actor_id, create_product_input())
        .await
        .unwrap();
    let published = catalog
        .publish_product(tenant_id, actor_id, created.id)
        .await
        .unwrap();
    let published_variant = published
        .variants
        .first()
        .expect("published product must have variant");

    let schema = build_schema(
        &db,
        tenant_context(tenant_id),
        request_context(tenant_id, "en"),
        Some(auth_context(tenant_id)),
    );
    let query = admin_query(tenant_id, created.id);

    let before = schema.execute(Request::new(query.clone())).await;
    assert!(
        before.errors.is_empty(),
        "unexpected admin GraphQL errors before checkout: {:?}",
        before.errors
    );
    let before_json = before
        .data
        .into_json()
        .expect("GraphQL response must serialize");

    let region = RegionService::new(db.clone())
        .create_region(
            tenant_id,
            CreateRegionInput {
                name: "Europe".to_string(),
                currency_code: "eur".to_string(),
                tax_rate: Decimal::from_str("20.00").expect("valid decimal"),
                tax_included: true,
                countries: vec!["de".to_string()],
                metadata: serde_json::json!({ "source": "admin-graphql-checkout-parity" }),
            },
        )
        .await
        .unwrap();
    let shipping_option = fulfillment
        .create_shipping_option(
            tenant_id,
            CreateShippingOptionInput {
                name: "Standard".to_string(),
                currency_code: "eur".to_string(),
                amount: Decimal::from_str("9.99").expect("valid decimal"),
                provider_id: None,
                metadata: serde_json::json!({ "source": "admin-graphql-checkout-parity" }),
            },
        )
        .await
        .unwrap();
    let cart = cart_service
        .create_cart(
            tenant_id,
            CreateCartInput {
                customer_id: Some(Uuid::new_v4()),
                email: Some("buyer@example.com".to_string()),
                region_id: Some(region.id),
                country_code: Some("de".to_string()),
                locale_code: Some("de".to_string()),
                selected_shipping_option_id: Some(shipping_option.id),
                currency_code: "eur".to_string(),
                metadata: serde_json::json!({ "source": "admin-graphql-checkout-parity" }),
            },
        )
        .await
        .unwrap();
    let cart = cart_service
        .add_line_item(
            tenant_id,
            cart.id,
            AddCartLineItemInput {
                product_id: Some(published.id),
                variant_id: Some(published_variant.id),
                sku: published_variant.sku.clone(),
                title: "Parity Product".to_string(),
                quantity: 1,
                unit_price: Decimal::from_str("19.99").expect("valid decimal"),
                metadata: serde_json::json!({ "source": "admin-graphql-checkout-parity" }),
            },
        )
        .await
        .unwrap();

    let completed = checkout
        .complete_checkout(
            tenant_id,
            actor_id,
            CompleteCheckoutInput {
                cart_id: cart.id,
                shipping_option_id: None,
                region_id: None,
                country_code: None,
                locale: None,
                create_fulfillment: true,
                metadata: serde_json::json!({ "source": "admin-graphql-checkout-parity" }),
            },
        )
        .await
        .unwrap();
    assert_eq!(completed.cart.status, "completed");
    assert_eq!(completed.order.status, "paid");

    let after = schema.execute(Request::new(query)).await;
    assert!(
        after.errors.is_empty(),
        "unexpected admin GraphQL errors after checkout: {:?}",
        after.errors
    );
    let after_json = after
        .data
        .into_json()
        .expect("GraphQL response must serialize");

    assert_eq!(before_json, after_json);
    assert_eq!(after_json["products"]["total"], Value::from(1));
    assert_eq!(
        after_json["product"]["translations"][0]["title"],
        Value::from("Parity Product")
    );
}

#[tokio::test]
async fn legacy_catalog_read_path_is_stable_after_complete_checkout() {
    let (db, catalog, cart_service, checkout, fulfillment) = setup_checkout().await;
    let tenant_id = Uuid::new_v4();
    let actor_id = Uuid::new_v4();
    seed_tenant_context(&db, tenant_id).await;

    let created = catalog
        .create_product(tenant_id, actor_id, create_product_input())
        .await
        .unwrap();
    let published = catalog
        .publish_product(tenant_id, actor_id, created.id)
        .await
        .unwrap();
    let published_variant = published
        .variants
        .first()
        .expect("published product must have variant");

    let before = serde_json::to_value(
        catalog
            .get_product(tenant_id, published.id)
            .await
            .expect("legacy catalog read path must resolve published product before checkout"),
    )
    .expect("product response must serialize");

    let region = RegionService::new(db.clone())
        .create_region(
            tenant_id,
            CreateRegionInput {
                name: "Europe".to_string(),
                currency_code: "eur".to_string(),
                tax_rate: Decimal::from_str("20.00").expect("valid decimal"),
                tax_included: true,
                countries: vec!["de".to_string()],
                metadata: serde_json::json!({ "source": "legacy-checkout-parity" }),
            },
        )
        .await
        .unwrap();
    let shipping_option = fulfillment
        .create_shipping_option(
            tenant_id,
            CreateShippingOptionInput {
                name: "Standard".to_string(),
                currency_code: "eur".to_string(),
                amount: Decimal::from_str("9.99").expect("valid decimal"),
                provider_id: None,
                metadata: serde_json::json!({ "source": "legacy-checkout-parity" }),
            },
        )
        .await
        .unwrap();
    let cart = cart_service
        .create_cart(
            tenant_id,
            CreateCartInput {
                customer_id: Some(Uuid::new_v4()),
                email: Some("buyer@example.com".to_string()),
                region_id: Some(region.id),
                country_code: Some("de".to_string()),
                locale_code: Some("de".to_string()),
                selected_shipping_option_id: Some(shipping_option.id),
                currency_code: "eur".to_string(),
                metadata: serde_json::json!({ "source": "legacy-checkout-parity" }),
            },
        )
        .await
        .unwrap();
    let cart = cart_service
        .add_line_item(
            tenant_id,
            cart.id,
            AddCartLineItemInput {
                product_id: Some(published.id),
                variant_id: Some(published_variant.id),
                sku: published_variant.sku.clone(),
                title: "Parity Product".to_string(),
                quantity: 1,
                unit_price: Decimal::from_str("19.99").expect("valid decimal"),
                metadata: serde_json::json!({ "source": "legacy-checkout-parity" }),
            },
        )
        .await
        .unwrap();

    let completed = checkout
        .complete_checkout(
            tenant_id,
            actor_id,
            CompleteCheckoutInput {
                cart_id: cart.id,
                shipping_option_id: None,
                region_id: None,
                country_code: None,
                locale: None,
                create_fulfillment: true,
                metadata: serde_json::json!({ "source": "legacy-checkout-parity" }),
            },
        )
        .await
        .unwrap();
    assert_eq!(completed.cart.status, "completed");
    assert_eq!(completed.order.status, "paid");

    let after = serde_json::to_value(
        catalog
            .get_product(tenant_id, published.id)
            .await
            .expect("legacy catalog read path must resolve published product after checkout"),
    )
    .expect("product response must serialize");

    assert_eq!(before, after);
    assert_eq!(
        after["translations"][0]["title"],
        Value::from("Parity Product")
    );
}

#[tokio::test]
async fn admin_graphql_order_payment_and_fulfillment_surface_matches_runtime_services() {
    let db = setup_test_db().await;
    support::ensure_commerce_schema(&db).await;
    let tenant_id = Uuid::new_v4();
    let actor_id = Uuid::new_v4();
    let customer_id = Uuid::new_v4();
    seed_tenant_context(&db, tenant_id).await;

    let order_service = OrderService::new(db.clone(), mock_transactional_event_bus());
    let payment_service = PaymentService::new(db.clone());
    let fulfillment_service = FulfillmentService::new(db.clone());

    let created_order = order_service
        .create_order(
            tenant_id,
            actor_id,
            CreateOrderInput {
                customer_id: Some(customer_id),
                currency_code: "eur".to_string(),
                line_items: vec![CreateOrderLineItemInput {
                    product_id: Some(Uuid::new_v4()),
                    variant_id: Some(Uuid::new_v4()),
                    sku: Some("GRAPHQL-ADMIN-ORDER-1".to_string()),
                    title: "GraphQL Admin Order".to_string(),
                    quantity: 1,
                    unit_price: Decimal::from_str("25.00").expect("valid decimal"),
                    metadata: serde_json::json!({ "source": "graphql-admin-order-parity" }),
                }],
                metadata: serde_json::json!({ "source": "graphql-admin-order-parity" }),
            },
        )
        .await
        .expect("order should be created");
    let confirmed_order = order_service
        .confirm_order(tenant_id, actor_id, created_order.id)
        .await
        .expect("order should be confirmed");
    let payment_collection = payment_service
        .create_collection(
            tenant_id,
            CreatePaymentCollectionInput {
                cart_id: None,
                order_id: Some(confirmed_order.id),
                customer_id: Some(customer_id),
                currency_code: "eur".to_string(),
                amount: Decimal::from_str("25.00").expect("valid decimal"),
                metadata: serde_json::json!({ "source": "graphql-admin-order-parity" }),
            },
        )
        .await
        .expect("payment collection should be created");
    let fulfillment = fulfillment_service
        .create_fulfillment(
            tenant_id,
            CreateFulfillmentInput {
                order_id: confirmed_order.id,
                shipping_option_id: None,
                customer_id: Some(customer_id),
                carrier: None,
                tracking_number: None,
                metadata: serde_json::json!({ "source": "graphql-admin-order-parity" }),
            },
        )
        .await
        .expect("fulfillment should be created");

    let schema = build_schema(
        &db,
        tenant_context(tenant_id),
        request_context(tenant_id, "en"),
        Some(admin_order_auth_context(tenant_id)),
    );

    let mutation = schema
        .execute(Request::new(admin_order_mutation(
            tenant_id,
            actor_id,
            confirmed_order.id,
            payment_collection.id,
            fulfillment.id,
        )))
        .await;
    assert!(
        mutation.errors.is_empty(),
        "unexpected admin GraphQL mutation errors: {:?}",
        mutation.errors
    );
    let mutation_json = mutation
        .data
        .into_json()
        .expect("GraphQL mutation response must serialize");
    assert_eq!(
        mutation_json["authorizePaymentCollection"]["status"],
        Value::from("authorized")
    );
    assert_eq!(
        mutation_json["capturePaymentCollection"]["status"],
        Value::from("captured")
    );
    assert_eq!(
        mutation_json["markOrderPaid"]["status"],
        Value::from("paid")
    );
    assert_eq!(mutation_json["shipOrder"]["status"], Value::from("shipped"));
    assert_eq!(
        mutation_json["deliverOrder"]["status"],
        Value::from("delivered")
    );
    assert_eq!(
        mutation_json["deliverFulfillment"]["status"],
        Value::from("delivered")
    );

    let query = schema
        .execute(Request::new(admin_order_parity_query(
            tenant_id,
            confirmed_order.id,
            payment_collection.id,
            fulfillment.id,
        )))
        .await;
    assert!(
        query.errors.is_empty(),
        "unexpected admin GraphQL query errors: {:?}",
        query.errors
    );
    let query_json = query
        .data
        .into_json()
        .expect("GraphQL query response must serialize");

    assert_eq!(
        query_json["order"]["order"]["status"],
        Value::from("delivered")
    );
    assert_eq!(
        query_json["order"]["order"]["paymentId"],
        Value::from("graphql-pay-1")
    );
    assert_eq!(
        query_json["order"]["order"]["trackingNumber"],
        Value::from("TRACK-789")
    );
    assert_eq!(
        query_json["order"]["paymentCollection"]["status"],
        Value::from("captured")
    );
    assert_eq!(
        query_json["order"]["fulfillment"]["status"],
        Value::from("delivered")
    );
    assert_eq!(query_json["orders"]["total"], Value::from(1));
    assert_eq!(
        query_json["orders"]["items"][0]["id"],
        Value::from(confirmed_order.id.to_string())
    );
    assert_eq!(
        query_json["paymentCollection"]["payments"][0]["status"],
        Value::from("captured")
    );
    assert_eq!(
        query_json["fulfillment"]["deliveredNote"],
        Value::from("Left at reception")
    );
    assert_eq!(query_json["paymentCollections"]["total"], Value::from(1));
    assert_eq!(
        query_json["paymentCollections"]["items"][0]["id"],
        Value::from(payment_collection.id.to_string())
    );
    assert_eq!(query_json["fulfillments"]["total"], Value::from(1));
    assert_eq!(
        query_json["fulfillments"]["items"][0]["id"],
        Value::from(fulfillment.id.to_string())
    );
}

#[tokio::test]
async fn storefront_graphql_customer_and_order_queries_match_customer_owned_read_path() {
    let db = setup_test_db().await;
    support::ensure_commerce_schema(&db).await;
    let tenant_id = Uuid::new_v4();
    let user_id = Uuid::new_v4();
    seed_tenant_context(&db, tenant_id).await;

    let customer = CustomerService::new(db.clone())
        .create_customer(
            tenant_id,
            CreateCustomerInput {
                user_id: Some(user_id),
                email: "buyer@example.com".to_string(),
                first_name: Some("GraphQL".to_string()),
                last_name: Some("Buyer".to_string()),
                phone: None,
                locale: Some("de".to_string()),
                metadata: serde_json::json!({ "source": "storefront-graphql-order-parity" }),
            },
        )
        .await
        .expect("customer should be created");

    let order = OrderService::new(db.clone(), mock_transactional_event_bus())
        .create_order(
            tenant_id,
            user_id,
            CreateOrderInput {
                customer_id: Some(customer.id),
                currency_code: "eur".to_string(),
                line_items: vec![CreateOrderLineItemInput {
                    product_id: Some(Uuid::new_v4()),
                    variant_id: Some(Uuid::new_v4()),
                    sku: Some("STOREFRONT-ORDER-1".to_string()),
                    title: "Storefront Order".to_string(),
                    quantity: 2,
                    unit_price: Decimal::from_str("15.00").expect("valid decimal"),
                    metadata: serde_json::json!({ "source": "storefront-graphql-order-parity" }),
                }],
                metadata: serde_json::json!({ "source": "storefront-graphql-order-parity" }),
            },
        )
        .await
        .expect("order should be created");

    let schema = build_schema(
        &db,
        tenant_context(tenant_id),
        request_context(tenant_id, "de"),
        Some(customer_auth_context(tenant_id, user_id)),
    );
    let response = schema
        .execute(Request::new(storefront_customer_order_query(
            tenant_id, order.id,
        )))
        .await;
    assert!(
        response.errors.is_empty(),
        "unexpected storefront GraphQL errors: {:?}",
        response.errors
    );
    let json = response
        .data
        .into_json()
        .expect("GraphQL response must serialize");

    assert_eq!(
        json["storefrontMe"]["email"],
        Value::from("buyer@example.com")
    );
    assert_eq!(json["storefrontMe"]["locale"], Value::from("de"));
    assert_eq!(
        json["storefrontOrder"]["id"],
        Value::from(order.id.to_string())
    );
    assert_eq!(
        json["storefrontOrder"]["customerId"],
        Value::from(customer.id.to_string())
    );
    assert_eq!(json["storefrontOrder"]["status"], Value::from("pending"));
    assert_eq!(json["storefrontOrder"]["currencyCode"], Value::from("EUR"));
    assert_eq!(json["storefrontOrder"]["totalAmount"], Value::from("30"));
    assert_eq!(
        json["storefrontOrder"]["lineItems"][0]["title"],
        Value::from("Storefront Order")
    );
    assert_eq!(
        json["storefrontOrder"]["lineItems"][0]["quantity"],
        Value::from(2)
    );
}

#[tokio::test]
async fn storefront_graphql_order_query_rejects_foreign_customer_access() {
    let db = setup_test_db().await;
    support::ensure_commerce_schema(&db).await;
    let tenant_id = Uuid::new_v4();
    let owner_user_id = Uuid::new_v4();
    let foreign_user_id = Uuid::new_v4();
    seed_tenant_context(&db, tenant_id).await;

    let owner_customer = CustomerService::new(db.clone())
        .create_customer(
            tenant_id,
            CreateCustomerInput {
                user_id: Some(owner_user_id),
                email: "owner@example.com".to_string(),
                first_name: Some("Owner".to_string()),
                last_name: None,
                phone: None,
                locale: Some("en".to_string()),
                metadata: serde_json::json!({ "source": "storefront-graphql-order-owner" }),
            },
        )
        .await
        .expect("owner customer should be created");
    CustomerService::new(db.clone())
        .create_customer(
            tenant_id,
            CreateCustomerInput {
                user_id: Some(foreign_user_id),
                email: "foreign@example.com".to_string(),
                first_name: Some("Foreign".to_string()),
                last_name: None,
                phone: None,
                locale: Some("en".to_string()),
                metadata: serde_json::json!({ "source": "storefront-graphql-order-foreign" }),
            },
        )
        .await
        .expect("foreign customer should be created");

    let order = OrderService::new(db.clone(), mock_transactional_event_bus())
        .create_order(
            tenant_id,
            owner_user_id,
            CreateOrderInput {
                customer_id: Some(owner_customer.id),
                currency_code: "eur".to_string(),
                line_items: vec![CreateOrderLineItemInput {
                    product_id: Some(Uuid::new_v4()),
                    variant_id: Some(Uuid::new_v4()),
                    sku: Some("FOREIGN-ORDER-1".to_string()),
                    title: "Foreign Guard".to_string(),
                    quantity: 1,
                    unit_price: Decimal::from_str("9.99").expect("valid decimal"),
                    metadata: serde_json::json!({ "source": "storefront-graphql-order-foreign" }),
                }],
                metadata: serde_json::json!({ "source": "storefront-graphql-order-foreign" }),
            },
        )
        .await
        .expect("order should be created");

    let schema = build_schema(
        &db,
        tenant_context(tenant_id),
        request_context(tenant_id, "en"),
        Some(customer_auth_context(tenant_id, foreign_user_id)),
    );
    let response = schema
        .execute(Request::new(format!(
            r#"
            query {{
              storefrontOrder(tenantId: "{tenant_id}", id: "{order_id}") {{
                id
              }}
            }}
            "#,
            order_id = order.id
        )))
        .await;

    assert_eq!(response.errors.len(), 1);
    assert_eq!(
        response.errors[0].message,
        "Order does not belong to the current customer"
    );
}

#[tokio::test]
async fn storefront_graphql_checkout_reuses_cart_payment_collection_for_guest_cart() {
    let (db, catalog, cart_service, _checkout, fulfillment) = setup_checkout().await;
    let tenant_id = Uuid::new_v4();
    let actor_id = Uuid::new_v4();
    seed_tenant_context(&db, tenant_id).await;

    let created = catalog
        .create_product(tenant_id, actor_id, create_product_input())
        .await
        .unwrap();
    let published = catalog
        .publish_product(tenant_id, actor_id, created.id)
        .await
        .unwrap();
    let published_variant = published
        .variants
        .first()
        .expect("published product must have variant");

    let region = RegionService::new(db.clone())
        .create_region(
            tenant_id,
            CreateRegionInput {
                name: "Europe".to_string(),
                currency_code: "eur".to_string(),
                tax_rate: Decimal::from_str("20.00").expect("valid decimal"),
                tax_included: true,
                countries: vec!["de".to_string()],
                metadata: serde_json::json!({ "source": "storefront-graphql-checkout" }),
            },
        )
        .await
        .unwrap();
    let shipping_option = fulfillment
        .create_shipping_option(
            tenant_id,
            CreateShippingOptionInput {
                name: "Standard".to_string(),
                currency_code: "eur".to_string(),
                amount: Decimal::from_str("9.99").expect("valid decimal"),
                provider_id: None,
                metadata: serde_json::json!({ "source": "storefront-graphql-checkout" }),
            },
        )
        .await
        .unwrap();
    let cart = cart_service
        .create_cart(
            tenant_id,
            CreateCartInput {
                customer_id: None,
                email: Some("guest@example.com".to_string()),
                region_id: Some(region.id),
                country_code: Some("de".to_string()),
                locale_code: Some("de".to_string()),
                selected_shipping_option_id: Some(shipping_option.id),
                currency_code: "eur".to_string(),
                metadata: serde_json::json!({ "source": "storefront-graphql-checkout" }),
            },
        )
        .await
        .unwrap();
    let cart = cart_service
        .add_line_item(
            tenant_id,
            cart.id,
            AddCartLineItemInput {
                product_id: Some(published.id),
                variant_id: Some(published_variant.id),
                sku: published_variant.sku.clone(),
                title: "Parity Product".to_string(),
                quantity: 1,
                unit_price: Decimal::from_str("19.99").expect("valid decimal"),
                metadata: serde_json::json!({ "source": "storefront-graphql-checkout" }),
            },
        )
        .await
        .unwrap();

    let schema = build_schema(
        &db,
        tenant_context(tenant_id),
        request_context(tenant_id, "de"),
        None,
    );
    let response = schema
        .execute(Request::new(storefront_checkout_mutation(
            tenant_id, cart.id,
        )))
        .await;
    assert!(
        response.errors.is_empty(),
        "unexpected storefront checkout GraphQL errors: {:?}",
        response.errors
    );
    let json = response
        .data
        .into_json()
        .expect("GraphQL response must serialize");

    assert_eq!(
        json["createStorefrontPaymentCollection"]["status"],
        Value::from("pending")
    );
    assert_eq!(
        json["completeStorefrontCheckout"]["cart"]["status"],
        Value::from("completed")
    );
    assert_eq!(
        json["completeStorefrontCheckout"]["order"]["status"],
        Value::from("paid")
    );
    assert_eq!(
        json["completeStorefrontCheckout"]["paymentCollection"]["status"],
        Value::from("captured")
    );
    assert_eq!(
        json["createStorefrontPaymentCollection"]["id"],
        json["completeStorefrontCheckout"]["paymentCollection"]["id"]
    );
    assert_eq!(
        json["completeStorefrontCheckout"]["fulfillment"]["status"],
        Value::from("pending")
    );
    assert_eq!(
        json["completeStorefrontCheckout"]["context"]["currencyCode"],
        Value::from("EUR")
    );
}

#[tokio::test]
async fn storefront_graphql_payment_collection_rejects_foreign_customer_cart_access() {
    let db = setup_test_db().await;
    support::ensure_commerce_schema(&db).await;
    let tenant_id = Uuid::new_v4();
    let owner_user_id = Uuid::new_v4();
    let foreign_user_id = Uuid::new_v4();
    seed_tenant_context(&db, tenant_id).await;

    let owner_customer = CustomerService::new(db.clone())
        .create_customer(
            tenant_id,
            CreateCustomerInput {
                user_id: Some(owner_user_id),
                email: "owner-cart@example.com".to_string(),
                first_name: Some("Owner".to_string()),
                last_name: None,
                phone: None,
                locale: Some("en".to_string()),
                metadata: serde_json::json!({ "source": "storefront-graphql-payment-owner" }),
            },
        )
        .await
        .expect("owner customer should be created");
    CustomerService::new(db.clone())
        .create_customer(
            tenant_id,
            CreateCustomerInput {
                user_id: Some(foreign_user_id),
                email: "foreign-cart@example.com".to_string(),
                first_name: Some("Foreign".to_string()),
                last_name: None,
                phone: None,
                locale: Some("en".to_string()),
                metadata: serde_json::json!({ "source": "storefront-graphql-payment-foreign" }),
            },
        )
        .await
        .expect("foreign customer should be created");

    let cart = CartService::new(db.clone())
        .create_cart(
            tenant_id,
            CreateCartInput {
                customer_id: Some(owner_customer.id),
                email: Some("owner-cart@example.com".to_string()),
                region_id: None,
                country_code: Some("de".to_string()),
                locale_code: Some("en".to_string()),
                selected_shipping_option_id: None,
                currency_code: "eur".to_string(),
                metadata: serde_json::json!({ "source": "storefront-graphql-payment-foreign" }),
            },
        )
        .await
        .expect("cart should be created");

    let schema = build_schema(
        &db,
        tenant_context(tenant_id),
        request_context(tenant_id, "en"),
        Some(customer_auth_context(tenant_id, foreign_user_id)),
    );
    let response = schema
        .execute(Request::new(format!(
            r#"
            mutation {{
              createStorefrontPaymentCollection(
                tenantId: "{tenant_id}",
                input: {{ cartId: "{cart_id}" }}
              ) {{
                id
              }}
            }}
            "#,
            cart_id = cart.id
        )))
        .await;

    assert_eq!(response.errors.len(), 1);
    assert_eq!(
        response.errors[0].message,
        "Cart belongs to another customer"
    );
}

#[tokio::test]
async fn storefront_graphql_cart_flow_creates_reads_updates_and_removes_line_items() {
    let (db, catalog, _cart_service) = setup().await;
    let tenant_id = Uuid::new_v4();
    let actor_id = Uuid::new_v4();
    seed_tenant_context(&db, tenant_id).await;

    let created = catalog
        .create_product(tenant_id, actor_id, create_product_input())
        .await
        .unwrap();
    let published = catalog
        .publish_product(tenant_id, actor_id, created.id)
        .await
        .unwrap();
    let published_variant = published
        .variants
        .first()
        .expect("published product must have variant");

    let schema = build_schema(
        &db,
        tenant_context(tenant_id),
        request_context(tenant_id, "de"),
        None,
    );

    let created_cart = schema
        .execute(Request::new(storefront_cart_flow_mutation(tenant_id)))
        .await;
    assert!(
        created_cart.errors.is_empty(),
        "unexpected create cart GraphQL errors: {:?}",
        created_cart.errors
    );
    let created_cart_json = created_cart
        .data
        .into_json()
        .expect("GraphQL response must serialize");
    let cart_id = Uuid::parse_str(
        created_cart_json["createStorefrontCart"]["cart"]["id"]
            .as_str()
            .expect("cart id must be a string"),
    )
    .expect("cart id must parse");
    assert_eq!(
        created_cart_json["createStorefrontCart"]["context"]["currencyCode"],
        Value::from("EUR")
    );

    let added = schema
        .execute(Request::new(storefront_cart_add_line_item_mutation(
            tenant_id,
            cart_id,
            published_variant.id,
        )))
        .await;
    assert!(
        added.errors.is_empty(),
        "unexpected add line item GraphQL errors: {:?}",
        added.errors
    );
    let added_json = added
        .data
        .into_json()
        .expect("GraphQL response must serialize");
    let line_id = Uuid::parse_str(
        added_json["addStorefrontCartLineItem"]["lineItems"][0]["id"]
            .as_str()
            .expect("line id must be a string"),
    )
    .expect("line id must parse");
    assert_eq!(
        added_json["addStorefrontCartLineItem"]["totalAmount"],
        Value::from("39.98")
    );

    let queried = schema
        .execute(Request::new(storefront_cart_query(tenant_id, cart_id)))
        .await;
    assert!(
        queried.errors.is_empty(),
        "unexpected cart query GraphQL errors: {:?}",
        queried.errors
    );
    let queried_json = queried
        .data
        .into_json()
        .expect("GraphQL response must serialize");
    assert_eq!(
        queried_json["storefrontCart"]["lineItems"][0]["title"],
        Value::from("Paritaet Produkt / Default")
    );
    assert_eq!(
        queried_json["storefrontCart"]["lineItems"][0]["quantity"],
        Value::from(2)
    );

    let updated = schema
        .execute(Request::new(storefront_cart_update_line_item_mutation(
            tenant_id, cart_id, line_id,
        )))
        .await;
    assert!(
        updated.errors.is_empty(),
        "unexpected update line item GraphQL errors: {:?}",
        updated.errors
    );
    let updated_json = updated
        .data
        .into_json()
        .expect("GraphQL response must serialize");
    assert_eq!(
        updated_json["updateStorefrontCartLineItem"]["totalAmount"],
        Value::from("59.97")
    );
    assert_eq!(
        updated_json["updateStorefrontCartLineItem"]["lineItems"][0]["quantity"],
        Value::from(3)
    );

    let removed = schema
        .execute(Request::new(storefront_cart_remove_line_item_mutation(
            tenant_id, cart_id, line_id,
        )))
        .await;
    assert!(
        removed.errors.is_empty(),
        "unexpected remove line item GraphQL errors: {:?}",
        removed.errors
    );
    let removed_json = removed
        .data
        .into_json()
        .expect("GraphQL response must serialize");
    assert_eq!(
        removed_json["removeStorefrontCartLineItem"]["totalAmount"],
        Value::from("0")
    );
    assert_eq!(
        removed_json["removeStorefrontCartLineItem"]["lineItems"],
        serde_json::json!([])
    );
}

#[tokio::test]
async fn storefront_graphql_cart_query_rejects_foreign_customer_access() {
    let db = setup_test_db().await;
    support::ensure_commerce_schema(&db).await;
    let tenant_id = Uuid::new_v4();
    let owner_user_id = Uuid::new_v4();
    let foreign_user_id = Uuid::new_v4();
    seed_tenant_context(&db, tenant_id).await;

    let owner_customer = CustomerService::new(db.clone())
        .create_customer(
            tenant_id,
            CreateCustomerInput {
                user_id: Some(owner_user_id),
                email: "owner-query@example.com".to_string(),
                first_name: Some("Owner".to_string()),
                last_name: None,
                phone: None,
                locale: Some("en".to_string()),
                metadata: serde_json::json!({ "source": "storefront-graphql-cart-owner" }),
            },
        )
        .await
        .expect("owner customer should be created");
    CustomerService::new(db.clone())
        .create_customer(
            tenant_id,
            CreateCustomerInput {
                user_id: Some(foreign_user_id),
                email: "foreign-query@example.com".to_string(),
                first_name: Some("Foreign".to_string()),
                last_name: None,
                phone: None,
                locale: Some("en".to_string()),
                metadata: serde_json::json!({ "source": "storefront-graphql-cart-foreign" }),
            },
        )
        .await
        .expect("foreign customer should be created");
    let cart = CartService::new(db.clone())
        .create_cart(
            tenant_id,
            CreateCartInput {
                customer_id: Some(owner_customer.id),
                email: Some("owner-query@example.com".to_string()),
                region_id: None,
                country_code: Some("de".to_string()),
                locale_code: Some("en".to_string()),
                selected_shipping_option_id: None,
                currency_code: "eur".to_string(),
                metadata: serde_json::json!({ "source": "storefront-graphql-cart-foreign" }),
            },
        )
        .await
        .expect("cart should be created");

    let schema = build_schema(
        &db,
        tenant_context(tenant_id),
        request_context(tenant_id, "en"),
        Some(customer_auth_context(tenant_id, foreign_user_id)),
    );
    let response = schema
        .execute(Request::new(storefront_cart_query(tenant_id, cart.id)))
        .await;

    assert_eq!(response.errors.len(), 1);
    assert_eq!(
        response.errors[0].message,
        "Cart belongs to another customer"
    );
}

#[tokio::test]
async fn storefront_graphql_cart_context_patch_keeps_tristate_semantics() {
    let db = setup_test_db().await;
    support::ensure_commerce_schema(&db).await;
    let tenant_id = Uuid::new_v4();
    seed_tenant_context(&db, tenant_id).await;

    let region = RegionService::new(db.clone())
        .create_region(
            tenant_id,
            CreateRegionInput {
                name: "Europe".to_string(),
                currency_code: "eur".to_string(),
                tax_rate: Decimal::from_str("20.00").expect("valid decimal"),
                tax_included: true,
                countries: vec!["de".to_string()],
                metadata: serde_json::json!({ "source": "storefront-graphql-cart-context" }),
            },
        )
        .await
        .expect("region should be created");
    let shipping_option = FulfillmentService::new(db.clone())
        .create_shipping_option(
            tenant_id,
            CreateShippingOptionInput {
                name: "Standard".to_string(),
                currency_code: "eur".to_string(),
                amount: Decimal::from_str("9.99").expect("valid decimal"),
                provider_id: None,
                metadata: serde_json::json!({ "source": "storefront-graphql-cart-context" }),
            },
        )
        .await
        .expect("shipping option should be created");
    let cart = CartService::new(db.clone())
        .create_cart(
            tenant_id,
            CreateCartInput {
                customer_id: None,
                email: Some("context@example.com".to_string()),
                region_id: None,
                country_code: Some("de".to_string()),
                locale_code: Some("de".to_string()),
                selected_shipping_option_id: None,
                currency_code: "eur".to_string(),
                metadata: serde_json::json!({ "source": "storefront-graphql-cart-context" }),
            },
        )
        .await
        .expect("cart should be created");

    let schema = build_schema(
        &db,
        tenant_context(tenant_id),
        request_context(tenant_id, "de"),
        None,
    );
    let response = schema
        .execute(Request::new(storefront_cart_context_update_mutation(
            tenant_id,
            cart.id,
            region.id,
            shipping_option.id,
        )))
        .await;
    assert!(
        response.errors.is_empty(),
        "unexpected cart context patch GraphQL errors: {:?}",
        response.errors
    );
    let json = response
        .data
        .into_json()
        .expect("GraphQL response must serialize");

    assert_eq!(
        json["updateStorefrontCartContext"]["cart"]["email"],
        Value::Null
    );
    assert_eq!(
        json["updateStorefrontCartContext"]["cart"]["regionId"],
        Value::from(region.id.to_string())
    );
    assert_eq!(
        json["updateStorefrontCartContext"]["cart"]["countryCode"],
        Value::Null
    );
    assert_eq!(
        json["updateStorefrontCartContext"]["cart"]["selectedShippingOptionId"],
        Value::from(shipping_option.id.to_string())
    );
    assert_eq!(
        json["updateStorefrontCartContext"]["context"]["region"]["id"],
        Value::from(region.id.to_string())
    );
    assert_eq!(
        json["updateStorefrontCartContext"]["context"]["currencyCode"],
        Value::from("EUR")
    );
}

#[tokio::test]
async fn storefront_graphql_discovery_queries_follow_live_region_and_shipping_context_contract() {
    let db = setup_test_db().await;
    support::ensure_commerce_schema(&db).await;
    let tenant_id = Uuid::new_v4();
    seed_tenant_context(&db, tenant_id).await;

    let region = RegionService::new(db.clone())
        .create_region(
            tenant_id,
            CreateRegionInput {
                name: "Europe".to_string(),
                currency_code: "eur".to_string(),
                tax_rate: Decimal::from_str("20.00").expect("valid decimal"),
                tax_included: true,
                countries: vec!["de".to_string()],
                metadata: serde_json::json!({ "source": "storefront-graphql-discovery" }),
            },
        )
        .await
        .expect("region should be created");
    FulfillmentService::new(db.clone())
        .create_shipping_option(
            tenant_id,
            CreateShippingOptionInput {
                name: "EUR Standard".to_string(),
                currency_code: "eur".to_string(),
                amount: Decimal::from_str("9.99").expect("valid decimal"),
                provider_id: None,
                metadata: serde_json::json!({ "source": "storefront-graphql-discovery" }),
            },
        )
        .await
        .expect("eur option should be created");
    FulfillmentService::new(db.clone())
        .create_shipping_option(
            tenant_id,
            CreateShippingOptionInput {
                name: "USD Express".to_string(),
                currency_code: "usd".to_string(),
                amount: Decimal::from_str("14.99").expect("valid decimal"),
                provider_id: None,
                metadata: serde_json::json!({ "source": "storefront-graphql-discovery" }),
            },
        )
        .await
        .expect("usd option should be created");
    let cart = CartService::new(db.clone())
        .create_cart(
            tenant_id,
            CreateCartInput {
                customer_id: None,
                email: Some("discovery@example.com".to_string()),
                region_id: Some(region.id),
                country_code: Some("de".to_string()),
                locale_code: Some("de".to_string()),
                selected_shipping_option_id: None,
                currency_code: "eur".to_string(),
                metadata: serde_json::json!({ "source": "storefront-graphql-discovery" }),
            },
        )
        .await
        .expect("cart should be created");

    let schema = build_schema(
        &db,
        tenant_context(tenant_id),
        request_context(tenant_id, "de"),
        None,
    );
    let response = schema
        .execute(Request::new(storefront_discovery_query(tenant_id, cart.id)))
        .await;
    assert!(
        response.errors.is_empty(),
        "unexpected storefront discovery GraphQL errors: {:?}",
        response.errors
    );
    let json = response
        .data
        .into_json()
        .expect("GraphQL response must serialize");

    assert_eq!(
        json["storefrontRegions"][0]["id"],
        Value::from(region.id.to_string())
    );
    assert_eq!(
        json["storefrontRegions"][0]["currencyCode"],
        Value::from("EUR")
    );
    assert_eq!(
        json["storefrontShippingOptions"],
        serde_json::json!([{
            "id": json["storefrontShippingOptions"][0]["id"].clone(),
            "name": "EUR Standard",
            "currencyCode": "EUR",
            "amount": "9.99"
        }])
    );
}

#[tokio::test]
async fn storefront_graphql_shipping_options_reject_foreign_customer_cart_access() {
    let db = setup_test_db().await;
    support::ensure_commerce_schema(&db).await;
    let tenant_id = Uuid::new_v4();
    let owner_user_id = Uuid::new_v4();
    let foreign_user_id = Uuid::new_v4();
    seed_tenant_context(&db, tenant_id).await;

    let owner_customer = CustomerService::new(db.clone())
        .create_customer(
            tenant_id,
            CreateCustomerInput {
                user_id: Some(owner_user_id),
                email: "shipping-owner@example.com".to_string(),
                first_name: Some("Owner".to_string()),
                last_name: None,
                phone: None,
                locale: Some("en".to_string()),
                metadata: serde_json::json!({ "source": "storefront-graphql-shipping-owner" }),
            },
        )
        .await
        .expect("owner customer should be created");
    CustomerService::new(db.clone())
        .create_customer(
            tenant_id,
            CreateCustomerInput {
                user_id: Some(foreign_user_id),
                email: "shipping-foreign@example.com".to_string(),
                first_name: Some("Foreign".to_string()),
                last_name: None,
                phone: None,
                locale: Some("en".to_string()),
                metadata: serde_json::json!({ "source": "storefront-graphql-shipping-foreign" }),
            },
        )
        .await
        .expect("foreign customer should be created");
    let cart = CartService::new(db.clone())
        .create_cart(
            tenant_id,
            CreateCartInput {
                customer_id: Some(owner_customer.id),
                email: Some("shipping-owner@example.com".to_string()),
                region_id: None,
                country_code: Some("de".to_string()),
                locale_code: Some("en".to_string()),
                selected_shipping_option_id: None,
                currency_code: "eur".to_string(),
                metadata: serde_json::json!({ "source": "storefront-graphql-shipping-foreign" }),
            },
        )
        .await
        .expect("cart should be created");

    let schema = build_schema(
        &db,
        tenant_context(tenant_id),
        request_context(tenant_id, "en"),
        Some(customer_auth_context(tenant_id, foreign_user_id)),
    );
    let response = schema
        .execute(Request::new(format!(
            r#"
            query {{
              storefrontShippingOptions(
                tenantId: "{tenant_id}",
                filter: {{ cartId: "{cart_id}" }}
              ) {{
                id
              }}
            }}
            "#,
            cart_id = cart.id
        )))
        .await;

    assert_eq!(response.errors.len(), 1);
    assert_eq!(
        response.errors[0].message,
        "Cart belongs to another customer"
    );
}
