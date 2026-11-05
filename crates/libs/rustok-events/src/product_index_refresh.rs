use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::contract::{ContractEventPayload, EventContract, sealed};
use crate::validation::{EventValidationError, ValidateEvent, validators};
use crate::{EventSchema, FieldSchema};

pub const PRODUCT_INDEX_LOCALE_REFRESH_REQUESTED_EVENT_TYPE: &str =
    "product.index.locale_refresh_requested";
pub const PRODUCT_INDEX_VARIANT_REFRESH_REQUESTED_EVENT_TYPE: &str =
    "product.index.variant_refresh_requested";
pub const PRODUCT_INDEX_REFRESH_EVENT_SCHEMA_VERSION: u16 = 1;
pub const MAX_PRODUCT_INDEX_REFRESH_LOCALE_BYTES: usize = 128;

pub const PRODUCT_INDEX_REFRESH_EVENT_SCHEMAS: &[EventSchema] = &[
    EventSchema {
        event_type: PRODUCT_INDEX_LOCALE_REFRESH_REQUESTED_EVENT_TYPE,
        version: PRODUCT_INDEX_REFRESH_EVENT_SCHEMA_VERSION,
        description: "A Product-owned request to refresh one exact localized Product Index identity from canonical source state.",
        fields: PRODUCT_INDEX_LOCALE_REFRESH_REQUESTED_FIELDS,
    },
    EventSchema {
        event_type: PRODUCT_INDEX_VARIANT_REFRESH_REQUESTED_EVENT_TYPE,
        version: PRODUCT_INDEX_REFRESH_EVENT_SCHEMA_VERSION,
        description: "A Product-owned request to refresh one exact ProductVariant Index identity from canonical source state.",
        fields: PRODUCT_INDEX_VARIANT_REFRESH_REQUESTED_FIELDS,
    },
];

const PRODUCT_INDEX_LOCALE_REFRESH_REQUESTED_FIELDS: &[FieldSchema] = &[
    FieldSchema {
        name: "product_id",
        data_type: "uuid",
        optional: false,
    },
    FieldSchema {
        name: "locale",
        data_type: "string",
        optional: false,
    },
    FieldSchema {
        name: "source_version",
        data_type: "uint64",
        optional: false,
    },
];

const PRODUCT_INDEX_VARIANT_REFRESH_REQUESTED_FIELDS: &[FieldSchema] = &[
    FieldSchema {
        name: "product_id",
        data_type: "uuid",
        optional: false,
    },
    FieldSchema {
        name: "variant_id",
        data_type: "uuid",
        optional: false,
    },
    FieldSchema {
        name: "source_version",
        data_type: "uint64",
        optional: false,
    },
];

/// Sealed Product-owned wire family for exact source-refresh instructions consumed by Index.
///
/// Tenant, actor, refresh identity and root-event causation are intentionally envelope metadata.
/// The payload carries only the immutable target facts that the Product canonical writer compares
/// with its append-only refresh ledger before publication.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq, JsonSchema)]
#[serde(tag = "type", content = "data")]
pub enum ProductIndexRefreshEvent {
    LocaleRefreshRequested {
        product_id: Uuid,
        locale: String,
        source_version: u64,
    },
    VariantRefreshRequested {
        product_id: Uuid,
        variant_id: Uuid,
        source_version: u64,
    },
}

impl ProductIndexRefreshEvent {
    pub fn event_type(&self) -> &'static str {
        match self {
            Self::LocaleRefreshRequested { .. } => {
                PRODUCT_INDEX_LOCALE_REFRESH_REQUESTED_EVENT_TYPE
            }
            Self::VariantRefreshRequested { .. } => {
                PRODUCT_INDEX_VARIANT_REFRESH_REQUESTED_EVENT_TYPE
            }
        }
    }

    pub const fn schema_version(&self) -> u16 {
        PRODUCT_INDEX_REFRESH_EVENT_SCHEMA_VERSION
    }
}

impl sealed::Sealed for ProductIndexRefreshEvent {}

impl EventContract for ProductIndexRefreshEvent {
    fn event_type(&self) -> &'static str {
        ProductIndexRefreshEvent::event_type(self)
    }

    fn schema_version(&self) -> u16 {
        ProductIndexRefreshEvent::schema_version(self)
    }

    fn into_contract_payload(self) -> ContractEventPayload {
        ContractEventPayload::ProductIndexRefresh(self)
    }
}

impl ValidateEvent for ProductIndexRefreshEvent {
    fn validate(&self) -> Result<(), EventValidationError> {
        match self {
            Self::LocaleRefreshRequested {
                product_id,
                locale,
                source_version,
            } => {
                validators::validate_not_nil_uuid("product_id", product_id)?;
                validators::validate_not_empty("locale", locale)?;
                validators::validate_max_length(
                    "locale",
                    locale,
                    MAX_PRODUCT_INDEX_REFRESH_LOCALE_BYTES,
                )?;
                if locale.trim() != locale {
                    return Err(EventValidationError::InvalidValue(
                        "locale",
                        "must not contain leading or trailing whitespace".to_owned(),
                    ));
                }
                validate_source_version(*source_version)
            }
            Self::VariantRefreshRequested {
                product_id,
                variant_id,
                source_version,
            } => {
                validators::validate_not_nil_uuid("product_id", product_id)?;
                validators::validate_not_nil_uuid("variant_id", variant_id)?;
                validate_source_version(*source_version)
            }
        }
    }
}

fn validate_source_version(source_version: u64) -> Result<(), EventValidationError> {
    if source_version == 0 || source_version > i64::MAX as u64 {
        return Err(EventValidationError::InvalidValue(
            "source_version",
            "must fit the positive Product owner revision range".to_owned(),
        ));
    }
    Ok(())
}

pub fn product_index_refresh_event_schema(event_type: &str) -> Option<&'static EventSchema> {
    PRODUCT_INDEX_REFRESH_EVENT_SCHEMAS
        .iter()
        .find(|schema| schema.event_type == event_type)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ContractEventEnvelope;

    #[test]
    fn locale_refresh_round_trips_through_registered_contract_envelope() {
        let event = ProductIndexRefreshEvent::LocaleRefreshRequested {
            product_id: Uuid::new_v4(),
            locale: "en-US".to_owned(),
            source_version: 7,
        };
        let envelope = ContractEventEnvelope::new_with_envelope_id_and_causation(
            Uuid::new_v4(),
            Uuid::new_v4(),
            Some(Uuid::new_v4()),
            Uuid::new_v4(),
            event,
        )
        .expect("Product locale refresh envelope should validate");

        envelope
            .validate_registered_schema()
            .expect("Product locale refresh envelope should remain registered");
    }

    #[test]
    fn variant_refresh_rejects_zero_source_version() {
        let event = ProductIndexRefreshEvent::VariantRefreshRequested {
            product_id: Uuid::new_v4(),
            variant_id: Uuid::new_v4(),
            source_version: 0,
        };

        assert!(event.validate().is_err());
    }

    #[test]
    fn locale_refresh_rejects_noncanonical_outer_whitespace() {
        let event = ProductIndexRefreshEvent::LocaleRefreshRequested {
            product_id: Uuid::new_v4(),
            locale: " en-US ".to_owned(),
            source_version: 1,
        };

        assert!(event.validate().is_err());
    }
}
