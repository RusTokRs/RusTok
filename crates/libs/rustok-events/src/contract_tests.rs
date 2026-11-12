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
fn contract_envelope_rejects_nil_envelope_id() {
    let tenant_id = uuid::Uuid::new_v4();
    let event = crate::RbacRoleMutationEvent::UserRoleAssignmentRepaired {
        user_id: uuid::Uuid::new_v4(),
        role: "admin".to_string(),
        generation: 1,
    };
    let err = crate::ContractEventEnvelope::new_with_envelope_id(
        uuid::Uuid::nil(),
        tenant_id,
        None,
        event,
    );
    assert!(err.is_err());
}

#[test]
fn contract_envelope_rejects_nil_causation_id() {
    let tenant_id = uuid::Uuid::new_v4();
    let event = crate::RbacRoleMutationEvent::UserRoleAssignmentRepaired {
        user_id: uuid::Uuid::new_v4(),
        role: "admin".to_string(),
        generation: 1,
    };
    let err =
        crate::ContractEventEnvelope::new_caused_by(tenant_id, None, uuid::Uuid::nil(), event);
    assert!(err.is_err());
}
