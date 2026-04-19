use migration::Migrator;
use rust_decimal::Decimal;
use rustok_cart::dto::{AddCartLineItemInput, CreateCartInput};
use rustok_cart::services::CartService;
use rustok_commerce::dto::{
    CreateProductInput, CreateVariantInput, PriceInput, ProductOptionInput,
    ProductOptionTranslationInput, ProductTranslationInput, ResolveStoreContextInput,
};
use rustok_commerce::entities;
use rustok_commerce::services::{
    CatalogService, InventoryService, PricingService, StoreContextService,
};
use rustok_customer::dto::{CreateCustomerInput, UpdateCustomerInput};
use rustok_customer::services::CustomerService;
use rustok_fulfillment::dto::{
    CreateFulfillmentInput, CreateShippingOptionInput, DeliverFulfillmentInput,
    ShipFulfillmentInput, ShippingOptionTranslationInput,
};
use rustok_fulfillment::services::FulfillmentService;
use rustok_order::dto::{CreateOrderInput, CreateOrderLineItemInput};
use rustok_order::services::OrderService;
use rustok_payment::dto::{
    AuthorizePaymentInput, CapturePaymentInput, CompleteRefundInput, CreatePaymentCollectionInput,
    CreateRefundInput,
};
use rustok_payment::services::PaymentService;
use rustok_region::dto::{CreateRegionInput, RegionTranslationInput};
use rustok_region::services::RegionService;
use rustok_test_utils::{db::setup_test_db_with_migrations, mock_transactional_event_bus};
use sea_orm_migration::sea_orm::{
    ColumnTrait, ConnectionTrait, DatabaseBackend, DatabaseConnection, EntityTrait, PaginatorTrait,
    QueryFilter, Statement,
};
use std::collections::BTreeSet;
use std::str::FromStr;
use uuid::Uuid;

async fn load_sqlite_tables(db: &DatabaseConnection) -> BTreeSet<String> {
    let rows = db
        .query_all(Statement::from_string(
            DatabaseBackend::Sqlite,
            "SELECT name FROM sqlite_master WHERE type = 'table'".to_string(),
        ))
        .await
        .expect("failed to query sqlite_master");

    rows.into_iter()
        .map(|row| {
            row.try_get::<String>("", "name")
                .expect("sqlite_master row must expose table name")
        })
        .collect()
}

#[tokio::test]
async fn pricing_service_supports_decimal_prices_on_migrated_schema() {
    let db = setup_test_db_with_migrations::<Migrator>().await;
    let event_bus = mock_transactional_event_bus();
    let catalog = CatalogService::new(db.clone(), event_bus.clone());
    let pricing = PricingService::new(db.clone(), event_bus);
    let tenant_id = Uuid::new_v4();
    let actor_id = Uuid::new_v4();
    seed_tenant(&db, tenant_id).await;

    let created = catalog
        .create_product(tenant_id, actor_id, create_product_input())
        .await
        .expect("catalog create_product should work before pricing smoke");

    let variant_id = created.variants[0].id;
    pricing
        .set_price(
            tenant_id,
            actor_id,
            variant_id,
            "EUR",
            Decimal::from_str("89.99").expect("valid decimal"),
            Some(Decimal::from_str("109.99").expect("valid decimal")),
        )
        .await
        .expect("pricing service should write decimal price on migrated schema");

    let fetched = pricing
        .get_price(variant_id, "EUR")
        .await
        .expect("pricing service should read decimal price on migrated schema");

    assert_eq!(
        fetched,
        Some(Decimal::from_str("89.99").expect("valid decimal"))
    );
}

