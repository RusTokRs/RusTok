#[test]
fn marketplace_listing_external_events_use_a_sealed_typed_contract() {
    let contract = include_str!("../../../crates/rustok-events/src/contract.rs");
    let listing = include_str!("../../../crates/rustok-events/src/marketplace_listing.rs");
    let outbox = include_str!("../../../crates/rustok-outbox/src/transactional.rs");

    for marker in [
        "pub(crate) mod sealed",
        "pub trait EventContract:",
        "pub struct ContractEventEnvelope",
    ] {
        assert!(
            contract.contains(marker),
            "missing sealed contract marker {marker}"
        );
    }

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
        assert!(listing.contains(variant), "missing {variant}");
        assert!(
            listing.contains(event_type),
            "missing event type {event_type}"
        );
    }

    for forbidden in ["note:", "reason:", "metadata:"] {
        assert!(
            !listing.contains(forbidden),
            "external listing event contract leaked {forbidden}"
        );
    }

    assert!(outbox.contains("publish_contract_in_tx"));
    assert!(outbox.contains("E: EventContract"));
    assert!(outbox.contains("write_contract_to_outbox"));
}
