#[test]
fn marketplace_seller_graphql_and_admin_transports_share_owner_ports() {
    let owner_graphql = include_str!("../../../crates/rustok-marketplace-seller/src/graphql.rs");
    let owner_ports = include_str!("../../../crates/rustok-marketplace-seller/src/ports.rs");
    let native_adapter = include_str!(
        "../../../crates/rustok-marketplace-seller/admin/src/transport/native_server_adapter.rs"
    );
    let graphql_adapter = include_str!(
        "../../../crates/rustok-marketplace-seller/admin/src/transport/graphql_adapter.rs"
    );
    let transport =
        include_str!("../../../crates/rustok-marketplace-seller/admin/src/transport.rs");
    let admin_model = include_str!("../../../crates/rustok-marketplace-seller/admin/src/model.rs");
    let admin_ui = include_str!("../../../crates/rustok-marketplace-seller/admin/src/ui/leptos.rs");

    for marker in [
        "MarketplaceSellerReadPort::list_sellers",
        "MarketplaceSellerReadPort::read_seller",
        "MarketplaceSellerCommandPort::create_seller",
        "MarketplaceSellerCommandPort::update_seller_profile",
        "MarketplaceSellerCommandPort::submit_seller_onboarding",
        "MarketplaceSellerCommandPort::review_seller_onboarding",
        "MarketplaceSellerCommandPort::suspend_seller",
        "MarketplaceSellerCommandPort::reactivate_seller",
        "MarketplaceSellerCommandPort::add_seller_member",
        "MarketplaceSellerCommandPort::update_seller_member",
        "Permission::MARKETPLACE_SELLERS_MANAGE",
        "with_idempotency_key",
        "marketplace seller service is temporarily unavailable",
    ] {
        assert!(
            owner_graphql.contains(marker),
            "marketplace seller GraphQL transport is missing {marker}"
        );
    }
    assert!(!owner_graphql.contains("entities::"));
    assert!(!owner_graphql.contains("storage unavailable: {error}"));

    for marker in [
        "async fn list_members",
        "ListMarketplaceSellerMembersRequest",
        "PortCallPolicy::read()",
    ] {
        assert!(
            owner_ports.contains(marker),
            "marketplace seller read port is missing {marker}"
        );
    }

    for marker in [
        "marketplace_seller_directory_native",
        "marketplace_seller_detail_native",
        "marketplace_seller_command_native",
        "MarketplaceSellerReadPort::list_sellers",
        "MarketplaceSellerReadPort::list_members",
        "MarketplaceSellerCommandPort::create_seller",
        "MarketplaceSellerCommandPort::update_seller_member",
        "ensure_permission",
        "ensure_tenant",
        "idempotency_key",
    ] {
        assert!(
            native_adapter.contains(marker),
            "marketplace seller native adapter is missing {marker}"
        );
    }
    assert!(!native_adapter.contains("entities::"));

    for marker in [
        "marketplaceSellers",
        "marketplaceSellerMembers",
        "createMarketplaceSeller",
        "updateMarketplaceSellerProfile",
        "submitMarketplaceSellerOnboarding",
        "reviewMarketplaceSellerOnboarding",
        "suspendMarketplaceSeller",
        "reactivateMarketplaceSeller",
        "addMarketplaceSellerMember",
        "updateMarketplaceSellerMember",
        "idempotencyKey",
    ] {
        assert!(
            graphql_adapter.contains(marker),
            "marketplace seller GraphQL admin adapter is missing {marker}"
        );
    }

    assert!(admin_model.contains("pub enum MarketplaceSellerAdminCommand"));
    assert!(admin_model.contains("ReviewOnboarding"));
    assert!(admin_model.contains("UpdateMember"));
    assert!(transport.contains("execute_selected_transport"));
    assert!(transport.contains("MARKETPLACE_SELLER_TRANSPORT_FALLBACK_POLICY"));
    assert!(transport.contains("never falls back"));
    assert!(admin_ui.contains("pending_command"));
    assert!(admin_ui.contains("Retry same command"));
    assert!(admin_ui.contains("idempotency_key"));
    assert!(admin_ui.contains("load_marketplace_seller_directory"));
    assert!(admin_ui.contains("load_marketplace_seller_detail"));
}

#[test]
fn marketplace_seller_transport_is_manifest_and_feature_wired_without_default_enablement() {
    let module_manifest =
        include_str!("../../../crates/rustok-marketplace-seller/rustok-module.toml");
    let modules_manifest = include_str!("../../../modules.toml");
    let distribution_manifest = include_str!("../../../crates/rustok-distribution/Cargo.toml");
    let distribution_source = include_str!("../../../crates/rustok-distribution/src/lib.rs");
    let server_manifest = include_str!("../../../apps/server/Cargo.toml");
    let admin_manifest = include_str!("../../../apps/admin/Cargo.toml");

    assert!(module_manifest.contains("[provides.graphql]"));
    assert!(module_manifest.contains("graphql::MarketplaceSellerQuery"));
    assert!(module_manifest.contains("graphql::MarketplaceSellerMutation"));
    assert!(module_manifest.contains("rustok-marketplace-seller-admin"));

    assert!(distribution_manifest.contains("mod-marketplace_seller"));
    assert!(distribution_manifest.contains("mod-marketplace"));
    assert!(distribution_source.contains("rustok_marketplace_seller::MarketplaceSellerModule"));
    assert!(distribution_source.contains("rustok_marketplace::MarketplaceModule"));
    assert!(server_manifest.contains("rustok-marketplace-seller/graphql"));
    assert!(server_manifest.contains("rustok-distribution/mod-marketplace_seller"));
    assert!(admin_manifest.contains("rustok-marketplace-seller-admin/hydrate"));
    assert!(admin_manifest.contains("rustok-marketplace-seller-admin/ssr"));

    let default_enabled = modules_manifest
        .split("default_enabled =")
        .nth(1)
        .unwrap_or_default();
    assert!(
        !default_enabled.contains("marketplace_seller")
            && !default_enabled.contains("\"marketplace\""),
        "marketplace modules must not be default-enabled before retained runtime evidence"
    );
}
