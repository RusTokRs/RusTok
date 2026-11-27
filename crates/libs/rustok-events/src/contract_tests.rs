use uuid::Uuid;

#[test]
fn crate_api_defines_minimal_contract_sections() {
    let api = include_str!("../CRATE_API.md");
    for marker in [
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
fn registry_covers_every_published_event_family_without_duplicates() {
    use std::collections::HashSet;

    let mut seen = HashSet::new();
    for schema in crate::event_schemas() {
        assert!(
            seen.insert(schema.event_type),
            "duplicate event schema registered for `{}`",
            schema.event_type
        );
        assert_eq!(
            crate::event_schema(schema.event_type).map(|found| found.event_type),
            Some(schema.event_type),
            "schema `{}` must be findable through event_schema()",
            schema.event_type
        );
    }
    // Families that publish through the validated durable boundary must stay
    // registered; dropping one silently rejects every publish at runtime.
    for required in [
        "forms.submission.received",
        "module.artifact_node.reconciliation_requested",
        "module.artifact_node.assignment_observed",
        "module.artifact_node.reconciliation_status_changed",
        "module.guest.event_emitted",
    ] {
        assert!(
            seen.contains(required),
            "event schema `{required}` must stay registered"
        );
    }
}

#[test]
fn artifact_node_and_guest_root_events_validate_against_the_registry() {
    use crate::{DomainEvent, EventEnvelope};

    let requested = EventEnvelope::new(
        Uuid::nil(),
        None,
        DomainEvent::ModuleArtifactNodeReconciliationRequested {
            reconciliation_id: Uuid::new_v4(),
            predecessor_reconciliation_id: None,
            reconciliation_revision: 1,
            reconciliation_state_revision: 1,
            topology_digest:
                "sha256:e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855"
                    .to_string(),
            policy_revision:
                "sha256:e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855"
                    .to_string(),
            target_assignments: 1,
        },
    );
    requested
        .validate_registered_schema()
        .expect("reconciliation request must validate against its registered schema");

    // Guest events are emitted for both tenant and platform scopes; the
    // platform scope uses the nil tenant sentinel and must not fail closed.
    let guest = EventEnvelope::new(
        Uuid::nil(),
        None,
        DomainEvent::ModuleGuestEventEmitted {
            module_slug: "guest-module".to_string(),
            topic: "guest.topic".to_string(),
            payload: serde_json::json!({ "value": 1 }),
        },
    );
    guest
        .validate_registered_schema()
        .expect("platform-scoped guest event must validate against its registered schema");

    let oversized = EventEnvelope::new(
        Uuid::new_v4(),
        None,
        DomainEvent::ModuleGuestEventEmitted {
            module_slug: "guest-module".to_string(),
            topic: "guest.topic".to_string(),
            payload: serde_json::Value::String("x".repeat(128 * 1024)),
        },
    );
    assert!(
        oversized.validate_registered_schema().is_err(),
        "oversized guest payloads must fail closed before the durable boundary"
    );
}

#[test]
fn contract_envelope_rejects_nil_envelope_id() {
    let tenant_id = Uuid::new_v4();
    let event =
        crate::RbacRoleMutationEvent::user_role_assignment_repaired(Uuid::new_v4(), "admin", 1);
    let result =
        crate::ContractEventEnvelope::new_with_envelope_id(Uuid::nil(), tenant_id, None, event);
    assert!(matches!(
        result,
        Err(crate::EventContractEnvelopeError::Validation(
            crate::EventValidationError::NilUuid("id")
        ))
    ));
}

#[test]
fn contract_envelope_rejects_nil_causation_id() {
    let tenant_id = Uuid::new_v4();
    let event =
        crate::RbacRoleMutationEvent::user_role_assignment_repaired(Uuid::new_v4(), "admin", 1);
    let result = crate::ContractEventEnvelope::new_caused_by(tenant_id, None, Uuid::nil(), event);
    assert!(matches!(
        result,
        Err(crate::EventContractEnvelopeError::Validation(
            crate::EventValidationError::NilUuid("causation_id")
        ))
    ));
}
