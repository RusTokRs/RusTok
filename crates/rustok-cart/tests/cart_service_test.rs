use chrono::Utc;
use rust_decimal::Decimal;
use rustok_cart::dto::{
    AddCartLineItemInput, CartShippingSelectionInput, CreateCartInput, SetCartAdjustmentInput,
    UpdateCartContextInput,
};
use rustok_cart::error::CartError;
use rustok_cart::services::{cart::CartPricingAdjustmentUpdate, CartService};
use rustok_commerce_foundation::entities::region;
use rustok_fulfillment::entities::shipping_option;
use rustok_test_utils::db::setup_test_db;
use sea_orm::{ActiveModelTrait, ActiveValue::Set, DatabaseConnection};
use std::str::FromStr;
use uuid::Uuid;

mod support;

async fn setup() -> CartService {
    let (_, service) = setup_with_db().await;
    service
}

async fn setup_with_db() -> (DatabaseConnection, CartService) {
    let db = setup_test_db().await;
    support::ensure_cart_schema(&db).await;
    (db.clone(), CartService::new(db))
}

async fn insert_shipping_option(
    db: &DatabaseConnection,
    tenant_id: Uuid,
    id: Uuid,
    currency_code: &str,
    amount: Decimal,
) {
    shipping_option::ActiveModel {
        id: Set(id),
        tenant_id: Set(tenant_id),
        currency_code: Set(currency_code.to_ascii_uppercase()),
        amount: Set(amount),
        provider_id: Set("manual".to_string()),
        active: Set(true),
        metadata: Set(serde_json::json!({})),
        created_at: Set(Utc::now().into()),
        updated_at: Set(Utc::now().into()),
    }
    .insert(db)
    .await
    .expect("shipping option should insert");
}



async fn insert_region(
    db: &DatabaseConnection,
    tenant_id: Uuid,
    id: Uuid,
    currency_code: &str,
    tax_provider_id: Option<&str>,
    metadata: serde_json::Value,
) {
    region::ActiveModel {
        id: Set(id),
        tenant_id: Set(tenant_id),
        currency_code: Set(currency_code.to_ascii_uppercase()),
        tax_provider_id: Set(tax_provider_id.map(|value| value.to_string())),
        tax_rate: Set(Decimal::from_str("10.00").expect("decimal")),
        tax_included: Set(false),
        countries: Set(serde_json::json!([])),
        metadata: Set(metadata),
        created_at: Set(Utc::now().into()),
        updated_at: Set(Utc::now().into()),
    }
    .insert(db)
    .await
    .expect("region should insert");
}

fn create_cart_input() -> CreateCartInput {
    CreateCartInput {
        customer_id: Some(Uuid::new_v4()),
        email: Some("buyer@example.com".to_string()),
        region_id: None,
        country_code: None,
        locale_code: None,
        selected_shipping_option_id: None,
        currency_code: "usd".to_string(),
        metadata: serde_json::json!({ "source": "cart-test" }),
    }
}

fn line_item_input() -> AddCartLineItemInput {
    AddCartLineItemInput {
        product_id: Some(Uuid::new_v4()),
        variant_id: Some(Uuid::new_v4()),
        shipping_profile_slug: None,
        sku: Some("SKU-CART-1".to_string()),
        title: "Cart product".to_string(),
        quantity: 2,
        unit_price: Decimal::from_str("15.50").unwrap(),
        metadata: serde_json::json!({ "slot": 1 }),
    }
}

#[tokio::test]
async fn create_cart_and_add_line_item_updates_totals() {
    let service = setup().await;
    let tenant_id = support::TEST_TENANT_ID;

    let cart = service
        .create_cart(tenant_id, create_cart_input())
        .await
        .unwrap();
    let updated = service
        .add_line_item(tenant_id, cart.id, line_item_input())
        .await
        .unwrap();

    assert_eq!(updated.status, "active");
    assert_eq!(updated.currency_code, "USD");
    assert_eq!(updated.line_items.len(), 1);
    assert_eq!(updated.subtotal_amount, Decimal::from_str("31.00").unwrap());
    assert_eq!(updated.adjustment_total, Decimal::ZERO);
    assert_eq!(updated.total_amount, Decimal::from_str("31.00").unwrap());
}

