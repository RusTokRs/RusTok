#[test]
fn marketplace_listing_schema_preserves_owner_and_version_boundaries() {
    let lib = include_str!("../../../crates/rustok-marketplace-listing/src/lib.rs");
    let listing_entity = include_str!(
        "../../../crates/rustok-marketplace-listing/src/entities/listing.rs"
    );
    let terms_entity = include_str!(
        "../../../crates/rustok-marketplace-listing/src/entities/listing_terms.rs"
    );
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

    assert!(lib.contains("mod replay_safe_commands;"));
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
    for source in [listing_entity, terms_entity] {
        for forbidden in [
            "pub title:",
            "pub description:",
            "pub localized_title:",
            "pub localized_description:",
            "pub translations_json:",
            "pub localized_fields_json:",
        ] {
            assert!(
                !source.contains(forbidden),
                "listing owner must not copy product localized content: {forbidden}"
            );
        }
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
        "replay_safe_lifecycle(&context",
        "self.create_listing(context, input).await",
    ] {
        assert!(replay_safe.contains(marker), "replay-safe path is missing {marker}");
    }
    assert!(!replay_safe.contains("map_or_else(\n                || async"));

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
    assert!(!ports.contains("self.create_listing(context, request)"));

    for marker in [
        "\"status\": \"in_progress\"",
        "\"canonical_product_content_copied\": false",
        "\"localized_business_copy_owned\": false",
        "\"localized_business_copy_provider\": \"rustok-product\"",
        "\"operator_prose_target\": \"immutable_marketplace_listing_events_with_actor_and_effective_locale\"",
        "\"cross_module_foreign_keys\": false",
        "\"buy_box_ranking_owned\": false",
        "\"atomic_with_owner_write\": true",
        "replay_checked_before_provider_reads",
        "lost_response_replay_returns_saved_result",
    ] {
        assert!(registry.contains(marker), "listing registry is missing {marker}");
    }
}

#[test]
fn marketplace_root_consumes_listing_projection_without_owner_internals() {
    let root = include_str!("../../../crates/rustok-marketplace/src/lib.rs");
    let consumer = include_str!(
        "../../../crates/rustok-marketplace/src/listing_directory.rs"
    );
    let root_manifest = include_str!("../../../crates/rustok-marketplace/rustok-module.toml");
    let modules = include_str!("../../../modules.toml");
    let distribution_manifest = include_str!("../../../crates/rustok-distribution/Cargo.toml");
    let distribution_source = include_str!("../../../crates/rustok-distribution/src/lib.rs");
    let server_manifest = include_str!("../../../apps/server/Cargo.toml");

    assert!(root.contains("MarketplaceListingDirectoryService"));
    assert!(consumer.contains("Arc<dyn MarketplaceListingReadPort>"));
    assert!(consumer.contains("list_eligibility"));
    assert!(!consumer.contains("sea_orm"));
    assert!(!consumer.contains("entities::"));
    assert!(root_manifest.contains("marketplace_listing"));
    assert!(root_manifest.contains("MarketplaceListingReadPort") || root_manifest.contains("providers = [\"marketplace_seller\", \"marketplace_listing\"]"));
    assert!(modules.contains("marketplace_listing ="));
    assert!(distribution_manifest.contains("mod-marketplace_listing"));
    assert!(distribution_manifest.contains("rustok-marketplace-listing"));
    assert!(distribution_source.contains("rustok_marketplace_listing::MarketplaceListingModule"));
    assert!(server_manifest.contains("mod-marketplace_listing"));
    assert!(server_manifest.contains("rustok-marketplace-listing"));

    let default_enabled = modules
        .split("default_enabled =")
        .nth(1)
        .unwrap_or_default();
    assert!(!default_enabled.contains("marketplace_listing"));
    assert!(!default_enabled.contains("\"marketplace\""));
    let server_defaults = server_manifest
        .split("default = [")
        .nth(1)
        .and_then(|value| value.split(']').next())
        .unwrap_or_default();
    assert!(!server_defaults.contains("mod-marketplace_listing"));
    assert!(!server_defaults.contains("mod-marketplace\""));
}
