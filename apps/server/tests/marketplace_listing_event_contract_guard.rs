#[test]
fn marketplace_listing_external_event_contract_is_typed_and_safe() {
    let types = include_str!("../../../crates/rustok-events/src/types.rs");
    let schema = include_str!("../../../crates/rustok-events/src/schema.rs");
    let tests = include_str!("../../../crates/rustok-events/tests/canonical_contracts.rs");

    for (variant, event_type) in [
        ("MarketplaceListingCreated", "marketplace.listing.created"),
        (
            "MarketplaceListingTermsUpdated",
            "marketplace.listing.terms_updated",
        ),
        (
            "MarketplaceListingSubmittedForReview",
            "marketplace.listing.submitted_for_review",
        ),
        ("MarketplaceListingApproved", "marketplace.listing.approved"),
        ("MarketplaceListingRejected", "marketplace.listing.rejected"),
        (
            "MarketplaceListingPublished",
            "marketplace.listing.published",
        ),
        (
            "MarketplaceListingSuspended",
            "marketplace.listing.suspended",
        ),
        (
            "MarketplaceListingReactivated",
            "marketplace.listing.reactivated",
        ),
        ("MarketplaceListingArchived", "marketplace.listing.archived"),
    ] {
        assert!(
            types.contains(&format!("{variant} {{")),
            "missing {variant}"
        );
        assert!(
            types.contains(&format!("Self::{variant} {{ .. }}")) && types.contains(event_type),
            "missing event type mapping for {variant}"
        );
        assert!(
            types.contains(&format!("Self::{variant} {{ .. }} => 1")),
            "missing schema version for {variant}"
        );
        assert!(
            schema.contains(&format!("event_type: \"{event_type}\"")),
            "missing schema registry entry for {event_type}"
        );
    }

    for marker in [
        "validate_marketplace_listing_slug(\"market_slug\", market_slug)?",
        "validate_marketplace_listing_slug(\"channel_slug\", channel_slug)?",
        "marketplace_listing_events_reject_noncanonical_scope_and_invalid_terms_version",
        "marketplace_listing_external_payload_excludes_owner_notes_and_metadata",
    ] {
        assert!(
            types.contains(marker) || tests.contains(marker),
            "missing marketplace listing event contract guard marker {marker}"
        );
    }
}