#[tokio::test]
async fn set_adjustments_recalculates_cart_total_without_localized_labels() {
    let service = setup().await;
    let tenant_id = support::TEST_TENANT_ID;

    let cart = service
        .create_cart(tenant_id, create_cart_input())
        .await
        .unwrap();
    let cart = service
        .add_line_item(tenant_id, cart.id, line_item_input())
        .await
        .unwrap();
    let line_item_id = cart.line_items[0].id;

    let updated = service
        .set_adjustments(
            tenant_id,
            cart.id,
            vec![SetCartAdjustmentInput {
                line_item_id: Some(line_item_id),
                source_type: "Promotion".to_string(),
                source_id: Some("promo-spring".to_string()),
                amount: Decimal::from_str("5.00").unwrap(),
                metadata: serde_json::json!({
                    "rule_code": "spring",
                    "label": "Spring sale"
                }),
            }],
        )
        .await
        .unwrap();

    assert_eq!(updated.subtotal_amount, Decimal::from_str("31.00").unwrap());
    assert_eq!(updated.adjustment_total, Decimal::from_str("5.00").unwrap());
    assert_eq!(updated.total_amount, Decimal::from_str("26.00").unwrap());
    assert_eq!(updated.adjustments.len(), 1);
    assert_eq!(updated.adjustments[0].line_item_id, Some(line_item_id));
    assert_eq!(updated.adjustments[0].source_type, "promotion");
    assert_eq!(updated.adjustments[0].currency_code, "USD");
    assert!(updated.adjustments[0].metadata.get("label").is_none());
}

#[tokio::test]
async fn reprice_line_items_snapshots_pricing_adjustment_without_double_discount() {
    let service = setup().await;
    let tenant_id = support::TEST_TENANT_ID;

    let cart = service
        .create_cart(tenant_id, create_cart_input())
        .await
        .unwrap();
    let cart = service
        .add_line_item(tenant_id, cart.id, line_item_input())
        .await
        .unwrap();
    let line_item_id = cart.line_items[0].id;

    let updated = service
        .reprice_line_items(
            tenant_id,
            cart.id,
            vec![rustok_cart::services::cart::CartLineItemPricingUpdate {
                line_item_id,
                unit_price: Decimal::from_str("20.00").unwrap(),
                pricing_adjustment: Some(CartPricingAdjustmentUpdate {
                    source_id: Some("price-list-1".to_string()),
                    amount: Decimal::from_str("3.00").unwrap(),
                    metadata: serde_json::json!({
                        "kind": "price_list",
                        "discount_percent": "15",
                        "display_label": "Spring sale"
                    }),
                }),
            }],
        )
        .await
        .unwrap();

    assert_eq!(
        updated.line_items[0].unit_price,
        Decimal::from_str("20.00").unwrap()
    );
    assert_eq!(
        updated.line_items[0].total_price,
        Decimal::from_str("40.00").unwrap()
    );
    assert_eq!(updated.subtotal_amount, Decimal::from_str("40.00").unwrap());
    assert_eq!(updated.adjustment_total, Decimal::from_str("3.00").unwrap());
    assert_eq!(updated.total_amount, Decimal::from_str("37.00").unwrap());
    assert_eq!(updated.adjustments.len(), 1);
    assert_eq!(updated.adjustments[0].source_type, "pricing");
    assert_eq!(
        updated.adjustments[0].source_id.as_deref(),
        Some("price-list-1")
    );
    assert!(updated.adjustments[0]
        .metadata
        .get("display_label")
        .is_none());
}

#[tokio::test]
async fn add_line_item_can_snapshot_pricing_adjustment_atomically() {
    let service = setup().await;
    let tenant_id = support::TEST_TENANT_ID;

    let cart = service
        .create_cart(tenant_id, create_cart_input())
        .await
        .unwrap();

    let updated = service
        .add_line_item_with_pricing_adjustment(
            tenant_id,
            cart.id,
            AddCartLineItemInput {
                unit_price: Decimal::from_str("20.00").unwrap(),
                ..line_item_input()
            },
            Some(CartPricingAdjustmentUpdate {
                source_id: Some("price-list-atomic".to_string()),
                amount: Decimal::from_str("3.00").unwrap(),
                metadata: serde_json::json!({
                    "kind": "price_list",
                    "display_label": "Spring sale"
                }),
            }),
        )
        .await
        .unwrap();

    assert_eq!(updated.line_items.len(), 1);
    assert_eq!(
        updated.line_items[0].unit_price,
        Decimal::from_str("20.00").unwrap()
    );
    assert_eq!(
        updated.line_items[0].total_price,
        Decimal::from_str("40.00").unwrap()
    );
    assert_eq!(updated.subtotal_amount, Decimal::from_str("40.00").unwrap());
    assert_eq!(updated.adjustment_total, Decimal::from_str("3.00").unwrap());
    assert_eq!(updated.total_amount, Decimal::from_str("37.00").unwrap());
    assert_eq!(updated.adjustments.len(), 1);
    assert_eq!(updated.adjustments[0].source_type, "pricing");
    assert_eq!(
        updated.adjustments[0].source_id.as_deref(),
        Some("price-list-atomic")
    );
    assert!(updated.adjustments[0]
        .metadata
        .get("display_label")
        .is_none());
}