#[tokio::test]
async fn region_and_store_context_services_resolve_currency_and_locales_on_migrated_schema() {
    let db = setup_test_db_with_migrations::<Migrator>().await;
    let region_service = RegionService::new(db.clone());
    let context_service = StoreContextService::new(db.clone());
    let tenant_id = Uuid::new_v4();
    seed_tenant(&db, tenant_id).await;
    seed_tenant_locale(&db, tenant_id, "de", false).await;

    let region = region_service
        .create_region(
            tenant_id,
            CreateRegionInput {
                translations: vec![RegionTranslationInput {
                    locale: "en".to_string(),
                    name: "Europe".to_string(),
                }],
                currency_code: "eur".to_string(),
                tax_provider_id: None,
                tax_rate: Decimal::from_str("20.00").expect("valid decimal"),
                tax_included: true,
                country_tax_policies: None,
                countries: vec!["de".to_string(), "fr".to_string()],
                metadata: serde_json::json!({ "source": "migration-smoke" }),
            },
        )
        .await
        .expect("region service should create region on migrated schema");

    let context = context_service
        .resolve_context(
            tenant_id,
            ResolveStoreContextInput {
                region_id: Some(region.id),
                country_code: None,
                locale: Some("de".to_string()),
                currency_code: None,
            },
        )
        .await
        .expect("store context should resolve on migrated schema");

    assert_eq!(context.locale, "de");
    assert_eq!(context.currency_code.as_deref(), Some("EUR"));
    assert_eq!(
        context.region.as_ref().map(|value| value.id),
        Some(region.id)
    );
}

#[tokio::test]
async fn cart_service_supports_cart_lifecycle_on_migrated_schema() {
    let db = setup_test_db_with_migrations::<Migrator>().await;
    let service = CartService::new(db);
    let tenant_id = Uuid::new_v4();

    let created = service
        .create_cart(tenant_id, create_cart_input())
        .await
        .expect("cart service should create cart on migrated schema");
    let with_item = service
        .add_line_item(tenant_id, created.id, create_cart_line_item_input())
        .await
        .expect("cart service should add line item on migrated schema");
    assert_eq!(with_item.status, "active");
    assert_eq!(with_item.line_items.len(), 1);
    assert_eq!(
        with_item.total_amount,
        Decimal::from_str("31.00").expect("valid decimal")
    );

    let completed = service
        .complete_cart(tenant_id, created.id)
        .await
        .expect("cart service should complete cart on migrated schema");
    assert_eq!(completed.status, "completed");
}

#[tokio::test]
async fn customer_service_supports_storefront_customer_boundary_on_migrated_schema() {
    let db = setup_test_db_with_migrations::<Migrator>().await;
    let service = CustomerService::new(db);
    let tenant_id = Uuid::new_v4();

    let created = service
        .create_customer(tenant_id, create_customer_input())
        .await
        .expect("customer service should create customer on migrated schema");
    assert_eq!(created.email, "migration-customer@example.com");

    let fetched = service
        .get_customer_by_user(tenant_id, created.user_id.expect("user id"))
        .await
        .expect("customer service should resolve customer by user");
    assert_eq!(fetched.id, created.id);

    let updated = service
        .update_customer(
            tenant_id,
            created.id,
            UpdateCustomerInput {
                email: Some("migration-updated@example.com".to_string()),
                first_name: Some("Updated".to_string()),
                last_name: Some("Customer".to_string()),
                phone: Some("+9988776655".to_string()),
                locale: Some("ru".to_string()),
                metadata: Some(serde_json::json!({ "source": "migration-smoke-updated" })),
            },
        )
        .await
        .expect("customer service should update customer on migrated schema");
    assert_eq!(updated.email, "migration-updated@example.com");
    assert_eq!(updated.locale.as_deref(), Some("ru"));
}

