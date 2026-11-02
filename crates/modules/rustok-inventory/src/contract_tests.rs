use rustok_core::module::{ModuleKind, RusToKModule};

use crate::InventoryModule;

#[test]
fn crate_api_defines_minimal_contract_sections() {
    let api = include_str!("../CRATE_API.md");
    for marker in [
        "## Public Modules",
        "## Platform Capability Bindings",
        "## Domain Collaborations",
        "## Minimum Contract Set",
        "### Input DTOs/Commands",
        "### Domain Invariants",
        "### Events / Outbox Side Effects",
        "### Errors / Failure Codes",
    ] {
        assert!(
            api.contains(marker),
            "CRATE_API.md must contain section: {marker}"
        );
    }
}

#[test]
fn module_contract_metadata_matches() {
    let module = InventoryModule::new();
    assert_eq!(module.slug(), "inventory");
    assert_eq!(module.name(), "Inventory");
    assert_eq!(module.kind(), ModuleKind::Optional);
    assert_eq!(module.dependencies(), &["product"]);
}

#[test]
fn public_port_paths_use_the_same_admitted_owner_boundaries() {
    use std::any::TypeId;

    assert_eq!(
        TypeId::of::<crate::ports::PersistentInventoryReservationIdentityPort>(),
        TypeId::of::<crate::PersistentInventoryReservationIdentityPort>(),
    );
    assert_eq!(
        TypeId::of::<crate::ports::PersistentInventoryReservationIdentityPort>(),
        TypeId::of::<crate::reservation_owner_context::PersistentInventoryReservationIdentityPort>(
        ),
    );
    assert_eq!(
        TypeId::of::<crate::ports::InProcessInventoryReservationPort>(),
        TypeId::of::<crate::reservation_port_context::InProcessInventoryReservationPort>(),
    );
}

#[tokio::test]
async fn identity_adoption_public_boundary_rejects_bad_admission_before_persistence() {
    use crate::{
        InventoryIdentityReservationAdoptionRequest, in_process_inventory_reservation_identity_port,
    };
    use rustok_api::{PortActor, PortContext};
    use std::time::Duration;
    let ports: [std::sync::Arc<dyn crate::InventoryReservationIdentityPort>; 4] = [
        in_process_inventory_reservation_identity_port(sea_orm::DatabaseConnection::Disconnected),
        crate::ports::in_process_inventory_reservation_identity_port(
            sea_orm::DatabaseConnection::Disconnected,
        ),
        std::sync::Arc::new(crate::PersistentInventoryReservationIdentityPort::new(
            sea_orm::DatabaseConnection::Disconnected,
        )),
        std::sync::Arc::new(
            crate::ports::PersistentInventoryReservationIdentityPort::new(
                sea_orm::DatabaseConnection::Disconnected,
            ),
        ),
    ];
    let request = InventoryIdentityReservationAdoptionRequest {
        checkout_operation_id: uuid::Uuid::new_v4(),
        cart_id: uuid::Uuid::new_v4(),
        order_id: uuid::Uuid::new_v4(),
        items: vec![],
    };
    let context = PortContext::new(
        uuid::Uuid::new_v4().to_string(),
        PortActor::system(),
        "en",
        "adoption-admission",
    );
    for port in ports {
        let error = port
            .adopt_inventory_by_identity(
                context.clone().with_deadline(Duration::from_secs(1)),
                request.clone(),
            )
            .await
            .unwrap_err();
        assert_eq!(error.code, "port.idempotency_key_required");
        for deadline_ms in [None, Some(0)] {
            let mut missing_deadline = context.clone().with_idempotency_key("same-adoption");
            missing_deadline.deadline_ms = deadline_ms;
            let error = port
                .adopt_inventory_by_identity(missing_deadline, request.clone())
                .await
                .unwrap_err();
            assert_eq!(error.code, "port.deadline_required");
            assert!(error.retryable);
        }
        let invalid_tenant = PortContext::new(
            "invalid-uuid",
            PortActor::system(),
            "en",
            "adoption-admission",
        )
        .with_idempotency_key("same-adoption")
        .with_deadline(Duration::from_secs(1));
        let error = port
            .adopt_inventory_by_identity(invalid_tenant, request.clone())
            .await
            .unwrap_err();
        assert_eq!(error.code, "inventory.context_invalid");
        // A valid public boundary reaches batch validation, still before the disconnected DB.
        let error = port
            .adopt_inventory_by_identity(
                context
                    .clone()
                    .with_idempotency_key("same-adoption")
                    .with_deadline(Duration::from_secs(1)),
                request.clone(),
            )
            .await
            .unwrap_err();
        assert_eq!(error.code, "inventory.adoption_batch_invalid");
    }
}
