use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::contract::{sealed, ContractEventPayload, EventContract};
use crate::validation::{validators, EventValidationError, ValidateEvent};
use crate::{EventSchema, FieldSchema};

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
#[serde(tag = "type", content = "data")]
pub enum MarketplaceSellerEvent {
    MarketplaceSellerCreated { seller_id: Uuid },
    MarketplaceSellerProfileUpdated { seller_id: Uuid },
    MarketplaceSellerOnboardingSubmitted { seller_id: Uuid },
    MarketplaceSellerOnboardingApproved { seller_id: Uuid },
    MarketplaceSellerOnboardingRejected { seller_id: Uuid },
    MarketplaceSellerSuspended { seller_id: Uuid },
    MarketplaceSellerReactivated { seller_id: Uuid },
    MarketplaceSellerMemberAdded {
        seller_id: Uuid,
        member_id: Uuid,
        user_id: Uuid,
        role: String,
        status: String,
    },
    MarketplaceSellerMemberUpdated {
        seller_id: Uuid,
        member_id: Uuid,
        user_id: Uuid,
        role: String,
        status: String,
    },
}

impl MarketplaceSellerEvent {
    pub fn event_type(&self) -> &'static str {
        match self {
            Self::MarketplaceSellerCreated { .. } => "marketplace.seller.created",
            Self::MarketplaceSellerProfileUpdated { .. } => "marketplace.seller.profile_updated",
            Self::MarketplaceSellerOnboardingSubmitted { .. } => {
                "marketplace.seller.onboarding_submitted"
            }
            Self::MarketplaceSellerOnboardingApproved { .. } => {
                "marketplace.seller.onboarding_approved"
            }
            Self::MarketplaceSellerOnboardingRejected { .. } => {
                "marketplace.seller.onboarding_rejected"
            }
            Self::MarketplaceSellerSuspended { .. } => "marketplace.seller.suspended",
            Self::MarketplaceSellerReactivated { .. } => "marketplace.seller.reactivated",
            Self::MarketplaceSellerMemberAdded { .. } => "marketplace.seller.member_added",
            Self::MarketplaceSellerMemberUpdated { .. } => "marketplace.seller.member_updated",
        }
    }

    pub fn schema_version(&self) -> u16 {
        1
    }

    fn seller_id(&self) -> &Uuid {
        match self {
            Self::MarketplaceSellerCreated { seller_id }
            | Self::MarketplaceSellerProfileUpdated { seller_id }
            | Self::MarketplaceSellerOnboardingSubmitted { seller_id }
            | Self::MarketplaceSellerOnboardingApproved { seller_id }
            | Self::MarketplaceSellerOnboardingRejected { seller_id }
            | Self::MarketplaceSellerSuspended { seller_id }
            | Self::MarketplaceSellerReactivated { seller_id }
            | Self::MarketplaceSellerMemberAdded { seller_id, .. }
            | Self::MarketplaceSellerMemberUpdated { seller_id, .. } => seller_id,
        }
    }
}

const SELLER_EVENT_FIELDS: &[FieldSchema] = &[FieldSchema {
    name: "seller_id",
    data_type: "uuid",
    optional: false,
}];

const SELLER_MEMBER_EVENT_FIELDS: &[FieldSchema] = &[
    FieldSchema {
        name: "seller_id",
        data_type: "uuid",
        optional: false,
    },
    FieldSchema {
        name: "member_id",
        data_type: "uuid",
        optional: false,
    },
    FieldSchema {
        name: "user_id",
        data_type: "uuid",
        optional: false,
    },
    FieldSchema {
        name: "role",
        data_type: "string",
        optional: false,
    },
    FieldSchema {
        name: "status",
        data_type: "string",
        optional: false,
    },
];

pub const MARKETPLACE_SELLER_EVENT_SCHEMAS: &[EventSchema] = &[
    EventSchema {
        event_type: "marketplace.seller.created",
        version: 1,
        description: "A marketplace seller was created.",
        fields: SELLER_EVENT_FIELDS,
    },
    EventSchema {
        event_type: "marketplace.seller.profile_updated",
        version: 1,
        description: "A marketplace seller profile was updated.",
        fields: SELLER_EVENT_FIELDS,
    },
    EventSchema {
        event_type: "marketplace.seller.onboarding_submitted",
        version: 1,
        description: "A marketplace seller submitted onboarding for review.",
        fields: SELLER_EVENT_FIELDS,
    },
    EventSchema {
        event_type: "marketplace.seller.onboarding_approved",
        version: 1,
        description: "Marketplace seller onboarding was approved.",
        fields: SELLER_EVENT_FIELDS,
    },
    EventSchema {
        event_type: "marketplace.seller.onboarding_rejected",
        version: 1,
        description: "Marketplace seller onboarding was rejected.",
        fields: SELLER_EVENT_FIELDS,
    },
    EventSchema {
        event_type: "marketplace.seller.suspended",
        version: 1,
        description: "A marketplace seller was suspended.",
        fields: SELLER_EVENT_FIELDS,
    },
    EventSchema {
        event_type: "marketplace.seller.reactivated",
        version: 1,
        description: "A marketplace seller was reactivated.",
        fields: SELLER_EVENT_FIELDS,
    },
    EventSchema {
        event_type: "marketplace.seller.member_added",
        version: 1,
        description: "A member was added to a marketplace seller.",
        fields: SELLER_MEMBER_EVENT_FIELDS,
    },
    EventSchema {
        event_type: "marketplace.seller.member_updated",
        version: 1,
        description: "A marketplace seller member was updated.",
        fields: SELLER_MEMBER_EVENT_FIELDS,
    },
];

impl sealed::Sealed for MarketplaceSellerEvent {}

impl EventContract for MarketplaceSellerEvent {
    fn event_type(&self) -> &'static str {
        MarketplaceSellerEvent::event_type(self)
    }

    fn schema_version(&self) -> u16 {
        MarketplaceSellerEvent::schema_version(self)
    }

    fn into_contract_payload(self) -> ContractEventPayload {
        ContractEventPayload::MarketplaceSeller(self)
    }
}

impl ValidateEvent for MarketplaceSellerEvent {
    fn validate(&self) -> Result<(), EventValidationError> {
        validators::validate_not_nil_uuid("seller_id", self.seller_id())?;
        match self {
            Self::MarketplaceSellerMemberAdded {
                member_id,
                user_id,
                role,
                status,
                ..
            }
            | Self::MarketplaceSellerMemberUpdated {
                member_id,
                user_id,
                role,
                status,
                ..
            } => {
                validators::validate_not_nil_uuid("member_id", member_id)?;
                validators::validate_not_nil_uuid("user_id", user_id)?;
                validate_token("role", role)?;
                validate_token("status", status)?;
            }
            _ => {}
        }
        Ok(())
    }
}

fn validate_token(field_name: &'static str, value: &str) -> Result<(), EventValidationError> {
    validators::validate_not_empty(field_name, value)?;
    validators::validate_max_length(field_name, value, 40)?;
    if !value
        .bytes()
        .all(|byte| matches!(byte, b'a'..=b'z' | b'0'..=b'9' | b'_' | b'-'))
    {
        return Err(EventValidationError::InvalidValue(
            field_name,
            "must contain lowercase ASCII letters, digits, underscores, or hyphens".to_string(),
        ));
    }
    Ok(())
}

pub fn marketplace_seller_event_schema(event_type: &str) -> Option<&'static EventSchema> {
    MARKETPLACE_SELLER_EVENT_SCHEMAS
        .iter()
        .find(|schema| schema.event_type == event_type)
}
