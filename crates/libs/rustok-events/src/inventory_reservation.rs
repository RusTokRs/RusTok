//! Quantity-free Inventory reservation invalidation facts.
//!
//! Availability may legitimately be negative under `continue`. Consumers refetch current
//! owner state; this fact is neither a counter snapshot nor a replay/stock guarantee.

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::contract::{sealed, ContractEventPayload, EventContract};
use crate::validation::{validators, EventValidationError, ValidateEvent};
use crate::{EventSchema, FieldSchema};

pub const INVENTORY_RESERVATION_CHANGED_EVENT_TYPE: &str = "inventory.reservation.changed";
pub const INVENTORY_RESERVATION_EVENT_SCHEMA_VERSION: u16 = 1;

pub const INVENTORY_RESERVATION_EVENT_SCHEMAS: &[EventSchema] = &[EventSchema {
    event_type: INVENTORY_RESERVATION_CHANGED_EVENT_TYPE,
    version: INVENTORY_RESERVATION_EVENT_SCHEMA_VERSION,
    description: "Inventory committed a reservation counter change; refetch the current Product/Variant owner state without interpreting this as a quantity, receipt or lease.",
    fields: &[
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
    ],
}];

/// A newly committed Inventory reservation counter change, not Product ledger evidence.
/// Tenant and the current human actor belong to the canonical envelope, never receipt metadata.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq, JsonSchema)]
#[serde(tag = "type", content = "data", deny_unknown_fields)]
pub enum InventoryReservationEvent {
    Changed { product_id: Uuid, variant_id: Uuid },
}

impl InventoryReservationEvent {
    pub const fn event_type(&self) -> &'static str {
        INVENTORY_RESERVATION_CHANGED_EVENT_TYPE
    }

    pub const fn schema_version(&self) -> u16 {
        INVENTORY_RESERVATION_EVENT_SCHEMA_VERSION
    }
}

impl sealed::Sealed for InventoryReservationEvent {}

impl EventContract for InventoryReservationEvent {
    fn event_type(&self) -> &'static str {
        self.event_type()
    }

    fn schema_version(&self) -> u16 {
        self.schema_version()
    }

    fn into_contract_payload(self) -> ContractEventPayload {
        ContractEventPayload::InventoryReservation(self)
    }
}

impl ValidateEvent for InventoryReservationEvent {
    fn validate(&self) -> Result<(), EventValidationError> {
        match self {
            Self::Changed {
                product_id,
                variant_id,
            } => {
                validators::validate_not_nil_uuid("product_id", product_id)?;
                validators::validate_not_nil_uuid("variant_id", variant_id)
            }
        }
    }
}

pub fn inventory_reservation_event_schema(event_type: &str) -> Option<&'static EventSchema> {
    INVENTORY_RESERVATION_EVENT_SCHEMAS
        .iter()
        .find(|schema| schema.event_type == event_type)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ContractEventEnvelope;

    fn event() -> InventoryReservationEvent {
        InventoryReservationEvent::Changed {
            product_id: Uuid::new_v4(),
            variant_id: Uuid::new_v4(),
        }
    }

    #[test]
    fn reservation_fact_round_trips_with_current_actor_and_no_quantities() {
        let tenant = Uuid::new_v4();
        let actor = Uuid::new_v4();
        let envelope = ContractEventEnvelope::new(tenant, Some(actor), event()).unwrap();
        let wire = serde_json::to_value(&envelope).unwrap();
        let decoded: ContractEventEnvelope = serde_json::from_value(wire.clone()).unwrap();
        decoded.validate_registered_schema().unwrap();
        assert_eq!(decoded, envelope);
        assert_eq!(decoded.tenant_id(), tenant);
        assert_eq!(decoded.actor_id(), Some(actor));
        let fields = wire["event"]["event"]["data"].as_object().unwrap();
        assert_eq!(fields.len(), 2);
        assert!(fields.contains_key("product_id") && fields.contains_key("variant_id"));
    }

    #[test]
    fn reservation_fact_rejects_nil_subjects_and_platform_tenant() {
        for event in [
            InventoryReservationEvent::Changed {
                product_id: Uuid::nil(),
                variant_id: Uuid::new_v4(),
            },
            InventoryReservationEvent::Changed {
                product_id: Uuid::new_v4(),
                variant_id: Uuid::nil(),
            },
        ] {
            assert!(event.validate().is_err());
        }
        assert!(ContractEventEnvelope::new(Uuid::nil(), None, event()).is_err());
        assert!(ContractEventEnvelope::new(Uuid::new_v4(), Some(Uuid::nil()), event()).is_err());
    }

    #[test]
    fn reservation_fact_does_not_enter_the_root_family() {
        let envelope = ContractEventEnvelope::new(Uuid::new_v4(), None, event()).unwrap();
        assert!(matches!(
            envelope.payload().unwrap(),
            ContractEventPayload::InventoryReservation(_)
        ));
        assert!(envelope.into_root_envelope().is_err());
        assert_eq!(
            crate::event_schemas()
                .filter(|s| s.event_type == INVENTORY_RESERVATION_CHANGED_EVENT_TYPE)
                .count(),
            1
        );
    }

    #[test]
    fn reservation_fact_rejects_fabricated_quantity_and_header_version() {
        let envelope = ContractEventEnvelope::new(Uuid::new_v4(), None, event()).unwrap();
        let mut wire = serde_json::to_value(&envelope).unwrap();
        wire["event"]["event"]["data"]["quantity"] = serde_json::json!(-1);
        assert!(serde_json::from_value::<ContractEventEnvelope>(wire).is_err());
        let mut wire = serde_json::to_value(envelope).unwrap();
        wire["schema_version"] = serde_json::json!(2);
        let decoded: ContractEventEnvelope = serde_json::from_value(wire).unwrap();
        assert!(decoded.validate_registered_schema().is_err());
    }
}
