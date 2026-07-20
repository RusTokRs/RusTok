#[test]
fn marketplace_listing_owner_writes_are_receipted_and_evented() {
    let provider =
        include_str!("../../../crates/rustok-marketplace-listing/src/replay_safe_commands.rs");
    let lifecycle =
        include_str!("../../../crates/rustok-marketplace-listing/src/lifecycle_event_commands.rs");
    let moderation =
        include_str!("../../../crates/rustok-marketplace-listing/src/evented_commands.rs");
    let storage = include_str!("../../../crates/rustok-marketplace-listing/src/listing_events.rs");
    let service = include_str!("../../../crates/rustok-marketplace-listing/src/service.rs");
    let ports = include_str!("../../../crates/rustok-marketplace-listing/src/ports.rs");
    let lib = include_str!("../../../crates/rustok-marketplace-listing/src/lib.rs");

    for marker in [
        "mod replay_safe_commands;",
        "mod lifecycle_event_commands;",
        "mod evented_commands;",
        "mod listing_events;",
    ] {
        assert!(lib.contains(marker), "listing crate is missing {marker}");
    }

    for marker in [
        "create_listing_replay_safe",
        "publish_listing_replay_safe",
        "reactivate_listing_replay_safe",
        "replay_existing(",
        "MarketplaceListingEventKind::Created",
        "MarketplaceListingEventKind::Published",
        "MarketplaceListingEventKind::Reactivated",
        "\"locale\": locale.clone()",
        "append_listing_event(",
        "complete(receipt, &response).await",
        "rollback(receipt, error).await",
    ] {
        assert!(
            provider.contains(marker),
            "provider event executor is missing {marker}"
        );
    }
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
            "lifecycle event executor is missing {marker}"
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
            "moderation event executor is missing {marker}"
        );
    }
    for marker in [
        "normalize_locale_tag",
        "limit.clamp(1, MAX_EVENTS_PER_READ)",
        "order_by_desc(listing_event::Column::CreatedAt)",
        "order_by_desc(listing_event::Column::Id)",
    ] {
        assert!(
            storage.contains(marker),
            "listing event storage is missing {marker}"
        );
    }

    for marker in [
        "self.create_listing_replay_safe(context, request)",
        "self.update_terms_evented(context, request)",
        "self.submit_for_review_evented(context, request.listing_id)",
        "self.review_listing_evented(context, request)",
        "self.publish_listing_replay_safe(context, request.listing_id)",
        "self.suspend_listing_evented(context, request)",
        "self.reactivate_listing_replay_safe(context, request.listing_id)",
        "self.archive_listing_evented(context, request.listing_id)",
    ] {
        assert!(
            ports.contains(marker),
            "listing FBA routing is missing {marker}"
        );
    }
    for forbidden in [
        "pub async fn create_listing(",
        "pub async fn update_terms(",
        "pub async fn submit_for_review(",
        "pub async fn review_listing(",
        "pub async fn publish_listing(",
        "pub async fn suspend_listing(",
        "pub async fn reactivate_listing(",
        "pub async fn archive_listing(",
    ] {
        assert!(
            !service.contains(forbidden),
            "listing service write bypass remains: {forbidden}"
        );
    }
}