#[tokio::test]
async fn payment_service_supports_payment_collection_lifecycle_on_migrated_schema() {
    let db = setup_test_db_with_migrations::<Migrator>().await;
    let service = PaymentService::new(db);
    let tenant_id = Uuid::new_v4();

    let created = service
        .create_collection(tenant_id, create_payment_collection_input())
        .await
        .expect("payment service should create collection on migrated schema");
    assert_eq!(created.status, "pending");

    let authorized = service
        .authorize_collection(
            tenant_id,
            created.id,
            AuthorizePaymentInput {
                provider_id: None,
                provider_payment_id: None,
                amount: None,
                metadata: serde_json::json!({ "step": "authorized" }),
            },
        )
        .await
        .expect("payment service should authorize collection on migrated schema");
    assert_eq!(authorized.status, "authorized");

    let captured = service
        .capture_collection(
            tenant_id,
            created.id,
            CapturePaymentInput {
                amount: Some(Decimal::from_str("59.99").expect("valid decimal")),
                metadata: serde_json::json!({ "step": "captured" }),
            },
        )
        .await
        .expect("payment service should capture collection on migrated schema");
    assert_eq!(captured.status, "captured");

    let refund = service
        .create_refund(
            tenant_id,
            created.id,
            CreateRefundInput {
                amount: Decimal::from_str("10.00").expect("valid decimal"),
                reason: Some("migration-smoke".to_string()),
                metadata: serde_json::json!({ "step": "refund-created" }),
            },
        )
        .await
        .expect("payment service should create refund on migrated schema");
    assert_eq!(refund.status, "pending");

    let refunded = service
        .complete_refund(
            tenant_id,
            refund.id,
            CompleteRefundInput {
                metadata: serde_json::json!({ "step": "refund-completed" }),
            },
        )
        .await
        .expect("payment service should complete refund on migrated schema");
    assert_eq!(refunded.status, "refunded");
}

#[tokio::test]
async fn fulfillment_service_supports_shipping_and_delivery_on_migrated_schema() {
    let db = setup_test_db_with_migrations::<Migrator>().await;
    let service = FulfillmentService::new(db);
    let tenant_id = Uuid::new_v4();

    let shipping_option = service
        .create_shipping_option(tenant_id, create_shipping_option_input())
        .await
        .expect("fulfillment service should create shipping option on migrated schema");

    let created = service
        .create_fulfillment(
            tenant_id,
            CreateFulfillmentInput {
                order_id: Uuid::new_v4(),
                shipping_option_id: Some(shipping_option.id),
                customer_id: Some(Uuid::new_v4()),
                carrier: None,
                tracking_number: None,
                items: None,
                metadata: serde_json::json!({ "source": "migration-smoke" }),
            },
        )
        .await
        .expect("fulfillment service should create fulfillment on migrated schema");
    assert_eq!(created.status, "pending");

    let shipped = service
        .ship_fulfillment(
            tenant_id,
            created.id,
            ShipFulfillmentInput {
                carrier: "dhl".to_string(),
                tracking_number: "trk_987".to_string(),
                items: None,
                metadata: serde_json::json!({ "step": "shipped" }),
            },
        )
        .await
        .expect("fulfillment service should ship on migrated schema");
    assert_eq!(shipped.status, "shipped");

    let delivered = service
        .deliver_fulfillment(
            tenant_id,
            created.id,
            DeliverFulfillmentInput {
                delivered_note: Some("front-desk".to_string()),
                items: None,
                metadata: serde_json::json!({ "step": "delivered" }),
            },
        )
        .await
        .expect("fulfillment service should deliver on migrated schema");
    assert_eq!(delivered.status, "delivered");
}

#[tokio::test]
async fn inventory_service_supports_normalized_inventory_on_migrated_schema() {
    let db = setup_test_db_with_migrations::<Migrator>().await;
    let event_bus = mock_transactional_event_bus();
    let catalog = CatalogService::new(db.clone(), event_bus.clone());
    let inventory = InventoryService::new(db.clone(), event_bus);
    let tenant_id = Uuid::new_v4();
    let actor_id = Uuid::new_v4();
    seed_tenant(&db, tenant_id).await;

    let created = catalog
        .create_product(tenant_id, actor_id, create_product_input())
        .await
        .expect("catalog create_product should work before inventory smoke");

    let variant_id = created.variants[0].id;
    inventory
        .set_inventory(tenant_id, actor_id, variant_id, 20)
        .await
        .expect("inventory service should initialize normalized inventory rows");
    inventory
        .reserve(tenant_id, variant_id, 6)
        .await
        .expect("inventory service should write reservation rows");

    let inventory_item = entities::inventory_item::Entity::find()
        .filter(entities::inventory_item::Column::VariantId.eq(variant_id))
        .one(&db)
        .await
        .expect("inventory item query should succeed")
        .expect("inventory item should exist");
    let level = entities::inventory_level::Entity::find()
        .filter(entities::inventory_level::Column::InventoryItemId.eq(inventory_item.id))
        .one(&db)
        .await
        .expect("inventory level query should succeed")
        .expect("inventory level should exist");
    let reservation_count = entities::reservation_item::Entity::find()
        .filter(entities::reservation_item::Column::InventoryItemId.eq(inventory_item.id))
        .count(&db)
        .await
        .expect("reservation query should succeed");
    assert_eq!(level.stocked_quantity, 20);
    assert_eq!(level.reserved_quantity, 6);
    assert_eq!(reservation_count, 1);
    assert!(inventory
        .check_availability(tenant_id, variant_id, 14)
        .await
        .expect("availability check should succeed"));
    assert!(!inventory
        .check_availability(tenant_id, variant_id, 15)
        .await
        .expect("availability check should succeed"));
}

