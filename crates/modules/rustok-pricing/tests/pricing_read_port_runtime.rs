use rust_decimal::Decimal;
use rustok_api::{PortActor, PortContext, PortErrorKind};
use rustok_commerce_foundation::entities::product::ProductStatus;
use rustok_commerce_foundation::entities::{product, product_variant};
use rustok_core::events::MemoryTransport;
use rustok_pricing::{
    PriceListProjectionRequest, PriceResolutionContext, PricingReadPort, PricingService,
    ResolveProductPriceRequest, ResolveProductPricesRequest,
};
use rustok_pricing_persistence::entities::{price, price_list, price_list_translation};
use sea_orm::{
    ActiveModelTrait, ActiveValue::Set, ConnectionTrait, Database, DatabaseConnection, DbBackend,
    EntityTrait, Schema,
};
use std::sync::Arc;
use uuid::Uuid;

async fn create_entity_table(
    db: &DatabaseConnection,
    builder: &DbBackend,
    mut statement: sea_orm::sea_query::TableCreateStatement,
) {
    statement.if_not_exists();
    db.execute_raw(builder.build(&statement))
        .await
        .expect("failed to create pricing port runtime test table");
}

async fn setup_pricing_port_service() -> (DatabaseConnection, PricingService, Uuid, Uuid) {
    let db = Database::connect("sqlite::memory:")
        .await
        .expect("failed to open in-memory pricing port runtime database");
    let builder = db.get_database_backend();
    let schema = Schema::new(builder);
    create_entity_table(
        &db,
        &builder,
        schema.create_table_from_entity(product::Entity),
    )
    .await;
    create_entity_table(
        &db,
        &builder,
        schema.create_table_from_entity(product_variant::Entity),
    )
    .await;
    create_entity_table(
        &db,
        &builder,
        schema.create_table_from_entity(price::Entity),
    )
    .await;
    create_entity_table(
        &db,
        &builder,
        schema.create_table_from_entity(price_list::Entity),
    )
    .await;
    create_entity_table(
        &db,
        &builder,
        schema.create_table_from_entity(price_list_translation::Entity),
    )
    .await;

    let tenant_id = Uuid::new_v4();
    let product_id = Uuid::new_v4();
    let variant_id = Uuid::new_v4();
    let now = chrono::Utc::now();
    product::ActiveModel {
        id: Set(product_id),
        tenant_id: Set(tenant_id),
        status: Set(ProductStatus::Draft),
        seller_id: Set(None),
        vendor: Set(None),
        product_type: Set(None),
        shipping_profile_slug: Set(None),
        primary_category_id: Set(None),
        metadata: Set(serde_json::json!({})),
        created_at: Set(now.into()),
        updated_at: Set(now.into()),
        published_at: Set(None),
    }
    .insert(&db)
    .await
    .expect("failed to seed pricing product");
    product_variant::ActiveModel {
        id: Set(variant_id),
        product_id: Set(product_id),
        tenant_id: Set(tenant_id),
        sku: Set(Some("pricing-port-runtime".to_string())),
        barcode: Set(None),
        shipping_profile_slug: Set(None),
        ean: Set(None),
        upc: Set(None),
        inventory_policy: Set("deny".to_string()),
        inventory_management: Set("managed".to_string()),
        inventory_quantity: Set(0),
        weight: Set(None),
        weight_unit: Set(None),
        combination_identity: Set(None),
        position: Set(0),
        created_at: Set(now.into()),
        updated_at: Set(now.into()),
    }
    .insert(&db)
    .await
    .expect("failed to seed pricing variant");
    price::ActiveModel {
        id: Set(Uuid::new_v4()),
        variant_id: Set(variant_id),
        price_list_id: Set(None),
        channel_id: Set(None),
        channel_slug: Set(None),
        currency_code: Set("USD".to_string()),
        region_id: Set(None),
        amount: Set(Decimal::new(8999, 2)),
        compare_at_amount: Set(Some(Decimal::new(9999, 2))),
        legacy_amount: Set(Some(8999)),
        legacy_compare_at_amount: Set(Some(9999)),
        min_quantity: Set(None),
        max_quantity: Set(None),
    }
    .insert(&db)
    .await
    .expect("failed to seed pricing row");

    (
        db.clone(),
        PricingService::new(
            db,
            rustok_outbox::TransactionalEventBus::new(Arc::new(MemoryTransport::new())),
        ),
        tenant_id,
        variant_id,
    )
}

