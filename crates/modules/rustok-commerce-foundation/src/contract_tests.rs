use chrono::Utc;
use rust_decimal::Decimal;
use sea_orm::DbBackend;
use uuid::Uuid;

use crate::dto::{
    AdjustInventoryInput, CreateVariantInput, PriceInput, PriceResponse, VariantResponse,
};
use crate::entities::product::ProductStatus;
use crate::entities::{inventory_level, product, reservation_item, shipping_profile, stock_location};
use crate::error::CommerceError;
use crate::search::{
    MAX_SEARCH_QUERY_LEN, product_translation_title_and_locale_search_condition,
    product_translation_title_search_condition,
};
use rustok_core::error::{ErrorKind, RichError};

#[test]
fn test_crate_api_compliance() {
    let api_doc = include_str!("../CRATE_API.md");
    assert!(api_doc.contains("## Platform Capability Bindings"));
    assert!(api_doc.contains("## Domain Collaborations"));
    assert!(api_doc.contains("## Minimum Contract Set"));
    assert!(api_doc.contains("### Input DTOs/Commands"));
    assert!(api_doc.contains("### Domain Invariants"));
    assert!(api_doc.contains("### Events / Outbox Side Effects"));
    assert!(api_doc.contains("### Errors / Failure Codes"));
}

#[test]
fn search_query_bounding_and_backends() {
    assert_eq!(MAX_SEARCH_QUERY_LEN, 255);

    // Empty search queries produce empty conditions
    let cond_empty_pg = product_translation_title_search_condition(DbBackend::Postgres, "en", "");
    let cond_empty_sqlite =
        product_translation_title_search_condition(DbBackend::Sqlite, "en", "   ");
    assert!(cond_empty_pg.is_empty());
    assert!(cond_empty_sqlite.is_empty());

    // Localized search conditions with empty search
    let cond_loc_empty =
        product_translation_title_and_locale_search_condition(DbBackend::Postgres, "en", "");
    assert!(cond_loc_empty.is_empty());

    // Non-empty queries generate non-empty conditions across backends
    let cond_pg = product_translation_title_search_condition(DbBackend::Postgres, "en", "shoes");
    let cond_sqlite = product_translation_title_search_condition(DbBackend::Sqlite, "en", "shoes");
    assert!(!cond_pg.is_empty());
    assert!(!cond_sqlite.is_empty());

    // Localized condition with locale fallback
    let cond_fallback = product_translation_title_and_locale_search_condition(
        DbBackend::Sqlite,
        "   ",
        "shoes",
    );
    assert!(!cond_fallback.is_empty());

    // Truncation at MAX_SEARCH_QUERY_LEN
    let oversized = "x".repeat(500);
    let cond_truncated =
        product_translation_title_search_condition(DbBackend::Postgres, "en", &oversized);
    assert!(!cond_truncated.is_empty());
}

#[test]
fn price_input_invariants() {
    let valid = PriceInput {
        currency_code: "USD".to_string(),
        channel_id: Some(Uuid::new_v4()),
        channel_slug: Some("main".to_string()),
        amount: Decimal::new(2999, 2),
        compare_at_amount: Some(Decimal::new(3999, 2)),
    };
    assert!(valid.validate_invariants().is_ok());

    // Invalid currency codes: non-alpha, wrong length
    let invalid_currencies = ["US1", "US", "USDD", "123", "   "];
    for curr in invalid_currencies {
        let p = PriceInput {
            currency_code: curr.to_string(),
            ..valid.clone()
        };
        assert!(p.validate_invariants().is_err());
    }

    // Negative amounts rejected
    let neg_amount = PriceInput {
        amount: Decimal::new(-100, 2),
        ..valid.clone()
    };
    assert!(neg_amount.validate_invariants().is_err());

    // Negative compare_at_amount rejected
    let neg_compare = PriceInput {
        compare_at_amount: Some(Decimal::new(-50, 2)),
        ..valid.clone()
    };
    assert!(neg_compare.validate_invariants().is_err());

    // Nil channel_id rejected
    let nil_channel = PriceInput {
        channel_id: Some(Uuid::nil()),
        ..valid.clone()
    };
    assert!(nil_channel.validate_invariants().is_err());

    // Blank or oversized channel_slug rejected
    let blank_slug = PriceInput {
        channel_slug: Some("   ".to_string()),
        ..valid.clone()
    };
    assert!(blank_slug.validate_invariants().is_err());

    let long_slug = PriceInput {
        channel_slug: Some("s".repeat(101)),
        ..valid.clone()
    };
    assert!(long_slug.validate_invariants().is_err());
}

