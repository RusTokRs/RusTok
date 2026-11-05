//! Definitions only. Canonical digest generation, Rust compilation and tests remain unrun.
use super::*;
use crate::{ContractEventEnvelope, InventoryReservationEvent, ProductIndexRefreshEvent};

fn changed(source_version: u64) -> InventorySourceEvent {
    InventorySourceEvent::Changed {
        baseline_id: Uuid::new_v4(),
        variant_id: Uuid::new_v4(),
        source_version,
    }
}

#[test]
fn inventory_source_facts_are_registered_with_distinct_sealed_v1_routes() {
    for event in [
        changed(1),
        InventorySourceEvent::BaselineActivated {
            baseline_id: Uuid::new_v4(),
        },
    ] {
        let envelope = ContractEventEnvelope::new(Uuid::new_v4(), None, event.clone()).unwrap();
        envelope.validate_registered_schema().unwrap();
        assert_eq!(crate::event_schema(event.event_type()).unwrap().version, 1);
        assert_eq!(
            crate::event_schemas()
                .filter(|schema| schema.event_type == event.event_type())
                .count(),
            1
        );
        let wire = serde_json::to_value(&envelope).unwrap();
        assert_eq!(wire["event"]["family"], "inventory_source");
        assert_eq!(
            serde_json::from_value::<ContractEventEnvelope>(wire).unwrap(),
            envelope
        );
        assert_eq!(
            envelope.payload().unwrap(),
            &ContractEventPayload::InventorySource(event)
        );
    }
}

#[test]
fn inventory_source_changed_revision_is_positive_and_fits_its_owner_storage() {
    for version in [0, i64::MAX as u64 + 1, u64::MAX] {
        assert!(changed(version).validate().is_err());
    }
    for version in [1, i64::MAX as u64] {
        changed(version).validate().unwrap();
    }
}

#[test]
fn inventory_source_facts_refuse_missing_original_baseline_or_variant_identity() {
    for event in [
        InventorySourceEvent::Changed {
            baseline_id: Uuid::nil(),
            variant_id: Uuid::new_v4(),
            source_version: 1,
        },
        InventorySourceEvent::Changed {
            baseline_id: Uuid::new_v4(),
            variant_id: Uuid::nil(),
            source_version: 1,
        },
        InventorySourceEvent::BaselineActivated {
            baseline_id: Uuid::nil(),
        },
    ] {
        assert!(ContractEventEnvelope::new(Uuid::new_v4(), None, event).is_err());
    }
}

#[test]
fn inventory_source_data_does_not_admit_quantity_product_or_authority_snapshots() {
    for field in [
        "quantity",
        "deleted",
        "product_id",
        "actor_id",
        "tenant_id",
        "metadata",
    ] {
        let mut wire = serde_json::to_value(changed(1)).unwrap();
        assert_eq!(wire["data"].as_object().unwrap().len(), 3);
        wire["data"][field] = serde_json::json!(1);
        assert!(serde_json::from_value::<InventorySourceEvent>(wire).is_err());
    }
    let wire = serde_json::to_value(InventorySourceEvent::BaselineActivated {
        baseline_id: Uuid::new_v4(),
    })
    .unwrap();
    assert_eq!(wire["data"].as_object().unwrap().len(), 1);
}

#[test]
fn inventory_source_envelope_uses_the_owner_identity_without_invented_product_causation() {
    let change_id = Uuid::new_v4();
    let actor = Uuid::new_v4();
    let tenant = Uuid::new_v4();
    let envelope =
        ContractEventEnvelope::new_with_envelope_id(change_id, tenant, Some(actor), changed(2))
            .unwrap();
    assert_eq!(envelope.id(), change_id);
    assert_eq!(envelope.correlation_id(), change_id);
    assert_eq!(envelope.tenant_id(), tenant);
    assert_eq!(envelope.actor_id(), Some(actor));
    assert_eq!(envelope.causation_id(), None);
    let mut wire = serde_json::to_value(envelope).unwrap();
    wire["schema_version"] = serde_json::json!(2);
    assert!(
        serde_json::from_value::<ContractEventEnvelope>(wire)
            .unwrap()
            .validate_registered_schema()
            .is_err()
    );
}

#[test]
fn inventory_source_and_reservation_module_ack_policy_does_not_capture_remote_product_routes() {
    let source = changed(1).into_contract_payload();
    let activation = InventorySourceEvent::BaselineActivated {
        baseline_id: Uuid::new_v4(),
    }
    .into_contract_payload();
    let reservation = InventoryReservationEvent::Changed {
        product_id: Uuid::new_v4(),
        variant_id: Uuid::new_v4(),
    }
    .into_contract_payload();
    let product = ProductIndexRefreshEvent::VariantRefreshRequested {
        product_id: Uuid::new_v4(),
        variant_id: Uuid::new_v4(),
        source_version: 1,
    }
    .into_contract_payload();
    assert!(source.requires_module_delivery());
    assert!(activation.requires_module_delivery());
    assert!(reservation.requires_module_delivery());
    assert!(!product.requires_module_delivery());
}
