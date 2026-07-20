use std::path::Path;

#[test]
fn marketplace_family_names_and_ownership_stay_explicit() {
    let root_manifest = include_str!("../../../crates/rustok-marketplace/rustok-module.toml");
    let seller_manifest =
        include_str!("../../../crates/rustok-marketplace-seller/rustok-module.toml");
    let modules_manifest = include_str!("../../../modules.toml");
    let workspace = include_str!("../../../Cargo.toml");
    let root_source = include_str!("../../../crates/rustok-marketplace/src/lib.rs");
    let root_consumer = include_str!("../../../crates/rustok-marketplace/src/seller_directory.rs");

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
    assert!(
        seller_manifest.contains("registry = \"contracts/marketplace-seller-fba-registry.json\"")
    );

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
    assert!(
        !Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../crates/rustok-marketplace/src/entities")
            .exists()
    );
    assert!(
        !Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../crates/rustok-marketplace/src/migrations")
            .exists()
    );
}

#[test]
fn marketplace_seller_owner_and_ports_preserve_contracts() {
    let owner_migration = include_str!(
        "../../../crates/rustok-marketplace-seller/src/migrations/m20260716_000001_create_marketplace_sellers.rs"
    );
    let receipt_migration = include_str!(
        "../../../crates/rustok-marketplace-seller/src/migrations/m20260716_000002_create_seller_command_receipts.rs"
    );
    let seller_entity =
        include_str!("../../../crates/rustok-marketplace-seller/src/entities/seller.rs");
    let translation_entity = include_str!(
        "../../../crates/rustok-marketplace-seller/src/entities/seller_translation.rs"
    );
    let localized =
        include_str!("../../../crates/rustok-marketplace-seller/src/localized_sellers.rs");
    let service = include_str!("../../../crates/rustok-marketplace-seller/src/service.rs");
    let dto = include_str!("../../../crates/rustok-marketplace-seller/src/dto.rs");
    let ports = include_str!("../../../crates/rustok-marketplace-seller/src/ports.rs");
    let receipt_entity = include_str!(
        "../../../crates/rustok-marketplace-seller/src/entities/seller_command_receipt.rs"
    );
    let receipt_executor =
        include_str!("../../../crates/rustok-marketplace-seller/src/command_receipts.rs");
    let receipted_commands =
        include_str!("../../../crates/rustok-marketplace-seller/src/receipted_commands.rs");
    let registry = include_str!(
        "../../../crates/rustok-marketplace-seller/contracts/marketplace-seller-fba-registry.json"
    );
    let admin_core = include_str!("../../../crates/rustok-marketplace-seller/admin/src/core.rs");
    let admin_transport =
        include_str!("../../../crates/rustok-marketplace-seller/admin/src/transport.rs");
    let admin_ui = include_str!("../../../crates/rustok-marketplace-seller/admin/src/ui/leptos.rs");

    for marker in [
        "marketplace_sellers",
        "marketplace_seller_translations",
        "marketplace_seller_members",
        "ux_marketplace_sellers_tenant_handle",
        "ux_marketplace_seller_translations_locale",
        "idx_marketplace_seller_translations_search",
        "fk_marketplace_seller_translations_tenant_seller",
        "ux_marketplace_seller_members_scope_user",
        "fk_marketplace_seller_members_tenant_seller",
        "MarketplaceSellerTranslations::Locale",
        ".string_len(32)",
    ] {
        assert!(
            owner_migration.contains(marker),
            "seller multilingual schema is missing {marker}"
        );
    }
    assert!(translation_entity.contains("table_name = \"marketplace_seller_translations\""));
    assert!(translation_entity.contains("pub locale: String"));
    assert!(translation_entity.contains("pub display_name: String"));
    assert!(!seller_entity.contains("pub display_name:"));
    assert!(!owner_migration.contains("MarketplaceSellers::DisplayName"));

    for marker in [
        "marketplace_seller_command_receipts",
        "uq_marketplace_seller_command_receipt_key",
        "RequestHash",
        "ResponseJson",
        "CompletedAt",
    ] {
        assert!(
            receipt_migration.contains(marker) || receipt_entity.contains(marker),
            "seller receipt schema is missing {marker}"
        );
    }

    for marker in [
        "normalize_locale_tag",
        "Column::Locale.eq(locale.as_str())",
        "OnConflict::columns",
        "update_column(Alias::new(\"display_name\"))",
        "MISSING_TRANSLATION_PREFIX",
        "resolved_locale: translation.locale",
    ] {
        assert!(
            localized.contains(marker),
            "localized seller storage is missing {marker}"
        );
    }
    assert!(!localized.contains("build_locale_candidates"));
    assert!(!localized.contains("PLATFORM_FALLBACK_LOCALE"));
    assert!(service.contains("localized_seller_ids_for_search"));
    assert!(service.contains("owner membership role cannot be changed"));
    assert!(service.contains("owner membership cannot be disabled"));
    for forbidden in [
        "pub async fn create_seller(",
        "pub async fn update_profile(",
        "pub async fn submit_onboarding(",
        "pub async fn review_onboarding(",
        "pub async fn suspend_seller(",
        "pub async fn reactivate_seller(",
    ] {
        assert!(
            !service.contains(forbidden),
            "seller service must not expose a non-receipted write bypass: {forbidden}"
        );
    }
    assert!(dto.contains("pub resolved_locale: String"));

    for marker in [
        "pub trait MarketplaceSellerReadPort",
        "pub trait MarketplaceSellerCommandPort",
        "PortCallPolicy::read()",
        "PortCallPolicy::write()",
        "port.idempotency_key_required",
        "context.locale.as_str()",
        "create_seller_with_receipt",
        "update_profile_with_receipt",
        "submit_onboarding_with_receipt",
        "review_onboarding_with_receipt",
        "suspend_seller_with_receipt",
        "reactivate_seller_with_receipt",
        "add_member_with_receipt",
        "update_member_with_receipt",
        "marketplace_seller.translation_missing",
        "marketplace seller storage is temporarily unavailable",
    ] {
        assert!(
            ports.contains(marker),
            "seller FBA port is missing {marker}"
        );
    }
    assert!(!ports.contains("storage unavailable: {error}"));
    assert!(!ports.contains("self.create_seller(\n"));
    assert!(!ports.contains("self.update_profile(\n"));

    for marker in [
        "canonical_sha256",
        "command_request_hash",
        "CommandReceiptAdmission",
        "RECEIPT_STATUS_COMPLETED",
        "response_json",
        "transaction.commit().await?",
        "IdempotencyConflict",
    ] {
        assert!(
            receipt_executor.contains(marker) || registry.contains(marker),
            "seller receipt executor is missing {marker}"
        );
    }
    for marker in [
        "create_seller_with_receipt",
        "update_profile_with_receipt",
        "submit_onboarding_with_receipt",
        "review_onboarding_with_receipt",
        "suspend_seller_with_receipt",
        "reactivate_seller_with_receipt",
        "add_member_with_receipt",
        "update_member_with_receipt",
        "\"locale\": locale",
        "upsert_translation(",
        "complete_command(receipt",
        "rollback_command(receipt",
        "let policy_input = input.clone()",
    ] {
        assert!(
            receipted_commands.contains(marker),
            "seller localized receipted commands are missing {marker}"
        );
    }
    assert!(registry.contains("\"status\": \"in_progress\""));
    assert!(registry.contains("\"atomic_with_owner_write\": true"));
    assert!(registry.contains("lost_response_replay_returns_saved_result"));
    assert!(!registry.contains("durable command receipts are not yet implemented"));

    assert!(admin_core.contains("MarketplaceSellerAdminTransportProfile"));
    assert!(admin_core.contains("Graphql"));
    assert!(admin_transport.contains("transport_unmounted"));
    assert!(admin_transport.contains("never falls back"));
    assert!(admin_ui.contains("pub fn MarketplaceSellerAdmin()"));
}
