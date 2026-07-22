use rustok_events::MarketplaceListingEvent;

use crate::dto::{MarketplaceListingApprovalStatus, MarketplaceListingResponse};
use crate::error::{MarketplaceListingError, MarketplaceListingResult};

pub(crate) fn event_for_completed_command(
    command_kind: &str,
    listing: &MarketplaceListingResponse,
) -> MarketplaceListingResult<MarketplaceListingEvent> {
    macro_rules! event {
        ($variant:ident) => {
            MarketplaceListingEvent::$variant {
                listing_id: listing.id,
                seller_id: listing.seller_id,
                master_product_id: listing.master_product_id,
                master_variant_id: listing.master_variant_id,
                market_slug: listing.market_slug.clone(),
                channel_slug: listing.channel_slug.clone(),
                terms_version: listing.current_terms_version,
            }
        };
    }

    let event = match command_kind {
        "create_listing" => event!(MarketplaceListingCreated),
        "update_listing_terms" => event!(MarketplaceListingTermsUpdated),
        "submit_listing_for_review" => event!(MarketplaceListingSubmittedForReview),
        "review_listing" => match listing.approval_status {
            MarketplaceListingApprovalStatus::Approved => event!(MarketplaceListingApproved),
            MarketplaceListingApprovalStatus::Rejected => event!(MarketplaceListingRejected),
            status => {
                return Err(MarketplaceListingError::EventContractInvariant(format!(
                    "completed review_listing receipt has incompatible approval status `{}`",
                    status.as_str()
                )));
            }
        },
        "publish_listing" => event!(MarketplaceListingPublished),
        "suspend_listing" => event!(MarketplaceListingSuspended),
        "reactivate_listing" => event!(MarketplaceListingReactivated),
        "archive_listing" => event!(MarketplaceListingArchived),
        other => {
            return Err(MarketplaceListingError::EventContractInvariant(format!(
                "completed marketplace listing receipt has unsupported command kind `{other}`"
            )));
        }
    };

    Ok(event)
}

#[cfg(test)]
mod tests {
    use chrono::Utc;

    use super::*;
    use crate::dto::{MarketplaceListingStatus, MarketplaceListingTermsResponse};

    fn listing() -> MarketplaceListingResponse {
        let now = Utc::now().fixed_offset();
        let listing_id = uuid::Uuid::new_v4();
        MarketplaceListingResponse {
            id: listing_id,
            tenant_id: uuid::Uuid::new_v4(),
            seller_id: uuid::Uuid::new_v4(),
            master_product_id: uuid::Uuid::new_v4(),
            master_variant_id: uuid::Uuid::new_v4(),
            seller_sku: "seller-sku".to_string(),
            market_slug: "primary-market".to_string(),
            channel_slug: "web".to_string(),
            status: MarketplaceListingStatus::Draft,
            approval_status: MarketplaceListingApprovalStatus::Approved,
            current_terms_version: 3,
            current_terms: MarketplaceListingTermsResponse {
                id: uuid::Uuid::new_v4(),
                listing_id,
                version: 3,
                pricing_reference: Some("price-list".to_string()),
                inventory_reference: Some("inventory-item".to_string()),
                fulfillment_profile_slug: Some("standard".to_string()),
                metadata: serde_json::json!({"owner_private": true}),
                created_at: now,
            },
            metadata: serde_json::json!({"owner_private": true}),
            published_at: None,
            approved_at: None,
            created_at: now,
            updated_at: now,
        }
    }

    #[test]
    fn maps_every_completed_command_to_the_expected_contract_type() {
        let listing = listing();
        for (command_kind, event_type) in [
            ("create_listing", "marketplace.listing.created"),
            ("update_listing_terms", "marketplace.listing.terms_updated"),
            (
                "submit_listing_for_review",
                "marketplace.listing.submitted_for_review",
            ),
            ("review_listing", "marketplace.listing.approved"),
            ("publish_listing", "marketplace.listing.published"),
            ("suspend_listing", "marketplace.listing.suspended"),
            ("reactivate_listing", "marketplace.listing.reactivated"),
            ("archive_listing", "marketplace.listing.archived"),
        ] {
            let event = event_for_completed_command(command_kind, &listing).unwrap();
            assert_eq!(event.event_type(), event_type);
            assert_external_payload_is_safe(&event);
        }

        let mut rejected = listing;
        rejected.approval_status = MarketplaceListingApprovalStatus::Rejected;
        let event = event_for_completed_command("review_listing", &rejected).unwrap();
        assert_eq!(event.event_type(), "marketplace.listing.rejected");
        assert_external_payload_is_safe(&event);
    }

    #[test]
    fn fails_closed_for_unknown_or_inconsistent_completed_receipts() {
        let listing = listing();
        assert!(matches!(
            event_for_completed_command("legacy_snapshot", &listing),
            Err(MarketplaceListingError::EventContractInvariant(_))
        ));

        let mut pending = listing;
        pending.approval_status = MarketplaceListingApprovalStatus::Pending;
        assert!(matches!(
            event_for_completed_command("review_listing", &pending),
            Err(MarketplaceListingError::EventContractInvariant(_))
        ));
    }

    fn assert_external_payload_is_safe(event: &MarketplaceListingEvent) {
        let encoded = serde_json::to_string(event).unwrap();
        for forbidden in [
            "\"note\"",
            "\"reason\"",
            "\"metadata\"",
            "\"approval_note\"",
            "\"suspension_reason\"",
            "owner_private",
        ] {
            assert!(
                !encoded.contains(forbidden),
                "external event leaked owner-private marker {forbidden}: {encoded}"
            );
        }
    }
}