#[test]
fn create_variant_input_invariants() {
    let valid_price = PriceInput {
        currency_code: "EUR".to_string(),
        channel_id: None,
        channel_slug: None,
        amount: Decimal::new(1500, 2),
        compare_at_amount: None,
    };

    let valid_variant = CreateVariantInput {
        sku: Some("SKU-100".to_string()),
        barcode: Some("123456789".to_string()),
        shipping_profile_slug: Some("standard".to_string()),
        prices: vec![valid_price],
        inventory_quantity: 10,
        inventory_policy: "deny".to_string(),
        weight: Some(Decimal::new(500, 0)),
        weight_unit: Some("g".to_string()),
    };
    assert!(valid_variant.validate_invariants().is_ok());

    // Negative inventory quantity rejected
    let neg_qty = CreateVariantInput {
        inventory_quantity: -1,
        ..valid_variant.clone()
    };
    assert!(neg_qty.validate_invariants().is_err());

    // Blank inventory policy rejected
    let blank_policy = CreateVariantInput {
        inventory_policy: "  ".to_string(),
        ..valid_variant.clone()
    };
    assert!(blank_policy.validate_invariants().is_err());

    // Negative weight rejected
    let neg_weight = CreateVariantInput {
        weight: Some(Decimal::new(-1, 0)),
        ..valid_variant.clone()
    };
    assert!(neg_weight.validate_invariants().is_err());

    // Blank weight unit rejected
    let blank_unit = CreateVariantInput {
        weight_unit: Some("   ".to_string()),
        ..valid_variant.clone()
    };
    assert!(blank_unit.validate_invariants().is_err());

    // Blank SKU rejected
    let blank_sku = CreateVariantInput {
        sku: Some("   ".to_string()),
        ..valid_variant.clone()
    };
    assert!(blank_sku.validate_invariants().is_err());

    // Invalid nested price propagates error
    let invalid_price_variant = CreateVariantInput {
        prices: vec![PriceInput {
            currency_code: "INVALID".to_string(),
            channel_id: None,
            channel_slug: None,
            amount: Decimal::ZERO,
            compare_at_amount: None,
        }],
        ..valid_variant
    };
    assert!(invalid_price_variant.validate_invariants().is_err());
}

#[test]
fn adjust_inventory_input_invariants() {
    let valid_pos = AdjustInventoryInput {
        variant_id: Uuid::new_v4(),
        adjustment: 5,
        reason: Some("restock".to_string()),
    };
    assert!(valid_pos.validate_invariants().is_ok());

    let valid_neg = AdjustInventoryInput {
        variant_id: Uuid::new_v4(),
        adjustment: -3,
        reason: None,
    };
    assert!(valid_neg.validate_invariants().is_ok());

    // Zero adjustment rejected
    let zero_adj = AdjustInventoryInput {
        adjustment: 0,
        ..valid_pos.clone()
    };
    assert!(zero_adj.validate_invariants().is_err());

    // Nil variant ID rejected
    let nil_var = AdjustInventoryInput {
        variant_id: Uuid::nil(),
        ..valid_pos.clone()
    };
    assert!(nil_var.validate_invariants().is_err());

    // Blank reason rejected
    let blank_reason = AdjustInventoryInput {
        reason: Some("   ".to_string()),
        ..valid_pos.clone()
    };
    assert!(blank_reason.validate_invariants().is_err());

    // Oversized reason rejected (> 255 chars)
    let long_reason = AdjustInventoryInput {
        reason: Some("r".repeat(256)),
        ..valid_pos
    };
    assert!(long_reason.validate_invariants().is_err());
}

