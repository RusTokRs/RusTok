use chrono::Utc;
use rustok_core::generate_id;
use rustok_marketplace_seller::{MarketplaceSellerStatus, ReadMarketplaceSellerRequest};
use rustok_product::VariantProductProjectionRequest;
use sea_orm::{ActiveModelTrait, Set};
use uuid::Uuid;
use validator::Validate;

use crate::MarketplaceListingService;
use crate::command_receipts::{
    ListingCommandAdmission, NewListingCommandReceipt, admit, complete, normalize_idempotency_key,
    replay, replay_existing, request_hash, rollback,
};
use crate::dto::{
    CreateMarketplaceListingInput, MarketplaceListingApprovalStatus, MarketplaceListingEventKind,
    MarketplaceListingResponse, MarketplaceListingStatus,
};
use crate::entities::{listing, listing_terms};
use crate::error::{MarketplaceListingError, MarketplaceListingResult};
use crate::listing_events::{append_listing_event, normalize_listing_event_locale};
use crate::service::{
    ensure_listing_identity_available, find_listing, listing_reason_codes_without_lifecycle,
    load_response_for_model, map_listing, map_listing_insert_error, map_product_port_error,
    map_seller_port_error,
};

impl MarketplaceListingService {
    pub async fn create_listing_replay_safe(
        &self,
        context: rustok_api::PortContext,
        input: CreateMarketplaceListingInput,
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
        let seller_sku = required_text(input.seller_sku, "seller_sku")?;
        let market_slug = required_slug(input.market_slug, "market_slug")?;
        let channel_slug = required_slug(input.channel_slug, "channel_slug")?;
        let pricing_reference = normalize_optional_text(input.pricing_reference);
        let inventory_reference = normalize_optional_text(input.inventory_reference);
        let fulfillment_profile_slug = normalize_optional_text(input.fulfillment_profile_slug);
        let metadata = object_or_empty(input.metadata, "metadata")?;
        let normalized = serde_json::json!({
            "seller_id": input.seller_id,
            "master_variant_id": input.master_variant_id,
            "seller_sku": seller_sku.clone(),
            "market_slug": market_slug.clone(),
            "channel_slug": channel_slug.clone(),
            "pricing_reference": pricing_reference.clone(),
            "inventory_reference": inventory_reference.clone(),
            "fulfillment_profile_slug": fulfillment_profile_slug.clone(),
            "metadata": metadata.clone(),
            "locale": locale.clone(),
        });
        let hash = request_hash("create_listing", actor_id, &normalized)?;
        if let Some(response) = replay_existing(
            self.database(),
            tenant_id,
            key.as_str(),
            "create_listing",
            hash.as_str(),
        )
        .await?
        {
            return Ok(response);
        }

        let seller = self
            .seller_reader()
            .read_seller(
                context.clone(),
                ReadMarketplaceSellerRequest {
                    seller_id: input.seller_id,
                },
            )
            .await
            .map_err(map_seller_port_error)?;
        if seller.status == MarketplaceSellerStatus::Closed {
            return Err(MarketplaceListingError::Validation(
                "closed seller cannot create listings".to_string(),
            ));
        }
        let product = self
            .product_reader()
            .read_variant_product_projection(
                context.clone(),
                VariantProductProjectionRequest {
                    variant_id: input.master_variant_id,
                    locale: None,
                    fallback_locale: None,
                },
            )
            .await
            .map_err(map_product_port_error)?;

        match admit(
            self.database(),
            self.event_bus().clone(),
            tenant_id,
            actor_id,
            key,
            "create_listing",
            hash.as_str(),
        )
        .await?
        {
            ListingCommandAdmission::Replay(receipt) => {
                replay(receipt, "create_listing", hash.as_str())
            }
            ListingCommandAdmission::New(receipt) => {
                let result = create_in_transaction(
                    &receipt,
                    tenant_id,
                    actor_id,
                    locale.as_str(),
                    input.seller_id,
                    product.id,
                    input.master_variant_id,
                    seller_sku,
                    market_slug,
                    channel_slug,
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

    pub async fn publish_listing_replay_safe(
        &self,
        context: rustok_api::PortContext,
        listing_id: Uuid,
    ) -> MarketplaceListingResult<MarketplaceListingResponse> {
        self.provider_lifecycle_evented(
            context,
            listing_id,
            "publish_listing",
            MarketplaceListingStatus::Draft,
            MarketplaceListingEventKind::Published,
        )
        .await
    }

    pub async fn reactivate_listing_replay_safe(
        &self,
        context: rustok_api::PortContext,
        listing_id: Uuid,
    ) -> MarketplaceListingResult<MarketplaceListingResponse> {
        self.provider_lifecycle_evented(
            context,
            listing_id,
            "reactivate_listing",
            MarketplaceListingStatus::Suspended,
            MarketplaceListingEventKind::Reactivated,
        )
        .await
    }

    async fn provider_lifecycle_evented(
        &self,
        context: rustok_api::PortContext,
        listing_id: Uuid,
        command_kind: &'static str,
        expected_status: MarketplaceListingStatus,
        event_kind: MarketplaceListingEventKind,
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
        let hash = request_hash(command_kind, actor_id, &normalized)?;
        if let Some(response) = replay_existing(
            self.database(),
            tenant_id,
            key.as_str(),
            command_kind,
            hash.as_str(),
        )
        .await?
        {
            return Ok(response);
        }

        let current = self.get_listing(tenant_id, listing_id).await?;
        let seller = self
            .seller_reader()
            .read_seller(
                context,
                ReadMarketplaceSellerRequest {
                    seller_id: current.seller_id,
                },
            )
            .await
            .map_err(map_seller_port_error)?;
        if seller.status != MarketplaceSellerStatus::Active {
            return Err(MarketplaceListingError::Validation(format!(
                "listing cannot execute {command_kind} while seller is not active"
            )));
        }
        let reasons = listing_reason_codes_without_lifecycle(&current);
        if !reasons.is_empty() {
            return Err(MarketplaceListingError::Validation(format!(
                "listing cannot execute {command_kind}: {}",
                reasons.join(",")
            )));
        }

        match admit(
            self.database(),
            self.event_bus().clone(),
            tenant_id,
            actor_id,
            key,
            command_kind,
            hash.as_str(),
        )
        .await?
        {
            ListingCommandAdmission::Replay(receipt) => {
                replay(receipt, command_kind, hash.as_str())
            }
            ListingCommandAdmission::New(receipt) => {
                let result = activate_in_transaction(
                    &receipt,
                    tenant_id,
                    actor_id,
                    locale.as_str(),
                    listing_id,
                    expected_status,
                    event_kind,
                )
                .await;
                finish(receipt, result).await
            }
        }
    }
}

#[allow(clippy::too_many_arguments)]
async fn create_in_transaction(
    receipt: &NewListingCommandReceipt,
    tenant_id: Uuid,
    actor_id: Uuid,
    locale: &str,
    seller_id: Uuid,
    master_product_id: Uuid,
    master_variant_id: Uuid,
    seller_sku: String,
    market_slug: String,
    channel_slug: String,
    pricing_reference: Option<String>,
    inventory_reference: Option<String>,
    fulfillment_profile_slug: Option<String>,
    metadata: serde_json::Value,
) -> MarketplaceListingResult<MarketplaceListingResponse> {
    ensure_listing_identity_available(
        &receipt.transaction,
        tenant_id,
        seller_id,
        master_variant_id,
        market_slug.as_str(),
        channel_slug.as_str(),
        seller_sku.as_str(),
    )
    .await?;
    let listing_id = generate_id();
    let now = Utc::now();
    let listing_model = listing::ActiveModel {
        id: Set(listing_id),
        tenant_id: Set(tenant_id),
        seller_id: Set(seller_id),
        master_product_id: Set(master_product_id),
        master_variant_id: Set(master_variant_id),
        seller_sku: Set(seller_sku),
        market_slug: Set(market_slug),
        channel_slug: Set(channel_slug),
        status: Set(MarketplaceListingStatus::Draft.as_str().to_string()),
        approval_status: Set(MarketplaceListingApprovalStatus::Draft.as_str().to_string()),
        current_terms_version: Set(1),
        metadata: Set(metadata),
        published_at: Set(None),
        approved_at: Set(None),
        created_at: Set(now.into()),
        updated_at: Set(now.into()),
    }
    .insert(&receipt.transaction)
    .await
    .map_err(map_listing_insert_error)?;
    let terms_model = listing_terms::ActiveModel {
        id: Set(generate_id()),
        tenant_id: Set(tenant_id),
        listing_id: Set(listing_id),
        version: Set(1),
        pricing_reference: Set(pricing_reference),
        inventory_reference: Set(inventory_reference),
        fulfillment_profile_slug: Set(fulfillment_profile_slug),
        metadata: Set(serde_json::json!({})),
        created_at: Set(now.into()),
    }
    .insert(&receipt.transaction)
    .await?;
    append_listing_event(
        &receipt.transaction,
        tenant_id,
        listing_id,
        actor_id,
        MarketplaceListingEventKind::Created,
        locale,
        None,
        serde_json::json!({
            "seller_id": seller_id,
            "master_product_id": master_product_id,
            "master_variant_id": master_variant_id,
            "terms_version": 1,
        }),
    )
    .await?;
    map_listing(listing_model, terms_model)
}

async fn activate_in_transaction(
    receipt: &NewListingCommandReceipt,
    tenant_id: Uuid,
    actor_id: Uuid,
    locale: &str,
    listing_id: Uuid,
    expected_status: MarketplaceListingStatus,
    event_kind: MarketplaceListingEventKind,
) -> MarketplaceListingResult<MarketplaceListingResponse> {
    let current = find_listing(&receipt.transaction, tenant_id, listing_id).await?;
    if current.status != expected_status.as_str() {
        return Err(MarketplaceListingError::InvalidTransition {
            from: current.status,
            to: MarketplaceListingStatus::Active.as_str().to_string(),
        });
    }
    if current.approval_status != MarketplaceListingApprovalStatus::Approved.as_str() {
        return Err(MarketplaceListingError::InvalidTransition {
            from: current.approval_status,
            to: MarketplaceListingStatus::Active.as_str().to_string(),
        });
    }
    let response = load_response_for_model(&receipt.transaction, current.clone()).await?;
    let reasons = listing_reason_codes_without_lifecycle(&response);
    if !reasons.is_empty() {
        return Err(MarketplaceListingError::Validation(format!(
            "listing cannot become active: {}",
            reasons.join(",")
        )));
    }

    let now = Utc::now();
    let mut active: listing::ActiveModel = current.into();
    active.status = Set(MarketplaceListingStatus::Active.as_str().to_string());
    active.published_at = Set(Some(now.into()));
    active.updated_at = Set(now.into());
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

fn required_slug(value: String, field: &str) -> MarketplaceListingResult<String> {
    let value = value.trim().to_ascii_lowercase().replace('_', "-");
    if value.is_empty()
        || value.starts_with('-')
        || value.ends_with('-')
        || value
            .chars()
            .any(|character| !(character.is_ascii_alphanumeric() || character == '-'))
    {
        return Err(MarketplaceListingError::Validation(format!(
            "{field} must contain lowercase ASCII letters, digits, or internal hyphens"
        )));
    }
    Ok(value)
}

fn normalize_optional_text(value: Option<String>) -> Option<String> {
    value.and_then(|value| {
        let value = value.trim();
        (!value.is_empty()).then(|| value.to_string())
    })
}

fn object_or_empty(
    value: serde_json::Value,
    field: &str,
) -> MarketplaceListingResult<serde_json::Value> {
    match value {
        serde_json::Value::Null => Ok(serde_json::json!({})),
        serde_json::Value::Object(_) => Ok(value),
        _ => Err(MarketplaceListingError::Validation(format!(
            "{field} must be a JSON object"
        ))),
    }
}