#[tokio::test]
async fn order_service_supports_order_lifecycle_on_migrated_schema() {
    let db = setup_test_db_with_migrations::<Migrator>().await;
    let service = OrderService::new(db.clone(), mock_transactional_event_bus());
    let tenant_id = Uuid::new_v4();
    let actor_id = Uuid::new_v4();
    seed_tenant(&db, tenant_id).await;

    let created = service
        .create_order(tenant_id, actor_id, create_order_input())
        .await
        .expect("order service should create order on migrated schema");
    assert_eq!(created.status, "pending");
    assert_eq!(created.line_items.len(), 2);

    let confirmed = service
        .confirm_order(tenant_id, actor_id, created.id)
        .await
        .expect("order should confirm on migrated schema");
    assert_eq!(confirmed.status, "confirmed");

    let paid = service
        .mark_paid(
            tenant_id,
            actor_id,
            created.id,
            "pay_123".to_string(),
            "manual".to_string(),
        )
        .await
        .expect("order should be payable on migrated schema");
    assert_eq!(paid.status, "paid");

    let delivered = service
        .ship_order(
            tenant_id,
            actor_id,
            created.id,
            "trk_123".to_string(),
            "dhl".to_string(),
        )
        .await
        .expect("order should be shippable on migrated schema");
    let delivered = service
        .deliver_order(
            tenant_id,
            actor_id,
            delivered.id,
            Some("front-desk".to_string()),
        )
        .await
        .expect("order should be deliverable on migrated schema");
    assert_eq!(delivered.status, "delivered");
    assert_eq!(delivered.delivered_signature.as_deref(), Some("front-desk"));
}

async fn seed_tenant(db: &DatabaseConnection, tenant_id: Uuid) {
    db.execute(Statement::from_sql_and_values(
        DatabaseBackend::Sqlite,
        "INSERT INTO tenants (id, name, slug, domain, settings, default_locale, is_active, created_at, updated_at)
         VALUES (?, ?, ?, ?, ?, ?, ?, CURRENT_TIMESTAMP, CURRENT_TIMESTAMP)",
        vec![
            tenant_id.into(),
            "Migration Test Tenant".into(),
            format!("migration-test-tenant-{tenant_id}").into(),
            sea_orm_migration::sea_orm::Value::String(None),
            serde_json::json!({}).to_string().into(),
            "en".into(),
            true.into(),
        ],
    ))
    .await
    .expect("failed to seed tenant");
}

async fn seed_tenant_locale(
    db: &DatabaseConnection,
    tenant_id: Uuid,
    locale: &str,
    is_default: bool,
) {
    db.execute(Statement::from_sql_and_values(
        DatabaseBackend::Sqlite,
        "INSERT INTO tenant_locales (id, tenant_id, locale, name, native_name, is_default, is_enabled, fallback_locale, created_at)
         VALUES (?, ?, ?, ?, ?, ?, ?, ?, CURRENT_TIMESTAMP)",
        vec![
            Uuid::new_v4().into(),
            tenant_id.into(),
            locale.into(),
            locale.into(),
            locale.into(),
            is_default.into(),
            true.into(),
            sea_orm_migration::sea_orm::Value::String(None),
        ],
    ))
    .await
    .expect("failed to seed tenant locale");
}

