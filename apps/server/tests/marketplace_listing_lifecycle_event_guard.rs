#[test]
fn marketplace_listing_owner_lifecycle_writes_are_evented() {
    let lifecycle = include_str!(
        "../../../crates/rustok-marketplace-listing/src/lifecycle_event_commands.rs"
    );
    let moderation = include_str!(
        "../../../crates/rustok-marketplace-listing/src/evented_commands.rs"
    );
    let storage = include_str!(
        "../../../crates/rustok-marketplace-listing/src/listing_events.rs"
    );
    let ports = include_str!("../../../crates/rustok-marketplace-listing/src/ports.rs");
    let lib = include_str!("../../../crates/rustok-marketplace-listing/src/lib.rs");

    assert!(lib.contains("mod lifecycle_event_commands;"));
    for marker in [
        "update_terms_evented",
        "submit_for_review_evented",
        "archive_listing_evented",
        "MarketplaceListingEventKind::TermsUpdated",
        "MarketplaceListingEventKind::SubmittedForReview",
        "MarketplaceListingEventKind::Archived",
        "\"locale\": locale.clone()",
        "append_listing_event(",
        "complete(receipt, &response).await",
        "rollback(receipt, error).await",
    ] {
        assert!(
            lifecycle.contains(marker),
            "listing lifecycle event executor is missing {marker}"
        );
    }
    for marker in [
        "review_listing_evented",
        "suspend_listing_evented",
        "MarketplaceListingEventKind::Approved",
        "MarketplaceListingEventKind::Rejected",
        "MarketplaceListingEventKind::Suspended",
        "\"locale\": locale.clone()",
        "append_listing_event(",
    ] {
        assert!(
            moderation.contains(marker),
            "listing moderation event executor is missing {marker}"
        );
    }
    for marker in [
        "normalize_locale_tag",
        "limit.clamp(1, MAX_EVENTS_PER_READ)",
        "order_by_desc(listing_event::Column::CreatedAt)",
        "order_by_desc(listing_event::Column::Id)",
    ] {
        assert!(storage.contains(marker), "listing event storage is missing {marker}");
    }

    for marker in [
        "self.update_terms_evented(context, request)",
        "self.submit_for_review_evented(context, request.listing_id)",
        "self.review_listing_evented(context, request)",
        "self.suspend_listing_evented(context, request)",
        "self.archive_listing_evented(context, request.listing_id)",
    ] {
        assert!(ports.contains(marker), "listing FBA routing is missing {marker}");
    }
    for forbidden in [
        "self.update_terms(context, request)",
        "self.submit_for_review(context, request.listing_id)",
        "self.review_listing(context, request)",
        "self.suspend_listing(context, request)",
        "self.archive_listing(context, request.listing_id)",
    ] {
        assert!(
            !ports.contains(forbidden),
            "listing FBA must not bypass immutable events through {forbidden}"
        );
    }
}
