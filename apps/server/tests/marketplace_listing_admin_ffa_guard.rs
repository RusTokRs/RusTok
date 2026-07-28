#[test]
fn marketplace_listing_admin_ffa_is_module_owned_and_transport_explicit() {
    let workspace = include_str!("../../../Cargo.toml");
    let admin_host = include_str!("../../admin/Cargo.toml");
    let permissions = include_str!("../../../crates/rustok-api/src/permissions.rs");
    let owner = include_str!("../../../crates/rustok-marketplace-listing/src/lib.rs");
    let owner_ports = include_str!("../../../crates/rustok-marketplace-listing/src/ports.rs");
    let owner_graphql = include_str!("../../../crates/rustok-marketplace-listing/src/graphql.rs");
    let seller_graphql = include_str!("../../../crates/rustok-marketplace-seller/src/graphql.rs");
    let seller_ports = include_str!("../../../crates/rustok-marketplace-seller/src/ports.rs");
    let seller_manifest =
        include_str!("../../../crates/rustok-marketplace-seller/rustok-module.toml");
    let api_runtime = include_str!("../../../crates/rustok-api/src/runtime.rs");
    let manifest = include_str!("../../../crates/rustok-marketplace-listing/rustok-module.toml");
    let server_runtime = include_str!("../src/services/commerce_provider_runtime.rs");
    let server_manifest = include_str!("../Cargo.toml");
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
        "[provides.graphql]",
        "graphql::MarketplaceListingQuery",
        "graphql::MarketplaceListingMutation",
        "graphql::graphql_runtime_data",
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
        "MarketplaceListingReadPort::list_listings",
        "MarketplaceListingReadPort::list_listing_events",
        "MarketplaceListingCommandPort::create_listing",
        "MarketplaceListingCommandPort::archive_listing",
        "action.permission()",
        "use_context::<HostRuntimeContext>()",
        "shared_get::<rustok_marketplace_listing::MarketplaceListingRuntime>()",
        "leptos_axum::extract::<AuthContext>()",
        "leptos_axum::extract::<TenantContext>()",
        "leptos_axum::extract::<RequestContext>()",
        "has_effective_permission",
        "request.user_id != Some(auth.user_id)",
        "is_tenant_module_enabled(host.db(), tenant.id, \"marketplace_listing\")",
    ] {
        assert!(
            native.contains(marker),
            "listing native adapter is missing {marker}"
        );
    }
    assert!(!native.contains("entities::"));
    assert!(!native.contains("DatabaseConnection"));
    assert!(!native.contains("MarketplaceListingService::new"));

    for marker in [
        "marketplaceListings",
        "marketplaceListingEvents",
        "createMarketplaceListing",
        "updateMarketplaceListingTerms",
        "submitMarketplaceListingForReview",
        "reviewMarketplaceListing",
        "publishMarketplaceListing",
        "suspendMarketplaceListing",
        "reactivateMarketplaceListing",
        "archiveMarketplaceListing",
        "execute_graphql",
    ] {
        assert!(
            graphql.contains(marker),
            "listing GraphQL admin adapter is missing {marker}"
        );
    }
    assert!(!graphql.contains("UNMOUNTED"));

    for marker in [
        "pub trait MarketplaceListingPorts",
        "pub struct MarketplaceListingRuntime",
        "Arc<dyn MarketplaceListingPorts>",
    ] {
        assert!(
            owner_ports.contains(marker),
            "listing owner runtime is missing {marker}"
        );
    }
    for marker in [
        "pub struct MarketplaceListingQuery",
        "pub struct MarketplaceListingMutation",
        "MarketplaceListingReadPort::list_listings",
        "MarketplaceListingReadPort::list_listing_events",
        "MarketplaceListingCommandPort::create_listing",
        "MarketplaceListingCommandPort::archive_listing",
        "graphql_runtime_data",
        "RequestContext",
        "require_module_enabled(ctx, MODULE_SLUG).await",
    ] {
        assert!(
            owner_graphql.contains(marker),
            "listing owner GraphQL transport is missing {marker}"
        );
    }
    for marker in [
        "MarketplaceListingRuntime",
        "MarketplaceSellerRuntime",
        "shared_read_port",
        "ProductCatalogReadPort",
        "server.shared_insert(runtime.clone())",
    ] {
        assert!(
            server_runtime.contains(marker),
            "listing host runtime composition is missing {marker}"
        );
    }
    assert!(server_manifest.contains("rustok-marketplace-listing/graphql"));
    assert!(api_runtime.contains("pub async fn is_tenant_module_enabled"));
    assert!(transport.contains("pub locale: Option<String>"));
    assert!(graphql.contains("tenant_slug,\n        locale,"));
    assert!(
        owner_graphql
            .contains("require_permissions(ctx, &[Permission::MARKETPLACE_LISTINGS_LIST]).await")
    );
    assert!(seller_ports.contains("pub struct MarketplaceSellerRuntime"));
    assert!(seller_graphql.contains("graphql_runtime_data"));
    assert!(seller_graphql.contains("require_module_enabled(ctx, MODULE_SLUG).await"));
    assert!(!seller_graphql.contains("MarketplaceSellerService"));
    assert!(!seller_graphql.contains("sea_orm::DatabaseConnection"));
    assert!(seller_manifest.contains("runtime_data_factory = \"graphql::graphql_runtime_data\""));

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