#[tokio::test]
async fn apply_percentage_promotion_stacks_on_remaining_line_item_amount() {
    let service = setup().await;
    let tenant_id = support::TEST_TENANT_ID;

    let cart = service
        .create_cart(tenant_id, create_cart_input())
        .await
        .unwrap();
    let cart = service
        .add_line_item_with_pricing_adjustment(
            tenant_id,
            cart.id,
            AddCartLineItemInput {
                unit_price: Decimal::from_str("20.00").unwrap(),
                ..line_item_input()
            },
            Some(CartPricingAdjustmentUpdate {
                source_id: Some("price-list-1".to_string()),
                amount: Decimal::from_str("3.00").unwrap(),
                metadata: serde_json::json!({
                    "kind": "price_list"
                }),
            }),
        )
        .await
        .unwrap();
    let line_item_id = cart.line_items[0].id;

    let preview = service
        .preview_percentage_promotion(
            tenant_id,
            cart.id,
            Some(line_item_id),
            "promo-line-10",
            Decimal::from_str("10").unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(preview.base_amount, Decimal::from_str("37.00").unwrap());
    assert_eq!(
        preview.adjustment_amount,
        Decimal::from_str("3.70").unwrap()
    );
    assert_eq!(preview.adjusted_amount, Decimal::from_str("33.30").unwrap());

    let updated = service
        .apply_percentage_promotion(
            tenant_id,
            cart.id,
            Some(line_item_id),
            "promo-line-10",
            Decimal::from_str("10").unwrap(),
            serde_json::json!({
                "display_label": "Line promo"
            }),
        )
        .await
        .unwrap();

    assert_eq!(updated.subtotal_amount, Decimal::from_str("40.00").unwrap());
    assert_eq!(updated.adjustment_total, Decimal::from_str("6.70").unwrap());
    assert_eq!(updated.total_amount, Decimal::from_str("33.30").unwrap());
    assert_eq!(updated.adjustments.len(), 2);
    let promotion = updated
        .adjustments
        .iter()
        .find(|item| item.source_id.as_deref() == Some("promo-line-10"))
        .expect("promotion adjustment should be present");
    assert_eq!(promotion.source_type, "promotion");
    assert_eq!(promotion.line_item_id, Some(line_item_id));
    assert_eq!(promotion.amount, Decimal::from_str("3.70").unwrap());
    assert_eq!(
        promotion.metadata["kind"],
        serde_json::json!("percentage_discount")
    );
    assert_eq!(promotion.metadata["scope"], serde_json::json!("line_item"));
    assert!(promotion.metadata.get("display_label").is_none());
}

#[tokio::test]
async fn apply_fixed_cart_promotion_replaces_same_source_id_instead_of_stacking_duplicate() {
    let service = setup().await;
    let tenant_id = support::TEST_TENANT_ID;

    let cart = service
        .create_cart(tenant_id, create_cart_input())
        .await
        .unwrap();
    let cart = service
        .add_line_item(tenant_id, cart.id, line_item_input())
        .await
        .unwrap();

    let cart = service
        .apply_fixed_promotion(
            tenant_id,
            cart.id,
            None,
            "promo-cart",
            Decimal::from_str("5.00").unwrap(),
            serde_json::json!({}),
        )
        .await
        .unwrap();
    let updated = service
        .apply_fixed_promotion(
            tenant_id,
            cart.id,
            None,
            "promo-cart",
            Decimal::from_str("7.00").unwrap(),
            serde_json::json!({}),
        )
        .await
        .unwrap();

    assert_eq!(cart.adjustments.len(), 1);
    assert_eq!(updated.adjustments.len(), 1);
    assert_eq!(updated.adjustments[0].source_type, "promotion");
    assert_eq!(
        updated.adjustments[0].amount,
        Decimal::from_str("7.00").unwrap()
    );
    assert_eq!(updated.adjustment_total, Decimal::from_str("7.00").unwrap());
    assert_eq!(updated.total_amount, Decimal::from_str("24.00").unwrap());
}

#[tokio::test]
async fn create_cart_persists_multilingual_context_snapshot() {
    let service = setup().await;
    let tenant_id = support::TEST_TENANT_ID;
    let region_id = Uuid::new_v4();
    let shipping_option_id = Uuid::new_v4();

    let cart = service
        .create_cart(
            tenant_id,
            CreateCartInput {
                customer_id: Some(Uuid::new_v4()),
                email: Some("buyer@example.com".to_string()),
                region_id: Some(region_id),
                country_code: Some("de".to_string()),
                locale_code: Some("pt_BR".to_string()),
                selected_shipping_option_id: Some(shipping_option_id),
                currency_code: "eur".to_string(),
                metadata: serde_json::json!({ "source": "locale-test" }),
            },
        )
        .await
        .unwrap();

    assert_eq!(cart.region_id, Some(region_id));
    assert_eq!(cart.country_code.as_deref(), Some("DE"));
    assert_eq!(cart.locale_code.as_deref(), Some("pt-br"));
    assert_eq!(cart.selected_shipping_option_id, Some(shipping_option_id));
    assert_eq!(cart.currency_code, "EUR");
}

#[tokio::test]
async fn create_cart_with_channel_persists_channel_snapshot() {
    let service = setup().await;
    let tenant_id = support::TEST_TENANT_ID;
    let channel_id = Uuid::new_v4();

    let cart = service
        .create_cart_with_channel(
            tenant_id,
            create_cart_input(),
            Some(channel_id),
            Some("web-store".to_string()),
        )
        .await
        .unwrap();

    assert_eq!(cart.channel_id, Some(channel_id));
    assert_eq!(cart.channel_slug.as_deref(), Some("web-store"));
}

#[tokio::test]
async fn channel_tax_provider_mapping_overrides_region_provider() {
    let (db, service) = setup_with_db().await;
    let tenant_id = support::TEST_TENANT_ID;
    let region_id = Uuid::new_v4();
    let channel_id = Uuid::new_v4();

    insert_region(
        &db,
        tenant_id,
        region_id,
        "usd",
        Some("region_default"),
        serde_json::json!({
            "channel_tax_provider_ids": {
                channel_id.to_string(): "external_tax"
            }
        }),
    )
    .await;

    let cart = service
        .create_cart_with_channel(
            tenant_id,
            CreateCartInput {
                region_id: Some(region_id),
                ..create_cart_input()
            },
            Some(channel_id),
            Some("web".to_string()),
        )
        .await
        .unwrap();

    let error = service
        .add_line_item(tenant_id, cart.id, line_item_input())
        .await
        .expect_err("unknown channel provider should fail");

    match error {
        CartError::Tax(rustok_tax::TaxError::Validation(message)) => {
            assert!(message.contains("unknown tax provider_id: external_tax"));
        }
        other => panic!("expected tax validation error, got {other:?}"),
    }
}

#[tokio::test]
async fn channel_mapping_is_ignored_without_cart_channel_context() {
    let (db, service) = setup_with_db().await;
    let tenant_id = support::TEST_TENANT_ID;
    let region_id = Uuid::new_v4();
    let mapped_channel_id = Uuid::new_v4();

    insert_region(
        &db,
        tenant_id,
        region_id,
        "usd",
        Some("region_default"),
        serde_json::json!({
            "channel_tax_provider_ids": {
                mapped_channel_id.to_string(): "external_tax"
            }
        }),
    )
    .await;

    let cart = service
        .create_cart(
            tenant_id,
            CreateCartInput {
                region_id: Some(region_id),
                ..create_cart_input()
            },
        )
        .await
        .unwrap();

    let updated = service
        .add_line_item(tenant_id, cart.id, line_item_input())
        .await
        .expect("channel mapping should not apply without cart channel context");

    assert_eq!(updated.tax_lines.len(), 1);
    assert_eq!(updated.tax_lines[0].provider_id, "region_default");
}

#[tokio::test]
async fn channel_tax_provider_mapping_is_normalized_and_overrides_region_provider() {
    let (db, service) = setup_with_db().await;
    let tenant_id = support::TEST_TENANT_ID;
    let region_id = Uuid::new_v4();
    let channel_id = Uuid::new_v4();

    insert_region(
        &db,
        tenant_id,
        region_id,
        "usd",
        Some("external_tax"),
        serde_json::json!({
            "channel_tax_provider_ids": {
                channel_id.to_string(): "  REGION_DEFAULT  "
            }
        }),
    )
    .await;

    let cart = service
        .create_cart_with_channel(
            tenant_id,
            CreateCartInput {
                region_id: Some(region_id),
                ..create_cart_input()
            },
            Some(channel_id),
            Some("web".to_string()),
        )
        .await
        .unwrap();

    let updated = service
        .add_line_item(tenant_id, cart.id, line_item_input())
        .await
        .expect("normalized channel mapping should override invalid region provider");

    assert_eq!(updated.tax_lines.len(), 1);
    assert_eq!(updated.tax_lines[0].provider_id, "region_default");
}

#[tokio::test]
async fn object_channel_tax_provider_mapping_uses_provider_id_successfully() {
    let (db, service) = setup_with_db().await;
    let tenant_id = support::TEST_TENANT_ID;
    let region_id = Uuid::new_v4();
    let channel_id = Uuid::new_v4();

    insert_region(
        &db,
        tenant_id,
        region_id,
        "usd",
        Some("external_tax"),
        serde_json::json!({
            "channel_tax_provider_ids": {
                channel_id.to_string(): {"provider_id": "region_default"}
            }
        }),
    )
    .await;

    let cart = service
        .create_cart_with_channel(
            tenant_id,
            CreateCartInput {
                region_id: Some(region_id),
                ..create_cart_input()
            },
            Some(channel_id),
            Some("web".to_string()),
        )
        .await
        .unwrap();

    let updated = service
        .add_line_item(tenant_id, cart.id, line_item_input())
        .await
        .expect("provider_id object mapping should override invalid region provider");

    assert_eq!(updated.tax_lines.len(), 1);
    assert_eq!(updated.tax_lines[0].provider_id, "region_default");
}

#[tokio::test]
async fn object_channel_tax_provider_mapping_uses_provider_key_alias() {
    let (db, service) = setup_with_db().await;
    let tenant_id = support::TEST_TENANT_ID;
    let region_id = Uuid::new_v4();
    let channel_id = Uuid::new_v4();

    insert_region(
        &db,
        tenant_id,
        region_id,
        "usd",
        Some("region_default"),
        serde_json::json!({
            "channel_tax_provider_ids": {
                channel_id.to_string(): {"provider": "external_tax"}
            }
        }),
    )
    .await;

    let cart = service
        .create_cart_with_channel(
            tenant_id,
            CreateCartInput {
                region_id: Some(region_id),
                ..create_cart_input()
            },
            Some(channel_id),
            Some("web".to_string()),
        )
        .await
        .unwrap();

    let updated = service
        .add_line_item(tenant_id, cart.id, line_item_input())
        .await
        .expect_err("unknown provider from object mapping should be validated");

    match updated {
        CartError::Tax(rustok_tax::TaxError::Validation(message)) => {
            assert!(message.contains("unknown tax provider_id: external_tax"));
        }
        other => panic!("expected tax validation error, got {other:?}"),
    }
}

#[tokio::test]
async fn channel_tax_provider_alias_is_ignored_without_channel_context() {
    let (db, service) = setup_with_db().await;
    let tenant_id = support::TEST_TENANT_ID;
    let region_id = Uuid::new_v4();
    let mapped_channel_id = Uuid::new_v4();

    insert_region(
        &db,
        tenant_id,
        region_id,
        "usd",
        Some("region_default"),
        serde_json::json!({
            "channel_tax_provider_ids": {
                mapped_channel_id.to_string(): {"provider": "external_tax"}
            }
        }),
    )
    .await;

    let cart = service
        .create_cart(
            tenant_id,
            CreateCartInput {
                region_id: Some(region_id),
                ..create_cart_input()
            },
        )
        .await
        .unwrap();

    let updated = service
        .add_line_item(tenant_id, cart.id, line_item_input())
        .await
        .expect("alias mapping should be ignored without channel context");

    assert_eq!(updated.tax_lines.len(), 1);
    assert_eq!(updated.tax_lines[0].provider_id, "region_default");
    assert_eq!(updated.tax_lines[0].metadata["channel_id"], serde_json::json!(null));
}

#[tokio::test]
async fn channel_tax_provider_alias_is_normalized_and_snapshots_channel_id() {
    let (db, service) = setup_with_db().await;
    let tenant_id = support::TEST_TENANT_ID;
    let region_id = Uuid::new_v4();
    let channel_id = Uuid::new_v4();

    insert_region(
        &db,
        tenant_id,
        region_id,
        "usd",
        Some("external_tax"),
        serde_json::json!({
            "channel_tax_provider_ids": {
                channel_id.to_string(): {"provider": "  REGION_DEFAULT  "}
            }
        }),
    )
    .await;

    let cart = service
        .create_cart_with_channel(
            tenant_id,
            CreateCartInput {
                region_id: Some(region_id),
                ..create_cart_input()
            },
            Some(channel_id),
            Some("web".to_string()),
        )
        .await
        .unwrap();

    let updated = service
        .add_line_item(tenant_id, cart.id, line_item_input())
        .await
        .expect("alias mapping should normalize and override invalid region provider");

    assert_eq!(updated.tax_lines.len(), 1);
    assert_eq!(updated.tax_lines[0].provider_id, "region_default");
    assert_eq!(
        updated.tax_lines[0].metadata["channel_id"],
        serde_json::json!(channel_id.to_string())
    );
}

#[tokio::test]
async fn channel_tax_provider_mapping_with_invalid_chars_is_rejected() {
    let (db, service) = setup_with_db().await;
    let tenant_id = support::TEST_TENANT_ID;
    let region_id = Uuid::new_v4();
    let channel_id = Uuid::new_v4();

    insert_region(
        &db,
        tenant_id,
        region_id,
        "usd",
        Some("region_default"),
        serde_json::json!({
            "channel_tax_provider_ids": {
                channel_id.to_string(): "INVALID PROVIDER"
            }
        }),
    )
    .await;

    let cart = service
        .create_cart_with_channel(
            tenant_id,
            CreateCartInput {
                region_id: Some(region_id),
                ..create_cart_input()
            },
            Some(channel_id),
            Some("web".to_string()),
        )
        .await
        .unwrap();

    let error = service
        .add_line_item(tenant_id, cart.id, line_item_input())
        .await
        .expect_err("invalid provider id should be rejected");

    match error {
        CartError::Tax(rustok_tax::TaxError::Validation(message)) => {
            assert!(message.contains("tax provider_id must use lowercase ASCII"));
        }
        other => panic!("expected tax validation error, got {other:?}"),
    }
}

#[tokio::test]
async fn blank_channel_tax_provider_mapping_falls_back_to_region_provider() {
    let (db, service) = setup_with_db().await;
    let tenant_id = support::TEST_TENANT_ID;
    let region_id = Uuid::new_v4();
    let channel_id = Uuid::new_v4();

    insert_region(
        &db,
        tenant_id,
        region_id,
        "usd",
        Some("region_default"),
        serde_json::json!({
            "channel_tax_provider_ids": {
                channel_id.to_string(): "   "
            }
        }),
    )
    .await;

    let cart = service
        .create_cart_with_channel(
            tenant_id,
            CreateCartInput {
                region_id: Some(region_id),
                ..create_cart_input()
            },
            Some(channel_id),
            Some("web".to_string()),
        )
        .await
        .unwrap();

    let updated = service
        .add_line_item(tenant_id, cart.id, line_item_input())
        .await
        .expect("blank channel mapping should use region provider");

    assert_eq!(updated.tax_lines.len(), 1);
    assert_eq!(updated.tax_lines[0].provider_id, "region_default");
}

#[tokio::test]
async fn update_cart_context_rewrites_snapshot_fields() {
    let service = setup().await;
    let tenant_id = support::TEST_TENANT_ID;
    let initial_region_id = Uuid::new_v4();
    let updated_region_id = Uuid::new_v4();
    let initial_shipping_option_id = Uuid::new_v4();
    let updated_shipping_option_id = Uuid::new_v4();

    let cart = service
        .create_cart(
            tenant_id,
            CreateCartInput {
                customer_id: Some(Uuid::new_v4()),
                email: Some("buyer@example.com".to_string()),
                region_id: Some(initial_region_id),
                country_code: Some("de".to_string()),
                locale_code: Some("de".to_string()),
                selected_shipping_option_id: Some(initial_shipping_option_id),
                currency_code: "eur".to_string(),
                metadata: serde_json::json!({ "source": "update-context-test" }),
            },
        )
        .await
        .unwrap();

    let updated = service
        .update_context(
            tenant_id,
            cart.id,
            UpdateCartContextInput {
                email: Some("checkout@example.com".to_string()),
                region_id: Some(updated_region_id),
                country_code: Some("pl".to_string()),
                locale_code: Some("pl_PL".to_string()),
                selected_shipping_option_id: Some(updated_shipping_option_id),
                shipping_selections: None,
            },
        )
        .await
        .unwrap();

    assert_eq!(updated.email.as_deref(), Some("checkout@example.com"));
    assert_eq!(updated.region_id, Some(updated_region_id));
    assert_eq!(updated.country_code.as_deref(), Some("PL"));
    assert_eq!(updated.locale_code.as_deref(), Some("pl-pl"));
    assert_eq!(
        updated.selected_shipping_option_id,
        Some(updated_shipping_option_id)
    );
}

#[tokio::test]
async fn update_and_remove_line_items_recalculate_cart() {
    let service = setup().await;
    let tenant_id = support::TEST_TENANT_ID;

    let cart = service
        .create_cart(tenant_id, create_cart_input())
        .await
        .unwrap();
    let cart = service
        .add_line_item(tenant_id, cart.id, line_item_input())
        .await
        .unwrap();
    let line_item_id = cart.line_items[0].id;

    let cart = service
        .update_line_item_quantity(tenant_id, cart.id, line_item_id, 3)
        .await
        .unwrap();
    assert_eq!(cart.total_amount, Decimal::from_str("46.50").unwrap());

    let cart = service
        .remove_line_item(tenant_id, cart.id, line_item_id)
        .await
        .unwrap();
    assert_eq!(cart.line_items.len(), 0);
    assert_eq!(cart.total_amount, Decimal::ZERO);
}

#[tokio::test]
async fn selected_shipping_option_contributes_shipping_total_to_cart_totals() {
    let (db, service) = setup_with_db().await;
    let tenant_id = support::TEST_TENANT_ID;
    let shipping_option_id = Uuid::new_v4();
    insert_shipping_option(
        &db,
        tenant_id,
        shipping_option_id,
        "usd",
        Decimal::from_str("9.99").unwrap(),
    )
    .await;

    let cart = service
        .create_cart(
            tenant_id,
            CreateCartInput {
                selected_shipping_option_id: Some(shipping_option_id),
                ..create_cart_input()
            },
        )
        .await
        .unwrap();
    let updated = service
        .add_line_item(tenant_id, cart.id, line_item_input())
        .await
        .unwrap();

    assert_eq!(updated.subtotal_amount, Decimal::from_str("31.00").unwrap());
    assert_eq!(updated.adjustment_total, Decimal::ZERO);
    assert_eq!(updated.shipping_total, Decimal::from_str("9.99").unwrap());
    assert_eq!(updated.total_amount, Decimal::from_str("40.99").unwrap());
}

#[tokio::test]
async fn apply_percentage_shipping_promotion_uses_shipping_total_as_base() {
    let (db, service) = setup_with_db().await;
    let tenant_id = support::TEST_TENANT_ID;
    let shipping_option_id = Uuid::new_v4();
    insert_shipping_option(
        &db,
        tenant_id,
        shipping_option_id,
        "usd",
        Decimal::from_str("9.99").unwrap(),
    )
    .await;

    let cart = service
        .create_cart(
            tenant_id,
            CreateCartInput {
                selected_shipping_option_id: Some(shipping_option_id),
                ..create_cart_input()
            },
        )
        .await
        .unwrap();
    let cart = service
        .add_line_item(tenant_id, cart.id, line_item_input())
        .await
        .unwrap();

    let preview = service
        .preview_percentage_shipping_promotion(
            tenant_id,
            cart.id,
            "promo-shipping-10",
            Decimal::from_str("10").unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(preview.base_amount, Decimal::from_str("9.99").unwrap());
    assert_eq!(
        preview.adjustment_amount,
        Decimal::from_str("1.00").unwrap()
    );
    assert_eq!(preview.adjusted_amount, Decimal::from_str("8.99").unwrap());

    let updated = service
        .apply_percentage_shipping_promotion(
            tenant_id,
            cart.id,
            "promo-shipping-10",
            Decimal::from_str("10").unwrap(),
            serde_json::json!({
                "display_label": "Ship promo"
            }),
        )
        .await
        .unwrap();

    assert_eq!(updated.subtotal_amount, Decimal::from_str("31.00").unwrap());
    assert_eq!(updated.shipping_total, Decimal::from_str("9.99").unwrap());
    assert_eq!(updated.adjustment_total, Decimal::from_str("1.00").unwrap());
    assert_eq!(updated.total_amount, Decimal::from_str("39.99").unwrap());
    let promotion = updated
        .adjustments
        .iter()
        .find(|item| item.source_id.as_deref() == Some("promo-shipping-10"))
        .expect("shipping promotion should be present");
    assert_eq!(promotion.source_type, "promotion");
    assert_eq!(promotion.line_item_id, None);
    assert_eq!(promotion.metadata["scope"], serde_json::json!("shipping"));
    assert!(promotion.metadata.get("display_label").is_none());
}

#[tokio::test]
async fn completed_cart_rejects_mutations() {
    let service = setup().await;
    let tenant_id = support::TEST_TENANT_ID;

    let cart = service
        .create_cart(tenant_id, create_cart_input())
        .await
        .unwrap();
    let cart = service
        .add_line_item(tenant_id, cart.id, line_item_input())
        .await
        .unwrap();
    let cart = service.complete_cart(tenant_id, cart.id).await.unwrap();
    assert_eq!(cart.status, "completed");

    let error = service
        .add_line_item(tenant_id, cart.id, line_item_input())
        .await
        .unwrap_err();
    match error {
        CartError::InvalidTransition { from, .. } => assert_eq!(from, "completed"),
        other => panic!("expected invalid transition, got {other:?}"),
    }
}

#[tokio::test]
async fn checkout_lifecycle_uses_checking_out_before_completion() {
    let service = setup().await;
    let tenant_id = support::TEST_TENANT_ID;

    let cart = service
        .create_cart(tenant_id, create_cart_input())
        .await
        .unwrap();
    let checking_out = service.begin_checkout(tenant_id, cart.id).await.unwrap();
    assert_eq!(checking_out.status, "checking_out");

    let error = service
        .add_line_item(tenant_id, cart.id, line_item_input())
        .await
        .unwrap_err();
    match error {
        CartError::InvalidTransition { from, .. } => assert_eq!(from, "checking_out"),
        other => panic!("expected invalid transition, got {other:?}"),
    }

    let reopened = service.release_checkout(tenant_id, cart.id).await.unwrap();
    assert_eq!(reopened.status, "active");

    let checking_out = service.begin_checkout(tenant_id, cart.id).await.unwrap();
    let completed = service
        .complete_cart(tenant_id, checking_out.id)
        .await
        .unwrap();
    assert_eq!(completed.status, "completed");
}

#[tokio::test]
async fn seller_aware_delivery_groups_split_same_shipping_profile() {
    let service = setup().await;
    let tenant_id = support::TEST_TENANT_ID;
    let seller_a_option_id = Uuid::new_v4();
    let seller_b_option_id = Uuid::new_v4();
    let seller_a_id = "seller-a-id";
    let seller_b_id = "seller-b-id";

    let cart = service
        .create_cart(tenant_id, create_cart_input())
        .await
        .unwrap();
    let cart = service
        .add_line_item(
            tenant_id,
            cart.id,
            AddCartLineItemInput {
                metadata: serde_json::json!({
                    "seller": {
                        "id": seller_a_id,
                        "scope": "seller-a"
                    }
                }),
                ..line_item_input()
            },
        )
        .await
        .unwrap();
    let cart = service
        .add_line_item(
            tenant_id,
            cart.id,
            AddCartLineItemInput {
                sku: Some("SKU-CART-2".to_string()),
                metadata: serde_json::json!({
                    "seller": {
                        "id": seller_b_id,
                        "scope": "seller-b"
                    }
                }),
                ..line_item_input()
            },
        )
        .await
        .unwrap();

    assert_eq!(cart.delivery_groups.len(), 2);
    assert!(cart
        .delivery_groups
        .iter()
        .all(|group| group.shipping_profile_slug == "default"));

    let seller_ids = cart
        .line_items
        .iter()
        .map(|item| item.seller_id.clone().expect("seller id should be present"))
        .collect::<Vec<_>>();
    assert!(cart.line_items.iter().all(|item| {
        item.metadata
            .get("seller")
            .and_then(|seller| seller.get("label"))
            .is_none()
            && item.metadata.get("seller_label").is_none()
    }));
    assert!(seller_ids.contains(&seller_a_id.to_string()));
    assert!(seller_ids.contains(&seller_b_id.to_string()));

    let updated = service
        .update_context(
            tenant_id,
            cart.id,
            UpdateCartContextInput {
                email: cart.email.clone(),
                region_id: cart.region_id,
                country_code: cart.country_code.clone(),
                locale_code: cart.locale_code.clone(),
                selected_shipping_option_id: None,
                shipping_selections: Some(vec![
                    CartShippingSelectionInput {
                        shipping_profile_slug: "default".to_string(),
                        seller_id: Some(seller_a_id.to_string()),
                        seller_scope: None,
                        selected_shipping_option_id: Some(seller_a_option_id),
                    },
                    CartShippingSelectionInput {
                        shipping_profile_slug: "default".to_string(),
                        seller_id: Some(seller_b_id.to_string()),
                        seller_scope: None,
                        selected_shipping_option_id: Some(seller_b_option_id),
                    },
                ]),
            },
        )
        .await
        .unwrap();

    let delivery_groups = updated
        .delivery_groups
        .iter()
        .map(|group| {
            (
                group.shipping_profile_slug.clone(),
                group
                    .seller_id
                    .clone()
                    .expect("seller id should be present"),
                group
                    .seller_scope
                    .clone()
                    .expect("seller scope should be present"),
                group.selected_shipping_option_id,
            )
        })
        .collect::<Vec<_>>();
    assert!(delivery_groups.contains(&(
        String::from("default"),
        seller_a_id.to_string(),
        String::from("seller-a"),
        Some(seller_a_option_id),
    )));
    assert!(delivery_groups.contains(&(
        String::from("default"),
        seller_b_id.to_string(),
        String::from("seller-b"),
        Some(seller_b_option_id),
    )));
}

#[tokio::test]
async fn legacy_seller_scope_still_splits_delivery_groups_without_seller_id() {
    let service = setup().await;
    let tenant_id = support::TEST_TENANT_ID;

    let cart = service
        .create_cart(tenant_id, create_cart_input())
        .await
        .unwrap();
    let cart = service
        .add_line_item(
            tenant_id,
            cart.id,
            AddCartLineItemInput {
                metadata: serde_json::json!({
                    "seller": {
                        "scope": "legacy-seller-a"
                    }
                }),
                ..line_item_input()
            },
        )
        .await
        .unwrap();
    let cart = service
        .add_line_item(
            tenant_id,
            cart.id,
            AddCartLineItemInput {
                sku: Some("SKU-CART-LEGACY-2".to_string()),
                metadata: serde_json::json!({
                    "seller": {
                        "scope": "legacy-seller-b"
                    }
                }),
                ..line_item_input()
            },
        )
        .await
        .unwrap();

    assert_eq!(cart.delivery_groups.len(), 2);
    assert!(cart
        .delivery_groups
        .iter()
        .all(|group| group.seller_id.is_none()));
    assert!(cart
        .delivery_groups
        .iter()
        .any(|group| { group.seller_scope.as_deref() == Some("legacy-seller-a") }));
    assert!(cart
        .delivery_groups
        .iter()
        .any(|group| { group.seller_scope.as_deref() == Some("legacy-seller-b") }));
}
