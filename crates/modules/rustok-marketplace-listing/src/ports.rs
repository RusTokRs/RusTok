use async_trait::async_trait;
use rustok_api::{PortCallPolicy, PortContext, PortError, PortErrorKind};
use serde::{Deserialize, Serialize};
use std::sync::Arc;
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

/// Combined owner boundary transferred by the host to native and GraphQL
/// transports. The handle contains no host, database, or product-specific
/// infrastructure and can therefore wrap either embedded or remote adapters.
pub trait MarketplaceListingPorts:
    MarketplaceListingReadPort + MarketplaceListingCommandPort
{
}

impl<T> MarketplaceListingPorts for T where
    T: MarketplaceListingReadPort + MarketplaceListingCommandPort
{
}

#[derive(Clone)]
pub struct MarketplaceListingRuntime {
    ports: Arc<dyn MarketplaceListingPorts>,
}

impl MarketplaceListingRuntime {
    pub fn new(ports: Arc<dyn MarketplaceListingPorts>) -> Self {
        Self { ports }
    }

    pub fn ports(&self) -> &dyn MarketplaceListingPorts {
        self.ports.as_ref()
    }
}

#[async_trait]
impl MarketplaceListingReadPort for crate::MarketplaceListingService {
    async fn read_listing(
        &self,
        context: PortContext,
        request: ReadMarketplaceListingRequest,
    ) -> Result<MarketplaceListingResponse, PortError> {
        context.require_policy(PortCallPolicy::read())?;
        if request.listing_id.is_nil() {
            return Err(PortError::validation(
                "marketplace_listing.listing_id_invalid",
                "listing_id must not be nil",
            ));
        }
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
        if request.listing_id.is_nil() {
            return Err(PortError::validation(
                "marketplace_listing.listing_id_invalid",
                "listing_id must not be nil",
            ));
        }
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
        if request.seller_id.is_nil() || request.master_variant_id.is_nil() {
            return Err(PortError::validation(
                "marketplace_listing.identity_invalid",
                "seller_id and master_variant_id must not be nil",
            ));
        }
        self.create_listing_replay_safe(context, request)
            .await
            .map_err(map_owner_error)
    }

    async fn update_listing_terms(
        &self,
        context: PortContext,
        request: UpdateMarketplaceListingTermsInput,
    ) -> Result<MarketplaceListingResponse, PortError> {
        if request.listing_id.is_nil() {
            return Err(PortError::validation(
                "marketplace_listing.listing_id_invalid",
                "listing_id must not be nil",
            ));
        }
        self.update_terms_evented(context, request)
            .await
            .map_err(map_owner_error)
    }

    async fn submit_listing_for_review(
        &self,
        context: PortContext,
        request: MarketplaceListingIdRequest,
    ) -> Result<MarketplaceListingResponse, PortError> {
        if request.listing_id.is_nil() {
            return Err(PortError::validation(
                "marketplace_listing.listing_id_invalid",
                "listing_id must not be nil",
            ));
        }
        self.submit_for_review_evented(context, request.listing_id)
            .await
            .map_err(map_owner_error)
    }

    async fn review_listing(
        &self,
        context: PortContext,
        request: ReviewMarketplaceListingInput,
    ) -> Result<MarketplaceListingResponse, PortError> {
        if request.listing_id.is_nil() {
            return Err(PortError::validation(
                "marketplace_listing.listing_id_invalid",
                "listing_id must not be nil",
            ));
        }
        self.review_listing_evented(context, request)
            .await
            .map_err(map_owner_error)
    }

    async fn publish_listing(
        &self,
        context: PortContext,
        request: MarketplaceListingIdRequest,
    ) -> Result<MarketplaceListingResponse, PortError> {
        if request.listing_id.is_nil() {
            return Err(PortError::validation(
                "marketplace_listing.listing_id_invalid",
                "listing_id must not be nil",
            ));
        }
        self.publish_listing_replay_safe(context, request.listing_id)
            .await
            .map_err(map_owner_error)
    }

