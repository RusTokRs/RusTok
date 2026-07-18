#[test]
fn marketplace_listing_schema_preserves_owner_and_version_boundaries() {
    let lib = include_str!("../../../crates/rustok-marketplace-listing/src/lib.rs");
    let listing_entity =
        include_str!("../../../crates/rustok-marketplace-listing/src/entities/listing.rs");
    let terms_entity =
        include_str!("../../../crates/rustok-marketplace-listing/src/entities/listing_terms.rs");
    let event_entity =
        include_str!("../../../crates/rustok-marketplace-listing/src/entities/listing_event.rs");
    let migration = include_str!(
        "../../../crates/rustok-marketplace-listing/src/migrations/m20260716_000001_create_marketplace_listings.rs"
    );
    let event_migration = include_str!(
        "../../../crates/rustok-marketplace-listing/src/migrations/m20260717_000002_create_marketplace_listing_events.rs"
    );
    let service = include_str!("../../../crates/rustok-marketplace-listing/src/service.rs");
    let receipt =
        include_str!("../../../crates/rustok-marketplace-listing/src/command_receipts.rs");
    let provider_events =
        include_str!("../../../crates/rustok-marketplace-listing/src/replay_safe_commands.rs");
    let moderation_events =
        include_str!("../../../crates/rustok-marketplace-listing/src/evented_commands.rs");
    let lifecycle_events =
        include_str!("../../../crates/rustok-marketplace-listing/src/lifecycle_event_commands.rs");
    let ports = include_str!("../../../crates/rustok-marketplace-listing/src/ports.rs");
    let registry = include_str!(
        "../../../crates/rustok-marketplace-listing/contracts/marketplace-listing-fba-registry.json"
    );

    for marker in [
        "mod replay_safe_commands;",
        "mod evented_commands;",
        "mod lifecycle_event_commands;",
        "mod listing_events;",
    ] {
        assert!(lib.contains(marker), "listing crate is missing {marker}");
    }
    for marker in [
        "marketplace_listings",
        "marketplace_listing_terms",
        "MarketplaceListingCommandReceipts",
        "uq_marketplace_listings_scope",
        "uq_marketplace_listings_seller_sku",
        "uq_marketplace_listing_terms_version",
        "fk_marketplace_listing_terms_tenant_listing",
        "uq_marketplace_listing_command_receipt_key",
    ] {
        assert!(
            migration.contains(marker),
            "listing schema is missing {marker}"
        );
    }
    for marker in [
        "marketplace_listing_events",
        "fk_marketplace_listing_events_tenant_listing",
        "idx_marketplace_listing_events_timeline",
        "MarketplaceListingEvents::Locale",
        ".string_len(32)",
    ] {
        assert!(
            event_migration.contains(marker),
            "event schema is missing {marker}"
        );
    }
    for marker in [
        "table_name = \"marketplace_listing_events\"",
        "pub actor_id: Option<Uuid>",
        "pub event_kind: String",
        "pub locale: Option<String>",
        "pub provenance: String",
        "pub note: Option<String>",
    ] {
        assert!(
            event_entity.contains(marker),
            "event entity is missing {marker}"
        );
    }
    for forbidden in [
        "fk_marketplace_listings_seller",
        "fk_marketplace_listings_product",
        "fk_marketplace_listing_terms_pricing",
        "fk_marketplace_listing_terms_inventory",
    ] {
        assert!(
            !migration.contains(forbidden),
            "cross-module FK forbidden: {forbidden}"
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
                "localized product copy forbidden: {forbidden}"
            );
        }
    }

    for marker in [
        "event_bus: TransactionalEventBus",
        "Arc<dyn MarketplaceSellerReadPort>",
        "Arc<dyn ProductCatalogReadPort>",
        "pub(crate) fn event_bus(&self) -> &TransactionalEventBus",
        "seller_reader(&self)",
        "product_reader(&self)",
        "listing_not_active",
        "listing_not_approved",
        "pricing_reference_missing",
        "inventory_reference_missing",
        "seller_not_active",
        "seller_unavailable",
        "order_by_asc(listing::Column::SellerId)",
    ] {
        assert!(
            service.contains(marker),
            "listing read service is missing {marker}"
        );
    }
    for forbidden in [
        "pub async fn create_listing(",
        "pub async fn update_terms(",
        "pub async fn submit_for_review(",
        "pub async fn review_listing(",
        "pub async fn publish_listing(",
        "pub async fn suspend_listing(",
        "pub async fn reactivate_listing(",
        "pub async fn archive_listing(",
        "ListingCommandAdmission",
        "append_listing_event(",
        "OutboxTransport::new",
    ] {
        assert!(
            !service.contains(forbidden),
            "read service contains write bypass: {forbidden}"
        );
    }
    assert!(!service.contains("rustok_marketplace_seller::entities"));
    assert!(!service.contains("rustok_product::entities"));
    assert!(!service.contains("buy_box"));
    assert!(!service.contains("rank_score"));

    for marker in [
        "canonical_json",
        "Sha256::digest",
        "hex::encode",
        "replay_existing",
        "STATUS_COMPLETED",
        "event_bus: TransactionalEventBus",
        "publish_contract_in_tx(&transaction, tenant_id, Some(actor_id), event)",
        "transaction.commit().await?",
        "IdempotencyConflict",
    ] {
        assert!(
            receipt.contains(marker),
            "listing receipt is missing {marker}"
        );
    }
    assert!(
        !receipt.contains("OutboxTransport::new"),
        "listing receipt executor must not construct its event transport"
    );

    for source in [provider_events, moderation_events, lifecycle_events] {
        assert!(
            source.contains("self.event_bus().clone()"),
            "listing command path does not pass the injected event bus"
        );
    }
    for marker in [
        "create_listing_replay_safe",
        "publish_listing_replay_safe",
        "reactivate_listing_replay_safe",
        "replay_existing(",
        "seller_reader()",
        "product_reader()",
        "MarketplaceListingEventKind::Created",
        "MarketplaceListingEventKind::Published",
        "MarketplaceListingEventKind::Reactivated",
        "\"locale\": locale.clone()",
        "append_listing_event(",
        "complete(receipt, &response).await",
        "rollback(receipt, error).await",
    ] {
        assert!(
            provider_events.contains(marker),
            "provider event path is missing {marker}"
        );
    }
    for forbidden in [
        "self.create_listing(context, input).await",
        "self.publish_listing(context, listing_id).await",
        "self.reactivate_listing(context, listing_id).await",
    ] {
        assert!(
            !provider_events.contains(forbidden),
            "provider event path bypasses events: {forbidden}"
        );
    }

    for marker in [
        "pub trait MarketplaceListingReadPort",
        "pub trait MarketplaceListingCommandPort",
        "list_listing_events",
        "create_listing_replay_safe",
        "update_terms_evented",
        "submit_for_review_evented",
        "review_listing_evented",
        "publish_listing_replay_safe",
        "suspend_listing_evented",
        "reactivate_listing_replay_safe",
        "archive_listing_evented",
        "marketplace listing storage is temporarily unavailable",
    ] {
        assert!(ports.contains(marker), "listing ports are missing {marker}");
    }
    assert!(!ports.contains("storage unavailable: {error}"));

    for marker in [
        "\"status\": \"in_progress\"",
        "\"event_table\": \"marketplace_listing_events\"",
        "\"direct_write_methods_in_service\": false",
        "\"canonical_product_content_copied\": false",
        "\"localized_business_copy_owned\": false",
        "\"localized_business_copy_provider\": \"rustok-product\"",
        "\"cross_module_foreign_keys\": false",
        "\"buy_box_ranking_owned\": false",
        "\"atomic_with_owner_write\": true",
        "\"atomic_with_external_contract_event\": true",
        "\"receipt_executor_constructs_transport\": false",
        "\"event_bus_composition\": \"injected_through_marketplace_listing_service\"",
        "lost_response_replay_returns_saved_result",
    ] {
        assert!(
            registry.contains(marker),
            "listing registry is missing {marker}"
        );
    }
}

#[test]
fn marketplace_root_consumes_listing_projection_without_owner_internals() {
    let root = include_str!("../../../crates/rustok-marketplace/src/lib.rs");
    let consumer = include_str!("../../../crates/rustok-marketplace/src/listing_directory.rs");
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
