use async_graphql::{EmptyMutation, EmptySubscription, Request, Schema};
use rust_decimal::Decimal;
use rustok_api::{AuthContext, RequestContext, TenantContext};
use rustok_commerce::dto::{
    AddCartLineItemInput, CompleteCheckoutInput, CreateCartInput, CreateProductInput,
    CreateShippingOptionInput, CreateVariantInput, PriceInput, ProductTranslationInput,
};
use rustok_commerce::graphql::CommerceQuery;
use rustok_commerce::{CartService, CatalogService, CheckoutService, FulfillmentService};
use rustok_core::Permission;
use rustok_region::dto::CreateRegionInput;
use rustok_region::services::RegionService;
use rustok_test_utils::{db::setup_test_db, helpers::unique_slug, mock_transactional_event_bus};
use sea_orm::{ConnectionTrait, DatabaseBackend, DatabaseConnection, Statement};
use serde_json::Value;
use std::str::FromStr;
use uuid::Uuid;

mod support;

type CommerceSchema = Schema<CommerceQuery, EmptyMutation, EmptySubscription>;

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

fn build_schema(
    db: &DatabaseConnection,
    tenant: TenantContext,
    request_context: RequestContext,
    auth: Option<AuthContext>,
) -> CommerceSchema {
    let event_bus = mock_transactional_event_bus();
    let mut builder = Schema::build(CommerceQuery::default(), EmptyMutation, EmptySubscription)
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
