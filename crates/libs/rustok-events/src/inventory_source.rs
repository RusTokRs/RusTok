//! Inventory-owned source revision facts, separate from quantity/reservation notifications.
//! Consumers refetch authoritative owner state. No copied stock, Product revision or absence claim.
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::contract::{ContractEventPayload, EventContract, sealed};
use crate::validation::{EventValidationError, ValidateEvent, validators};
use crate::{EventSchema, FieldSchema};

pub const INVENTORY_SOURCE_CHANGED_EVENT_TYPE: &str = "inventory.source.changed";
pub const INVENTORY_SOURCE_BASELINE_ACTIVATED_EVENT_TYPE: &str =
    "inventory.source.baseline_activated";
pub const INVENTORY_SOURCE_EVENT_SCHEMA_VERSION: u16 = 1;

pub const INVENTORY_SOURCE_EVENT_SCHEMAS: &[EventSchema] = &[
    EventSchema {
        event_type: INVENTORY_SOURCE_CHANGED_EVENT_TYPE,
        version: INVENTORY_SOURCE_EVENT_SCHEMA_VERSION,
        description: "An Inventory source revision committed on the original Variant identity; load owner live state or its retained tombstone at least at this version.",
        fields: &[
            FieldSchema {
                name: "baseline_id",
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
        ],
    },
    EventSchema {
        event_type: INVENTORY_SOURCE_BASELINE_ACTIVATED_EVENT_TYPE,
        version: INVENTORY_SOURCE_EVENT_SCHEMA_VERSION,
        description: "The explicit Inventory current-state baseline became active; the selected consumer must durably arrange its initial bounded source replay before acknowledgement.",
        fields: &[FieldSchema {
            name: "baseline_id",
            data_type: "uuid",
            optional: false,
        }],
    },
];

/// Tenant/current actor and retained change/activation UUID belong to the sealed envelope.
/// Changed never prescribes upsert/delete: authoritative owner loading decides that outcome.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq, JsonSchema)]
#[serde(tag = "type", content = "data", deny_unknown_fields)]
pub enum InventorySourceEvent {
    Changed {
        baseline_id: Uuid,
        variant_id: Uuid,
        source_version: u64,
    },
    BaselineActivated {
        baseline_id: Uuid,
    },
}

impl InventorySourceEvent {
    pub const fn event_type(&self) -> &'static str {
        match self {
            Self::Changed { .. } => INVENTORY_SOURCE_CHANGED_EVENT_TYPE,
            Self::BaselineActivated { .. } => INVENTORY_SOURCE_BASELINE_ACTIVATED_EVENT_TYPE,
        }
    }

    pub const fn schema_version(&self) -> u16 {
        INVENTORY_SOURCE_EVENT_SCHEMA_VERSION
    }
}

impl sealed::Sealed for InventorySourceEvent {}
impl EventContract for InventorySourceEvent {
    fn event_type(&self) -> &'static str {
        self.event_type()
    }
    fn schema_version(&self) -> u16 {
        self.schema_version()
    }
    fn into_contract_payload(self) -> ContractEventPayload {
        ContractEventPayload::InventorySource(self)
    }
}
impl ValidateEvent for InventorySourceEvent {
    fn validate(&self) -> Result<(), EventValidationError> {
        let baseline_id = match self {
            Self::Changed {
                baseline_id,
                variant_id,
                source_version,
            } => {
                validators::validate_not_nil_uuid("variant_id", variant_id)?;
                if *source_version == 0 || *source_version > i64::MAX as u64 {
                    return Err(EventValidationError::InvalidValue(
                        "source_version",
                        "must fit the positive Inventory owner revision range".to_owned(),
                    ));
                }
                baseline_id
            }
            Self::BaselineActivated { baseline_id } => baseline_id,
        };
        validators::validate_not_nil_uuid("baseline_id", baseline_id)
    }
}

pub fn inventory_source_event_schema(event_type: &str) -> Option<&'static EventSchema> {
    INVENTORY_SOURCE_EVENT_SCHEMAS
        .iter()
        .find(|schema| schema.event_type == event_type)
}

#[cfg(test)]
mod tests;
