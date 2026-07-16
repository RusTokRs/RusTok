use uuid::Uuid;
use validator::Validate;

use crate::command_receipts::{
    normalize_idempotency_key, replay_existing, request_hash,
};
use crate::dto::{CreateMarketplaceListingInput, MarketplaceListingResponse};
use crate::error::{MarketplaceListingError, MarketplaceListingResult};
use crate::MarketplaceListingService;

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
        let key = normalize_idempotency_key(required_idempotency_key(&context)?)?;
        let normalized = serde_json::json!({
            "seller_id": input.seller_id,
            "master_variant_id": input.master_variant_id,
            "seller_sku": required_text(input.seller_sku.as_str(), "seller_sku")?,
            "market_slug": required_slug(input.market_slug.as_str(), "market_slug")?,
            "channel_slug": required_slug(input.channel_slug.as_str(), "channel_slug")?,
            "pricing_reference": normalize_optional_text(input.pricing_reference.as_deref()),
            "inventory_reference": normalize_optional_text(input.inventory_reference.as_deref()),
            "fulfillment_profile_slug": normalize_optional_text(input.fulfillment_profile_slug.as_deref()),
            "metadata": object_or_empty(input.metadata.clone(), "metadata")?,
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
        self.create_listing(context, input).await
    }

    pub async fn publish_listing_replay_safe(
        &self,
        context: rustok_api::PortContext,
        listing_id: Uuid,
    ) -> MarketplaceListingResult<MarketplaceListingResponse> {
        self.replay_safe_lifecycle(context, "publish_listing", listing_id)
            .await?
            .map_or_else(
                || async { self.publish_listing(context, listing_id).await },
                |response| async { Ok(response) },
            )
            .await
    }

    pub async fn reactivate_listing_replay_safe(
        &self,
        context: rustok_api::PortContext,
        listing_id: Uuid,
    ) -> MarketplaceListingResult<MarketplaceListingResponse> {
        self.replay_safe_lifecycle(context, "reactivate_listing", listing_id)
            .await?
            .map_or_else(
                || async { self.reactivate_listing(context, listing_id).await },
                |response| async { Ok(response) },
            )
            .await
    }

    async fn replay_safe_lifecycle(
        &self,
        context: rustok_api::PortContext,
        command_kind: &'static str,
        listing_id: Uuid,
    ) -> MarketplaceListingResult<Option<MarketplaceListingResponse>> {
        context
            .require_write_semantics()
            .map_err(|error| MarketplaceListingError::Validation(error.message))?;
        let tenant_id = parse_tenant_id(&context)?;
        let actor_id = parse_actor_id(&context)?;
        let key = normalize_idempotency_key(required_idempotency_key(&context)?)?;
        let hash = request_hash(
            command_kind,
            actor_id,
            &serde_json::json!({"listing_id": listing_id}),
        )?;
        replay_existing(
            self.database(),
            tenant_id,
            key.as_str(),
            command_kind,
            hash.as_str(),
        )
        .await
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

fn required_idempotency_key(
    context: &rustok_api::PortContext,
) -> MarketplaceListingResult<String> {
    context.idempotency_key.clone().ok_or_else(|| {
        MarketplaceListingError::Validation(
            "marketplace listing write requires an idempotency key".to_string(),
        )
    })
}

fn required_text(value: &str, field: &str) -> MarketplaceListingResult<String> {
    let value = value.trim();
    if value.is_empty() {
        return Err(MarketplaceListingError::Validation(format!(
            "{field} must not be empty"
        )));
    }
    Ok(value.to_string())
}

fn required_slug(value: &str, field: &str) -> MarketplaceListingResult<String> {
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

fn normalize_optional_text(value: Option<&str>) -> Option<String> {
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