fn read_context(tenant_id: Uuid, correlation_id: &str) -> PortContext {
    PortContext::new(
        tenant_id.to_string(),
        PortActor::service("rustok-cart.storefront"),
        "en",
        correlation_id,
    )
    .with_deadline(std::time::Duration::from_secs(2))
}

#[tokio::test]
async fn pricing_read_port_executes_variant_first_resolution_against_persistence() {
    let (db, service, tenant_id, variant_id) = setup_pricing_port_service().await;

    let snapshot = service
        .resolve_product_price(
            read_context(tenant_id, "pricing-port-runtime-variant-first"),
            ResolveProductPriceRequest {
                product_id: None,
                variant_id,
                region_id: None,
                channel_id: None,
                channel_slug: None,
                price_list_id: None,
                quantity: Some(2),
                currency_code: "usd".to_string(),
            },
        )
        .await
        .expect("variant-first pricing port resolution must succeed");

    let stored = product_variant::Entity::find_by_id(variant_id)
        .one(&db)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(snapshot.product_id, stored.product_id);
    assert_eq!(snapshot.context.quantity, Some(2));
    assert!(!snapshot.source.price_id().is_nil());
    assert_eq!(snapshot.variant_id, variant_id);
    assert_eq!(snapshot.currency_code, "USD");
    assert_eq!(snapshot.amount, Decimal::new(8999, 2));
    assert_eq!(snapshot.compare_at_amount, Some(Decimal::new(9999, 2)));
    assert!(snapshot.on_sale);
}

#[tokio::test]
async fn pricing_read_port_rejects_missing_read_deadline_before_provider_access() {
    let (_db, service, tenant_id, variant_id) = setup_pricing_port_service().await;

    let error = service
        .resolve_product_price(
            PortContext::new(
                tenant_id.to_string(),
                PortActor::service("rustok-cart.storefront"),
                "en",
                "pricing-port-runtime-missing-deadline",
            ),
            ResolveProductPriceRequest {
                product_id: None,
                variant_id,
                region_id: None,
                channel_id: None,
                channel_slug: None,
                price_list_id: None,
                quantity: Some(1),
                currency_code: "USD".to_string(),
            },
        )
        .await
        .expect_err("read port must require a deadline");

    assert_eq!(error.kind, PortErrorKind::Timeout);
    assert_eq!(error.code, "port.deadline_required");
}

#[tokio::test]
async fn pricing_read_port_executes_price_list_projection_against_persistence() {
    let (db, service, tenant_id, _) = setup_pricing_port_service().await;
    let price_list_id = Uuid::new_v4();
    let now = chrono::Utc::now();
    price_list::ActiveModel {
        id: Set(price_list_id),
        tenant_id: Set(tenant_id),
        r#type: Set("sale".to_string()),
        status: Set("active".to_string()),
        channel_id: Set(None),
        channel_slug: Set(None),
        rule_kind: Set(None),
        adjustment_percent: Set(None),
        starts_at: Set(None),
        ends_at: Set(None),
        created_at: Set(now.into()),
        updated_at: Set(now.into()),
    }
    .insert(&db)
    .await
    .expect("failed to seed active price list");
    price_list_translation::ActiveModel {
        id: Set(Uuid::new_v4()),
        price_list_id: Set(price_list_id),
        locale: Set("en".to_string()),
        name: Set("Storefront sale".to_string()),
        description: Set(None),
    }
    .insert(&db)
    .await
    .expect("failed to seed price list translation");

    let projection = service
        .read_price_list_projection(
            read_context(tenant_id, "pricing-port-runtime-price-list"),
            PriceListProjectionRequest {
                price_list_id,
                locale: Some("en".to_string()),
            },
        )
        .await
        .expect("price-list projection must resolve through the pricing port");

    assert_eq!(projection.price_list_id, price_list_id);
    assert_eq!(projection.title, "Storefront sale");
}

