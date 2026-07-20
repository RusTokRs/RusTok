use rustok_events::MarketplaceSellerEvent;
use serde_json::Value;

use crate::dto::{
    MarketplaceSellerMemberResponse, MarketplaceSellerOnboardingStatus, MarketplaceSellerResponse,
    MarketplaceSellerStatus,
};
use crate::error::{MarketplaceSellerError, MarketplaceSellerResult};

const RESPONSE_KIND_SELLER: &str = "seller";
const RESPONSE_KIND_MEMBER: &str = "member";

pub(crate) fn event_for_completed_command(
    command_kind: &str,
    response_kind: &str,
    response_json: &Value,
) -> MarketplaceSellerResult<MarketplaceSellerEvent> {
    match response_kind {
        RESPONSE_KIND_SELLER => {
            let seller: MarketplaceSellerResponse = serde_json::from_value(response_json.clone())
                .map_err(|_| event_invariant("seller response could not be decoded"))?;
            seller_event(command_kind, &seller)
        }
        RESPONSE_KIND_MEMBER => {
            let member: MarketplaceSellerMemberResponse =
                serde_json::from_value(response_json.clone())
                    .map_err(|_| event_invariant("member response could not be decoded"))?;
            member_event(command_kind, &member)
        }
        other => Err(event_invariant(format!(
            "unsupported completed response kind `{other}`"
        ))),
    }
}

fn seller_event(
    command_kind: &str,
    seller: &MarketplaceSellerResponse,
) -> MarketplaceSellerResult<MarketplaceSellerEvent> {
    let event = match command_kind {
        "create_seller" => MarketplaceSellerEvent::MarketplaceSellerCreated {
            seller_id: seller.id,
        },
        "update_seller_profile" => MarketplaceSellerEvent::MarketplaceSellerProfileUpdated {
            seller_id: seller.id,
        },
        "submit_seller_onboarding" => {
            if seller.onboarding_status != MarketplaceSellerOnboardingStatus::Submitted {
                return Err(event_invariant(
                    "completed onboarding submission is not submitted",
                ));
            }
            MarketplaceSellerEvent::MarketplaceSellerOnboardingSubmitted {
                seller_id: seller.id,
            }
        }
        "review_seller_onboarding" => match seller.onboarding_status {
            MarketplaceSellerOnboardingStatus::Approved => {
                MarketplaceSellerEvent::MarketplaceSellerOnboardingApproved {
                    seller_id: seller.id,
                }
            }
            MarketplaceSellerOnboardingStatus::Rejected => {
                MarketplaceSellerEvent::MarketplaceSellerOnboardingRejected {
                    seller_id: seller.id,
                }
            }
            status => {
                return Err(event_invariant(format!(
                    "completed onboarding review has incompatible status `{}`",
                    status.as_str()
                )))
            }
        },
        "suspend_seller" => {
            if seller.status != MarketplaceSellerStatus::Suspended {
                return Err(event_invariant("completed suspension is not suspended"));
            }
            MarketplaceSellerEvent::MarketplaceSellerSuspended {
                seller_id: seller.id,
            }
        }
        "reactivate_seller" => {
            if seller.status != MarketplaceSellerStatus::Active
                || seller.onboarding_status != MarketplaceSellerOnboardingStatus::Approved
            {
                return Err(event_invariant(
                    "completed reactivation is not active and approved",
                ));
            }
            MarketplaceSellerEvent::MarketplaceSellerReactivated {
                seller_id: seller.id,
            }
        }
        other => {
            return Err(event_invariant(format!(
                "seller response has no external event mapping for command `{other}`"
            )))
        }
    };
    Ok(event)
}

fn member_event(
    command_kind: &str,
    member: &MarketplaceSellerMemberResponse,
) -> MarketplaceSellerResult<MarketplaceSellerEvent> {
    let fields = || (
        member.seller_id,
        member.id,
        member.user_id,
        member.role.as_str().to_string(),
        member.status.as_str().to_string(),
    );
    let event = match command_kind {
        "add_seller_member" => {
            let (seller_id, member_id, user_id, role, status) = fields();
            MarketplaceSellerEvent::MarketplaceSellerMemberAdded {
                seller_id,
                member_id,
                user_id,
                role,
                status,
            }
        }
        "update_seller_member" => {
            let (seller_id, member_id, user_id, role, status) = fields();
            MarketplaceSellerEvent::MarketplaceSellerMemberUpdated {
                seller_id,
                member_id,
                user_id,
                role,
                status,
            }
        }
        other => {
            return Err(event_invariant(format!(
                "member response has no external event mapping for command `{other}`"
            )))
        }
    };
    Ok(event)
}

fn event_invariant(message: impl Into<String>) -> MarketplaceSellerError {
    MarketplaceSellerError::Validation(format!(
        "marketplace seller external event invariant failed: {}",
        message.into()
    ))
}
