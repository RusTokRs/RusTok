use crate::CommerceModule;
use crate::error::CommerceError;
use crate::storefront_channel::normalize_public_channel_slug;
use crate::storefront_shipping::{
    effective_shipping_profile_slug, normalize_shipping_profile_slug,
    shipping_profile_slug_from_product_metadata,
};
use rustok_core::{MigrationSource, RusToKModule};
use serde_json::json;
use uuid::Uuid;

#[test]
fn crate_api_defines_minimal_contract_sections() {
    let api = include_str!("../CRATE_API.md");
    for marker in [
        "## Platform Capability Bindings",
        "## Domain Collaborations",
        "## Minimum Contract Set",
        "### Input DTOs/Commands",
        "### Domain Invariants",
        "### Events / Outbox Side Effects",
        "### Errors / Failure Codes",
    ] {
        assert!(
            api.contains(marker),
            "CRATE_API.md must contain section: {marker}"
        );
    }
}

#[test]
fn commerce_module_metadata_contract() {
    let module = CommerceModule;
    assert_eq!(module.slug(), "commerce");
    assert_eq!(module.name(), "Ecommerce");
    assert_eq!(
        module.description(),
        "Ecommerce umbrella/root module for the commerce family and orchestration surface"
    );
    assert_eq!(module.version(), env!("CARGO_PKG_VERSION"));
    assert_eq!(
        module.dependencies(),
        [
            "tenant",
            "cart",
            "customer",
            "product",
            "region",
            "pricing",
            "inventory",
            "order",
            "payment",
            "fulfillment",
        ]
    );

    let migrations = module.migrations();
    assert!(
        !migrations.is_empty(),
        "CommerceModule must expose non-empty umbrella migrations"
    );
}

#[test]
fn commerce_error_variants_and_display() {
    let err = CommerceError::Validation("invalid payload".into());
    assert_eq!(format!("{err}"), "Validation error: invalid payload");

    let product_id = Uuid::new_v4();
    let err = CommerceError::product_not_found(product_id);
    assert_eq!(format!("{err}"), format!("Product not found: {product_id}"));

    let variant_id = Uuid::new_v4();
    let err = CommerceError::variant_not_found(variant_id);
    assert_eq!(format!("{err}"), format!("Variant not found: {variant_id}"));

    let err = CommerceError::duplicate_handle("summer-shirt", "en");
    assert_eq!(
        format!("{err}"),
        "Duplicate handle: summer-shirt already exists for locale en"
    );

    let err = CommerceError::duplicate_sku("SKU-100");
    assert_eq!(format!("{err}"), "Duplicate SKU: SKU-100");

    let err = CommerceError::InvalidPrice("negative price".into());
    assert_eq!(format!("{err}"), "Invalid price: negative price");

    let err = CommerceError::InsufficientInventory {
        requested: 10,
        available: 3,
    };
    assert_eq!(
        format!("{err}"),
        "Insufficient inventory: requested 10, available 3"
    );

    let err = CommerceError::InvalidOptionCombination;
    assert_eq!(format!("{err}"), "Invalid option combination");

    let err = CommerceError::NoVariants;
    assert_eq!(format!("{err}"), "Product must have at least one variant");

    let err = CommerceError::CannotDeletePublished;
    assert_eq!(format!("{err}"), "Cannot delete published product");
}

#[test]
fn storefront_channel_slug_normalization() {
    assert_eq!(
        normalize_public_channel_slug(Some("  WEB-Store  ")),
        Some("web-store".to_string())
    );
    assert_eq!(
        normalize_public_channel_slug(Some("mobile")),
        Some("mobile".to_string())
    );
    assert_eq!(normalize_public_channel_slug(Some("   ")), None);
    assert_eq!(normalize_public_channel_slug(Some("")), None);
    assert_eq!(normalize_public_channel_slug(None), None);
}

#[test]
fn effective_shipping_profile_hierarchy() {
    // 1. Variant profile slug takes precedence over everything
    let profile = effective_shipping_profile_slug(
        Some("product-profile"),
        &json!({"shipping_profile": {"slug": "meta-profile"}}),
        Some("variant-heavy"),
    );
    assert_eq!(profile, "variant-heavy");

    // 2. Product profile slug takes precedence when variant slug is None or empty
    let profile = effective_shipping_profile_slug(
        Some("product-profile"),
        &json!({"shipping_profile": {"slug": "meta-profile"}}),
        None,
    );
    assert_eq!(profile, "product-profile");

    let profile = effective_shipping_profile_slug(
        Some("product-profile"),
        &json!({"shipping_profile": {"slug": "meta-profile"}}),
        Some("   "),
    );
    assert_eq!(profile, "product-profile");

    // 3. Metadata profile slug is used when product and variant slugs are absent
    let profile = effective_shipping_profile_slug(
        None,
        &json!({"shipping_profile": {"slug": "meta-profile"}}),
        None,
    );
    assert_eq!(profile, "meta-profile");

    // 4. Flat metadata shipping_profile_slug format
    let profile = effective_shipping_profile_slug(
        None,
        &json!({"shipping_profile_slug": "flat-meta-profile"}),
        None,
    );
    assert_eq!(profile, "flat-meta-profile");

    // 5. Defaults to "default" when nothing is specified
    let profile = effective_shipping_profile_slug(None, &json!({}), None);
    assert_eq!(profile, "default");
}

#[test]
fn shipping_profile_slug_normalization_rules() {
    assert_eq!(
        normalize_shipping_profile_slug("  Expedited-Overnight  "),
        Some("expedited-overnight".to_string())
    );
    assert_eq!(normalize_shipping_profile_slug("   "), None);
    assert_eq!(normalize_shipping_profile_slug(""), None);

    let empty_meta = json!({});
    assert_eq!(
        shipping_profile_slug_from_product_metadata(&empty_meta),
        "default"
    );
}
