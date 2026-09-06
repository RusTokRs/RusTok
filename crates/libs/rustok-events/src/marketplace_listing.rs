use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::contract::{ContractEventPayload, EventContract, sealed};
use crate::validation::{EventValidationError, ValidateEvent, validators};
use crate::{EventSchema, FieldSchema};

type ListingContractFields<'a> = (
    &'a Uuid,
    &'a Uuid,
    &'a Uuid,
    &'a Uuid,
    &'a str,
    &'a str,
    &'a i32,
);

macro_rules! marketplace_listing_event_contract {
    ($($variant:ident => $event_type:literal, $description:literal;)+) => {
        #[derive(Clone, Debug, Serialize, Deserialize, PartialEq, JsonSchema)]
        #[serde(tag = "type", content = "data")]
        pub enum MarketplaceListingEvent {
            $(
                $variant {
                    listing_id: Uuid,
                    seller_id: Uuid,
                    master_product_id: Uuid,
                    master_variant_id: Uuid,
                    market_slug: String,
                    channel_slug: String,
                    terms_version: i32,
                },
            )+
        }

        impl MarketplaceListingEvent {
            pub fn event_type(&self) -> &'static str {
                match self {
                    $(Self::$variant { .. } => $event_type,)+
                }
            }

            pub fn schema_version(&self) -> u16 {
                match self {
                    $(Self::$variant { .. } => 1,)+
                }
            }

            fn contract_fields(&self) -> ListingContractFields<'_> {
                match self {
                    $(
                        Self::$variant {
                            listing_id,
                            seller_id,
                            master_product_id,
                            master_variant_id,
                            market_slug,
                            channel_slug,
                            terms_version,
                        } => (
                            listing_id,
                            seller_id,
                            master_product_id,
                            master_variant_id,
                            market_slug,
                            channel_slug,
                            terms_version,
                        ),
                    )+
                }
            }
        }

        pub const MARKETPLACE_LISTING_EVENT_SCHEMAS: &[EventSchema] = &[
            $(EventSchema {
                event_type: $event_type,
                version: 1,
                description: $description,
                fields: MARKETPLACE_LISTING_EVENT_FIELDS,
            },)+
        ];
    };
}

marketplace_listing_event_contract! {
    MarketplaceListingCreated => "marketplace.listing.created", "A seller marketplace listing was created.";
    MarketplaceListingTermsUpdated => "marketplace.listing.terms_updated", "Marketplace listing commercial terms were versioned.";
    MarketplaceListingSubmittedForReview => "marketplace.listing.submitted_for_review", "A marketplace listing was submitted for review.";
    MarketplaceListingApproved => "marketplace.listing.approved", "A marketplace listing was approved.";
    MarketplaceListingRejected => "marketplace.listing.rejected", "A marketplace listing was rejected.";
    MarketplaceListingPublished => "marketplace.listing.published", "A marketplace listing was published.";
    MarketplaceListingSuspended => "marketplace.listing.suspended", "A marketplace listing was suspended.";
    MarketplaceListingReactivated => "marketplace.listing.reactivated", "A marketplace listing was reactivated.";
    MarketplaceListingArchived => "marketplace.listing.archived", "A marketplace listing was archived.";
}

const MARKETPLACE_LISTING_EVENT_FIELDS: &[FieldSchema] = &[
    FieldSchema {
        name: "listing_id",
        data_type: "uuid",
        optional: false,
    },
    FieldSchema {
        name: "seller_id",
        data_type: "uuid",
        optional: false,
    },
    FieldSchema {
        name: "master_product_id",
        data_type: "uuid",
        optional: false,
    },
    FieldSchema {
        name: "master_variant_id",
        data_type: "uuid",
        optional: false,
    },
    FieldSchema {
        name: "market_slug",
        data_type: "string",
        optional: false,
    },
    FieldSchema {
        name: "channel_slug",
        data_type: "string",
        optional: false,
    },
    FieldSchema {
        name: "terms_version",
        data_type: "int32",
        optional: false,
    },
];

impl sealed::Sealed for MarketplaceListingEvent {}

impl EventContract for MarketplaceListingEvent {
    fn event_type(&self) -> &'static str {
        MarketplaceListingEvent::event_type(self)
    }

    fn schema_version(&self) -> u16 {
        MarketplaceListingEvent::schema_version(self)
    }

    fn into_contract_payload(self) -> ContractEventPayload {
        ContractEventPayload::MarketplaceListing(self)
    }
}

impl ValidateEvent for MarketplaceListingEvent {
    fn validate(&self) -> Result<(), EventValidationError> {
        let (
            listing_id,
            seller_id,
            master_product_id,
            master_variant_id,
            market_slug,
            channel_slug,
            terms_version,
        ) = self.contract_fields();

        validators::validate_not_nil_uuid("listing_id", listing_id)?;
        validators::validate_not_nil_uuid("seller_id", seller_id)?;
        validators::validate_not_nil_uuid("master_product_id", master_product_id)?;
        validators::validate_not_nil_uuid("master_variant_id", master_variant_id)?;
        validate_scope_slug("market_slug", market_slug)?;
        validate_scope_slug("channel_slug", channel_slug)?;
        validators::validate_range("terms_version", i64::from(*terms_version), 1, i64::MAX)?;
        Ok(())
    }
}

fn validate_scope_slug(field_name: &'static str, value: &str) -> Result<(), EventValidationError> {
    validators::validate_not_empty(field_name, value)?;
    validators::validate_max_length(field_name, value, 80)?;
    let valid = value.bytes().enumerate().all(|(index, byte)| match byte {
        b'a'..=b'z' | b'0'..=b'9' => true,
        b'-' => index > 0 && index + 1 < value.len(),
        _ => false,
    });
    if !valid {
        return Err(EventValidationError::InvalidValue(
            field_name,
            "must contain lowercase ASCII letters, digits, or internal hyphens".to_string(),
        ));
    }
    Ok(())
}

pub fn marketplace_listing_event_schema(event_type: &str) -> Option<&'static EventSchema> {
    MARKETPLACE_LISTING_EVENT_SCHEMAS
        .iter()
        .find(|schema| schema.event_type == event_type)
}
