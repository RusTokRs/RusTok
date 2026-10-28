use uuid::Uuid;

use super::{DomainEvent, EventEnvelope, EventEnvelopeError};
use crate::validation::{EventValidationError, ValidateEvent};

#[test]
fn target_deleted_event_is_registered_and_validates() {
    let envelope = EventEnvelope::new(
        Uuid::new_v4(),
        None,
        DomainEvent::TargetDeleted {
            target_type: "blog_post".to_string(),
            target_id: Uuid::new_v4(),
        },
    );
    assert_eq!(envelope.event_type, "target.deleted");
    assert_eq!(envelope.schema_version, 1);
    assert!(envelope.validate_registered_schema().is_ok());
}

#[test]
fn target_deleted_event_rejects_nil_target_id() {
    let event = DomainEvent::TargetDeleted {
        target_type: "blog_post".to_string(),
        target_id: Uuid::nil(),
    };
    assert!(event.validate().is_err());
}

#[test]
fn test_node_created_valid() {
    let event = DomainEvent::NodeCreated {
        node_id: Uuid::new_v4(),
        kind: "post".to_string(),
        author_id: Some(Uuid::new_v4()),
    };
    assert!(event.validate().is_ok());
}

#[test]
fn test_node_created_nil_id() {
    let event = DomainEvent::NodeCreated {
        node_id: Uuid::nil(),
        kind: "post".to_string(),
        author_id: None,
    };
    assert!(event.validate().is_err());
}

#[test]
fn test_node_created_empty_kind() {
    let event = DomainEvent::NodeCreated {
        node_id: Uuid::new_v4(),
        kind: "".to_string(),
        author_id: None,
    };
    assert!(event.validate().is_err());
}

#[test]
fn test_node_created_invalid_kind_characters() {
    let event = DomainEvent::NodeCreated {
        node_id: Uuid::new_v4(),
        kind: "invalid@kind".to_string(),
        author_id: None,
    };
    assert!(event.validate().is_err());
}

#[test]
fn test_order_placed_valid() {
    let event = DomainEvent::OrderPlaced {
        order_id: Uuid::new_v4(),
        customer_id: Some(Uuid::new_v4()),
        total: 10000,
        currency: "USD".to_string(),
    };
    assert!(event.validate().is_ok());
}

#[test]
fn test_order_placed_negative_total() {
    let event = DomainEvent::OrderPlaced {
        order_id: Uuid::new_v4(),
        customer_id: None,
        total: -100,
        currency: "USD".to_string(),
    };
    assert!(event.validate().is_err());
}

#[test]
fn test_order_placed_invalid_currency() {
    let event = DomainEvent::OrderPlaced {
        order_id: Uuid::new_v4(),
        customer_id: None,
        total: 10000,
        currency: "US".to_string(), // too short
    };
    assert!(event.validate().is_err());
}

#[test]
fn user_account_registered_requires_only_a_non_nil_identity() {
    let event = DomainEvent::UserAccountRegistered {
        user_id: Uuid::new_v4(),
    };
    assert!(event.validate().is_ok());
}

#[test]
fn user_account_registered_rejects_a_nil_identity() {
    let event = DomainEvent::UserAccountRegistered {
        user_id: Uuid::nil(),
    };
    assert!(event.validate().is_err());
}

#[test]
fn user_account_registered_serialization_contains_no_contact_data() {
    let event = DomainEvent::UserAccountRegistered {
        user_id: Uuid::from_u128(42),
    };

    let serialized = serde_json::to_string(&event).expect("serialize event");
    assert_eq!(event.event_type(), "user.account_registered");
    assert_eq!(event.schema_version(), 1);
    assert!(!serialized.contains("email"));
    assert!(!serialized.contains('@'));
}

#[test]
fn test_inventory_updated_valid() {
    let event = DomainEvent::InventoryUpdated {
        variant_id: Uuid::new_v4(),
        product_id: Uuid::new_v4(),
        location_id: Uuid::new_v4(),
        old_quantity: 10,
        new_quantity: 5,
    };
    assert!(event.validate().is_ok());
}

