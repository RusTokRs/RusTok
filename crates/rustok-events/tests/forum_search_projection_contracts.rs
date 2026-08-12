use rustok_events::{
    ContractEventEnvelope, ContractEventPayload, ForumSearchProjectionEvent, ValidateEvent,
    event_schema,
};
use uuid::Uuid;

#[test]
fn forum_search_projection_contract_roundtrips_with_root_causation() {
    let tenant_id = Uuid::new_v4();
    let actor_id = Uuid::new_v4();
    let root_event_id = Uuid::new_v4();
    let target_id = Uuid::new_v4();
    let event = ForumSearchProjectionEvent::InvalidationIssued {
        owner_revision: 7,
        target_type: "forum_topic".to_string(),
        target_id: Some(target_id),
    };
    event.validate().expect("valid Forum Search invalidation");

    let envelope =
        ContractEventEnvelope::new_caused_by(tenant_id, Some(actor_id), root_event_id, event)
            .expect("registered caused contract envelope");
    assert_eq!(
        envelope.event_type(),
        "forum.search_projection.invalidation_issued"
    );
    assert_eq!(envelope.schema_version(), 1);
    assert_eq!(envelope.tenant_id(), tenant_id);
    assert_eq!(envelope.causation_id(), Some(root_event_id));

    let encoded = serde_json::to_vec(&envelope).expect("serialize envelope");
    let decoded: ContractEventEnvelope =
        serde_json::from_slice(&encoded).expect("deserialize envelope");
    decoded
        .validate_registered_schema()
        .expect("decoded envelope remains registered");
    assert_eq!(decoded.causation_id(), Some(root_event_id));
    assert!(matches!(
        decoded.payload().expect("validated payload"),
        ContractEventPayload::ForumSearchProjection(
            ForumSearchProjectionEvent::InvalidationIssued {
                owner_revision: 7,
                target_type,
                target_id: Some(decoded_target_id),
            }
        ) if target_type == "forum_topic" && *decoded_target_id == target_id
    ));
}

#[test]
fn forum_search_projection_registry_and_scope_validation_are_fail_closed() {
    let schema = event_schema("forum.search_projection.invalidation_issued")
        .expect("Forum Search event must be registered");
    assert_eq!(schema.version, 1);
    assert_eq!(schema.fields.len(), 3);

    for invalid in [
        ForumSearchProjectionEvent::InvalidationIssued {
            owner_revision: 0,
            target_type: "forum".to_string(),
            target_id: None,
        },
        ForumSearchProjectionEvent::InvalidationIssued {
            owner_revision: 1,
            target_type: "forum".to_string(),
            target_id: Some(Uuid::new_v4()),
        },
        ForumSearchProjectionEvent::InvalidationIssued {
            owner_revision: 1,
            target_type: "forum_category".to_string(),
            target_id: None,
        },
        ForumSearchProjectionEvent::InvalidationIssued {
            owner_revision: 1,
            target_type: "forum_topic".to_string(),
            target_id: Some(Uuid::nil()),
        },
        ForumSearchProjectionEvent::InvalidationIssued {
            owner_revision: 1,
            target_type: "reply".to_string(),
            target_id: Some(Uuid::new_v4()),
        },
    ] {
        assert!(invalid.validate().is_err());
    }
}
