#[test]
fn marketplace_listing_admin_ffa_is_module_owned_and_transport_explicit() {
    let workspace = include_str!("../../../Cargo.toml");
    let admin_host = include_str!("../../admin/Cargo.toml");
    let permissions = include_str!("../../../crates/rustok-api/src/permissions.rs");
    let owner = include_str!("../../../crates/rustok-marketplace-listing/src/lib.rs");
    let manifest = include_str!("../../../crates/rustok-marketplace-listing/rustok-module.toml");
    let cargo = include_str!("../../../crates/rustok-marketplace-listing/admin/Cargo.toml");
    let model = include_str!("../../../crates/rustok-marketplace-listing/admin/src/model.rs");
    let transport =
        include_str!("../../../crates/rustok-marketplace-listing/admin/src/transport.rs");
    let native = include_str!(
        "../../../crates/rustok-marketplace-listing/admin/src/transport/native_server_adapter.rs"
    );
    let graphql = include_str!(
        "../../../crates/rustok-marketplace-listing/admin/src/transport/graphql_adapter.rs"
    );
    let ui = include_str!("../../../crates/rustok-marketplace-listing/admin/src/ui/leptos.rs");

    for marker in [
        "ui_classification = \"admin_only\"",
        "rustok-marketplace-listing-admin",
        "route_segment = \"marketplace-listings\"",
        "supported_locales = [\"en\", \"ru\"]",
    ] {
        assert!(
            manifest.contains(marker),
            "listing manifest is missing {marker}"
        );
    }
    assert!(cargo.contains("rustok-marketplace-listing = { path = \"..\", optional = true }"));
    assert!(workspace.contains("\"crates/rustok-marketplace-listing/admin\""));
    assert!(workspace.contains(
        "rustok-marketplace-listing-admin = { path = \"crates/rustok-marketplace-listing/admin\" }"
    ));
    for marker in [
        "rustok-marketplace-listing-admin/hydrate",
        "rustok-marketplace-listing-admin/ssr",
        "rustok-marketplace-listing-admin = { path = \"../../crates/rustok-marketplace-listing/admin\"",
    ] {
        assert!(
            admin_host.contains(marker),
            "admin host is missing {marker}"
        );
    }

    for marker in [
        "MarketplaceListings",
        "marketplace_listings",
        "MARKETPLACE_LISTINGS_CREATE",
        "MARKETPLACE_LISTINGS_READ",
        "MARKETPLACE_LISTINGS_UPDATE",
        "MARKETPLACE_LISTINGS_LIST",
        "MARKETPLACE_LISTINGS_MANAGE",
        "MARKETPLACE_LISTINGS_PUBLISH",
        "MARKETPLACE_LISTINGS_MODERATE",
    ] {
        assert!(
            permissions.contains(marker),
            "platform RBAC is missing {marker}"
        );
        assert!(
            owner.contains(marker)
                || marker == "MarketplaceListings"
                || marker == "marketplace_listings",
            "listing owner permission declaration is missing {marker}"
        );
    }

    for marker in [
        "MarketplaceListingAdminDetail",
        "MarketplaceListingAdminEvent",
        "MarketplaceListingAdminCommand",
        "MarketplaceListingAdminAction",
        "pub const fn permission",
        "legacy_snapshot",
        "has_unknown_attribution",
    ] {
        assert!(
            model.contains(marker),
            "listing admin model is missing {marker}"
        );
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
        "action.permission()",
        "marketplace listing native runtime is not mounted in this host",
        "use_context::<MarketplaceListingAdminNativeRuntime>()",
        "port_context",
        "authorize",
    ] {
        assert!(
            native.contains(marker),
            "listing native adapter is missing {marker}"
        );
    }
    assert!(!native.contains("expect_context::<MarketplaceListingAdminNativeRuntime>"));
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
