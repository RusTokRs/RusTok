use chrono::Utc;
use sea_orm::{ActiveModelTrait, ColumnTrait, ConnectionTrait, EntityTrait, QueryFilter, Set};
use uuid::Uuid;
use validator::Validate;

use crate::command_receipts::{
    admit, complete, normalize_idempotency_key, replay, request_hash, rollback,
    ListingCommandAdmission, NewListingCommandReceipt,
};
use crate::dto::{
    ListMarketplaceListingEventsRequest, MarketplaceListingApprovalStatus,
    MarketplaceListingEventKind, MarketplaceListingEventResponse, MarketplaceListingResponse,
    MarketplaceListingStatus, MarketplaceListingTermsResponse, ReviewMarketplaceListingInput,
    SuspendMarketplaceListingInput,
};
use crate::entities::{listing, listing_terms};
use crate::error::{MarketplaceListingError, MarketplaceListingResult};
use crate::listing_events::{
    append_listing_event, list_listing_events, normalize_listing_event_locale,
};
use crate::MarketplaceListingService;

impl MarketplaceListingService {
    pub async fn list_events(
        &self,
        tenant_id: Uuid,
        request: ListMarketplaceListingEventsRequest,
    ) -> MarketplaceListingResult<Vec<MarketplaceListingEventResponse>> {
        ensure_listing_exists(self.database(), tenant_id, request.listing_id).await?;
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
            "note": note,
            "locale": locale,
        });
        let hash = request_hash("review_listing", actor_id, &normalized)?;

        match admit(
            self.database(),
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
                let result = review_listing_in_transaction(
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
            "reason": reason,
            "locale": locale,
        });
        let hash = request_hash("suspend_listing", actor_id, &normalized)?;

        match admit(
            self.database(),
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
                let result = suspend_listing_in_transaction(
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

async fn review_listing_in_transaction(
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
    active.approval_status = Set(
        if approved {
            MarketplaceListingApprovalStatus::Approved
        } else {
            MarketplaceListingApprovalStatus::Rejected
        }
        .as_str()
        .to_string(),
    );
    active.approval_note = Set(note.clone());
    active.approved_at = Set(approved.then(|| now.into()));
    active.updated_at = Set(now.into());
    active.update(&receipt.transaction).await?;

    append_listing_event(
        &receipt.transaction,
        tenant_id,
        listing_id,
        actor_id,
        event_kind,
        locale,
        note,
        serde_json::json!({"compatibility_snapshot": "approval_note"}),
    )
    .await?;
    load_listing_response(&receipt.transaction, tenant_id, listing_id).await
}

async fn suspend_listing_in_transaction(
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

    let now = Utc::now();
    let mut active: listing::ActiveModel = current.into();
    active.status = Set(MarketplaceListingStatus::Suspended.as_str().to_string());
    active.suspension_reason = Set(Some(reason.clone()));
    active.updated_at = Set(now.into());
    active.update(&receipt.transaction).await?;

    append_listing_event(
        &receipt.transaction,
        tenant_id,
        listing_id,
        actor_id,
        MarketplaceListingEventKind::Suspended,
        locale,
        Some(reason),
        serde_json::json!({"compatibility_snapshot": "suspension_reason"}),
    )
    .await?;
    load_listing_response(&receipt.transaction, tenant_id, listing_id).await
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

async fn ensure_listing_exists<C: ConnectionTrait>(
    connection: &C,
    tenant_id: Uuid,
    listing_id: Uuid,
) -> MarketplaceListingResult<()> {
    find_listing(connection, tenant_id, listing_id).await.map(|_| ())
}

async fn find_listing<C: ConnectionTrait>(
    connection: &C,
    tenant_id: Uuid,
    listing_id: Uuid,
) -> MarketplaceListingResult<listing::Model> {
    listing::Entity::find_by_id(listing_id)
        .filter(listing::Column::TenantId.eq(tenant_id))
        .one(connection)
        .await?
        .ok_or(MarketplaceListingError::ListingNotFound(listing_id))
}

async fn load_listing_response<C: ConnectionTrait>(
    connection: &C,
    tenant_id: Uuid,
    listing_id: Uuid,
) -> MarketplaceListingResult<MarketplaceListingResponse> {
    let model = find_listing(connection, tenant_id, listing_id).await?;
    let terms = listing_terms::Entity::find()
        .filter(listing_terms::Column::TenantId.eq(tenant_id))
        .filter(listing_terms::Column::ListingId.eq(listing_id))
        .filter(listing_terms::Column::Version.eq(model.current_terms_version))
        .one(connection)
        .await?
        .ok_or(MarketplaceListingError::TermsNotFound {
            listing_id,
            version: model.current_terms_version,
        })?;
    map_listing(model, terms)
}

fn map_listing(
    model: listing::Model,
    terms: listing_terms::Model,
) -> MarketplaceListingResult<MarketplaceListingResponse> {
    let status = MarketplaceListingStatus::parse(model.status.as_str()).ok_or_else(|| {
        MarketplaceListingError::Validation(format!(
            "unknown marketplace listing status `{}`",
            model.status
        ))
    })?;
    let approval_status = MarketplaceListingApprovalStatus::parse(model.approval_status.as_str())
        .ok_or_else(|| {
            MarketplaceListingError::Validation(format!(
                "unknown marketplace listing approval status `{}`",
                model.approval_status
            ))
        })?;
    Ok(MarketplaceListingResponse {
        id: model.id,
        tenant_id: model.tenant_id,
        seller_id: model.seller_id,
        master_product_id: model.master_product_id,
        master_variant_id: model.master_variant_id,
        seller_sku: model.seller_sku,
        market_slug: model.market_slug,
        channel_slug: model.channel_slug,
        status,
        approval_status,
        approval_note: model.approval_note,
        suspension_reason: model.suspension_reason,
        current_terms_version: model.current_terms_version,
        current_terms: MarketplaceListingTermsResponse {
            id: terms.id,
            listing_id: terms.listing_id,
            version: terms.version,
            pricing_reference: terms.pricing_reference,
            inventory_reference: terms.inventory_reference,
            fulfillment_profile_slug: terms.fulfillment_profile_slug,
            metadata: terms.metadata,
            created_at: terms.created_at,
        },
        metadata: model.metadata,
        published_at: model.published_at,
        approved_at: model.approved_at,
        created_at: model.created_at,
        updated_at: model.updated_at,
    })
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

fn required_idempotency_key(
    context: &rustok_api::PortContext,
) -> MarketplaceListingResult<String> {
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