    async fn suspend_listing(
        &self,
        context: PortContext,
        request: SuspendMarketplaceListingInput,
    ) -> Result<MarketplaceListingResponse, PortError> {
        if request.listing_id.is_nil() {
            return Err(PortError::validation(
                "marketplace_listing.listing_id_invalid",
                "listing_id must not be nil",
            ));
        }
        self.suspend_listing_evented(context, request)
            .await
            .map_err(map_owner_error)
    }

    async fn reactivate_listing(
        &self,
        context: PortContext,
        request: MarketplaceListingIdRequest,
    ) -> Result<MarketplaceListingResponse, PortError> {
        if request.listing_id.is_nil() {
            return Err(PortError::validation(
                "marketplace_listing.listing_id_invalid",
                "listing_id must not be nil",
            ));
        }
        self.reactivate_listing_replay_safe(context, request.listing_id)
            .await
            .map_err(map_owner_error)
    }

    async fn archive_listing(
        &self,
        context: PortContext,
        request: MarketplaceListingIdRequest,
    ) -> Result<MarketplaceListingResponse, PortError> {
        if request.listing_id.is_nil() {
            return Err(PortError::validation(
                "marketplace_listing.listing_id_invalid",
                "listing_id must not be nil",
            ));
        }
        self.archive_listing_evented(context, request.listing_id)
            .await
            .map_err(map_owner_error)
    }
}

fn parse_tenant_id(context: &PortContext) -> Result<Uuid, PortError> {
    let id = Uuid::parse_str(context.tenant_id.as_str()).map_err(|_| {
        PortError::validation(
            "marketplace_listing.tenant_id_invalid",
            "PortContext.tenant_id must be a UUID for marketplace listing ports",
        )
    })?;
    if id.is_nil() {
        return Err(PortError::validation(
            "marketplace_listing.tenant_id_invalid",
            "PortContext.tenant_id must not be nil for marketplace listing ports",
        ));
    }
    Ok(id)
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

#[cfg(test)]
mod tests {
    use super::*;
    use rustok_api::PortActor;

    #[test]
    fn parse_tenant_id_requires_valid_non_nil_uuid() {
        let nil_context = PortContext::new(
            Uuid::nil().to_string(),
            PortActor::system(),
            "en",
            "contract",
        );
        let err = parse_tenant_id(&nil_context).unwrap_err();
        assert_eq!(err.code, "marketplace_listing.tenant_id_invalid");

        let invalid_context = PortContext::new(
            "not-a-uuid",
            PortActor::system(),
            "en",
            "contract",
        );
        let err2 = parse_tenant_id(&invalid_context).unwrap_err();
        assert_eq!(err2.code, "marketplace_listing.tenant_id_invalid");

        let valid_id = Uuid::new_v4();
        let valid_context = PortContext::new(
            valid_id.to_string(),
            PortActor::system(),
            "en",
            "contract",
        );
        assert_eq!(parse_tenant_id(&valid_context).unwrap(), valid_id);
    }

    #[test]
    fn map_owner_error_maps_typed_errors_correctly() {
        let err = map_owner_error(MarketplaceListingError::ListingNotFound(Uuid::nil()));
        assert_eq!(err.code, "marketplace_listing.not_found");

        let err2 = map_owner_error(MarketplaceListingError::TermsNotFound {
            listing_id: Uuid::nil(),
            version: 1,
        });
        assert_eq!(err2.code, "marketplace_listing.terms_missing");

        let err3 = map_owner_error(MarketplaceListingError::Validation("invalid input".into()));
        assert_eq!(err3.code, "marketplace_listing.validation");

        let err4 = map_owner_error(MarketplaceListingError::DuplicateScope);
        assert_eq!(err4.code, "marketplace_listing.scope_conflict");
    }
}
