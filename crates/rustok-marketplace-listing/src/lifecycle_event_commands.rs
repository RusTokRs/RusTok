use chrono::Utc;
use rustok_core::generate_id;
use sea_orm::{ActiveModelTrait, Set};
use uuid::Uuid;
use validator::Validate;

use crate::command_receipts::{
    admit, complete, normalize_idempotency_key, replay, request_hash, rollback,
    ListingCommandAdmission, NewListingCommandReceipt,
};
use crate::dto::{
    MarketplaceListingApprovalStatus, MarketplaceListingEventKind, MarketplaceListingResponse,
    MarketplaceListingStatus, UpdateMarketplaceListingTermsInput,
};
use crate::entities::{listing, listing_terms};
use crate::error::{MarketplaceListingError, MarketplaceListingResult};
use crate::listing_events::{append_listing_event, normalize_listing_event_locale};
use crate::service::{find_listing, load_response_for_model, map_listing};
use crate::MarketplaceListingService;

impl MarketplaceListingService {
    pub async fn update_terms_evented(
        &self,
        context: rustok_api::PortContext,
        input: UpdateMarketplaceListingTermsInput,
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
        let pricing_reference = normalize_optional_text(input.pricing_reference);
        let inventory_reference = normalize_optional_text(input.inventory_reference);
        let fulfillment_profile_slug = normalize_optional_text(input.fulfillment_profile_slug);
        let metadata = object_or_empty(input.metadata)?;
        let normalized = serde_json::json!({
            "listing_id": input.listing_id,
            "pricing_reference": pricing_reference.clone(),
            "inventory_reference": inventory_reference.clone(),
            "fulfillment_profile_slug": fulfillment_profile_slug.clone(),
            "metadata": metadata.clone(),
            "locale": locale.clone(),
        });
        let hash = request_hash("update_listing_terms", actor_id, &normalized)?;

        match admit(
            self.database(),
            self.event_bus().clone(),
            tenant_id,
            actor_id,
            key,
            "update_listing_terms",
            hash.as_str(),
        )
        .await?
        {
            ListingCommandAdmission::Replay(receipt) => {
                replay(receipt, "update_listing_terms", hash.as_str())
            }
            ListingCommandAdmission::New(receipt) => {
                let result = update_terms_in_transaction(
                    &receipt,
                    tenant_id,
                    actor_id,
                    locale.as_str(),
                    input.listing_id,
                    pricing_reference,
                    inventory_reference,
                    fulfillment_profile_slug,
                    metadata,
                )
                .await;
                finish(receipt, result).await
            }
        }
    }

    pub async fn submit_for_review_evented(
        &self,
        context: rustok_api::PortContext,
        listing_id: Uuid,
    ) -> MarketplaceListingResult<MarketplaceListingResponse> {
        context
            .require_write_semantics()
            .map_err(|error| MarketplaceListingError::Validation(error.message))?;
        let tenant_id = parse_tenant_id(&context)?;
        let actor_id = parse_actor_id(&context)?;
        let locale = normalize_listing_event_locale(context.locale.as_str())?;
        let key = normalize_idempotency_key(required_idempotency_key(&context)?)?;
        let normalized = serde_json::json!({
            "listing_id": listing_id,
            "locale": locale.clone(),
        });
        let hash = request_hash("submit_listing_for_review", actor_id, &normalized)?;

        match admit(
            self.database(),
            self.event_bus().clone(),
            tenant_id,
            actor_id,
            key,
            "submit_listing_for_review",
            hash.as_str(),
        )
        .await?
        {
            ListingCommandAdmission::Replay(receipt) => {
                replay(receipt, "submit_listing_for_review", hash.as_str())
            }
            ListingCommandAdmission::New(receipt) => {
                let result = transition_in_transaction(
                    &receipt,
                    tenant_id,
                    actor_id,
                    locale.as_str(),
                    listing_id,
                    MarketplaceListingEventKind::SubmittedForReview,
                    MarketplaceListingStatus::PendingReview,
                    MarketplaceListingApprovalStatus::Pending,
                )
                .await;
                finish(receipt, result).await
            }
        }
    }

    pub async fn archive_listing_evented(
        &self,
        context: rustok_api::PortContext,
        listing_id: Uuid,
    ) -> MarketplaceListingResult<MarketplaceListingResponse> {
        context
            .require_write_semantics()
            .map_err(|error| MarketplaceListingError::Validation(error.message))?;
        let tenant_id = parse_tenant_id(&context)?;
        let actor_id = parse_actor_id(&context)?;
        let locale = normalize_listing_event_locale(context.locale.as_str())?;
        let key = normalize_idempotency_key(required_idempotency_key(&context)?)?;
        let normalized = serde_json::json!({
            "listing_id": listing_id,
            "locale": locale.clone(),
        });
        let hash = request_hash("archive_listing", actor_id, &normalized)?;

        match admit(
            self.database(),
            self.event_bus().clone(),
            tenant_id,
            actor_id,
            key,
            "archive_listing",
            hash.as_str(),
        )
        .await?
        {
            ListingCommandAdmission::Replay(receipt) => {
                replay(receipt, "archive_listing", hash.as_str())
            }
            ListingCommandAdmission::New(receipt) => {
                let result = archive_in_transaction(
                    &receipt,
                    tenant_id,
                    actor_id,
                    locale.as_str(),
                    listing_id,
                )
                .await;
                finish(receipt, result).await
            }
        }
    }
}