fn create_product_input() -> CreateProductInput {
    CreateProductInput {
        translations: vec![
            ProductTranslationInput {
                locale: "en".to_string(),
                title: "Migration-backed Product".to_string(),
                description: Some("English translation".to_string()),
                handle: Some(format!("migration-backed-{}", Uuid::new_v4())),
                meta_title: Some("EN meta".to_string()),
                meta_description: Some("EN description".to_string()),
            },
            ProductTranslationInput {
                locale: "ru".to_string(),
                title: "Migration-backed RU product".to_string(),
                description: Some("Russian localization".to_string()),
                handle: Some(format!("migration-backed-ru-{}", Uuid::new_v4())),
                meta_title: Some("RU meta".to_string()),
                meta_description: Some("RU description".to_string()),
            },
        ],
        options: vec![ProductOptionInput {
            translations: vec![ProductOptionTranslationInput {
                locale: "en".to_string(),
                name: "Size".to_string(),
                values: vec!["S".to_string(), "M".to_string()],
            }],
        }],
        variants: vec![CreateVariantInput {
            sku: Some(format!("SKU-{}", Uuid::new_v4())),
            barcode: None,
            shipping_profile_slug: None,
            option1: Some("Default".to_string()),
            option2: None,
            option3: None,
            prices: vec![PriceInput {
                currency_code: "USD".to_string(),
                channel_id: None,
                channel_slug: None,
                amount: Decimal::from_str("99.99").expect("valid decimal"),
                compare_at_amount: Some(Decimal::from_str("149.99").expect("valid decimal")),
            }],
            inventory_quantity: 10,
            inventory_policy: "deny".to_string(),
            weight: Some(Decimal::from_str("1.5").expect("valid decimal")),
            weight_unit: Some("kg".to_string()),
        }],
        seller_id: None,
        vendor: Some("Migration Test Vendor".to_string()),
        product_type: Some("Physical".to_string()),
        shipping_profile_slug: None,
        tags: Vec::new(),
        publish: false,
        metadata: serde_json::json!({ "source": "migration-smoke" }),
    }
}

#[tokio::test]
async fn ecommerce_migrations_create_expected_tables() {
    let db = setup_test_db_with_migrations::<Migrator>().await;
    let tables = load_sqlite_tables(&db).await;

    for table in [
        "products",
        "product_translations",
        "product_images",
        "product_image_translations",
        "product_options",
        "product_option_translations",
        "product_option_values",
        "product_option_value_translations",
        "product_variants",
        "product_variant_translations",
        "variant_option_values",
        "price_lists",
        "prices",
        "regions",
        "carts",
        "cart_line_items",
        "cart_line_item_translations",
        "cart_tax_lines",
        "customers",
        "payment_collections",
        "payments",
        "refunds",
        "shipping_options",
        "fulfillments",
        "stock_locations",
        "inventory_items",
        "inventory_levels",
        "reservation_items",
        "orders",
        "order_line_items",
        "order_line_item_translations",
        "order_tax_lines",
    ] {
        assert!(
            tables.contains(table),
            "expected migrated schema to include table `{table}`, found: {tables:?}"
        );
    }
}

fn create_order_input() -> CreateOrderInput {
    CreateOrderInput {
        customer_id: Some(Uuid::new_v4()),
        currency_code: "usd".to_string(),
        shipping_total: Decimal::from_str("0.00").expect("valid decimal"),
        line_items: vec![
            CreateOrderLineItemInput {
                product_id: Some(Uuid::new_v4()),
                variant_id: Some(Uuid::new_v4()),
                shipping_profile_slug: "default".to_string(),
                seller_id: None,
                sku: Some(format!("ORD-SKU-{}", Uuid::new_v4())),
                title: "Migration order product".to_string(),
                quantity: 2,
                unit_price: Decimal::from_str("29.99").expect("valid decimal"),
                metadata: serde_json::json!({ "source": "migration-smoke", "slot": 1 }),
            },
            CreateOrderLineItemInput {
                product_id: None,
                variant_id: None,
                shipping_profile_slug: "default".to_string(),
                seller_id: None,
                sku: Some(format!("ORD-ADDON-{}", Uuid::new_v4())),
                title: "Migration add-on".to_string(),
                quantity: 1,
                unit_price: Decimal::from_str("5.00").expect("valid decimal"),
                metadata: serde_json::json!({ "source": "migration-smoke", "slot": 2 }),
            },
        ],
        adjustments: Vec::new(),
        tax_lines: Vec::new(),
        metadata: serde_json::json!({ "source": "migration-smoke" }),
    }
}

