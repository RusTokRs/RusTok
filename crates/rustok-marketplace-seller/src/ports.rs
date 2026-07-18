use async_trait::async_trait;
use rustok_api::{PortCallPolicy, PortContext, PortError, PortErrorKind};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::dto::{
    AddMarketplaceSellerMemberInput, CreateMarketplaceSellerInput,
    ListMarketplaceSellerEventsRequest, ListMarketplaceSellerMembersRequest,
    ListMarketplaceSellersInput, MarketplaceSellerEventResponse, MarketplaceSellerMemberResponse,
    MarketplaceSellerResponse, ReadMarketplaceSellerMembershipRequest,
    ReadMarketplaceSellerRequest, ReviewMarketplaceSellerOnboardingInput,
    SubmitMarketplaceSellerOnboardingInput, SuspendMarketplaceSellerInput,
    UpdateMarketplaceSellerMemberInput, UpdateMarketplaceSellerProfileInput,
};
use crate::error::MarketplaceSellerError;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MarketplaceSellerListResponse {
    pub items: Vec<MarketplaceSellerResponse>,
    pub total: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UpdateMarketplaceSellerProfileRequest {
    pub seller_id: Uuid,
    pub input: UpdateMarketplaceSellerProfileInput,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SubmitMarketplaceSellerOnboardingRequest {
    pub seller_id: Uuid,
    pub input: SubmitMarketplaceSellerOnboardingInput,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReviewMarketplaceSellerOnboardingRequest {
    pub seller_id: Uuid,
    pub input: ReviewMarketplaceSellerOnboardingInput,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SuspendMarketplaceSellerRequest {
    pub seller_id: Uuid,
    pub input: SuspendMarketplaceSellerInput,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReactivateMarketplaceSellerRequest {
    pub seller_id: Uuid,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AddMarketplaceSellerMemberRequest {
    pub seller_id: Uuid,
    pub input: AddMarketplaceSellerMemberInput,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UpdateMarketplaceSellerMemberRequest {
    pub seller_id: Uuid,
    pub member_id: Uuid,
    pub input: UpdateMarketplaceSellerMemberInput,
}

#[async_trait]
pub trait MarketplaceSellerReadPort: Send + Sync {
    async fn read_seller(
        &self,
        context: PortContext,
        request: ReadMarketplaceSellerRequest,
    ) -> Result<MarketplaceSellerResponse, PortError>;

    async fn list_sellers(
        &self,
        context: PortContext,
        request: ListMarketplaceSellersInput,
    ) -> Result<MarketplaceSellerListResponse, PortError>;

    async fn read_membership(
        &self,
        context: PortContext,
        request: ReadMarketplaceSellerMembershipRequest,
    ) -> Result<MarketplaceSellerMemberResponse, PortError>;

    async fn list_members(
        &self,
        context: PortContext,
        request: ListMarketplaceSellerMembersRequest,
    ) -> Result<Vec<MarketplaceSellerMemberResponse>, PortError>;

    async fn list_seller_events(
        &self,
        context: PortContext,
        request: ListMarketplaceSellerEventsRequest,
    ) -> Result<Vec<MarketplaceSellerEventResponse>, PortError>;
}

#[async_trait]
pub trait MarketplaceSellerCommandPort: Send + Sync {
    async fn create_seller(
        &self,
        context: PortContext,
        request: CreateMarketplaceSellerInput,
    ) -> Result<MarketplaceSellerResponse, PortError>;

    async fn update_seller_profile(
        &self,
        context: PortContext,
        request: UpdateMarketplaceSellerProfileRequest,
    ) -> Result<MarketplaceSellerResponse, PortError>;

    async fn submit_seller_onboarding(
        &self,
        context: PortContext,
        request: SubmitMarketplaceSellerOnboardingRequest,
    ) -> Result<MarketplaceSellerResponse, PortError>;

    async fn review_seller_onboarding(
        &self,
        context: PortContext,
        request: ReviewMarketplaceSellerOnboardingRequest,
    ) -> Result<MarketplaceSellerResponse, PortError>;

    async fn suspend_seller(
        &self,
        context: PortContext,
        request: SuspendMarketplaceSellerRequest,
    ) -> Result<MarketplaceSellerResponse, PortError>;

    async fn reactivate_seller(
        &self,
        context: PortContext,
        request: ReactivateMarketplaceSellerRequest,
    ) -> Result<MarketplaceSellerResponse, PortError>;

    async fn add_seller_member(
        &self,
        context: PortContext,
        request: AddMarketplaceSellerMemberRequest,
    ) -> Result<MarketplaceSellerMemberResponse, PortError>;

    async fn update_seller_member(
        &self,
        context: PortContext,
        request: UpdateMarketplaceSellerMemberRequest,
    ) -> Result<MarketplaceSellerMemberResponse, PortError>;
}

#[async_trait]
impl MarketplaceSellerReadPort for crate::MarketplaceSellerService {
    async fn read_seller(
        &self,
        context: PortContext,
        request: ReadMarketplaceSellerRequest,
    ) -> Result<MarketplaceSellerResponse, PortError> {
        context.require_policy(PortCallPolicy::read())?;
        self.get_seller(
            parse_tenant_id(&context)?,
            request.seller_id,
            context.locale.as_str(),
        )
        .await
        .map_err(map_owner_error)
    }

    async fn list_sellers(
        &self,
        context: PortContext,
        request: ListMarketplaceSellersInput,
    ) -> Result<MarketplaceSellerListResponse, PortError> {
        context.require_policy(PortCallPolicy::read())?;
        let (items, total) = self
            .list_sellers(
                parse_tenant_id(&context)?,
                context.locale.as_str(),
                request,
            )
            .await
            .map_err(map_owner_error)?;
        Ok(MarketplaceSellerListResponse { items, total })
    }

    async fn read_membership(
        &self,
        context: PortContext,
        request: ReadMarketplaceSellerMembershipRequest,
    ) -> Result<MarketplaceSellerMemberResponse, PortError> {
        context.require_policy(PortCallPolicy::read())?;
        self.get_membership(
            parse_tenant_id(&context)?,
            request.seller_id,
            request.user_id,
        )
        .await
        .map_err(map_owner_error)
    }

    async fn list_members(
        &self,
        context: PortContext,
        request: ListMarketplaceSellerMembersRequest,
    ) -> Result<Vec<MarketplaceSellerMemberResponse>, PortError> {
        context.require_policy(PortCallPolicy::read())?;
        self.list_members(parse_tenant_id(&context)?, request.seller_id)
            .await
            .map_err(map_owner_error)
    }

    async fn list_seller_events(
        &self,
        context: PortContext,
        request: ListMarketplaceSellerEventsRequest,
    ) -> Result<Vec<MarketplaceSellerEventResponse>, PortError> {
        context.require_policy(PortCallPolicy::read())?;
        self.list_events(
            parse_tenant_id(&context)?,
            request.seller_id,
            request.limit,
        )
        .await
        .map_err(map_owner_error)
    }
}

#[async_trait]
impl MarketplaceSellerCommandPort for crate::MarketplaceSellerService {
    async fn create_seller(
        &self,
        context: PortContext,
        request: CreateMarketplaceSellerInput,
    ) -> Result<MarketplaceSellerResponse, PortError> {
        context.require_policy(PortCallPolicy::write())?;
        self.create_seller_with_receipt(
            parse_tenant_id(&context)?,
            parse_actor_id(&context)?,
            parse_idempotency_key(&context)?,
            context.locale.as_str(),
            request,
        )
        .await
        .map_err(map_owner_error)
    }

    async fn update_seller_profile(
        &self,
        context: PortContext,
        request: UpdateMarketplaceSellerProfileRequest,
    ) -> Result<MarketplaceSellerResponse, PortError> {
        context.require_policy(PortCallPolicy::write())?;
        self.update_profile_with_receipt(
            parse_tenant_id(&context)?,
            parse_actor_id(&context)?,
            parse_idempotency_key(&context)?,
            context.locale.as_str(),
            request.seller_id,
            request.input,
        )
        .await
        .map_err(map_owner_error)
    }

    async fn submit_seller_onboarding(
        &self,
        context: PortContext,
        request: SubmitMarketplaceSellerOnboardingRequest,
    ) -> Result<MarketplaceSellerResponse, PortError> {
        context.require_policy(PortCallPolicy::write())?;
        self.submit_onboarding_with_receipt(
            parse_tenant_id(&context)?,
            parse_actor_id(&context)?,
            parse_idempotency_key(&context)?,
            context.locale.as_str(),
            request.seller_id,
            request.input,
        )
        .await
        .map_err(map_owner_error)
    }

    async fn review_seller_onboarding(
        &self,
        context: PortContext,
        request: ReviewMarketplaceSellerOnboardingRequest,
    ) -> Result<MarketplaceSellerResponse, PortError> {
        context.require_policy(PortCallPolicy::write())?;
        self.review_onboarding_with_receipt(
            parse_tenant_id(&context)?,
            parse_actor_id(&context)?,
            parse_idempotency_key(&context)?,
            context.locale.as_str(),
            request.seller_id,
            request.input,
        )
        .await
        .map_err(map_owner_error)
    }

    async fn suspend_seller(
        &self,
        context: PortContext,
        request: SuspendMarketplaceSellerRequest,
    ) -> Result<MarketplaceSellerResponse, PortError> {
        context.require_policy(PortCallPolicy::write())?;
        self.suspend_seller_with_receipt(
            parse_tenant_id(&context)?,
            parse_actor_id(&context)?,
            parse_idempotency_key(&context)?,
            context.locale.as_str(),
            request.seller_id,
            request.input,
        )
        .await
        .map_err(map_owner_error)
    }

    async fn reactivate_seller(
        &self,
        context: PortContext,
        request: ReactivateMarketplaceSellerRequest,
    ) -> Result<MarketplaceSellerResponse, PortError> {
        context.require_policy(PortCallPolicy::write())?;
        self.reactivate_seller_with_receipt(
            parse_tenant_id(&context)?,
            parse_actor_id(&context)?,
            parse_idempotency_key(&context)?,
            context.locale.as_str(),
            request.seller_id,
        )
        .await
        .map_err(map_owner_error)
    }

    async fn add_seller_member(
        &self,
        context: PortContext,
        request: AddMarketplaceSellerMemberRequest,
    ) -> Result<MarketplaceSellerMemberResponse, PortError> {
        context.require_policy(PortCallPolicy::write())?;
        self.add_member_with_receipt(
            parse_tenant_id(&context)?,
            parse_actor_id(&context)?,
            parse_idempotency_key(&context)?,
            request.seller_id,
            request.input,
        )
        .await
        .map_err(map_owner_error)
    }

    async fn update_seller_member(
        &self,
        context: PortContext,
        request: UpdateMarketplaceSellerMemberRequest,
    ) -> Result<MarketplaceSellerMemberResponse, PortError> {
        context.require_policy(PortCallPolicy::write())?;
        self.update_member_with_receipt(
            parse_tenant_id(&context)?,
            parse_actor_id(&context)?,
            parse_idempotency_key(&context)?,
            request.seller_id,
            request.member_id,
            request.input,
        )
        .await
        .map_err(map_owner_error)
    }
}

fn parse_tenant_id(context: &PortContext) -> Result<Uuid, PortError> {
    Uuid::parse_str(context.tenant_id.as_str()).map_err(|_| {
        PortError::validation(
            "marketplace_seller.tenant_id_invalid",
            "PortContext.tenant_id must be a UUID for marketplace seller ports",
        )
    })
}

fn parse_actor_id(context: &PortContext) -> Result<Uuid, PortError> {
    Uuid::parse_str(context.actor.id.as_str()).map_err(|_| {
        PortError::validation(
            "marketplace_seller.actor_id_invalid",
            "write PortContext.actor.id must be a UUID for marketplace seller audit",
        )
    })
}

fn parse_idempotency_key(context: &PortContext) -> Result<String, PortError> {
    context
        .idempotency_key
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_string)
        .ok_or_else(|| {
            PortError::validation(
                "marketplace_seller.idempotency_key_required",
                "write marketplace seller ports require an idempotency key",
            )
        })
}

fn map_owner_error(error: MarketplaceSellerError) -> PortError {
    match error {
        MarketplaceSellerError::SellerNotFound(id) => PortError::not_found(
            "marketplace_seller.seller_not_found",
            format!("marketplace seller {id} not found"),
        ),
        MarketplaceSellerError::MemberNotFound(id) => PortError::not_found(
            "marketplace_seller.member_not_found",
            format!("marketplace seller member {id} not found"),
        ),
        MarketplaceSellerError::MembershipNotFound { seller_id, user_id } => PortError::not_found(
            "marketplace_seller.membership_not_found",
            format!("membership for user {user_id} in seller {seller_id} not found"),
        ),
        MarketplaceSellerError::DuplicateHandle(handle) => PortError::conflict(
            "marketplace_seller.handle_conflict",
            format!("marketplace seller handle `{handle}` is already in use"),
        ),
        MarketplaceSellerError::DuplicateMembership { seller_id, user_id } => PortError::conflict(
            "marketplace_seller.membership_conflict",
            format!("user {user_id} is already a member of seller {seller_id}"),
        ),
        MarketplaceSellerError::IdempotencyConflict(_) => PortError::conflict(
            "marketplace_seller.idempotency_conflict",
            "marketplace seller idempotency key is already bound to another command",
        ),
        MarketplaceSellerError::CommandReceiptCorrupt(_) => PortError::invariant_violation(
            "marketplace_seller.command_receipt_corrupt",
            "marketplace seller command receipt requires operator review",
        ),
        MarketplaceSellerError::InvalidTransition { from, to } => PortError::conflict(
            "marketplace_seller.lifecycle_conflict",
            format!("marketplace seller transition from `{from}` to `{to}` is not allowed"),
        ),
        MarketplaceSellerError::Validation(message)
            if message.starts_with(crate::service::MISSING_TRANSLATION_PREFIX) =>
        {
            PortError::invariant_violation(
                "marketplace_seller.translation_missing",
                "marketplace seller translation is missing for the effective locale",
            )
        }
        MarketplaceSellerError::Validation(message) => {
            PortError::validation("marketplace_seller.validation", message)
        }
        MarketplaceSellerError::Database(_) => PortError::new(
            PortErrorKind::Unavailable,
            "marketplace_seller.storage_unavailable",
            "marketplace seller storage is temporarily unavailable",
            true,
        ),
    }
}

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use rustok_api::{PortActor, PortErrorKind};

    use super::*;

    #[test]
    fn read_and_write_policies_are_explicit() {
        let base = PortContext::new(
            Uuid::nil().to_string(),
            PortActor::user(Uuid::nil().to_string()),
            "en",
            "marketplace-seller-contract",
        );
        assert_eq!(
            base.require_policy(PortCallPolicy::read()).unwrap_err().kind,
            PortErrorKind::Timeout
        );
        let read = base.clone().with_deadline(Duration::from_secs(3));
        assert!(read.require_policy(PortCallPolicy::read()).is_ok());
        assert_eq!(
            read.require_policy(PortCallPolicy::write()).unwrap_err().code,
            "port.idempotency_key_required"
        );
        assert!(read
            .with_idempotency_key("marketplace-seller-command")
            .require_policy(PortCallPolicy::write())
            .is_ok());
    }
}