#[allow(clippy::too_many_arguments)]
async fn update_terms_in_transaction(
    receipt: &NewListingCommandReceipt,
    tenant_id: Uuid,
    actor_id: Uuid,
    locale: &str,
    listing_id: Uuid,
    pricing_reference: Option<String>,
    inventory_reference: Option<String>,
    fulfillment_profile_slug: Option<String>,
    metadata: serde_json::Value,
) -> MarketplaceListingResult<MarketplaceListingResponse> {
    let current = find_listing(&receipt.transaction, tenant_id, listing_id).await?;
    if current.status == MarketplaceListingStatus::Archived.as_str() {
        return Err(MarketplaceListingError::InvalidTransition {
            from: current.status,
            to: "terms_updated".to_string(),
        });
    }
    let next_version = current
        .current_terms_version
        .checked_add(1)
        .ok_or_else(|| {
            MarketplaceListingError::Validation("listing terms version overflow".to_string())
        })?;
    let terms = listing_terms::ActiveModel {
        id: Set(generate_id()),
        tenant_id: Set(tenant_id),
        listing_id: Set(listing_id),
        version: Set(next_version),
        pricing_reference: Set(pricing_reference),
        inventory_reference: Set(inventory_reference),
        fulfillment_profile_slug: Set(fulfillment_profile_slug),
        metadata: Set(metadata),
        created_at: Set(Utc::now().into()),
    }
    .insert(&receipt.transaction)
    .await?;

    let mut active: listing::ActiveModel = current.into();
    active.current_terms_version = Set(next_version);
    active.status = Set(MarketplaceListingStatus::Draft.as_str().to_string());
    active.approval_status = Set(MarketplaceListingApprovalStatus::Draft.as_str().to_string());
    active.approved_at = Set(None);
    active.published_at = Set(None);
    active.updated_at = Set(Utc::now().into());
    let model = active.update(&receipt.transaction).await?;

    append_listing_event(
        &receipt.transaction,
        tenant_id,
        listing_id,
        actor_id,
        MarketplaceListingEventKind::TermsUpdated,
        locale,
        None,
        serde_json::json!({"terms_version": next_version}),
    )
    .await?;
    map_listing(model, terms)
}

async fn transition_in_transaction(
    receipt: &NewListingCommandReceipt,
    tenant_id: Uuid,
    actor_id: Uuid,
    locale: &str,
    listing_id: Uuid,
    event_kind: MarketplaceListingEventKind,
    target_status: MarketplaceListingStatus,
    target_approval: MarketplaceListingApprovalStatus,
) -> MarketplaceListingResult<MarketplaceListingResponse> {
    let current = find_listing(&receipt.transaction, tenant_id, listing_id).await?;
    if current.status != MarketplaceListingStatus::Draft.as_str()
        || ![
            MarketplaceListingApprovalStatus::Draft.as_str(),
            MarketplaceListingApprovalStatus::Rejected.as_str(),
        ]
        .contains(&current.approval_status.as_str())
    {
        return Err(MarketplaceListingError::InvalidTransition {
            from: format!("{}:{}", current.status, current.approval_status),
            to: format!("{}:{}", target_status.as_str(), target_approval.as_str()),
        });
    }

    let mut active: listing::ActiveModel = current.into();
    active.status = Set(target_status.as_str().to_string());
    active.approval_status = Set(target_approval.as_str().to_string());
    active.updated_at = Set(Utc::now().into());
    let model = active.update(&receipt.transaction).await?;
    append_listing_event(
        &receipt.transaction,
        tenant_id,
        listing_id,
        actor_id,
        event_kind,
        locale,
        None,
        serde_json::json!({}),
    )
    .await?;
    load_response_for_model(&receipt.transaction, model).await
}

async fn archive_in_transaction(
    receipt: &NewListingCommandReceipt,
    tenant_id: Uuid,
    actor_id: Uuid,
    locale: &str,
    listing_id: Uuid,
) -> MarketplaceListingResult<MarketplaceListingResponse> {
    let current = find_listing(&receipt.transaction, tenant_id, listing_id).await?;
    if current.status == MarketplaceListingStatus::Archived.as_str() {
        return Err(MarketplaceListingError::InvalidTransition {
            from: current.status,
            to: MarketplaceListingStatus::Archived.as_str().to_string(),
        });
    }
    let mut active: listing::ActiveModel = current.into();
    active.status = Set(MarketplaceListingStatus::Archived.as_str().to_string());
    active.published_at = Set(None);
    active.updated_at = Set(Utc::now().into());
    let model = active.update(&receipt.transaction).await?;
    append_listing_event(
        &receipt.transaction,
        tenant_id,
        listing_id,
        actor_id,
        MarketplaceListingEventKind::Archived,
        locale,
        None,
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

fn normalize_optional_text(value: Option<String>) -> Option<String> {
    value.and_then(|value| {
        let value = value.trim();
        (!value.is_empty()).then(|| value.to_string())
    })
}

fn object_or_empty(value: serde_json::Value) -> MarketplaceListingResult<serde_json::Value> {
    match value {
        serde_json::Value::Null => Ok(serde_json::json!({})),
        serde_json::Value::Object(_) => Ok(value),
        _ => Err(MarketplaceListingError::Validation(
            "listing terms metadata must be a JSON object".to_string(),
        )),
    }
}
