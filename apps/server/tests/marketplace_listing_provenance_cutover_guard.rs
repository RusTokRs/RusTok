#[test]
fn marketplace_listing_legacy_provenance_is_truthful_and_irreversible() {
    let listing =
        include_str!("../../../crates/rustok-marketplace-listing/src/entities/listing.rs");
    let event =
        include_str!("../../../crates/rustok-marketplace-listing/src/entities/listing_event.rs");
    let dto = include_str!("../../../crates/rustok-marketplace-listing/src/dto.rs");
    let storage = include_str!("../../../crates/rustok-marketplace-listing/src/listing_events.rs");
    let migration = include_str!(
        "../../../crates/rustok-marketplace-listing/src/migrations/m20260717_000003_backfill_listing_event_provenance.rs"
    );
    let migrations =
        include_str!("../../../crates/rustok-marketplace-listing/src/migrations/mod.rs");
    let registry = include_str!(
        "../../../crates/rustok-marketplace-listing/contracts/marketplace-listing-fba-registry.json"
    );

    for forbidden in ["pub approval_note:", "pub suspension_reason:"] {
        assert!(
            !listing.contains(forbidden),
            "listing aggregate retains {forbidden}"
        );
        assert!(
            !dto.contains(forbidden),
            "listing response retains {forbidden}"
        );
    }

    for marker in [
        "pub actor_id: Option<Uuid>",
        "pub locale: Option<String>",
        "pub provenance: String",
    ] {
        assert!(
            event.contains(marker),
            "listing event entity is missing {marker}"
        );
    }
    for marker in [
        "MarketplaceListingEventProvenance",
        "LegacyApprovalSnapshot",
        "LegacySuspensionSnapshot",
        "pub actor_id: Option<Uuid>",
        "pub locale: Option<String>",
        "pub provenance: MarketplaceListingEventProvenance",
    ] {
        assert!(
            dto.contains(marker),
            "listing event DTO is missing {marker}"
        );
    }

    for marker in [
        "actor_id: Set(Some(actor_id))",
        "locale: Set(Some(locale))",
        "MarketplaceListingEventProvenance::Command",
        "command listing event is missing actor or locale attribution",
        "legacy listing snapshot must not fabricate actor or locale attribution",
    ] {
        assert!(
            storage.contains(marker),
            "listing event storage is missing {marker}"
        );
    }

    for marker in [
        "ALTER COLUMN actor_id DROP NOT NULL",
        "ALTER COLUMN locale DROP NOT NULL",
        "ADD COLUMN provenance VARCHAR(32) NOT NULL DEFAULT 'command'",
        "ck_marketplace_listing_events_attribution",
        "legacy_approval_snapshot",
        "legacy_suspension_snapshot",
        "original_actor_known\": false",
        "original_locale_known\": false",
        "DROP COLUMN approval_note",
        "DROP COLUMN suspension_reason",
        "intentionally irreversible",
    ] {
        assert!(
            migration.contains(marker),
            "provenance migration is missing {marker}"
        );
    }
    for forbidden in [
        "Uuid::nil()",
        "default_locale",
        "PLATFORM_FALLBACK_LOCALE",
        "actor_id, event_kind, locale, provenance, note, metadata, created_at) VALUES ($1, $2, $3, $4",
    ] {
        assert!(
            !migration.contains(forbidden),
            "provenance migration fabricates attribution: {forbidden}"
        );
    }
    assert!(migrations.contains("m20260717_000003_backfill_listing_event_provenance"));

    for marker in [
        "\"legacy_snapshot\"",
        "\"actor_id_must_be_null\": true",
        "\"locale_must_be_null\": true",
        "\"fabricated_attribution_forbidden\": true",
        "\"compatibility_snapshot_columns_removed\"",
        "m20260717_000003_backfill_listing_event_provenance",
    ] {
        assert!(
            registry.contains(marker),
            "listing registry is missing {marker}"
        );
    }
}