fn quote_request(variant_ids: Vec<Uuid>) -> ResolveProductPricesRequest {
    ResolveProductPricesRequest {
        product_id: None,
        variant_ids,
        pricing: PriceResolutionContext {
            currency_code: "usd".into(),
            region_id: None,
            price_list_id: None,
            channel_id: None,
            channel_slug: None,
            quantity: Some(2),
        },
    }
}

async fn additional_sku(
    db: &DatabaseConnection,
    from_variant: Uuid,
    tenant_id: Uuid,
) -> (Uuid, Uuid) {
    // One default SKU in each separate Product: do not seed invalid multiple no-axis defaults.
    let mut variant = product_variant::Entity::find_by_id(from_variant)
        .one(db)
        .await
        .unwrap()
        .unwrap();
    let mut parent = product::Entity::find_by_id(variant.product_id)
        .one(db)
        .await
        .unwrap()
        .unwrap();
    parent.id = Uuid::new_v4();
    parent.tenant_id = tenant_id;
    let product_id = parent.id;
    let parent: product::ActiveModel = parent.into();
    parent.insert(db).await.unwrap();
    variant.id = Uuid::new_v4();
    variant.product_id = product_id;
    variant.tenant_id = tenant_id;
    variant.sku = Some(format!("runtime-{}", variant.id));
    let variant_id = variant.id;
    let variant: product_variant::ActiveModel = variant.into();
    variant.insert(db).await.unwrap();
    (product_id, variant_id)
}

#[tokio::test]
async fn batch_quotes_keep_identity_order_context_and_an_explicit_unpriced_item() {
    let (db, service, tenant_id, variant_id) = setup_pricing_port_service().await;
    let (second_product, second_variant) = additional_sku(&db, variant_id, tenant_id).await;
    let batch = service
        .resolve_product_prices(
            read_context(tenant_id, "pricing-batch-order"),
            quote_request(vec![second_variant, variant_id]),
        )
        .await
        .unwrap();
    assert_eq!(batch.context.currency_code, "USD");
    assert_eq!(batch.context.quantity, Some(2));
    assert_eq!(batch.items.len(), 2);
    assert_eq!(batch.items[0].product_id, second_product);
    assert_eq!(batch.items[0].variant_id, second_variant);
    assert!(batch.items[0].price.is_none());
    assert_eq!(batch.items[1].variant_id, variant_id);
    let price = batch.items[1].price.as_ref().unwrap();
    assert_eq!(price.amount, Decimal::new(8999, 2));
    assert!(!price.source.price_id().is_nil());
    // No Product translations, axes, attribute, media or inventory tables exist in this fixture.
    // SKU validation must remain a small Product-owned identity projection.
}

#[tokio::test]
async fn batch_rejects_foreign_or_wrong_product_skus_without_returning_partial_prices() {
    let (db, service, tenant_id, variant_id) = setup_pricing_port_service().await;
    let (_, foreign) = additional_sku(&db, variant_id, Uuid::new_v4()).await;
    let error = service
        .resolve_product_prices(
            read_context(tenant_id, "pricing-batch-foreign"),
            quote_request(vec![variant_id, foreign]),
        )
        .await
        .unwrap_err();
    assert_eq!(error.kind, PortErrorKind::NotFound);
    let mut request = quote_request(vec![variant_id]);
    request.product_id = Some(Uuid::new_v4());
    let error = service
        .resolve_product_prices(read_context(tenant_id, "pricing-batch-root"), request)
        .await
        .unwrap_err();
    assert_eq!(error.kind, PortErrorKind::Validation);
    assert_eq!(error.code, "pricing.variant_product_mismatch");
}

