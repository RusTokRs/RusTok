use async_trait::async_trait;
use rustok_api::{PortCallPolicy, PortContext, PortError, PortErrorKind};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::dto::{
    CreateMarketplaceListingInput, ListMarketplaceListingEventsRequest,
    ListMarketplaceListingsInput, MarketplaceListingEligibilityProjection,
    MarketplaceListingEligibilityRequest, MarketplaceListingEventResponse,
    MarketplaceListingListResponse, MarketplaceListingResponse, ReadMarketplaceListingRequest,
    ReviewMarketplaceListingInput, SuspendMarketplaceListingInput,
    UpdateMarketplaceListingTermsInput,
};
use crate::error::MarketplaceListingError;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MarketplaceListingIdRequest {
    pub listing_id: Uuid,
}

#[async_trait]
pub trait MarketplaceListingReadPort: Send + Sync {
    async fn read_listing(
        &self,
        context: PortContext,
        request: ReadMarketplaceListingRequest,
    ) -> Result<MarketplaceListingResponse, PortError>;

    async fn list_listings(
        &self,
        context: PortContext,
        request: ListMarketplaceListingsInput,
    ) -> Result<MarketplaceListingListResponse, PortError>;

    async fn list_eligibility(
        &self,
        context: PortContext,
        request: MarketplaceListingEligibilityRequest,
    ) -> Result<Vec<MarketplaceListingEligibilityProjection>, PortError>;

    async fn list_listing_events(
        &self,
        context: PortContext,
        request: ListMarketplaceListingEventsRequest,
    ) -> Result<Vec<MarketplaceListingEventResponse>, PortError>;
}

#[async_trait]
pub trait MarketplaceListingCommandPort: Send + Sync {
    async fn create_listing(
        &self,
        context: PortContext,
        request: CreateMarketplaceListingInput,
    ) -> Result<MarketplaceListingResponse, PortError>;

    async fn update_listing_terms(
        &self,
        context: PortContext,
        request: UpdateMarketplaceListingTermsInput,
    ) -> Result<MarketplaceListingResponse, PortError>;

    async fn submit_listing_for_review(
        &self,
        context: PortContext,
        request: MarketplaceListingIdRequest,
    ) -> Result<MarketplaceListingResponse, PortError>;

    async fn review_listing(
        &self,
        context: PortContext,
        request: ReviewMarketplaceListingInput,
    ) -> Result<MarketplaceListingResponse, PortError>;

    async fn publish_listing(
        &self,
        context: PortContext,
        request: MarketplaceListingIdRequest,
    ) -> Result<MarketplaceListingResponse, PortError>;

    async fn suspend_listing(
        &self,
        context: PortContext,
        request: SuspendMarketplaceListingInput,
    ) -> Result<MarketplaceListingResponse, PortError>;

    async fn reactivate_listing(
        &self,
        context: PortContext,
        request: MarketplaceListingIdRequest,
    ) -> Result<MarketplaceListingResponse, PortError>;

    async fn archive_listing(
        &self,
        context: PortContext,
        request: MarketplaceListingIdRequest,
    ) -> Result<MarketplaceListingResponse, PortError>;
}

#[async_trait]
impl MarketplaceListingReadPort for crate::MarketplaceListingService {
    async fn read_listing(
        &self,
        context: PortContext,
        request: ReadMarketplaceListingRequest,
    ) -> Result<MarketplaceListingResponse, PortError> {
        context.require_policy(PortCallPolicy::read())?;
        self.get_listing(parse_tenant_id(&context)?, request.listing_id)
            .await
            .map_err(map_owner_error)
    }

    async fn list_listings(
        &self,
        context: PortContext,
        request: ListMarketplaceListingsInput,
    ) -> Result<MarketplaceListingListResponse, PortError> {
        context.require_policy(PortCallPolicy::read())?;
        self.list_listings(parse_tenant_id(&context)?, request)
            .await
            .map_err(map_owner_error)
    }

    async fn list_eligibility(
        &self,
        context: PortContext,
        request: MarketplaceListingEligibilityRequest,
    ) -> Result<Vec<MarketplaceListingEligibilityProjection>, PortError> {
        context.require_policy(PortCallPolicy::read())?;
        self.list_eligibility(context, request)
            .await
            .map_err(map_owner_error)
    }

    async fn list_listing_events(
        &self,
        context: PortContext,
        request: ListMarketplaceListingEventsRequest,
    ) -> Result<Vec<MarketplaceListingEventResponse>, PortError> {
        context.require_policy(PortCallPolicy::read())?;
        self.list_events(parse_tenant_id(&context)?, request)
            .await
            .map_err(map_owner_error)
    }
}

#[async_trait]
impl MarketplaceListingCommandPort for crate::MarketplaceListingService {
    async fn create_listing(
        &self,
        context: PortContext,
        request: CreateMarketplaceListingInput,
    ) -> Result<MarketplaceListingResponse, PortError> {
        self.create_listing_replay_safe(context, request)
            .await
            .map_err(map_owner_error)
    }

