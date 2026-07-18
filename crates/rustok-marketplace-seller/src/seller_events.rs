use sea_orm::{ColumnTrait, ConnectionTrait, EntityTrait, QueryFilter, QueryOrder, QuerySelect};
use uuid::Uuid;

use crate::dto::{
    MarketplaceSellerEventKind, MarketplaceSellerEventProvenance, MarketplaceSellerEventResponse,
};
use crate::entities::seller_event;
use crate::error::{MarketplaceSellerError, MarketplaceSellerResult};

const MAX_EVENTS_PER_READ: u64 = 200;

pub(crate) async fn list_seller_events<C: ConnectionTrait>(
    connection: &C,
    tenant_id: Uuid,
    seller_id: Uuid,
    limit: u64,
) -> MarketplaceSellerResult<Vec<MarketplaceSellerEventResponse>> {
    seller_event::Entity::find()
        .filter(seller_event::Column::TenantId.eq(tenant_id))
        .filter(seller_event::Column::SellerId.eq(seller_id))
        .order_by_desc(seller_event::Column::CreatedAt)
        .order_by_desc(seller_event::Column::Id)
        .limit(limit.clamp(1, MAX_EVENTS_PER_READ))
        .all(connection)
        .await?
        .into_iter()
        .map(map_seller_event)
        .collect()
}

fn map_seller_event(
    model: seller_event::Model,
) -> MarketplaceSellerResult<MarketplaceSellerEventResponse> {
    let event_kind =
        MarketplaceSellerEventKind::parse(model.event_kind.as_str()).ok_or_else(|| {
            MarketplaceSellerError::Validation(format!(
                "unknown marketplace seller event kind `{}`",
                model.event_kind
            ))
        })?;
    let provenance = MarketplaceSellerEventProvenance::parse(model.provenance.as_str())
        .ok_or_else(|| {
            MarketplaceSellerError::Validation(format!(
                "unknown marketplace seller event provenance `{}`",
                model.provenance
            ))
        })?;
    match provenance {
        MarketplaceSellerEventProvenance::Command
            if model.actor_id.is_none() || model.locale.is_none() =>
        {
            return Err(MarketplaceSellerError::Validation(
                "command seller event is missing actor or locale attribution".to_string(),
            ));
        }
        MarketplaceSellerEventProvenance::LegacySnapshot
            if model.actor_id.is_some() || model.locale.is_some() =>
        {
            return Err(MarketplaceSellerError::Validation(
                "legacy seller snapshot must not fabricate actor or locale attribution".to_string(),
            ));
        }
        _ => {}
    }
    Ok(MarketplaceSellerEventResponse {
        id: model.id,
        tenant_id: model.tenant_id,
        seller_id: model.seller_id,
        actor_id: model.actor_id,
        event_kind,
        locale: model.locale,
        provenance,
        note: model.note,
        metadata: model.metadata,
        created_at: model.created_at,
    })
}
