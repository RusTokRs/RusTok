#[test]
fn marketplace_listing_schema_preserves_owner_and_version_boundaries() {
    let migration = include_str!(
        "../../../crates/rustok-marketplace-listing/src/migrations/m20260716_000001_create_marketplace_listings.rs"
    );
    let service = include_str!(
        "../../../crates/rustok-marketplace-listing/src/service.rs"
    );
    let receipt = include_str!(
        "../../../crates/rustok-marketplace-listing/src/command_receipts.rs"
    );
    let replay_safe = include_str!(
        "../../../crates/rustok-marketplace-listing/src/replay_safe_commands.rs"
    );
    let ports = include_str!(
        "../../../crates/rustok-marketplace-listing/src/ports.rs"
    );
    let registry = include_str!(
        "../../../crates/rustok-marketplace-listing/contracts/marketplace-listing-fba-registry.json"
    );

    for marker in [
        "marketplace_listings",
        "marketplace_listing_terms",
        "marketplace_listing_command_receipts",
        "uq_marketplace_listings_scope",
        "uq_marketplace_listings_seller_sku",
        "uq_marketplace_listing_terms_version",
        "fk_marketplace_listing_terms_tenant_listing",
        "uq_marketplace_listing_command_receipt_key",
    ] {
        assert!(migration.contains(marker), "listing schema is missing {marker}");
    }
    for forbidden in [
        "foreign_key(ForeignKey::create().name(\"fk_marketplace_listings_seller",
        "fk_marketplace_listings_product",
        "fk_marketplace_listing_terms_pricing",
        "fk_marketplace_listing_terms_inventory",
    ] {
        assert!(
            !migration.contains(forbidden),
            "listing schema must not add cross-module FK {forbidden}"
        );
    }

    for marker in [
        "Arc<dyn MarketplaceSellerReadPort>",
        "Arc<dyn ProductCatalogReadPort>",
        "read_variant_product_projection",
        "MarketplaceSellerStatus::Active",
        "current_terms_version",
        "listing_not_active",
        "listing_not_approved",
        "pricing_reference_missing",
        "inventory_reference_missing",
        "seller_not_active",
        "seller_unavailable",
        "order_by_asc(listing::Column::SellerId)",
    ] {
        assert!(service.contains(marker), "listing service is missing {marker}");
    }
    assert!(!service.contains("rustok_marketplace_seller::entities"));
    assert!(!service.contains("rustok_product::entities"));
    assert!(!service.contains("buy_box"));
    assert!(!service.contains("rank_score"));

    for marker in [
        "canonical_json",
        "Sha256::digest",
        "replay_existing",
        "STATUS_COMPLETED",
        "transaction.commit().await?",
        "IdempotencyConflict",
    ] {
        assert!(receipt.contains(marker), "listing receipt is missing {marker}");
    }
    for marker in [
        "create_listing_replay_safe",
        "publish_listing_replay_safe",
        "reactivate_listing_replay_safe",
        "replay_existing",
        "self.create_listing(context, input).await",
    ] {
        assert!(replay_safe.contains(marker), "replay-safe path is missing {marker}");
    }

    for marker in [
        "pub trait MarketplaceListingReadPort",
        "pub trait MarketplaceListingCommandPort",
        "create_listing_replay_safe",
        "publish_listing_replay_safe",
        "reactivate_listing_replay_safe",
        "marketplace listing storage is temporarily unavailable",
    ] {
        assert!(ports.contains(marker), "listing ports are missing {marker}");
    }
    assert!(!ports.contains("storage unavailable: {error}"));

    assert!(registry.contains("\"status\": \"in_progress\""));
    assert!(registry.contains("\"canonical_product_content_copied\": false"));
    assert!(registry.contains("\"cross_module_foreign_keys\": false"));
    assert!(registry.contains("\"buy_box_ranking_owned\": false"));
    assert!(registry.contains("\"atomic_with_owner_write\": true"));
    assert!(registry.contains("lost_response_replay_returns_saved_result"));
}

#[test]
fn marketplace_root_consumes_listing_projection_without_owner_internals() {
    let root = include_str!("../../../crates/rustok-marketplace/src/lib.rs");
    let consumer = include_str!(
        "../../../crates/rustok-marketplace/src/listing_directory.rs"
    );
    let root_manifest = include_str!("../../../crates/rustok-marketplace/rustok-module.toml");
    let modules = include_str!("../../../modules.toml");

    assert!(root.contains("MarketplaceListingDirectoryService"));
    assert!(consumer.contains("Arc<dyn MarketplaceListingReadPort>"));
    assert!(consumer.contains("list_eligibility"));
    assert!(!consumer.contains("sea_orm"));
    assert!(!consumer.contains("entities::"));
    assert!(root_manifest.contains("marketplace_listing"));
    assert!(root_manifest.contains("MarketplaceListingReadPort") || root_manifest.contains("providers = [\"marketplace_seller\", \"marketplace_listing\"]"));
    assert!(modules.contains("marketplace_listing ="));

    let default_enabled = modules
        .split("default_enabled =")
        .nth(1)
        .unwrap_or_default();
    assert!(!default_enabled.contains("marketplace_listing"));
    assert!(!default_enabled.contains("\"marketplace\""));
}
