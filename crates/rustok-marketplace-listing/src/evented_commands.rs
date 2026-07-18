use chrono::Utc;
use sea_orm::{ActiveModelTrait, Set};
use uuid::Uuid;
use validator::Validate;

use crate::command_receipts::{
    admit, complete, normalize_idempotency_key, replay, request_hash, rollback,
    ListingCommandAdmission, NewListingCommandReceipt,
};
use crate::dto::{
    ListMarketplaceListingEventsRequest, MarketplaceListingApprovalStatus,
    MarketplaceListingEventKind, MarketplaceListingEventResponse, MarketplaceListingResponse,
    MarketplaceListingStatus, ReviewMarketplaceListingInput, SuspendMarketplaceListingInput,
};
use crate::entities::listing;
use crate::error::{MarketplaceListingError, MarketplaceListingResult};
use crate::listing_events::{
    append_listing_event, list_listing_events, normalize_listing_event_locale,
};
use crate::service::{find_listing, load_response_for_model};
use crate::MarketplaceListingService;

impl MarketplaceListingService {
    pub async fn list_events(
        &self,
        tenant_id: Uuid,
        request: ListMarketplaceListingEventsRequest,
    ) -> MarketplaceListingResult<Vec<MarketplaceListingEventResponse>> {
        find_listing(self.database(), tenant_id, request.listing_id).await?;
        list_listing_events(
            self.database(),
            tenant_id,
            request.listing_id,
            request.limit,
        )
        .await
    }

    pub async fn review_listing_evented(
        &self,
        context: rustok_api::PortContext,
        input: ReviewMarketplaceListingInput,
    ) -> MarketplaceListingResult<MarketplaceListingResponse> {
        context
            .require_write_semantics()
            .map_err(|error| MarketplaceListingError::Validation(error.message))?;
        input
            .validate()
            .map_err(|error| MarketplaceListingError::Validation(error.to_string()))?;
        let tenant_id = parse_tenant_id(&context)?;
        let actor_id = parse_actor_id(&context)?;
        let locale = normalize_listing_event_locale(context.locale.as_str())?;
        let key = normalize_idempotency_key(required_idempotency_key(&context)?)?;
        let note = normalize_optional_text(input.note);
        let event_kind = if input.approved {
            MarketplaceListingEventKind::Approved
        } else {
            MarketplaceListingEventKind::Rejected
        };
        let normalized = serde_json::json!({
            "listing_id": input.listing_id,
            "approved": input.approved,
            "note": note.clone(),
            "locale": locale.clone(),
        });
        let hash = request_hash("review_listing", actor_id, &normalized)?;

        match admit(
            self.database(),
            self.event_bus().clone(),
            tenant_id,
            actor_id,
            key,
            "review_listing",
            hash.as_str(),
        )
        .await?
        {
            ListingCommandAdmission::Replay(receipt) => {
                replay(receipt, "review_listing", hash.as_str())
            }
            ListingCommandAdmission::New(receipt) => {
                let result = review_in_transaction(
                    &receipt,
                    tenant_id,
                    actor_id,
                    locale.as_str(),
                    input.listing_id,
                    input.approved,
                    note,
                    event_kind,
                )
                .await;
                finish(receipt, result).await
            }
        }
    }

    pub async fn suspend_listing_evented(
        &self,
        context: rustok_api::PortContext,
        input: SuspendMarketplaceListingInput,
    ) -> MarketplaceListingResult<MarketplaceListingResponse> {
        context
            .require_write_semantics()
            .map_err(|error| MarketplaceListingError::Validation(error.message))?;
        input
            .validate()
            .map_err(|error| MarketplaceListingError::Validation(error.to_string()))?;
        let tenant_id = parse_tenant_id(&context)?;
        let actor_id = parse_actor_id(&context)?;
        let locale = normalize_listing_event_locale(context.locale.as_str())?;
        let key = normalize_idempotency_key(required_idempotency_key(&context)?)?;
        let reason = required_text(input.reason, "reason")?;
        let normalized = serde_json::json!({
            "listing_id": input.listing_id,
            "reason": reason.clone(),
            "locale": locale.clone(),
        });
        let hash = request_hash("suspend_listing", actor_id, &normalized)?;

        match admit(
            self.database(),
            self.event_bus().clone(),
            tenant_id,
            actor_id,
            key,
            "suspend_listing",
            hash.as_str(),
        )
        .await?
        {
            ListingCommandAdmission::Replay(receipt) => {
                replay(receipt, "suspend_listing", hash.as_str())
            }
            ListingCommandAdmission::New(receipt) => {
                let result = suspend_in_transaction(
                    &receipt,
                    tenant_id,
                    actor_id,
                    locale.as_str(),
                    input.listing_id,
                    reason,
                )
                .await;
                finish(receipt, result).await
            }
        }
    }
}

