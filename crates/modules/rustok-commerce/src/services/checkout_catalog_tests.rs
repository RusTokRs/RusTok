use super::*;
use rust_decimal::Decimal;
use serde_json::json;

pub(crate) fn fixture() -> (CartResponse, ProductResponse) {
    let tenant_id = Uuid::from_u128(1);
    let cart_id = Uuid::from_u128(2);
    let product_id = Uuid::from_u128(3);
    let variant_id = Uuid::from_u128(4);
    let time = "2026-10-30T12:00:00Z";
    let line = json!({
        "id": Uuid::from_u128(5), "cart_id": cart_id,
        "product_id": product_id, "variant_id": variant_id,
        "fulfillment_requirement": "physical", "shipping_profile_slug": "default",
        "sku": "original-sku-text", "title": "Original cart title", "quantity": 2,
        "unit_price": "10", "total_price": "20", "currency_code": "USD",
        "metadata": {"original": true}, "created_at": time, "updated_at": time
    });
    let cart: CartResponse = serde_json::from_value(json!({
        "id": cart_id, "tenant_id": tenant_id, "status": "checking_out",
        "locale_code": "en-GB", "currency_code": "USD",
        "subtotal_amount": "20", "adjustment_total": "0", "shipping_total": "0",
        "total_amount": "20", "tax_total": "0", "metadata": {},
        "created_at": time, "updated_at": time, "line_items": [line],
        "adjustments": [], "tax_lines": [], "delivery_groups": []
    }))
    .expect("complete Cart owner fixture");
    let product: ProductResponse = serde_json::from_value(json!({
        "id": product_id, "tenant_id": tenant_id, "status": "active",
        "product_type": "physical", "fulfillment_requirement": "physical",
        "tags": [], "metadata": {}, "created_at": time, "updated_at": time,
        "published_at": time, "revision": 1, "translations": [], "images": [],
        "variant_axes": [], "variants": [{
            "id": variant_id, "product_id": product_id, "sku": "renamed-catalog-sku",
            "title": "Current catalog title", "translations": [], "axis_values": [],
            "prices": [], "inventory_quantity": 5, "inventory_policy": "deny",
            "in_stock": true, "position": 0
        }]
    }))
    .expect("complete Product owner fixture");
    (cart, product)
}

fn duplicate_line(cart: &mut CartResponse, quantity: i32) {
    let mut line = cart.line_items[0].clone();
    line.id = Uuid::new_v4();
    line.quantity = quantity;
    line.total_price = line.unit_price * Decimal::from(quantity);
    cart.line_items.push(line);
}

#[test]
fn stock_demand_sums_same_uuid_without_merging_lines_or_prices() {
    let (mut cart, _) = fixture();
    duplicate_line(&mut cart, 3);
    cart.line_items[1].unit_price = Decimal::from(8);
    cart.line_items[1].metadata = json!({"separate_tier_snapshot": true});
    let before = serde_json::to_value(&cart).unwrap();
    let demands = variant_demands(&cart).unwrap();
    assert_eq!(demands.len(), 1);
    assert_eq!(demands[0].quantity, 5);
    assert_eq!(demands[0].line_items.len(), 2);
    assert_ne!(demands[0].line_items[0].id, demands[0].line_items[1].id);
    assert_ne!(
        demands[0].line_items[0].unit_price,
        demands[0].line_items[1].unit_price
    );
    assert_eq!(serde_json::to_value(&cart).unwrap(), before);
}

#[test]
fn sku_text_and_product_position_never_group_distinct_variant_uuids() {
    let (mut cart, _) = fixture();
    duplicate_line(&mut cart, 3);
    cart.line_items[1].variant_id = Some(Uuid::from_u128(9));
    let demands = variant_demands(&cart).unwrap();
    assert_eq!(demands.len(), 2);
    assert_eq!(demands[0].quantity, 2);
    assert_eq!(demands[1].quantity, 3);
}