#[test]
fn price_response_discount_math_invariants() {
    // Normal sale: amount 80, compare_at 100 -> 20 discount, 20%
    let sale = PriceResponse {
        currency_code: "USD".to_string(),
        amount: Decimal::new(80, 0),
        compare_at_amount: Some(Decimal::new(100, 0)),
        on_sale: true,
    };
    assert!(sale.is_on_sale());
    assert_eq!(sale.discount_amount(), Some(Decimal::new(20, 0)));
    assert_eq!(sale.discount_percentage(), Some(Decimal::new(20, 0)));

    // Not on sale: compare_at <= amount
    let equal = PriceResponse {
        amount: Decimal::new(100, 0),
        compare_at_amount: Some(Decimal::new(100, 0)),
        ..sale.clone()
    };
    assert!(!equal.is_on_sale());
    assert_eq!(equal.discount_amount(), None);
    assert_eq!(equal.discount_percentage(), None);

    let cheaper = PriceResponse {
        amount: Decimal::new(120, 0),
        compare_at_amount: Some(Decimal::new(100, 0)),
        ..sale.clone()
    };
    assert!(!cheaper.is_on_sale());
    assert_eq!(cheaper.discount_amount(), None);
    assert_eq!(cheaper.discount_percentage(), None);

    // Zero compare_at_amount does not panic
    let zero_compare = PriceResponse {
        amount: Decimal::ZERO,
        compare_at_amount: Some(Decimal::ZERO),
        ..sale
    };
    assert!(!zero_compare.is_on_sale());
    assert_eq!(zero_compare.discount_percentage(), None);
}

#[test]
fn variant_response_helpers() {
    let variant = VariantResponse {
        id: Uuid::new_v4(),
        product_id: Uuid::new_v4(),
        sku: Some("SKU-1".to_string()),
        barcode: None,
        shipping_profile_slug: None,
        title: "Default Title".to_string(),
        translations: vec![],
        combination_identity: None,
        prices: vec![
            PriceResponse {
                currency_code: "USD".to_string(),
                amount: Decimal::new(100, 0),
                compare_at_amount: None,
                on_sale: false,
            },
            PriceResponse {
                currency_code: "EUR".to_string(),
                amount: Decimal::new(90, 0),
                compare_at_amount: None,
                on_sale: false,
            },
        ],
        inventory_quantity: 0,
        inventory_policy: "continue".to_string(),
        in_stock: false,
        weight: None,
        weight_unit: None,
        position: 1,
    };

    assert!(variant.allows_backorders());
    assert!(variant.is_available_to_sell()); // allows backorders despite in_stock = false

    // Price lookup is case-insensitive
    assert!(variant.price_for_currency("usd").is_some());
    assert!(variant.price_for_currency("USD").is_some());
    assert!(variant.price_for_currency("eur").is_some());
    assert!(variant.price_for_currency("GBP").is_none());
}