#[tokio::test]
async fn batch_bounds_and_deadline_are_checked_before_accessing_owner_storage() {
    // Deliberately no owner tables: accidental lookup before validation would fail.
    let empty_db = Database::connect("sqlite::memory:").await.unwrap();
    let service = PricingService::new(
        empty_db,
        rustok_outbox::TransactionalEventBus::new(Arc::new(MemoryTransport::new())),
    );
    let tenant = Uuid::new_v4();
    let variant = Uuid::new_v4();
    for ids in [
        vec![],
        vec![variant, variant],
        vec![Uuid::nil()],
        vec![variant; 257],
    ] {
        let error = service
            .resolve_product_prices(
                read_context(tenant, "pricing-batch-bounds"),
                quote_request(ids),
            )
            .await
            .unwrap_err();
        assert_eq!(error.kind, PortErrorKind::Validation);
    }
    let mut context = read_context(tenant, "pricing-batch-deadline");
    context.deadline_ms = None;
    let error = service
        .resolve_product_prices(context, quote_request(vec![variant]))
        .await
        .unwrap_err();
    assert_eq!(error.code, "port.deadline_required");
    let mut context = read_context(tenant, "pricing-batch-deadline-bound");
    context.deadline_ms = Some(u64::MAX);
    let error = service
        .resolve_product_prices(context, quote_request(vec![variant]))
        .await
        .unwrap_err();
    assert_eq!(error.code, "pricing.deadline_invalid");
}

#[tokio::test]
async fn ambiguous_prices_refuse_a_quote_instead_of_using_uuid_order() {
    let (db, service, tenant_id, variant_id) = setup_pricing_port_service().await;
    let mut duplicate = price::Entity::find().one(&db).await.unwrap().unwrap();
    duplicate.id = Uuid::new_v4();
    duplicate.amount = Decimal::new(5000, 2);
    let duplicate: price::ActiveModel = duplicate.into();
    duplicate.insert(&db).await.unwrap();
    let error = service
        .resolve_product_prices(
            read_context(tenant_id, "pricing-batch-ambiguous"),
            quote_request(vec![variant_id]),
        )
        .await
        .unwrap_err();
    assert_eq!(error.kind, PortErrorKind::Conflict);
    assert_eq!(error.code, "pricing.price_ambiguous");
    assert!(!error.retryable);
}

#[tokio::test]
async fn unsupported_persisted_list_rules_cannot_fall_back_to_available_base_prices() {
    let (db, service, tenant_id, variant_id) = setup_pricing_port_service().await;
    let list_id = Uuid::new_v4();
    let now = chrono::Utc::now();
    price_list::ActiveModel {
        id: Set(list_id),
        tenant_id: Set(tenant_id),
        r#type: Set("sale".into()),
        status: Set("active".into()),
        channel_id: Set(None),
        channel_slug: Set(None),
        rule_kind: Set(Some("future_rule".into())),
        adjustment_percent: Set(Some(Decimal::new(10, 0))),
        starts_at: Set(None),
        ends_at: Set(None),
        created_at: Set(now.into()),
        updated_at: Set(now.into()),
    }
    .insert(&db)
    .await
    .unwrap();
    let mut request = quote_request(vec![variant_id]);
    request.pricing.price_list_id = Some(list_id);
    let error = service
        .resolve_product_prices(read_context(tenant_id, "pricing-batch-rule"), request)
        .await
        .unwrap_err();
    assert_eq!(error.kind, PortErrorKind::InvariantViolation);
    assert_eq!(error.code, "pricing.stored_data_invalid");
}

#[tokio::test]
async fn zero_price_is_priced_and_keeps_its_real_sku_identity() {
    let (db, service, tenant_id, variant_id) = setup_pricing_port_service().await;
    let (product_id, free_variant) = additional_sku(&db, variant_id, tenant_id).await;
    let mut row = price::Entity::find().one(&db).await.unwrap().unwrap();
    row.id = Uuid::new_v4();
    row.variant_id = free_variant;
    row.amount = Decimal::ZERO;
    row.compare_at_amount = None;
    row.legacy_amount = Some(9999);
    let row: price::ActiveModel = row.into();
    row.insert(&db).await.unwrap();
    let mut request = quote_request(vec![free_variant]);
    request.product_id = Some(product_id);
    let result = service
        .resolve_product_prices(read_context(tenant_id, "pricing-batch-free"), request)
        .await
        .unwrap();
    assert_eq!(
        result.items[0].price.as_ref().unwrap().amount,
        Decimal::ZERO
    );
    assert_eq!(result.items[0].product_id, product_id);
}
