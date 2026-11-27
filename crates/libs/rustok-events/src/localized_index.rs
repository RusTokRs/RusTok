//! Durable invalidation fact for localized records consumed by the relational Index.

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::contract::{ContractEventPayload, EventContract, sealed};
use crate::{
    EventSchema, EventValidationError, FieldSchema, ValidateEvent, validation::validators,
};

pub const LOCALIZED_INDEX_REFRESH_EVENT_TYPE: &str = "index.localized_record.refresh_requested";
const SCHEMA_VERSION: u16 = 1;

pub const LOCALIZED_INDEX_EVENT_SCHEMAS: &[EventSchema] = &[EventSchema {
    event_type: LOCALIZED_INDEX_REFRESH_EVENT_TYPE,
    version: SCHEMA_VERSION,
    description: "Refresh an exact tenant record from its localized owner at or above the committed revision.",
    fields: &[
        FieldSchema {
            name: "module_slug",
            data_type: "string",
            optional: false,
        },
        FieldSchema {
            name: "entity_type",
            data_type: "string",
            optional: false,
        },
        FieldSchema {
            name: "record_id",
            data_type: "uuid",
            optional: false,
        },
        FieldSchema {
            name: "source_version",
            data_type: "uint64",
            optional: false,
        },
    ],
}];

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct LocalizedIndexRefreshEvent {
    pub module_slug: String,
    pub entity_type: String,
    pub record_id: Uuid,
    pub source_version: u64,
}

impl sealed::Sealed for LocalizedIndexRefreshEvent {}

impl EventContract for LocalizedIndexRefreshEvent {
    fn event_type(&self) -> &'static str {
        LOCALIZED_INDEX_REFRESH_EVENT_TYPE
    }

    fn schema_version(&self) -> u16 {
        SCHEMA_VERSION
    }

    fn into_contract_payload(self) -> ContractEventPayload {
        ContractEventPayload::LocalizedIndexRefresh(self)
    }
}

impl ValidateEvent for LocalizedIndexRefreshEvent {
    fn validate(&self) -> Result<(), EventValidationError> {
        for (name, value) in [
            ("module_slug", &self.module_slug),
            ("entity_type", &self.entity_type),
        ] {
            if !rustok_api::is_valid_module_slug(value) {
                return Err(EventValidationError::InvalidValue(
                    name,
                    "expected a platform identifier".into(),
                ));
            }
        }
        validators::validate_not_nil_uuid("record_id", &self.record_id)?;
        if !(1..=i64::MAX as u64).contains(&self.source_version) {
            return Err(EventValidationError::InvalidValue(
                "source_version",
                "expected a positive owner revision".into(),
            ));
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ContractEventEnvelope;

    #[test]
    fn refresh_is_acknowledged_by_the_matching_module_listener() {
        let event = LocalizedIndexRefreshEvent {
            module_slug: "test_records".into(),
            entity_type: "record".into(),
            record_id: Uuid::new_v4(),
            source_version: i64::MAX as u64,
        };
        let envelope = ContractEventEnvelope::new(Uuid::new_v4(), None, event.clone()).unwrap();
        let round_trip: ContractEventEnvelope =
            serde_json::from_value(serde_json::to_value(envelope).unwrap()).unwrap();
        let payload = round_trip.payload().unwrap();
        assert_eq!(payload, &ContractEventPayload::LocalizedIndexRefresh(event));
        assert!(payload.requires_module_delivery());
    }

    #[test]
    fn refresh_rejects_out_of_scope_identifiers_and_revisions() {
        let valid = LocalizedIndexRefreshEvent {
            module_slug: "test_records".into(),
            entity_type: "record".into(),
            record_id: Uuid::new_v4(),
            source_version: 1,
        };
        for invalid in [
            LocalizedIndexRefreshEvent {
                module_slug: "Test Records".into(),
                ..valid.clone()
            },
            LocalizedIndexRefreshEvent {
                entity_type: "../record".into(),
                ..valid.clone()
            },
            LocalizedIndexRefreshEvent {
                record_id: Uuid::nil(),
                ..valid.clone()
            },
            LocalizedIndexRefreshEvent {
                source_version: 0,
                ..valid.clone()
            },
            LocalizedIndexRefreshEvent {
                source_version: i64::MAX as u64 + 1,
                ..valid
            },
        ] {
            assert!(ContractEventEnvelope::new(Uuid::new_v4(), None, invalid).is_err());
        }
    }
}