    async fn update_listing_terms(
        &self,
        context: PortContext,
        request: UpdateMarketplaceListingTermsInput,
    ) -> Result<MarketplaceListingResponse, PortError> {
        self.update_terms_evented(context, request)
            .await
            .map_err(map_owner_error)
    }

    async fn submit_listing_for_review(
        &self,
        context: PortContext,
        request: MarketplaceListingIdRequest,
    ) -> Result<MarketplaceListingResponse, PortError> {
        self.submit_for_review_evented(context, request.listing_id)
            .await
            .map_err(map_owner_error)
    }

    async fn review_listing(
        &self,
        context: PortContext,
        request: ReviewMarketplaceListingInput,
    ) -> Result<MarketplaceListingResponse, PortError> {
        self.review_listing_evented(context, request)
            .await
            .map_err(map_owner_error)
    }

    async fn publish_listing(
        &self,
        context: PortContext,
        request: MarketplaceListingIdRequest,
    ) -> Result<MarketplaceListingResponse, PortError> {
        self.publish_listing_replay_safe(context, request.listing_id)
            .await
            .map_err(map_owner_error)
    }

    async fn suspend_listing(
        &self,
        context: PortContext,
        request: SuspendMarketplaceListingInput,
    ) -> Result<MarketplaceListingResponse, PortError> {
        self.suspend_listing_evented(context, request)
            .await
            .map_err(map_owner_error)
    }

    async fn reactivate_listing(
        &self,
        context: PortContext,
        request: MarketplaceListingIdRequest,
    ) -> Result<MarketplaceListingResponse, PortError> {
        self.reactivate_listing_replay_safe(context, request.listing_id)
            .await
            .map_err(map_owner_error)
    }

    async fn archive_listing(
        &self,
        context: PortContext,
        request: MarketplaceListingIdRequest,
    ) -> Result<MarketplaceListingResponse, PortError> {
        self.archive_listing_evented(context, request.listing_id)
            .await
            .map_err(map_owner_error)
    }
}

fn parse_tenant_id(context: &PortContext) -> Result<Uuid, PortError> {
    Uuid::parse_str(context.tenant_id.as_str()).map_err(|_| {
        PortError::validation(
            "marketplace_listing.tenant_id_invalid",
            "PortContext.tenant_id must be a UUID for marketplace listing ports",
        )
    })
}

fn map_owner_error(error: MarketplaceListingError) -> PortError {
    match error {
        MarketplaceListingError::ListingNotFound(id) => PortError::not_found(
            "marketplace_listing.not_found",
            format!("marketplace listing {id} not found"),
        ),
        MarketplaceListingError::TermsNotFound {
            listing_id,
            version,
        } => PortError::invariant_violation(
            "marketplace_listing.terms_missing",
            format!("listing {listing_id} terms version {version} requires operator review"),
        ),
        MarketplaceListingError::SellerUnavailable(_) => PortError::unavailable(
            "marketplace_listing.seller_unavailable",
            "marketplace seller service is temporarily unavailable",
        ),
        MarketplaceListingError::ProductUnavailable(_) => PortError::unavailable(
            "marketplace_listing.product_unavailable",
            "product catalog service is temporarily unavailable",
        ),
        MarketplaceListingError::DuplicateScope => PortError::conflict(
            "marketplace_listing.scope_conflict",
            "a marketplace listing already exists for this seller, variant, market, and channel",
        ),
        MarketplaceListingError::DuplicateSellerSku(_) => PortError::conflict(
            "marketplace_listing.seller_sku_conflict",
            "marketplace listing seller SKU is already in use",
        ),
        MarketplaceListingError::IdempotencyConflict => PortError::conflict(
            "marketplace_listing.idempotency_conflict",
            "marketplace listing idempotency key is already bound to another command",
        ),
        MarketplaceListingError::CommandReceiptCorrupt => PortError::invariant_violation(
            "marketplace_listing.command_receipt_corrupt",
            "marketplace listing command receipt requires operator review",
        ),
        MarketplaceListingError::EventContractInvariant(_) => PortError::invariant_violation(
            "marketplace_listing.event_contract_invariant",
            "marketplace listing event contract requires operator review",
        ),
        MarketplaceListingError::EventPublicationUnavailable => PortError::unavailable(
            "marketplace_listing.event_publication_unavailable",
            "marketplace listing event publication is temporarily unavailable",
        ),
        MarketplaceListingError::Validation(message) => {
            PortError::validation("marketplace_listing.validation", message)
        }
        MarketplaceListingError::InvalidTransition { from, to } => PortError::conflict(
            "marketplace_listing.lifecycle_conflict",
            format!("marketplace listing transition from `{from}` to `{to}` is not allowed"),
        ),
        MarketplaceListingError::Database(_) => PortError::new(
            PortErrorKind::Unavailable,
            "marketplace_listing.storage_unavailable",
            "marketplace listing storage is temporarily unavailable",
            true,
        ),
    }
}
