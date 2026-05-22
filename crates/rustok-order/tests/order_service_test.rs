use chrono::Utc;
use flex::attached;
use rust_decimal::Decimal;
use rustok_order::dto::{CreateOrderAdjustmentInput, CreateOrderInput, CreateOrderLineItemInput};
use rustok_order::entities::{order, order_tax_line};
use rustok_order::error::OrderError;
use rustok_order::services::OrderService;
use rustok_test_utils::{db::setup_test_db, mock_transactional_event_bus};
use sea_orm::{ActiveModelTrait, ActiveValue::Set, ColumnTrait, ConnectionTrait, DbBackend, EntityTrait, QueryFilter, Statement};
use std::str::FromStr;
use uuid::Uuid;

mod support;

mod order_field_definitions {
    rustok_core::define_field_definitions_entity!("order_field_definitions");
}

async fn setup() -> OrderService {
    let db = setup_test_db().await;
    support::ensure_order_schema(&db).await;
    OrderService::new(db, mock_transactional_event_bus())
}

fn create_order_input() -> CreateOrderInput {
    CreateOrderInput {
        customer_id: Some(Uuid::new_v4()),
        currency_code: "usd".to_string(),
        shipping_total: Decimal::ZERO,
        line_items: vec![
            CreateOrderLineItemInput {
                product_id: Some(Uuid::new_v4()),
                variant_id: Some(Uuid::new_v4()),
                shipping_profile_slug: "default".to_string(),
                seller_id: None,
                sku: Some("SKU-1".to_string()),
                title: "Test product".to_string(),
                quantity: 2,
                unit_price: Decimal::from_str("19.99").unwrap(),
                metadata: serde_json::json!({ "slot": 1 }),
            },
            CreateOrderLineItemInput {
                product_id: None,
                variant_id: None,
                shipping_profile_slug: "gift".to_string(),
                seller_id: None,
                sku: Some("SKU-2".to_string()),
                title: "Gift wrap".to_string(),
                quantity: 1,
                unit_price: Decimal::from_str("4.00").unwrap(),
                metadata: serde_json::json!({ "slot": 2 }),
            },
        ],
        adjustments: Vec::new(),
        tax_lines: Vec::new(),
        metadata: serde_json::json!({ "source": "order-test" }),
    }
}

async fn insert_order_field_definition(
    db: &sea_orm::DatabaseConnection,
    tenant_id: Uuid,
    field_key: &str,
    is_localized: bool,
    is_required: bool,
    default_value: Option<serde_json::Value>,
    position: i32,
) {
    let now = Utc::now();
    order_field_definitions::ActiveModel {
        id: Set(rustok_core::generate_id()),
        tenant_id: Set(tenant_id),
        field_key: Set(field_key.to_string()),
        field_type: Set("text".to_string()),
        label: Set(serde_json::json!({ "en": field_key })),
        description: Set(None),
        is_localized: Set(is_localized),
        is_required: Set(is_required),
        default_value: Set(default_value),
        validation: Set(None),
        position: Set(position),
        is_active: Set(true),
        created_at: Set(now.into()),
        updated_at: Set(now.into()),
    }
    .insert(db)
    .await
    .expect("field definition should insert");
}

#[tokio::test]
async fn create_order_persists_snapshot_and_total() {
    let service = setup().await;
    let tenant_id = Uuid::new_v4();
    let actor_id = Uuid::new_v4();

    let created = service
        .create_order(tenant_id, actor_id, create_order_input())
        .await
        .unwrap();

    assert_eq!(created.status, "pending");
    assert_eq!(created.currency_code, "USD");
    assert_eq!(created.line_items.len(), 2);
    assert_eq!(created.subtotal_amount, Decimal::from_str("43.98").unwrap());
    assert_eq!(created.adjustment_total, Decimal::ZERO);
    assert_eq!(created.total_amount, Decimal::from_str("43.98").unwrap());
}

