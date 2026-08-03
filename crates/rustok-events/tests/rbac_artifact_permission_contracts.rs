use rustok_events::{
    ContractEventEnvelope, ContractEventPayload, RBAC_ARTIFACT_PERMISSION_EVENT_SCHEMAS,
    RbacArtifactPermissionEvent, ValidateEvent, event_schema,
};
use uuid::Uuid;

fn event() -> RbacArtifactPermissionEvent {
    RbacArtifactPermissionEvent::AssignmentChanged {
        operation_id: Uuid::new_v4(),
        artifact_permission_id: Uuid::new_v4(),
        role_id: Uuid::new_v4(),
        installation_id: Uuid::new_v4(),
        permission_key: "sample.events.handle".to_string(),
        granted: true,
    }
}

#[test]
fn rbac_artifact_permission_family_has_one_registered_contract() {
    assert_eq!(RBAC_ARTIFACT_PERMISSION_EVENT_SCHEMAS.len(), 1);
    let schema = event_schema("rbac.artifact_role_permission.assignment_changed")
        .expect("registered RBAC artifact permission schema");
    assert_eq!(schema.version, 1);
    assert_eq!(schema.fields.len(), 6);
}

#[test]
fn rbac_artifact_permission_contract_is_typed_validated_and_enveloped() {
    let tenant_id = Uuid::new_v4();
    let actor_id = Uuid::new_v4();
    let event = event();
    event.validate().expect("valid event");

    let envelope = ContractEventEnvelope::new(tenant_id, Some(actor_id), event.clone())
        .expect("valid typed envelope");
    assert_eq!(
        envelope.event_type(),
        "rbac.artifact_role_permission.assignment_changed"
    );
    assert_eq!(envelope.schema_version(), 1);
    assert_eq!(envelope.tenant_id(), tenant_id);
    assert_eq!(
        envelope.payload().expect("registered payload"),
        &ContractEventPayload::RbacArtifactPermission(event)
    );
}

#[test]
fn rbac_artifact_permission_contract_rejects_invalid_identity_and_key() {
    assert!(
        RbacArtifactPermissionEvent::AssignmentChanged {
            operation_id: Uuid::nil(),
            artifact_permission_id: Uuid::new_v4(),
            role_id: Uuid::new_v4(),
            installation_id: Uuid::new_v4(),
            permission_key: "sample.events.handle".to_string(),
            granted: true,
        }
        .validate()
        .is_err()
    );
    assert!(
        RbacArtifactPermissionEvent::AssignmentChanged {
            operation_id: Uuid::new_v4(),
            artifact_permission_id: Uuid::nil(),
            role_id: Uuid::new_v4(),
            installation_id: Uuid::new_v4(),
            permission_key: "sample.events.handle".to_string(),
            granted: true,
        }
        .validate()
        .is_err()
    );
    assert!(
        RbacArtifactPermissionEvent::AssignmentChanged {
            operation_id: Uuid::new_v4(),
            artifact_permission_id: Uuid::new_v4(),
            role_id: Uuid::new_v4(),
            installation_id: Uuid::new_v4(),
            permission_key: " sample.events.handle".to_string(),
            granted: true,
        }
        .validate()
        .is_err()
    );
}