#[test]
fn test_inventory_updated_negative_quantity() {
    let event = DomainEvent::InventoryUpdated {
        variant_id: Uuid::new_v4(),
        product_id: Uuid::new_v4(),
        location_id: Uuid::new_v4(),
        old_quantity: -5,
        new_quantity: 10,
    };
    assert!(event.validate().is_err());
}

#[test]
fn test_inventory_low_valid() {
    let event = DomainEvent::InventoryLow {
        variant_id: Uuid::new_v4(),
        product_id: Uuid::new_v4(),
        remaining: 5,
        threshold: 10,
    };
    assert!(event.validate().is_ok());
}

#[test]
fn test_inventory_low_invalid_remaining_above_threshold() {
    let event = DomainEvent::InventoryLow {
        variant_id: Uuid::new_v4(),
        product_id: Uuid::new_v4(),
        remaining: 15,
        threshold: 10,
    };
    assert!(event.validate().is_err());
}

#[test]
fn test_order_status_changed_valid() {
    let event = DomainEvent::OrderStatusChanged {
        order_id: Uuid::new_v4(),
        old_status: "pending".to_string(),
        new_status: "processing".to_string(),
    };
    assert!(event.validate().is_ok());
}

#[test]
fn test_order_status_changed_same_status() {
    let event = DomainEvent::OrderStatusChanged {
        order_id: Uuid::new_v4(),
        old_status: "pending".to_string(),
        new_status: "pending".to_string(),
    };
    assert!(event.validate().is_err());
}

#[test]
fn test_media_uploaded_valid() {
    let event = DomainEvent::MediaUploaded {
        media_id: Uuid::new_v4(),
        mime_type: "image/jpeg".to_string(),
        size: 102400,
    };
    assert!(event.validate().is_ok());
}

#[test]
fn test_media_uploaded_invalid_mime_type() {
    let event = DomainEvent::MediaUploaded {
        media_id: Uuid::new_v4(),
        mime_type: "invalid".to_string(), // no slash
        size: 102400,
    };
    assert!(event.validate().is_err());
}

#[test]
fn test_build_requested_valid_and_metadata() {
    let event = DomainEvent::BuildRequested {
        build_id: Uuid::new_v4(),
        requested_by: "admin@rustok.local".to_string(),
    };

    assert_eq!(event.event_type(), "build.requested");
    assert_eq!(event.schema_version(), 1);
    assert!(event.validate().is_ok());
}

#[test]
fn test_build_requested_invalid_requested_by() {
    let event = DomainEvent::BuildRequested {
        build_id: Uuid::new_v4(),
        requested_by: "".to_string(),
    };

    assert!(event.validate().is_err());
}

#[test]
fn test_build_rolled_back_valid_and_metadata() {
    let event = DomainEvent::BuildRolledBack {
        requested_build_id: Uuid::new_v4(),
        restored_build_id: Uuid::new_v4(),
        from_release_id: "release-current".to_string(),
        to_release_id: "release-previous".to_string(),
    };

    assert_eq!(event.event_type(), "build.rolled_back");
    assert_eq!(event.schema_version(), 1);
    assert!(event.validate().is_ok());
}

#[test]
fn test_build_rolled_back_rejects_same_release() {
    let event = DomainEvent::BuildRolledBack {
        requested_build_id: Uuid::new_v4(),
        restored_build_id: Uuid::new_v4(),
        from_release_id: "release-current".to_string(),
        to_release_id: "release-current".to_string(),
    };

    assert!(event.validate().is_err());
}

#[test]
fn test_flex_entry_created_valid_standalone_binding() {
    let event = DomainEvent::FlexEntryCreated {
        tenant_id: Uuid::new_v4(),
        schema_id: Uuid::new_v4(),
        entry_id: Uuid::new_v4(),
        entity_type: None,
        entity_id: None,
    };

    assert!(event.validate().is_ok());
}