#[tokio::test]
async fn order_tax_lines_insert_without_provider_id_use_region_default() {
    let db = setup_test_db().await;
    support::ensure_order_schema(&db).await;
    let service = OrderService::new(db.clone(), mock_transactional_event_bus());
    let tenant_id = Uuid::new_v4();
    let actor_id = Uuid::new_v4();

    let created = service
        .create_order(tenant_id, actor_id, create_order_input())
        .await
        .expect("order should be created");

    db.execute(Statement::from_sql_and_values(
        DbBackend::Sqlite,
        "INSERT INTO order_tax_lines (id, tenant_id, order_id, line_item_id, shipping_option_id, rate, amount, name, metadata, created_at, updated_at) VALUES (?, ?, ?, NULL, NULL, ?, ?, ?, ?, CURRENT_TIMESTAMP, CURRENT_TIMESTAMP)",
        vec![
            rustok_core::generate_id().into(),
            tenant_id.into(),
            created.id.into(),
            Decimal::from_str("7.00").expect("valid decimal").into(),
            Decimal::from_str("3.50").expect("valid decimal").into(),
            "VAT backfill smoke".to_string().into(),
            serde_json::json!({"scope": "order"}).into(),
        ],
    ))
    .await
    .expect("legacy-style insert should use provider_id default");

    let inserted = order_tax_line::Entity::find()
        .filter(order_tax_line::Column::OrderId.eq(created.id))
        .filter(order_tax_line::Column::Name.eq("VAT backfill smoke"))
        .one(&db)
        .await
        .expect("tax line query should succeed")
        .expect("tax line should exist");

    assert_eq!(inserted.provider_id, "region_default");

    db.execute(Statement::from_sql_and_values(
        DbBackend::Sqlite,
        "INSERT INTO order_tax_lines (id, tenant_id, order_id, line_item_id, shipping_option_id, rate, amount, name, provider_id, metadata, created_at, updated_at) VALUES (?, ?, ?, NULL, NULL, ?, ?, ?, ?, ?, CURRENT_TIMESTAMP, CURRENT_TIMESTAMP)",
        vec![
            rustok_core::generate_id().into(),
            tenant_id.into(),
            created.id.into(),
            Decimal::from_str("9.00").expect("valid decimal").into(),
            Decimal::from_str("4.00").expect("valid decimal").into(),
            "VAT explicit provider".to_string().into(),
            "custom_tax".to_string().into(),
            serde_json::json!({"scope": "order"}).into(),
        ],
    ))
    .await
    .expect("explicit provider insert should succeed");

    let explicit = order_tax_line::Entity::find()
        .filter(order_tax_line::Column::OrderId.eq(created.id))
        .filter(order_tax_line::Column::Name.eq("VAT explicit provider"))
        .one(&db)
        .await
        .expect("explicit provider tax line query should succeed")
        .expect("explicit provider tax line should exist");

    assert_eq!(explicit.provider_id, "custom_tax");
}

#[tokio::test]
async fn create_order_persists_typed_adjustments_and_net_total() {
    let service = setup().await;
    let tenant_id = Uuid::new_v4();
    let actor_id = Uuid::new_v4();

    let created = service
        .create_order(
            tenant_id,
            actor_id,
            CreateOrderInput {
                adjustments: vec![CreateOrderAdjustmentInput {
                    line_item_index: Some(0),
                    source_type: "Promotion".to_string(),
                    source_id: Some("promo-spring".to_string()),
                    amount: Decimal::from_str("3.98").unwrap(),
                    metadata: serde_json::json!({
                        "rule_code": "spring",
                        "display_label": "Spring sale"
                    }),
                }],
                ..create_order_input()
            },
        )
        .await
        .unwrap();

    assert_eq!(created.subtotal_amount, Decimal::from_str("43.98").unwrap());
    assert_eq!(created.adjustment_total, Decimal::from_str("3.98").unwrap());
    assert_eq!(created.total_amount, Decimal::from_str("40.00").unwrap());
    assert_eq!(created.adjustments.len(), 1);
    assert_eq!(
        created.adjustments[0].line_item_id,
        Some(created.line_items[0].id)
    );
    assert_eq!(created.adjustments[0].source_type, "promotion");
    assert_eq!(created.adjustments[0].currency_code, "USD");
    assert!(created.adjustments[0]
        .metadata
        .get("display_label")
        .is_none());
}