#[test]
fn variant_first_history_is_verified_without_rewriting_its_product_reference() {
    let (mut cart, product) = fixture();
    cart.line_items[0].product_id = None;
    duplicate_line(&mut cart, 1);
    cart.line_items[1].product_id = Some(product.id);
    let demands = variant_demands(&cart).unwrap();
    assert_eq!(demands[0].product_id, Some(product.id));
    assert_eq!(demands[0].quantity, 3);
    assert!(cart.line_items[0].product_id.is_none());
    for line in &cart.line_items {
        validate_variant_line(cart.tenant_id, line, &product, None).unwrap();
    }
}

#[test]
fn contradictory_parents_and_checked_quantity_overflow_refuse() {
    let (mut cart, _) = fixture();
    duplicate_line(&mut cart, 1);
    cart.line_items[1].product_id = Some(Uuid::new_v4());
    assert!(variant_demands(&cart).is_err());
    cart.line_items[1].product_id = cart.line_items[0].product_id;
    cart.line_items[0].quantity = i32::MAX;
    assert!(variant_demands(&cart).is_err());
}

#[test]
fn invalid_snapshot_lines_refuse_before_owner_reads() {
    let (cart, _) = fixture();
    for quantity in [0, -1] {
        let mut invalid = cart.clone();
        invalid.line_items[0].quantity = quantity;
        assert!(variant_demands(&invalid).is_err());
    }
    let mut invalid = cart.clone();
    invalid.line_items[0].cart_id = Uuid::new_v4();
    assert!(variant_demands(&invalid).is_err());
    invalid = cart.clone();
    invalid.line_items[0].variant_id = Some(Uuid::nil());
    assert!(variant_demands(&invalid).is_err());
    invalid = cart.clone();
    invalid.line_items[0].currency_code = "EUR".to_string();
    assert!(variant_demands(&invalid).is_err());
    invalid = cart.clone();
    invalid.line_items.push(invalid.line_items[0].clone());
    assert!(variant_demands(&invalid).is_err());
}

#[test]
fn checkout_processing_bound_refuses_without_pruning_stored_lines() {
    let (mut cart, _) = fixture();
    let line = cart.line_items[0].clone();
    cart.line_items = (0..MAX_CHECKOUT_LINE_ITEMS)
        .map(|index| {
            let mut line = line.clone();
            line.id = Uuid::from_u128(100 + index as u128);
            line
        })
        .collect();
    assert!(variant_demands(&cart).is_ok());
    let mut extra = line;
    extra.id = Uuid::from_u128(100 + MAX_CHECKOUT_LINE_ITEMS as u128);
    cart.line_items.push(extra);
    assert!(variant_demands(&cart).is_err());
    assert_eq!(cart.line_items.len(), MAX_CHECKOUT_LINE_ITEMS + 1);
}

#[test]
fn explicit_custom_lines_never_acquire_an_implicit_sku() {
    let (mut cart, _) = fixture();
    cart.line_items[0].product_id = None;
    cart.line_items[0].variant_id = None;
    assert!(variant_demands(&cart).unwrap().is_empty());
}

#[test]
fn owner_tenant_parent_and_selected_uuid_membership_are_checked() {
    let (cart, product) = fixture();
    let line = &cart.line_items[0];
    validate_variant_line(cart.tenant_id, line, &product, None).unwrap();
    let mut invalid = product.clone();
    invalid.tenant_id = Uuid::new_v4();
    assert!(validate_variant_line(cart.tenant_id, line, &invalid, None).is_err());
    invalid = product.clone();
    invalid.id = Uuid::new_v4();
    assert!(validate_variant_line(cart.tenant_id, line, &invalid, None).is_err());
    invalid = product.clone();
    invalid.variants[0].product_id = Uuid::new_v4();
    assert!(validate_variant_line(cart.tenant_id, line, &invalid, None).is_err());
    invalid = product.clone();
    invalid.variants.push(invalid.variants[0].clone());
    assert!(validate_variant_line(cart.tenant_id, line, &invalid, None).is_err());
}