fn create_cart_input() -> CreateCartInput {
    CreateCartInput {
        customer_id: Some(Uuid::new_v4()),
        email: Some("migration-buyer@example.com".to_string()),
        region_id: None,
        country_code: None,
        locale_code: None,
        selected_shipping_option_id: None,
        currency_code: "usd".to_string(),
        metadata: serde_json::json!({ "source": "migration-smoke" }),
    }
}

fn create_cart_line_item_input() -> AddCartLineItemInput {
    AddCartLineItemInput {
        product_id: Some(Uuid::new_v4()),
        variant_id: Some(Uuid::new_v4()),
        shipping_profile_slug: None,
        sku: Some(format!("CART-SKU-{}", Uuid::new_v4())),
        title: "Migration cart product".to_string(),
        quantity: 2,
        unit_price: Decimal::from_str("15.50").expect("valid decimal"),
        metadata: serde_json::json!({ "source": "migration-smoke" }),
    }
}

fn create_customer_input() -> CreateCustomerInput {
    CreateCustomerInput {
        user_id: Some(Uuid::new_v4()),
        email: "migration-customer@example.com".to_string(),
        first_name: Some("Migration".to_string()),
        last_name: Some("Customer".to_string()),
        phone: Some("+123456789".to_string()),
        locale: Some("en".to_string()),
        metadata: serde_json::json!({ "source": "migration-smoke" }),
    }
}

fn create_payment_collection_input() -> CreatePaymentCollectionInput {
    CreatePaymentCollectionInput {
        cart_id: Some(Uuid::new_v4()),
        order_id: Some(Uuid::new_v4()),
        customer_id: Some(Uuid::new_v4()),
        currency_code: "usd".to_string(),
        amount: Decimal::from_str("59.99").expect("valid decimal"),
        metadata: serde_json::json!({ "source": "migration-smoke" }),
    }
}

fn create_shipping_option_input() -> CreateShippingOptionInput {
    CreateShippingOptionInput {
        translations: vec![ShippingOptionTranslationInput {
            locale: "en".to_string(),
            name: "Migration Shipping".to_string(),
        }],
        currency_code: "usd".to_string(),
        amount: Decimal::from_str("12.50").expect("valid decimal"),
        provider_id: None,
        allowed_shipping_profile_slugs: None,
        metadata: serde_json::json!({ "source": "migration-smoke" }),
    }
}

#[tokio::test]
async fn catalog_service_supports_multilingual_catalog_data_on_migrated_schema() {
    let db = setup_test_db_with_migrations::<Migrator>().await;
    let event_bus = mock_transactional_event_bus();
    let service = CatalogService::new(db.clone(), event_bus);
    let tenant_id = Uuid::new_v4();
    let actor_id = Uuid::new_v4();
    seed_tenant(&db, tenant_id).await;

    let created = service
        .create_product(tenant_id, actor_id, create_product_input())
        .await
        .expect("catalog create_product should work on migrated schema");

    assert_eq!(created.translations.len(), 2);
    assert!(created.translations.iter().any(|item| item.locale == "en"));
    assert!(created.translations.iter().any(|item| item.locale == "ru"));
    assert_eq!(created.options.len(), 1);
    assert_eq!(created.options[0].translations.len(), 1);
    assert_eq!(created.variants[0].translations.len(), 2);

    let fetched = service
        .get_product(tenant_id, created.id)
        .await
        .expect("catalog get_product should work on migrated schema");

    assert_eq!(fetched.translations.len(), 2);
    assert_eq!(fetched.options[0].translations.len(), 1);
    assert_eq!(fetched.options[0].translations[0].values.len(), 2);
    assert_eq!(fetched.variants[0].translations.len(), 2);
    assert_eq!(
        fetched
            .translations
            .iter()
            .find(|item| item.locale == "ru")
            .map(|item| item.title.as_str()),
        Some("Migration-backed RU product")
    );
}
