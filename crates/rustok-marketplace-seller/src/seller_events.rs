use rustok_core::generate_id;
use sea_orm::{
    ActiveModelTrait, ColumnTrait, ConnectionTrait, EntityTrait, QueryFilter, QueryOrder,
    QuerySelect, Set,
};
use serde_json::Value;
use uuid::Uuid;

use crate::dto::{
    MarketplaceSellerEventKind, MarketplaceSellerEventProvenance, MarketplaceSellerEventResponse,
    MarketplaceSellerOnboardingStatus, MarketplaceSellerResponse, MarketplaceSellerStatus,
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

pub(crate) async fn append_receipted_seller_event<C: ConnectionTrait>(
    connection: &C,
    tenant_id: Uuid,
    actor_id: Uuid,
    command_kind: &str,
    response_kind: &str,
    response_json: &Value,
) -> MarketplaceSellerResult<()> {
    if response_kind != "seller" {
        return Ok(());
    }

    let response: MarketplaceSellerResponse = serde_json::from_value(response_json.clone())
        .map_err(|_| {
            MarketplaceSellerError::Validation(
                "marketplace seller command result could not be mapped to an immutable event"
                    .to_string(),
            )
        })?;
    if response.tenant_id != tenant_id {
        return Err(MarketplaceSellerError::Validation(
            "marketplace seller command result tenant does not match its receipt".to_string(),
        ));
    }

    let status_metadata = || {
        serde_json::json!({
            "seller_status": response.status.as_str(),
            "onboarding_status": response.onboarding_status.as_str(),
        })
    };
    let (event_kind, note, metadata) = match command_kind {
        "create_seller" => {
            if response.status != MarketplaceSellerStatus::Draft
                || response.onboarding_status != MarketplaceSellerOnboardingStatus::Draft
            {
                return Err(MarketplaceSellerError::Validation(
                    "seller creation result is not draft".to_string(),
                ));
            }
            (MarketplaceSellerEventKind::Created, None, status_metadata())
        }
        "update_seller_profile" => (
            MarketplaceSellerEventKind::ProfileUpdated,
            None,
            status_metadata(),
        ),
        "submit_seller_onboarding" => {
            if response.status != MarketplaceSellerStatus::Draft
                || response.onboarding_status != MarketplaceSellerOnboardingStatus::Submitted
            {
                return Err(MarketplaceSellerError::Validation(
                    "onboarding submission result is not submitted".to_string(),
                ));
            }
            (
                MarketplaceSellerEventKind::OnboardingSubmitted,
                response.onboarding_note.clone(),
                status_metadata(),
            )
        }
        "review_seller_onboarding" => {
            let event_kind = match response.onboarding_status {
                MarketplaceSellerOnboardingStatus::Approved => {
                    MarketplaceSellerEventKind::OnboardingApproved
                }
                MarketplaceSellerOnboardingStatus::Rejected => {
                    MarketplaceSellerEventKind::OnboardingRejected
                }
                _ => {
                    return Err(MarketplaceSellerError::Validation(
                        "onboarding review result has no approved or rejected state".to_string(),
                    ));
                }
            };
            (
                event_kind,
                response.onboarding_note.clone(),
                status_metadata(),
            )
        }
        "suspend_seller" => {
            if response.status != MarketplaceSellerStatus::Suspended {
                return Err(MarketplaceSellerError::Validation(
                    "seller suspension result is not suspended".to_string(),
                ));
            }
            (
                MarketplaceSellerEventKind::Suspended,
                response.suspension_reason.clone(),
                status_metadata(),
            )
        }
        "reactivate_seller" => {
            if response.status != MarketplaceSellerStatus::Active
                || response.onboarding_status != MarketplaceSellerOnboardingStatus::Approved
            {
                return Err(MarketplaceSellerError::Validation(
                    "seller reactivation result is not active and approved".to_string(),
                ));
            }
            (
                MarketplaceSellerEventKind::Reactivated,
                None,
                status_metadata(),
            )
        }
        _ => {
            return Err(MarketplaceSellerError::Validation(format!(
                "seller response has no immutable event mapping for command `{command_kind}`"
            )));
        }
    };

    seller_event::ActiveModel {
        id: Set(generate_id()),
        tenant_id: Set(tenant_id),
        seller_id: Set(response.id),
        actor_id: Set(Some(actor_id)),
        event_kind: Set(event_kind.as_str().to_string()),
        locale: Set(Some(response.resolved_locale)),
        provenance: Set(MarketplaceSellerEventProvenance::Command
            .as_str()
            .to_string()),
        note: Set(note),
        metadata: Set(metadata),
        created_at: Set(response.updated_at),
    }
    .insert(connection)
    .await?;
    Ok(())
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