#[test]
fn missing_uuid_never_substitutes_an_available_variant() {
    let (cart, mut product) = fixture();
    product.variants[0].id = Uuid::new_v4();
    assert!(validate_variant_line(cart.tenant_id, &cart.line_items[0], &product, None).is_err());
}

#[test]
fn current_catalog_text_does_not_replace_the_immutable_cart_snapshot() {
    let (cart, product) = fixture();
    let before = serde_json::to_value(&cart).unwrap();
    assert_ne!(cart.line_items[0].sku, product.variants[0].sku);
    validate_variant_line(cart.tenant_id, &cart.line_items[0], &product, None).unwrap();
    assert_eq!(serde_json::to_value(&cart).unwrap(), before);
}

#[test]
fn direct_and_staged_preflight_share_digital_and_stale_fulfillment_rules() {
    let (mut cart, mut product) = fixture();
    product.product_type = Some("digital".to_string());
    product.fulfillment_requirement = ProductFulfillmentRequirement::Digital;
    assert!(validate_variant_line(cart.tenant_id, &cart.line_items[0], &product, None).is_err());
    cart.line_items[0].fulfillment_requirement = CartLineFulfillmentRequirement::Digital;
    assert!(validate_variant_line(cart.tenant_id, &cart.line_items[0], &product, None).is_err());
    cart.line_items[0].shipping_profile_slug = None;
    validate_variant_line(cart.tenant_id, &cart.line_items[0], &product, None).unwrap();
    product.fulfillment_requirement = ProductFulfillmentRequirement::Physical;
    assert!(validate_variant_line(cart.tenant_id, &cart.line_items[0], &product, None).is_err());
}

#[test]
fn variant_shipping_override_and_publication_are_not_dropped() {
    let (mut cart, mut product) = fixture();
    product.shipping_profile_slug = Some("parcel".to_string());
    product.variants[0].shipping_profile_slug = Some("express".to_string());
    cart.line_items[0].shipping_profile_slug = Some("parcel".to_string());
    assert!(validate_variant_line(cart.tenant_id, &cart.line_items[0], &product, None).is_err());
    cart.line_items[0].shipping_profile_slug = Some("express".to_string());
    validate_variant_line(cart.tenant_id, &cart.line_items[0], &product, None).unwrap();
    product.published_at = None;
    assert!(validate_variant_line(cart.tenant_id, &cart.line_items[0], &product, None).is_err());
}

#[test]
fn channel_visibility_and_availability_response_correlation_refuse_mismatches() {
    let (cart, mut product) = fixture();
    product.metadata = json!({"channel_visibility": {"allowed_channel_slugs": ["web"]}});
    assert!(validate_variant_line(cart.tenant_id, &cart.line_items[0], &product, None).is_err());
    validate_variant_line(cart.tenant_id, &cart.line_items[0], &product, Some("web")).unwrap();
    let id = cart.line_items[0].variant_id.unwrap();
    let mut observation = rustok_inventory::InventoryAvailabilitySnapshot {
        variant_id: id,
        requested_quantity: 5,
        available: true,
    };
    validate_inventory_observation(&observation, id, 5).unwrap();
    observation.requested_quantity = 4;
    assert!(validate_inventory_observation(&observation, id, 5).is_err());
    observation.requested_quantity = 5;
    observation.variant_id = Uuid::new_v4();
    assert!(validate_inventory_observation(&observation, id, 5).is_err());
}

#[test]
fn guest_delegation_never_fabricates_a_nil_user_actor() {
    let guest = checkout_actor(Uuid::nil());
    assert_eq!(guest.kind, rustok_api::PortActorKind::Service);
    let id = Uuid::from_u128(12);
    assert_eq!(
        checkout_actor(id),
        rustok_api::PortActor::user(id.to_string())
    );
}
