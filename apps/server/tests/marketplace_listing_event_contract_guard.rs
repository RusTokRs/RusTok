#[test]
fn marketplace_listing_external_events_use_a_sealed_typed_contract() {
    let contract = include_str!("../../../crates/rustok-events/src/contract.rs");
    let listing = include_str!("../../../crates/rustok-events/src/marketplace_listing.rs");
    let outbox = include_str!("../../../crates/rustok-outbox/src/transactional.rs");
    let owner_service = include_str!("../../../crates/rustok-marketplace-listing/src/service.rs");
    let owner_receipts =
        include_str!("../../../crates/rustok-marketplace-listing/src/command_receipts.rs");
    let owner_events =
        include_str!("../../../crates/rustok-marketplace-listing/src/external_events.rs");
    let owner_evented =
        include_str!("../../../crates/rustok-marketplace-listing/src/evented_commands.rs");
    let owner_lifecycle =
        include_str!("../../../crates/rustok-marketplace-listing/src/lifecycle_event_commands.rs");
    let owner_replay =
        include_str!("../../../crates/rustok-marketplace-listing/src/replay_safe_commands.rs");
    let owner_tests =
        include_str!("../../../crates/rustok-marketplace-listing/src/command_receipts_tests.rs");

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
        assert!(
            owner_events.contains(variant),
            "owner completion mapper does not publish {variant}"
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

    for marker in [
        "event_bus: TransactionalEventBus",
        "event_bus: TransactionalEventBus,",
        "pub(crate) fn event_bus(&self) -> &TransactionalEventBus",
    ] {
        assert!(
            owner_service.contains(marker),
            "listing owner service is missing injected event bus marker {marker}"
        );
    }
    assert!(
        !owner_receipts.contains("OutboxTransport::new"),
        "receipt executor must not construct the owner event transport"
    );
    for source in [owner_evented, owner_lifecycle, owner_replay] {
        assert!(
            source.contains("self.event_bus().clone()"),
            "listing command path does not pass the injected event bus to admission"
        );
    }

    for marker in [
        "event_for_completed_command(command_kind.as_str(), response)",
        "publish_contract_in_tx(&transaction, tenant_id, Some(actor_id), event)",
        "transaction.rollback().await?",
        "transaction.commit().await?",
    ] {
        assert!(
            owner_receipts.contains(marker),
            "owner receipt executor is missing transactional outbox marker {marker}"
        );
    }
    for marker in [
        "completed_receipt_commits_one_contract_event_and_replay_adds_none",
        "missing_outbox_storage_rolls_back_the_pending_receipt",
        "receipt_completion_failure_rolls_back_the_inserted_outbox_event",
    ] {
        assert!(
            owner_tests.contains(marker),
            "owner outbox execution test is missing {marker}"
        );
    }
}