#[allow(clippy::too_many_arguments)]
async fn review_in_transaction(
    receipt: &NewListingCommandReceipt,
    tenant_id: Uuid,
    actor_id: Uuid,
    locale: &str,
    listing_id: Uuid,
    approved: bool,
    note: Option<String>,
    event_kind: MarketplaceListingEventKind,
) -> MarketplaceListingResult<MarketplaceListingResponse> {
    let current = find_listing(&receipt.transaction, tenant_id, listing_id).await?;
    if current.status != MarketplaceListingStatus::PendingReview.as_str() {
        return Err(MarketplaceListingError::InvalidTransition {
            from: format!("{}:{}", current.status, current.approval_status),
            to: if approved { "approved" } else { "rejected" }.to_string(),
        });
    }

    let now = Utc::now();
    let mut active: listing::ActiveModel = current.into();
    active.status = Set(MarketplaceListingStatus::Draft.as_str().to_string());
    active.approval_status = Set(if approved {
        MarketplaceListingApprovalStatus::Approved
    } else {
        MarketplaceListingApprovalStatus::Rejected
    }
    .as_str()
    .to_string());
    active.approved_at = Set(if approved { Some(now.into()) } else { None });
    active.updated_at = Set(now.into());
    let model = active.update(&receipt.transaction).await?;

    append_listing_event(
        &receipt.transaction,
        tenant_id,
        listing_id,
        actor_id,
        event_kind,
        locale,
        note,
        serde_json::json!({}),
    )
    .await?;
    load_response_for_model(&receipt.transaction, model).await
}

async fn suspend_in_transaction(
    receipt: &NewListingCommandReceipt,
    tenant_id: Uuid,
    actor_id: Uuid,
    locale: &str,
    listing_id: Uuid,
    reason: String,
) -> MarketplaceListingResult<MarketplaceListingResponse> {
    let current = find_listing(&receipt.transaction, tenant_id, listing_id).await?;
    if current.status != MarketplaceListingStatus::Active.as_str() {
        return Err(MarketplaceListingError::InvalidTransition {
            from: format!("{}:{}", current.status, current.approval_status),
            to: MarketplaceListingStatus::Suspended.as_str().to_string(),
        });
    }

    let mut active: listing::ActiveModel = current.into();
    active.status = Set(MarketplaceListingStatus::Suspended.as_str().to_string());
    active.updated_at = Set(Utc::now().into());
    let model = active.update(&receipt.transaction).await?;

    append_listing_event(
        &receipt.transaction,
        tenant_id,
        listing_id,
        actor_id,
        MarketplaceListingEventKind::Suspended,
        locale,
        Some(reason),
        serde_json::json!({}),
    )
    .await?;
    load_response_for_model(&receipt.transaction, model).await
}

async fn finish(
    receipt: NewListingCommandReceipt,
    result: MarketplaceListingResult<MarketplaceListingResponse>,
) -> MarketplaceListingResult<MarketplaceListingResponse> {
    match result {
        Ok(response) => complete(receipt, &response).await,
        Err(error) => rollback(receipt, error).await,
    }
}

fn parse_tenant_id(context: &rustok_api::PortContext) -> MarketplaceListingResult<Uuid> {
    Uuid::parse_str(context.tenant_id.as_str()).map_err(|_| {
        MarketplaceListingError::Validation(
            "PortContext.tenant_id must be a UUID for marketplace listing ports".to_string(),
        )
    })
}

fn parse_actor_id(context: &rustok_api::PortContext) -> MarketplaceListingResult<Uuid> {
    Uuid::parse_str(context.actor.id.as_str()).map_err(|_| {
        MarketplaceListingError::Validation(
            "write PortContext.actor.id must be a UUID for marketplace listing audit".to_string(),
        )
    })
}

fn required_idempotency_key(context: &rustok_api::PortContext) -> MarketplaceListingResult<String> {
    context.idempotency_key.clone().ok_or_else(|| {
        MarketplaceListingError::Validation(
            "marketplace listing write requires an idempotency key".to_string(),
        )
    })
}

fn required_text(value: String, field: &str) -> MarketplaceListingResult<String> {
    let value = value.trim();
    if value.is_empty() {
        return Err(MarketplaceListingError::Validation(format!(
            "{field} must not be empty"
        )));
    }
    Ok(value.to_string())
}

fn normalize_optional_text(value: Option<String>) -> Option<String> {
    value.and_then(|value| {
        let value = value.trim();
        (!value.is_empty()).then(|| value.to_string())
    })
}