#[tokio::test]
async fn create_order_with_channel_persists_channel_snapshot() {
    let service = setup().await;
    let tenant_id = Uuid::new_v4();
    let actor_id = Uuid::new_v4();
    let channel_id = Uuid::new_v4();

    let created = service
        .create_order_with_channel(
            tenant_id,
            actor_id,
            create_order_input(),
            Some(channel_id),
            Some("marketplace-eu".to_string()),
        )
        .await
        .unwrap();

    assert_eq!(created.channel_id, Some(channel_id));
    assert_eq!(created.channel_slug.as_deref(), Some("marketplace-eu"));
}

#[tokio::test]
async fn order_lifecycle_transitions_persist_status_metadata() {
    let service = setup().await;
    let tenant_id = Uuid::new_v4();
    let actor_id = Uuid::new_v4();

    let created = service
        .create_order(tenant_id, actor_id, create_order_input())
        .await
        .unwrap();

    let confirmed = service
        .confirm_order(tenant_id, actor_id, created.id)
        .await
        .unwrap();
    assert_eq!(confirmed.status, "confirmed");
    assert!(confirmed.confirmed_at.is_some());

    let paid = service
        .mark_paid(
            tenant_id,
            actor_id,
            created.id,
            "pay_123".to_string(),
            "manual".to_string(),
        )
        .await
        .unwrap();
    assert_eq!(paid.status, "paid");
    assert_eq!(paid.payment_id.as_deref(), Some("pay_123"));

    let shipped = service
        .ship_order(
            tenant_id,
            actor_id,
            created.id,
            "trk_123".to_string(),
            "dhl".to_string(),
        )
        .await
        .unwrap();
    assert_eq!(shipped.status, "shipped");
    assert_eq!(shipped.tracking_number.as_deref(), Some("trk_123"));

    let delivered = service
        .deliver_order(
            tenant_id,
            actor_id,
            created.id,
            Some("front-desk".to_string()),
        )
        .await
        .unwrap();
    assert_eq!(delivered.status, "delivered");
    assert_eq!(delivered.delivered_signature.as_deref(), Some("front-desk"));
}

#[tokio::test]
async fn invalid_transition_is_rejected() {
    let service = setup().await;
    let tenant_id = Uuid::new_v4();
    let actor_id = Uuid::new_v4();

    let created = service
        .create_order(tenant_id, actor_id, create_order_input())
        .await
        .unwrap();

    let error = service
        .ship_order(
            tenant_id,
            actor_id,
            created.id,
            "trk_123".to_string(),
            "dhl".to_string(),
        )
        .await
        .unwrap_err();

    match error {
        OrderError::InvalidTransition { from, to } => {
            assert_eq!(from, "pending");
            assert_eq!(to, "shipped");
        }
        other => panic!("expected invalid transition, got {other:?}"),
    }
}

#[tokio::test]
async fn localized_order_custom_fields_resolve_from_attached_values() {
    let db = setup_test_db().await;
    support::ensure_order_schema(&db).await;
    let service = OrderService::new(db.clone(), mock_transactional_event_bus());
    let tenant_id = Uuid::new_v4();
    let actor_id = Uuid::new_v4();
    let now = Utc::now();

    order_field_definitions::ActiveModel {
        id: Set(rustok_core::generate_id()),
        tenant_id: Set(tenant_id),
        field_key: Set("gift_message".to_string()),
        field_type: Set("text".to_string()),
        label: Set(serde_json::json!({ "en": "Gift message" })),
        description: Set(None),
        is_localized: Set(true),
        is_required: Set(false),
        default_value: Set(None),
        validation: Set(None),
        position: Set(0),
        is_active: Set(true),
        created_at: Set(now.into()),
        updated_at: Set(now.into()),
    }
    .insert(&db)
    .await
    .expect("field definition should insert");

    let created = service
        .create_order(
            tenant_id,
            actor_id,
            CreateOrderInput {
                metadata: serde_json::json!({
                    "locale": "de-DE",
                    "gift_message": "Danke",
                    "source": "order-test"
                }),
                ..create_order_input()
            },
        )
        .await
        .expect("order should be created");

    assert_eq!(created.metadata["gift_message"], serde_json::json!("Danke"));
    assert_eq!(created.metadata["source"], serde_json::json!("order-test"));

    let storefront_view = service
        .get_order_with_locale_fallback(tenant_id, created.id, "de-DE", Some("en"))
        .await
        .expect("order should load with locale fallback");
    assert_eq!(
        storefront_view.metadata["gift_message"],
        serde_json::json!("Danke")
    );
    assert_eq!(
        storefront_view.metadata["source"],
        serde_json::json!("order-test")
    );

    let localized_rows = attached::Entity::find()
        .filter(attached::Column::TenantId.eq(tenant_id))
        .filter(attached::Column::EntityType.eq("order"))
        .filter(attached::Column::EntityId.eq(created.id))
        .all(&db)
        .await
        .expect("localized rows query should succeed");
    assert_eq!(localized_rows.len(), 1);
    assert_eq!(localized_rows[0].field_key, "gift_message");
    assert_eq!(localized_rows[0].locale, "de-DE");
    assert_eq!(localized_rows[0].value, serde_json::json!("Danke"));
}