#[test]
fn entity_lifecycle_and_helpers() {
    // ProductStatus lifecycle transitions
    assert!(ProductStatus::Draft.can_transition_to(&ProductStatus::Active));
    assert!(ProductStatus::Draft.can_transition_to(&ProductStatus::Archived));
    assert!(ProductStatus::Active.can_transition_to(&ProductStatus::Draft));
    assert!(ProductStatus::Active.can_transition_to(&ProductStatus::Archived));
    assert!(ProductStatus::Archived.can_transition_to(&ProductStatus::Draft));
    assert!(ProductStatus::Archived.can_transition_to(&ProductStatus::Active));
    assert!(ProductStatus::Draft.can_transition_to(&ProductStatus::Draft));
    assert_eq!(ProductStatus::Draft.to_string(), "draft");
    assert_eq!(ProductStatus::Active.to_string(), "active");
    assert_eq!(ProductStatus::Archived.to_string(), "archived");

    // Product published and draft helpers
    let product_model = product::Model {
        id: Uuid::new_v4(),
        tenant_id: Uuid::new_v4(),
        status: ProductStatus::Active,
        seller_id: None,
        vendor: None,
        product_type: None,
        shipping_profile_slug: None,
        primary_category_id: None,
        metadata: serde_json::json!({}),
        created_at: Utc::now().into(),
        updated_at: Utc::now().into(),
        published_at: Some(Utc::now().into()),
    };
    assert!(product_model.is_published());
    assert!(!product_model.is_draft());

    // InventoryLevel saturating subtraction and low stock
    let inv_level = inventory_level::Model {
        id: Uuid::new_v4(),
        inventory_item_id: Uuid::new_v4(),
        location_id: Uuid::new_v4(),
        stocked_quantity: 5,
        reserved_quantity: 10, // reserved > stocked: available is negative deficit (-5)
        incoming_quantity: 0,
        low_stock_threshold: Some(3),
        updated_at: Utc::now().into(),
    };
    assert_eq!(inv_level.available_quantity(), -5);
    assert!(inv_level.is_low_stock());
    assert!(!inv_level.can_reserve(1));

    // ReservationItem active check
    let res_item = reservation_item::Model {
        id: Uuid::new_v4(),
        inventory_item_id: Uuid::new_v4(),
        location_id: Uuid::new_v4(),
        quantity: 5,
        line_item_id: None,
        description: None,
        external_id: None,
        metadata: serde_json::json!({}),
        created_at: Utc::now().into(),
        updated_at: Utc::now().into(),
        deleted_at: None,
    };
    assert!(res_item.is_active());

    // StockLocation active check
    let stock_loc = stock_location::Model {
        id: Uuid::new_v4(),
        tenant_id: Uuid::new_v4(),
        code: Some("LOC-1".to_string()),
        address_line1: None,
        address_line2: None,
        city: None,
        province: None,
        postal_code: None,
        country_code: None,
        phone: None,
        metadata: serde_json::json!({}),
        created_at: Utc::now().into(),
        updated_at: Utc::now().into(),
        deleted_at: None,
    };
    assert!(stock_loc.is_active());

    // ShippingProfile usable check
    let ship_prof = shipping_profile::Model {
        id: Uuid::new_v4(),
        tenant_id: Uuid::new_v4(),
        slug: "standard".to_string(),
        active: true,
        metadata: serde_json::json!({}),
        created_at: Utc::now().into(),
        updated_at: Utc::now().into(),
    };
    assert!(ship_prof.is_usable());
}

#[test]
fn commerce_error_conversions() {
    let id = Uuid::new_v4();

    let err_prod = CommerceError::ProductNotFound(id);
    let rich_prod: RichError = err_prod.into();
    assert_eq!(rich_prod.kind, ErrorKind::NotFound);
    assert_eq!(rich_prod.error_code, Some("PRODUCT_NOT_FOUND".to_string()));

    let err_var = CommerceError::VariantNotFound(id);
    let rich_var: RichError = err_var.into();
    assert_eq!(rich_var.kind, ErrorKind::NotFound);
    assert_eq!(rich_var.error_code, Some("VARIANT_NOT_FOUND".to_string()));

    let err_ship = CommerceError::ShippingProfileNotFound(id);
    let rich_ship: RichError = err_ship.into();
    assert_eq!(rich_ship.kind, ErrorKind::NotFound);
    assert_eq!(
        rich_ship.error_code,
        Some("SHIPPING_PROFILE_NOT_FOUND".to_string())
    );

    let err_slug = CommerceError::DuplicateShippingProfileSlug("express".to_string());
    let rich_slug: RichError = err_slug.into();
    assert_eq!(rich_slug.kind, ErrorKind::Conflict);

    let err_no_var = CommerceError::NoVariants;
    let rich_no_var: RichError = err_no_var.into();
    assert_eq!(rich_no_var.kind, ErrorKind::Validation);

    let err_del_pub = CommerceError::CannotDeletePublished;
    let rich_del_pub: RichError = err_del_pub.into();
    assert_eq!(rich_del_pub.kind, ErrorKind::BusinessLogic);

    let err_opt = CommerceError::InvalidOptionCombination;
    let rich_opt: RichError = err_opt.into();
    assert_eq!(rich_opt.kind, ErrorKind::Validation);

    let err_price = CommerceError::InvalidPrice("negative".to_string());
    let rich_price: RichError = err_price.into();
    assert_eq!(rich_price.kind, ErrorKind::Validation);
}