#[test]
fn test_flex_entry_created_invalid_partial_binding() {
    let event = DomainEvent::FlexEntryCreated {
        tenant_id: Uuid::new_v4(),
        schema_id: Uuid::new_v4(),
        entry_id: Uuid::new_v4(),
        entity_type: Some("product".to_string()),
        entity_id: None,
    };

    assert!(event.validate().is_err());
}

#[test]
fn test_tenant_module_toggled_event_contract() {
    let event = DomainEvent::TenantModuleToggled {
        tenant_id: Uuid::new_v4(),
        module_slug: "blog".to_string(),
        enabled: true,
    };

    assert_eq!(event.event_type(), "tenant.module.toggled");
    assert_eq!(event.schema_version(), 1);
    assert!(event.validate().is_ok());
}

#[test]
fn test_tenant_module_toggled_rejects_empty_module_slug() {
    let event = DomainEvent::TenantModuleToggled {
        tenant_id: Uuid::new_v4(),
        module_slug: "".to_string(),
        enabled: true,
    };

    assert!(event.validate().is_err());
}

#[test]
fn tenant_locale_events_require_canonical_runtime_locale() {
    let tenant_id = Uuid::new_v4();
    assert!(
        DomainEvent::LocaleEnabled {
            tenant_id,
            locale: "pt-BR".to_string(),
        }
        .validate()
        .is_ok()
    );
    for locale in ["pt_br", "PT-br", "und"] {
        assert!(
            DomainEvent::LocaleDisabled {
                tenant_id,
                locale: locale.to_string(),
            }
            .validate()
            .is_err(),
            "{locale} must not cross the event boundary"
        );
    }
}

#[test]
fn test_effective_policy_revision_transition_is_predecessor_bound() {
    let event = DomainEvent::ModuleEffectivePolicyRevisionChanged {
        consumer_key: "runtime.node-1".to_string(),
        previous_revision: Some(format!("sha256:{}", "a".repeat(64))),
        next_revision: format!("sha256:{}", "b".repeat(64)),
    };

    assert_eq!(
        event.event_type(),
        "module.effective_policy_revision_changed"
    );
    assert_eq!(event.schema_version(), 1);
    assert!(event.validate().is_ok());
}

#[test]
fn test_effective_policy_revision_transition_rejects_noop() {
    let revision = format!("sha256:{}", "a".repeat(64));
    let event = DomainEvent::ModuleEffectivePolicyRevisionChanged {
        consumer_key: "runtime.node-1".to_string(),
        previous_revision: Some(revision.clone()),
        next_revision: revision,
    };

    assert!(event.validate().is_err());
}

#[test]
fn platform_scoped_module_event_accepts_the_root_tenant_sentinel() {
    let envelope = EventEnvelope::new(
        Uuid::nil(),
        None,
        DomainEvent::ModuleArtifactAdmitted {
            installation_id: Uuid::new_v4(),
            artifact_digest: "sha256:artifact".to_string(),
            media_type: "application/vnd.rustok.wasm.component.v1+wasm".to_string(),
            size_bytes: 1,
        },
    );

    assert!(envelope.validate_registered_schema().is_ok());
}

#[test]
fn platform_scoped_build_event_accepts_the_root_tenant_sentinel() {
    let envelope = EventEnvelope::new(
        Uuid::nil(),
        None,
        DomainEvent::BuildRequested {
            build_id: Uuid::new_v4(),
            requested_by: "operator".to_string(),
        },
    );

    assert!(envelope.validate_registered_schema().is_ok());
}

#[test]
fn tenant_scoped_module_event_rejects_the_root_tenant_sentinel() {
    let tenant_id = Uuid::new_v4();
    let envelope = EventEnvelope::new(
        Uuid::nil(),
        None,
        DomainEvent::ModuleBuildQueued {
            request_id: Uuid::new_v4(),
            tenant_id,
            project_id: "module-build-test".to_string(),
            attempt: 1,
        },
    );

    assert!(matches!(
        envelope.validate_registered_schema(),
        Err(EventEnvelopeError::Validation(
            EventValidationError::NilUuid("tenant_id")
        ))
    ));
}