#[tokio::test]
async fn create_order_applies_defaults_and_splits_localized_values() {
    let db = setup_test_db().await;
    support::ensure_order_schema(&db).await;
    let service = OrderService::new(db.clone(), mock_transactional_event_bus());
    let tenant_id = Uuid::new_v4();
    let actor_id = Uuid::new_v4();

    insert_order_field_definition(
        &db,
        tenant_id,
        "delivery_note",
        false,
        false,
        Some(serde_json::json!("pack carefully")),
        0,
    )
    .await;
    insert_order_field_definition(&db, tenant_id, "gift_message", true, false, None, 1).await;

    let created = service
        .create_order(
            tenant_id,
            actor_id,
            CreateOrderInput {
                metadata: serde_json::json!({
                    "locale": "de-DE",
                    "gift_message": "Danke",
                    "source": "order-test",
                    "unknown_custom": "drop-me"
                }),
                ..create_order_input()
            },
        )
        .await
        .expect("order should be created");

    assert_eq!(created.metadata["gift_message"], serde_json::json!("Danke"));
    assert_eq!(
        created.metadata["delivery_note"],
        serde_json::json!("pack carefully")
    );
    assert_eq!(created.metadata["source"], serde_json::json!("order-test"));
    assert_eq!(
        created.metadata["unknown_custom"],
        serde_json::json!("drop-me")
    );

    let stored_order = order::Entity::find_by_id(created.id)
        .one(&db)
        .await
        .expect("stored order query should succeed")
        .expect("stored order should exist");
    assert_eq!(
        stored_order.metadata,
        serde_json::json!({
            "locale": "de-DE",
            "source": "order-test",
            "unknown_custom": "drop-me",
            "delivery_note": "pack carefully"
        })
    );

    let localized_rows = attached::Entity::find()
        .filter(attached::Column::TenantId.eq(tenant_id))
        .filter(attached::Column::EntityType.eq("order"))
        .filter(attached::Column::EntityId.eq(created.id))
        .all(&db)
        .await
        .expect("localized rows query should succeed");
    assert_eq!(localized_rows.len(), 1);
    assert_eq!(localized_rows[0].field_key, "gift_message");
    assert_eq!(localized_rows[0].locale, "de-DE");
    assert_eq!(localized_rows[0].value, serde_json::json!("Danke"));
}

#[tokio::test]
async fn create_order_rejects_missing_required_custom_field() {
    let db = setup_test_db().await;
    support::ensure_order_schema(&db).await;
    let service = OrderService::new(db.clone(), mock_transactional_event_bus());
    let tenant_id = Uuid::new_v4();
    let actor_id = Uuid::new_v4();

    insert_order_field_definition(&db, tenant_id, "gift_message", true, true, None, 0).await;

    let error = service
        .create_order(
            tenant_id,
            actor_id,
            CreateOrderInput {
                metadata: serde_json::json!({
                    "locale": "de-DE",
                    "source": "order-test"
                }),
                ..create_order_input()
            },
        )
        .await
        .expect_err("order creation should fail without required custom field");

    match error {
        OrderError::Validation(message) => {
            assert!(
                message.contains("Custom field validation failed"),
                "unexpected error: {message}"
            );
        }
        other => panic!("expected validation error, got {other:?}"),
    }
}
