use std::collections::HashMap;

use chrono::{DateTime, FixedOffset};
use sea_orm::{ColumnTrait, ConnectionTrait, EntityTrait, QueryFilter, QueryOrder};
use uuid::Uuid;

use crate::dto::MarketplaceSellerEventKind;
use crate::entities::seller_event;
use crate::error::MarketplaceSellerResult;

#[derive(Clone, Debug, Default)]
pub(crate) struct SellerProseProjection {
    pub onboarding_note: Option<String>,
    pub onboarding_at: Option<DateTime<FixedOffset>>,
    pub suspension_reason: Option<String>,
    pub suspension_at: Option<DateTime<FixedOffset>>,
}

#[derive(Default)]
struct SellerProseState {
    projection: SellerProseProjection,
    onboarding_seen: bool,
    suspension_seen: bool,
}

pub(crate) async fn load_seller_prose<C: ConnectionTrait>(
    connection: &C,
    tenant_id: Uuid,
    seller_id: Uuid,
) -> MarketplaceSellerResult<SellerProseProjection> {
    Ok(load_seller_prose_map(connection, tenant_id, vec![seller_id])
        .await?
        .remove(&seller_id)
        .unwrap_or_default())
}

pub(crate) async fn load_seller_prose_map<C: ConnectionTrait>(
    connection: &C,
    tenant_id: Uuid,
    seller_ids: Vec<Uuid>,
) -> MarketplaceSellerResult<HashMap<Uuid, SellerProseProjection>> {
    if seller_ids.is_empty() {
        return Ok(HashMap::new());
    }
    let relevant = [
        MarketplaceSellerEventKind::OnboardingSubmitted.as_str(),
        MarketplaceSellerEventKind::OnboardingApproved.as_str(),
        MarketplaceSellerEventKind::OnboardingRejected.as_str(),
        MarketplaceSellerEventKind::Suspended.as_str(),
        MarketplaceSellerEventKind::Reactivated.as_str(),
        MarketplaceSellerEventKind::LegacyOnboardingSnapshot.as_str(),
        MarketplaceSellerEventKind::LegacySuspensionSnapshot.as_str(),
    ];
    let events = seller_event::Entity::find()
        .filter(seller_event::Column::TenantId.eq(tenant_id))
        .filter(seller_event::Column::SellerId.is_in(seller_ids))
        .filter(seller_event::Column::EventKind.is_in(relevant))
        .order_by_desc(seller_event::Column::CreatedAt)
        .order_by_desc(seller_event::Column::Id)
        .all(connection)
        .await?;

    let mut states = HashMap::<Uuid, SellerProseState>::new();
    for event in events {
        let Some(kind) = MarketplaceSellerEventKind::parse(event.event_kind.as_str()) else {
            continue;
        };
        let state = states.entry(event.seller_id).or_default();
        match kind {
            MarketplaceSellerEventKind::OnboardingSubmitted
            | MarketplaceSellerEventKind::OnboardingApproved
            | MarketplaceSellerEventKind::OnboardingRejected
            | MarketplaceSellerEventKind::LegacyOnboardingSnapshot
                if !state.onboarding_seen =>
            {
                state.projection.onboarding_note = event.note;
                state.projection.onboarding_at = Some(event.created_at);
                state.onboarding_seen = true;
            }
            MarketplaceSellerEventKind::Suspended
            | MarketplaceSellerEventKind::LegacySuspensionSnapshot
                if !state.suspension_seen =>
            {
                state.projection.suspension_reason = event.note;
                state.projection.suspension_at = Some(event.created_at);
                state.suspension_seen = true;
            }
            MarketplaceSellerEventKind::Reactivated if !state.suspension_seen => {
                state.projection.suspension_reason = None;
                state.projection.suspension_at = Some(event.created_at);
                state.suspension_seen = true;
            }
            _ => {}
        }
    }
    Ok(states
        .into_iter()
        .map(|(seller_id, state)| (seller_id, state.projection))
        .collect())
}
