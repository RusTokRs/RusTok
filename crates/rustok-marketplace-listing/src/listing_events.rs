use chrono::Utc;
use rustok_api::normalize_locale_tag;
use rustok_core::generate_id;
use sea_orm::{
    ActiveModelTrait, ColumnTrait, ConnectionTrait, EntityTrait, QueryFilter, QueryOrder,
    QuerySelect, Set,
};
use uuid::Uuid;

use crate::dto::{
    MarketplaceListingEventKind, MarketplaceListingEventProvenance,
    MarketplaceListingEventResponse,
};
use crate::entities::listing_event;
use crate::error::{MarketplaceListingError, MarketplaceListingResult};

const MAX_EVENTS_PER_READ: u64 = 200;

#[allow(clippy::too_many_arguments)]
pub(crate) async fn append_listing_event<C: ConnectionTrait>(
    connection: &C,
    tenant_id: Uuid,
    listing_id: Uuid,
    actor_id: Uuid,
    event_kind: MarketplaceListingEventKind,
    locale: &str,
    note: Option<String>,
    metadata: serde_json::Value,
) -> MarketplaceListingResult<MarketplaceListingEventResponse> {
    let locale = normalize_listing_event_locale(locale)?;
    let model = listing_event::ActiveModel {
        id: Set(generate_id()),
        tenant_id: Set(tenant_id),
        listing_id: Set(listing_id),
        actor_id: Set(Some(actor_id)),
        event_kind: Set(event_kind.as_str().to_string()),
        locale: Set(Some(locale)),
        provenance: Set(MarketplaceListingEventProvenance::Command
            .as_str()
            .to_string()),
        note: Set(note),
        metadata: Set(object_or_empty(metadata)?),
        created_at: Set(Utc::now().into()),
    }
    .insert(connection)
    .await?;
    map_listing_event(model)
}

pub(crate) async fn list_listing_events<C: ConnectionTrait>(
    connection: &C,
    tenant_id: Uuid,
    listing_id: Uuid,
    limit: u64,
) -> MarketplaceListingResult<Vec<MarketplaceListingEventResponse>> {
    listing_event::Entity::find()
        .filter(listing_event::Column::TenantId.eq(tenant_id))
        .filter(listing_event::Column::ListingId.eq(listing_id))
        .order_by_desc(listing_event::Column::CreatedAt)
        .order_by_desc(listing_event::Column::Id)
        .limit(limit.clamp(1, MAX_EVENTS_PER_READ))
        .all(connection)
        .await?
        .into_iter()
        .map(map_listing_event)
        .collect()
}

pub(crate) fn normalize_listing_event_locale(
    value: &str,
) -> MarketplaceListingResult<String> {
    normalize_locale_tag(value).ok_or_else(|| {
        MarketplaceListingError::Validation(
            "listing event locale must be a normalized BCP47-like tag with at most 32 bytes"
                .to_string(),
        )
    })
}

fn map_listing_event(
    model: listing_event::Model,
) -> MarketplaceListingResult<MarketplaceListingEventResponse> {
    let event_kind = MarketplaceListingEventKind::parse(model.event_kind.as_str()).ok_or_else(|| {
        MarketplaceListingError::Validation(format!(
            "unknown marketplace listing event kind `{}`",
            model.event_kind
        ))
    })?;
    let provenance = MarketplaceListingEventProvenance::parse(model.provenance.as_str())
        .ok_or_else(|| {
            MarketplaceListingError::Validation(format!(
                "unknown marketplace listing event provenance `{}`",
                model.provenance
            ))
        })?;
    match provenance {
        MarketplaceListingEventProvenance::Command
            if model.actor_id.is_none() || model.locale.is_none() =>
        {
            return Err(MarketplaceListingError::Validation(
                "command listing event is missing actor or locale attribution".to_string(),
            ));
        }
        MarketplaceListingEventProvenance::LegacySnapshot
            if model.actor_id.is_some() || model.locale.is_some() =>
        {
            return Err(MarketplaceListingError::Validation(
                "legacy listing snapshot must not fabricate actor or locale attribution"
                    .to_string(),
            ));
        }
        _ => {}
    }
    Ok(MarketplaceListingEventResponse {
        id: model.id,
        tenant_id: model.tenant_id,
        listing_id: model.listing_id,
        actor_id: model.actor_id,
        event_kind,
        locale: model.locale,
        provenance,
        note: model.note,
        metadata: model.metadata,
        created_at: model.created_at,
    })
}

fn object_or_empty(value: serde_json::Value) -> MarketplaceListingResult<serde_json::Value> {
    match value {
        serde_json::Value::Null => Ok(serde_json::json!({})),
        serde_json::Value::Object(_) => Ok(value),
        _ => Err(MarketplaceListingError::Validation(
            "listing event metadata must be a JSON object".to_string(),
        )),
    }
}
