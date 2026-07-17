#[test]
fn marketplace_listing_admin_ffa_is_module_owned_and_transport_explicit() {
    let manifest = include_str!(
        "../../../crates/rustok-marketplace-listing/rustok-module.toml"
    );
    let cargo = include_str!(
        "../../../crates/rustok-marketplace-listing/admin/Cargo.toml"
    );
    let model = include_str!(
        "../../../crates/rustok-marketplace-listing/admin/src/model.rs"
    );
    let transport = include_str!(
        "../../../crates/rustok-marketplace-listing/admin/src/transport.rs"
    );
    let native = include_str!(
        "../../../crates/rustok-marketplace-listing/admin/src/transport/native_server_adapter.rs"
    );
    let graphql = include_str!(
        "../../../crates/rustok-marketplace-listing/admin/src/transport/graphql_adapter.rs"
    );
    let ui = include_str!(
        "../../../crates/rustok-marketplace-listing/admin/src/ui/leptos.rs"
    );

    for marker in [
        "ui_classification = \"admin_only\"",
        "rustok-marketplace-listing-admin",
        "route_segment = \"marketplace-listings\"",
        "supported_locales = [\"en\", \"ru\"]",
    ] {
        assert!(manifest.contains(marker), "listing manifest is missing {marker}");
    }
    assert!(cargo.contains("rustok-marketplace-listing = { path = \"..\", optional = true }"));

    for marker in [
        "MarketplaceListingAdminDetail",
        "MarketplaceListingAdminEvent",
        "MarketplaceListingAdminCommand",
        "legacy_snapshot",
        "has_unknown_attribution",
    ] {
        assert!(model.contains(marker), "listing admin model is missing {marker}");
    }

    assert!(transport.contains("execute_selected_transport"));
    assert!(transport.contains("MARKETPLACE_LISTING_TRANSPORT_FALLBACK_POLICY"));
    assert!(transport.contains("never falls back"));

    for marker in [
        "MarketplaceListingAdminPorts",
        "MarketplaceListingAdminRequestScope",
        "MarketplaceListingReadPort::list_listings",
        "MarketplaceListingReadPort::list_listing_events",
        "MarketplaceListingCommandPort::create_listing",
        "MarketplaceListingCommandPort::archive_listing",
        "port_context",
        "authorize",
    ] {
        assert!(native.contains(marker), "listing native adapter is missing {marker}");
    }
    assert!(!native.contains("entities::"));
    assert!(!native.contains("DatabaseConnection"));
    assert!(!native.contains("MarketplaceListingService::new"));

    assert!(graphql.contains("declared_unmounted"));
    assert!(graphql.contains("must provide module-owned listing queries and mutations"));
    assert!(!graphql.contains("fallback"));

    for marker in [
        "pending_command",
        "Retry same command",
        "idempotency_key",
        "load_marketplace_listing_directory",
        "load_marketplace_listing_detail",
        "Immutable history",
        "has_unknown_attribution",
    ] {
        assert!(ui.contains(marker), "listing admin UI is missing {marker}");
    }
}
