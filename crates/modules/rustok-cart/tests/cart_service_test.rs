use chrono::Utc;
use rust_decimal::Decimal;
use rustok_cart::dto::{
    AddCartLineItemInput, CartLineFulfillmentRequirement, CartShippingSelectionInput,
    CreateCartInput, SetCartAdjustmentInput, UpdateCartContextInput,
};
use rustok_cart::error::CartError;
use rustok_cart::services::{CartService, cart::CartPricingAdjustmentUpdate};
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
        fulfillment_requirement: CartLineFulfillmentRequirement::Physical,
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
                quantity: cart.line_items[0].quantity,
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
            rustok_cart::CartPricingContextGuard::capture(&cart).unwrap(),
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
    assert!(
        updated.adjustments[0]
            .metadata
            .get("display_label")
            .is_none()
    );
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
    assert!(
        updated.adjustments[0]
            .metadata
            .get("display_label")
            .is_none()
    );
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
        CartError::TaxBoundary { code, message, .. } => {
            assert_eq!(code, "tax.validation");
            assert!(
                message.contains("tax request is invalid")
                    || message.contains("tax calculation request is invalid")
                    || message.contains("unknown tax provider_id: external_tax")
            );
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
        CartError::TaxBoundary { code, message, .. } => {
            assert_eq!(code, "tax.validation");
            assert!(
                message.contains("tax request is invalid")
                    || message.contains("tax calculation request is invalid")
                    || message.contains("unknown tax provider_id: external_tax")
            );
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
    assert_eq!(
        updated.tax_lines[0].metadata["channel_id"],
        serde_json::json!(null)
    );
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
        CartError::TaxBoundary { code, message, .. } => {
            assert_eq!(code, "tax.validation");
            assert!(
                message.contains("tax request is invalid")
                    || message.contains("tax calculation request is invalid")
                    || message.contains("tax provider_id must use lowercase ASCII")
            );
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
    let (db, service) = setup_with_db().await;
    let tenant_id = support::TEST_TENANT_ID;
    let initial_region_id = Uuid::new_v4();
    let updated_region_id = Uuid::new_v4();
    let initial_shipping_option_id = Uuid::new_v4();
    let updated_shipping_option_id = Uuid::new_v4();

    insert_region(
        &db,
        tenant_id,
        initial_region_id,
        "eur",
        None,
        serde_json::json!({}),
    )
    .await;
    insert_region(
        &db,
        tenant_id,
        updated_region_id,
        "eur",
        None,
        serde_json::json!({}),
    )
    .await;
    insert_shipping_option(
        &db,
        tenant_id,
        initial_shipping_option_id,
        "eur",
        Decimal::from_str("5.00").unwrap(),
    )
    .await;
    insert_shipping_option(
        &db,
        tenant_id,
        updated_shipping_option_id,
        "eur",
        Decimal::from_str("7.00").unwrap(),
    )
    .await;

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
            rustok_cart::CartContextUpdateRequest {
                cart_id: cart.id,
                pricing_context: rustok_cart::CartPricingContextGuard::capture(&cart).unwrap(),
                pricing: rustok_cart::CartContextPricing::Preserve,
                input: UpdateCartContextInput {
                    email: Some("checkout@example.com".to_string()),
                    region_id: Some(updated_region_id),
                    country_code: Some("pl".to_string()),
                    locale_code: Some("pl_PL".to_string()),
                    selected_shipping_option_id: Some(updated_shipping_option_id),
                    shipping_selections: None,
                },
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
        .update_line_item_pricing(
            tenant_id,
            cart.id,
            rustok_cart::CartLineItemPricingUpdate {
                line_item_id,
                quantity: 3,
                unit_price: cart.line_items[0].unit_price,
                pricing_adjustment: None,
            },
            rustok_cart::CartPricingContextGuard::capture(&cart).unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(cart.total_amount, Decimal::from_str("46.50").unwrap());

    let cart = service
        .remove_line_item(
            tenant_id,
            cart.id,
            line_item_id,
            rustok_cart::CartPricingContextGuard::capture_removal(&cart, line_item_id).unwrap(),
        )
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
async fn completed_cart_rejects_typed_promotion_mutations() {
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
    let completed = service.complete_cart(tenant_id, cart.id).await.unwrap();
    assert_eq!(completed.status, "completed");

    let err = service
        .apply_percentage_promotion(
            tenant_id,
            cart.id,
            None,
            "promo-on-completed",
            Decimal::from_str("10.00").unwrap(),
            serde_json::json!({"source": "completed-guard-test"}),
        )
        .await
        .unwrap_err();

    match err {
        CartError::InvalidTransition { from, .. } => assert_eq!(from, "completed"),
        other => panic!("expected invalid transition, got {other:?}"),
    }
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
async fn release_checkout_does_not_set_completed_at_and_complete_cart_does() {
    let service = setup().await;
    let tenant_id = support::TEST_TENANT_ID;

    let cart = service
        .create_cart(tenant_id, create_cart_input())
        .await
        .unwrap();
    assert!(cart.completed_at.is_none());

    let checking_out = service.begin_checkout(tenant_id, cart.id).await.unwrap();
    assert_eq!(checking_out.status, "checking_out");
    assert!(checking_out.completed_at.is_none());

    let reopened = service.release_checkout(tenant_id, cart.id).await.unwrap();
    assert_eq!(reopened.status, "active");
    assert!(reopened.completed_at.is_none());

    let completed = service.complete_cart(tenant_id, cart.id).await.unwrap();
    assert_eq!(completed.status, "completed");
    assert!(completed.completed_at.is_some());
}

#[tokio::test]
async fn completed_checkout_rejects_release_and_reentry() {
    let service = setup().await;
    let tenant_id = support::TEST_TENANT_ID;

    let cart = service
        .create_cart(tenant_id, create_cart_input())
        .await
        .unwrap();

    let checking_out = service.begin_checkout(tenant_id, cart.id).await.unwrap();
    assert_eq!(checking_out.status, "checking_out");

    let completed = service.complete_cart(tenant_id, cart.id).await.unwrap();
    assert_eq!(completed.status, "completed");

    let err = service
        .release_checkout(tenant_id, cart.id)
        .await
        .unwrap_err();
    match err {
        CartError::InvalidTransition { from, to } => {
            assert_eq!(from, "completed");
            assert_eq!(to, "active");
        }
        other => panic!("expected invalid transition, got {other:?}"),
    }

    let err = service
        .begin_checkout(tenant_id, cart.id)
        .await
        .unwrap_err();
    match err {
        CartError::InvalidTransition { from, to } => {
            assert_eq!(from, "completed");
            assert_eq!(to, "checking_out");
        }
        other => panic!("expected invalid transition, got {other:?}"),
    }
}

#[tokio::test]
async fn checkout_transition_guards_reject_invalid_reentry_and_release() {
    let service = setup().await;
    let tenant_id = support::TEST_TENANT_ID;

    let cart = service
        .create_cart(tenant_id, create_cart_input())
        .await
        .unwrap();

    let err = service
        .release_checkout(tenant_id, cart.id)
        .await
        .unwrap_err();
    match err {
        CartError::InvalidTransition { from, to } => {
            assert_eq!(from, "active");
            assert_eq!(to, "active");
        }
        other => panic!("expected invalid transition, got {other:?}"),
    }

    let checking_out = service.begin_checkout(tenant_id, cart.id).await.unwrap();
    assert_eq!(checking_out.status, "checking_out");

    let err = service
        .begin_checkout(tenant_id, cart.id)
        .await
        .unwrap_err();
    match err {
        CartError::InvalidTransition { from, to } => {
            assert_eq!(from, "checking_out");
            assert_eq!(to, "checking_out");
        }
        other => panic!("expected invalid transition, got {other:?}"),
    }
}

#[tokio::test]
async fn release_checkout_restores_mutation_paths() {
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

    let checking_out = service.begin_checkout(tenant_id, cart.id).await.unwrap();
    assert_eq!(checking_out.status, "checking_out");

    let reopened = service.release_checkout(tenant_id, cart.id).await.unwrap();
    assert_eq!(reopened.status, "active");

    let updated = service
        .apply_percentage_promotion(
            tenant_id,
            cart.id,
            None,
            "promo-after-release",
            Decimal::from_str("10.00").unwrap(),
            serde_json::json!({ "source": "release-checkout-test" }),
        )
        .await
        .unwrap();

    assert_eq!(updated.status, "active");
    assert!(
        updated
            .adjustments
            .iter()
            .any(|item| item.source_id.as_deref() == Some("promo-after-release"))
    );
}

#[tokio::test]
async fn checking_out_cart_rejects_typed_promotion_mutations() {
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

    let checking_out = service.begin_checkout(tenant_id, cart.id).await.unwrap();
    assert_eq!(checking_out.status, "checking_out");

    let error = service
        .apply_percentage_promotion(
            tenant_id,
            cart.id,
            None,
            "promo-typed-while-checkout",
            Decimal::from_str("10.00").unwrap(),
            serde_json::json!({ "source": "typed-checkout-guard" }),
        )
        .await
        .unwrap_err();

    match error {
        CartError::InvalidTransition { from, .. } => assert_eq!(from, "checking_out"),
        other => panic!("expected invalid transition, got {other:?}"),
    }
}

#[tokio::test]
async fn checking_out_cart_rejects_adjustment_mutations() {
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

    let checking_out = service.begin_checkout(tenant_id, cart.id).await.unwrap();
    assert_eq!(checking_out.status, "checking_out");

    let error = service
        .set_adjustments(
            tenant_id,
            cart.id,
            vec![SetCartAdjustmentInput {
                line_item_id: None,
                source_type: "Promotion".to_string(),
                source_id: Some("promo-while-checkout".to_string()),
                amount: Decimal::from_str("1.00").unwrap(),
                metadata: serde_json::json!({ "rule_code": "forbidden" }),
            }],
        )
        .await
        .unwrap_err();

    match error {
        CartError::InvalidTransition { from, .. } => assert_eq!(from, "checking_out"),
        other => panic!("expected invalid transition, got {other:?}"),
    }
}

#[tokio::test]
async fn seller_aware_delivery_groups_split_same_shipping_profile() {
    let (db, service) = setup_with_db().await;
    let tenant_id = support::TEST_TENANT_ID;
    let seller_a_option_id = Uuid::new_v4();
    let seller_b_option_id = Uuid::new_v4();
    let seller_a_id = "seller-a-id";
    let seller_b_id = "seller-b-id";

    insert_shipping_option(
        &db,
        tenant_id,
        seller_a_option_id,
        "USD",
        Decimal::from_str("10.00").unwrap(),
    )
    .await;
    insert_shipping_option(
        &db,
        tenant_id,
        seller_b_option_id,
        "USD",
        Decimal::from_str("10.00").unwrap(),
    )
    .await;

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
    assert!(
        cart.delivery_groups
            .iter()
            .all(|group| group.shipping_profile_slug == "default")
    );

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
            rustok_cart::CartContextUpdateRequest {
                cart_id: cart.id,
                pricing_context: rustok_cart::CartPricingContextGuard::capture(&cart).unwrap(),
                pricing: rustok_cart::CartContextPricing::Preserve,
                input: UpdateCartContextInput {
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
                group.seller_scope.clone(),
                group.selected_shipping_option_id,
            )
        })
        .collect::<Vec<_>>();
    assert!(delivery_groups.contains(&(
        String::from("default"),
        seller_a_id.to_string(),
        None,
        Some(seller_a_option_id),
    )));
    assert!(delivery_groups.contains(&(
        String::from("default"),
        seller_b_id.to_string(),
        None,
        Some(seller_b_option_id),
    )));
}

#[tokio::test]
async fn seller_scope_without_seller_id_does_not_split_delivery_groups() {
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
                sku: Some("SKU-CART-LEGACY-2".to_string()),
                metadata: serde_json::json!({
                    "seller": {
                        "scope": "seller-b"
                    }
                }),
                ..line_item_input()
            },
        )
        .await
        .unwrap();

    assert_eq!(cart.delivery_groups.len(), 1);
    assert!(
        cart.delivery_groups
            .iter()
            .all(|group| group.seller_id.is_none())
    );
    assert!(
        cart.delivery_groups
            .iter()
            .all(|group| group.seller_scope.is_none())
    );
}

#[tokio::test]
async fn tax_calculated_on_net_discounted_line_item_price() {
    let (db, service) = setup_with_db().await;
    let tenant_id = support::TEST_TENANT_ID;
    let region_id = Uuid::new_v4();

    insert_region(
        &db,
        tenant_id,
        region_id,
        "usd",
        Some("region_default"),
        serde_json::json!({}),
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

    let cart = service
        .add_line_item(
            tenant_id,
            cart.id,
            AddCartLineItemInput {
                unit_price: Decimal::from_str("100.00").unwrap(),
                quantity: 1,
                ..line_item_input()
            },
        )
        .await
        .unwrap();

    let line_item_id = cart.line_items[0].id;
    assert_eq!(cart.tax_total, Decimal::from_str("10.00").unwrap());

    let updated = service
        .set_adjustments(
            tenant_id,
            cart.id,
            vec![SetCartAdjustmentInput {
                line_item_id: Some(line_item_id),
                source_type: "promotion".to_string(),
                source_id: Some("promo-20".to_string()),
                amount: Decimal::from_str("20.00").unwrap(),
                metadata: serde_json::json!({}),
            }],
        )
        .await
        .unwrap();

    assert_eq!(updated.tax_total, Decimal::from_str("8.00").unwrap());
    assert_eq!(updated.total_amount, Decimal::from_str("88.00").unwrap());
}

#[tokio::test]
async fn sanitization_strips_customer_tax_exempt_from_line_item_metadata() {
    let service = setup().await;
    let tenant_id = support::TEST_TENANT_ID;

    let cart = service
        .create_cart(tenant_id, create_cart_input())
        .await
        .unwrap();

    let updated = service
        .add_line_item(
            tenant_id,
            cart.id,
            AddCartLineItemInput {
                metadata: serde_json::json!({
                    "customer_tax_exempt": true,
                    "custom_note": "gift"
                }),
                ..line_item_input()
            },
        )
        .await
        .unwrap();

    assert!(
        updated.line_items[0]
            .metadata
            .get("customer_tax_exempt")
            .is_none()
    );
    assert_eq!(
        updated.line_items[0].metadata.get("custom_note"),
        Some(&serde_json::json!("gift"))
    );
}

#[tokio::test]
async fn whitespace_or_empty_line_item_title_rejected() {
    let service = setup().await;
    let tenant_id = support::TEST_TENANT_ID;

    let cart = service
        .create_cart(tenant_id, create_cart_input())
        .await
        .unwrap();

    let err = service
        .add_line_item(
            tenant_id,
            cart.id,
            AddCartLineItemInput {
                title: "   ".to_string(),
                ..line_item_input()
            },
        )
        .await
        .expect_err("whitespace title must be rejected");

    assert!(matches!(err, CartError::Validation(_)));

    let err = service
        .add_line_item(
            tenant_id,
            cart.id,
            AddCartLineItemInput {
                title: "".to_string(),
                ..line_item_input()
            },
        )
        .await
        .expect_err("empty title must be rejected");

    assert!(matches!(err, CartError::Validation(_)));
}

#[tokio::test]
async fn line_item_title_trimmed_and_blank_sku_filtered() {
    let service = setup().await;
    let tenant_id = support::TEST_TENANT_ID;

    let cart = service
        .create_cart(tenant_id, create_cart_input())
        .await
        .unwrap();

    let updated = service
        .add_line_item(
            tenant_id,
            cart.id,
            AddCartLineItemInput {
                title: "  Trimmed Widget  ".to_string(),
                sku: Some("   ".to_string()),
                ..line_item_input()
            },
        )
        .await
        .unwrap();

    assert_eq!(updated.line_items[0].title, "Trimmed Widget");
    assert_eq!(updated.line_items[0].sku, None);
}

#[tokio::test]
async fn create_cart_and_update_context_normalize_whitespace_email() {
    let service = setup().await;
    let tenant_id = support::TEST_TENANT_ID;

    let cart = service
        .create_cart(
            tenant_id,
            CreateCartInput {
                email: Some("  buyer@example.com  ".to_string()),
                ..create_cart_input()
            },
        )
        .await
        .unwrap();

    assert_eq!(cart.email.as_deref(), Some("buyer@example.com"));

    let updated = service
        .update_context(
            tenant_id,
            rustok_cart::CartContextUpdateRequest {
                cart_id: cart.id,
                pricing_context: rustok_cart::CartPricingContextGuard::capture(&cart).unwrap(),
                pricing: rustok_cart::CartContextPricing::Preserve,
                input: UpdateCartContextInput {
                    email: Some("  updated@example.com  ".to_string()),
                    region_id: None,
                    country_code: None,
                    locale_code: None,
                    selected_shipping_option_id: None,
                    shipping_selections: None,
                },
            },
        )
        .await
        .unwrap();

    assert_eq!(updated.email.as_deref(), Some("updated@example.com"));

    let cleared = service
        .update_context(
            tenant_id,
            rustok_cart::CartContextUpdateRequest {
                cart_id: cart.id,
                pricing_context: rustok_cart::CartPricingContextGuard::capture(&updated).unwrap(),
                pricing: rustok_cart::CartContextPricing::Preserve,
                input: UpdateCartContextInput {
                    email: Some("   ".to_string()),
                    region_id: None,
                    country_code: None,
                    locale_code: None,
                    selected_shipping_option_id: None,
                    shipping_selections: None,
                },
            },
        )
        .await
        .unwrap();

    assert_eq!(cleared.email, None);
}

#[test]
fn normalize_locale_code_validates_and_lowercases() {
    use rustok_cart::services::cart::helpers::normalize_locale_code;

    assert_eq!(normalize_locale_code("en").unwrap(), "en");
    assert_eq!(normalize_locale_code("pt_BR").unwrap(), "pt-br");
    assert_eq!(normalize_locale_code("pl_PL").unwrap(), "pl-pl");
    assert_eq!(normalize_locale_code("ru-RU").unwrap(), "ru-ru");

    assert!(normalize_locale_code("12").is_err());
    assert!(normalize_locale_code("$$").is_err());
    assert!(normalize_locale_code("").is_err());
    assert!(normalize_locale_code("toolonglocalecode").is_err());
}

// Regression sources for priced adoption. The maintainer runs these; no execution is implied.
#[tokio::test]
async fn concurrent_quantity_observation_cannot_overwrite_the_first_priced_write() {
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
    let guard = rustok_cart::CartPricingContextGuard::capture(&cart).unwrap();
    let id = cart.line_items[0].id;
    service
        .update_line_item_pricing(
            tenant_id,
            cart.id,
            rustok_cart::CartLineItemPricingUpdate {
                line_item_id: id,
                quantity: 3,
                unit_price: Decimal::from(9),
                pricing_adjustment: None,
            },
            guard.clone(),
        )
        .await
        .unwrap();
    let error = service
        .update_line_item_pricing(
            tenant_id,
            cart.id,
            rustok_cart::CartLineItemPricingUpdate {
                line_item_id: id,
                quantity: 4,
                unit_price: Decimal::from(8),
                pricing_adjustment: None,
            },
            guard,
        )
        .await
        .unwrap_err();
    assert!(matches!(error, CartError::PricingContextChanged));
    let current = service.get_cart(tenant_id, cart.id).await.unwrap();
    assert_eq!(current.line_items[0].quantity, 3);
    assert_eq!(current.line_items[0].unit_price, Decimal::from(9));
    assert_eq!(current.line_items[0].total_price, Decimal::from(27));
}

#[tokio::test]
async fn bulk_adoption_rejects_duplicates_unknown_lines_and_wrong_tier_quantity_without_partial_writes()
 {
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
    let update = rustok_cart::CartLineItemPricingUpdate {
        line_item_id: cart.line_items[0].id,
        quantity: cart.line_items[0].quantity,
        unit_price: Decimal::from(99),
        pricing_adjustment: None,
    };
    let mut unknown = update.clone();
    unknown.line_item_id = Uuid::new_v4();
    let mut wrong_quantity = update.clone();
    wrong_quantity.quantity += 1;
    for updates in [
        vec![update.clone(), update.clone()],
        vec![update.clone(), unknown],
        vec![wrong_quantity],
    ] {
        let guard = rustok_cart::CartPricingContextGuard::capture(&cart).unwrap();
        assert!(
            service
                .reprice_line_items(tenant_id, cart.id, updates, guard)
                .await
                .is_err()
        );
        let current = service.get_cart(tenant_id, cart.id).await.unwrap();
        assert_eq!(current.line_items[0].quantity, cart.line_items[0].quantity);
        assert_eq!(
            current.line_items[0].unit_price,
            cart.line_items[0].unit_price
        );
        assert_eq!(current.adjustment_total, cart.adjustment_total);
    }
}

#[tokio::test]
async fn batch_quote_for_an_old_quantity_is_refused_after_an_intervening_update() {
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
    let guard = rustok_cart::CartPricingContextGuard::capture(&cart).unwrap();
    let id = cart.line_items[0].id;
    service
        .update_line_item_pricing(
            tenant_id,
            cart.id,
            rustok_cart::CartLineItemPricingUpdate {
                line_item_id: id,
                quantity: 3,
                unit_price: Decimal::from(12),
                pricing_adjustment: None,
            },
            guard.clone(),
        )
        .await
        .unwrap();
    let error = service
        .reprice_line_items(
            tenant_id,
            cart.id,
            vec![rustok_cart::CartLineItemPricingUpdate {
                line_item_id: id,
                quantity: 2,
                unit_price: Decimal::from(7),
                pricing_adjustment: None,
            }],
            guard,
        )
        .await
        .unwrap_err();
    assert!(matches!(error, CartError::PricingContextChanged));
    let current = service.get_cart(tenant_id, cart.id).await.unwrap();
    assert_eq!(current.line_items[0].quantity, 3);
    assert_eq!(current.line_items[0].unit_price, Decimal::from(12));
}

#[tokio::test]
async fn decrement_to_removal_cannot_delete_a_line_changed_since_observation() {
    let service = setup().await;
    let tenant_id = support::TEST_TENANT_ID;
    let cart = service
        .create_cart(tenant_id, create_cart_input())
        .await
        .unwrap();
    let mut input = line_item_input();
    input.quantity = 1;
    let cart = service
        .add_line_item(tenant_id, cart.id, input)
        .await
        .unwrap();
    let guard = rustok_cart::CartPricingContextGuard::capture(&cart).unwrap();
    let removal =
        rustok_cart::CartPricingContextGuard::capture_removal(&cart, cart.line_items[0].id)
            .unwrap();
    let id = cart.line_items[0].id;
    service
        .update_line_item_pricing(
            tenant_id,
            cart.id,
            rustok_cart::CartLineItemPricingUpdate {
                line_item_id: id,
                quantity: 2,
                unit_price: Decimal::from(8),
                pricing_adjustment: None,
            },
            guard.clone(),
        )
        .await
        .unwrap();
    assert!(matches!(
        service
            .remove_line_item(tenant_id, cart.id, id, removal)
            .await
            .unwrap_err(),
        CartError::PricingContextChanged
    ));
    assert_eq!(
        service
            .get_cart(tenant_id, cart.id)
            .await
            .unwrap()
            .line_items[0]
            .quantity,
        2
    );
}

#[tokio::test]
async fn root_and_module_storefront_providers_preserve_the_prequote_add_guard() {
    let (db, service) = setup_with_db().await;
    let tenant_id = support::TEST_TENANT_ID;
    for port in [
        rustok_cart::in_process_cart_storefront_port(db.clone()),
        rustok_cart::ports::in_process_cart_storefront_port(db.clone()),
    ] {
        let cart = service
            .create_cart(tenant_id, create_cart_input())
            .await
            .unwrap();
        let guard = rustok_cart::CartPricingContextGuard::capture_add(&cart).unwrap();
        service
            .add_line_item(tenant_id, cart.id, line_item_input())
            .await
            .unwrap();
        let context = rustok_api::PortContext::new(
            tenant_id.to_string(),
            rustok_api::PortActor::service("cart-test"),
            "en",
            Uuid::new_v4().to_string(),
        )
        .with_deadline(std::time::Duration::from_secs(2))
        .with_idempotency_key(Uuid::new_v4().to_string());
        let error = port
            .add_storefront_line_item(
                context,
                rustok_cart::CartStorefrontAddLineItemRequest {
                    cart_id: cart.id,
                    input: line_item_input(),
                    pricing_adjustment: None,
                    pricing_context: guard,
                },
            )
            .await
            .unwrap_err();
        assert_eq!(error.kind, rustok_api::PortErrorKind::Conflict);
        assert_eq!(error.code, "cart.pricing_context_changed");
        assert!(!error.retryable);
        assert_eq!(
            service
                .get_cart(tenant_id, cart.id)
                .await
                .unwrap()
                .line_items
                .len(),
            1
        );
    }
}

#[tokio::test]
async fn target_removal_observation_does_not_require_priced_cart_validity_and_cannot_authorize_prices()
 {
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
    let id = cart.line_items[0].id;
    let removal = rustok_cart::CartPricingContextGuard::capture_removal(&cart, id).unwrap();
    let error = service
        .update_line_item_pricing(
            tenant_id,
            cart.id,
            rustok_cart::CartLineItemPricingUpdate {
                line_item_id: id,
                quantity: 3,
                unit_price: Decimal::ONE,
                pricing_adjustment: None,
            },
            removal,
        )
        .await
        .unwrap_err();
    assert!(matches!(error, CartError::Validation(_)));
    let mut oversized = cart.clone();
    while oversized.line_items.len() <= rustok_cart::MAX_PRICED_CART_LINES {
        let mut line = cart.line_items[0].clone();
        line.id = Uuid::new_v4();
        oversized.line_items.push(line);
    }
    oversized.line_items[0].unit_price = -Decimal::ONE;
    assert!(rustok_cart::CartPricingContextGuard::capture(&oversized).is_err());
    assert!(rustok_cart::CartPricingContextGuard::capture_removal(&oversized, id).is_ok());
    assert_eq!(
        service
            .get_cart(tenant_id, cart.id)
            .await
            .unwrap()
            .line_items[0]
            .quantity,
        2
    );
}

#[tokio::test]
async fn explicit_target_removal_can_reduce_an_oversized_persisted_cart() {
    use rustok_cart::entities::cart_line_item;
    use sea_orm::EntityTrait;
    let (db, service) = setup_with_db().await;
    let tenant_id = support::TEST_TENANT_ID;
    let cart = service
        .create_cart(tenant_id, create_cart_input())
        .await
        .unwrap();
    let cart = service
        .add_line_item(tenant_id, cart.id, line_item_input())
        .await
        .unwrap();
    let id = cart.line_items[0].id;
    // Fixture-owned historical state; production code does not bypass Cart admission.
    let template = cart_line_item::Entity::find_by_id(id)
        .one(&db)
        .await
        .unwrap()
        .unwrap();
    let rows: Vec<cart_line_item::ActiveModel> = (0..rustok_cart::MAX_PRICED_CART_LINES)
        .map(|_| {
            let mut row: cart_line_item::ActiveModel = template.clone().into();
            row.id = Set(Uuid::new_v4());
            row
        })
        .collect();
    for chunk in rows.chunks(128) {
        cart_line_item::Entity::insert_many(chunk.to_vec())
            .exec(&db)
            .await
            .unwrap();
    }
    let cart = service.get_cart(tenant_id, cart.id).await.unwrap();
    assert_eq!(
        cart.line_items.len(),
        rustok_cart::MAX_PRICED_CART_LINES + 1
    );
    assert!(rustok_cart::CartPricingContextGuard::capture(&cart).is_err());
    let guard = rustok_cart::CartPricingContextGuard::capture_removal(&cart, id).unwrap();
    let current = service
        .remove_line_item(tenant_id, cart.id, id, guard)
        .await
        .unwrap();
    assert_eq!(current.line_items.len(), rustok_cart::MAX_PRICED_CART_LINES);
    assert!(rustok_cart::CartPricingContextGuard::capture(&current).is_ok());
}

#[tokio::test]
async fn quantity_tier_adoption_replaces_pricing_discount_and_preserves_manual_adjustments() {
    let service = setup().await;
    let tenant_id = support::TEST_TENANT_ID;
    let cart = service
        .create_cart(tenant_id, create_cart_input())
        .await
        .unwrap();
    let mut input = line_item_input();
    input.quantity = 2;
    input.unit_price = Decimal::from(10);
    let cart = service
        .add_line_item_with_pricing_adjustment(
            tenant_id,
            cart.id,
            input,
            Some(CartPricingAdjustmentUpdate {
                source_id: None,
                amount: Decimal::from(2),
                metadata: serde_json::json!({"kind":"sale"}),
            }),
        )
        .await
        .unwrap();
    let cart = service
        .apply_fixed_promotion(
            tenant_id,
            cart.id,
            None,
            "manual-keep",
            Decimal::ONE,
            serde_json::json!({}),
        )
        .await
        .unwrap();
    let guard = rustok_cart::CartPricingContextGuard::capture(&cart).unwrap();
    let current = service
        .update_line_item_pricing(
            tenant_id,
            cart.id,
            rustok_cart::CartLineItemPricingUpdate {
                line_item_id: cart.line_items[0].id,
                quantity: 5,
                unit_price: Decimal::from(10),
                pricing_adjustment: Some(CartPricingAdjustmentUpdate {
                    source_id: None,
                    amount: Decimal::from(15),
                    metadata: serde_json::json!({"kind":"sale"}),
                }),
            },
            guard,
        )
        .await
        .unwrap();
    assert_eq!(current.line_items[0].quantity, 5);
    assert_eq!(current.subtotal_amount, Decimal::from(50));
    assert_eq!(current.total_amount, Decimal::from(34));
    assert_eq!(
        current
            .adjustments
            .iter()
            .filter(|a| a.source_type == "pricing")
            .count(),
        1
    );
    assert!(
        current
            .adjustments
            .iter()
            .any(|a| a.source_type == "pricing" && a.amount == Decimal::from(15))
    );
    assert!(
        current
            .adjustments
            .iter()
            .any(|a| a.source_id.as_deref() == Some("manual-keep") && a.amount == Decimal::ONE)
    );
}

fn repriced_context_command(
    cart: &rustok_cart::CartResponse,
) -> rustok_cart::CartContextUpdateRequest {
    let input = UpdateCartContextInput {
        email: cart.email.clone(),
        region_id: cart.region_id,
        country_code: cart.country_code.clone(),
        locale_code: Some("fr".into()),
        selected_shipping_option_id: cart.selected_shipping_option_id,
        shipping_selections: None,
    };
    let scope = rustok_cart::CartPricingScope::capture(&cart.project_context(&input).unwrap());
    rustok_cart::CartContextUpdateRequest {
        cart_id: cart.id,
        input,
        pricing_context: rustok_cart::CartPricingContextGuard::capture(cart).unwrap(),
        pricing: rustok_cart::CartContextPricing::Reprice(rustok_cart::CartContextPricingPlan {
            scope,
            updates: cart
                .line_items
                .iter()
                .map(|line| rustok_cart::CartLineItemPricingUpdate {
                    line_item_id: line.id,
                    quantity: line.quantity,
                    unit_price: Decimal::from(20),
                    pricing_adjustment: Some(CartPricingAdjustmentUpdate {
                        source_id: None,
                        amount: Decimal::from(4),
                        metadata: serde_json::json!({"quoted_locale":"fr"}),
                    }),
                })
                .collect(),
        }),
    }
}

async fn cart_for_context_adoption(service: &CartService) -> rustok_cart::CartResponse {
    let tenant = support::TEST_TENANT_ID;
    let cart = service
        .create_cart(tenant, create_cart_input())
        .await
        .unwrap();
    service
        .add_line_item(
            tenant,
            cart.id,
            AddCartLineItemInput {
                fulfillment_requirement: CartLineFulfillmentRequirement::Digital,
                ..line_item_input()
            },
        )
        .await
        .unwrap()
}

#[tokio::test]
async fn context_and_complete_price_plan_commit_together_with_adjustments_and_totals() {
    let service = setup().await;
    let cart = cart_for_context_adoption(&service).await;
    let updated = service
        .update_context(cart.tenant_id, repriced_context_command(&cart))
        .await
        .unwrap();
    assert_eq!(updated.locale_code.as_deref(), Some("fr"));
    assert_eq!(updated.line_items[0].quantity, 2);
    assert_eq!(updated.line_items[0].unit_price, Decimal::from(20));
    assert_eq!(updated.line_items[0].total_price, Decimal::from(40));
    assert_eq!(updated.total_amount, Decimal::from(36));
    assert_eq!(
        updated
            .adjustments
            .iter()
            .filter(|item| item.source_type == "pricing")
            .count(),
        1
    );
    assert!(
        updated
            .context_is_current(&repriced_context_command(&updated).input)
            .unwrap()
    );
}

#[tokio::test]
async fn nonempty_context_cannot_preserve_a_different_unit_price_basis() {
    let service = setup().await;
    let cart = cart_for_context_adoption(&service).await;
    let mut command = repriced_context_command(&cart);
    command.pricing = rustok_cart::CartContextPricing::Preserve;
    let error = service
        .update_context(cart.tenant_id, command)
        .await
        .unwrap_err();
    assert!(matches!(error, CartError::PricingContextChanged));
    let current = service.get_cart(cart.tenant_id, cart.id).await.unwrap();
    assert_eq!(current.locale_code, cart.locale_code);
    assert_eq!(current.updated_at, cart.updated_at);
    assert_eq!(
        current.line_items[0].unit_price,
        cart.line_items[0].unit_price
    );
    assert_eq!(current.total_amount, cart.total_amount);
}

#[tokio::test]
async fn context_plan_rejects_missing_duplicate_unknown_quantity_scope_and_stale_observations() {
    let service = setup().await;
    let cart = cart_for_context_adoption(&service).await;
    for fault in 0..5 {
        let mut command = repriced_context_command(&cart);
        let rustok_cart::CartContextPricing::Reprice(plan) = &mut command.pricing else {
            unreachable!()
        };
        match fault {
            0 => plan.updates.clear(),
            1 => plan.updates.push(plan.updates[0].clone()),
            2 => plan.updates[0].line_item_id = Uuid::new_v4(),
            3 => plan.updates[0].quantity += 1,
            _ => plan.scope.locale_code = Some("de".into()),
        }
        assert!(
            service
                .update_context(cart.tenant_id, command)
                .await
                .is_err()
        );
        let current = service.get_cart(cart.tenant_id, cart.id).await.unwrap();
        assert_eq!(current.locale_code, cart.locale_code);
        assert_eq!(current.updated_at, cart.updated_at);
        assert_eq!(
            current.line_items[0].unit_price,
            cart.line_items[0].unit_price
        );
    }
    let stale = repriced_context_command(&cart);
    let accepted = service
        .update_context(cart.tenant_id, repriced_context_command(&cart))
        .await
        .unwrap();
    assert!(matches!(
        service
            .update_context(cart.tenant_id, stale)
            .await
            .unwrap_err(),
        CartError::PricingContextChanged
    ));
    let current = service.get_cart(cart.tenant_id, cart.id).await.unwrap();
    assert_eq!(current.updated_at, accepted.updated_at);
    assert_eq!(current.locale_code.as_deref(), Some("fr"));
}

#[tokio::test]
async fn shipping_refusal_rolls_back_already_materialized_context_line_and_adjustment_changes() {
    let service = setup().await;
    let cart = cart_for_context_adoption(&service).await;
    let mut command = repriced_context_command(&cart);
    // The scope/price plan is valid; group application fails after line/adjustment writes.
    command.input.shipping_selections = Some(vec![CartShippingSelectionInput {
        shipping_profile_slug: "unavailable".into(),
        seller_id: None,
        seller_scope: None,
        selected_shipping_option_id: Some(Uuid::new_v4()),
    }]);
    assert!(
        service
            .update_context(cart.tenant_id, command)
            .await
            .is_err()
    );
    let current = service.get_cart(cart.tenant_id, cart.id).await.unwrap();
    assert_eq!(current.locale_code, cart.locale_code);
    assert_eq!(current.updated_at, cart.updated_at);
    assert_eq!(current.total_amount, cart.total_amount);
    assert_eq!(current.adjustments.len(), cart.adjustments.len());
    assert_eq!(
        current.line_items[0].unit_price,
        cart.line_items[0].unit_price
    );
    assert_eq!(
        current.line_items[0].updated_at,
        cart.line_items[0].updated_at
    );
}

#[tokio::test]
async fn context_write_refuses_removal_observation_and_locked_or_completed_carts() {
    let service = setup().await;
    let cart = cart_for_context_adoption(&service).await;
    let mut command = repriced_context_command(&cart);
    command.pricing_context =
        rustok_cart::CartPricingContextGuard::capture_removal(&cart, cart.line_items[0].id)
            .unwrap();
    assert!(
        service
            .update_context(cart.tenant_id, command)
            .await
            .is_err()
    );
    service
        .begin_checkout(cart.tenant_id, cart.id)
        .await
        .unwrap();
    assert!(matches!(
        service
            .update_context(cart.tenant_id, repriced_context_command(&cart))
            .await
            .unwrap_err(),
        CartError::InvalidTransition { .. }
    ));
    service
        .complete_cart(cart.tenant_id, cart.id)
        .await
        .unwrap();
    assert!(matches!(
        service
            .update_context(cart.tenant_id, repriced_context_command(&cart))
            .await
            .unwrap_err(),
        CartError::InvalidTransition { .. }
    ));
    let current = service.get_cart(cart.tenant_id, cart.id).await.unwrap();
    assert_eq!(current.status, "completed");
    assert_eq!(current.locale_code, cart.locale_code);
    assert_eq!(
        current.line_items[0].unit_price,
        cart.line_items[0].unit_price
    );
}

#[tokio::test]
async fn full_replacement_scalar_none_clears_the_single_group_shipping_selection() {
    let (db, service) = setup_with_db().await;
    let tenant_id = support::TEST_TENANT_ID;
    let option = Uuid::new_v4();
    insert_shipping_option(&db, tenant_id, option, "USD", Decimal::from(7)).await;
    let cart = service
        .create_cart(
            tenant_id,
            CreateCartInput {
                selected_shipping_option_id: Some(option),
                ..create_cart_input()
            },
        )
        .await
        .unwrap();
    let cart = service
        .add_line_item(tenant_id, cart.id, line_item_input())
        .await
        .unwrap();
    assert_eq!(cart.shipping_total, Decimal::from(7));
    let command = rustok_cart::CartContextUpdateRequest {
        cart_id: cart.id,
        input: UpdateCartContextInput {
            email: cart.email.clone(),
            region_id: cart.region_id,
            country_code: cart.country_code.clone(),
            locale_code: cart.locale_code.clone(),
            selected_shipping_option_id: None,
            shipping_selections: None,
        },
        pricing_context: rustok_cart::CartPricingContextGuard::capture(&cart).unwrap(),
        pricing: rustok_cart::CartContextPricing::Preserve,
    };
    let target = cart.project_context(&command.input).unwrap();
    assert_eq!(target.delivery_groups[0].selected_shipping_option_id, None);
    assert!(!cart.context_is_current(&command.input).unwrap());
    let updated = service.update_context(tenant_id, command).await.unwrap();
    assert_eq!(updated.selected_shipping_option_id, None);
    assert_eq!(updated.delivery_groups[0].selected_shipping_option_id, None);
    assert_eq!(updated.shipping_total, Decimal::ZERO);
}

#[tokio::test]
async fn planless_checkout_refuses_new_price_basis_and_locked_snapshot_prepare_never_reapplies_it()
{
    let service = setup().await;
    let cart = cart_for_context_adoption(&service).await;
    let input = repriced_context_command(&cart).input;
    let request = rustok_cart::PrepareCartCheckoutSnapshotRequest {
        cart_id: cart.id,
        input,
    };
    assert!(matches!(
        service
            .prepare_checkout(cart.tenant_id, request.clone())
            .await
            .unwrap_err(),
        CartError::PricingContextChanged
    ));
    let current = service.get_cart(cart.tenant_id, cart.id).await.unwrap();
    assert_eq!(current.status, "active");
    assert_eq!(current.locale_code, cart.locale_code);
    service
        .begin_checkout(cart.tenant_id, cart.id)
        .await
        .unwrap();
    let port = rustok_cart::checkout_snapshot::in_process_cart_checkout_port(service.clone());
    let context = rustok_api::PortContext::new(
        cart.tenant_id.to_string(),
        rustok_api::PortActor::service("test"),
        "en",
        "original-checkout",
    )
    .with_deadline(std::time::Duration::from_secs(1))
    .with_idempotency_key("original-checkout");
    assert_eq!(
        port.prepare_checkout(context, request)
            .await
            .unwrap_err()
            .kind,
        rustok_api::PortErrorKind::Conflict
    );
    let current = service.get_cart(cart.tenant_id, cart.id).await.unwrap();
    assert_eq!(current.status, "checking_out");
    assert_eq!(current.locale_code, cart.locale_code);
    assert_eq!(
        current.line_items[0].unit_price,
        cart.line_items[0].unit_price
    );
}
