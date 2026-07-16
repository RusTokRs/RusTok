use std::path::Path;

#[test]
fn marketplace_family_names_and_ownership_stay_explicit() {
    let root_manifest = include_str!(
        "../../../crates/rustok-marketplace/rustok-module.toml"
    );
    let seller_manifest = include_str!(
        "../../../crates/rustok-marketplace-seller/rustok-module.toml"
    );
    let modules_manifest = include_str!("../../../modules.toml");
    let workspace = include_str!("../../../Cargo.toml");
    let root_source = include_str!(
        "../../../crates/rustok-marketplace/src/lib.rs"
    );
    let root_consumer = include_str!(
        "../../../crates/rustok-marketplace/src/seller_directory.rs"
    );

    for marker in [
        "rustok-marketplace",
        "rustok-marketplace-seller",
        "rustok-marketplace-seller-admin",
        "marketplace_seller",
    ] {
        assert!(
            workspace.contains(marker) || modules_manifest.contains(marker),
            "marketplace family registration is missing {marker}"
        );
    }
    assert!(root_manifest.contains("slug = \"marketplace\""));
    assert!(seller_manifest.contains("slug = \"marketplace_seller\""));
    assert!(seller_manifest.contains("leptos_crate = \"rustok-marketplace-seller-admin\""));
    assert!(seller_manifest.contains("registry = \"contracts/marketplace-seller-fba-registry.json\""));

    for forbidden in [
        "crates/rustok-seller",
        "crates/rustok-offer",
        "crates/rustok-listing",
        "crates/rustok-commission",
        "crates/rustok-ledger",
        "crates/rustok-payout",
    ] {
        assert!(
            !workspace.contains(forbidden) && !modules_manifest.contains(forbidden),
            "marketplace capability must preserve family prefix: {forbidden}"
        );
    }

    assert!(root_source.contains("MARKETPLACE_FAMILY_MODULES"));
    assert!(root_source.contains("MarketplaceSellerDirectoryService"));
    assert!(root_consumer.contains("Arc<dyn MarketplaceSellerReadPort>"));
    assert!(!root_consumer.contains("sea_orm"));
    assert!(!root_consumer.contains("entities::"));
    assert!(!Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../crates/rustok-marketplace/src/entities")
        .exists());
    assert!(!Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../crates/rustok-marketplace/src/migrations")
        .exists());
}

#[test]
fn marketplace_seller_owner_and_ports_preserve_contracts() {
    let migration = include_str!(
        "../../../crates/rustok-marketplace-seller/src/migrations/m20260716_000001_create_marketplace_sellers.rs"
    );
    let service = include_str!(
        "../../../crates/rustok-marketplace-seller/src/service.rs"
    );
    let ports = include_str!(
        "../../../crates/rustok-marketplace-seller/src/ports.rs"
    );
    let registry = include_str!(
        "../../../crates/rustok-marketplace-seller/contracts/marketplace-seller-fba-registry.json"
    );
    let admin_core = include_str!(
        "../../../crates/rustok-marketplace-seller/admin/src/core.rs"
    );
    let admin_transport = include_str!(
        "../../../crates/rustok-marketplace-seller/admin/src/transport.rs"
    );
    let admin_ui = include_str!(
        "../../../crates/rustok-marketplace-seller/admin/src/ui/leptos.rs"
    );

    for marker in [
        "marketplace_sellers",
        "marketplace_seller_members",
        "ux_marketplace_sellers_tenant_handle",
        "ux_marketplace_seller_members_scope_user",
        "fk_marketplace_seller_members_tenant_seller",
    ] {
        assert!(migration.contains(marker), "seller schema is missing {marker}");
    }
    for marker in [
        "self.db.begin().await?",
        "MarketplaceSellerMemberRole::Owner",
        "MarketplaceSellerMemberStatus::Active",
        "owner membership role cannot be changed",
        "owner membership cannot be disabled",
        "MarketplaceSellerOnboardingStatus::Submitted",
        "MarketplaceSellerStatus::Suspended",
    ] {
        assert!(service.contains(marker), "seller service is missing {marker}");
    }
    for marker in [
        "pub trait MarketplaceSellerReadPort",
        "pub trait MarketplaceSellerCommandPort",
        "PortCallPolicy::read()",
        "PortCallPolicy::write()",
        "port.idempotency_key_required",
        "marketplace seller storage is temporarily unavailable",
    ] {
        assert!(ports.contains(marker), "seller FBA port is missing {marker}");
    }
    assert!(!ports.contains("storage unavailable: {error}"));
    assert!(registry.contains("\"status\": \"in_progress\""));
    assert!(registry.contains("durable command receipts are not yet implemented"));

    assert!(admin_core.contains("MarketplaceSellerAdminTransportProfile"));
    assert!(admin_core.contains("Graphql"));
    assert!(admin_transport.contains("transport_unmounted"));
    assert!(admin_transport.contains("never falls back"));
    assert!(admin_ui.contains("pub fn MarketplaceSellerAdmin()"));
}
